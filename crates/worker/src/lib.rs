use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{
    Router,
    routing::{get, post},
};
use poko_core::{
    Card, DemoSchedule, MAX_DELETE_IDS_PER_REQUEST, MAX_STATEMENTS_PER_REQUEST, ManifestEntry,
    Rating, Statement, SyncNote, Value, delete_statements, schedule, statements_for,
    upsert_statements,
};
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
        .route("/api/demo-schedule", get(demo_schedule))
        .route("/api/sync/manifest", get(sync_manifest))
        .route("/api/sync/notes", post(sync_notes))
        .route("/api/sync/delete", post(sync_delete))
        .with_state(state))
}

#[derive(Serialize)]
struct HealthBody {
    ok: bool,
}

async fn health() -> Json<HealthBody> {
    Json(HealthBody { ok: true })
}

async fn demo_schedule() -> Result<Json<DemoSchedule>, StatusCode> {
    let now_unix = now_unix();
    let scheduled = schedule(Card::New, Rating::Good, now_unix)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(DemoSchedule {
        rating: "good",
        interval_days: scheduled.interval_days,
        stability: scheduled.memory.stability,
        difficulty: scheduled.memory.difficulty,
    }))
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
