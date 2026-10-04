use std::fs;
use std::path::PathBuf;

use poko_core::{LintKind, lint_vault};

const TAXONOMY: &str = include_str!("fixtures/vault/learning/taxonomy.yaml");
const NOTE: &str = "---\nid: gain\n---\n\n## 初級\n";

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn gain_vault_has_no_findings() {
    let report = lint_vault(&fixture("vault")).unwrap();
    assert!(report.findings.is_empty());
    assert_eq!(report.notes, 1);
    assert_eq!(report.cards, 4);
}

#[test]
fn duplicate_id_names_the_id() {
    let report = lint_vault(&fixture("bad-vaults/duplicate-id")).unwrap();
    assert_eq!(report.findings.len(), 2);
    assert!(
        report
            .findings
            .iter()
            .all(|finding| { finding.kind == LintKind::DuplicateId && finding.detail == "dup" })
    );
}

#[test]
fn duplicate_filename_names_the_concept() {
    let report = lint_vault(&fixture("bad-vaults/duplicate-filename")).unwrap();
    assert_eq!(report.findings.len(), 2);
    assert!(report.findings.iter().all(|finding| {
        finding.kind == LintKind::DuplicateFilename && finding.detail == "ゲイン"
    }));
}

#[test]
fn undefined_taxonomy_names_the_minor() {
    let report = lint_vault(&fixture("bad-vaults/undefined-taxonomy")).unwrap();
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].kind, LintKind::UndefinedTaxonomy);
    assert!(report.findings[0].detail.contains("未定義"));
}

#[test]
fn nfd_filename_is_non_nfc() {
    let tmp = Tmp::new("nfd");
    tmp.write("learning/taxonomy.yaml", TAXONOMY);
    let ge = "\u{30B1}\u{3099}";
    tmp.write(&format!("learning/画像処理/カメラ/露出/{ge}イン.md"), NOTE);
    let report = lint_vault(&tmp.0).unwrap();
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.kind == LintKind::NonNfc)
    );
}

#[test]
fn missing_frontmatter_and_shallow_path() {
    let tmp = Tmp::new("shape");
    tmp.write("learning/taxonomy.yaml", TAXONOMY);
    tmp.write("learning/画像処理/カメラ/露出/ゲイン.md", "本文だけ\n");
    tmp.write("learning/直置き.md", NOTE);
    let report = lint_vault(&tmp.0).unwrap();
    assert!(report.findings.iter().any(|finding| {
        finding.kind == LintKind::Frontmatter && finding.detail.contains("frontmatter")
    }));
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.kind == LintKind::BadLayout)
    );
}

struct Tmp(PathBuf);

impl Tmp {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!("poko-lint-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        Self(path)
    }

    fn write(&self, rel: &str, body: &str) {
        let path = self.0.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }
}
