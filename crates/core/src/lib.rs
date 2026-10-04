mod schedule;

use serde::Serialize;
use ts_rs::TS;

pub use schedule::{Card, Memory, Rating, ScheduleError, Scheduled, schedule};

/// `/api/demo-schedule` のレスポンス。`cargo test` が `web/src/types/` に出す生成物はコミットする。
#[derive(Debug, Clone, Copy, Serialize, TS)]
#[ts(export, export_to = "../../../web/src/types/DemoSchedule.ts")]
pub struct DemoSchedule {
    pub rating: &'static str,
    pub interval_days: f32,
    pub stability: f32,
    pub difficulty: f32,
}

pub fn ping() -> &'static str {
    "pong"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_returns_pong() {
        assert_eq!(ping(), "pong");
    }
}
