use axum::http::StatusCode;
use axum::{Json, Router, routing::get};
use poko_core::{Card, Rating, schedule};
use serde::Serialize;
use tower_service::Service;
use worker::*;

fn router() -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/demo-schedule", get(demo_schedule))
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

#[event(fetch)]
async fn fetch(
    req: HttpRequest,
    _env: Env,
    _ctx: Context,
) -> Result<axum::http::Response<axum::body::Body>> {
    console_error_panic_hook::set_once();

    let mut app = router();
    let resp = app
        .call(req)
        .await
        .map_err(|err| worker::Error::RustError(format!("axum error: {err}")))?;

    Ok(resp)
}
