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
}

/// `relative_path` は vault 相対の `learning/<大>/<中>/<小>/<概念>.md`。
/// 本文の段はまだ切らない。
pub fn parse_note(relative_path: &str, markdown: &str) -> Result<Note, NoteError> {
    let (major, middle, minor, concept) = location(relative_path)?;
    let yaml = split_frontmatter(markdown)?;
    let frontmatter: Frontmatter = serde_yaml_ng::from_str(yaml)
        .map_err(|err| NoteError::InvalidFrontmatter(err.to_string()))?;
    let id = frontmatter.id.ok_or(NoteError::MissingId)?;
    if !is_kebab(&id) {
        return Err(NoteError::InvalidFrontmatter(
            "id は小文字ケバブケース".into(),
        ));
    }
    Ok(Note {
        id,
        sources: frontmatter.sources,
        major,
        middle,
        minor,
        concept,
        sections: Vec::new(),
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

fn split_frontmatter(markdown: &str) -> Result<&str, NoteError> {
    let Some(rest) = markdown.strip_prefix("---\n") else {
        return Err(NoteError::MissingFrontmatter);
    };
    let Some(end) = rest.find("\n---\n") else {
        return Err(NoteError::InvalidFrontmatter(
            "frontmatter が閉じていない".into(),
        ));
    };
    Ok(&rest[..end])
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
