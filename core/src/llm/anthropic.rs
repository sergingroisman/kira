//! Native Anthropic Messages API client (not OpenAI‑compatible).
//!
//! Wire format: `POST /v1/messages` with `x-api-key` header, content‑block
//! responses, and SSE events for streaming.  Covers Claude Sonnet 4, Opus, etc.

use anyhow::{Context, Result};
use futures::channel::mpsc;
use futures::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use tracing::instrument;

use super::types::{ChatOutput, ChatMessageUnified, ToolDef, ChatStream, StreamChunk, ToolCall};

// ═══════════════════════════════════════════════════════════════════════
// Public API — called by `LlmProvider` protocol dispatch
// ═══════════════════════════════════════════════════════════════════════

/// Non‑streaming chat via `/v1/messages`.
#[instrument(skip(http, messages))]
pub async fn chat(
    http: &Client,
    api_key: &str,
    base_url: &str,
    max_tokens: u32,
    api_version: &str,
    model: &str,
    messages: &[crate::proto::chat::ChatMessage],
) -> Result<ChatOutput> {
    let body = json!({
        "model": model,
        "max_tokens": max_tokens,
        "messages": simple_messages_to_anthropic(messages),
    });

    let resp: Value = http
        .post(format!("{}/messages", base_url.trim_end_matches('/')))
        .header("x-api-key", api_key)
        .header("anthropic-version", api_version)
        .json(&body)
        .send()
        .await
        .context("anthropic chat request failed")?
        .error_for_status()
        .context("anthropic chat returned error status")?
        .json()
        .await
        .context("anthropic chat response parse failed")?;

    parse_message_response(resp)
}

/// Non‑streaming chat with tool definitions.
#[instrument(skip(http, messages, tools))]
pub async fn chat_with_tools(
    http: &Client,
    api_key: &str,
    base_url: &str,
    max_tokens: u32,
    api_version: &str,
    model: &str,
    messages: Vec<ChatMessageUnified>,
    tools: Vec<ToolDef>,
) -> Result<ChatOutput> {
    let mut body = json!({
        "model": model,
        "max_tokens": max_tokens,
    });

    let (system, msgs) = split_system_messages(&messages);
    if let Some(sys) = system {
        body["system"] = json!(sys);
    }
    body["messages"] = json!(chat_messages_to_anthropic(&msgs));

    if !tools.is_empty() {
        let atools: Vec<Value> = tools.iter().map(tool_def_to_anthropic).collect();
        body["tools"] = json!(atools);
    }

    let resp: Value = http
        .post(format!("{}/messages", base_url.trim_end_matches('/')))
        .header("x-api-key", api_key)
        .header("anthropic-version", api_version)
        .json(&body)
        .send()
        .await
        .context("anthropic chat_with_tools request failed")?
        .error_for_status()
        .context("anthropic chat_with_tools returned error status")?
        .json()
        .await
        .context("anthropic chat_with_tools response parse failed")?;

    parse_message_response(resp)
}

/// Streaming chat via SSE from `/v1/messages` with `stream: true`.
#[instrument(skip(http, messages))]
pub async fn chat_stream(
    http: &Client,
    api_key: &str,
    base_url: &str,
    max_tokens: u32,
    api_version: &str,
    model: &str,
    messages: &[crate::proto::chat::ChatMessage],
) -> Result<ChatStream> {
    let body = json!({
        "model": model,
        "max_tokens": max_tokens,
        "messages": simple_messages_to_anthropic(messages),
        "stream": true,
    });

    let response = http
        .post(format!("{}/messages", base_url.trim_end_matches('/')))
        .header("x-api-key", api_key)
        .header("anthropic-version", api_version)
        .json(&body)
        .send()
        .await
        .context("anthropic stream request failed")?;

    let (tx, rx) = mpsc::unbounded::<Result<StreamChunk>>();

    tokio::spawn(async move {
        let mut byte_stream = response.bytes_stream();
        let mut buf = String::new();
        let mut current_event = String::new();

        while let Some(chunk) = byte_stream.next().await {
            let bytes = match chunk {
                Ok(b) => b,
                Err(e) => {
                    let _ = tx.unbounded_send(Ok(StreamChunk::Error(e.to_string())));
                    break;
                }
            };

            buf.push_str(&String::from_utf8_lossy(&bytes));

            loop {
                let nl = match buf.find('\n') {
                    Some(p) => p,
                    None => break,
                };

                let line = buf[..nl].trim().to_string();
                buf = buf[nl + 1..].to_string();

                // Empty line = end of SSE event.
                if line.is_empty() {
                    current_event.clear();
                    continue;
                }

                if let Some(event) = line.strip_prefix("event: ") {
                    current_event = event.to_string();
                    continue;
                }

                let Some(data) = line.strip_prefix("data: ") else {
                    continue;
                };

                let val: Value = match serde_json::from_str(data) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                match current_event.as_str() {
                    "content_block_delta"
                        if val["delta"]["type"] == "text_delta" => {
                            let text = val["delta"]["text"].as_str().unwrap_or("");
                            let _ = tx.unbounded_send(Ok(StreamChunk::Token(text.to_string())));
                        }
                    "message_delta" => {
                        let reason = val["delta"]["stop_reason"].as_str().map(std::string::ToString::to_string);
                        let _ = tx.unbounded_send(Ok(StreamChunk::Finish { reason }));
                    }
                    "message_stop" => break,
                    _ => {}
                }
            }
        }
    });

    Ok(Box::pin(rx))
}

// ═══════════════════════════════════════════════════════════════════════
// Internal helpers
// ═══════════════════════════════════════════════════════════════════════

/// Convert simple REPL messages (role + content strings) to Anthropic format.
fn simple_messages_to_anthropic(messages: &[crate::proto::chat::ChatMessage]) -> Vec<Value> {
    messages
        .iter()
        .map(|m| {
            json!({
                "role": m.role,
                "content": m.content,
            })
        })
        .collect()
}

/// Split system messages out of a `ChatMessageUnified` vec.
///
/// Anthropic expects system in a top-level `system` field, not in `messages[]`.
/// Returns `(system_prompt, remaining_messages)`.
fn split_system_messages(messages: &[ChatMessageUnified]) -> (Option<String>, Vec<&ChatMessageUnified>) {
    let mut system = None;
    let mut rest = Vec::new();

    for msg in messages {
        match msg {
            ChatMessageUnified::System { content } => {
                let prev: String = system.take().unwrap_or_default();
                system = Some(if prev.is_empty() {
                    content.clone()
                } else {
                    format!("{prev}\n{content}")
                });
            }
            other => rest.push(other),
        }
    }

    (system, rest)
}

/// Convert `ChatMessageUnified` to Anthropic message array entries.
fn chat_messages_to_anthropic(messages: &[&ChatMessageUnified]) -> Vec<Value> {
    let mut out = Vec::new();

    for msg in messages {
        match msg {
            ChatMessageUnified::System { .. } => {
                // Already handled in split_system_messages.
            }
            ChatMessageUnified::User { content } => {
                out.push(json!({
                    "role": "user",
                    "content": content,
                }));
            }
            ChatMessageUnified::Assistant {
                content,
                tool_calls,
                ..
            } => {
                let mut blocks = Vec::new();
                if let Some(c) = content
                    && !c.is_empty() {
                        blocks.push(json!({"type": "text", "text": c}));
                    }
                for tc in tool_calls {
                    let input: Value =
                        serde_json::from_str(&tc.arguments).unwrap_or(json!({}));
                    blocks.push(json!({
                        "type": "tool_use",
                        "id": tc.id,
                        "name": tc.name,
                        "input": input,
                    }));
                }
                out.push(json!({
                    "role": "assistant",
                    "content": blocks,
                }));
            }
            ChatMessageUnified::Tool { call_id, content } => {
                // Anthropic tool results go into a "user" role message with
                // content of type "tool_result".
                out.push(json!({
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": call_id,
                        "content": content,
                    }],
                }));
            }
        }
    }

    out
}

/// Convert a `ToolDef` to an Anthropic tool schema.
fn tool_def_to_anthropic(tool: &ToolDef) -> Value {
    json!({
        "name": tool.name,
        "description": tool.description,
        "input_schema": tool.parameters,
    })
}

/// Parse an Anthropic `/v1/messages` response into our unified `ChatOutput`.
fn parse_message_response(resp: Value) -> Result<ChatOutput> {
    let content_blocks = resp["content"]
        .as_array()
        .context("anthropic response missing content array")?;

    let mut text = String::new();
    let mut tool_calls = Vec::new();

    for block in content_blocks {
        match block["type"].as_str() {
            Some("text") => {
                if let Some(t) = block["text"].as_str() {
                    text.push_str(t);
                }
            }
            Some("tool_use") => {
                let id = block["id"].as_str().unwrap_or("").to_string();
                let name = block["name"].as_str().unwrap_or("").to_string();
                let arguments = block["input"].to_string();
                tool_calls.push(ToolCall {
                    id,
                    name,
                    arguments,
                });
            }
            _ => {}
        }
    }

    let usage = &resp["usage"];
    let tokens_in = usage["input_tokens"].as_u64().unwrap_or(0) as u32;
    let tokens_out = usage["output_tokens"].as_u64().unwrap_or(0) as u32;

    let finish_reason = resp["stop_reason"]
        .as_str()
        .map(std::string::ToString::to_string);

    Ok(ChatOutput {
        content: text,
        tokens_in,
        tokens_out,
        finish_reason,
        tool_calls,
        reasoning_content: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_system_works() {
        let msgs = vec![
            ChatMessageUnified::System {
                content: "You are a helpful assistant.".into(),
            },
            ChatMessageUnified::User {
                content: "Hello".into(),
            },
        ];
        let (sys, rest) = split_system_messages(&msgs);
        assert!(sys.is_some());
        assert_eq!(rest.len(), 1);
    }

    #[test]
    fn parse_simple_text_response() {
        let resp = json!({
            "id": "msg_01",
            "type": "message",
            "role": "assistant",
            "content": [
                {"type": "text", "text": "Hello!"}
            ],
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 5, "output_tokens": 1},
        });
        let output = parse_message_response(resp).unwrap();
        assert_eq!(output.content, "Hello!");
        assert_eq!(output.tokens_in, 5);
        assert_eq!(output.tokens_out, 1);
        assert_eq!(output.finish_reason, Some("end_turn".into()));
        assert!(output.tool_calls.is_empty());
    }

    #[test]
    fn parse_tool_use_response() {
        let resp = json!({
            "id": "msg_02",
            "type": "message",
            "role": "assistant",
            "content": [
                {"type": "text", "text": "Let me check..."},
                {
                    "type": "tool_use",
                    "id": "toolu_01",
                    "name": "bash",
                    "input": {"argv": ["ls", "-la"]}
                }
            ],
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 10, "output_tokens": 20},
        });
        let output = parse_message_response(resp).unwrap();
        assert!(output.content.contains("Let me check"));
        assert_eq!(output.tool_calls.len(), 1);
        assert_eq!(output.tool_calls[0].name, "bash");
        assert!(output.tool_calls[0].arguments.contains("ls"));
    }

    #[test]
    fn tool_def_conversion() {
        let def = ToolDef {
            name: "test".into(),
            description: "A test".into(),
            parameters: json!({
                "type": "object",
                "properties": {"x": {"type": "string"}},
                "required": ["x"],
            }),
            strict: true,
        };
        let v = tool_def_to_anthropic(&def);
        assert_eq!(v["name"], "test");
        assert!(v.get("input_schema").is_some());
    }
}
