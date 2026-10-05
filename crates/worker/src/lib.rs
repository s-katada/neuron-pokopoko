use std::sync::Arc;

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::{StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::{
    Router,
    routing::{get, post},
};
use poko_core::{
    AnswerRequest, AnswerResponse, CardState, MAX_DELETE_IDS_PER_REQUEST,
    MAX_STATEMENTS_PER_REQUEST, ManifestEntry, NextResponse, Rating, ReviewCardRow, StatCard,
    Statement, SyncNote, Value, answer_statements, build_tree, card_state_query, daily_counts,
    daily_reviews_query, delete_statements, next_card_query, normalize_response, note_detail,
    note_stat_cards_query, review_card_from_row, stat_cards_query, statements_for,
    upsert_statements,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tower_service::Service;
use worker::wasm_bindgen::JsValue;
use worker::*;

#[derive(Clone)]
struct AppState {
    db: Arc<D1Database>,
}

fn router(env: Env) -> Result<Router> {
    let state = AppState {
        db: Arc::new(env.d1("DB")?),
    };
    Ok(Router::new()
        .route("/api/health", get(health))
        .route("/api/sync/manifest", get(sync_manifest))
        .route("/api/sync/notes", post(sync_notes))
        .route("/api/sync/delete", post(sync_delete))
        .route("/api/review/next", get(review_next))
        .route("/api/review/answer", post(review_answer))
        .route("/api/tree", get(tree))
        .route("/api/notes/{id}", get(note_stats))
        .route("/api/stats/daily", get(daily_stats))
        .with_state(state))
}

#[derive(Serialize)]
struct HealthBody {
    ok: bool,
}

async fn health() -> Json<HealthBody> {
    Json(HealthBody { ok: true })
}

#[derive(Deserialize)]
struct NotesBody {
    notes: Vec<SyncNote>,
}

#[derive(Serialize)]
struct NotesResult {
    notes: usize,
    cards: usize,
}

#[derive(Deserialize)]
struct DeleteBody {
    ids: Vec<String>,
}

#[derive(Serialize)]
struct DeleteResult {
    deleted: usize,
}

#[worker::send]
async fn sync_manifest(State(state): State<AppState>) -> Response {
    let queried = state
        .db
        .prepare("SELECT id, content_hash FROM notes WHERE deleted_at IS NULL ORDER BY id")
        .all()
        .await;
    let result = match queried {
        Ok(result) => result,
        Err(err) => return server_error(err),
    };
    match result.results::<ManifestEntry>() {
        Ok(entries) => Json(entries).into_response(),
        Err(err) => server_error(err),
    }
}

#[worker::send]
async fn sync_notes(
    State(state): State<AppState>,
    body: Result<Json<NotesBody>, JsonRejection>,
) -> Response {
    let Json(body) = match body {
        Ok(body) => body,
        Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    };
    let statements: usize = body.notes.iter().map(statements_for).sum();
    if statements > MAX_STATEMENTS_PER_REQUEST {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("文数が {MAX_STATEMENTS_PER_REQUEST} を超えている"),
        )
            .into_response();
    }
    let notes: Vec<&SyncNote> = body.notes.iter().collect();
    let cards: usize = notes.iter().map(|note| note.cards.len()).sum();
    let generated = upsert_statements(&notes, now_unix());
    if let Err(err) = execute_batch(&state.db, &generated).await {
        return server_error(err);
    }
    Json(NotesResult {
        notes: notes.len(),
        cards,
    })
    .into_response()
}

#[worker::send]
async fn sync_delete(
    State(state): State<AppState>,
    body: Result<Json<DeleteBody>, JsonRejection>,
) -> Response {
    let Json(body) = match body {
        Ok(body) => body,
        Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    };
    if body.ids.len() > MAX_DELETE_IDS_PER_REQUEST {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("削除件数が {MAX_DELETE_IDS_PER_REQUEST} を超えている"),
        )
            .into_response();
    }
    let deleted = body.ids.len();
    let generated = delete_statements(&body.ids, now_unix());
    if deleted > 0
        && let Err(err) = execute_batch(&state.db, &generated).await
    {
        return server_error(err);
    }
    Json(DeleteResult { deleted }).into_response()
}

#[worker::send]
async fn review_next(State(state): State<AppState>) -> Response {
    match query_first::<ReviewCardRow>(&state.db, &next_card_query(now_unix())).await {
        Ok(Some(row)) => match review_card_from_row(row) {
            Ok(card) => Json(NextResponse { card: Some(card) }).into_response(),
            Err(err) => server_error(err),
        },
        Ok(None) => Json(NextResponse { card: None }).into_response(),
        Err(err) => server_error(err),
    }
}

#[worker::send]
async fn review_answer(
    State(state): State<AppState>,
    body: Result<Json<AnswerRequest>, JsonRejection>,
) -> Response {
    let Json(body) = match body {
        Ok(body) => body,
        Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    };
    let rating = match Rating::parse(&body.rating) {
        Some(rating) => rating,
        None => return (StatusCode::BAD_REQUEST, "rating が不正".to_owned()).into_response(),
    };
    let response = match normalize_response(body.response.as_deref()) {
        Ok(response) => response,
        Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    };
    let card_state =
        match query_first::<CardState>(&state.db, &card_state_query(&body.stable_key)).await {
            Ok(Some(card_state)) => card_state,
            Ok(None) => return (StatusCode::NOT_FOUND, "カードがない".to_owned()).into_response(),
            Err(err) => return server_error(err),
        };
    let answer = match answer_statements(
        &body.stable_key,
        &card_state,
        rating,
        now_unix(),
        response.as_deref(),
    ) {
        Ok(answer) => answer,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "スケジュールに失敗した".to_owned(),
            )
                .into_response();
        }
    };
    if let Err(err) = execute_batch(&state.db, &answer.statements).await {
        return server_error(err);
    }
    Json(AnswerResponse {
        interval_days: answer.interval_days,
        due_at: answer.due_at,
    })
    .into_response()
}

#[worker::send]
async fn tree(State(state): State<AppState>) -> Response {
    let now = now_unix();
    match query_all::<StatCard>(&state.db, &stat_cards_query()).await {
        Ok(rows) => Json(build_tree(&rows, now)).into_response(),
        Err(err) => server_error(err),
    }
}

#[worker::send]
async fn note_stats(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let now = now_unix();
    match query_all::<StatCard>(&state.db, &note_stat_cards_query(&id)).await {
        Ok(rows) => match note_detail(&rows, &id, now) {
            Some(detail) => Json(detail).into_response(),
            None => (StatusCode::NOT_FOUND, "ノートがない".to_owned()).into_response(),
        },
        Err(err) => server_error(err),
    }
}

#[derive(Deserialize)]
struct ReviewedAt {
    reviewed_at: i64,
}

#[worker::send]
async fn daily_stats(State(state): State<AppState>, uri: Uri) -> Response {
    let days = match parse_days(uri.query()) {
        Ok(days) => days,
        Err(message) => return (StatusCode::BAD_REQUEST, message.to_owned()).into_response(),
    };
    let now = now_unix();
    match query_all::<ReviewedAt>(&state.db, &daily_reviews_query(now, days)).await {
        Ok(rows) => {
            let reviewed_at: Vec<i64> = rows.into_iter().map(|row| row.reviewed_at).collect();
            Json(daily_counts(&reviewed_at, now, days)).into_response()
        }
        Err(err) => server_error(err),
    }
}

fn parse_days(query: Option<&str>) -> Result<usize, &'static str> {
    let Some(query) = query.filter(|text| !text.is_empty()) else {
        return Ok(30);
    };
    let mut days = None;
    for pair in query.split('&') {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        if key != "days" {
            continue;
        }
        let parsed: usize = value.parse().map_err(|_| "days が不正")?;
        if !(1..=90).contains(&parsed) {
            return Err("days は 1 から 90");
        }
        days = Some(parsed);
    }
    Ok(days.unwrap_or(30))
}

async fn query_all<T: DeserializeOwned>(db: &D1Database, statement: &Statement) -> Result<Vec<T>> {
    let params: Vec<JsValue> = statement.params.iter().map(to_d1).collect();
    let result = db
        .prepare(statement.sql.as_str())
        .bind(&params)?
        .all()
        .await?;
    result
        .results()
        .map_err(|err| Error::RustError(err.to_string()))
}

async fn query_first<T: DeserializeOwned>(
    db: &D1Database,
    statement: &Statement,
) -> Result<Option<T>> {
    let params: Vec<JsValue> = statement.params.iter().map(to_d1).collect();
    db.prepare(statement.sql.as_str())
        .bind(&params)?
        .first(None)
        .await
}

async fn execute_batch(db: &D1Database, statements: &[Statement]) -> Result<()> {
    if statements.is_empty() {
        return Ok(());
    }
    let mut prepared = Vec::with_capacity(statements.len());
    for statement in statements {
        let params: Vec<JsValue> = statement.params.iter().map(to_d1).collect();
        prepared.push(db.prepare(statement.sql.as_str()).bind(&params)?);
    }
    let results = db.batch(prepared).await?;
    for result in &results {
        if !result.success() {
            let message = result
                .error()
                .unwrap_or_else(|| "D1 batch が失敗した".into());
            return Err(Error::RustError(message));
        }
    }
    Ok(())
}

fn to_d1(value: &Value) -> JsValue {
    match value {
        Value::Null => JsValue::NULL,
        Value::Integer(n) => JsValue::from_f64(*n as f64),
        Value::Real(n) => JsValue::from_f64(*n),
        Value::Text(text) => JsValue::from_str(text),
    }
}

fn now_unix() -> i64 {
    i64::try_from(Date::now().as_millis() / 1000).unwrap_or(i64::MAX)
}

fn server_error(err: impl std::fmt::Display) -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response()
}

#[event(fetch)]
async fn fetch(
    req: HttpRequest,
    env: Env,
    _ctx: Context,
) -> Result<axum::http::Response<axum::body::Body>> {
    console_error_panic_hook::set_once();

    let mut app = router(env)?;
    let resp = app
        .call(req)
        .await
        .map_err(|err| worker::Error::RustError(format!("axum error: {err}")))?;

    Ok(resp)
}
