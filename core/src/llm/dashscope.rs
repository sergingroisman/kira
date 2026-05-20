//! DashScope (Alibaba) — Qwen Max/Plus/Flash via Singapore international endpoint.

use anyhow::{Context, Result};
use async_openai::{config::OpenAIConfig, Client};

use super::LlmProvider;

const DASHSCOPE_INTL_BASE_URL: &str = "https://dashscope-intl.aliyuncs.com/compatible-mode/v1";

/// Build a DashScope `LlmProvider` reading `DASHSCOPE_API_KEY` from env.
pub fn build() -> Result<LlmProvider> {
    let api_key = std::env::var("DASHSCOPE_API_KEY")
        .context("DASHSCOPE_API_KEY missing — set it in .env or environment")?;
    let config = OpenAIConfig::new()
        .with_api_base(DASHSCOPE_INTL_BASE_URL)
        .with_api_key(api_key);
    Ok(LlmProvider::new("dashscope", Client::with_config(config)))
}
