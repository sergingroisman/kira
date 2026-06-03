//! Embedding client — calls Ollama (or any OpenAI-compatible) for vector embeddings.
//!
//! Used by Memory Retrieval (query embedding) and Memory Extractor (fragment embedding).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::debug;

/// Configuration for the embedding client.
#[derive(Debug, Clone)]
pub struct EmbedConfig {
    /// Base URL of the embedding provider (e.g., "http://127.0.0.1:11434").
    pub base_url: String,
    /// Model name (e.g., "nomic-embed-text").
    pub model: String,
}

impl Default for EmbedConfig {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:11434".to_string(),
            model: "nomic-embed-text".to_string(),
        }
    }
}

/// Embedding client that calls Ollama's /api/embed endpoint.
pub struct EmbedClient {
    http: reqwest::Client,
    config: EmbedConfig,
}

#[derive(Serialize)]
struct EmbedRequest<'a> {
    model: &'a str,
    input: &'a str,
}

#[derive(Deserialize)]
struct EmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

impl EmbedClient {
    pub fn new(config: EmbedConfig) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build HTTP client");
        Self { http, config }
    }

    /// Generate an embedding vector for the given text.
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let url = format!("{}/api/embed", self.config.base_url);

        let resp = self
            .http
            .post(&url)
            .json(&EmbedRequest {
                model: &self.config.model,
                input: text,
            })
            .send()
            .await
            .context("embedding request failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("embedding failed: {status} — {body}");
        }

        let data: EmbedResponse = resp.json().await.context("parsing embedding response")?;

        let embedding = data
            .embeddings
            .into_iter()
            .next()
            .context("no embedding returned")?;

        debug!(
            model = %self.config.model,
            dims = embedding.len(),
            text_len = text.len(),
            "embedding generated"
        );

        Ok(embedding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let cfg = EmbedConfig::default();
        assert_eq!(cfg.base_url, "http://127.0.0.1:11434");
        assert_eq!(cfg.model, "nomic-embed-text");
    }

    /// Integration test — only runs when Ollama is available.
    /// Run with: cargo test -p kira-core memory::embed::tests::embed_real -- --ignored
    #[tokio::test]
    #[ignore]
    async fn embed_real() {
        let client = EmbedClient::new(EmbedConfig::default());
        let embedding = client.embed("hello world").await.unwrap();
        assert!(!embedding.is_empty());
        assert!(embedding.len() > 100); // nomic-embed-text produces 768-dim vectors
        // Verify it's normalized (magnitude ≈ 1.0)
        let mag: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((mag - 1.0).abs() < 0.1, "magnitude should be ~1.0, got {mag}");
    }
}
