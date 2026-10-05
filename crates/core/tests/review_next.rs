use poko_core::{
    CardState, NEW_CARDS_PER_DAY, Rating, SyncCard, SyncNote, Value, answer_statements,
    card_state_query, next_card_query, study_day_start, upsert_statements,
};
use rusqlite::{Connection, OptionalExtension, params_from_iter};

/// 2026-10-05 04:00:00 JST
const NOW: i64 = 1_791_140_400;

const SPIKE: &str = include_str!("../../worker/migrations/0001_spike.sql");
const INIT: &str = include_str!("../../worker/migrations/0002_init.sql");

struct Drawn {
    stable_key: String,
    note_id: String,
    title: String,
    level: String,
}

fn db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    conn.execute_batch(SPIKE).unwrap();
    conn.execute_batch(INIT).unwrap();
    let mut cards: Vec<_> = (1..=7)
        .map(|n| card(&format!("b{n}"), "beginner"))
        .collect();
    cards.push(card("mid", "intermediate"));
    let note = SyncNote {
        id: "gain".into(),
        path: "learning/image-processing/camera/exposure/gain.md".into(),
        title: "ゲイン".into(),
        major: "image-processing".into(),
        middle: "camera".into(),
        minor: "exposure".into(),
        content_hash: "hash".into(),
        cards,
    };
    apply(&conn, &upsert_statements(&[&note], NOW));
    conn
}

fn card(key: &str, level: &str) -> SyncCard {
    SyncCard {
        stable_key: key.into(),
        level: level.into(),
        question: format!("q-{key}"),
        answer: format!("a-{key}"),
        rubric: None,
        refs: vec![],
    }
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

fn draw(conn: &Connection, now: i64) -> Option<Drawn> {
    let statement = next_card_query(now);
    let params: Vec<_> = statement.params.iter().map(to_sql).collect();
    conn.query_row(&statement.sql, params_from_iter(params), |row| {
        Ok(Drawn {
            stable_key: row.get(0)?,
            note_id: row.get(1)?,
            title: row.get(2)?,
            level: row.get(5)?,
        })
    })
    .optional()
    .unwrap()
}

fn introduce(conn: &Connection, key: &str, at: i64) {
    conn.execute(
        "INSERT INTO reviews (card_key, rating, reviewed_at, interval_days, stability, difficulty) VALUES (?1, 'good', ?2, 1.0, 1.0, 5.0)",
        (key, at),
    )
    .unwrap();
}

fn load_state(conn: &Connection, key: &str) -> CardState {
    let statement = card_state_query(key);
    let params: Vec<_> = statement.params.iter().map(to_sql).collect();
    let row = conn
        .query_row(&statement.sql, params_from_iter(params), |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap();
    CardState {
        fsrs_state: row.0,
        stability: row.1,
        difficulty: row.2,
        last_reviewed_at: row.3,
    }
}

fn answer(conn: &Connection, key: &str, rating: Rating) {
    let state = load_state(conn, key);
    let planned = answer_statements(key, &state, rating, NOW).unwrap();
    apply(conn, &planned.statements);
}

#[test]
fn good_answer_schedules_review_and_draws_the_next_new_card() {
    let conn = db();
    let state = load_state(&conn, "b1");
    assert_eq!(state.fsrs_state, "new");
    let planned = answer_statements("b1", &state, Rating::Good, NOW).unwrap();
    let expected_due = NOW + (f64::from(planned.interval_days) * 86_400.0).round() as i64;
    assert_eq!(planned.due_at, expected_due);
    assert!(planned.due_at > NOW);
    apply(&conn, &planned.statements);
    let (fsrs_state, due_at): (String, i64) = conn
        .query_row(
            "SELECT fsrs_state, due_at FROM cards WHERE stable_key = 'b1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(fsrs_state, "review");
    assert_eq!(due_at, planned.due_at);
    assert_eq!(draw(&conn, NOW).unwrap().stable_key, "b2");
}

#[test]
fn answer_appends_one_review_and_increments_reps() {
    let conn = db();
    answer(&conn, "b1", Rating::Good);
    let reviews: i64 = conn
        .query_row("SELECT count(*) FROM reviews", [], |row| row.get(0))
        .unwrap();
    assert_eq!(reviews, 1);
    let row: (String, String, i64) = conn
        .query_row(
            "SELECT card_key, rating, reviewed_at FROM reviews",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(row, ("b1".into(), "good".into(), NOW));
    let reps: i64 = conn
        .query_row(
            "SELECT reps FROM cards WHERE stable_key = 'b1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(reps, 1);
}

#[test]
fn again_increments_lapses_only_after_the_card_is_in_review() {
    let conn = db();
    answer(&conn, "b1", Rating::Again);
    let lapses: i64 = conn
        .query_row(
            "SELECT lapses FROM cards WHERE stable_key = 'b1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(lapses, 0);
    answer(&conn, "b1", Rating::Again);
    let lapses: i64 = conn
        .query_row(
            "SELECT lapses FROM cards WHERE stable_key = 'b1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(lapses, 1);
}

#[test]
fn no_reviews_draws_the_first_new_beginner() {
    let conn = db();
    let statement = next_card_query(NOW);
    assert_eq!(
        statement.params,
        vec![
            Value::Integer(study_day_start(NOW)),
            Value::Integer(NOW),
            Value::Integer(i64::try_from(NEW_CARDS_PER_DAY).unwrap()),
        ]
    );
    let drawn = draw(&conn, NOW).unwrap();
    assert_eq!(drawn.stable_key, "b1");
    assert_eq!(drawn.note_id, "gain");
    assert_eq!(drawn.title, "ゲイン");
    assert_eq!(drawn.level, "beginner");
}

#[test]
fn five_introductions_hide_remaining_new_cards() {
    let conn = db();
    for n in 1..=5 {
        introduce(&conn, &format!("b{n}"), NOW);
    }
    let fresh: i64 = conn
        .query_row(
            "SELECT count(*) FROM cards WHERE level = 'beginner' AND fsrs_state = 'new'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(fresh, 7);
    assert!(draw(&conn, NOW).is_none());
}

#[test]
fn next_study_day_draws_a_new_card_again() {
    let conn = db();
    for n in 1..=5 {
        introduce(&conn, &format!("b{n}"), NOW);
    }
    let morning = study_day_start(NOW) + 86_400;
    let drawn = draw(&conn, morning).unwrap();
    assert_eq!(drawn.stable_key, "b1");
    assert_eq!(drawn.level, "beginner");
}

#[test]
fn due_review_comes_before_a_new_card() {
    let conn = db();
    conn.execute(
        "UPDATE cards SET fsrs_state = 'review', due_at = ?1 WHERE stable_key = 'b3'",
        [NOW],
    )
    .unwrap();
    let drawn = draw(&conn, NOW).unwrap();
    assert_eq!(drawn.stable_key, "b3");
}

#[test]
fn retired_and_intermediate_cards_are_never_drawn() {
    let conn = db();
    conn.execute(
        "UPDATE cards SET retired_at = ?1, fsrs_state = 'review', due_at = ?1 WHERE stable_key = 'b1'",
        [NOW],
    )
    .unwrap();
    conn.execute(
        "UPDATE cards SET fsrs_state = 'review', due_at = ?1 WHERE stable_key = 'mid'",
        [NOW],
    )
    .unwrap();
    assert_eq!(draw(&conn, NOW).unwrap().stable_key, "b2");
    conn.execute(
        "UPDATE cards SET retired_at = ?1 WHERE level = 'beginner'",
        [NOW],
    )
    .unwrap();
    assert!(draw(&conn, NOW).is_none());
}
