use std::path::PathBuf;

mod common;

use common::{poko, serve_seq};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("poko-import-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn line(key: &str, at: i64, rating: &str) -> String {
    format!(
        r#"{{"card_key":"{key}","rating":"{rating}","reviewed_at":{at},"interval_days":1.0,"stability":1.0,"difficulty":1.0,"response":null}}"#
    )
}

fn ok_body(inserted: usize) -> String {
    format!(r#"{{"inserted":{inserted},"duplicates":0,"missing":[]}}"#)
}

#[test]
fn import_splits_at_two_hundred_rows() {
    let (url, seen) = serve_seq(vec![(200, ok_body(200)), (200, ok_body(1))]);
    let dir = scratch("split");
    let file = dir.join("reviews.jsonl");
    let mut text = String::new();
    for at in 1..=201 {
        text.push_str(&line("a", at, "good"));
        text.push('\n');
        if at == 1 {
            text.push('\n');
        }
    }
    std::fs::write(&file, text).unwrap();
    let assert = poko()
        .args(["import", file.to_str().unwrap(), "--endpoint", &url])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert_eq!(stdout, "imported 201, duplicates 0, missing 0\n");
    let hits = seen.lock().unwrap();
    assert_eq!(hits.len(), 2);
    assert!(hits.iter().all(|hit| hit.path == "/api/import/reviews"));
    assert_eq!(hits[0].body.matches("\"card_key\"").count(), 200);
    assert_eq!(hits[1].body.matches("\"card_key\"").count(), 1);
}

#[test]
fn import_sends_the_first_of_a_duplicate_pair() {
    let (url, seen) = serve_seq(vec![(200, ok_body(1))]);
    let dir = scratch("dup");
    let file = dir.join("reviews.jsonl");
    let row = line("a", 1, "good");
    std::fs::write(&file, format!("{row}\n{row}\n")).unwrap();
    poko()
        .args(["import", file.to_str().unwrap(), "--endpoint", &url])
        .assert()
        .success()
        .stdout("imported 1, duplicates 1, missing 0\n");
    assert_eq!(
        seen.lock().unwrap()[0].body.matches("\"card_key\"").count(),
        1
    );
}

#[test]
fn import_sends_nothing_when_a_line_is_invalid() {
    let (url, seen) = serve_seq(vec![(200, ok_body(1))]);
    let dir = scratch("bad");
    let file = dir.join("reviews.jsonl");
    std::fs::write(
        &file,
        format!("{}\n\n{}\n", line("a", 1, "good"), line("a", 2, "nope")),
    )
    .unwrap();
    let assert = poko()
        .args(["import", file.to_str().unwrap(), "--endpoint", &url])
        .assert()
        .code(1);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(stderr.contains("3: rating が不正"), "{stderr}");
    assert!(seen.lock().unwrap().is_empty());
}

#[test]
fn import_exits_one_when_cards_are_missing() {
    let (url, seen) = serve_seq(vec![(
        200,
        r#"{"inserted":1,"duplicates":0,"missing":["gone"]}"#.to_owned(),
    )]);
    let dir = scratch("missing");
    let file = dir.join("reviews.jsonl");
    std::fs::write(&file, format!("{}\n", line("a", 1, "good"))).unwrap();
    let assert = poko()
        .args(["import", file.to_str().unwrap(), "--endpoint", &url])
        .assert()
        .code(1);
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(stdout, "imported 1, duplicates 0, missing 1\n");
    assert!(stderr.contains("gone"), "{stderr}");
    assert_eq!(seen.lock().unwrap().len(), 1);
}
