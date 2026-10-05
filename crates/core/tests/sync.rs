use std::path::PathBuf;

use poko_core::{
    MAX_BIND_PARAMS, ManifestEntry, SyncCard, SyncError, SyncNote, chunk, chunk_deletes, collect,
    delete_statements, parse_note, plan, statements_for, upsert_statements,
};

const GAIN_PATH: &str = "learning/image-processing/camera/exposure/gain.md";

fn from_markdown(markdown: &str) -> SyncNote {
    SyncNote::from_note(&parse_note(GAIN_PATH, markdown).unwrap())
}

#[test]
fn gain_fixture_is_a_sync_note() {
    let markdown = include_str!("fixtures/vault/learning/image-processing/camera/exposure/gain.md");
    let note = from_markdown(markdown);
    assert_eq!(note.title, "ゲイン");
    assert_eq!(note.cards.len(), 4);
    assert_eq!(note.path, GAIN_PATH);
    assert_eq!(note.content_hash.len(), 64);
}

#[test]
fn hash_ignores_blank_lines_and_changes_with_answer() {
    let base = "---\nid: gain\n---\n# ゲイン\n\n## 初級\n- 答えである。\n  - Q: 質問？\n";
    let spaced = "---\nid: gain\n---\n# ゲイン\n\n\n## 初級\n\n- 答えである。\n  - Q: 質問？\n";
    let changed = "---\nid: gain\n---\n# ゲイン\n\n## 初級\n- 別の答えである。\n  - Q: 質問？\n";
    let base = from_markdown(base);
    assert_eq!(base.content_hash, from_markdown(spaced).content_hash);
    assert_ne!(base.content_hash, from_markdown(changed).content_hash);
}

#[test]
fn plan_splits_upsert_delete_and_unchanged() {
    let same = sample("same", "h1");
    let edited = sample("edited", "h2");
    let added = sample("added", "h3");
    let local = [same, edited, added];
    let remote = [
        ManifestEntry {
            id: "same".into(),
            content_hash: "h1".into(),
        },
        ManifestEntry {
            id: "edited".into(),
            content_hash: "old".into(),
        },
        ManifestEntry {
            id: "gone".into(),
            content_hash: "h4".into(),
        },
    ];
    let diff = plan(&local, &remote);
    let upserts: Vec<_> = diff.upserts.iter().map(|note| note.id.as_str()).collect();
    assert_eq!(upserts, ["edited", "added"]);
    assert_eq!(diff.deletes, ["gone"]);
    assert_eq!(diff.unchanged, 1);
}

#[test]
fn chunk_splits_when_statements_pass_forty() {
    let fits: Vec<_> = (0..2).map(|i| sized(&format!("n{i}"), 18)).collect();
    let refs: Vec<_> = fits.iter().collect();
    let chunks = chunk(&refs).unwrap();
    assert_eq!(statements_for(&fits[0]) + statements_for(&fits[1]), 40);
    assert_eq!(chunks.len(), 1);

    let overflow = [sized("big", 19), sized("rest", 18)];
    let refs: Vec<_> = overflow.iter().collect();
    let chunks = chunk(&refs).unwrap();
    assert_eq!(
        statements_for(&overflow[0]) + statements_for(&overflow[1]),
        41
    );
    assert_eq!(chunks.len(), 2);
}

#[test]
fn chunk_rejects_a_note_with_thirty_nine_cards() {
    let note = sized("huge", 39);
    assert_eq!(statements_for(&note), 41);
    let err = chunk(&[&note]).unwrap_err();
    match err {
        SyncError::NoteTooLarge { id } => assert_eq!(id, "huge"),
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn chunk_deletes_splits_ninety_one_ids() {
    let ids: Vec<_> = (0..91).map(|i| i.to_string()).collect();
    let chunks = chunk_deletes(&ids);
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0].len(), 90);
    assert_eq!(chunks[1].len(), 1);
}

#[test]
fn collect_reads_the_gain_fixture() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vault");
    let notes = collect(&root).unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].title, "ゲイン");
    assert_eq!(notes[0].cards.len(), 4);
    assert_eq!(notes[0].path, GAIN_PATH);
}

#[test]
fn generated_statements_match_statements_for_and_bind_limit() {
    let note = sized("gain", 2);
    let batch = upsert_statements(&[&note], 10);
    assert_eq!(batch.len(), statements_for(&note));
    assert!(
        batch
            .iter()
            .all(|statement| statement.params.len() <= MAX_BIND_PARAMS)
    );
    assert!(batch[0].sql.contains("INSERT INTO notes"));
    assert!(batch[1].sql.contains("INSERT INTO cards"));
    assert!(batch[3].sql.contains("NOT IN (?, ?)"));
    let empty = sized("empty", 0);
    let batch = upsert_statements(&[&empty], 10);
    assert_eq!(batch.len(), statements_for(&empty));
    assert!(!batch[1].sql.contains("NOT IN"));
    let ids = vec!["gain".into(), "empty".into()];
    let deleted = delete_statements(&ids, 10);
    assert_eq!(deleted.len(), 2);
    assert!(
        deleted
            .iter()
            .all(|statement| statement.params.len() <= MAX_BIND_PARAMS)
    );
    assert!(deleted[0].sql.contains("id IN (?, ?)"));
    assert!(deleted[1].sql.contains("note_id IN (?, ?)"));
}

fn sized(id: &str, cards: usize) -> SyncNote {
    let mut note = sample(id, "hash");
    note.cards = (0..cards)
        .map(|index| SyncCard {
            stable_key: format!("{id}/{index}"),
            level: "beginner".into(),
            question: format!("q{index}"),
            answer: format!("a{index}"),
            rubric: None,
            refs: Vec::new(),
        })
        .collect();
    note
}

fn sample(id: &str, hash: &str) -> SyncNote {
    SyncNote {
        id: id.into(),
        path: format!("learning/image-processing/camera/exposure/{id}.md"),
        title: id.into(),
        major: "image-processing".into(),
        middle: "camera".into(),
        minor: "exposure".into(),
        content_hash: hash.into(),
        cards: Vec::new(),
    }
}
