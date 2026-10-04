use poko_core::{Level, Note, NoteError, parse_note};

const GAIN: &str = "learning/画像処理/カメラ/露出/ゲイン.md";

#[test]
fn reads_id_and_path_classification() {
    let markdown = "---\nid: gain\nsources:\n  - https://example.com/camera-gain\n---\n# ゲイン\n\n## 初級\n- 項目\n";
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
    assert_eq!(count(&note, Level::Beginner), 1);
}

#[test]
fn gain_note_has_id_and_item_counts() {
    let markdown = include_str!("fixtures/vault/learning/画像処理/カメラ/露出/ゲイン.md");
    let note = parse_note(GAIN, markdown).unwrap();

    assert_eq!(note.id, "gain");
    assert_eq!(count(&note, Level::Beginner), 1);
    assert_eq!(count(&note, Level::Intermediate), 2);
    assert_eq!(count(&note, Level::Advanced), 1);
    assert_eq!(count(&note, Level::Integration), 0);
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

#[test]
fn unknown_heading_is_an_error() {
    let markdown = "---\nid: gain\n---\n# ゲイン\n\n## 初級\n- 項目\n\n## おまけ\n- 項目\n";
    let err = parse_note(GAIN, markdown).unwrap_err();
    assert_eq!(err, NoteError::UnknownHeading("おまけ".into()));
}

fn count(note: &Note, level: Level) -> usize {
    note.sections
        .iter()
        .find(|section| section.level == level)
        .map(|section| section.items.len())
        .unwrap_or(0)
}
