//! Pluggable LLM backends.
//!
//! KIRA talks to models through the [`LlmBackend`] trait so the backend can be
//! swapped later (planned: `copilot-sdk`, `byok-http`, `ollama`). In this phase
//! two backends exist: the local [`ollama::OllamaBackend`] (the DEFAULT —
//! localhost only) and the "costly" [`copilot::CopilotBackend`] (spawn of the
//! Copilot CLI, explicit flag only, ADR-0019). Neither sends secrets (PAT,
//! env) in the prompt.

pub mod copilot;
pub mod ollama;
pub mod prompts;

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

/// Which backend a command runs on: [`LlmKind::Ollama`] is the DEFAULT
/// (local, free) and [`LlmKind::Copilot`] only comes from an EXPLICIT
/// `--llm copilot` flag or `KIRA_LLM=copilot` env (ADR-0019) — a choice
/// never happens implicitly, and `copilot` never silently falls back to
/// `ollama`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmKind {
    /// Local Ollama server (default).
    Ollama,
    /// Spawn of the Copilot CLI (costly, corporate license).
    Copilot,
}

/// Env var that selects the backend like the `--llm` flag does (flag wins).
pub const LLM_ENV: &str = "KIRA_LLM";

impl LlmKind {
    /// Resolves the backend from the flag value first, then `KIRA_LLM`,
    /// defaulting to [`LlmKind::Ollama`]. An unknown value is an error (not
    /// a silent default).
    pub fn from_flag_and_env(flag: Option<&str>) -> Result<Self, Error> {
        let name = flag
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or_else(|| {
                std::env::var(LLM_ENV)
                    .ok()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
            });
        let Some(name) = name else {
            return Ok(Self::Ollama);
        };
        match name.to_ascii_lowercase().as_str() {
            "ollama" => Ok(Self::Ollama),
            "copilot" => Ok(Self::Copilot),
            _ => Err(Error::UnknownLlm { name }),
        }
    }

    /// Stable, printable name (used in messages and the session trail).
    pub fn name(self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::Copilot => "copilot",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only flag values below are asserted (never `None`): the env fallback
    /// makes `None` depend on the environment, which parallel tests cannot
    /// mutate safely.
    #[test]
    fn explicit_flag_selects_the_backend_and_unknown_fails() {
        assert!(matches!(
            LlmKind::from_flag_and_env(Some("copilot")),
            Ok(LlmKind::Copilot)
        ));
        assert!(matches!(
            LlmKind::from_flag_and_env(Some("ollama")),
            Ok(LlmKind::Ollama)
        ));
        assert!(matches!(
            LlmKind::from_flag_and_env(Some("Copilot")),
            Ok(LlmKind::Copilot)
        ));
        let Err(err) = LlmKind::from_flag_and_env(Some("gpt")) else {
            panic!("backend desconhecido deve falhar, não calar");
        };
        let Error::UnknownLlm { name } = err else {
            panic!("variante deve ser UnknownLlm");
        };
        assert_eq!(name, "gpt");
        let message = LlmKind::from_flag_and_env(Some("gpt"))
            .unwrap_err()
            .to_string();
        assert!(
            message.contains("ollama") && message.contains("copilot"),
            "o erro lista as opções válidas: {message}"
        );
    }
}
