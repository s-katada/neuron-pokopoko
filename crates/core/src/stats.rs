//! 定着率・分類ツリー・日次件数。時刻は引数の `now` だけを使う。

const SECS_PER_DAY: f64 = 86_400.0;

/// 経過日数に対する想起確率。負の経過は 0 日として扱う。
///
/// `difficulty` は FSRS の関数が受け取れるよう 0 を渡す。想起確率の式は stability だけを使う。
pub fn retention(stability: f64, last_reviewed_at: i64, now: i64) -> f64 {
    let elapsed_secs = now.saturating_sub(last_reviewed_at).max(0);
    let elapsed_days = elapsed_secs as f64 / SECS_PER_DAY;
    f64::from(fsrs::current_retrievability(
        fsrs::MemoryState {
            stability: stability as f32,
            difficulty: 0.0,
        },
        elapsed_days as f32,
        fsrs::FSRS6_DEFAULT_DECAY,
    ))
}
