//! `/v1/embed` handler — proxy to DashScope's text-embedding-v3 (default).
//! Reads through EmbedCache to avoid re-paying for identical (model, text) pairs.

use anyhow::Context;
use axum::{extract::State, Json};
use tracing::instrument;

use crate::proto::embed::{EmbedRequest, EmbedResponse};
use crate::server::{error::AppError, AppState};

#[instrument(skip(state, req), fields(model = %req.model, n_inputs = req.input.len()))]
pub async fn embed(
    State(state): State<AppState>,
    Json(req): Json<EmbedRequest>,
) -> Result<Json<EmbedResponse>, AppError> {
    let provider = state
        .router
        .provider_handle("dashscope")
        .context("dashscope provider not configured")?;
    let vectors = state
        .embed_cache
        .embed_or_compute(&provider, &req.model, req.input)
        .await?;
    let dim = vectors.first().map(|v| v.len()).unwrap_or(0);
    Ok(Json(EmbedResponse {
        model: req.model,
        vectors,
        dim,
    }))
}
