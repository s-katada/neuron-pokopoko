use poko_core::{NoteError, parse_note};

const GAIN: &str = "learning/画像処理/カメラ/露出/ゲイン.md";

#[test]
fn reads_id_and_path_classification() {
    let markdown = "---\nid: gain\nsources:\n  - https://example.com/camera-gain\n---\n# ゲイン\n";
    let note = parse_note(GAIN, markdown).unwrap();

    assert_eq!(note.id, "gain");
    assert_eq!(
        note.sources,
        vec!["https://example.com/camera-gain".to_owned()]
    );
    assert_eq!(note.major, "画像処理");
    assert_eq!(note.middle, "カメラ");
    assert_eq!(note.minor, "露出");
    assert_eq!(note.concept, "ゲイン");
    assert!(note.sections.is_empty());
}

#[test]
fn missing_frontmatter_is_an_error() {
    let err = parse_note(GAIN, "# ゲイン\n").unwrap_err();
    assert_eq!(err, NoteError::MissingFrontmatter);
}

#[test]
fn missing_id_is_an_error() {
    let markdown = "---\nsources:\n  - https://example.com/camera-gain\n---\n# ゲイン\n";
    let err = parse_note(GAIN, markdown).unwrap_err();
    assert_eq!(err, NoteError::MissingId);
}
