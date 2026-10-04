use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use thiserror::Error;
use unicode_normalization::UnicodeNormalization;

use crate::note::{self, NoteError};

/// 大分類 → 中分類 → 小分類。読み込み時に NFC へ揃える。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Taxonomy {
    majors: BTreeMap<String, BTreeMap<String, BTreeSet<String>>>,
}

/// 未定義だった分類の段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaxonRank {
    Major,
    Middle,
    Minor,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TaxonomyError {
    #[error("taxonomy が不正: {0}")]
    Invalid(String),
    #[error("ノートのパスが learning/<大>/<中>/<小>/<概念>.md ではない")]
    InvalidPath,
    #[error("未定義の{level}: {name}")]
    Undefined { level: TaxonRank, name: String },
}

impl fmt::Display for TaxonRank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Major => "大分類",
            Self::Middle => "中分類",
            Self::Minor => "小分類",
        })
    }
}

/// `learning/taxonomy.yaml` の中身を読む。
pub fn load(yaml: &str) -> Result<Taxonomy, TaxonomyError> {
    let raw: BTreeMap<String, BTreeMap<String, Vec<String>>> =
        serde_yaml_ng::from_str(yaml).map_err(|err| TaxonomyError::Invalid(err.to_string()))?;
    let mut majors = BTreeMap::new();
    for (major, middles) in raw {
        let mut middle_map = BTreeMap::new();
        for (middle, minors) in middles {
            let minors = minors.into_iter().map(|name| nfc(&name)).collect();
            middle_map.insert(nfc(&middle), minors);
        }
        majors.insert(nfc(&major), middle_map);
    }
    Ok(Taxonomy { majors })
}

/// パスを NFC で解析し、taxonomy にある大/中/小の組か確かめる。
pub fn validate(note_path: &str, taxonomy: &Taxonomy) -> Result<(), TaxonomyError> {
    let (major, middle, minor, _) = note::parse_location(note_path).map_err(map_path_error)?;
    let Some(middles) = taxonomy.majors.get(&major) else {
        return Err(undefined(TaxonRank::Major, major));
    };
    let Some(minors) = middles.get(&middle) else {
        return Err(undefined(TaxonRank::Middle, middle));
    };
    if !minors.contains(&minor) {
        return Err(undefined(TaxonRank::Minor, minor));
    }
    Ok(())
}

fn undefined(level: TaxonRank, name: String) -> TaxonomyError {
    TaxonomyError::Undefined { level, name }
}

fn map_path_error(err: NoteError) -> TaxonomyError {
    match err {
        NoteError::InvalidPath => TaxonomyError::InvalidPath,
        other => TaxonomyError::Invalid(other.to_string()),
    }
}

fn nfc(text: &str) -> String {
    text.nfc().collect()
}
