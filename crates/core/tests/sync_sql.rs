use poko_core::{SyncCard, SyncNote, Value, delete_statements, upsert_statements};
use rusqlite::{Connection, params_from_iter};

mod common;

fn db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    common::apply_migrations(&conn);
    conn
}

fn apply(conn: &Connection, statements: &[poko_core::Statement]) {
    let tx = conn.unchecked_transaction().unwrap();
    for statement in statements {
        let params: Vec<rusqlite::types::Value> = statement.params.iter().map(to_sql).collect();
        tx.prepare(&statement.sql)
            .unwrap()
            .execute(params_from_iter(params))
            .unwrap();
    }
    tx.commit().unwrap();
}

fn to_sql(value: &Value) -> rusqlite::types::Value {
    match value {
        Value::Null => rusqlite::types::Value::Null,
        Value::Integer(n) => rusqlite::types::Value::Integer(*n),
        Value::Real(n) => rusqlite::types::Value::Real(*n),
        Value::Text(text) => rusqlite::types::Value::Text(text.clone()),
    }
}

fn count(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
        row.get(0)
    })
    .unwrap()
}

fn counts(conn: &Connection) -> (i64, i64, i64) {
    (
        count(conn, "notes"),
        count(conn, "cards"),
        count(conn, "reviews"),
    )
}

fn card(key: &str, answer: &str) -> SyncCard {
    SyncCard {
        stable_key: key.into(),
        level: "beginner".into(),
        question: key.into(),
        answer: answer.into(),
        rubric: None,
        refs: vec!["露光".into()],
    }
}

fn gain(cards: Vec<SyncCard>) -> SyncNote {
    SyncNote {
        id: "gain".into(),
        path: "learning/image-processing/camera/exposure/gain.md".into(),
        title: "ゲイン".into(),
        major: "image-processing".into(),
        middle: "camera".into(),
        minor: "exposure".into(),
        content_hash: "hash".into(),
        cards,
    }
}

#[test]
fn resend_does_not_change_row_counts_and_new_cards_are_not_due() {
    let conn = db();
    let note = gain(vec![card("gain/a", "答え"), card("gain/b", "別")]);
    let batch = upsert_statements(&[&note], 100);
    apply(&conn, &batch);
    let before = counts(&conn);
    apply(&conn, &batch);
    assert_eq!(before, counts(&conn));
    assert_eq!(before, (1, 2, 0));
    let fresh: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cards WHERE fsrs_state = 'new' AND due_at IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(fresh, 2);
}

#[test]
fn answer_edit_keeps_fsrs_columns_and_reviews() {
    let conn = db();
    let note = gain(vec![card("gain/a", "答え")]);
    apply(&conn, &upsert_statements(&[&note], 100));
    conn.execute(
        "UPDATE cards SET fsrs_state = 'review', stability = 1.5, difficulty = 2.0, due_at = 50 WHERE stable_key = 'gain/a'",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO reviews (card_key, rating, reviewed_at, interval_days, stability, difficulty) VALUES ('gain/a', 'good', 10, 1.0, 1.5, 2.0)",
        [],
    )
    .unwrap();
    let edited = gain(vec![card("gain/a", "直した答え")]);
    apply(&conn, &upsert_statements(&[&edited], 200));
    let row: (String, String, f64, i64) = conn
        .query_row(
            "SELECT answer, fsrs_state, stability, due_at FROM cards WHERE stable_key = 'gain/a'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(row.0, "直した答え");
    assert_eq!(row.1, "review");
    assert_eq!(row.2, 1.5);
    assert_eq!(row.3, 50);
    assert_eq!(count(&conn, "reviews"), 1);
}

#[test]
fn removed_question_retires_only_that_card() {
    let conn = db();
    let note = gain(vec![card("gain/a", "答え"), card("gain/b", "別")]);
    apply(&conn, &upsert_statements(&[&note], 100));
    let kept = gain(vec![card("gain/a", "答え")]);
    apply(&conn, &upsert_statements(&[&kept], 300));
    let retired: Option<i64> = conn
        .query_row(
            "SELECT retired_at FROM cards WHERE stable_key = 'gain/b'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let live: Option<i64> = conn
        .query_row(
            "SELECT retired_at FROM cards WHERE stable_key = 'gain/a'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(retired, Some(300));
    assert_eq!(live, None);
}

#[test]
fn delete_marks_note_and_cards_and_upsert_restores_them() {
    let conn = db();
    let note = gain(vec![card("gain/a", "答え")]);
    apply(&conn, &upsert_statements(&[&note], 100));
    conn.execute(
        "INSERT INTO reviews (card_key, rating, reviewed_at, interval_days, stability, difficulty) VALUES ('gain/a', 'good', 10, 1.0, 1.5, 2.0)",
        [],
    )
    .unwrap();
    let ids = vec!["gain".to_owned()];
    apply(&conn, &delete_statements(&ids, 400));
    let deleted: Option<i64> = conn
        .query_row(
            "SELECT deleted_at FROM notes WHERE id = 'gain'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let retired: Option<i64> = conn
        .query_row(
            "SELECT retired_at FROM cards WHERE stable_key = 'gain/a'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(deleted, Some(400));
    assert_eq!(retired, Some(400));
    apply(&conn, &upsert_statements(&[&note], 500));
    let deleted: Option<i64> = conn
        .query_row(
            "SELECT deleted_at FROM notes WHERE id = 'gain'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let retired: Option<i64> = conn
        .query_row(
            "SELECT retired_at FROM cards WHERE stable_key = 'gain/a'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(deleted, None);
    assert_eq!(retired, None);
    assert_eq!(count(&conn, "reviews"), 1);
}
