mod day;
mod import;
mod lint;
pub mod note;
mod optimize;
mod params;
mod review;
mod review_log;
mod schedule;
mod stats;
mod sync;
mod taxonomy;
mod vault;

pub use day::{
    DAY_ROLLOVER_HOUR, JST_OFFSET_SECS, NEW_CARDS_PER_DAY, UNLOCK_ADVANCED_DAYS,
    UNLOCK_INTERMEDIATE_DAYS, study_day_start,
};
pub use import::{
    IMPORT_BATCH_MAX, ImportResult, ReviewLogError, import_reviews_statement, missing_cards_query,
    parse_review_log_line, rebuild_cards_statement, reviews_json, validate_review_log,
};
pub use lint::{Finding, IntegrationRefReason, LintError, LintKind, Report, lint_vault};
pub use note::{Level, Note, NoteError, Section, parse_note};
pub use optimize::{
    MIN_OPTIMIZE_ITEMS, OptimizeError, OptimizePlan, compute_fsrs_parameters, fsrs_items,
    optimize_plan,
};
pub use params::{
    FSRS_PARAMETER_LEN, FsrsParamsBody, ParamsError, PutFsrsParams, decay_of, default_decay,
    default_parameters, empty_fsrs_params, fsrs_params_query, parameters_or_default,
    upsert_fsrs_params_statement, validate_parameters, validate_review_count,
};
pub use review::{
    Answer, AnswerRequest, AnswerResponse, CardState, MAX_RESPONSE_CHARS, NextResponse,
    ResponseError, ReviewCard, ReviewCardRow, answer_statements, card_state_query, next_card_query,
    normalize_response, review_card_from_row,
};
pub use review_log::{
    EXPORT_PAGE_DEFAULT, EXPORT_PAGE_MAX, ReviewLog, ReviewLogPage, export_reviews_query,
};
pub use schedule::{Card, Memory, Rating, ScheduleError, Scheduled, schedule};
pub use stats::{
    DailyCount, DetailCard, MajorStats, MiddleStats, MinorStats, NoteDetail, NoteStats, StatCard,
    Tree, build_tree, daily_counts, daily_reviews_query, note_detail, note_stat_cards_query,
    retention, stat_cards_query,
};
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
