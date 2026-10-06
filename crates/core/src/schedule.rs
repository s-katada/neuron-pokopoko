use fsrs::FSRS;

const DESIRED_RETENTION: f32 = 0.9;
const SECONDS_PER_DAY: i64 = 86_400;

/// 4 段階評価。間隔は Again < Hard < Good < Easy の順に長くなる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rating {
    Again,
    Hard,
    Good,
    Easy,
}

impl Rating {
    /// reviews.rating に入れる値。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Again => "again",
            Self::Hard => "hard",
            Self::Good => "good",
            Self::Easy => "easy",
        }
    }

    /// `as_str` の逆。未知の文字列は `None`。
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "again" => Some(Self::Again),
            "hard" => Some(Self::Hard),
            "good" => Some(Self::Good),
            "easy" => Some(Self::Easy),
            _ => None,
        }
    }
}

/// カードの記憶状態。新規カードはまだ持たない。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Memory {
    pub stability: f32,
    pub difficulty: f32,
}

/// スケジュール計算の入力。既存カードは最後の復習時刻を持つ。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Card {
    New,
    Reviewed {
        memory: Memory,
        reviewed_at_unix: i64,
    },
}

/// 選んだ評価の次回間隔(日。小数を含む)と、更新後の記憶状態。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scheduled {
    pub interval_days: f32,
    pub memory: Memory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduleError;

/// `now_unix` は呼び出し側が渡す Unix 秒。この関数は現在時刻を取得しない。
/// `parameters` は 21 個。呼び出し側が既定値か保存値を渡す。
pub fn schedule(
    card: Card,
    rating: Rating,
    now_unix: i64,
    parameters: &[f32; 21],
) -> Result<Scheduled, ScheduleError> {
    let (current, days_elapsed) = match card {
        Card::New => (None, 0),
        Card::Reviewed {
            memory,
            reviewed_at_unix,
        } => (
            Some(fsrs::MemoryState {
                stability: memory.stability,
                difficulty: memory.difficulty,
            }),
            days_between(reviewed_at_unix, now_unix),
        ),
    };

    let states = FSRS::new(parameters)
        .map_err(|_| ScheduleError)?
        .next_states(current, DESIRED_RETENTION, days_elapsed)
        .map_err(|_| ScheduleError)?;
    let chosen = match rating {
        Rating::Again => states.again,
        Rating::Hard => states.hard,
        Rating::Good => states.good,
        Rating::Easy => states.easy,
    };

    Ok(Scheduled {
        interval_days: chosen.interval,
        memory: Memory {
            stability: chosen.memory.stability,
            difficulty: chosen.memory.difficulty,
        },
    })
}

fn days_between(reviewed_at_unix: i64, now_unix: i64) -> u32 {
    let seconds = now_unix.saturating_sub(reviewed_at_unix).max(0);
    u32::try_from(seconds / SECONDS_PER_DAY).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_700_000_000;

    fn defaults() -> [f32; 21] {
        crate::default_parameters()
    }

    fn intervals(card: Card, now_unix: i64) -> (f32, f32, f32, f32) {
        let again = schedule(card, Rating::Again, now_unix, &defaults()).unwrap();
        let hard = schedule(card, Rating::Hard, now_unix, &defaults()).unwrap();
        let good = schedule(card, Rating::Good, now_unix, &defaults()).unwrap();
        let easy = schedule(card, Rating::Easy, now_unix, &defaults()).unwrap();
        (
            again.interval_days,
            hard.interval_days,
            good.interval_days,
            easy.interval_days,
        )
    }

    fn assert_longer(card: Card, now_unix: i64) {
        let (again, hard, good, easy) = intervals(card, now_unix);
        assert!(again < hard);
        assert!(hard < good);
        assert!(good < easy);
    }

    #[test]
    fn rating_text_round_trips() {
        for rating in [Rating::Again, Rating::Hard, Rating::Good, Rating::Easy] {
            assert_eq!(Rating::parse(rating.as_str()), Some(rating));
        }
        assert_eq!(Rating::parse("nope"), None);
    }

    #[test]
    fn new_card_intervals_increase_with_rating() {
        assert_longer(Card::New, NOW);
    }

    #[test]
    fn reviewed_card_intervals_increase_with_rating() {
        let first = schedule(Card::New, Rating::Good, NOW, &defaults()).unwrap();
        let card = Card::Reviewed {
            memory: first.memory,
            reviewed_at_unix: NOW,
        };
        assert_longer(card, NOW + SECONDS_PER_DAY);
    }

    #[test]
    fn custom_w2_changes_the_good_interval_of_a_new_card() {
        let base = schedule(Card::New, Rating::Good, NOW, &defaults()).unwrap();
        let mut parameters = defaults();
        parameters[2] = 10.0;
        let changed = schedule(Card::New, Rating::Good, NOW, &parameters).unwrap();
        assert_ne!(base.interval_days, changed.interval_days);
    }
}
