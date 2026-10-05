//! 定着率・分類ツリー・日次件数。時刻は引数の `now` だけを使う。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

const SECS_PER_DAY: f64 = 86_400.0;

const EXPORT_TO: &str = "../../../web/src/types/";

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

#[derive(Default)]
struct Group<'a> {
    cards: Vec<&'a StatCard>,
    children: BTreeMap<&'a str, Group<'a>>,
}

/// 大 → 中 → 小 → ノート。各階層の定着率は、配下の `review` カードの算術平均。
pub fn build_tree(cards: &[StatCard], now: i64) -> Tree {
    let root = group_cards(cards);
    Tree {
        overall: mean_retention(&root.cards, now),
        majors: root
            .children
            .iter()
            .map(|(name, group)| major_stats(name, group, now))
            .collect(),
    }
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
        fn $fn_name(name: &str, group: &Group<'_>, now: i64) -> $struct_name {
            $struct_name {
                $name_field: name.to_owned(),
                retention: mean_retention(&group.cards, now),
                card_count: card_count(&group.cards),
                $children_field: group
                    .children
                    .iter()
                    .map(|(child, group)| $child_fn(child, group, now))
                    .collect(),
            }
        }
    };
}

rollup!(major_stats, MajorStats, major, middles, middle_stats);
rollup!(middle_stats, MiddleStats, middle, minors, minor_stats);

fn minor_stats(name: &str, group: &Group<'_>, now: i64) -> MinorStats {
    MinorStats {
        minor: name.to_owned(),
        retention: mean_retention(&group.cards, now),
        card_count: card_count(&group.cards),
        notes: notes_of(group, now),
    }
}

fn notes_of(minor: &Group<'_>, now: i64) -> Vec<NoteStats> {
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
                retention: mean_retention(&group.cards, now),
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

fn mean_retention(cards: &[&StatCard], now: i64) -> Option<f64> {
    let mut sum = 0.0;
    let mut count = 0u32;
    for card in cards {
        if let Some(value) = card_retention(card, now) {
            sum += value;
            count += 1;
        }
    }
    (count > 0).then_some(sum / f64::from(count))
}

fn card_retention(card: &StatCard, now: i64) -> Option<f64> {
    if card.fsrs_state != "review" {
        return None;
    }
    Some(retention(card.stability?, card.last_reviewed_at?, now))
}

fn card_count(cards: &[&StatCard]) -> u32 {
    u32::try_from(cards.len()).expect("カード数は u32 に収まる")
}
