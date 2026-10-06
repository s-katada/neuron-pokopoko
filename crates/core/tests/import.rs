use poko_core::{
    CardState, Rating, ReviewLog, SyncCard, SyncNote, Value, answer_statements, card_state_query,
    import_reviews_statement, missing_cards_query, rebuild_cards_statement, reviews_json,
    upsert_statements,
};
use rusqlite::{Connection, params_from_iter};

mod common;

const NOW: i64 = 1_791_140_400;

#[derive(Debug, PartialEq)]
struct Fsrs {
    fsrs_state: String,
    stability: f64,
    difficulty: f64,
    due_at: i64,
    last_reviewed_at: i64,
    reps: i64,
    lapses: i64,
}

fn note() -> SyncNote {
    SyncNote {
        id: "gain".into(),
        path: "learning/image-processing/camera/exposure/gain.md".into(),
        title: "ゲイン".into(),
        major: "image-processing".into(),
        middle: "camera".into(),
        minor: "exposure".into(),
        content_hash: "hash".into(),
        cards: vec![card("a"), card("b")],
    }
}

fn card(key: &str) -> SyncCard {
    SyncCard {
        stable_key: key.into(),
        level: "beginner".into(),
        question: format!("q-{key}"),
        answer: format!("a-{key}"),
        rubric: None,
        refs: vec![],
    }
}

fn db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    common::apply_migrations(&conn);
    let note = note();
    apply(&conn, &upsert_statements(&[&note], NOW));
    conn
}

fn apply(conn: &Connection, statements: &[poko_core::Statement]) {
    for statement in statements {
        exec(conn, statement);
    }
}

fn exec(conn: &Connection, statement: &poko_core::Statement) -> usize {
    let params: Vec<_> = statement.params.iter().map(to_sql).collect();
    conn.prepare(&statement.sql)
        .unwrap()
        .execute(params_from_iter(params))
        .unwrap()
}

fn to_sql(value: &Value) -> rusqlite::types::Value {
    match value {
        Value::Null => rusqlite::types::Value::Null,
        Value::Integer(n) => rusqlite::types::Value::Integer(*n),
        Value::Real(n) => rusqlite::types::Value::Real(*n),
        Value::Text(text) => rusqlite::types::Value::Text(text.clone()),
    }
}

fn state(conn: &Connection, key: &str) -> CardState {
    let statement = card_state_query(key);
    let params: Vec<_> = statement.params.iter().map(to_sql).collect();
    conn.query_row(&statement.sql, params_from_iter(params), |row| {
        Ok(CardState {
            fsrs_state: row.get(0)?,
            stability: row.get(1)?,
            difficulty: row.get(2)?,
            last_reviewed_at: row.get(3)?,
        })
    })
    .unwrap()
}

fn answer(conn: &Connection, key: &str, rating: Rating, now: i64, response: Option<&str>) {
    let answered = answer_statements(
        key,
        &state(conn, key),
        rating,
        now,
        response,
        &poko_core::default_parameters(),
    )
    .unwrap();
    apply(conn, &answered.statements);
}

fn fsrs(conn: &Connection, key: &str) -> Fsrs {
    conn.query_row(
        "SELECT fsrs_state, stability, difficulty, due_at, last_reviewed_at, reps, lapses FROM cards WHERE stable_key = ?1",
        [key],
        |row| {
            Ok(Fsrs {
                fsrs_state: row.get(0)?,
                stability: row.get(1)?,
                difficulty: row.get(2)?,
                due_at: row.get(3)?,
                last_reviewed_at: row.get(4)?,
                reps: row.get(5)?,
                lapses: row.get(6)?,
            })
        },
    )
    .unwrap()
}

fn logs(conn: &Connection) -> Vec<ReviewLog> {
    let mut stmt = conn
        .prepare(
            "SELECT card_key, rating, reviewed_at, interval_days, stability, difficulty, response FROM reviews ORDER BY id",
        )
        .unwrap();
    stmt.query_map([], |row| {
        Ok(ReviewLog {
            card_key: row.get(0)?,
            rating: row.get(1)?,
            reviewed_at: row.get(2)?,
            interval_days: row.get(3)?,
            stability: row.get(4)?,
            difficulty: row.get(5)?,
            response: row.get(6)?,
        })
    })
    .unwrap()
    .map(|row| row.unwrap())
    .collect()
}

fn payload(logs: &[ReviewLog]) -> String {
    let json = serde_json::to_string(logs).unwrap();
    let logs: Vec<ReviewLog> = serde_json::from_str(&json).unwrap();
    reviews_json(&logs).unwrap()
}

fn import(conn: &Connection, json: &str, now: i64) -> usize {
    let inserted = exec(conn, &import_reviews_statement(json));
    exec(conn, &rebuild_cards_statement(json, now));
    inserted
}

#[test]
fn import_rebuilds_fsrs_columns_from_answers() {
    let source = db();
    answer(&source, "a", Rating::Good, NOW, None);
    answer(&source, "a", Rating::Again, NOW + 3_600, Some("もう一回"));
    answer(&source, "a", Rating::Hard, NOW + 7_200, None);
    let expected = fsrs(&source, "a");
    assert_eq!(expected.reps, 3);
    assert_eq!(expected.lapses, 1);
    let json = payload(&logs(&source));

    let restored = db();
    assert_eq!(import(&restored, &json, NOW + 10_000), 3);
    assert_eq!(fsrs(&restored, "a"), expected);
    assert_eq!(import(&restored, &json, NOW + 20_000), 0);
    assert_eq!(fsrs(&restored, "a"), expected);
    assert_eq!(logs(&restored).len(), 3);
}

#[test]
fn import_skips_duplicates_and_missing_cards() {
    let conn = db();
    let first = ReviewLog {
        card_key: "a".into(),
        rating: "good".into(),
        reviewed_at: NOW,
        interval_days: 1.0,
        stability: 2.0,
        difficulty: 3.0,
        response: None,
    };
    let again = ReviewLog {
        reviewed_at: NOW + 10,
        ..first.clone()
    };
    let json = payload(&[first.clone(), first.clone(), again.clone(), again]);
    assert_eq!(import(&conn, &json, NOW), 2);
    assert_eq!(logs(&conn).len(), 2);

    let mut gone = first.clone();
    gone.card_key = "gone".into();
    gone.reviewed_at = NOW + 20;
    let mixed = payload(&[first, gone]);
    assert_eq!(import(&conn, &mixed, NOW), 0);
    let missing = missing_cards_query(&mixed);
    let params: Vec<_> = missing.params.iter().map(to_sql).collect();
    let keys: Vec<String> = conn
        .prepare(&missing.sql)
        .unwrap()
        .query_map(params_from_iter(params), |row| row.get(0))
        .unwrap()
        .map(|key| key.unwrap())
        .collect();
    assert_eq!(keys, vec!["gone".to_owned()]);
    assert_eq!(logs(&conn).len(), 2);
}
