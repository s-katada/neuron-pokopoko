use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::note::{Card, Note, NoteError};

/// 1 リクエストに載せる SQL 文の上限。D1 Free は 50。
pub const MAX_STATEMENTS_PER_REQUEST: usize = 40;
/// 1 文のバインドパラメータ上限。D1 Free は 100。
pub const MAX_BIND_PARAMS: usize = 100;
/// `IN (?, ...)` の削除 id 上限。バインド上限より小さく取る。
pub const MAX_DELETE_IDS_PER_REQUEST: usize = 90;

const _: () = assert!(MAX_DELETE_IDS_PER_REQUEST <= MAX_BIND_PARAMS);

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

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("ノート {id} は 1 リクエストに入らない")]
    NoteTooLarge { id: String },
    #[error("{path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: NoteError,
    },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// ノート upsert 1 + カード upsert N + 消えたカードの retire 1。
pub fn statements_for(note: &SyncNote) -> usize {
    note.cards.len() + 2
}

/// 文数の合計が [`MAX_STATEMENTS_PER_REQUEST`] 以下になるよう分ける。
pub fn chunk<'a>(upserts: &[&'a SyncNote]) -> Result<Vec<Vec<&'a SyncNote>>, SyncError> {
    let mut chunks = Vec::new();
    let mut current = Vec::new();
    let mut used = 0;
    for note in upserts {
        let cost = statements_for(note);
        if cost > MAX_STATEMENTS_PER_REQUEST {
            return Err(SyncError::NoteTooLarge {
                id: note.id.clone(),
            });
        }
        if used > 0 && used + cost > MAX_STATEMENTS_PER_REQUEST {
            chunks.push(std::mem::take(&mut current));
            used = 0;
        }
        current.push(*note);
        used += cost;
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    Ok(chunks)
}

/// 削除 id を [`MAX_DELETE_IDS_PER_REQUEST`] 件ずつに分ける。
pub fn chunk_deletes(ids: &[String]) -> Vec<Vec<String>> {
    ids.chunks(MAX_DELETE_IDS_PER_REQUEST)
        .map(<[String]>::to_vec)
        .collect()
}

/// `learning/` 配下のノートを同期ペイロードにする。
pub fn collect(vault_root: &Path) -> Result<Vec<SyncNote>, SyncError> {
    let mut notes = Vec::new();
    for path in crate::vault::learning_markdown(vault_root)? {
        let rel = crate::vault::relative(vault_root, &path);
        let markdown = std::fs::read_to_string(&path)?;
        let note = crate::note::parse_note(&rel, &markdown)
            .map_err(|source| SyncError::Parse { path: rel, source })?;
        notes.push(SyncNote::from_note(&note));
    }
    Ok(notes)
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

/// D1 に渡す値。
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
}

/// 実行する SQL 1 文。`IN` 句の `?` 数が変わるため sql は所有する。
#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    pub sql: String,
    pub params: Vec<Value>,
}

const UPSERT_NOTE: &str = "INSERT INTO notes (id, path, title, major, middle, minor, content_hash, deleted_at, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, NULL, ?, ?) ON CONFLICT(id) DO UPDATE SET path = excluded.path, title = excluded.title, major = excluded.major, middle = excluded.middle, minor = excluded.minor, content_hash = excluded.content_hash, deleted_at = NULL, updated_at = excluded.updated_at";

const UPSERT_CARD: &str = "INSERT INTO cards (stable_key, note_id, level, question, answer, rubric, refs, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(stable_key) DO UPDATE SET note_id = excluded.note_id, level = excluded.level, question = excluded.question, answer = excluded.answer, rubric = excluded.rubric, refs = excluded.refs, retired_at = NULL, updated_at = excluded.updated_at";

const RETIRE_ALL: &str =
    "UPDATE cards SET retired_at = ?, updated_at = ? WHERE note_id = ? AND retired_at IS NULL";

/// ノート upsert、カード upsert、消えたカードの retire の順。
pub fn upsert_statements(notes: &[&SyncNote], now_unix: i64) -> Vec<Statement> {
    let mut out = Vec::new();
    for note in notes {
        out.push(Statement {
            sql: UPSERT_NOTE.to_owned(),
            params: vec![
                text(&note.id),
                text(&note.path),
                text(&note.title),
                text(&note.major),
                text(&note.middle),
                text(&note.minor),
                text(&note.content_hash),
                Value::Integer(now_unix),
                Value::Integer(now_unix),
            ],
        });
        for card in &note.cards {
            out.push(Statement {
                sql: UPSERT_CARD.to_owned(),
                params: vec![
                    text(&card.stable_key),
                    text(&note.id),
                    text(&card.level),
                    text(&card.question),
                    text(&card.answer),
                    match &card.rubric {
                        Some(rubric) => text(rubric),
                        None => Value::Null,
                    },
                    text(&serde_json::to_string(&card.refs).expect("refs は JSON になる")),
                    Value::Integer(now_unix),
                    Value::Integer(now_unix),
                ],
            });
        }
        let mut params = vec![
            Value::Integer(now_unix),
            Value::Integer(now_unix),
            text(&note.id),
        ];
        params.extend(note.cards.iter().map(|card| text(&card.stable_key)));
        out.push(Statement {
            sql: retire_sql(note.cards.len()),
            params,
        });
    }
    out
}

/// ノートの論理削除と、配下カードの retire。
pub fn delete_statements(ids: &[String], now_unix: i64) -> Vec<Statement> {
    let list = placeholders(ids.len());
    let id_params: Vec<Value> = ids.iter().map(|id| text(id)).collect();
    vec![
        Statement {
            sql: format!(
                "UPDATE notes SET deleted_at = ?, updated_at = ? WHERE id IN ({list}) AND deleted_at IS NULL"
            ),
            params: stamp(now_unix, &id_params),
        },
        Statement {
            sql: format!(
                "UPDATE cards SET retired_at = ?, updated_at = ? WHERE note_id IN ({list}) AND retired_at IS NULL"
            ),
            params: stamp(now_unix, &id_params),
        },
    ]
}

fn retire_sql(cards: usize) -> String {
    if cards == 0 {
        return RETIRE_ALL.to_owned();
    }
    format!(
        "UPDATE cards SET retired_at = ?, updated_at = ? WHERE note_id = ? AND retired_at IS NULL AND stable_key NOT IN ({})",
        placeholders(cards)
    )
}

fn placeholders(count: usize) -> String {
    std::iter::repeat_n("?", count)
        .collect::<Vec<_>>()
        .join(", ")
}

fn stamp(now_unix: i64, ids: &[Value]) -> Vec<Value> {
    let mut params = vec![Value::Integer(now_unix), Value::Integer(now_unix)];
    params.extend(ids.iter().cloned());
    params
}

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
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
