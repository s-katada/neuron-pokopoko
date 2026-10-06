//! 復習ログから FSRS の学習データと判断を作る。計算はネイティブの CLI が呼ぶ。

use std::collections::BTreeMap;

use fsrs::{ComputeParametersInput, FSRSItem, FSRSReview, compute_parameters};
use thiserror::Error;

use crate::params::{FSRS_PARAMETER_LEN, ParamsError, default_parameters, validate_parameters};
use crate::review_log::ReviewLog;
use crate::schedule::{Rating, days_between};

/// 学習に使える item がこれ未満ならパラメータを変えない。
pub const MIN_OPTIMIZE_ITEMS: usize = 400;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum OptimizeError {
    #[error("rating が不正")]
    Rating,
    #[error(transparent)]
    Parameters(#[from] ParamsError),
}

/// 保存するかの 3 通り。item 数が先。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OptimizePlan {
    TooFew {
        count: usize,
    },
    SameAsDefault,
    Save {
        parameters: [f32; FSRS_PARAMETER_LEN],
    },
}

/// カードごとに時刻順。同時刻は入力の順。2 回目以降で、その復習の経過日数が 1 以上のものだけ item にする。
pub fn fsrs_items(logs: &[ReviewLog]) -> Result<Vec<FSRSItem>, OptimizeError> {
    let mut groups: BTreeMap<&str, Vec<(usize, &ReviewLog)>> = BTreeMap::new();
    for (index, log) in logs.iter().enumerate() {
        groups
            .entry(log.card_key.as_str())
            .or_default()
            .push((index, log));
    }
    let mut ordered: Vec<_> = groups.into_values().collect();
    ordered.sort_by_key(|group| group.iter().map(|(index, _)| *index).min());
    let mut items = Vec::new();
    for mut reviews in ordered {
        reviews.sort_by(|left, right| {
            left.1
                .reviewed_at
                .cmp(&right.1.reviewed_at)
                .then(left.0.cmp(&right.0))
        });
        for end in 1..reviews.len() {
            let mut history = Vec::with_capacity(end + 1);
            history.push(FSRSReview {
                rating: rating_code(&reviews[0].1.rating)?,
                delta_t: 0,
            });
            let mut last_delta = 0;
            for index in 1..=end {
                last_delta = days_between(
                    reviews[index - 1].1.reviewed_at,
                    reviews[index].1.reviewed_at,
                );
                history.push(FSRSReview {
                    rating: rating_code(&reviews[index].1.rating)?,
                    delta_t: last_delta,
                });
            }
            if last_delta == 0 {
                continue;
            }
            items.push(FSRSItem { reviews: history });
        }
    }
    Ok(items)
}

/// `item_count` が最低未満なら、渡したパラメータに関わらず不足。
pub fn optimize_plan(item_count: usize, parameters: &[f32; FSRS_PARAMETER_LEN]) -> OptimizePlan {
    if item_count < MIN_OPTIMIZE_ITEMS {
        return OptimizePlan::TooFew { count: item_count };
    }
    if *parameters == default_parameters() {
        return OptimizePlan::SameAsDefault;
    }
    OptimizePlan::Save {
        parameters: *parameters,
    }
}

/// fsrs の学習。結果は #80 の検証を通った 21 個。
pub fn compute_fsrs_parameters(
    items: Vec<FSRSItem>,
) -> Result<[f32; FSRS_PARAMETER_LEN], OptimizeError> {
    let trained = compute_parameters(ComputeParametersInput {
        train_set: items,
        ..ComputeParametersInput::default()
    })
    .map_err(|_| ParamsError::Invalid)?;
    let values: Vec<f64> = trained.iter().copied().map(f64::from).collect();
    Ok(validate_parameters(&values)?)
}

fn rating_code(text: &str) -> Result<u32, OptimizeError> {
    match Rating::parse(text) {
        Some(Rating::Again) => Ok(1),
        Some(Rating::Hard) => Ok(2),
        Some(Rating::Good) => Ok(3),
        Some(Rating::Easy) => Ok(4),
        None => Err(OptimizeError::Rating),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log(key: &str, rating: &str, at: i64) -> ReviewLog {
        ReviewLog {
            card_key: key.into(),
            rating: rating.into(),
            reviewed_at: at,
            interval_days: 1.0,
            stability: 1.0,
            difficulty: 1.0,
            response: None,
        }
    }

    #[test]
    fn items_follow_each_card_and_drop_same_day_reviews() {
        let day = 86_400;
        let logs = vec![
            log("b", "hard", 10),
            log("a", "good", 0),
            log("a", "again", day),
            log("b", "good", 10 + day - 1),
            log("a", "easy", 3 * day),
            log("b", "easy", 10 + 2 * day),
            log("c", "good", 0),
            log("c", "hard", day - 1),
        ];
        let items = fsrs_items(&logs).unwrap();
        let review = |rating, delta_t| FSRSReview { rating, delta_t };
        assert_eq!(
            items,
            vec![
                FSRSItem {
                    reviews: vec![review(2, 0), review(3, 0), review(4, 1)],
                },
                FSRSItem {
                    reviews: vec![review(3, 0), review(1, 1)],
                },
                FSRSItem {
                    reviews: vec![review(3, 0), review(1, 1), review(4, 2)],
                },
            ]
        );
    }

    #[test]
    fn plan_is_too_few_same_as_default_or_save() {
        let defaults = default_parameters();
        assert_eq!(
            optimize_plan(MIN_OPTIMIZE_ITEMS - 1, &defaults),
            OptimizePlan::TooFew {
                count: MIN_OPTIMIZE_ITEMS - 1
            }
        );
        assert_eq!(
            optimize_plan(MIN_OPTIMIZE_ITEMS, &defaults),
            OptimizePlan::SameAsDefault
        );
        let mut changed = defaults;
        changed[2] = 10.0;
        assert_eq!(
            optimize_plan(MIN_OPTIMIZE_ITEMS, &changed),
            OptimizePlan::Save {
                parameters: changed
            }
        );
    }

    #[test]
    fn compute_learns_from_synthetic_items() {
        let mut items = Vec::new();
        for card in 0..80 {
            let mut reviews = vec![FSRSReview {
                rating: 3,
                delta_t: 0,
            }];
            for step in 1..4 {
                reviews.push(FSRSReview {
                    rating: if (card + step) % 5 == 0 { 1 } else { 3 },
                    delta_t: step as u32 * 2,
                });
                items.push(FSRSItem {
                    reviews: reviews.clone(),
                });
            }
        }
        let parameters = compute_fsrs_parameters(items).unwrap();
        assert!(parameters.iter().all(|value| value.is_finite()));
        assert_ne!(parameters, default_parameters());
    }
}
