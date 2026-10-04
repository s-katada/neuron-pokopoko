use axum::{Json, Router, routing::get};
use serde::Serialize;
use tower_service::Service;
use worker::*;

fn router() -> Router {
    Router::new().route("/api/health", get(health))
}

#[derive(Serialize)]
struct HealthBody {
    ok: bool,
}

async fn health() -> Json<HealthBody> {
    Json(HealthBody { ok: true })
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
