//! Protocol‑agnostic LLM provider — dispatches to the right wire format
//! internally so callers never touch `async_openai` types directly.

use anyhow::{Context, Result};
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionMessageToolCall, ChatCompletionRequestAssistantMessageArgs,
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestToolMessageArgs, ChatCompletionRequestUserMessageArgs,
        ChatCompletionTool, ChatCompletionToolType, CreateChatCompletionRequestArgs,
        CreateEmbeddingRequestArgs, EmbeddingInput, FunctionCall, FunctionObject,
    },
    Client,
};
use futures::StreamExt;
use serde_json::json;
use tracing::instrument;

use crate::config::ProviderEntry;

use super::anthropic;
use super::protocol::Protocol;
use super::types::*;

/// A single LLM provider, configured at startup from `tokens.toml`.
///
/// Internally holds a lazy `async_openai::Client` (for `OpenAiCompatible`)
/// or bare HTTP state (for `Anthropic`, v0.5+).  Callers use the same
/// methods regardless of the wire protocol.
#[derive(Clone)]
pub struct LlmProvider {
    pub name: String,
    pub protocol: Protocol,
    pub base_url: String,
    api_key: String,
    #[allow(dead_code)]
    http: reqwest::Client,
    /// Eagerly‑initialised `async_openai` client — only `Some` for
    /// `Protocol::OpenAiCompatible`.
    openai_client: Option<Client<OpenAIConfig>>,
}

impl LlmProvider {
    /// Create a new provider from raw parts.
    pub fn new(name: String, protocol: Protocol, base_url: String, api_key: String) -> Self {
        let openai_client = if protocol == Protocol::OpenAiCompatible {
            Some(Client::with_config(
                OpenAIConfig::new()
                    .with_api_base(&base_url)
                    .with_api_key(&api_key),
            ))
        } else {
            None
        };
        Self {
            name,
            protocol,
            base_url,
            api_key,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("reqwest client build"),
            openai_client,
        }
    }

    /// Build a provider from a config entry.  Returns `Ok(None)` when the
    /// provider is disabled or its API key is unavailable (soft skip).
    pub fn from_config(name: &str, entry: &ProviderEntry) -> Result<Option<Self>> {
        if !entry.enabled {
            return Ok(None);
        }

        let base_url = match &entry.base_url {
            Some(url) => url.clone(),
            None => anyhow::bail!("provider '{name}' enabled but no base_url set"),
        };

        let api_key = match &entry.api_key_env {
            Some(env) if !env.is_empty() => std::env::var(env).unwrap_or_default(),
            _ => String::new(),
        };

        let protocol = match entry.protocol.as_deref().unwrap_or("openai-compatible") {
            "openai-compatible" => Protocol::OpenAiCompatible,
            "anthropic" => Protocol::Anthropic,
            other => anyhow::bail!(
                "unknown protocol '{other}' for provider '{name}' — \
                 expected 'openai-compatible' or 'anthropic'"
            ),
        };

        Ok(Some(Self::new(name.to_string(), protocol, base_url, api_key)))
    }

    // ── Eager async-openai client access ─────────────────────────────

    fn openai(&self) -> Result<&Client<OpenAIConfig>> {
        self.openai_client
            .as_ref()
            .context("OpenAI client not available for this protocol")
    }

    // ── Public API ───────────────────────────────────────────────────

    /// Non‑streaming chat.  Returns the full model output.
    #[instrument(skip(self, messages), fields(provider = %self.name, model))]
    pub async fn chat(
        &self,
        model: &str,
        messages: &[crate::proto::chat::ChatMessage],
    ) -> Result<ChatOutput> {
        match self.protocol {
            Protocol::OpenAiCompatible => {
                let msgs = chat_messages_to_openai(messages)?;
                let req = CreateChatCompletionRequestArgs::default()
                    .model(model)
                    .messages(msgs)
                    .build()
                    .context("build openai request")?;
                let resp = self
                    .openai()?
                    .chat()
                    .create(req)
                    .await
                    .with_context(|| format!("{name} chat failed (model={model})", name = self.name))?;
                Ok(chat_response_to_output(resp))
            }
            Protocol::Anthropic => {
                anthropic::chat(&self.http, &self.api_key, &self.base_url, model, messages).await
            }
        }
    }

    /// Chat with tool‑calling enabled.
    ///
    /// Accepts pre‑built `ChatMessageUnified` messages (so agent loops can
    /// include `Tool` result messages between turns) and `ToolDef` schemas.
    /// Optional `ChatParams` overrides sampling (temperature, response_format).
    #[instrument(skip(self, messages, tools, params), fields(provider = %self.name, model))]
    pub async fn chat_with_tools(
        &self,
        model: &str,
        messages: Vec<ChatMessageUnified>,
        tools: Vec<ToolDef>,
        params: Option<ChatParams>,
    ) -> Result<ChatOutput> {
        match self.protocol {
            Protocol::OpenAiCompatible => {
                self.chat_with_tools_raw(model, &messages, &tools, params.as_ref()).await
            }
            Protocol::Anthropic => {
                anthropic::chat_with_tools(
                    &self.http,
                    &self.api_key,
                    &self.base_url,
                    model,
                    messages,
                    tools,
                )
                .await
            }
        }
    }

    /// Raw JSON path: used when a `ChatMessageUnified::Assistant` carries
    /// `reasoning_content` (DeepSeek V4 thinking mode).  Builds the request
    /// as JSON, injects `reasoning_content`, sends via `reqwest`, and parses
    /// the response — including `reasoning_content` — back into `ChatOutput`.
    async fn chat_with_tools_raw(
        &self,
        model: &str,
        messages: &[ChatMessageUnified],
        tools: &[ToolDef],
        params: Option<&ChatParams>,
    ) -> Result<ChatOutput> {
        let body = build_request_json_with_reasoning(model, messages, tools, params)?;
        let url = format!(
            "{}/chat/completions",
            self.base_url.trim_end_matches('/')
        );
        let resp: serde_json::Value = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .with_context(|| {
                format!(
                    "{name} chat_with_tools_raw request failed (model={model})",
                    name = self.name
                )
            })?
            .json()
            .await
            .with_context(|| {
                format!(
                    "{name} chat_with_tools_raw parse failed (model={model})",
                    name = self.name
                )
            })?;
        Ok(chat_output_from_json(resp))
    }

    /// Streaming chat.  Returns a `ChatStream` — regardless of the
    /// underlying wire format, the consumer sees uniform `StreamChunk`s.
    #[instrument(skip(self, messages), fields(provider = %self.name, model))]
    pub async fn chat_stream(
        &self,
        model: &str,
        messages: &[crate::proto::chat::ChatMessage],
    ) -> Result<ChatStream> {
        match self.protocol {
            Protocol::OpenAiCompatible => {
                let msgs = chat_messages_to_openai(messages)?;
                let req = CreateChatCompletionRequestArgs::default()
                    .model(model)
                    .messages(msgs)
                    .stream(true)
                    .build()
                    .context("build openai stream request")?;
                let stream = self
                    .openai()?
                    .chat()
                    .create_stream(req)
                    .await
                    .with_context(|| {
                        format!(
                            "{name} chat_stream failed (model={model})",
                            name = self.name
                        )
                    })?;
                Ok(Box::pin(stream.map(openai_chunk_to_stream_chunk)))
            }
            Protocol::Anthropic => {
                anthropic::chat_stream(
                    &self.http,
                    &self.api_key,
                    &self.base_url,
                    model,
                    messages,
                )
                .await
            }
        }
    }

    /// Compute embeddings.  Only supported on `OpenAiCompatible` providers.
    #[instrument(skip(self, inputs), fields(provider = %self.name, model))]
    pub async fn embed(&self, model: &str, inputs: Vec<String>) -> Result<Vec<Vec<f32>>> {
        match self.protocol {
            Protocol::OpenAiCompatible => {
                let req = CreateEmbeddingRequestArgs::default()
                    .model(model)
                    .input(EmbeddingInput::StringArray(inputs))
                    .build()
                    .context("build embedding request")?;
                let resp = self
                    .openai()?
                    .embeddings()
                    .create(req)
                    .await
                    .with_context(|| {
                        format!(
                            "{name} embed failed (model={model})",
                            name = self.name
                        )
                    })?;
                Ok(resp.data.into_iter().map(|d| d.embedding).collect())
            }
            Protocol::Anthropic => {
                anyhow::bail!("Anthropic does not support embeddings");
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Internal conversions — wire‑format ↔ unified types
// ═══════════════════════════════════════════════════════════════════════

/// Convert wire `ChatMessage` (simple user‑only, for REPL use) to
/// `async_openai` request messages.
fn chat_messages_to_openai(messages: &[crate::proto::chat::ChatMessage]) -> Result<Vec<ChatCompletionRequestMessage>> {
    messages
        .iter()
        .map(|m| {
            Ok(ChatCompletionRequestUserMessageArgs::default()
                .content(m.content.clone())
                .build()
                .context("build user message")?
                .into())
        })
        .collect()
}

/// Convert an `async_openai` response into our unified `ChatOutput`.
/// The `reasoning_content` field is always `None` here — async‑openai
/// 0.27 does not surface it.  Use `chat_output_from_json` for raw parsing.
fn chat_response_to_output(resp: async_openai::types::CreateChatCompletionResponse) -> ChatOutput {
    let choice = resp.choices.into_iter().next().unwrap_or_else(|| {
        panic!("{} chat response has no choices", resp.model)
    });
    let usage = resp.usage.unwrap_or_else(|| async_openai::types::CompletionUsage {
        prompt_tokens: 0,
        completion_tokens: 0,
        total_tokens: 0,
        completion_tokens_details: None,
        prompt_tokens_details: None,
    });

    let tool_calls = choice
        .message
        .tool_calls
        .unwrap_or_default()
        .into_iter()
        .map(|tc| ToolCall {
            id: tc.id,
            name: tc.function.name,
            arguments: tc.function.arguments,
        })
        .collect();

    ChatOutput {
        content: choice.message.content.unwrap_or_default(),
        tokens_in: usage.prompt_tokens,
        tokens_out: usage.completion_tokens,
        finish_reason: choice.finish_reason.map(|r| format!("{r:?}")),
        tool_calls,
        reasoning_content: None,
    }
}

/// Parse the raw JSON from an OpenAI‑compatible chat‑completions response.
/// Extracts every field, including `reasoning_content` (DeepSeek thinking mode).
fn chat_output_from_json(resp: serde_json::Value) -> ChatOutput {
    let choice = &resp["choices"][0];
    let msg = &choice["message"];
    let usage = &resp["usage"];

    let tool_calls: Vec<ToolCall> = msg["tool_calls"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|tc| ToolCall {
                    id: tc["id"].as_str().unwrap_or("").to_string(),
                    name: tc["function"]["name"].as_str().unwrap_or("").to_string(),
                    arguments: tc["function"]["arguments"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                })
                .collect()
        })
        .unwrap_or_default();

    ChatOutput {
        content: msg["content"].as_str().unwrap_or("").to_string(),
        tokens_in: usage["prompt_tokens"].as_u64().unwrap_or(0) as u32,
        tokens_out: usage["completion_tokens"].as_u64().unwrap_or(0) as u32,
        finish_reason: choice["finish_reason"]
            .as_str()
            .map(|s| s.to_string()),
        tool_calls,
        reasoning_content: msg["reasoning_content"].as_str().map(|s| s.to_string()),
    }
}

/// Map a single OpenAI streaming chunk to our `StreamChunk` enum.
fn openai_chunk_to_stream_chunk(
    chunk_result: Result<async_openai::types::CreateChatCompletionStreamResponse, async_openai::error::OpenAIError>,
) -> Result<StreamChunk> {
    match chunk_result {
        Ok(chunk) => {
            let choice = chunk.choices.first();
            let delta = choice
                .and_then(|c| c.delta.content.as_ref())
                .cloned()
                .unwrap_or_default();
            if !delta.is_empty() {
                return Ok(StreamChunk::Token(delta));
            }
            if let Some(reason) = choice.and_then(|c| c.finish_reason) {
                return Ok(StreamChunk::Finish {
                    reason: Some(format!("{reason:?}")),
                });
            }
            // Empty chunk (e.g. role announcement) — skip.
            Ok(StreamChunk::Token(String::new()))
        }
        Err(e) => Ok(StreamChunk::Error(e.to_string())),
    }
}

/// Convert a `ChatMessageUnified` into an `async_openai` request message.
fn chat_message_unified_to_openai(msg: &ChatMessageUnified) -> Result<ChatCompletionRequestMessage> {
    match msg {
        ChatMessageUnified::System { content } => {
            Ok(ChatCompletionRequestSystemMessageArgs::default()
                .content(content.as_str())
                .build()
                .context("build system message")?
                .into())
        }
        ChatMessageUnified::User { content } => {
            Ok(ChatCompletionRequestUserMessageArgs::default()
                .content(content.as_str())
                .build()
                .context("build user message")?
                .into())
        }
        ChatMessageUnified::Assistant {
            content,
            tool_calls,
            ..
        } => {
            let content_str = content.as_deref().unwrap_or("");
            let calls: Vec<ChatCompletionMessageToolCall> = if tool_calls.is_empty() {
                vec![]
            } else {
                tool_calls
                    .iter()
                    .map(|tc| ChatCompletionMessageToolCall {
                        id: tc.id.clone(),
                        r#type: ChatCompletionToolType::Function,
                        function: FunctionCall {
                            name: tc.name.clone(),
                            arguments: tc.arguments.clone(),
                        },
                    })
                    .collect()
            };
            let m = ChatCompletionRequestAssistantMessageArgs::default()
                .content(content_str)
                .tool_calls(calls)
                .build()
                .context("build assistant message")?;
            Ok(m.into())
        }
        ChatMessageUnified::Tool { call_id, content } => {
            Ok(ChatCompletionRequestToolMessageArgs::default()
                .tool_call_id(call_id)
                .content(content.as_str())
                .build()
                .context("build tool result message")?
                .into())
        }
    }
}

fn chat_message_unified_vec_to_openai(
    messages: &[ChatMessageUnified],
) -> Result<Vec<ChatCompletionRequestMessage>> {
    messages.iter().map(chat_message_unified_to_openai).collect()
}

/// Convert a `ToolDef` into an `async_openai` tool schema.
fn tool_def_to_openai(tool: &ToolDef) -> ChatCompletionTool {
    ChatCompletionTool {
        r#type: ChatCompletionToolType::Function,
        function: FunctionObject {
            name: tool.name.clone(),
            description: Some(tool.description.clone()),
            parameters: Some(tool.parameters.clone()),
            strict: if tool.strict { Some(true) } else { None },
        },
    }
}

/// Build an OpenAI‑compatible request body as JSON, injecting
/// `reasoning_content` into any `Assistant` message that carries it.
fn build_request_json_with_reasoning(
    model: &str,
    messages: &[ChatMessageUnified],
    tools: &[ToolDef],
    params: Option<&ChatParams>,
) -> Result<serde_json::Value> {
    let msgs: Vec<serde_json::Value> = messages
        .iter()
        .map(chat_message_unified_to_json)
        .collect();

    let mut body = json!({
        "model": model,
        "messages": msgs,
    });

    if !tools.is_empty() {
        body["tools"] = json!(tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    }
                })
            })
            .collect::<Vec<_>>());
    }

    if let Some(p) = params {
        if let Some(t) = p.temperature {
            body["temperature"] = json!(t);
        }
        if let Some(ref fmt) = p.response_format {
            body["response_format"] = json!({"type": fmt});
        }
    }

    Ok(body)
}

/// Convert a `ChatMessageUnified` into a JSON object ready for the wire.
fn chat_message_unified_to_json(msg: &ChatMessageUnified) -> serde_json::Value {
    match msg {
        ChatMessageUnified::System { content } => {
            json!({"role": "system", "content": content})
        }
        ChatMessageUnified::User { content } => {
            json!({"role": "user", "content": content})
        }
        ChatMessageUnified::Assistant {
            content,
            tool_calls,
            reasoning_content,
        } => {
            let mut m = json!({"role": "assistant"});
            if let Some(c) = content {
                m["content"] = json!(c);
            }
            if !tool_calls.is_empty() {
                m["tool_calls"] = json!(tool_calls
                    .iter()
                    .map(|tc| {
                        json!({
                            "id": tc.id,
                            "type": "function",
                            "function": {
                                "name": tc.name,
                                "arguments": tc.arguments,
                            }
                        })
                    })
                    .collect::<Vec<_>>());
            }
            if let Some(rc) = reasoning_content {
                m["reasoning_content"] = json!(rc);
            }
            m
        }
        ChatMessageUnified::Tool { call_id, content } => {
            json!({
                "role": "tool",
                "tool_call_id": call_id,
                "content": content,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_def_roundtrip() {
        let def = ToolDef {
            name: "test".into(),
            description: "A test tool".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": { "x": { "type": "string" } },
                "required": ["x"],
            }),
            strict: true,
        };
        let openai = tool_def_to_openai(&def);
        assert_eq!(openai.function.name, "test");
    }

    #[test]
    fn empty_tool_calls_yields_no_assistant_tool_call_field() {
        let msg = ChatMessageUnified::Assistant {
            content: Some("hello".into()),
            tool_calls: vec![],
            reasoning_content: None,
        };
        let result = chat_message_unified_to_openai(&msg);
        assert!(result.is_ok());
    }

    #[test]
    fn json_building_injects_reasoning() {
        let msg = ChatMessageUnified::Assistant {
            content: Some("thinking...".into()),
            tool_calls: vec![],
            reasoning_content: Some("deep reasoning here".into()),
        };
        let json = chat_message_unified_to_json(&msg);
        assert_eq!(json["reasoning_content"], "deep reasoning here");
    }
}
