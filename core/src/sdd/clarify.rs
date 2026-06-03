//! Clarifying questions for SDD SPECIFY phase (P1, 2026-05-25).
//!
//! Workshop SPEKTOR is explicit: "Agente recebe 'implementa SSO'. Sem spec,
//! decide na hora se é SAML, OIDC, cookie, JWT, se cobre logout. Cada
//! execução produz coisa diferente. Pior: o juiz que vai avaliar depois não
//! tem critério de aceitação." Clarifying questions in the SPECIFY phase
//! are how the harness forces the agent to converge before any code lands.
//!
//! Implementation: a lightweight LLM call (tier `local` primary, `light`
//! fallback) returns 1-3 PlanQuestion JSON objects. If the model judges the
//! goal already unambiguous it returns an empty list — we skip questions
//! entirely so simple bug-fixes don't get bureaucratised (SDD auto-sizing).
//!
//! Cost: by routing this through `tier=local` (Qwen 3B on the M1) the
//! clarification step is effectively free in tokens, aligning with the
//! workshop's "barato primeiro" principle.

use anyhow::{Context, Result};
use serde::Deserialize;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::llm::types::ChatMessageUnified;
use crate::proto::sdd::{PlanOption, PlanQuestion, SddKind};
use crate::router::Router;

const SYSTEM_PROMPT: &str = include_str!("clarify_prompt.md");

/// Generate up to N clarifying questions for the given SDD goal. Returns an
/// empty Vec when the goal is already unambiguous (the model should decide).
///
/// Strategy:
/// 1. Try `tier=local` (Ollama). If unavailable or fails, fall back to `light`.
/// 2. Strict JSON output; tolerant of code fences and minor formatting noise.
/// 3. Each question gets a fresh UUID so REPL multi-select state is stable
///    across re-runs.
pub async fn generate_clarifying_questions(
    router: &Router,
    goal: &str,
    kind: SddKind,
) -> Result<Vec<PlanQuestion>> {
    let tier = if router.tier_has_candidates(crate::router::tier::Tier::Local) {
        "local"
    } else {
        "light"
    };

    let messages = vec![
        ChatMessageUnified::System {
            content: SYSTEM_PROMPT.to_string(),
        },
        ChatMessageUnified::User {
            content: format!(
                "GOAL: {goal}\nKIND: {}\n\nEmit the JSON now.",
                format!("{kind:?}").to_lowercase()
            ),
        },
    ];

    let (output, _provider, model) = router
        .route_chat_with_tools(tier, None, messages, Vec::new(), None)
        .await
        .context("clarify LLM call failed")?;

    debug!(%model, chars = output.content.len(), "clarify response");

    parse_questions(&output.content)
}

#[derive(Deserialize)]
struct QuestionDraft {
    question: String,
    #[serde(default)]
    options: Vec<OptionDraft>,
    #[serde(default)]
    multi_select: bool,
}

#[derive(Deserialize)]
struct OptionDraft {
    label: String,
    #[serde(default)]
    description: String,
}

#[derive(Deserialize)]
struct ClarifyResponse {
    #[serde(default)]
    questions: Vec<QuestionDraft>,
}

fn parse_questions(raw: &str) -> Result<Vec<PlanQuestion>> {
    let cleaned = strip_code_fences(raw.trim());
    let resp: ClarifyResponse = match serde_json::from_str(&cleaned) {
        Ok(r) => r,
        Err(e) => {
            warn!(error = %e, preview = %short(&cleaned), "clarify JSON parse failed — returning no questions");
            return Ok(Vec::new());
        }
    };
    // Cap at 3 questions to keep the user interaction short. Workshop:
    // "tarefa trivial não vira burocracia."
    let mut out = Vec::with_capacity(resp.questions.len().min(3));
    for q in resp.questions.into_iter().take(3) {
        if q.options.len() < 2 {
            // A single-option "choice" is not a question — drop it.
            continue;
        }
        out.push(PlanQuestion {
            id: Uuid::now_v7().to_string(),
            question: q.question,
            options: q
                .options
                .into_iter()
                .map(|o| PlanOption {
                    label: o.label,
                    description: o.description,
                })
                .collect(),
            multi_select: q.multi_select,
        });
    }
    Ok(out)
}

fn strip_code_fences(s: &str) -> String {
    let s = s.trim();
    if !s.starts_with("```") {
        return s.to_string();
    }
    let lines: Vec<&str> = s.lines().collect();
    if lines.len() < 2 {
        return s.to_string();
    }
    let body_start = usize::from(lines[0].starts_with("```"));
    let body_end = if *lines.last().unwrap_or(&"") == "```" {
        lines.len() - 1
    } else {
        lines.len()
    };
    lines[body_start..body_end].join("\n")
}

fn short(s: &str) -> String {
    if s.len() > 160 {
        format!("{}…", s.chars().take(160).collect::<String>())
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_well_formed_response() {
        let raw = r#"{
            "questions": [
                {
                    "question": "Which auth flow?",
                    "options": [
                        {"label": "OIDC", "description": "OpenID Connect"},
                        {"label": "SAML", "description": "Enterprise SSO"}
                    ],
                    "multi_select": false
                }
            ]
        }"#;
        let qs = parse_questions(raw).unwrap();
        assert_eq!(qs.len(), 1);
        assert_eq!(qs[0].options.len(), 2);
        assert!(!qs[0].multi_select);
    }

    #[test]
    fn parses_with_code_fence() {
        let raw = "```json\n{\"questions\": []}\n```";
        let qs = parse_questions(raw).unwrap();
        assert!(qs.is_empty());
    }

    #[test]
    fn returns_empty_when_goal_unambiguous() {
        let raw = r#"{"questions": []}"#;
        let qs = parse_questions(raw).unwrap();
        assert!(qs.is_empty());
    }

    #[test]
    fn caps_at_three_questions() {
        let raw = r#"{
            "questions": [
                {"question": "Q1", "options": [{"label":"a","description":""},{"label":"b","description":""}]},
                {"question": "Q2", "options": [{"label":"a","description":""},{"label":"b","description":""}]},
                {"question": "Q3", "options": [{"label":"a","description":""},{"label":"b","description":""}]},
                {"question": "Q4", "options": [{"label":"a","description":""},{"label":"b","description":""}]}
            ]
        }"#;
        let qs = parse_questions(raw).unwrap();
        assert_eq!(qs.len(), 3);
    }

    #[test]
    fn drops_questions_with_fewer_than_two_options() {
        let raw = r#"{
            "questions": [
                {"question": "single", "options": [{"label":"only","description":""}]},
                {"question": "real", "options": [{"label":"a","description":""},{"label":"b","description":""}]}
            ]
        }"#;
        let qs = parse_questions(raw).unwrap();
        assert_eq!(qs.len(), 1);
        assert_eq!(qs[0].question, "real");
    }

    #[test]
    fn malformed_json_returns_empty_not_error() {
        let raw = "this is not json at all";
        let qs = parse_questions(raw).unwrap();
        assert!(qs.is_empty());
    }
}
