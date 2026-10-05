/// JST は UTC+9。
pub const JST_OFFSET_SECS: i64 = 9 * 3600;
/// 学習日は JST のこの時刻に切り替わる。
pub const DAY_ROLLOVER_HOUR: i64 = 4;
/// 1 日の新規枠。段をまたいだ合計。
pub const NEW_CARDS_PER_DAY: usize = 5;
/// 中級の新規を出すための、初級 stability の下限(日)。
pub const UNLOCK_INTERMEDIATE_DAYS: f64 = 7.0;
/// 上級の新規を出すための、中級 stability の下限(日)。
pub const UNLOCK_ADVANCED_DAYS: f64 = 21.0;

const SECS_PER_DAY: i64 = 24 * 3600;

/// `now_unix` を含む学習日の開始(直近の JST 04:00)の Unix 秒。
pub fn study_day_start(now_unix: i64) -> i64 {
    let local = now_unix + JST_OFFSET_SECS;
    let adjusted = local - DAY_ROLLOVER_HOUR * 3600;
    let day = adjusted.div_euclid(SECS_PER_DAY);
    day * SECS_PER_DAY + DAY_ROLLOVER_HOUR * 3600 - JST_OFFSET_SECS
}
