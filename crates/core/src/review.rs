use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::day::{NEW_CARDS_PER_DAY, study_day_start};
use crate::schedule::{Card, Memory, Rating, ScheduleError, schedule};
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

const CARD_STATE: &str = "SELECT c.fsrs_state, c.stability, c.difficulty, c.last_reviewed_at FROM cards c JOIN notes nt ON nt.id = c.note_id WHERE c.stable_key = ? AND c.retired_at IS NULL AND nt.deleted_at IS NULL";

const INSERT_REVIEW: &str = "INSERT INTO reviews (card_key, rating, reviewed_at, interval_days, stability, difficulty) VALUES (?, ?, ?, ?, ?, ?)";

const UPDATE_CARD: &str = "UPDATE cards SET fsrs_state = 'review', stability = ?, difficulty = ?, due_at = ?, last_reviewed_at = ?, reps = reps + 1, lapses = lapses + ?, updated_at = ? WHERE stable_key = ? AND retired_at IS NULL";

/// 回答前の FSRS 列。行が無ければ Worker は 404 にする。
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CardState {
    pub fsrs_state: String,
    pub stability: Option<f64>,
    pub difficulty: Option<f64>,
    pub last_reviewed_at: Option<i64>,
}

/// 回答 1 回で流す文と、表示用の間隔。
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub statements: Vec<Statement>,
    pub interval_days: f32,
    pub due_at: i64,
}

/// 出題するカード。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../web/src/types/ReviewCard.ts")]
pub struct ReviewCard {
    pub stable_key: String,
    pub note_id: String,
    pub title: String,
    pub question: String,
    pub answer: String,
    pub level: String,
}

/// `GET /api/review/next`。`card` が null なら今日の出題は終わり。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../web/src/types/NextResponse.ts")]
pub struct NextResponse {
    pub card: Option<ReviewCard>,
}

/// `POST /api/review/answer` のリクエスト。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../web/src/types/AnswerRequest.ts")]
pub struct AnswerRequest {
    pub stable_key: String,
    #[ts(type = "\"again\" | \"hard\" | \"good\" | \"easy\"")]
    pub rating: String,
}

/// `POST /api/review/answer` のレスポンス。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../web/src/types/AnswerResponse.ts")]
pub struct AnswerResponse {
    pub interval_days: f32,
    #[ts(type = "number")]
    pub due_at: i64,
}

/// 存続しているカードの状態を 1 行読む。
pub fn card_state_query(stable_key: &str) -> Statement {
    Statement {
        sql: CARD_STATE.to_owned(),
        params: vec![text(stable_key)],
    }
}

/// 復習ログの追記と、カードの FSRS 列更新。
pub fn answer_statements(
    stable_key: &str,
    state: &CardState,
    rating: Rating,
    now_unix: i64,
) -> Result<Answer, ScheduleError> {
    let card = match (state.fsrs_state.as_str(), state.stability) {
        ("new", _) | (_, None) => Card::New,
        (_, Some(stability)) => Card::Reviewed {
            memory: Memory {
                stability: stability as f32,
                difficulty: state.difficulty.unwrap_or(0.0) as f32,
            },
            reviewed_at_unix: state.last_reviewed_at.unwrap_or(now_unix),
        },
    };
    let scheduled = schedule(card, rating, now_unix)?;
    let due_at = now_unix + (f64::from(scheduled.interval_days) * 86_400.0).round() as i64;
    let lapses = i64::from(state.fsrs_state == "review" && rating == Rating::Again);
    let stability = f64::from(scheduled.memory.stability);
    let difficulty = f64::from(scheduled.memory.difficulty);
    Ok(Answer {
        statements: vec![
            Statement {
                sql: INSERT_REVIEW.to_owned(),
                params: vec![
                    text(stable_key),
                    text(rating.as_str()),
                    Value::Integer(now_unix),
                    Value::Real(f64::from(scheduled.interval_days)),
                    Value::Real(stability),
                    Value::Real(difficulty),
                ],
            },
            Statement {
                sql: UPDATE_CARD.to_owned(),
                params: vec![
                    Value::Real(stability),
                    Value::Real(difficulty),
                    Value::Integer(due_at),
                    Value::Integer(now_unix),
                    Value::Integer(lapses),
                    Value::Integer(now_unix),
                    text(stable_key),
                ],
            },
        ],
        interval_days: scheduled.interval_days,
        due_at,
    })
}

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}
