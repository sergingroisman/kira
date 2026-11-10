//! Pluggable LLM backends.
//!
//! KIRA talks to models through the [`LlmBackend`] trait so the backend can be
//! swapped later (planned: `copilot-sdk`, `byok-http`, `ollama`). In this phase
//! only the local [`ollama::OllamaBackend`] exists: it talks to a local Ollama
//! server on localhost and never sends secrets (PAT, env) in the prompt.

pub mod ollama;

use std::future::Future;

use crate::error::Error;

/// A backend able to complete a single prompt.
///
/// Implementations are read-only: they send the system/prompt pair and return
/// the model text. Nothing here writes code or touches Azure DevOps.
pub trait LlmBackend {
    /// Completes one prompt with the given system instruction.
    fn complete(
        &self,
        system: &str,
        prompt: &str,
    ) -> impl Future<Output = Result<String, Error>> + Send;
}
