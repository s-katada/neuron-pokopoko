use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::{Json, Router, routing::get};
use poko_core::{Card, Rating, schedule};
use serde::{Deserialize, Serialize};
use tower_service::Service;
use wasm_bindgen::JsValue;
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
        .route("/api/demo-d1", get(demo_d1_count).post(demo_d1_insert))
        .with_state(state))
}

#[derive(Serialize)]
struct HealthBody {
    ok: bool,
}

async fn health() -> Json<HealthBody> {
    Json(HealthBody { ok: true })
}

#[derive(Serialize)]
struct DemoSchedule {
    rating: &'static str,
    interval_days: f32,
    stability: f32,
    difficulty: f32,
}

async fn demo_schedule() -> Result<Json<DemoSchedule>, StatusCode> {
    let now_unix = i64::try_from(Date::now().as_millis() / 1000).unwrap_or(i64::MAX);
    let scheduled = schedule(Card::New, Rating::Good, now_unix)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(DemoSchedule {
        rating: "good",
        interval_days: scheduled.interval_days,
        stability: scheduled.memory.stability,
        difficulty: scheduled.memory.difficulty,
    }))
}

#[derive(Serialize)]
struct InsertBody {
    ok: bool,
}

#[worker::send]
async fn demo_d1_insert(State(state): State<AppState>) -> Result<Json<InsertBody>, StatusCode> {
    state
        .db
        .prepare("INSERT INTO spike_entries (value) VALUES (?)")
        .bind(&[JsValue::from_str("spike")])
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .run()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(InsertBody { ok: true }))
}

#[derive(Deserialize)]
struct CountRow {
    count: i64,
}

#[derive(Serialize)]
struct CountBody {
    count: i64,
}

#[worker::send]
async fn demo_d1_count(State(state): State<AppState>) -> Result<Json<CountBody>, StatusCode> {
    let row: Option<CountRow> = state
        .db
        .prepare("SELECT COUNT(*) AS count FROM spike_entries")
        .first(None)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let count = row.map(|row| row.count).unwrap_or(0);
    Ok(Json(CountBody { count }))
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
