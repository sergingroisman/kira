//! Output Guardrail — Level 3 (LLM-as-Judge).
//!
//! Evaluates the final LLM response BEFORE delivering to the user.
//! Uses a separate Judge Model (MUST differ from the Writer Model) to
//! produce a verdict: pass, warn, or block.
//!
//! Latency target: P95 < 10s for responses up to 4K tokens.

use anyhow::{bail, Result};

/// Configuration for the Output Guardrail.
#[derive(Debug, Clone)]
pub struct OutputGuardrailConfig {
    /// Whether the output guardrail is enabled.
    pub enabled: bool,
    /// Timeout in seconds for the Judge LLM call.
    pub judge_timeout_secs: u64,
}

impl Default for OutputGuardrailConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            judge_timeout_secs: 10,
        }
    }
}

/// Verdict from the Judge Model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Response is safe — deliver unchanged.
    Pass,
    /// Response has minor issues — deliver with warning attached.
    Warn { reason: String },
    /// Response is harmful/incorrect — block delivery, substitute error.
    Block { reason: String },
}

/// Result of the output guardrail evaluation.
#[derive(Debug, Clone)]
pub struct OutputCheckResult {
    pub verdict: Verdict,
    pub judge_model: String,
    pub writer_model: String,
    /// The response to deliver (original, modified, or error substitute).
    pub response: String,
}

/// Context needed to evaluate an LLM response.
pub struct OutputCheckInput<'a> {
    /// The original user request/task.
    pub user_request: &'a str,
    /// The LLM-generated response to evaluate.
    pub llm_response: &'a str,
    /// The model that generated the response.
    pub writer_model: &'a str,
    /// The model designated for judging.
    pub judge_model: &'a str,
}

/// Evaluate an LLM response using the Judge Model.
///
/// This is the orchestration function — the actual LLM call is abstracted
/// behind the `JudgeCaller` trait so we can test without a real provider.
pub async fn evaluate<J: JudgeCaller>(
    input: &OutputCheckInput<'_>,
    config: &OutputGuardrailConfig,
    judge: &J,
) -> Result<OutputCheckResult> {
    // Guard: skip if disabled
    if !config.enabled {
        return Ok(OutputCheckResult {
            verdict: Verdict::Pass,
            judge_model: input.judge_model.to_string(),
            writer_model: input.writer_model.to_string(),
            response: input.llm_response.to_string(),
        });
    }

    // Invariant: judge_model ≠ writer_model
    if input.judge_model == input.writer_model {
        bail!(
            "judge_model_collision: judge '{}' == writer '{}'. \
             The judge model MUST differ from the writer model.",
            input.judge_model,
            input.writer_model
        );
    }

    // Call the Judge
    let verdict = match judge.call(input).await {
        Ok(v) => v,
        Err(_) => {
            // Judge unavailable — deliver with warning (fail-open for UX)
            return Ok(OutputCheckResult {
                verdict: Verdict::Warn {
                    reason: "judge_unavailable".to_string(),
                },
                judge_model: input.judge_model.to_string(),
                writer_model: input.writer_model.to_string(),
                response: input.llm_response.to_string(),
            });
        }
    };

    let response = match &verdict {
        Verdict::Pass | Verdict::Warn { .. } => input.llm_response.to_string(),
        Verdict::Block { reason } => {
            format!(
                "[BLOCKED by Output Guardrail] Reason: {reason}. \
                 The original response was suppressed."
            )
        }
    };

    Ok(OutputCheckResult {
        verdict,
        judge_model: input.judge_model.to_string(),
        writer_model: input.writer_model.to_string(),
        response,
    })
}

// ── Judge Caller Trait ────────────────────────────────────────────────────────

/// Abstraction over the actual LLM call for judging.
/// Implementations: real provider client (production) or mock (tests).
#[allow(async_fn_in_trait)]
pub trait JudgeCaller: Send + Sync {
    async fn call(&self, input: &OutputCheckInput<'_>) -> Result<Verdict>;
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Mock Judge ────────────────────────────────────────────────────────

    struct MockJudge {
        verdict: Verdict,
    }

    impl JudgeCaller for MockJudge {
        async fn call(&self, _input: &OutputCheckInput<'_>) -> Result<Verdict> {
            Ok(self.verdict.clone())
        }
    }

    struct FailingJudge;

    impl JudgeCaller for FailingJudge {
        async fn call(&self, _input: &OutputCheckInput<'_>) -> Result<Verdict> {
            bail!("provider timeout")
        }
    }

    fn make_input<'a>(
        response: &'a str,
        writer: &'a str,
        judge: &'a str,
    ) -> OutputCheckInput<'a> {
        OutputCheckInput {
            user_request: "implement login",
            llm_response: response,
            writer_model: writer,
            judge_model: judge,
        }
    }

    // ── Tests ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn pass_verdict_delivers_unchanged() {
        let judge = MockJudge { verdict: Verdict::Pass };
        let cfg = OutputGuardrailConfig::default();
        let input = make_input("here is the code", "deepseek-chat", "qwen-max");

        let result = evaluate(&input, &cfg, &judge).await.unwrap();
        assert_eq!(result.verdict, Verdict::Pass);
        assert_eq!(result.response, "here is the code");
        assert_eq!(result.judge_model, "qwen-max");
        assert_eq!(result.writer_model, "deepseek-chat");
    }

    #[tokio::test]
    async fn warn_verdict_delivers_with_warning() {
        let judge = MockJudge {
            verdict: Verdict::Warn { reason: "mild issue".to_string() },
        };
        let cfg = OutputGuardrailConfig::default();
        let input = make_input("response text", "deepseek-chat", "qwen-max");

        let result = evaluate(&input, &cfg, &judge).await.unwrap();
        assert_eq!(
            result.verdict,
            Verdict::Warn { reason: "mild issue".to_string() }
        );
        assert_eq!(result.response, "response text");
    }

    #[tokio::test]
    async fn block_verdict_substitutes_response() {
        let judge = MockJudge {
            verdict: Verdict::Block { reason: "harmful content".to_string() },
        };
        let cfg = OutputGuardrailConfig::default();
        let input = make_input("evil response", "deepseek-chat", "qwen-max");

        let result = evaluate(&input, &cfg, &judge).await.unwrap();
        assert!(matches!(result.verdict, Verdict::Block { .. }));
        assert!(result.response.contains("[BLOCKED by Output Guardrail]"));
        assert!(result.response.contains("harmful content"));
        assert!(!result.response.contains("evil response"));
    }

    #[tokio::test]
    async fn judge_model_collision_returns_error() {
        let judge = MockJudge { verdict: Verdict::Pass };
        let cfg = OutputGuardrailConfig::default();
        // Same model for both — must fail
        let input = make_input("response", "deepseek-chat", "deepseek-chat");

        let err = evaluate(&input, &cfg, &judge).await.unwrap_err();
        assert!(err.to_string().contains("judge_model_collision"));
    }

    #[tokio::test]
    async fn disabled_guardrail_skips_evaluation() {
        let judge = MockJudge {
            verdict: Verdict::Block { reason: "should not reach".to_string() },
        };
        let cfg = OutputGuardrailConfig {
            enabled: false,
            ..Default::default()
        };
        let input = make_input("response", "deepseek-chat", "qwen-max");

        let result = evaluate(&input, &cfg, &judge).await.unwrap();
        assert_eq!(result.verdict, Verdict::Pass);
        assert_eq!(result.response, "response");
    }

    #[tokio::test]
    async fn judge_failure_delivers_with_warn() {
        let judge = FailingJudge;
        let cfg = OutputGuardrailConfig::default();
        let input = make_input("response", "deepseek-chat", "qwen-max");

        let result = evaluate(&input, &cfg, &judge).await.unwrap();
        assert_eq!(
            result.verdict,
            Verdict::Warn { reason: "judge_unavailable".to_string() }
        );
        assert_eq!(result.response, "response");
    }

    #[tokio::test]
    async fn separation_invariant_always_holds() {
        // Property: for any evaluation, judge_model ≠ writer_model
        let judge = MockJudge { verdict: Verdict::Pass };
        let cfg = OutputGuardrailConfig::default();

        let pairs = [
            ("model-a", "model-b"),
            ("deepseek-chat", "qwen-max"),
            ("ollama:qwen2.5:7b", "deepseek-chat"),
        ];

        for (writer, judge_model) in pairs {
            let input = make_input("test", writer, judge_model);
            let result = evaluate(&input, &cfg, &judge).await.unwrap();
            assert_ne!(result.writer_model, result.judge_model);
        }
    }
}
