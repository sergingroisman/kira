//! Chat wire types — exported to TypeScript via `ts-rs`.
//!
//! Running `cargo test` regenerates `agent/src/generated/chat.ts`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Deserialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/chat.ts")]
pub struct ChatRequest {
    /// Tier hint — "heavy" | "standard" (default) | "light" | "`long_context`".
    #[serde(default = "default_tier")]
    pub tier: String,

    /// Override model explicitly (testing). If set, bypasses tier mapping.
    pub model: Option<String>,

    pub messages: Vec<ChatMessage>,

    /// Execution mode: "plan" (default, plan-first via Pilot) or "yolo"
    /// (direct agent spawn, reduced gates, no plan approval).
    #[serde(default)]
    pub mode: Option<String>,
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

/// Response for YOLO mode chat — direct agent spawn.
#[derive(Serialize, TS)]
#[ts(export, export_to = "../../agent/src/generated/chat.ts")]
pub struct YoloResponse {
    pub agent_id: String,
    pub kind: String,
    pub status: String,
    /// Direct inline answer (only on degraded fallback). When an agent is
    /// spawned the answer streams via the agent poller, so this is `None` and
    /// the client polls `agent_id`.
    pub message: Option<String>,
    /// Routing taken: "lite" (read-only Explorer for a conceptual question),
    /// "full" (Writer agent with all tools), or "fallback" (inline single-shot).
    pub route: String,
}
