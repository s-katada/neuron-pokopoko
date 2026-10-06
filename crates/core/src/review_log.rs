//! 復習ログの 1 行。JSONL には id を書かない。ページ取得は id 昇順。

use serde::{Deserialize, Serialize};

use crate::sync::{Statement, Value};

/// `GET /api/export/reviews` の既定件数。
pub const EXPORT_PAGE_DEFAULT: usize = 200;
/// 1 ページの上限。範囲外は API が 400 を返す。
pub const EXPORT_PAGE_MAX: usize = 500;

const EXPORT_REVIEWS_SQL: &str = "SELECT id, card_key, rating, reviewed_at, interval_days, stability, difficulty, response FROM reviews WHERE id > ? ORDER BY id LIMIT ?";

/// JSONL の 1 行。import もこの型を読む。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewLog {
    pub card_key: String,
    pub rating: String,
    pub reviewed_at: i64,
    pub interval_days: f64,
    pub stability: f64,
    pub difficulty: f64,
    #[serde(default)]
    pub response: Option<String>,
}

impl ReviewLog {
    /// JSONL の 1 行。id は含めない。
    pub fn to_json_line(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// API の 1 行。次ページの `after` に使う id を含む。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewLogPage {
    pub id: i64,
    #[serde(flatten)]
    pub log: ReviewLog,
}

/// `after` より大きい id を昇順で `limit` 件。`limit` は呼び出し側が 1..=[`EXPORT_PAGE_MAX`] に揃える。
pub fn export_reviews_query(after: i64, limit: usize) -> Statement {
    Statement {
        sql: EXPORT_REVIEWS_SQL.to_owned(),
        params: vec![
            Value::Integer(after),
            Value::Integer(i64::try_from(limit).expect("ページ件数は i64 に収まる")),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ReviewLog {
        ReviewLog {
            card_key: "gain/beginner/abc".to_owned(),
            rating: "good".to_owned(),
            reviewed_at: 1_700_000_000,
            interval_days: 2.5,
            stability: 3.25,
            difficulty: 4.5,
            response: Some("メモ".to_owned()),
        }
    }

    #[test]
    fn export_reviews_query_binds_after_and_limit() {
        let statement = export_reviews_query(10, EXPORT_PAGE_DEFAULT);
        assert_eq!(statement.sql, EXPORT_REVIEWS_SQL);
        assert!(statement.sql.contains("id > ? ORDER BY id LIMIT ?"));
        assert_eq!(
            statement.params,
            vec![Value::Integer(10), Value::Integer(200)]
        );
        assert_eq!(EXPORT_PAGE_DEFAULT, 200);
        assert_eq!(EXPORT_PAGE_MAX, 500);
    }

    #[test]
    fn review_log_round_trips_without_id() {
        let log = sample();
        let json = serde_json::to_string(&log).unwrap();
        assert!(!json.contains("\"id\""));
        let back: ReviewLog = serde_json::from_str(&json).unwrap();
        assert_eq!(back, log);

        let missing = r#"{"card_key":"k","rating":"again","reviewed_at":1,"interval_days":1.0,"stability":1.0,"difficulty":1.0}"#;
        let without_response: ReviewLog = serde_json::from_str(missing).unwrap();
        assert_eq!(without_response.response, None);

        let page = ReviewLogPage { id: 7, log };
        let page_json = serde_json::to_string(&page).unwrap();
        assert!(page_json.contains("\"id\":7"));
        let back_page: ReviewLogPage = serde_json::from_str(&page_json).unwrap();
        assert_eq!(back_page, page);
        let line = serde_json::to_string(&back_page.log).unwrap();
        assert!(!line.contains("\"id\""));
    }
}
