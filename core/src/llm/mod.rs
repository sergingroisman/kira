//! LLM providers — unified struct over OpenAI-compatible endpoints.
//!
//! v0 providers: DashScope (Qwen), DeepSeek. Both use `async-openai`.
//! v0.5+: SiliconFlow, Ollama, OpenRouter (eval-only).
//!
//! Per plan ("Banir abstrações precoces"): single `LlmProvider` struct,
//! constructors per provider. When a provider needs a non-OpenAI-compat shape
//! (e.g. GLM raw), refactor into enum/trait then.

pub mod dashscope;
pub mod deepseek;

use anyhow::{Context, Result};
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestUserMessageArgs,
        ChatCompletionTool, CreateChatCompletionRequestArgs, CreateChatCompletionResponse,
    },
    Client,
};
use std::sync::Arc;

use crate::proto::chat::ChatMessage;

/// A single LLM provider (DashScope, DeepSeek, ...).
#[derive(Clone)]
pub struct LlmProvider {
    pub name: String,
    client: Arc<Client<OpenAIConfig>>,
}

impl LlmProvider {
    pub fn new(name: impl Into<String>, client: Client<OpenAIConfig>) -> Self {
        Self {
            name: name.into(),
            client: Arc::new(client),
        }
    }

    /// Build `async-openai` request messages from wire format.
    fn build_messages(messages: &[ChatMessage]) -> Result<Vec<ChatCompletionRequestMessage>> {
        messages
            .iter()
            .map(|m| -> Result<ChatCompletionRequestMessage> {
                let msg = ChatCompletionRequestUserMessageArgs::default()
                    .content(m.content.clone())
                    .build()?;
                Ok(msg.into())
            })
            .collect()
    }

    /// Non-streaming chat. Returns full response.
    pub async fn chat(
        &self,
        model: &str,
        messages: &[ChatMessage],
    ) -> Result<async_openai::types::CreateChatCompletionResponse> {
        let msgs = Self::build_messages(messages)?;
        let request = CreateChatCompletionRequestArgs::default()
            .model(model)
            .messages(msgs)
            .build()?;
        self.client
            .chat()
            .create(request)
            .await
            .with_context(|| format!("{} chat failed (model={model})", self.name))
    }

    /// Chat with tool-calling enabled. Accepts pre-built request messages
    /// (so agent loops can include `Tool` result messages between turns) and
    /// a list of tool schemas exposed to the model.
    pub async fn chat_with_tools(
        &self,
        model: &str,
        messages: Vec<ChatCompletionRequestMessage>,
        tools: Vec<ChatCompletionTool>,
    ) -> Result<CreateChatCompletionResponse> {
        let mut builder = CreateChatCompletionRequestArgs::default();
        builder.model(model).messages(messages);
        if !tools.is_empty() {
            builder.tools(tools);
        }
        let request = builder.build()?;
        self.client
            .chat()
            .create(request)
            .await
            .with_context(|| format!("{} chat_with_tools failed (model={model})", self.name))
    }

    /// Streaming chat. Returns a `Stream` of chunks.
    pub async fn chat_stream(
        &self,
        model: &str,
        messages: &[ChatMessage],
    ) -> Result<async_openai::types::ChatCompletionResponseStream> {
        let msgs = Self::build_messages(messages)?;
        let request = CreateChatCompletionRequestArgs::default()
            .model(model)
            .messages(msgs)
            .stream(true)
            .build()?;
        self.client
            .chat()
            .create_stream(request)
            .await
            .with_context(|| format!("{} chat_stream failed (model={model})", self.name))
    }
}
