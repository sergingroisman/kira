//! Pluggable LLM backends.
//!
//! KIRA talks to models through the [`LlmBackend`] trait so the backend can be
//! swapped later. Three backends exist: the local [`ollama::OllamaBackend`]
//! (the DEFAULT — localhost only), the "costly" [`copilot::CopilotBackend`]
//! (spawn of the Copilot CLI, explicit flag only, ADR-0019) and the
//! [`foundry::FoundryBackend`] (HTTP BYOK Microsoft Foundry, explicit flag
//! only, ADR-0020). The expensive backends are NEVER implicit and NEVER fall
//! back silently to Ollama: they only run under `--llm copilot|foundry` /
//! `KIRA_LLM=copilot|foundry`, and none of them sends secrets (PAT, env) in
//! the prompt — the prompt is redacted upstream by `redact`.

pub mod copilot;
pub mod foundry;
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
/// (local, free) and [`LlmKind::Copilot`] / [`LlmKind::Foundry`] only come
/// from an EXPLICIT `--llm` flag or `KIRA_LLM` env (ADR-0019 for Copilot,
/// ADR-0020 for Foundry) — a choice never happens implicitly, and a costly
/// backend never silently falls back to `ollama`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LlmKind {
    /// Local Ollama server (default).
    #[default]
    Ollama,
    /// Spawn of the Copilot CLI (costly, corporate license; ADR-0019).
    Copilot,
    /// Microsoft Foundry via HTTP OpenAI-compatible (BYOK, explicit; ADR-0020).
    Foundry,
}

/// Env var that selects the backend like the `--llm` flag does (flag wins).
pub const LLM_ENV: &str = "KIRA_LLM";

impl LlmKind {
    /// Parses one explicit value of `--llm`/`KIRA_LLM` (pure; no env access).
    /// An unknown value is an error (not a silent default).
    pub fn parse(value: &str) -> Result<Self, Error> {
        match value.trim().to_ascii_lowercase().as_str() {
            "ollama" => Ok(Self::Ollama),
            "copilot" => Ok(Self::Copilot),
            "foundry" => Ok(Self::Foundry),
            other => Err(Error::UnknownLlm {
                name: other.to_string(),
            }),
        }
    }

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
        Self::parse(&name)
    }

    /// Stable, printable name (used in messages and the session trail).
    pub fn name(self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::Copilot => "copilot",
            Self::Foundry => "foundry",
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
        assert!(matches!(
            LlmKind::from_flag_and_env(Some("foundry")),
            Ok(LlmKind::Foundry)
        ));
        assert!(matches!(
            LlmKind::from_flag_and_env(Some("FOUNDRY")),
            Ok(LlmKind::Foundry)
        ));
        assert_eq!(LlmKind::Copilot.name(), "copilot");
        assert_eq!(LlmKind::Foundry.name(), "foundry");
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
            message.contains("ollama")
                && message.contains("copilot")
                && message.contains("foundry"),
            "o erro lista as opções válidas: {message}"
        );
    }
}
