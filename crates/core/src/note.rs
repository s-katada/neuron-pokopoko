use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use thiserror::Error;
use unicode_normalization::UnicodeNormalization;

/// 段。仕様の beginner / intermediate / advanced / integration。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Beginner,
    Intermediate,
    Advanced,
    Integration,
}

/// `## 初級` などの 1 段。項目はトップレベルの箇条書き。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub level: Level,
    pub items: Vec<Item>,
}

/// 箇条書き 1 項目。`question` が無い項目はカードにしない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub answer: String,
    pub question: Option<String>,
    pub rubric: Option<String>,
    pub refs: Vec<String>,
}

/// 出題カード。安定キーは並び順に依存しない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub stable_key: String,
    pub note_id: String,
    pub level: Level,
    pub question: String,
    pub answer: String,
    pub rubric: Option<String>,
    pub refs: Vec<String>,
}

impl Level {
    /// 安定キーに使う段の英名。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Beginner => "beginner",
            Self::Intermediate => "intermediate",
            Self::Advanced => "advanced",
            Self::Integration => "integration",
        }
    }
}

impl Note {
    /// `Q:` がある項目だけをカードにする。
    pub fn cards(&self) -> Vec<Card> {
        let mut cards = Vec::new();
        for section in &self.sections {
            for item in &section.items {
                let Some(question) = &item.question else {
                    continue;
                };
                cards.push(Card {
                    stable_key: format!(
                        "{}/{}/{}",
                        self.id,
                        section.level.as_str(),
                        qkey(question)
                    ),
                    note_id: self.id.clone(),
                    level: section.level,
                    question: question.clone(),
                    answer: item.answer.clone(),
                    rubric: item.rubric.clone(),
                    refs: if section.level == Level::Integration {
                        item.refs.clone()
                    } else {
                        Vec::new()
                    },
                });
            }
        }
        cards
    }
}

/// 1 ノート。分類と id はパス由来。時刻は持たない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub aliases: Vec<String>,
    pub sources: Vec<String>,
    pub major: String,
    pub middle: String,
    pub minor: String,
    pub concept: String,
    pub sections: Vec<Section>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NoteError {
    #[error("ノートのパスが learning/<major>/<middle>/<minor>/<id>.md ではない")]
    InvalidPath,
    #[error("パスのセグメントが規則に合わない: {0}")]
    InvalidSegment(String),
    #[error("ファイル名が id と一致しない: {filename} != {id}")]
    FilenameIdMismatch { filename: String, id: String },
    #[error("frontmatter がない")]
    MissingFrontmatter,
    #[error("frontmatter が不正: {0}")]
    InvalidFrontmatter(String),
    #[error("id がない")]
    MissingId,
    #[error("未知の段見出し: {0}")]
    UnknownHeading(String),
    #[error("段が重複している: {0}")]
    DuplicateSection(String),
    #[error("初級の段がない")]
    MissingBeginner,
    #[error("段の外に項目がある")]
    ItemOutsideSection,
}

/// `relative_path` は vault 相対の `learning/<major>/<middle>/<minor>/<id>.md`。
pub fn parse_note(relative_path: &str, markdown: &str) -> Result<Note, NoteError> {
    let (major, middle, minor, concept) = parse_location(relative_path)?;
    let (yaml, body) = split_frontmatter(markdown)?;
    let frontmatter: Frontmatter = serde_yaml_ng::from_str(yaml)
        .map_err(|err| NoteError::InvalidFrontmatter(err.to_string()))?;
    let id = frontmatter.id.ok_or(NoteError::MissingId)?;
    if !is_segment(&id) {
        return Err(NoteError::InvalidFrontmatter(
            "id は小文字英数とハイフン".into(),
        ));
    }
    if concept != id {
        return Err(NoteError::FilenameIdMismatch {
            filename: concept,
            id,
        });
    }
    let (sections, heading) = sections(body)?;
    let title = heading.unwrap_or_else(|| id.clone());
    Ok(Note {
        id,
        title,
        aliases: frontmatter.aliases,
        sources: frontmatter.sources,
        major,
        middle,
        minor,
        concept,
        sections,
    })
}

#[derive(Deserialize)]
struct Frontmatter {
    id: Option<String>,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    sources: Vec<String>,
}

pub(crate) fn parse_location(path: &str) -> Result<(String, String, String, String), NoteError> {
    let mut parts = path.split('/');
    let (Some("learning"), Some(major), Some(middle), Some(minor), Some(file)) = (
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
    ) else {
        return Err(NoteError::InvalidPath);
    };
    if parts.next().is_some() || !file.ends_with(".md") {
        return Err(NoteError::InvalidPath);
    }
    let concept = &file[..file.len() - 3];
    if major.is_empty() || middle.is_empty() || minor.is_empty() || concept.is_empty() {
        return Err(NoteError::InvalidPath);
    }
    for segment in [major, middle, minor, concept] {
        if !is_segment(segment) {
            return Err(NoteError::InvalidSegment(segment.to_owned()));
        }
    }
    Ok((
        major.to_owned(),
        middle.to_owned(),
        minor.to_owned(),
        concept.to_owned(),
    ))
}

fn split_frontmatter(markdown: &str) -> Result<(&str, &str), NoteError> {
    let Some(rest) = markdown.strip_prefix("---\n") else {
        return Err(NoteError::MissingFrontmatter);
    };
    let Some(end) = rest.find("\n---\n") else {
        return Err(NoteError::InvalidFrontmatter(
            "frontmatter が閉じていない".into(),
        ));
    };
    Ok((&rest[..end], &rest[end + "\n---\n".len()..]))
}

fn sections(body: &str) -> Result<(Vec<Section>, Option<String>), NoteError> {
    let mut sections = Vec::new();
    let mut title = None;
    let mut current: Option<Level> = None;
    let mut heading: Option<HeadingLevel> = None;
    let mut heading_text = String::new();
    let mut item_depth = 0u32;
    let mut code_depth = 0u32;
    let mut open: Option<Item> = None;
    let mut nested = String::new();

    for (event, _range) in Parser::new(body).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                heading = Some(level);
                heading_text.clear();
            }
            Event::End(TagEnd::Heading(level)) => {
                finish_heading(
                    level,
                    &heading_text,
                    &mut sections,
                    &mut current,
                    &mut title,
                )?;
                heading = None;
            }
            Event::Text(text) | Event::Code(text) if heading.is_some() => {
                heading_text.push_str(&text);
            }
            Event::Start(Tag::CodeBlock(_)) => code_depth += 1,
            Event::End(TagEnd::CodeBlock) => {
                code_depth = code_depth.saturating_sub(1);
            }
            Event::Start(Tag::Item) => {
                item_depth += 1;
                if item_depth == 1 {
                    open = Some(Item {
                        answer: String::new(),
                        question: None,
                        rubric: None,
                        refs: Vec::new(),
                    });
                }
            }
            Event::End(TagEnd::Item) => {
                if item_depth == 0 {
                    return Err(NoteError::ItemOutsideSection);
                }
                if item_depth == 2 {
                    if let Some(item) = open.as_mut() {
                        absorb_nested(item, &nested);
                    }
                    nested.clear();
                } else if item_depth == 1 {
                    let Some(mut item) = open.take() else {
                        return Err(NoteError::ItemOutsideSection);
                    };
                    let Some(level) = current else {
                        return Err(NoteError::ItemOutsideSection);
                    };
                    item.answer = item.answer.trim().to_owned();
                    push_item(&mut sections, level, item);
                }
                item_depth -= 1;
            }
            Event::Text(text) | Event::Code(text) if code_depth == 0 && heading.is_none() => {
                append_item_text(&mut open, &mut nested, item_depth, &text);
            }
            Event::SoftBreak | Event::HardBreak if code_depth == 0 && heading.is_none() => {
                append_item_text(&mut open, &mut nested, item_depth, "\n");
            }
            _ => {}
        }
    }

    if !sections
        .iter()
        .any(|section| section.level == Level::Beginner)
    {
        return Err(NoteError::MissingBeginner);
    }
    Ok((sections, title))
}

fn finish_heading(
    level: HeadingLevel,
    text: &str,
    sections: &mut Vec<Section>,
    current: &mut Option<Level>,
    title: &mut Option<String>,
) -> Result<(), NoteError> {
    if level == HeadingLevel::H1 {
        if title.is_none() {
            let text = text.trim();
            if !text.is_empty() {
                *title = Some(text.to_owned());
            }
        }
        return Ok(());
    }
    if level != HeadingLevel::H2 {
        return Err(NoteError::UnknownHeading(text.trim().to_owned()));
    }
    let name = text.trim();
    let Some(parsed) = level_of(name) else {
        return Err(NoteError::UnknownHeading(name.to_owned()));
    };
    if sections.iter().any(|section| section.level == parsed) {
        return Err(NoteError::DuplicateSection(name.to_owned()));
    }
    sections.push(Section {
        level: parsed,
        items: Vec::new(),
    });
    *current = Some(parsed);
    Ok(())
}

fn level_of(name: &str) -> Option<Level> {
    match name {
        "初級" => Some(Level::Beginner),
        "中級" => Some(Level::Intermediate),
        "上級" => Some(Level::Advanced),
        "統合" => Some(Level::Integration),
        _ => None,
    }
}

fn push_item(sections: &mut [Section], level: Level, item: Item) {
    if let Some(section) = sections.iter_mut().find(|section| section.level == level) {
        section.items.push(item);
    }
}

fn append_item_text(open: &mut Option<Item>, nested: &mut String, item_depth: u32, text: &str) {
    if item_depth >= 2 {
        nested.push_str(text);
    } else if let Some(item) = open.as_mut() {
        item.answer.push_str(text);
    }
}

fn absorb_nested(item: &mut Item, raw: &str) {
    let text = raw.trim();
    if let Some(rest) = text.strip_prefix("Q:") {
        if item.question.is_none() {
            item.question = Some(rest.trim().to_owned());
        }
    } else if let Some(rest) = text.strip_prefix("採点:") {
        if item.rubric.is_none() {
            item.rubric = Some(rest.trim().to_owned());
        }
    } else if let Some(rest) = text.strip_prefix("参照:") {
        item.refs.extend(split_refs(rest));
    }
}

fn split_refs(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(['、', ',', '，'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
}

fn qkey(question: &str) -> String {
    let nfc: String = question.nfc().collect();
    let digest = Sha256::digest(nfc.as_bytes());
    digest.iter().take(8).fold(String::new(), |mut out, byte| {
        out.push_str(&format!("{byte:02x}"));
        out
    })
}

/// 仕様の `^[a-z0-9][a-z0-9-]*$`。
fn is_segment(text: &str) -> bool {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() || first.is_ascii_digit() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}
