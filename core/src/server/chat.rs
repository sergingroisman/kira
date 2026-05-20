//! `POST /v1/chat` (non-streaming) + `POST /v1/chat/stream` (SSE).

use anyhow::{Context, Result};
use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use futures::StreamExt;
use std::convert::Infallible;
use std::sync::Arc;
use tracing::{instrument, warn};

use crate::llm::LlmProvider;
use crate::proto::chat::{ChatMessage, ChatRequest, ChatResponse, StreamEvent};
use crate::router::tier::{candidates_for, Candidate, Tier};
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
    let (response, _provider, model) = state.router.route_chat(&req, &req.messages).await?;
    let choice = response.choices.into_iter().next().context("no choices")?;
    let usage = response.usage.context("no usage returned")?;

    Ok(Json(ChatResponse {
        content: choice.message.content.unwrap_or_default(),
        model,
        tokens_in: usage.prompt_tokens,
        tokens_out: usage.completion_tokens,
        finish_reason: choice.finish_reason.map(|r| format!("{r:?}")),
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
                breaker.record_success(); // setup succeeded
                let model = cand.model.clone();
                let sse_stream = upstream.flat_map(move |chunk_result| {
                    let model = model.clone();
                    let events: Vec<Result<Event, Infallible>> = match chunk_result {
                        Ok(chunk) => render_chunk(&chunk, &model),
                        Err(e) => vec![Ok(error_event(&e.to_string()))],
                    };
                    futures::stream::iter(events)
                });
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

fn candidates_for_request(_router: &Router, req: &ChatRequest) -> Vec<Candidate> {
    // For now mirror Router::candidates logic — simple tier-based selection.
    // Future: expose Router::candidates() (or merge stream path into route_chat).
    if let Some(model) = req.model.as_deref() {
        // Try first known provider with this model (simplification).
        return vec![Candidate::new("deepseek", model.to_string())];
    }
    candidates_for(Tier::from_str_lossy(&req.tier))
}

fn provider_clone(router: &Router, name: &str) -> Option<Arc<LlmProvider>> {
    router.provider_handle(name)
}

fn render_chunk(
    chunk: &async_openai::types::CreateChatCompletionStreamResponse,
    model: &str,
) -> Vec<Result<Event, Infallible>> {
    let choice = chunk.choices.first();
    let delta = choice
        .and_then(|c| c.delta.content.as_ref())
        .cloned()
        .unwrap_or_default();
    let finish = choice.and_then(|c| c.finish_reason.map(|r| format!("{r:?}")));

    let mut events: Vec<Result<Event, Infallible>> = Vec::new();
    if !delta.is_empty() {
        let payload = StreamEvent::Token { delta };
        events.push(Ok(Event::default()
            .event("token")
            .data(serde_json::to_string(&payload).unwrap_or_default())));
    }
    if let Some(reason) = finish {
        let payload = StreamEvent::Done {
            model: model.to_string(),
            finish_reason: Some(reason),
        };
        events.push(Ok(Event::default()
            .event("done")
            .data(serde_json::to_string(&payload).unwrap_or_default())));
    }
    events
}

fn error_event(message: &str) -> Event {
    let payload = StreamEvent::Error {
        message: message.to_string(),
    };
    Event::default()
        .event("error")
        .data(serde_json::to_string(&payload).unwrap_or_default())
}

// The `dead_code` unused helpers will be cleaned in next iteration when we
// fully merge stream + non-stream into a single Router method.
#[allow(dead_code)]
fn _unused_for_now(_msgs: &[ChatMessage]) {}
