mod day;
mod lint;
pub mod note;
mod review;
mod schedule;
mod sync;
mod taxonomy;
mod vault;

pub use day::{
    DAY_ROLLOVER_HOUR, JST_OFFSET_SECS, NEW_CARDS_PER_DAY, UNLOCK_ADVANCED_DAYS,
    UNLOCK_INTERMEDIATE_DAYS, study_day_start,
};
pub use lint::{Finding, IntegrationRefReason, LintError, LintKind, Report, lint_vault};
pub use note::{Level, Note, NoteError, Section, parse_note};
pub use review::{
    Answer, AnswerRequest, AnswerResponse, CardState, MAX_RESPONSE_CHARS, NextResponse,
    ResponseError, ReviewCard, answer_statements, card_state_query, next_card_query,
    normalize_response,
};
pub use schedule::{Card, Memory, Rating, ScheduleError, Scheduled, schedule};
pub use sync::{
    MAX_BIND_PARAMS, MAX_DELETE_IDS_PER_REQUEST, MAX_STATEMENTS_PER_REQUEST, ManifestEntry,
    Statement, SyncCard, SyncError, SyncNote, SyncPlan, Value, chunk, chunk_deletes, collect,
    delete_statements, plan, statements_for, upsert_statements,
};
pub use taxonomy::{TaxonRank, Taxonomy, TaxonomyError, load, validate};

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
