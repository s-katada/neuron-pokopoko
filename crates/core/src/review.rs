use crate::day::{NEW_CARDS_PER_DAY, study_day_start};
use crate::sync::{Statement, Value};

const NEXT_CARD: &str = "\
WITH intake AS (
  SELECT count(*) AS cnt FROM (
    SELECT card_key FROM reviews GROUP BY card_key HAVING min(reviewed_at) >= ?
  )
),
due AS (
  SELECT c.stable_key, c.note_id, nt.title, c.question, c.answer, c.level,
         0 AS pri, c.due_at AS ord1, c.stable_key AS ord2
  FROM cards c JOIN notes nt ON nt.id = c.note_id
  WHERE c.retired_at IS NULL AND nt.deleted_at IS NULL
    AND c.level = 'beginner' AND c.fsrs_state = 'review' AND c.due_at <= ?
),
fresh AS (
  SELECT c.stable_key, c.note_id, nt.title, c.question, c.answer, c.level,
         1 AS pri, c.created_at AS ord1, c.stable_key AS ord2
  FROM cards c JOIN notes nt ON nt.id = c.note_id
  WHERE c.retired_at IS NULL AND nt.deleted_at IS NULL
    AND c.level = 'beginner' AND c.fsrs_state = 'new'
    AND (SELECT cnt FROM intake) < ?
)
SELECT stable_key, note_id, title, question, answer, level
FROM (SELECT * FROM due UNION ALL SELECT * FROM fresh)
ORDER BY pri, ord1, ord2
LIMIT 1";

/// 次に出すカードを 1 件選ぶ。params は学習日の開始、now、新規枠。
pub fn next_card_query(now_unix: i64) -> Statement {
    Statement {
        sql: NEXT_CARD.to_owned(),
        params: vec![
            Value::Integer(study_day_start(now_unix)),
            Value::Integer(now_unix),
            Value::Integer(i64::try_from(NEW_CARDS_PER_DAY).expect("新規枠は i64 に収まる")),
        ],
    }
}
