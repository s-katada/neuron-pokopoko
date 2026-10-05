use poko_core::{Level, Note, NoteError, parse_note};

const GAIN: &str = "learning/image-processing/camera/exposure/gain.md";

#[test]
fn reads_id_and_path_classification() {
    let markdown = "---\nid: gain\nsources:\n  - https://example.com/camera-gain\n---\n# ゲイン\n\n## 初級\n- 項目\n";
    let note = parse_note(GAIN, markdown).unwrap();

    assert_eq!(note.id, "gain");
    assert!(note.aliases.is_empty());
    assert_eq!(
        note.sources,
        vec!["https://example.com/camera-gain".to_owned()]
    );
    assert_eq!(note.major, "image-processing");
    assert_eq!(note.middle, "camera");
    assert_eq!(note.minor, "exposure");
    assert_eq!(note.concept, "gain");
    assert_eq!(count(&note, Level::Beginner), 1);
}

#[test]
fn gain_note_has_id_and_item_counts() {
    let markdown = include_str!("fixtures/vault/learning/image-processing/camera/exposure/gain.md");
    let note = parse_note(GAIN, markdown).unwrap();

    assert_eq!(note.id, "gain");
    assert_eq!(note.aliases, vec!["ゲイン".to_owned()]);
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
fn filename_must_match_id() {
    let markdown = "---\nid: other\n---\n# ゲイン\n\n## 初級\n- 項目\n";
    let err = parse_note(GAIN, markdown).unwrap_err();
    assert_eq!(
        err,
        NoteError::FilenameIdMismatch {
            filename: "gain".into(),
            id: "other".into(),
        }
    );
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
