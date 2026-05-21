//! Embedding wire types.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Deserialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/embed.ts")]
pub struct EmbedRequest {
    pub input: Vec<String>,
    #[serde(default = "default_embed_model")]
    pub model: String,
}

fn default_embed_model() -> String {
    "text-embedding-v3".to_string()
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../../agent/src/generated/embed.ts")]
pub struct EmbedResponse {
    pub model: String,
    pub vectors: Vec<Vec<f32>>,
    pub dim: usize,
}
