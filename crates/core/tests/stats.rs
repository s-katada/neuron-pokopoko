use poko_core::{
    StatCard, SyncCard, SyncNote, Value, build_tree, daily_counts, daily_reviews_query,
    default_decay, note_detail, note_stat_cards_query, retention, stat_cards_query,
    study_day_start, upsert_statements,
};
use rusqlite::{Connection, params_from_iter};

mod common;

const NOW: i64 = 1_791_140_400;

#[test]
fn retention_is_point_nine_when_elapsed_equals_stability() {
    let stability = 10.0;
    let reviewed_at = NOW - (stability as i64) * 86_400;
    let got = retention(stability, reviewed_at, NOW, default_decay());
    assert!((got - 0.9).abs() < 1e-4, "{got}");
}

#[test]
fn retention_is_one_when_no_time_has_elapsed() {
    let got = retention(4.0, NOW, NOW, default_decay());
    assert!((got - 1.0).abs() < 1e-4, "{got}");
}

#[test]
fn negative_elapsed_is_clamped_to_zero() {
    let got = retention(10.0, NOW + 86_400, NOW, default_decay());
    assert!((got - 1.0).abs() < 1e-4, "{got}");
}

#[test]
fn retention_matches_an_independent_fsrs6_calculation() {
    // Python で式だけを別計算した値。decay = 0.1542、
    // factor = 0.9 ** (1 / -decay) - 1、R = (30 / 10 * factor + 1) ** (-decay)。
    let reviewed_at = NOW - 30 * 86_400;
    let got = retention(10.0, reviewed_at, NOW, default_decay());
    let expected = 0.809_388_103_573_170_8;
    assert!((got - expected).abs() < 1e-4, "{got}");
}

#[test]
fn a_different_decay_changes_retention() {
    let reviewed_at = NOW - 30 * 86_400;
    let base = retention(10.0, reviewed_at, NOW, default_decay());
    let changed = retention(10.0, reviewed_at, NOW, 0.5);
    assert!((base - changed).abs() > 1e-3, "{base} {changed}");
}

#[test]
fn tree_retention_is_the_mean_of_cards_not_of_children() {
    let mut cards = vec![
        reviewed("a1", "beta", "rev-beta", 0, "露光とは？"),
        fresh("a1", "beta", "new-beta", "未学習の質問"),
        reviewed("z9", "alpha", "rev-a", 10, "一つ目"),
        reviewed("z9", "alpha", "rev-b", 10, "二つ目"),
        reviewed("z9", "alpha", "rev-c", 10, "三つ目"),
    ];
    let mut untouched = fresh("n-new", "gamma", "new-1", "触らない");
    untouched.major = "b".into();
    let mut untouched_again = fresh("n-new", "gamma", "new-2", "まだ触らない");
    untouched_again.major = "b".into();
    cards.insert(0, untouched);
    cards.push(untouched_again);

    let tree = build_tree(&cards, NOW, default_decay());
    let learned = &tree.majors[0];
    let minor = &learned.middles[0].minors[0];
    let parent = minor.retention.expect("対象カードがある");
    let child_mean =
        (minor.notes[0].retention.expect("alpha") + minor.notes[1].retention.expect("beta")) / 2.0;

    assert_eq!(learned.major, "a");
    assert_eq!(tree.majors[1].major, "b");
    assert_eq!(minor.notes[0].title, "alpha");
    assert_eq!(minor.notes[0].note_id, "z9");
    assert_eq!(minor.notes[1].title, "beta");
    assert_eq!(minor.notes[1].note_id, "a1");
    assert_eq!(minor.notes[1].card_count, 2);
    assert!((minor.notes[1].retention.expect("beta") - 1.0).abs() < 1e-4);
    assert!((parent - 0.925).abs() < 1e-4, "{parent}");
    assert!((child_mean - 0.95).abs() < 1e-4, "{child_mean}");
    assert!((parent - child_mean).abs() > 1e-3);
    assert!((learned.middles[0].retention.expect("middle") - parent).abs() < 1e-4);
    assert!((learned.retention.expect("major") - parent).abs() < 1e-4);
    assert!((tree.overall.expect("overall") - parent).abs() < 1e-4);

    let unlearned = &tree.majors[1];
    assert!(unlearned.retention.is_none());
    assert_eq!(unlearned.card_count, 2);
    assert!(unlearned.middles[0].minors[0].notes[0].retention.is_none());
    assert_eq!(unlearned.middles[0].minors[0].notes[0].card_count, 2);
}

#[test]
fn note_detail_skips_retention_until_the_card_is_reviewed() {
    let cards = vec![
        reviewed("a1", "beta", "rev-beta", 0, "露光とは？"),
        fresh("a1", "beta", "new-beta", "未学習の質問"),
    ];
    let detail = note_detail(&cards, "a1", NOW, default_decay()).expect("ノートがある");
    assert_eq!(detail.title, "beta");
    let new_card = detail
        .cards
        .iter()
        .find(|card| card.fsrs_state == "new")
        .expect("未学習");
    let review = detail
        .cards
        .iter()
        .find(|card| card.fsrs_state == "review")
        .expect("復習");
    assert!(new_card.retention.is_none());
    assert!(new_card.due_at.is_none());
    assert_eq!(new_card.question, "未学習の質問");
    assert!((review.retention.expect("定着率") - 1.0).abs() < 1e-4);
    assert_eq!(review.due_at, Some(NOW + 86_400));
    assert_eq!(review.level, "beginner");
    assert_eq!(review.question, "露光とは？");
    assert!(note_detail(&cards, "missing", NOW, default_decay()).is_none());
}

#[test]
fn daily_counts_split_at_four_jst_and_keep_empty_days() {
    let at_0359 = 1_791_140_399;
    let at_0400 = 1_791_140_400;
    let counts = daily_counts(&[at_0359, at_0400, at_0400 + 1], at_0400, 3);
    assert_eq!(counts.len(), 3);
    assert_eq!(counts[0].count, 0);
    assert_eq!(counts[0].day_start, study_day_start(at_0400) - 2 * 86_400);
    assert_eq!(counts[1].day_start, study_day_start(at_0359));
    assert_eq!(counts[1].count, 1);
    assert_eq!(counts[2].day_start, study_day_start(at_0400));
    assert_eq!(counts[2].count, 2);
    assert_ne!(counts[1].day_start, counts[2].day_start);
    assert_eq!(daily_counts(&[], at_0400, 30).len(), 30);
}

#[test]
fn stat_queries_skip_retired_cards_and_deleted_notes() {
    let conn = Connection::open_in_memory().unwrap();
    common::apply_migrations(&conn);
    apply(
        &conn,
        &upsert_statements(
            &[
                &note("keep", "残す", "keep", &["beginner", "intermediate"]),
                &note("gone", "消す", "gone", &["beginner"]),
            ],
            NOW,
        ),
    );
    conn.execute(
        "UPDATE cards SET retired_at = ?1 WHERE stable_key = 'keep/intermediate'",
        [NOW],
    )
    .unwrap();
    conn.execute("UPDATE notes SET deleted_at = ?1 WHERE id = 'gone'", [NOW])
        .unwrap();
    let start = study_day_start(NOW) - 86_400;
    conn.execute(
        "INSERT INTO reviews (card_key, rating, reviewed_at, interval_days, stability, difficulty) VALUES ('keep/beginner', 'good', ?1, 1.0, 1.0, 5.0)",
        [start - 1],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO reviews (card_key, rating, reviewed_at, interval_days, stability, difficulty) VALUES ('keep/beginner', 'good', ?1, 1.0, 1.0, 5.0)",
        [NOW],
    )
    .unwrap();

    let cards = query_cards(&conn, &stat_cards_query());
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].note_id, "keep");
    assert_eq!(cards[0].title, "残す");
    assert_eq!(cards[0].major, "image-processing");
    assert_eq!(cards[0].middle, "camera");
    assert_eq!(cards[0].minor, "exposure");
    assert_eq!(cards[0].level, "beginner");
    assert_eq!(cards[0].fsrs_state, "new");
    assert_eq!(cards[0].stable_key, "keep/beginner");
    assert_eq!(cards[0].question, "q-beginner");
    assert_eq!(cards[0].stability, None);
    assert_eq!(cards[0].last_reviewed_at, None);
    assert_eq!(cards[0].due_at, None);

    assert_eq!(query_cards(&conn, &note_stat_cards_query("keep")).len(), 1);
    assert!(query_cards(&conn, &note_stat_cards_query("gone")).is_empty());
    assert!(query_cards(&conn, &note_stat_cards_query("missing")).is_empty());

    let statement = daily_reviews_query(NOW, 2);
    let params: Vec<_> = statement.params.iter().map(to_sql).collect();
    let mut stmt = conn.prepare(&statement.sql).unwrap();
    let mut rows = stmt.query(params_from_iter(params)).unwrap();
    let mut reviewed_at = Vec::new();
    while let Some(row) = rows.next().unwrap() {
        reviewed_at.push(row.get::<_, i64>("reviewed_at").unwrap());
    }
    assert_eq!(reviewed_at, vec![NOW]);
}

fn note(id: &str, title: &str, key_prefix: &str, levels: &[&str]) -> SyncNote {
    SyncNote {
        id: id.into(),
        path: format!("learning/image-processing/camera/exposure/{id}.md"),
        title: title.into(),
        major: "image-processing".into(),
        middle: "camera".into(),
        minor: "exposure".into(),
        content_hash: "hash".into(),
        cards: levels
            .iter()
            .map(|level| SyncCard {
                stable_key: format!("{key_prefix}/{level}"),
                level: (*level).into(),
                question: format!("q-{level}"),
                answer: format!("a-{level}"),
                rubric: None,
                refs: vec![],
            })
            .collect(),
    }
}

fn apply(conn: &Connection, statements: &[poko_core::Statement]) {
    for statement in statements {
        let params: Vec<_> = statement.params.iter().map(to_sql).collect();
        conn.prepare(&statement.sql)
            .unwrap()
            .execute(params_from_iter(params))
            .unwrap();
    }
}

fn to_sql(value: &Value) -> rusqlite::types::Value {
    match value {
        Value::Null => rusqlite::types::Value::Null,
        Value::Integer(n) => rusqlite::types::Value::Integer(*n),
        Value::Real(n) => rusqlite::types::Value::Real(*n),
        Value::Text(text) => rusqlite::types::Value::Text(text.clone()),
    }
}

fn query_cards(conn: &Connection, statement: &poko_core::Statement) -> Vec<StatCard> {
    let params: Vec<_> = statement.params.iter().map(to_sql).collect();
    let mut stmt = conn.prepare(&statement.sql).unwrap();
    let mut rows = stmt.query(params_from_iter(params)).unwrap();
    let mut cards = Vec::new();
    while let Some(row) = rows.next().unwrap() {
        cards.push(StatCard {
            major: row.get("major").unwrap(),
            middle: row.get("middle").unwrap(),
            minor: row.get("minor").unwrap(),
            note_id: row.get("note_id").unwrap(),
            title: row.get("title").unwrap(),
            level: row.get("level").unwrap(),
            fsrs_state: row.get("fsrs_state").unwrap(),
            stability: row.get("stability").unwrap(),
            last_reviewed_at: row.get("last_reviewed_at").unwrap(),
            due_at: row.get("due_at").unwrap(),
            stable_key: row.get("stable_key").unwrap(),
            question: row.get("question").unwrap(),
        });
    }
    cards
}

fn reviewed(note_id: &str, title: &str, key: &str, elapsed_days: i64, question: &str) -> StatCard {
    card(note_id, title, key, "review", Some(elapsed_days), question)
}

fn fresh(note_id: &str, title: &str, key: &str, question: &str) -> StatCard {
    card(note_id, title, key, "new", None, question)
}

fn card(
    note_id: &str,
    title: &str,
    key: &str,
    fsrs_state: &str,
    elapsed_days: Option<i64>,
    question: &str,
) -> StatCard {
    StatCard {
        major: "a".into(),
        middle: "m".into(),
        minor: "s".into(),
        note_id: note_id.into(),
        title: title.into(),
        level: "beginner".into(),
        fsrs_state: fsrs_state.into(),
        stability: elapsed_days.map(|_| 10.0),
        last_reviewed_at: elapsed_days.map(|days| NOW - days * 86_400),
        due_at: elapsed_days.map(|_| NOW + 86_400),
        stable_key: key.into(),
        question: question.into(),
    }
}
