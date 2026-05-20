//! `POST /v1/chat` (non-streaming) + `POST /v1/chat/stream` (SSE).

use anyhow::{Context, Result};
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestUserMessageArgs,
    CreateChatCompletionRequestArgs,
};
use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use futures::StreamExt;
use std::convert::Infallible;
use tracing::instrument;

use crate::proto::chat::{
    tier_to_model, ChatMessage, ChatRequest, ChatResponse, StreamEvent,
};
use crate::server::{error::AppError, AppState};

/// Convert wire-format messages to async-openai types.
/// v0: every message becomes a UserMessage. System/Assistant variants come later.
fn build_messages(messages: &[ChatMessage]) -> Result<Vec<ChatCompletionRequestMessage>> {
    messages
        .iter()
        .map(|m| -> Result<ChatCompletionRequestMessage> {
            let msg = ChatCompletionRequestUserMessageArgs::default()
                .content(m.content.clone())
                .build()?;
            Ok(msg.into())
        })
        .collect()
}

fn resolve_model(req: &ChatRequest) -> String {
    req.model
        .clone()
        .unwrap_or_else(|| tier_to_model(&req.tier).to_string())
}

#[instrument(skip(state, req), fields(tier = %req.tier, model))]
pub async fn chat(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, AppError> {
    let model = resolve_model(&req);
    tracing::Span::current().record("model", &model);

    let messages = build_messages(&req.messages)?;
    let request = CreateChatCompletionRequestArgs::default()
        .model(&model)
        .messages(messages)
        .build()?;

    let response = state
        .dashscope
        .chat()
        .create(request)
        .await
        .context("DashScope chat completion failed")?;

    let choice = response.choices.into_iter().next().context("no choices")?;
    let usage = response.usage.context("no usage returned")?;

    Ok(Json(ChatResponse {
        content: choice.message.content.unwrap_or_default(),
        model: response.model,
        tokens_in: usage.prompt_tokens,
        tokens_out: usage.completion_tokens,
        finish_reason: choice.finish_reason.map(|r| format!("{r:?}")),
    }))
}

#[instrument(skip(state, req), fields(tier = %req.tier, model))]
pub async fn chat_stream(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, AppError> {
    let model = resolve_model(&req);
    tracing::Span::current().record("model", &model);

    let messages = build_messages(&req.messages)?;
    let request = CreateChatCompletionRequestArgs::default()
        .model(&model)
        .messages(messages)
        .stream(true)
        .build()?;

    let upstream = state
        .dashscope
        .chat()
        .create_stream(request)
        .await
        .context("DashScope create_stream failed")?;

    let model_clone = model.clone();
    let sse_stream = upstream.flat_map(move |chunk_result| {
        let model = model_clone.clone();
        let events: Vec<Result<Event, Infallible>> = match chunk_result {
            Ok(chunk) => {
                let choice = chunk.choices.first();
                let delta = choice
                    .and_then(|c| c.delta.content.as_ref())
                    .cloned()
                    .unwrap_or_default();
                let finish = choice.and_then(|c| c.finish_reason.map(|r| format!("{r:?}")));

                let mut events = Vec::new();
                if !delta.is_empty() {
                    let payload = StreamEvent::Token { delta };
                    events.push(Ok(Event::default()
                        .event("token")
                        .data(serde_json::to_string(&payload).unwrap_or_default())));
                }
                if let Some(reason) = finish {
                    let payload = StreamEvent::Done {
                        model: model.clone(),
                        finish_reason: Some(reason),
                    };
                    events.push(Ok(Event::default()
                        .event("done")
                        .data(serde_json::to_string(&payload).unwrap_or_default())));
                }
                events
            }
            Err(e) => {
                let payload = StreamEvent::Error {
                    message: e.to_string(),
                };
                vec![Ok(Event::default()
                    .event("error")
                    .data(serde_json::to_string(&payload).unwrap_or_default()))]
            }
        };
        futures::stream::iter(events)
    });

    Ok(Sse::new(sse_stream).keep_alive(KeepAlive::new()))
}
