//! `GET /v1/health` — liveness + version + uptime.

use axum::Json;
use serde::Serialize;
use std::sync::OnceLock;
use std::time::Instant;

pub static START_TIME: OnceLock<Instant> = OnceLock::new();

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
    pub uptime_secs: u64,
}

pub async fn health() -> Json<HealthResponse> {
    let uptime = START_TIME.get().map_or(0, |t| t.elapsed().as_secs());
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        uptime_secs: uptime,
    })
}
