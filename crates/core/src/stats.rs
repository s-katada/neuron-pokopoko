//! 定着率・分類ツリー・日次件数。時刻は引数の `now` だけを使う。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::day::study_day_start;
use crate::sync::{Statement, Value};

const SECS_PER_DAY: f64 = 86_400.0;

const EXPORT_TO: &str = "../../../web/src/types/";

/// 経過日数に対する想起確率。負の経過は 0 日として扱う。
///
/// `decay` はパラメータの 21 番目。未保存なら呼び出し側が既定値を渡す。
/// `difficulty` は FSRS の関数が受け取れるよう 0 を渡す。想起確率の式は stability と decay だけを使う。
pub fn retention(stability: f64, last_reviewed_at: i64, now: i64, decay: f32) -> f64 {
    let elapsed_secs = now.saturating_sub(last_reviewed_at).max(0);
    let elapsed_days = elapsed_secs as f64 / SECS_PER_DAY;
    f64::from(fsrs::current_retrievability(
        fsrs::MemoryState {
            stability: stability as f32,
            difficulty: 0.0,
        },
        elapsed_days as f32,
        decay,
    ))
}

/// 統計の入力行。Worker が読んだカードをそのまま渡す。
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct StatCard {
    pub major: String,
    pub middle: String,
    pub minor: String,
    pub note_id: String,
    pub title: String,
    pub level: String,
    pub fsrs_state: String,
    pub stability: Option<f64>,
    pub last_reviewed_at: Option<i64>,
    pub due_at: Option<i64>,
    pub stable_key: String,
    pub question: String,
}

/// 分類ツリー。各階層の定着率は、配下の対象カードの平均。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = format!("{EXPORT_TO}Tree.ts"))]
pub struct Tree {
    pub overall: Option<f64>,
    pub majors: Vec<MajorStats>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = format!("{EXPORT_TO}MajorStats.ts"))]
pub struct MajorStats {
    pub major: String,
    pub retention: Option<f64>,
    pub card_count: u32,
    pub middles: Vec<MiddleStats>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = format!("{EXPORT_TO}MiddleStats.ts"))]
pub struct MiddleStats {
    pub middle: String,
    pub retention: Option<f64>,
    pub card_count: u32,
    pub minors: Vec<MinorStats>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = format!("{EXPORT_TO}MinorStats.ts"))]
pub struct MinorStats {
    pub minor: String,
    pub retention: Option<f64>,
    pub card_count: u32,
    pub notes: Vec<NoteStats>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = format!("{EXPORT_TO}NoteStats.ts"))]
pub struct NoteStats {
    pub note_id: String,
    pub title: String,
    pub retention: Option<f64>,
    pub card_count: u32,
}

/// ノート 1 件のカード一覧。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = format!("{EXPORT_TO}NoteDetail.ts"))]
pub struct NoteDetail {
    pub note_id: String,
    pub title: String,
    pub cards: Vec<DetailCard>,
}

/// ノート詳細のカード。未学習の定着率は null。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = format!("{EXPORT_TO}DetailCard.ts"))]
pub struct DetailCard {
    pub level: String,
    pub question: String,
    pub fsrs_state: String,
    #[ts(type = "number | null")]
    pub due_at: Option<i64>,
    pub retention: Option<f64>,
}

/// 学習日 1 日のレビュー件数。`day_start` は JST 04:00。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = format!("{EXPORT_TO}DailyCount.ts"))]
pub struct DailyCount {
    #[ts(type = "number")]
    pub day_start: i64,
    pub count: u32,
}

#[derive(Default)]
struct Group<'a> {
    cards: Vec<&'a StatCard>,
    children: BTreeMap<&'a str, Group<'a>>,
}

/// 大 → 中 → 小 → ノート。各階層の定着率は、配下の `review` カードの算術平均。
pub fn build_tree(cards: &[StatCard], now: i64, decay: f32) -> Tree {
    let root = group_cards(cards);
    Tree {
        overall: mean_retention(&root.cards, now, decay),
        majors: root
            .children
            .iter()
            .map(|(name, group)| major_stats(name, group, now, decay))
            .collect(),
    }
}

/// ノートが無ければ `None`。カードは `stable_key` 順。
pub fn note_detail(cards: &[StatCard], note_id: &str, now: i64, decay: f32) -> Option<NoteDetail> {
    let mut matched: Vec<&StatCard> = cards
        .iter()
        .filter(|card| card.note_id == note_id)
        .collect();
    if matched.is_empty() {
        return None;
    }
    matched.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
    let title = matched
        .iter()
        .map(|card| card.title.as_str())
        .min()
        .unwrap_or("")
        .to_owned();
    Some(NoteDetail {
        note_id: note_id.to_owned(),
        title,
        cards: matched
            .into_iter()
            .map(|card| DetailCard {
                level: card.level.clone(),
                question: card.question.clone(),
                fsrs_state: card.fsrs_state.clone(),
                due_at: card.due_at,
                retention: card_retention(card, now, decay),
            })
            .collect(),
    })
}

const STAT_CARD_COLUMNS: &str = "\
SELECT nt.major, nt.middle, nt.minor, nt.id AS note_id, nt.title, \
c.level, c.fsrs_state, c.stability, c.last_reviewed_at, c.due_at, c.stable_key, c.question \
FROM cards c JOIN notes nt ON nt.id = c.note_id \
WHERE c.retired_at IS NULL AND nt.deleted_at IS NULL";

/// 有効カードと未削除ノート。列は [`StatCard`] と同じ。
pub fn stat_cards_query() -> Statement {
    Statement {
        sql: STAT_CARD_COLUMNS.to_owned(),
        params: vec![],
    }
}

/// [`stat_cards_query`] を 1 ノートに絞る。
pub fn note_stat_cards_query(note_id: &str) -> Statement {
    Statement {
        sql: format!("{STAT_CARD_COLUMNS} AND nt.id = ?"),
        params: vec![Value::Text(note_id.to_owned())],
    }
}

/// 今日を含む直近 `days` 日に入る `reviewed_at`。`days` は 1 以上。
pub fn daily_reviews_query(now: i64, days: usize) -> Statement {
    Statement {
        sql: "SELECT reviewed_at FROM reviews WHERE reviewed_at >= ?".to_owned(),
        params: vec![Value::Integer(window_start(now, days))],
    }
}

/// 今日を含む直近 `days` 日を古い順に返す。0 件の日も含む。
pub fn daily_counts(reviewed_at: &[i64], now: i64, days: usize) -> Vec<DailyCount> {
    if days == 0 {
        return Vec::new();
    }
    let today = study_day_start(now);
    let start = window_start(now, days);
    let mut counts = vec![0u32; days];
    for &reviewed in reviewed_at {
        let day = study_day_start(reviewed);
        if (start..=today).contains(&day) {
            let index = usize::try_from((day - start) / 86_400).expect("日の添字は usize に収まる");
            counts[index] = counts[index].saturating_add(1);
        }
    }
    counts
        .into_iter()
        .enumerate()
        .map(|(index, count)| DailyCount {
            day_start: start + i64::try_from(index).expect("日の添字は i64 に収まる") * 86_400,
            count,
        })
        .collect()
}

fn window_start(now: i64, days: usize) -> i64 {
    study_day_start(now) - i64::try_from(days - 1).expect("日数は i64 に収まる") * 86_400
}

fn group_cards(cards: &[StatCard]) -> Group<'_> {
    let mut root = Group::default();
    for card in cards {
        let mut node = &mut root;
        for key in [
            card.major.as_str(),
            card.middle.as_str(),
            card.minor.as_str(),
            card.note_id.as_str(),
        ] {
            node.cards.push(card);
            node = node.children.entry(key).or_default();
        }
        node.cards.push(card);
    }
    root
}

macro_rules! rollup {
    ($fn_name:ident, $struct_name:ident, $name_field:ident, $children_field:ident, $child_fn:ident) => {
        fn $fn_name(name: &str, group: &Group<'_>, now: i64, decay: f32) -> $struct_name {
            $struct_name {
                $name_field: name.to_owned(),
                retention: mean_retention(&group.cards, now, decay),
                card_count: card_count(&group.cards),
                $children_field: group
                    .children
                    .iter()
                    .map(|(child, group)| $child_fn(child, group, now, decay))
                    .collect(),
            }
        }
    };
}

rollup!(major_stats, MajorStats, major, middles, middle_stats);
rollup!(middle_stats, MiddleStats, middle, minors, minor_stats);

fn minor_stats(name: &str, group: &Group<'_>, now: i64, decay: f32) -> MinorStats {
    MinorStats {
        minor: name.to_owned(),
        retention: mean_retention(&group.cards, now, decay),
        card_count: card_count(&group.cards),
        notes: notes_of(group, now, decay),
    }
}

fn notes_of(minor: &Group<'_>, now: i64, decay: f32) -> Vec<NoteStats> {
    let mut notes: Vec<NoteStats> = minor
        .children
        .iter()
        .map(|(note_id, group)| {
            let title = group
                .cards
                .iter()
                .map(|card| card.title.as_str())
                .min()
                .unwrap_or("")
                .to_owned();
            NoteStats {
                note_id: (*note_id).to_owned(),
                title,
                retention: mean_retention(&group.cards, now, decay),
                card_count: card_count(&group.cards),
            }
        })
        .collect();
    notes.sort_by(|left, right| {
        left.title
            .cmp(&right.title)
            .then(left.note_id.cmp(&right.note_id))
    });
    notes
}

fn mean_retention(cards: &[&StatCard], now: i64, decay: f32) -> Option<f64> {
    let mut sum = 0.0;
    let mut count = 0u32;
    for card in cards {
        if let Some(value) = card_retention(card, now, decay) {
            sum += value;
            count += 1;
        }
    }
    (count > 0).then_some(sum / f64::from(count))
}

fn card_retention(card: &StatCard, now: i64, decay: f32) -> Option<f64> {
    if card.fsrs_state != "review" {
        return None;
    }
    Some(retention(
        card.stability?,
        card.last_reviewed_at?,
        now,
        decay,
    ))
}

fn card_count(cards: &[&StatCard]) -> u32 {
    u32::try_from(cards.len()).expect("カード数は u32 に収まる")
}
