use poko_core::{TaxonRank, TaxonomyError, load, validate};

const TAXONOMY: &str = include_str!("fixtures/vault/learning/taxonomy.yaml");
const GAIN: &str = "learning/image-processing/camera/exposure/gain.md";

#[test]
fn gain_path_matches_fixture_taxonomy() {
    let taxonomy = load(TAXONOMY).unwrap();
    assert_eq!(validate(GAIN, &taxonomy), Ok(()));
}

#[test]
fn undefined_minor_names_the_missing_category() {
    let taxonomy = load(TAXONOMY).unwrap();
    let err = validate("learning/image-processing/qr/exposure/gain.md", &taxonomy).unwrap_err();
    assert_eq!(
        err,
        TaxonomyError::Undefined {
            level: TaxonRank::Minor,
            name: "exposure".into(),
        }
    );
}

#[test]
fn non_kebab_path_is_a_rule_violation() {
    let ge_nfd = "\u{30B1}\u{3099}";
    let stem = format!("{ge_nfd}in");
    let path = format!("learning/image-processing/camera/exposure/{stem}.md");
    let taxonomy = load(TAXONOMY).unwrap();
    assert_eq!(
        validate(&path, &taxonomy),
        Err(TaxonomyError::InvalidSegment(stem))
    );
}
