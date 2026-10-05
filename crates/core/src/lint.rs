use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use thiserror::Error;

use crate::note::{self, Level, Note, NoteError};
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
    FilenameIdMismatch,
    BadSegment,
    UndefinedTaxonomy,
    Frontmatter,
    BadLayout,
    IntegrationRef(IntegrationRefReason),
}

/// 統合カードの参照が、どの規則に反したか。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IntegrationRefReason {
    /// 件数が 1〜2 でない。
    Count,
    /// 参照先の id が vault に無い。
    Missing,
    /// 参照先が別の分類(major/middle/minor)。
    OtherClass,
    /// 自ノートを参照している。
    SelfRef,
    /// 同じ id を 2 回以上参照している。
    Duplicate,
}

impl std::fmt::Display for LintKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::DuplicateId => "id の重複",
            Self::FilenameIdMismatch => "ファイル名が id と一致していない",
            Self::BadSegment => "パス規則",
            Self::UndefinedTaxonomy => "taxonomy 未定義",
            Self::Frontmatter => "frontmatter",
            Self::BadLayout => "配置",
            Self::IntegrationRef(_) => "統合カードの参照",
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
    let paths = crate::vault::learning_markdown(root)?;

    let mut findings = Vec::new();
    let mut notes = 0;
    let mut cards = 0;
    let mut ids: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut parsed: Vec<(String, Note)> = Vec::new();

    for path in paths {
        let rel = crate::vault::relative(root, &path);
        match note::parse_location(&rel) {
            Err(NoteError::InvalidSegment(name)) => {
                push(&mut findings, &rel, LintKind::BadSegment, &name);
                continue;
            }
            Err(_) => {
                push(&mut findings, &rel, LintKind::BadLayout, "3 階層ではない");
                continue;
            }
            Ok(_) => {}
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
                ids.entry(note.id.clone()).or_default().push(rel.clone());
                parsed.push((rel, note));
            }
            Err(
                err @ (NoteError::MissingFrontmatter
                | NoteError::InvalidFrontmatter(_)
                | NoteError::MissingId),
            ) => {
                push(&mut findings, &rel, LintKind::Frontmatter, &err.to_string());
            }
            Err(NoteError::FilenameIdMismatch { filename, id }) => {
                push(
                    &mut findings,
                    &rel,
                    LintKind::FilenameIdMismatch,
                    &format!("{filename} != {id}"),
                );
            }
            Err(_) => {}
        }
    }

    push_integration_refs(&mut findings, &parsed);
    push_dups(&mut findings, ids, LintKind::DuplicateId);
    findings.sort_by(|a, b| (&a.path, a.kind, &a.detail).cmp(&(&b.path, b.kind, &b.detail)));
    Ok(Report {
        findings,
        notes,
        cards,
    })
}

fn push_integration_refs(findings: &mut Vec<Finding>, notes: &[(String, Note)]) {
    let mut by_id: BTreeMap<&str, Vec<&Note>> = BTreeMap::new();
    for (_, note) in notes {
        by_id.entry(note.id.as_str()).or_default().push(note);
    }
    for (path, note) in notes {
        for card in note.cards() {
            if card.level != Level::Integration {
                continue;
            }
            for (reason, detail) in ref_problems(note, &card.refs, &by_id) {
                push(findings, path, LintKind::IntegrationRef(reason), &detail);
            }
        }
    }
}

fn ref_problems(
    note: &Note,
    refs: &[String],
    by_id: &BTreeMap<&str, Vec<&Note>>,
) -> Vec<(IntegrationRefReason, String)> {
    let mut problems = Vec::new();
    if !(1..=2).contains(&refs.len()) {
        problems.push((IntegrationRefReason::Count, format!("{} 件", refs.len())));
    }
    let mut seen = BTreeMap::<&str, usize>::new();
    for id in refs {
        *seen.entry(id).or_default() += 1;
    }
    for id in seen.keys() {
        if *id == note.id {
            continue;
        }
        if by_id.get(id).is_none_or(|targets| targets.is_empty()) {
            problems.push((IntegrationRefReason::Missing, format!("存在しない: {id}")));
        }
    }
    for id in seen.keys() {
        if *id == note.id {
            continue;
        }
        let Some(targets) = by_id.get(id) else {
            continue;
        };
        if targets.is_empty() {
            continue;
        }
        let same = targets.iter().any(|target| same_class(note, target));
        if !same {
            problems.push((IntegrationRefReason::OtherClass, format!("別の分類: {id}")));
        }
    }
    if seen.contains_key(note.id.as_str()) {
        problems.push((
            IntegrationRefReason::SelfRef,
            format!("自ノート: {}", note.id),
        ));
    }
    for (id, count) in &seen {
        if *count >= 2 {
            problems.push((IntegrationRefReason::Duplicate, format!("重複: {id}")));
        }
    }
    problems
}

fn same_class(note: &Note, other: &Note) -> bool {
    note.major == other.major && note.middle == other.middle && note.minor == other.minor
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
