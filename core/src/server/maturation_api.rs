//! Maturation API — REST endpoints for memory maturation subsystem.
//!
//! - `GET  /v1/memory/maturation/status`      — consolidation state, fragment counts, decay_factor
//! - `POST /v1/memory/maturation/consolidate`  — trigger immediate consolidation (202 / 409)
//! - `GET  /v1/memory/handoff`                 — current handoff summary (200 / 404)
//! - `DELETE /v1/memory/handoff`               — clear pending handoff (204)
//! - `GET  /v1/memory/:fragment_id/metadata`   — fragment metadata with effective_score (200 / 404)

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use serde::Serialize;
use tracing::warn;
use uuid::Uuid;

use super::AppState;
use crate::memory::consolidator::ConsolidationCmd;
use crate::memory::decay::{self, DecayConfig};
use crate::memory::handoff;
use crate::memory::store::MemoryStore;
use crate::memory::tier::MemoryTier;

// ─── Response Types ──────────────────────────────────────────────────────────

/// Response for `GET /v1/memory/maturation/status`.
#[derive(Debug, Serialize)]
pub struct MaturationStatusResponse {
    pub consolidation_state: &'static str,
    pub last_consolidation: Option<String>,
    pub fragment_counts: FragmentCounts,
    pub decay_factor: f64,
}

/// Fragment counts per tier.
#[derive(Debug, Serialize)]
pub struct FragmentCounts {
    pub working: usize,
    pub episodic: usize,
    pub semantic: usize,
    pub procedural: usize,
}

/// Response for `POST /v1/memory/maturation/consolidate`.
#[derive(Debug, Serialize)]
pub struct ConsolidateResponse {
    pub triggered_at: String,
}

/// Response for `GET /v1/memory/:fragment_id/metadata`.
#[derive(Debug, Serialize)]
pub struct FragmentMetadataResponse {
    pub fragment_id: String,
    pub tier: String,
    pub access_count: u64,
    pub last_accessed: Option<String>,
    pub created_at: String,
    pub consolidated_into: Option<String>,
    pub effective_score: f64,
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// GET /v1/memory/maturation/status
///
/// Returns consolidation state, fragment counts per tier, and current decay_factor.
/// When maturation is disabled, still returns status with consolidation_state = "idle".
pub async fn status(State(state): State<AppState>) -> impl IntoResponse {
    let maturation_cfg = state
        .tokens
        .as_ref()
        .map(|t| &t.memory.maturation);

    let decay_factor = maturation_cfg
        .map_or(0.95, |c| c.decay_factor);

    // Determine consolidation state from ConsolidatorState if available.
    let consolidation_state = match state.consolidator_state.as_ref() {
        Some(cs) if cs.is_running() => "running",
        _ => "idle",
    };

    // Get last consolidation timestamp.
    let last_consolidation = state
        .consolidator_state
        .as_ref()
        .and_then(|cs| cs.last_completed_at())
        .map(|ts| ts.to_rfc3339());

    // Count fragments per tier from the memory store.
    let fragment_counts = match state.memory.as_ref() {
        Some(store) => {
            let working = store
                .get_by_tier(MemoryTier::Working)
                .await
                .map_or(0, |v| v.len());
            let episodic = store
                .get_by_tier(MemoryTier::Episodic)
                .await
                .map_or(0, |v| v.len());
            let semantic = store
                .get_by_tier(MemoryTier::Semantic)
                .await
                .map_or(0, |v| v.len());
            let procedural = store
                .get_by_tier(MemoryTier::Procedural)
                .await
                .map_or(0, |v| v.len());
            FragmentCounts {
                working,
                episodic,
                semantic,
                procedural,
            }
        }
        None => FragmentCounts {
            working: 0,
            episodic: 0,
            semantic: 0,
            procedural: 0,
        },
    };

    (
        StatusCode::OK,
        Json(MaturationStatusResponse {
            consolidation_state,
            last_consolidation,
            fragment_counts,
            decay_factor,
        }),
    )
}

/// POST /v1/memory/maturation/consolidate
///
/// Triggers an immediate consolidation check. Returns 202 Accepted on success,
/// 409 Conflict if consolidation is already running or maturation is disabled.
pub async fn consolidate(State(state): State<AppState>) -> impl IntoResponse {
    // Check if maturation is disabled.
    let enabled = state
        .tokens
        .as_ref()
        .map_or(true, |t| t.memory.maturation.enabled);

    if !enabled {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error": "maturation is disabled"})),
        );
    }

    // Check if consolidator state is available.
    let Some(ref consolidator) = state.consolidator_state else {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error": "consolidator not initialized"})),
        );
    };

    // Check if a consolidation pass is already running.
    if consolidator.is_running() {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error": "consolidation already in progress"})),
        );
    }

    // Send TriggerNow command to the consolidation timer task.
    if let Err(e) = consolidator.cmd_tx.send(ConsolidationCmd::TriggerNow).await {
        warn!(error = %e, "failed to send TriggerNow command to consolidator");
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error": "consolidator channel closed"})),
        );
    }

    let triggered_at = Utc::now().to_rfc3339();
    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({"triggered_at": triggered_at})),
    )
}

/// GET /v1/memory/handoff
///
/// Returns the current handoff summary as JSON, or 404 if no handoff exists.
pub async fn get_handoff_handler(State(state): State<AppState>) -> impl IntoResponse {
    let Some(ref db) = state.db else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "no handoff exists"})),
        );
    };

    match handoff::get_handoff(db) {
        Some(summary) => (StatusCode::OK, Json(serde_json::to_value(&summary).unwrap_or_default())),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "no handoff exists"})),
        ),
    }
}

/// DELETE /v1/memory/handoff
///
/// Clears the pending handoff. Returns 204 No Content (idempotent).
pub async fn delete_handoff_handler(State(state): State<AppState>) -> impl IntoResponse {
    if let Some(ref db) = state.db
        && let Err(e) = handoff::delete_handoff(db)
    {
        warn!(error = %e, "failed to delete handoff from redb");
    }
    StatusCode::NO_CONTENT
}

/// GET /v1/memory/:fragment_id/metadata
///
/// Returns fragment metadata including tier, access_count, last_accessed,
/// created_at, consolidated_into, and effective_score. Returns 404 if not found.
pub async fn fragment_metadata(
    State(state): State<AppState>,
    Path(fragment_id): Path<String>,
) -> impl IntoResponse {
    let frag_uuid = match Uuid::parse_str(&fragment_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid fragment_id format"})),
            );
        }
    };

    let Some(ref store) = state.memory else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "fragment not found"})),
        );
    };

    let fragment = match store.get_by_id(frag_uuid).await {
        Ok(Some(f)) => f,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "fragment not found"})),
            );
        }
        Err(e) => {
            warn!(error = %e, "failed to retrieve fragment metadata");
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "fragment not found"})),
            );
        }
    };

    // Compute effective_score using current decay config.
    let maturation_cfg = state.tokens.as_ref().map(|t| &t.memory.maturation);
    let decay_config = DecayConfig {
        decay_factor: maturation_cfg.map_or(0.95, |c| c.decay_factor),
        decay_floor: maturation_cfg.map_or(0.01, |c| c.decay_floor),
    };

    let now = Utc::now();
    let eff_score = decay::effective_score(
        f64::from(fragment.score),
        fragment.tier,
        fragment.last_accessed,
        fragment.created_at,
        now,
        &decay_config,
    );

    let response = FragmentMetadataResponse {
        fragment_id: fragment.fragment_id.to_string(),
        tier: fragment.tier.as_str().to_string(),
        access_count: fragment.access_count,
        last_accessed: fragment.last_accessed.map(|ts| ts.to_rfc3339()),
        created_at: fragment.created_at.to_rfc3339(),
        consolidated_into: fragment.consolidated_into.map(|id| id.to_string()),
        effective_score: eff_score,
    };

    (StatusCode::OK, Json(serde_json::to_value(&response).unwrap_or_default()))
}
