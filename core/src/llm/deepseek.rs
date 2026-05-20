//! DeepSeek — DeepSeek V4 Pro / Flash via OpenAI-compat endpoint.

use anyhow::Result;
use async_openai::{config::OpenAIConfig, Client};

use super::LlmProvider;

const DEEPSEEK_BASE_URL: &str = "https://api.deepseek.com/v1";

/// Build a DeepSeek `LlmProvider` reading `DEEPSEEK_API_KEY` from env.
/// Returns `Ok(None)` if the key is unset — caller decides whether that's fatal.
pub fn build() -> Result<Option<LlmProvider>> {
    let api_key = match std::env::var("DEEPSEEK_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => return Ok(None),
    };
    let config = OpenAIConfig::new()
        .with_api_base(DEEPSEEK_BASE_URL)
        .with_api_key(api_key);
    Ok(Some(LlmProvider::new(
        "deepseek",
        Client::with_config(config),
    )))
}
