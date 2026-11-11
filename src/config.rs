//! Configuration read from the environment (ADR-0016 layer 2: parsing, no LLM).
//!
//! These knobs belong to whoever *uses* them, not to the model backend: the
//! context window sizes the prompt budget in [`crate::context`] and the request
//! option sent by the Ollama backend. Keeping them here is what stops the context
//! packer from having to import the model layer — enforced by
//! `tests/llm_allowlist.rs`.

/// Context window used when `KIRA_NUM_CTX` is unset or invalid.
pub const DEFAULT_NUM_CTX: u32 = 4096;

/// Reads the context window from `KIRA_NUM_CTX`, falling back to
/// [`DEFAULT_NUM_CTX`]. An unparsable or zero value falls back too.
#[must_use]
pub fn num_ctx_from_env() -> u32 {
    std::env::var("KIRA_NUM_CTX")
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_NUM_CTX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_four_thousand_and_ninety_six() {
        // Não mexemos em env aqui: outros testes no mesmo binário dependem dela.
        // O contrato testável é o default e o piso.
        assert_eq!(DEFAULT_NUM_CTX, 4096);
    }
}
