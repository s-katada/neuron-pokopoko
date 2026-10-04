use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use serde::Deserialize;
use thiserror::Error;

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
    pub items: Vec<String>,
}

/// 1 ノート。分類と概念名はパス由来。時刻は持たない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub id: String,
    pub sources: Vec<String>,
    pub major: String,
    pub middle: String,
    pub minor: String,
    pub concept: String,
    pub sections: Vec<Section>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NoteError {
    #[error("ノートのパスが learning/<大>/<中>/<小>/<概念>.md ではない")]
    InvalidPath,
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

/// `relative_path` は vault 相対の `learning/<大>/<中>/<小>/<概念>.md`。
pub fn parse_note(relative_path: &str, markdown: &str) -> Result<Note, NoteError> {
    let (major, middle, minor, concept) = location(relative_path)?;
    let (yaml, body) = split_frontmatter(markdown)?;
    let frontmatter: Frontmatter = serde_yaml_ng::from_str(yaml)
        .map_err(|err| NoteError::InvalidFrontmatter(err.to_string()))?;
    let id = frontmatter.id.ok_or(NoteError::MissingId)?;
    if !is_kebab(&id) {
        return Err(NoteError::InvalidFrontmatter(
            "id は小文字ケバブケース".into(),
        ));
    }
    let sections = sections(body)?;
    Ok(Note {
        id,
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
    sources: Vec<String>,
}

fn location(path: &str) -> Result<(String, String, String, String), NoteError> {
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
    if parts.next().is_some()
        || major.is_empty()
        || middle.is_empty()
        || minor.is_empty()
        || !file.ends_with(".md")
    {
        return Err(NoteError::InvalidPath);
    }
    let concept = &file[..file.len() - 3];
    if concept.is_empty() {
        return Err(NoteError::InvalidPath);
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

fn sections(body: &str) -> Result<Vec<Section>, NoteError> {
    let mut sections = Vec::new();
    let mut current: Option<Level> = None;
    let mut heading: Option<HeadingLevel> = None;
    let mut heading_text = String::new();
    let mut item_depth = 0u32;

    for (event, range) in Parser::new(body).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                heading = Some(level);
                heading_text.clear();
            }
            Event::End(TagEnd::Heading(level)) => {
                finish_heading(level, &heading_text, &mut sections, &mut current)?;
                heading = None;
            }
            Event::Text(text) | Event::Code(text) if heading.is_some() => {
                heading_text.push_str(&text);
            }
            Event::Start(Tag::Item) => item_depth += 1,
            Event::End(TagEnd::Item) => {
                if item_depth == 0 {
                    return Err(NoteError::ItemOutsideSection);
                }
                item_depth -= 1;
                if item_depth == 0 {
                    let Some(level) = current else {
                        return Err(NoteError::ItemOutsideSection);
                    };
                    let item = body[range].trim().to_owned();
                    push_item(&mut sections, level, item);
                }
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
    Ok(sections)
}

fn finish_heading(
    level: HeadingLevel,
    text: &str,
    sections: &mut Vec<Section>,
    current: &mut Option<Level>,
) -> Result<(), NoteError> {
    if level == HeadingLevel::H1 {
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

fn push_item(sections: &mut [Section], level: Level, item: String) {
    if let Some(section) = sections.iter_mut().find(|section| section.level == level) {
        section.items.push(item);
    }
}

fn is_kebab(id: &str) -> bool {
    !id.is_empty()
        && id.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}
