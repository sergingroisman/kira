//! Unified, protocol-agnostic types for LLM interactions.
//!
//! These replace direct dependency on `async_openai` types in all external
//! interfaces (Router, `AgentManager`, handlers). Each `LlmProvider` impl
//! converts to/from its native format internally.

use std::pin::Pin;

use futures::Stream;
use serde::{Deserialize, Serialize};

/// Sampling/format overrides for a single chat call. Default = vendor defaults.
///
/// Judge calls MUST set `temperature=0.0` + `response_format=json_object`
/// for determinism + parseable output (Judge L0 — Structured + temp=0).
#[derive(Debug, Clone, Default)]
pub struct ChatParams {
    pub temperature: Option<f32>,
    /// `"json_object"` forces strict JSON output (DashScope/DeepSeek both support).
    pub response_format: Option<String>,
}

impl ChatParams {
    pub fn judge_deterministic() -> Self {
        Self {
            temperature: Some(0.0),
            response_format: Some("json_object".into()),
        }
    }
}

/// Unified output from any non-streaming chat call.
#[derive(Debug, Clone)]
pub struct ChatOutput {
    pub content: String,
    pub tokens_in: u32,
    pub tokens_out: u32,
    pub finish_reason: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    /// `DeepSeek` V4 "thinking mode" — must be echoed back verbatim on the
    /// next assistant message turn when tool‑calling. `None` for all
    /// other providers.
    pub reasoning_content: Option<String>,
}

/// A tool-call request emitted by the model — the agent loop feeds these
/// back via `ChatMessageUnified::Tool` after dispatching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    /// JSON-encoded arguments.
    pub arguments: String,
}

/// Tool definition exposed to the model via function-calling.
///
/// Replaces `async_openai::types::ChatCompletionTool` in all public APIs.
/// The internal conversion is done by each `LlmProvider` impl.
#[derive(Debug, Clone)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    /// JSON Schema object for the function parameters.
    pub parameters: serde_json::Value,
    pub strict: bool,
}

/// Unified message types for building chat history in the agent loop.
///
/// Replaces `async_openai::types::ChatCompletionRequestMessage*`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatMessageUnified {
    System {
        content: String,
    },
    User {
        content: String,
    },
    Assistant {
        content: Option<String>,
        tool_calls: Vec<ToolCall>,
        /// `DeepSeek` `reasoning_content` — must be echoed back in follow‑up calls.
        reasoning_content: Option<String>,
    },
    Tool {
        call_id: String,
        content: String,
    },
}

impl ChatMessageUnified {
    /// Concatenate all textual content in this message for token counting.
    pub fn content(&self) -> String {
        match self {
            Self::System { content } => content.clone(),
            Self::User { content } => content.clone(),
            Self::Assistant {
                content,
                tool_calls,
                reasoning_content,
            } => {
                let mut out = String::new();
                if let Some(rc) = reasoning_content {
                    out.push_str(rc);
                }
                if let Some(c) = content {
                    out.push_str(c);
                }
                for tc in tool_calls {
                    out.push_str(&tc.name);
                    out.push_str(&tc.arguments);
                }
                out
            }
            Self::Tool { content, .. } => content.clone(),
        }
    }
}

/// Streaming event from any protocol (OpenAI-compatible SSE, Anthropic SSE, …).
#[derive(Debug, Clone)]
pub enum StreamChunk {
    Token(String),
    Finish {
        reason: Option<String>,
    },
    Error(String),
}

/// Boxed, Send‑safe stream of `StreamChunk`s — the return type of
/// `LlmProvider::chat_stream()`.  Consumers (SSE handlers, test code)
/// treat this uniformly regardless of the underlying protocol.
pub type ChatStream = Pin<Box<dyn Stream<Item = anyhow::Result<StreamChunk>> + Send>>;
