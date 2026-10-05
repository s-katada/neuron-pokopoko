use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::note::{Card, Note};

/// 同期するノート。`content_hash` 以外を固定順の JSON にして SHA-256 する。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncNote {
    pub id: String,
    pub path: String,
    pub title: String,
    pub major: String,
    pub middle: String,
    pub minor: String,
    pub content_hash: String,
    pub cards: Vec<SyncCard>,
}

/// 同期するカード。段は `beginner` などの文字列。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncCard {
    pub stable_key: String,
    pub level: String,
    pub question: String,
    pub answer: String,
    pub rubric: Option<String>,
    pub refs: Vec<String>,
}

/// リモートが持つ id と内容ハッシュ。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub id: String,
    pub content_hash: String,
}

/// ローカルと manifest の差分。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncPlan<'a> {
    pub upserts: Vec<&'a SyncNote>,
    pub deletes: Vec<String>,
    pub unchanged: usize,
}

impl SyncNote {
    /// パース済みノートから同期ペイロードを作る。
    pub fn from_note(note: &Note) -> Self {
        let mut sync = Self {
            id: note.id.clone(),
            path: format!(
                "learning/{}/{}/{}/{}.md",
                note.major, note.middle, note.minor, note.id
            ),
            title: note.title.clone(),
            major: note.major.clone(),
            middle: note.middle.clone(),
            minor: note.minor.clone(),
            content_hash: String::new(),
            cards: note.cards().into_iter().map(SyncCard::from).collect(),
        };
        sync.content_hash = content_hash(&sync);
        sync
    }
}

impl From<Card> for SyncCard {
    fn from(card: Card) -> Self {
        Self {
            stable_key: card.stable_key,
            level: card.level.as_str().to_owned(),
            question: card.question,
            answer: card.answer,
            rubric: card.rubric,
            refs: card.refs,
        }
    }
}

/// ハッシュが違う、またはリモートに無いノートを upsert する。
pub fn plan<'a>(local: &'a [SyncNote], remote: &[ManifestEntry]) -> SyncPlan<'a> {
    let mut remote_hash = std::collections::BTreeMap::new();
    for entry in remote {
        remote_hash.insert(entry.id.as_str(), entry.content_hash.as_str());
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut upserts = Vec::new();
    let mut unchanged = 0;
    for note in local {
        seen.insert(note.id.as_str());
        match remote_hash.get(note.id.as_str()) {
            Some(hash) if *hash == note.content_hash => unchanged += 1,
            _ => upserts.push(note),
        }
    }
    let deletes = remote
        .iter()
        .filter(|entry| !seen.contains(entry.id.as_str()))
        .map(|entry| entry.id.clone())
        .collect();
    SyncPlan {
        upserts,
        deletes,
        unchanged,
    }
}

fn content_hash(note: &SyncNote) -> String {
    #[derive(Serialize)]
    struct Body<'a> {
        id: &'a str,
        path: &'a str,
        title: &'a str,
        major: &'a str,
        middle: &'a str,
        minor: &'a str,
        cards: &'a [SyncCard],
    }
    let body = Body {
        id: &note.id,
        path: &note.path,
        title: &note.title,
        major: &note.major,
        middle: &note.middle,
        minor: &note.minor,
        cards: &note.cards,
    };
    let json = serde_json::to_string(&body).expect("sync payload is JSON-serializable");
    hex(&Sha256::digest(json.as_bytes()))
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}
