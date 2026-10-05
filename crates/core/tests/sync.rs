use poko_core::{ManifestEntry, SyncNote, parse_note, plan};

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
