//! Auto-generate a Lesson from a failure context using a Light-tier LLM call.

use anyhow::{Context, Result};
use serde::Deserialize;
use tracing::{instrument, warn};

use crate::llm::types::ChatMessageUnified;
use crate::proto::agent::AgentKind;
use crate::proto::lesson::{LessonPriority, LessonScope};
use crate::router::Router;

const GEN_PROMPT: &str = "You are Kira's lesson scribe. \
Given a failure context, emit a STRICT JSON lesson to remember next time. \
No prose, no markdown fences. Schema: \
{\"trigger\":\"<terse signal that fired>\",\
\"anti_pattern\":\"<one short sentence: what the agent did wrong>\",\
\"preferred_pattern\":\"<one short imperative sentence: what to do instead>\",\
\"priority\":\"low|medium|high\"}. \
Rules: \
- Each field <= 15 words. \
- Reuse the failure's own vocabulary; do NOT invent new terminology. \
- Priority: high if it caused a destructive/expensive failure; medium if it \
  produced wrong output without damage; low for cosmetic issues. \
- NEVER fabricate a fix you don't actually know works. If unsure, prefer \
  defensive guidance like 'verify before acting' over a specific incorrect fix.";

#[derive(Deserialize)]
struct GeneratedLesson {
    trigger: String,
    anti_pattern: String,
    preferred_pattern: String,
    priority: String,
}

/// Build a Lesson via Light-tier LLM. Returns
/// `(trigger, anti_pattern, preferred_pattern, priority, scope)` ready for
/// `LessonRegistry::record(...)`. Returns Err on LLM failure or unparseable
/// output — callers should treat lesson-generation as best-effort.
#[instrument(skip(router, context), fields(context_len = context.len()))]
pub async fn generate_from_failure(
    router: &Router,
    context: &str,
    scope_hint: Option<AgentKind>,
) -> Result<(String, String, String, LessonPriority, LessonScope)> {
    let user = format!(
        "Failure context:\n```\n{ctx}\n```\nReturn JSON now.",
        ctx = context.chars().take(2000).collect::<String>()
    );
    let messages = vec![
        ChatMessageUnified::System {
            content: GEN_PROMPT.to_string(),
        },
        ChatMessageUnified::User {
            content: user,
        },
    ];
    let (resp, _provider, _model) = router
        .route_chat_with_tools("light", None, messages, Vec::new())
        .await
        .context("lesson generator LLM call failed")?;

    let content = resp.content;

    let json = extract_json(&content).context("lesson generator returned non-JSON")?;
    let parsed: GeneratedLesson =
        serde_json::from_str(&json).context("lesson generator JSON malformed")?;

    let priority = match parsed.priority.to_lowercase().as_str() {
        "high" => LessonPriority::High,
        "low" => LessonPriority::Low,
        _ => LessonPriority::Medium,
    };
    let scope = match scope_hint {
        Some(k) => LessonScope::AgentKind { agent_kind: k },
        None => LessonScope::Global,
    };
    Ok((
        parsed.trigger,
        parsed.anti_pattern,
        parsed.preferred_pattern,
        priority,
        scope,
    ))
}

fn extract_json(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if let Ok(_) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return Some(trimmed.to_string());
    }
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if end > start {
        Some(trimmed[start..=end].to_string())
    } else {
        None
    }
}

/// Fire-and-forget: spawn a tokio task that generates a lesson and records it.
/// Failure to generate is logged and dropped.
pub fn fire_and_forget(
    router: Router,
    registry: super::LessonRegistry,
    context: String,
    scope_hint: Option<AgentKind>,
) {
    tokio::spawn(async move {
        match generate_from_failure(&router, &context, scope_hint).await {
            Ok((trigger, anti, pref, prio, scope)) => {
                registry.record(trigger, anti, pref, prio, scope);
            }
            Err(e) => {
                warn!(error = ?e, "lesson auto-generation failed");
            }
        }
    });
}
