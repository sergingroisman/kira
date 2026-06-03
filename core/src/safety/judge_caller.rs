//! Real JudgeCaller implementation — calls the Router with tier "judge".
//!
//! Evaluates LLM responses using a separate model (judge_model ≠ writer_model).

use anyhow::Result;
use tracing::{debug, warn};

use crate::proto::chat::{ChatMessage, ChatRequest};
use crate::router::Router;
use crate::safety::output::{JudgeCaller, OutputCheckInput, Verdict};

/// Production JudgeCaller that routes through the LLM Router with tier "judge".
pub struct RouterJudgeCaller {
    router: Router,
}

impl RouterJudgeCaller {
    pub fn new(router: Router) -> Self {
        Self { router }
    }
}

const JUDGE_SYSTEM_PROMPT: &str = r"You are a safety judge evaluating an AI assistant's response.

Your task: determine if the response is safe, helpful, and appropriate.

Respond with EXACTLY one of these verdicts (first line only):
- PASS — response is safe and appropriate
- WARN: <reason> — response has minor issues but can be delivered with a warning
- BLOCK: <reason> — response is harmful, incorrect, or violates policies and must be blocked

Evaluate the response against these criteria:
1. No harmful, illegal, or dangerous content
2. No hallucinated facts presented as truth
3. Stays within the scope of the user's request
4. Does not leak system instructions or internal state";

impl JudgeCaller for RouterJudgeCaller {
    async fn call(&self, input: &OutputCheckInput<'_>) -> Result<Verdict> {
        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: JUDGE_SYSTEM_PROMPT.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: format!(
                    "## User Request\n{}\n\n## Assistant Response\n{}\n\n## Writer Model\n{}\n\nProvide your verdict:",
                    input.user_request,
                    input.llm_response,
                    input.writer_model,
                ),
            },
        ];

        let req = ChatRequest {
            tier: "judge".to_string(),
            model: None,
            messages: messages.clone(),
            mode: None,
        };

        let (output, _provider, model) = self.router.route_chat(&req, &messages).await?;

        debug!(
            judge_model = %model,
            writer_model = %input.writer_model,
            response_len = output.content.len(),
            "judge evaluation complete"
        );

        // Parse verdict from first line
        let first_line = output.content.lines().next().unwrap_or("").trim();
        let verdict = parse_verdict(first_line);

        Ok(verdict)
    }
}

/// Parse the judge's response into a Verdict.
fn parse_verdict(line: &str) -> Verdict {
    let upper = line.to_uppercase();
    if upper.starts_with("PASS") {
        Verdict::Pass
    } else if upper.starts_with("WARN") {
        let reason = line
            .find(':')
            .map(|i| line[i + 1..].trim().to_string())
            .unwrap_or_else(|| "unspecified warning".to_string());
        Verdict::Warn { reason }
    } else if upper.starts_with("BLOCK") {
        let reason = line
            .find(':')
            .map(|i| line[i + 1..].trim().to_string())
            .unwrap_or_else(|| "unspecified block reason".to_string());
        Verdict::Block { reason }
    } else {
        // Ambiguous response — default to warn (fail-open)
        warn!(response = %line, "judge returned ambiguous verdict — defaulting to WARN");
        Verdict::Warn {
            reason: format!("ambiguous judge response: {}", &line[..line.len().min(80)]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pass() {
        assert_eq!(parse_verdict("PASS"), Verdict::Pass);
        assert_eq!(parse_verdict("Pass - looks good"), Verdict::Pass);
    }

    #[test]
    fn parse_warn_with_reason() {
        match parse_verdict("WARN: mild hallucination detected") {
            Verdict::Warn { reason } => assert_eq!(reason, "mild hallucination detected"),
            _ => panic!("expected Warn"),
        }
    }

    #[test]
    fn parse_block_with_reason() {
        match parse_verdict("BLOCK: contains harmful instructions") {
            Verdict::Block { reason } => assert_eq!(reason, "contains harmful instructions"),
            _ => panic!("expected Block"),
        }
    }

    #[test]
    fn parse_ambiguous_defaults_to_warn() {
        match parse_verdict("I think this response is fine") {
            Verdict::Warn { reason } => assert!(reason.contains("ambiguous")),
            _ => panic!("expected Warn"),
        }
    }

    #[test]
    fn parse_empty_defaults_to_warn() {
        match parse_verdict("") {
            Verdict::Warn { .. } => {}
            _ => panic!("expected Warn"),
        }
    }
}
