//! `POST /v1/chat` (non-streaming) + `POST /v1/chat/stream` (SSE) + `POST /v1/chat/yolo` (direct agent spawn).

use anyhow::Result;
use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use futures::StreamExt;
use std::convert::Infallible;
use std::sync::Arc;
use tracing::{instrument, warn};

use crate::llm::types::StreamChunk;
use crate::llm::LlmProvider;
use crate::proto::agent::{AgentKind, SpawnAgentRequest};
use crate::proto::chat::{ChatRequest, ChatResponse, StreamEvent, YoloResponse};
use crate::router::tier::{Candidate, Tier};
use crate::router::Router;
use crate::server::{error::AppError, AppState};

// ============================================================================
// Non-streaming /v1/chat — delegates to Router.route_chat (handles fallback)
// ============================================================================

#[instrument(skip(state, req), fields(tier = %req.tier))]
pub async fn chat(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, AppError> {
    let (output, _provider, model) = state.router.route_chat(&req, &req.messages).await?;

    Ok(Json(ChatResponse {
        content: output.content,
        model,
        tokens_in: output.tokens_in,
        tokens_out: output.tokens_out,
        finish_reason: output.finish_reason,
    }))
}

// ============================================================================
// Streaming /v1/chat/stream
// ============================================================================
//
// For streaming the fallback chain is trickier: once bytes have been sent to
// the client, we can't switch providers mid-stream. v0 strategy: try the first
// non-broken candidate; if its `create_stream` setup call fails, try the next.
// Once `create_stream` succeeds we commit to that provider; downstream stream
// errors surface to the client.

#[instrument(skip(state, req), fields(tier = %req.tier, model))]
pub async fn chat_stream(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, AppError> {
    state.router.budget.check()?;

    let candidates = candidates_for_request(&state.router, &req);
    let mut last_err: Option<anyhow::Error> = None;

    for cand in candidates {
        let breaker = state.router.breaker_for_public(&cand.key());
        if breaker.is_open() {
            warn!(candidate = %cand.key(), "skipping — breaker Open");
            continue;
        }
        let Some(provider) = provider_clone(&state.router, &cand.provider) else {
            continue;
        };

        tracing::Span::current().record("model", &cand.model);

        match provider.chat_stream(&cand.model, &req.messages).await {
            Ok(upstream) => {
                breaker.record_success();
                let model = cand.model.clone();
                let sse_stream = upstream.filter_map(move |chunk_result| {
                    let model = model.clone();
                    async move {
                        match chunk_result {
                            Ok(chunk) => Some(render_stream_chunk(chunk, &model)),
                            Err(e) => Some(error_event(&e.to_string())),
                        }
                    }
                }).flat_map(futures::stream::iter);
                return Ok(Sse::new(sse_stream).keep_alive(KeepAlive::new()));
            }
            Err(e) => {
                warn!(candidate = %cand.key(), error = ?e, "stream setup failed, trying next");
                breaker.record_failure();
                last_err = Some(e);
                continue;
            }
        }
    }

    Err(last_err
        .unwrap_or_else(|| anyhow::anyhow!("all candidates exhausted (tier {})", req.tier))
        .into())
}

// ============================================================================
// Helpers
// ============================================================================

fn candidates_for_request(router: &Router, req: &ChatRequest) -> Vec<Candidate> {
    if let Some(model) = req.model.as_deref() {
        return router
            .known_providers()
            .into_iter()
            .map(|p| Candidate::new(p, model.to_string()))
            .collect();
    }
    router.tier_candidates(Tier::from_str_lossy(&req.tier))
}

fn provider_clone(router: &Router, name: &str) -> Option<Arc<LlmProvider>> {
    router.provider_handle(name)
}

fn render_stream_chunk(chunk: StreamChunk, model: &str) -> Vec<Result<Event, Infallible>> {
    match chunk {
        StreamChunk::Token(delta) if delta.is_empty() => vec![],
        StreamChunk::Token(delta) => {
            let payload = StreamEvent::Token { delta };
            vec![Ok(Event::default()
                .event("token")
                .data(serde_json::to_string(&payload).unwrap_or_default()))]
        }
        StreamChunk::Finish { reason } => {
            let payload = StreamEvent::Done {
                model: model.to_string(),
                finish_reason: reason,
            };
            vec![Ok(Event::default()
                .event("done")
                .data(serde_json::to_string(&payload).unwrap_or_default()))]
        }
        StreamChunk::Error(message) => {
            let payload = StreamEvent::Error { message };
            vec![Ok(Event::default()
                .event("error")
                .data(serde_json::to_string(&payload).unwrap_or_default()))]
        }
    }
}

fn error_event(message: &str) -> Vec<Result<Event, Infallible>> {
    let payload = StreamEvent::Error {
        message: message.to_string(),
    };
    vec![Ok(Event::default()
        .event("error")
        .data(serde_json::to_string(&payload).unwrap_or_default()))]
}

// ============================================================================
// YOLO mode — /v1/chat/yolo
// ============================================================================

#[instrument(skip(state, req), fields(yolo = true))]
pub async fn chat_yolo(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<YoloResponse>, AppError> {
    let mut task = String::new();
    for msg in &req.messages {
        match msg.role.as_str() {
            "user" => {
                task.push_str(&format!("User: {}\n", msg.content));
            }
            "assistant" => {
                let preview: String = msg.content.chars().take(500).collect();
                task.push_str(&format!("Previous agent output: {}\n", preview));
            }
            _ => {}
        }
    }
    let user_message = task.trim().to_string();

    if user_message.is_empty() {
        return Err(AppError::bad_request("no user message for yolo"));
    }

    let tier = req.tier.clone();
    let agent_req = SpawnAgentRequest {
        kind: AgentKind::Writer,
        task: user_message,
        worktree_id: None,
        base: None,
        budget_usd: None,
        max_iters: None,
        yolo_mode: Some(true),
    };

    let agent = state.agents.spawn(agent_req).await?;

    Ok(Json(YoloResponse {
        agent_id: agent.id.clone(),
        kind: format!("{:?}", agent.kind).to_lowercase(),
        status: format!("{:?}", agent.status).to_lowercase(),
        message: format!(
            "yolo agent {} spawned (tier={tier}, gates=reduced)",
            &agent.id[..8]
        ),
    }))
}
