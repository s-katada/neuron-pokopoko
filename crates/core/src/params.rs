//! 保存する FSRS パラメータ。21 個ちょうどだけを受け、壊れた保存値は既定値に戻す。

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::sync::{Statement, Value};

/// FSRS-6 のパラメータ数。17 個・19 個の旧形式は受けない。
pub const FSRS_PARAMETER_LEN: usize = 21;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ParamsError {
    #[error("パラメータは 21 個")]
    Length,
    #[error("数値が有限ではない")]
    NotFinite,
    #[error("パラメータが不正")]
    Invalid,
    #[error("review_count は 0 以上")]
    ReviewCount,
}

/// `GET /api/params` と、保存したあとの `PUT` の応答。未保存はすべて null。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FsrsParamsBody {
    pub params: Option<Vec<f64>>,
    pub review_count: Option<i64>,
    pub updated_at: Option<i64>,
}

/// `PUT /api/params` のリクエスト。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PutFsrsParams {
    pub params: Vec<f64>,
    pub review_count: i64,
}

pub fn empty_fsrs_params() -> FsrsParamsBody {
    FsrsParamsBody {
        params: None,
        review_count: None,
        updated_at: None,
    }
}

pub fn default_parameters() -> [f32; FSRS_PARAMETER_LEN] {
    fsrs::DEFAULT_PARAMETERS
}

pub fn default_decay() -> f32 {
    fsrs::FSRS6_DEFAULT_DECAY
}

/// 21 番目が forgetting curve の decay。
pub fn decay_of(parameters: &[f32; FSRS_PARAMETER_LEN]) -> f32 {
    parameters[FSRS_PARAMETER_LEN - 1]
}

/// 0 未満は保存しない。
pub fn validate_review_count(review_count: i64) -> Result<(), ParamsError> {
    if review_count < 0 {
        Err(ParamsError::ReviewCount)
    } else {
        Ok(())
    }
}

/// 21 個の有限な値だけ通す。`check_and_fill_parameters` は旧形式も埋めるので、長さは先に見る。
pub fn validate_parameters(values: &[f64]) -> Result<[f32; FSRS_PARAMETER_LEN], ParamsError> {
    if values.len() != FSRS_PARAMETER_LEN {
        return Err(ParamsError::Length);
    }
    let mut parameters = [0f32; FSRS_PARAMETER_LEN];
    for (slot, value) in parameters.iter_mut().zip(values) {
        if !value.is_finite() {
            return Err(ParamsError::NotFinite);
        }
        *slot = *value as f32;
        if !slot.is_finite() {
            return Err(ParamsError::NotFinite);
        }
    }
    fsrs::check_and_fill_parameters(&parameters).map_err(|_| ParamsError::Invalid)?;
    Ok(parameters)
}

/// 行が無い、JSON でない、21 個の有限値でないときは既定値。
pub fn parameters_or_default(json: Option<&str>) -> [f32; FSRS_PARAMETER_LEN] {
    let Some(json) = json else {
        return default_parameters();
    };
    let Ok(values) = serde_json::from_str::<Vec<f64>>(json) else {
        return default_parameters();
    };
    validate_parameters(&values).unwrap_or_else(|_| default_parameters())
}

const SELECT_SQL: &str = "SELECT params, review_count, updated_at FROM fsrs_params WHERE id = 1";

const UPSERT_SQL: &str = "\
INSERT INTO fsrs_params (id, params, review_count, updated_at) \
VALUES (1, ?, ?, ?) \
ON CONFLICT(id) DO UPDATE SET \
  params = excluded.params, \
  review_count = excluded.review_count, \
  updated_at = excluded.updated_at";

pub fn fsrs_params_query() -> Statement {
    Statement {
        sql: SELECT_SQL.to_owned(),
        params: Vec::new(),
    }
}

pub fn upsert_fsrs_params_statement(
    params_json: &str,
    review_count: i64,
    updated_at: i64,
) -> Statement {
    Statement {
        sql: UPSERT_SQL.to_owned(),
        params: vec![
            Value::Text(params_json.to_owned()),
            Value::Integer(review_count),
            Value::Integer(updated_at),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Vec<f64> {
        default_parameters()
            .iter()
            .map(|value| f64::from(*value))
            .collect()
    }

    #[test]
    fn rejects_the_wrong_length_nan_and_infinity() {
        assert!(validate_parameters(&defaults()).is_ok());
        assert_eq!(
            validate_parameters(&defaults()[..20]),
            Err(ParamsError::Length)
        );
        assert_eq!(
            validate_parameters(&defaults()[..17]),
            Err(ParamsError::Length)
        );
        assert_eq!(
            validate_parameters(&defaults()[..19]),
            Err(ParamsError::Length)
        );
        let mut values = defaults();
        values.push(0.1);
        assert_eq!(validate_parameters(&values), Err(ParamsError::Length));
        values = defaults();
        values[2] = f64::NAN;
        assert_eq!(validate_parameters(&values), Err(ParamsError::NotFinite));
        values[2] = f64::INFINITY;
        assert_eq!(validate_parameters(&values), Err(ParamsError::NotFinite));
    }

    #[test]
    fn rejects_a_negative_review_count() {
        assert_eq!(validate_review_count(-1), Err(ParamsError::ReviewCount));
        assert!(validate_review_count(0).is_ok());
    }

    #[test]
    fn broken_json_falls_back_to_the_defaults() {
        assert_eq!(parameters_or_default(None), default_parameters());
        assert_eq!(parameters_or_default(Some("nope")), default_parameters());
        assert_eq!(parameters_or_default(Some("[1, 2]")), default_parameters());
        let custom = serde_json::to_string(&vec![1.0; FSRS_PARAMETER_LEN]).unwrap();
        let parsed = parameters_or_default(Some(&custom));
        assert_ne!(parsed, default_parameters());
        assert!(parsed.iter().all(|value| *value == 1.0));
    }
}
