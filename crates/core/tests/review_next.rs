use poko_core::{
    CardState, NEW_CARDS_PER_DAY, Rating, SyncCard, SyncNote, UNLOCK_ADVANCED_DAYS,
    UNLOCK_INTERMEDIATE_DAYS, Value, answer_statements, card_state_query, next_card_query,
    study_day_start, upsert_statements,
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

fn park(conn: &Connection, level: &str, stability: f64) {
    conn.execute(
        "UPDATE cards SET fsrs_state = 'review', stability = ?1, due_at = ?2 WHERE level = ?3 AND retired_at IS NULL",
        (stability, NOW + 86_400, level),
    )
    .unwrap();
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
fn good_answer_schedules_review_and_hides_same_note_cards() {
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
    assert!(draw(&conn, NOW).is_none());
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
            Value::Real(UNLOCK_INTERMEDIATE_DAYS),
            Value::Real(UNLOCK_ADVANCED_DAYS),
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
fn retired_beginner_is_skipped_and_due_intermediate_is_drawn() {
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
    assert_eq!(draw(&conn, NOW).unwrap().stable_key, "mid");
    conn.execute(
        "UPDATE cards SET retired_at = ?1 WHERE level = 'beginner'",
        [NOW],
    )
    .unwrap();
    assert_eq!(draw(&conn, NOW).unwrap().stable_key, "mid");
}

#[test]
fn intermediate_new_waits_for_beginner_stability() {
    let conn = db();
    park(&conn, "beginner", 6.99);
    assert!(draw(&conn, NOW).is_none());
    park(&conn, "beginner", 7.0);
    let drawn = draw(&conn, NOW).unwrap();
    assert_eq!(drawn.stable_key, "mid");
    assert_eq!(drawn.level, "intermediate");
}

#[test]
fn advanced_new_waits_for_intermediate_stability() {
    let conn = db();
    park(&conn, "beginner", 7.0);
    park(&conn, "intermediate", 20.99);
    conn.execute(
        "INSERT INTO cards (stable_key, note_id, level, question, answer, refs, created_at, updated_at) VALUES ('adv', 'gain', 'advanced', 'q-adv', 'a-adv', '[]', ?1, ?1)",
        [NOW],
    )
    .unwrap();
    assert!(draw(&conn, NOW).is_none());
    park(&conn, "intermediate", 21.0);
    let drawn = draw(&conn, NOW).unwrap();
    assert_eq!(drawn.stable_key, "adv");
    assert_eq!(drawn.level, "advanced");
}

#[test]
fn note_without_beginners_draws_intermediate_new() {
    let conn = db();
    conn.execute(
        "UPDATE cards SET retired_at = ?1 WHERE level = 'beginner'",
        [NOW],
    )
    .unwrap();
    let drawn = draw(&conn, NOW).unwrap();
    assert_eq!(drawn.stable_key, "mid");
    assert_eq!(drawn.level, "intermediate");
}

#[test]
fn new_card_quota_counts_across_levels() {
    let conn = db();
    conn.execute("UPDATE cards SET retired_at = ?1", [NOW])
        .unwrap();
    push_note(
        &conn,
        "beg",
        &[
            ("b-a", "beginner"),
            ("b-b", "beginner"),
            ("b-c", "beginner"),
            ("b-left", "beginner"),
        ],
    );
    push_note(
        &conn,
        "midn",
        &[
            ("m-a", "intermediate"),
            ("m-b", "intermediate"),
            ("m-left", "intermediate"),
        ],
    );
    push_note(
        &conn,
        "advn",
        &[("m-done", "intermediate"), ("a-left", "advanced")],
    );
    conn.execute(
        "UPDATE cards SET fsrs_state = 'review', stability = 21.0, due_at = ?1 WHERE stable_key = 'm-done'",
        [NOW + 86_400],
    )
    .unwrap();
    for key in ["b-a", "b-b", "b-c", "m-a"] {
        introduce(&conn, key, NOW);
    }
    assert!(draw(&conn, NOW).is_some());
    introduce(&conn, "m-b", NOW);
    assert!(draw(&conn, NOW).is_none());
    let mut levels: Vec<String> = conn
        .prepare("SELECT DISTINCT level FROM cards WHERE retired_at IS NULL AND fsrs_state = 'new' ORDER BY level")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(|level| level.unwrap())
        .collect();
    levels.sort();
    assert_eq!(
        levels,
        vec![
            "advanced".to_owned(),
            "beginner".to_owned(),
            "intermediate".to_owned()
        ]
    );
}

fn push_note(conn: &Connection, id: &str, keys: &[(&str, &str)]) {
    push_cat(
        conn,
        id,
        "image-processing",
        "camera",
        "exposure",
        keys,
        NOW,
    );
}

fn push_cat(
    conn: &Connection,
    id: &str,
    major: &str,
    middle: &str,
    minor: &str,
    keys: &[(&str, &str)],
    at: i64,
) {
    let note = SyncNote {
        id: id.into(),
        path: format!("learning/{major}/{middle}/{minor}/{id}.md"),
        title: id.into(),
        major: major.into(),
        middle: middle.into(),
        minor: minor.into(),
        content_hash: "hash".into(),
        cards: keys.iter().map(|(key, level)| card(key, level)).collect(),
    };
    apply(conn, &upsert_statements(&[&note], at));
}

#[test]
fn answering_hides_the_sibling_new_card_until_the_next_study_day() {
    let conn = db();
    conn.execute("UPDATE cards SET retired_at = ?1", [NOW])
        .unwrap();
    push_note(&conn, "sib", &[("a", "beginner"), ("b", "beginner")]);
    push_note(&conn, "other", &[("o", "beginner")]);
    answer(&conn, "a", Rating::Good);
    assert_eq!(draw(&conn, NOW).unwrap().stable_key, "o");

    answer(&conn, "a", Rating::Again);
    conn.execute("UPDATE cards SET due_at = ?1 WHERE stable_key = 'a'", [NOW])
        .unwrap();
    assert_eq!(draw(&conn, NOW).unwrap().stable_key, "a");

    conn.execute(
        "UPDATE cards SET due_at = ?1 WHERE stable_key = 'a'",
        [NOW + 86_400 * 10],
    )
    .unwrap();
    let morning = study_day_start(NOW) + 86_400;
    assert_eq!(draw(&conn, morning).unwrap().stable_key, "b");
}

#[test]
fn answering_a_due_card_hides_a_due_sibling() {
    let conn = db();
    conn.execute("UPDATE cards SET retired_at = ?1", [NOW])
        .unwrap();
    push_note(&conn, "sib", &[("a", "beginner"), ("c", "intermediate")]);
    push_note(&conn, "other", &[("o", "beginner")]);
    conn.execute(
        "UPDATE cards SET fsrs_state = 'review', due_at = ?1 WHERE stable_key IN ('a', 'c', 'o')",
        [NOW],
    )
    .unwrap();
    assert_eq!(draw(&conn, NOW).unwrap().stable_key, "a");
    answer(&conn, "a", Rating::Good);
    assert_eq!(draw(&conn, NOW).unwrap().stable_key, "o");
}

#[test]
fn due_intermediate_review_ignores_a_locked_beginner() {
    let conn = db();
    park(&conn, "beginner", 6.99);
    conn.execute(
        "UPDATE cards SET fsrs_state = 'review', due_at = ?1, stability = 1.0 WHERE stable_key = 'mid'",
        [NOW],
    )
    .unwrap();
    let drawn = draw(&conn, NOW).unwrap();
    assert_eq!(drawn.stable_key, "mid");
    assert_eq!(drawn.level, "intermediate");
}

#[test]
fn answering_in_one_category_draws_the_other_category_next() {
    let conn = db();
    conn.execute("UPDATE cards SET retired_at = ?1", [NOW])
        .unwrap();
    push_cat(
        &conn,
        "x-old",
        "image-processing",
        "camera",
        "exposure",
        &[("xa", "beginner")],
        NOW,
    );
    push_cat(
        &conn,
        "x-rest",
        "image-processing",
        "camera",
        "exposure",
        &[("xb", "beginner")],
        NOW + 1,
    );
    push_cat(
        &conn,
        "y",
        "image-processing",
        "lens",
        "focus",
        &[("yc", "beginner")],
        NOW + 2,
    );
    answer(&conn, "xa", Rating::Good);
    assert_eq!(draw(&conn, NOW).unwrap().stable_key, "yc");
}

#[test]
fn same_category_is_drawn_when_no_other_category_remains() {
    let conn = db();
    conn.execute("UPDATE cards SET retired_at = ?1", [NOW])
        .unwrap();
    push_cat(
        &conn,
        "x-old",
        "image-processing",
        "camera",
        "exposure",
        &[("xa", "beginner")],
        NOW,
    );
    push_cat(
        &conn,
        "x-rest",
        "image-processing",
        "camera",
        "exposure",
        &[("xb", "beginner")],
        NOW + 1,
    );
    answer(&conn, "xa", Rating::Good);
    assert_eq!(draw(&conn, NOW).unwrap().stable_key, "xb");
}

#[test]
fn shared_minor_name_is_still_a_different_category() {
    let conn = db();
    conn.execute("UPDATE cards SET retired_at = ?1", [NOW])
        .unwrap();
    push_cat(
        &conn,
        "p-old",
        "a",
        "b",
        "basics",
        &[("p-old", "beginner")],
        NOW,
    );
    push_cat(
        &conn,
        "p-rest",
        "a",
        "b",
        "basics",
        &[("p-next", "beginner")],
        NOW + 1,
    );
    push_cat(
        &conn,
        "q",
        "c",
        "d",
        "basics",
        &[("q", "beginner")],
        NOW + 2,
    );
    answer(&conn, "p-old", Rating::Good);
    assert_eq!(draw(&conn, NOW).unwrap().stable_key, "q");
}

#[test]
fn different_category_new_card_precedes_same_category_due_review() {
    let conn = db();
    conn.execute("UPDATE cards SET retired_at = ?1", [NOW])
        .unwrap();
    push_cat(
        &conn,
        "x-reviewed",
        "image-processing",
        "camera",
        "exposure",
        &[("xr", "beginner")],
        NOW,
    );
    push_cat(
        &conn,
        "x-due",
        "image-processing",
        "camera",
        "exposure",
        &[("xd", "beginner")],
        NOW,
    );
    push_cat(
        &conn,
        "y-new",
        "image-processing",
        "lens",
        "focus",
        &[("yn", "beginner")],
        NOW + 1,
    );
    conn.execute(
        "UPDATE cards SET fsrs_state = 'review', due_at = ?1 WHERE stable_key = 'xd'",
        [NOW],
    )
    .unwrap();
    answer(&conn, "xr", Rating::Good);
    assert_eq!(draw(&conn, NOW).unwrap().stable_key, "yn");
}

#[test]
fn next_card_returns_rubric_for_an_unlocked_intermediate_and_none_for_a_beginner() {
    let conn = db();
    let (level, rubric) = draw_level_and_rubric(&conn, NOW);
    assert_eq!(level, "beginner");
    assert_eq!(rubric, None);

    park(&conn, "beginner", 7.0);
    conn.execute(
        "UPDATE cards SET rubric = ?1 WHERE stable_key = 'mid'",
        ["被写界深度が浅くなる"],
    )
    .unwrap();
    let (level, rubric) = draw_level_and_rubric(&conn, NOW);
    assert_eq!(level, "intermediate");
    assert_eq!(rubric.as_deref(), Some("被写界深度が浅くなる"));
}

fn draw_level_and_rubric(conn: &Connection, now: i64) -> (String, Option<String>) {
    let statement = next_card_query(now);
    let params: Vec<_> = statement.params.iter().map(to_sql).collect();
    conn.query_row(&statement.sql, params_from_iter(params), |row| {
        Ok((row.get(5)?, row.get(6)?))
    })
    .unwrap()
}
