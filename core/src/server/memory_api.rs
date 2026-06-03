//! Memory API — REST endpoints for cross-thread semantic memory.
//!
//! - `GET /v1/memory/:run_id/context` — fragments associated with a run
//! - `GET /v1/memory/search?q=<query>` — semantic search
//! - `DELETE /v1/memory/:fragment_id` — delete a fragment

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::AppState;
use crate::memory::MemoryStore;

/// Response for a single memory fragment.
#[derive(Debug, Serialize)]
pub struct FragmentResponse {
    pub fragment_id: String,
    pub content: String,
    pub thread_id: String,
    pub run_id: String,
    pub created_at: String,
    pub score: f32,
    pub tier: String,
}

/// Query params for search endpoint.
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}

/// GET /v1/memory/:run_id/context
///
/// Returns fragments associated with the given run_id, ordered by score desc.
pub async fn get_context(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> impl IntoResponse {
    let run_uuid = match Uuid::parse_str(&run_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid run_id format"})),
            );
        }
    };

    let Some(ref store) = state.memory else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "memory_backend_unavailable"})),
        );
    };

    match store.get_by_run(run_uuid).await {
        Ok(fragments) => {
            let items: Vec<serde_json::Value> = fragments
                .iter()
                .map(|f| {
                    serde_json::json!({
                        "fragment_id": f.fragment_id.to_string(),
                        "content": f.content,
                        "thread_id": f.thread_id.to_string(),
                        "run_id": f.run_id.to_string(),
                        "created_at": f.created_at.to_rfc3339(),
                        "score": f.score,
                        "tier": f.tier.as_str(),
                    })
                })
                .collect();
            (StatusCode::OK, Json(serde_json::json!({"fragments": items})))
        }
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "memory_backend_unavailable"})),
        ),
    }
}

/// GET /v1/memory/search?q=<query>
///
/// Returns top-K fragments by semantic similarity to the query.
pub async fn search(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> impl IntoResponse {
    let query = match params.q {
        Some(q) if !q.is_empty() => q,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "query_required"})),
            );
        }
    };

    let Some(ref store) = state.memory else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "memory_backend_unavailable"})),
        );
    };

    // TODO: compute embedding of query via EmbedClient, then search
    // For now, return empty (embedding call not yet wired here)
    let _ = (query, store);
    (StatusCode::OK, Json(serde_json::json!({"fragments": []})))
}

/// DELETE /v1/memory/:fragment_id
///
/// Deletes a fragment by its ID.
pub async fn delete_fragment(
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
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "memory_backend_unavailable"})),
        );
    };

    match store.delete(frag_uuid).await {
        Ok(true) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "deleted"})),
        ),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "fragment_not_found"})),
        ),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "memory_backend_unavailable"})),
        ),
    }
}
