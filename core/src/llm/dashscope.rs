//! DashScope (Alibaba) client — Qwen Max/Plus/Flash via OpenAI-compat endpoint.

use anyhow::{Context, Result};
use async_openai::{config::OpenAIConfig, Client};

const DASHSCOPE_INTL_BASE_URL: &str = "https://dashscope-intl.aliyuncs.com/compatible-mode/v1";

/// Build a DashScope client (Singapore region by default).
///
/// Reads `DASHSCOPE_API_KEY` from env. Errors if missing.
pub fn build_client() -> Result<Client<OpenAIConfig>> {
    let api_key = std::env::var("DASHSCOPE_API_KEY")
        .context("DASHSCOPE_API_KEY missing — set it in .env or environment")?;
    let config = OpenAIConfig::new()
        .with_api_base(DASHSCOPE_INTL_BASE_URL)
        .with_api_key(api_key);
    Ok(Client::with_config(config))
}
