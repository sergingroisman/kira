//! Chat wire types — exported to TypeScript via `ts-rs`.
//!
//! Running `cargo test` regenerates `agent/src/generated/chat.ts`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Deserialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/chat.ts")]
pub struct ChatRequest {
    /// Tier hint — "heavy" | "standard" (default) | "light" | "long_context".
    #[serde(default = "default_tier")]
    pub tier: String,

    /// Override model explicitly (testing). If set, bypasses tier mapping.
    pub model: Option<String>,

    pub messages: Vec<ChatMessage>,
}

fn default_tier() -> String {
    "standard".to_string()
}

#[derive(Deserialize, Serialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/chat.ts")]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../../agent/src/generated/chat.ts")]
pub struct ChatResponse {
    pub content: String,
    pub model: String,
    pub tokens_in: u32,
    pub tokens_out: u32,
    pub finish_reason: Option<String>,
}

/// SSE event payloads (server → client streaming).
#[derive(Serialize, TS)]
#[ts(export, export_to = "../../agent/src/generated/chat.ts")]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    Token { delta: String },
    Done { model: String, finish_reason: Option<String> },
    Error { message: String },
}
