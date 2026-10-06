//! JSONL の復習ログを D1 に戻す。行の検証は CLI と Worker で共有する。

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::review::MAX_RESPONSE_CHARS;
use crate::review_log::ReviewLog;
use crate::schedule::Rating;
use crate::sync::{Statement, Value};

/// `POST /api/import/reviews` の 1 回の行数。超えたら 413。
pub const IMPORT_BATCH_MAX: usize = 200;

const MISSING_CARDS_SQL: &str = "\
SELECT json_extract(row.value, '$.card_key') AS card_key
FROM json_each(?) AS row
WHERE NOT EXISTS (
  SELECT 1 FROM cards c
  JOIN notes nt ON nt.id = c.note_id
  WHERE c.stable_key = json_extract(row.value, '$.card_key')
    AND c.retired_at IS NULL AND nt.deleted_at IS NULL
)
ORDER BY row.key";

const IMPORT_REVIEWS_SQL: &str = "\
INSERT INTO reviews (card_key, rating, reviewed_at, interval_days, stability, difficulty, response)
SELECT
  json_extract(row.value, '$.card_key'),
  json_extract(row.value, '$.rating'),
  json_extract(row.value, '$.reviewed_at'),
  json_extract(row.value, '$.interval_days'),
  json_extract(row.value, '$.stability'),
  json_extract(row.value, '$.difficulty'),
  json_extract(row.value, '$.response')
FROM json_each(?) AS row
WHERE EXISTS (
  SELECT 1 FROM cards c
  JOIN notes nt ON nt.id = c.note_id
  WHERE c.stable_key = json_extract(row.value, '$.card_key')
    AND c.retired_at IS NULL AND nt.deleted_at IS NULL
)
AND NOT EXISTS (
  SELECT 1 FROM reviews r
  WHERE r.card_key = json_extract(row.value, '$.card_key')
    AND r.reviewed_at = json_extract(row.value, '$.reviewed_at')
)
AND NOT EXISTS (
  SELECT 1 FROM json_each(?) AS earlier
  WHERE json_extract(earlier.value, '$.card_key') = json_extract(row.value, '$.card_key')
    AND json_extract(earlier.value, '$.reviewed_at') = json_extract(row.value, '$.reviewed_at')
    AND earlier.key < row.key
)";

const REBUILD_CARDS_SQL: &str = "\
UPDATE cards SET
  fsrs_state = 'review',
  stability = (
    SELECT r.stability FROM reviews r
    WHERE r.card_key = cards.stable_key
    ORDER BY r.reviewed_at DESC, r.id DESC LIMIT 1
  ),
  difficulty = (
    SELECT r.difficulty FROM reviews r
    WHERE r.card_key = cards.stable_key
    ORDER BY r.reviewed_at DESC, r.id DESC LIMIT 1
  ),
  last_reviewed_at = (
    SELECT r.reviewed_at FROM reviews r
    WHERE r.card_key = cards.stable_key
    ORDER BY r.reviewed_at DESC, r.id DESC LIMIT 1
  ),
  due_at = (
    SELECT r.reviewed_at + CAST(round(r.interval_days * 86400) AS INTEGER)
    FROM reviews r
    WHERE r.card_key = cards.stable_key
    ORDER BY r.reviewed_at DESC, r.id DESC LIMIT 1
  ),
  reps = (SELECT count(*) FROM reviews r WHERE r.card_key = cards.stable_key),
  lapses = (
    SELECT count(*) FROM reviews r
    WHERE r.card_key = cards.stable_key AND r.rating = 'again'
      AND r.id != (
        SELECT first.id FROM reviews first
        WHERE first.card_key = cards.stable_key
        ORDER BY first.reviewed_at ASC, first.id ASC LIMIT 1
      )
  ),
  updated_at = ?
WHERE cards.retired_at IS NULL
  AND EXISTS (
    SELECT 1 FROM notes nt WHERE nt.id = cards.note_id AND nt.deleted_at IS NULL
  )
  AND cards.stable_key IN (
    SELECT json_extract(value, '$.card_key') FROM json_each(?)
  )";

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReviewLogError {
    #[error("JSON が不正")]
    Json,
    #[error("rating が不正")]
    Rating,
    #[error("数値が有限ではない")]
    NotFinite,
    #[error("reviewed_at が 0 以下")]
    ReviewedAt,
    #[error("回答は {MAX_RESPONSE_CHARS} 文字まで")]
    Response,
}

/// 取り込み結果。`missing` はカードが無かった card_key。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportResult {
    pub inserted: usize,
    pub duplicates: usize,
    pub missing: Vec<String>,
}

/// rating・有限な数値・reviewed_at・response の長さ。
pub fn validate_review_log(log: &ReviewLog) -> Result<(), ReviewLogError> {
    if Rating::parse(&log.rating).is_none() {
        return Err(ReviewLogError::Rating);
    }
    if !(log.interval_days.is_finite() && log.stability.is_finite() && log.difficulty.is_finite()) {
        return Err(ReviewLogError::NotFinite);
    }
    if log.reviewed_at <= 0 {
        return Err(ReviewLogError::ReviewedAt);
    }
    if log
        .response
        .as_ref()
        .is_some_and(|text| text.chars().count() > MAX_RESPONSE_CHARS)
    {
        return Err(ReviewLogError::Response);
    }
    Ok(())
}

/// JSONL の 1 行を検証まで済ませる。
pub fn parse_review_log_line(line: &str) -> Result<ReviewLog, ReviewLogError> {
    let log: ReviewLog = serde_json::from_str(line).map_err(|_| ReviewLogError::Json)?;
    validate_review_log(&log)?;
    Ok(log)
}

/// `json_each` に渡す配列。
pub fn reviews_json(logs: &[ReviewLog]) -> Result<String, serde_json::Error> {
    serde_json::to_string(logs)
}

/// カードが無い行の card_key。同じキーは行の数だけ返る。
pub fn missing_cards_query(reviews_json: &str) -> Statement {
    Statement {
        sql: MISSING_CARDS_SQL.to_owned(),
        params: vec![Value::Text(reviews_json.to_owned())],
    }
}

/// カードがあり、同じ (card_key, reviewed_at) が DB にも同じ JSON の前の行にも無い行だけ入れる。
pub fn import_reviews_statement(reviews_json: &str) -> Statement {
    Statement {
        sql: IMPORT_REVIEWS_SQL.to_owned(),
        params: vec![
            Value::Text(reviews_json.to_owned()),
            Value::Text(reviews_json.to_owned()),
        ],
    }
}

/// JSON に含まれるカードの FSRS 列を、そのカードの全レビューから作り直す。
pub fn rebuild_cards_statement(reviews_json: &str, now: i64) -> Statement {
    Statement {
        sql: REBUILD_CARDS_SQL.to_owned(),
        params: vec![Value::Integer(now), Value::Text(reviews_json.to_owned())],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log() -> ReviewLog {
        ReviewLog {
            card_key: "a".into(),
            rating: "good".into(),
            reviewed_at: 10,
            interval_days: 1.0,
            stability: 2.0,
            difficulty: 3.0,
            response: None,
        }
    }

    #[test]
    fn validate_review_log_rejects_bad_rows() {
        assert!(validate_review_log(&log()).is_ok());
        let mut bad = log();
        bad.rating = "great".into();
        assert_eq!(validate_review_log(&bad), Err(ReviewLogError::Rating));
        bad = log();
        bad.stability = f64::NAN;
        assert_eq!(validate_review_log(&bad), Err(ReviewLogError::NotFinite));
        bad = log();
        bad.reviewed_at = 0;
        assert_eq!(validate_review_log(&bad), Err(ReviewLogError::ReviewedAt));
        bad = log();
        bad.response = Some("あ".repeat(MAX_RESPONSE_CHARS + 1));
        assert_eq!(validate_review_log(&bad), Err(ReviewLogError::Response));
        assert_eq!(parse_review_log_line("nope"), Err(ReviewLogError::Json));
        assert_eq!(IMPORT_BATCH_MAX, 200);
    }
}
