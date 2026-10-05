use std::fs;
use std::path::PathBuf;

use poko_core::{IntegrationRefReason, LintKind, lint_vault};

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
fn filename_id_mismatch_names_both() {
    let report = lint_vault(&fixture("bad-vaults/filename-id-mismatch")).unwrap();
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].kind, LintKind::FilenameIdMismatch);
    assert_eq!(report.findings[0].detail, "gain != other");
}

#[test]
fn undefined_taxonomy_names_the_minor() {
    let report = lint_vault(&fixture("bad-vaults/undefined-taxonomy")).unwrap();
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].kind, LintKind::UndefinedTaxonomy);
    assert!(report.findings[0].detail.contains("unknown"));
}

#[test]
fn bad_path_segment_names_the_segment() {
    let report = lint_vault(&fixture("bad-vaults/bad-path-segment")).unwrap();
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].kind, LintKind::BadSegment);
    assert_eq!(report.findings[0].detail, "Image-processing");
}

#[test]
fn clean_vaults_have_no_integration_findings() {
    let gain = lint_vault(&fixture("vault")).unwrap();
    assert!(gain.findings.is_empty());
    let policy = lint_vault(&fixture("vault-policy")).unwrap();
    assert!(policy.findings.is_empty());
    assert_eq!(policy.notes, 6);
    let integration = lint_vault(&fixture("vault-integration")).unwrap();
    assert!(integration.findings.is_empty());
    assert_eq!(integration.notes, 3);
    assert_eq!(integration.cards, 8);
}

#[test]
fn integration_ref_rules_name_the_reason() {
    assert_eq!(
        reasons("integration-count"),
        vec![
            (IntegrationRefReason::Count, "0 件".to_owned()),
            (IntegrationRefReason::Count, "3 件".to_owned()),
        ]
    );
    assert_eq!(
        reasons("integration-missing"),
        vec![(
            IntegrationRefReason::Missing,
            "存在しない: ghost".to_owned()
        )]
    );
    assert_eq!(
        reasons("integration-other-class"),
        vec![(
            IntegrationRefReason::OtherClass,
            "別の分類: here".to_owned()
        )]
    );
    assert_eq!(
        reasons("integration-self"),
        vec![(IntegrationRefReason::SelfRef, "自ノート: mine".to_owned())]
    );
    assert_eq!(
        reasons("integration-duplicate"),
        vec![(IntegrationRefReason::Duplicate, "重複: peer".to_owned())]
    );
}

fn reasons(name: &str) -> Vec<(IntegrationRefReason, String)> {
    lint_vault(&fixture(&format!("bad-vaults/{name}")))
        .unwrap()
        .findings
        .into_iter()
        .map(|finding| match finding.kind {
            LintKind::IntegrationRef(reason) => (reason, finding.detail),
            other => panic!("{other:?}: {}", finding.detail),
        })
        .collect()
}

#[test]
fn missing_frontmatter_and_shallow_path() {
    let tmp = Tmp::new("shape");
    tmp.write("learning/taxonomy.yaml", TAXONOMY);
    tmp.write(
        "learning/image-processing/camera/exposure/gain.md",
        "本文だけ\n",
    );
    tmp.write("learning/shallow.md", NOTE);
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
