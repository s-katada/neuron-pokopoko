use poko_core::{TaxonRank, TaxonomyError, load, parse_note, validate};

const TAXONOMY: &str = include_str!("fixtures/vault/learning/taxonomy.yaml");
const GAIN: &str = "learning/画像処理/カメラ/露出/ゲイン.md";

#[test]
fn gain_path_matches_fixture_taxonomy() {
    let taxonomy = load(TAXONOMY).unwrap();
    assert_eq!(validate(GAIN, &taxonomy), Ok(()));
}

#[test]
fn undefined_minor_names_the_missing_category() {
    let taxonomy = load(TAXONOMY).unwrap();
    let err = validate("learning/画像処理/QR/露出/ゲイン.md", &taxonomy).unwrap_err();
    assert_eq!(
        err,
        TaxonomyError::Undefined {
            level: TaxonRank::Minor,
            name: "露出".into(),
        }
    );
}

#[test]
fn nfd_path_matches_after_nfc() {
    let ge_nfd = "\u{30B1}\u{3099}";
    let path = format!("learning/画像処理/カメラ/露出/{ge_nfd}イン.md");
    let taxonomy = load(TAXONOMY).unwrap();
    assert_eq!(validate(&path, &taxonomy), Ok(()));

    let markdown = "---\nid: gain\n---\n# ゲイン\n\n## 初級\n- 文である。\n";
    let note = parse_note(&path, markdown).unwrap();
    assert_eq!(note.concept, "ゲイン");
}
