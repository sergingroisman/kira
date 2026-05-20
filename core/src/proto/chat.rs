//! Chat wire types.

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct ChatRequest {
    /// Tier hint — "heavy" | "standard" (default) | "light".
    /// Router (Sem 4-5) will use this to select model + fallback chain.
    #[serde(default = "default_tier")]
    pub tier: String,

    /// Override model explicitly (testing). If set, bypasses tier mapping.
    pub model: Option<String>,

    pub messages: Vec<ChatMessage>,
}

fn default_tier() -> String {
    "standard".to_string()
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
pub struct ChatResponse {
    pub content: String,
    pub model: String,
    pub tokens_in: u32,
    pub tokens_out: u32,
    pub finish_reason: Option<String>,
}

/// SSE event payloads (server → client streaming).
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    Token { delta: String },
    Done { model: String, finish_reason: Option<String> },
    Error { message: String },
}

/// Map a tier hint to the v0 hardcoded model. M01 Router (Sem 4-5) replaces this.
pub fn tier_to_model(tier: &str) -> &'static str {
    match tier {
        "heavy" => "qwen-max",
        "light" => "qwen-flash",
        _ => "qwen-plus", // standard default
    }
}
