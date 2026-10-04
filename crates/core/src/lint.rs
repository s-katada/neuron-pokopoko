use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;
use unicode_normalization::UnicodeNormalization;

use crate::note::{self, NoteError};
use crate::taxonomy::{self, TaxonomyError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub path: String,
    pub kind: LintKind,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LintKind {
    DuplicateId,
    DuplicateFilename,
    UndefinedTaxonomy,
    NonNfc,
    Frontmatter,
    BadLayout,
}

impl std::fmt::Display for LintKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::DuplicateId => "id の重複",
            Self::DuplicateFilename => "ファイル名の重複",
            Self::UndefinedTaxonomy => "taxonomy 未定義",
            Self::NonNfc => "NFC 違反",
            Self::Frontmatter => "frontmatter",
            Self::BadLayout => "配置",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub findings: Vec<Finding>,
    pub notes: usize,
    pub cards: usize,
}

#[derive(Debug, Error)]
pub enum LintError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Taxonomy(#[from] TaxonomyError),
}

pub fn lint_vault(root: &Path) -> Result<Report, LintError> {
    let yaml = fs::read_to_string(root.join("learning/taxonomy.yaml"))?;
    let taxonomy = taxonomy::load(&yaml)?;
    let mut paths = Vec::new();
    walk(&root.join("learning"), &mut paths)?;
    paths.sort();

    let mut findings = Vec::new();
    let mut notes = 0;
    let mut cards = 0;
    let mut ids: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut names: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for path in paths {
        let rel = relative(root, &path);
        if rel != nfc(&rel) {
            push(&mut findings, &rel, LintKind::NonNfc, "NFC ではない");
        }
        let stem = rel.rsplit('/').next().unwrap_or(&rel);
        let stem = stem.strip_suffix(".md").unwrap_or(stem);
        names.entry(nfc(stem)).or_default().push(rel.clone());

        if note::parse_location(&rel).is_err() {
            push(&mut findings, &rel, LintKind::BadLayout, "3 階層ではない");
            continue;
        }
        if let Err(TaxonomyError::Undefined { level, name }) = taxonomy::validate(&rel, &taxonomy) {
            push(
                &mut findings,
                &rel,
                LintKind::UndefinedTaxonomy,
                &format!("未定義の{level}: {name}"),
            );
        }
        match note::parse_note(&rel, &fs::read_to_string(&path)?) {
            Ok(note) => {
                notes += 1;
                cards += note.cards().len();
                ids.entry(note.id).or_default().push(rel);
            }
            Err(
                err @ (NoteError::MissingFrontmatter
                | NoteError::InvalidFrontmatter(_)
                | NoteError::MissingId),
            ) => {
                push(&mut findings, &rel, LintKind::Frontmatter, &err.to_string());
            }
            Err(_) => {}
        }
    }

    push_dups(&mut findings, ids, LintKind::DuplicateId);
    push_dups(&mut findings, names, LintKind::DuplicateFilename);
    findings.sort_by(|a, b| (&a.path, a.kind, &a.detail).cmp(&(&b.path, b.kind, &b.detail)));
    Ok(Report {
        findings,
        notes,
        cards,
    })
}

fn push(findings: &mut Vec<Finding>, path: &str, kind: LintKind, detail: &str) {
    findings.push(Finding {
        path: path.to_owned(),
        kind,
        detail: detail.to_owned(),
    });
}

fn push_dups(findings: &mut Vec<Finding>, groups: BTreeMap<String, Vec<String>>, kind: LintKind) {
    for (detail, paths) in groups {
        if paths.len() < 2 {
            continue;
        }
        for path in paths {
            push(findings, &path, kind, &detail);
        }
    }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), LintError> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            walk(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "md") {
            out.push(path);
        }
    }
    Ok(())
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn nfc(text: &str) -> String {
    text.nfc().collect()
}
