mod day;
mod lint;
pub mod note;
mod schedule;
mod sync;
mod taxonomy;
mod vault;

use serde::Serialize;
use ts_rs::TS;

pub use day::{DAY_ROLLOVER_HOUR, JST_OFFSET_SECS, NEW_CARDS_PER_DAY, study_day_start};
pub use lint::{Finding, LintError, LintKind, Report, lint_vault};
pub use note::{Level, Note, NoteError, Section, parse_note};
pub use schedule::{Card, Memory, Rating, ScheduleError, Scheduled, schedule};
pub use sync::{
    MAX_BIND_PARAMS, MAX_DELETE_IDS_PER_REQUEST, MAX_STATEMENTS_PER_REQUEST, ManifestEntry,
    Statement, SyncCard, SyncError, SyncNote, SyncPlan, Value, chunk, chunk_deletes, collect,
    delete_statements, plan, statements_for, upsert_statements,
};
pub use taxonomy::{TaxonRank, Taxonomy, TaxonomyError, load, validate};

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
