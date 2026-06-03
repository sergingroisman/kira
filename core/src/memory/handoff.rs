//! Session Handoff — generation, persistence (redb), and injection.
//!
//! Persists a singleton `HandoffSummary` in redb under key `"current"`.
//! Only one handoff exists at a time; generating a new one overwrites the old.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::config::MaturationConfig;
use crate::observability::wire::PillarTx;
use crate::proto::chat::{ChatMessage, ChatRequest};
use crate::router::tier::Tier;
use crate::router::Router as LlmRouter;
use crate::safety::delimiters;
use crate::state::KiraDb;

// Re-export the table definition from state for external consumers.
pub use crate::state::TBL_HANDOFF;

/// The structured handoff summary persisted in redb.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffSummary {
    pub session_id: Uuid,
    pub context: String,
    pub open_questions: Vec<String>,
    pub next_steps: Vec<String>,
    pub decisions: Vec<String>,
    pub generated_at: DateTime<Utc>,
    pub consumed: bool,
    /// Timestamp of last SSE connection close (for reconnect debounce).
    pub last_disconnect_at: Option<DateTime<Utc>>,
}

/// Singleton key — only one handoff exists at a time.
const HANDOFF_KEY: &str = "current";

/// Retrieve the current handoff from redb.
///
/// Returns `None` if absent or corrupted (deserialization failure logs an
/// error and discards the value).
pub fn get_handoff(db: &KiraDb) -> Option<HandoffSummary> {
    match db.list_one(TBL_HANDOFF, HANDOFF_KEY) {
        Ok(Some(json)) => match serde_json::from_str::<HandoffSummary>(&json) {
            Ok(summary) => Some(summary),
            Err(e) => {
                error!(
                    error = %e,
                    "corrupted handoff in redb — discarding"
                );
                None
            }
        },
        Ok(None) => None,
        Err(e) => {
            warn!(error = %e, "failed to read handoff from redb");
            None
        }
    }
}

/// Mark the current handoff as consumed (injected into a session).
///
/// Reads the current handoff, sets `consumed = true`, and writes it back.
/// Returns an error if no handoff exists or the write fails.
pub fn mark_consumed(db: &KiraDb) -> Result<()> {
    let mut summary = get_handoff(db)
        .ok_or_else(|| anyhow::anyhow!("no handoff to mark as consumed"))?;
    summary.consumed = true;
    db.put(TBL_HANDOFF, HANDOFF_KEY, &summary)?;
    Ok(())
}

/// Delete the current handoff (manual clear via API).
///
/// Returns `Ok(true)` if a handoff was deleted, `Ok(false)` if nothing
/// existed (idempotent).
pub fn delete_handoff(db: &KiraDb) -> Result<bool> {
    db.delete(TBL_HANDOFF, HANDOFF_KEY)
}

// ─── Injection & Session Lifecycle ───────────────────────────────────────────

/// Reconnect debounce window: if the SSE connection drops and reconnects
/// within this duration, treat it as the same session (no re-injection,
/// no handoff generation, no consolidation).
const RECONNECT_DEBOUNCE_SECS: i64 = 30;

/// Format the handoff for prompt injection.
///
/// Wraps the handoff content in `<RAG_CONTEXT source="handoff" untrusted="true">`
/// delimiters, consistent with KIRA's XML delimiter enforcement. Includes all
/// four sections (context, open_questions, next_steps, decisions) in a readable
/// format.
pub fn format_handoff_injection(handoff: &HandoffSummary) -> String {
    let mut body = String::new();

    body.push_str("## Session Handoff\n\n");

    body.push_str("### Context\n");
    body.push_str(&handoff.context);
    body.push('\n');

    if !handoff.open_questions.is_empty() {
        body.push_str("\n### Open Questions\n");
        for q in &handoff.open_questions {
            body.push_str("- ");
            body.push_str(q);
            body.push('\n');
        }
    }

    if !handoff.next_steps.is_empty() {
        body.push_str("\n### Next Steps\n");
        for s in &handoff.next_steps {
            body.push_str("- ");
            body.push_str(s);
            body.push('\n');
        }
    }

    if !handoff.decisions.is_empty() {
        body.push_str("\n### Decisions\n");
        for d in &handoff.decisions {
            body.push_str("- ");
            body.push_str(d);
            body.push('\n');
        }
    }

    delimiters::wrap_rag_context("handoff", &body)
}

/// Session open hook: check for pending handoff and return injection if available.
///
/// Returns `None` if no handoff exists or it's already consumed.
/// If a pending handoff is found, formats it for injection and marks it as consumed.
pub fn on_session_open(db: &KiraDb) -> Option<String> {
    let handoff = get_handoff(db)?;

    if handoff.consumed {
        debug!("handoff already consumed — skipping injection");
        return None;
    }

    // Format the injection string before marking consumed.
    let injection = format_handoff_injection(&handoff);

    // Mark as consumed so it won't be re-injected on reconnection.
    if let Err(e) = mark_consumed(db) {
        warn!(error = %e, "failed to mark handoff as consumed — proceeding with injection");
    }

    info!(
        session_id = %handoff.session_id,
        "injecting handoff from previous session"
    );

    Some(injection)
}

/// Session close hook: determine whether this disconnect should trigger
/// background tasks (handoff generation + working→episodic consolidation).
///
/// Returns `true` if this is a real session close (should spawn background tasks).
/// Returns `false` if this is a reconnect within the 30-second debounce window.
///
/// Updates `last_disconnect_at` in the handoff record regardless.
pub fn should_process_disconnect(db: &KiraDb) -> bool {
    let now = Utc::now();

    // Read the current handoff to check last_disconnect_at.
    let handoff = get_handoff(db);

    // Check debounce: if last disconnect was within 30 seconds, this is a reconnect.
    if let Some(HandoffSummary { last_disconnect_at: Some(last_disconnect), .. }) = handoff.as_ref() {
        let elapsed = now.signed_duration_since(*last_disconnect);
        if elapsed.num_seconds() < RECONNECT_DEBOUNCE_SECS {
            debug!(
                elapsed_secs = elapsed.num_seconds(),
                debounce_secs = RECONNECT_DEBOUNCE_SECS,
                "SSE reconnect within debounce window — skipping session close tasks"
            );
            // Still update the disconnect timestamp for the next check.
            update_disconnect_at(db, handoff, now);
            return false;
        }
    }

    // Real disconnect — update timestamp and signal that background tasks should run.
    update_disconnect_at(db, handoff, now);
    true
}

/// Update the `last_disconnect_at` field in the handoff record.
/// If no handoff exists, creates a minimal placeholder record to track disconnect time.
fn update_disconnect_at(db: &KiraDb, handoff: Option<HandoffSummary>, now: DateTime<Utc>) {
    let mut summary = handoff.unwrap_or_else(|| HandoffSummary {
        session_id: Uuid::nil(),
        context: String::new(),
        open_questions: Vec::new(),
        next_steps: Vec::new(),
        decisions: Vec::new(),
        generated_at: now,
        consumed: true,
        last_disconnect_at: None,
    });

    summary.last_disconnect_at = Some(now);

    if let Err(e) = db.put(TBL_HANDOFF, HANDOFF_KEY, &summary) {
        warn!(error = %e, "failed to update last_disconnect_at in redb");
    }
}

// ─── LLM Response Schema (internal) ─────────────────────────────────────────

/// Raw JSON response from the LLM for handoff generation.
/// Must contain exactly four required fields.
#[derive(Debug, Clone, Deserialize)]
struct HandoffLlmResponse {
    context: String,
    open_questions: Vec<String>,
    next_steps: Vec<String>,
    decisions: Vec<String>,
}

// ─── Generation ──────────────────────────────────────────────────────────────

/// Generate a handoff summary from session history.
///
/// Loads the prompt from `core/agents/prompts/session_handoff.md` at each
/// invocation (no caching). Invokes the light-tier LLM with the session
/// messages (user messages only, max 50). Parses the structured JSON response,
/// applies truncation if needed, persists to redb, and emits a
/// `HandoffGenerated` event.
///
/// On any error (missing prompt, malformed JSON, LLM timeout, redb failure),
/// the function logs a warning and returns Ok(()) — leaving the previous
/// handoff intact. No retry is attempted.
pub async fn generate_handoff(
    session_id: Uuid,
    messages: Vec<ChatMessage>,
    config: &MaturationConfig,
    router: &LlmRouter,
    pillar_tx: &PillarTx,
    db: &KiraDb,
) -> Result<()> {
    // Gate: skip handoff generation when maturation is disabled.
    if !config.enabled {
        debug!(
            session_id = %session_id,
            "maturation disabled — skipping handoff generation"
        );
        return Ok(());
    }

    // Gate: skip if light-tier LLM is unavailable (circuit breaker open).
    if !router.is_tier_available(Tier::Light) {
        warn!(
            session_id = %session_id,
            "light-tier LLM unavailable (circuit breaker open) — skipping handoff generation"
        );
        return Ok(());
    }

    // 1. Load prompt from disk (no caching).
    let prompt_path = "core/agents/prompts/session_handoff.md";
    let prompt_content = match tokio::fs::read_to_string(prompt_path).await {
        Ok(content) => content,
        Err(e) => {
            warn!(
                path = prompt_path,
                error = %e,
                "handoff prompt file missing or unreadable — skipping generation"
            );
            return Ok(());
        }
    };

    // 2. Filter to user messages only, take the most recent 50.
    let user_messages: Vec<&ChatMessage> = messages
        .iter()
        .filter(|m| m.role == "user")
        .collect();
    let recent_messages: Vec<&ChatMessage> = if user_messages.len() > 50 {
        user_messages[user_messages.len() - 50..].to_vec()
    } else {
        user_messages
    };

    // 3. Format messages as context for the LLM.
    let messages_context = recent_messages
        .iter()
        .enumerate()
        .map(|(i, m)| format!("[Message {}]: {}", i + 1, m.content))
        .collect::<Vec<_>>()
        .join("\n\n");

    // 4. Build the LLM request: system message (prompt) + user message (session context).
    let llm_messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: prompt_content,
        },
        ChatMessage {
            role: "user".to_string(),
            content: format!(
                "Generate a handoff summary for the following session messages:\n\n{messages_context}"
            ),
        },
    ];

    let req = ChatRequest {
        tier: "light".to_string(),
        model: None,
        messages: llm_messages.clone(),
        mode: None,
    };

    // 5. Invoke LLM with 60-second timeout.
    let llm_result = tokio::time::timeout(
        std::time::Duration::from_mins(1),
        router.route_chat(&req, &llm_messages),
    )
    .await;

    let output = match llm_result {
        Ok(Ok((output, _provider, _model))) => output,
        Ok(Err(e)) => {
            warn!(
                session_id = %session_id,
                error = %e,
                "LLM call failed during handoff generation — skipping"
            );
            return Ok(());
        }
        Err(_) => {
            warn!(
                session_id = %session_id,
                "LLM call timed out (>60s) during handoff generation — skipping"
            );
            return Ok(());
        }
    };

    // 6. Parse the JSON response.
    let raw_response: HandoffLlmResponse = match serde_json::from_str(&output.content) {
        Ok(parsed) => parsed,
        Err(e) => {
            warn!(
                session_id = %session_id,
                error = %e,
                response = %output.content,
                "malformed JSON from LLM during handoff generation — skipping"
            );
            return Ok(());
        }
    };

    // 7. Build the HandoffSummary.
    let mut summary = HandoffSummary {
        session_id,
        context: raw_response.context,
        open_questions: raw_response.open_questions,
        next_steps: raw_response.next_steps,
        decisions: raw_response.decisions,
        generated_at: Utc::now(),
        consumed: false,
        last_disconnect_at: None,
    };

    // 8. Apply truncation if over max_handoff_chars.
    truncate_proportional(&mut summary, config.max_handoff_chars as usize);

    // 9. Persist to redb (overwriting previous).
    if let Err(e) = db.put(TBL_HANDOFF, HANDOFF_KEY, &summary) {
        error!(
            session_id = %session_id,
            error = %e,
            "failed to persist handoff to redb"
        );
        return Ok(());
    }

    // 10. Emit HandoffGenerated event.
    let summary_length = summary.context.len()
        + summary.open_questions.iter().map(String::len).sum::<usize>()
        + summary.next_steps.iter().map(String::len).sum::<usize>()
        + summary.decisions.iter().map(String::len).sum::<usize>();

    super::maturation_events::emit_handoff_generated(pillar_tx, session_id, summary_length);

    Ok(())
}

// ─── Spawn with Panic Guard ─────────────────────────────────────────────────

/// Spawn handoff generation as a background task with JoinHandle panic catching.
///
/// If the task panics, catches the panic via JoinHandle, logs an error, and
/// emits a `HandoffFailed` event. The previous handoff remains intact and all
/// API endpoints continue serving without interruption.
///
/// This is the recommended entry point for spawning handoff generation from
/// session lifecycle hooks.
pub fn spawn_handoff_with_panic_guard(
    session_id: Uuid,
    messages: Vec<ChatMessage>,
    config: MaturationConfig,
    router: std::sync::Arc<LlmRouter>,
    pillar_tx: PillarTx,
    db: KiraDb,
) {
    let pillar_tx_clone = pillar_tx.clone();
    tokio::spawn(async move {
        let handle = tokio::spawn(async move {
            generate_handoff(
                session_id,
                messages,
                &config,
                &router,
                &pillar_tx,
                &db,
            )
            .await
        });

        match handle.await {
            Ok(Ok(())) => {
                // Handoff generation completed normally.
            }
            Ok(Err(e)) => {
                // generate_handoff returned an error (already logged internally).
                warn!(
                    session_id = %session_id,
                    error = %e,
                    "handoff generation returned error"
                );
            }
            Err(join_err) => {
                // Task panicked or was cancelled — emit HandoffFailed event.
                let panic_msg = if join_err.is_panic() {
                    let panic_payload = join_err.into_panic();
                    if let Some(s) = panic_payload.downcast_ref::<&str>() {
                        format!("task panicked: {s}")
                    } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                        format!("task panicked: {s}")
                    } else {
                        "task panicked (unknown payload)".to_string()
                    }
                } else {
                    "task cancelled".to_string()
                };

                error!(
                    session_id = %session_id,
                    error = %panic_msg,
                    "handoff generation task panicked"
                );
                super::maturation_events::emit_handoff_failed(
                    &pillar_tx_clone,
                    session_id,
                    &panic_msg,
                );
            }
        }
    });
}

/// Truncate a handoff summary to fit within `max_chars`, distributing cuts
/// proportionally across all four sections based on their share of the total
/// pre-truncation length.
///
/// Algorithm:
/// 1. Calculate total length of all sections.
/// 2. If total <= max_chars, return unchanged.
/// 3. For each section, calculate its proportion of the total.
/// 4. Allocate max_chars proportionally (±1 char for rounding).
/// 5. Truncate each section to its allocated budget.
pub fn truncate_proportional(summary: &mut HandoffSummary, max_chars: usize) {
    let context_len = summary.context.len();
    let questions_len: usize = summary.open_questions.iter().map(String::len).sum();
    let steps_len: usize = summary.next_steps.iter().map(String::len).sum();
    let decisions_len: usize = summary.decisions.iter().map(String::len).sum();

    let total = context_len + questions_len + steps_len + decisions_len;

    if total <= max_chars {
        return;
    }

    // Allocate budget proportionally to each section.
    let budget_context = proportional_budget(context_len, total, max_chars);
    let budget_questions = proportional_budget(questions_len, total, max_chars);
    let budget_steps = proportional_budget(steps_len, total, max_chars);
    // Give decisions the remainder to avoid off-by-one from rounding.
    let budget_decisions = max_chars
        .saturating_sub(budget_context)
        .saturating_sub(budget_questions)
        .saturating_sub(budget_steps);

    // Truncate each section to its budget.
    truncate_string(&mut summary.context, budget_context);
    truncate_vec(&mut summary.open_questions, budget_questions);
    truncate_vec(&mut summary.next_steps, budget_steps);
    truncate_vec(&mut summary.decisions, budget_decisions);
}

/// Calculate the proportional budget for a section.
const fn proportional_budget(section_len: usize, total: usize, max_chars: usize) -> usize {
    if total == 0 {
        return 0;
    }
    // Use u128 to avoid overflow on large values.
    ((section_len as u128 * max_chars as u128) / total as u128) as usize
}

/// Truncate a string to at most `budget` bytes, respecting char boundaries.
fn truncate_string(s: &mut String, budget: usize) {
    if s.len() <= budget {
        return;
    }
    // Find the largest char boundary <= budget.
    let mut end = budget;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s.truncate(end);
}

/// Truncate a vec of strings to fit within a total `budget` of characters.
/// Distributes the budget evenly across items, then truncates each.
/// Removes items that get zero budget.
fn truncate_vec(items: &mut Vec<String>, budget: usize) {
    if items.is_empty() {
        return;
    }

    let current_total: usize = items.iter().map(String::len).sum();
    if current_total <= budget {
        return;
    }

    if budget == 0 {
        items.clear();
        return;
    }

    // Distribute budget proportionally across items based on their length.
    let mut budgets: Vec<usize> = items
        .iter()
        .map(|item| proportional_budget(item.len(), current_total, budget))
        .collect();

    // Assign remainder to the last item to avoid off-by-one.
    let allocated: usize = budgets.iter().sum();
    if allocated < budget && !budgets.is_empty() {
        let last = budgets.len() - 1;
        budgets[last] += budget - allocated;
    }

    // Truncate each item to its budget.
    for (item, &item_budget) in items.iter_mut().zip(budgets.iter()) {
        truncate_string(item, item_budget);
    }

    // Remove empty items.
    items.retain(|s| !s.is_empty());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_summary(context: &str, questions: &[&str], steps: &[&str], decisions: &[&str]) -> HandoffSummary {
        HandoffSummary {
            session_id: Uuid::now_v7(),
            context: context.to_string(),
            open_questions: questions.iter().map(|s| s.to_string()).collect(),
            next_steps: steps.iter().map(|s| s.to_string()).collect(),
            decisions: decisions.iter().map(|s| s.to_string()).collect(),
            generated_at: Utc::now(),
            consumed: false,
            last_disconnect_at: None,
        }
    }

    fn total_len(summary: &HandoffSummary) -> usize {
        summary.context.len()
            + summary.open_questions.iter().map(String::len).sum::<usize>()
            + summary.next_steps.iter().map(String::len).sum::<usize>()
            + summary.decisions.iter().map(String::len).sum::<usize>()
    }

    #[test]
    fn truncate_noop_when_under_limit() {
        let mut summary = make_summary("hello", &["q1"], &["s1"], &["d1"]);
        let before = summary.clone();
        truncate_proportional(&mut summary, 1000);
        assert_eq!(summary.context, before.context);
        assert_eq!(summary.open_questions, before.open_questions);
        assert_eq!(summary.next_steps, before.next_steps);
        assert_eq!(summary.decisions, before.decisions);
    }

    #[test]
    fn truncate_respects_max_chars() {
        let mut summary = make_summary(
            &"a".repeat(500),
            &["b".repeat(200).as_str(), "c".repeat(200).as_str()],
            &["d".repeat(300).as_str()],
            &["e".repeat(300).as_str()],
        );
        let max = 600;
        truncate_proportional(&mut summary, max);
        assert!(total_len(&summary) <= max);
    }

    #[test]
    fn truncate_proportional_distribution() {
        // Context is 60% of total, questions 20%, steps 10%, decisions 10%.
        let mut summary = make_summary(
            &"x".repeat(600),
            &["y".repeat(200).as_str()],
            &["z".repeat(100).as_str()],
            &["w".repeat(100).as_str()],
        );
        // Total = 1000, truncate to 500.
        truncate_proportional(&mut summary, 500);

        let total = total_len(&summary);
        assert!(total <= 500, "total {total} exceeds 500");

        // Context should get roughly 60% of 500 = 300 (±1).
        let ctx_len = summary.context.len();
        assert!(
            (295..=305).contains(&ctx_len),
            "context len {ctx_len} not proportional (expected ~300)"
        );
    }

    #[test]
    fn truncate_empty_sections() {
        let mut summary = make_summary(&"a".repeat(100), &[], &[], &[]);
        truncate_proportional(&mut summary, 50);
        assert!(total_len(&summary) <= 50);
        assert_eq!(summary.context.len(), 50);
    }

    #[test]
    fn truncate_all_empty() {
        let mut summary = make_summary("", &[], &[], &[]);
        truncate_proportional(&mut summary, 10);
        assert_eq!(total_len(&summary), 0);
    }

    #[test]
    fn truncate_respects_char_boundaries() {
        // Use multi-byte chars (each '日' is 3 bytes).
        let mut summary = make_summary(
            &"日".repeat(100), // 300 bytes
            &[],
            &[],
            &[],
        );
        truncate_proportional(&mut summary, 150);
        // Should truncate to a valid char boundary.
        assert!(summary.context.is_char_boundary(summary.context.len()));
        assert!(summary.context.len() <= 150);
    }

    #[test]
    fn get_handoff_returns_none_when_empty() {
        let db = KiraDb::open_temp().unwrap();
        assert!(get_handoff(&db).is_none());
    }

    #[test]
    fn put_and_get_handoff_roundtrip() {
        let db = KiraDb::open_temp().unwrap();
        let summary = make_summary("test context", &["q1"], &["s1"], &["d1"]);
        db.put(TBL_HANDOFF, "current", &summary).unwrap();
        let loaded = get_handoff(&db).unwrap();
        assert_eq!(loaded.context, "test context");
        assert_eq!(loaded.open_questions, vec!["q1"]);
        assert!(!loaded.consumed);
    }

    #[test]
    fn mark_consumed_sets_flag() {
        let db = KiraDb::open_temp().unwrap();
        let summary = make_summary("ctx", &[], &[], &[]);
        db.put(TBL_HANDOFF, "current", &summary).unwrap();
        mark_consumed(&db).unwrap();
        let loaded = get_handoff(&db).unwrap();
        assert!(loaded.consumed);
    }

    #[test]
    fn mark_consumed_fails_when_no_handoff() {
        let db = KiraDb::open_temp().unwrap();
        assert!(mark_consumed(&db).is_err());
    }

    #[test]
    fn delete_handoff_returns_true_when_exists() {
        let db = KiraDb::open_temp().unwrap();
        let summary = make_summary("ctx", &[], &[], &[]);
        db.put(TBL_HANDOFF, "current", &summary).unwrap();
        assert!(delete_handoff(&db).unwrap());
        assert!(get_handoff(&db).is_none());
    }

    #[test]
    fn delete_handoff_returns_false_when_empty() {
        let db = KiraDb::open_temp().unwrap();
        assert!(!delete_handoff(&db).unwrap());
    }

    #[test]
    fn corrupted_handoff_returns_none() {
        let db = KiraDb::open_temp().unwrap();
        db.put_raw(TBL_HANDOFF, "current", "not valid json {{{").unwrap();
        assert!(get_handoff(&db).is_none());
    }

    // ─── format_handoff_injection tests ──────────────────────────────────────

    #[test]
    fn format_handoff_injection_wraps_in_rag_context() {
        let summary = make_summary("working on auth module", &["q1"], &["s1"], &["d1"]);
        let result = format_handoff_injection(&summary);
        assert!(result.starts_with("<RAG_CONTEXT source=\"handoff\" untrusted=\"true\">"));
        assert!(result.ends_with("</RAG_CONTEXT>"));
    }

    #[test]
    fn format_handoff_injection_includes_all_sections() {
        let summary = make_summary(
            "implementing login flow",
            &["how to handle OAuth?", "rate limiting?"],
            &["add tests", "deploy to staging"],
            &["use JWT tokens", "no session cookies"],
        );
        let result = format_handoff_injection(&summary);
        assert!(result.contains("### Context"));
        assert!(result.contains("implementing login flow"));
        assert!(result.contains("### Open Questions"));
        assert!(result.contains("- how to handle OAuth?"));
        assert!(result.contains("- rate limiting?"));
        assert!(result.contains("### Next Steps"));
        assert!(result.contains("- add tests"));
        assert!(result.contains("- deploy to staging"));
        assert!(result.contains("### Decisions"));
        assert!(result.contains("- use JWT tokens"));
        assert!(result.contains("- no session cookies"));
    }

    #[test]
    fn format_handoff_injection_omits_empty_sections() {
        let summary = make_summary("just context", &[], &[], &[]);
        let result = format_handoff_injection(&summary);
        assert!(result.contains("### Context"));
        assert!(result.contains("just context"));
        assert!(!result.contains("### Open Questions"));
        assert!(!result.contains("### Next Steps"));
        assert!(!result.contains("### Decisions"));
    }

    #[test]
    fn format_handoff_injection_neutralizes_dangerous_closers() {
        let summary = make_summary(
            "context with </RAG_CONTEXT> injection attempt",
            &[],
            &[],
            &[],
        );
        let result = format_handoff_injection(&summary);
        // The dangerous closer inside content should be neutralized.
        // Only the outer closing tag should be a real </RAG_CONTEXT>.
        let count = result.matches("</RAG_CONTEXT>").count();
        assert_eq!(count, 1, "only the outer closing tag should be real");
    }

    // ─── on_session_open tests ───────────────────────────────────────────────

    #[test]
    fn on_session_open_returns_none_when_no_handoff() {
        let db = KiraDb::open_temp().unwrap();
        assert!(on_session_open(&db).is_none());
    }

    #[test]
    fn on_session_open_returns_injection_for_pending_handoff() {
        let db = KiraDb::open_temp().unwrap();
        let summary = make_summary("previous session context", &["q1"], &["s1"], &["d1"]);
        db.put(TBL_HANDOFF, "current", &summary).unwrap();

        let result = on_session_open(&db);
        assert!(result.is_some());
        let injection = result.unwrap();
        assert!(injection.contains("previous session context"));
        assert!(injection.contains("<RAG_CONTEXT source=\"handoff\" untrusted=\"true\">"));
    }

    #[test]
    fn on_session_open_marks_handoff_consumed() {
        let db = KiraDb::open_temp().unwrap();
        let summary = make_summary("ctx", &[], &[], &[]);
        db.put(TBL_HANDOFF, "current", &summary).unwrap();

        on_session_open(&db);

        let loaded = get_handoff(&db).unwrap();
        assert!(loaded.consumed);
    }

    #[test]
    fn on_session_open_returns_none_for_consumed_handoff() {
        let db = KiraDb::open_temp().unwrap();
        let mut summary = make_summary("ctx", &[], &[], &[]);
        summary.consumed = true;
        db.put(TBL_HANDOFF, "current", &summary).unwrap();

        assert!(on_session_open(&db).is_none());
    }

    #[test]
    fn on_session_open_does_not_reinject_on_second_call() {
        let db = KiraDb::open_temp().unwrap();
        let summary = make_summary("ctx", &["q1"], &[], &[]);
        db.put(TBL_HANDOFF, "current", &summary).unwrap();

        // First call: inject.
        assert!(on_session_open(&db).is_some());
        // Second call: already consumed.
        assert!(on_session_open(&db).is_none());
    }

    // ─── should_process_disconnect tests ─────────────────────────────────────

    #[test]
    fn should_process_disconnect_returns_true_on_first_disconnect() {
        let db = KiraDb::open_temp().unwrap();
        assert!(should_process_disconnect(&db));
    }

    #[test]
    fn should_process_disconnect_returns_true_when_no_recent_disconnect() {
        let db = KiraDb::open_temp().unwrap();
        // Set last_disconnect_at to 60 seconds ago (beyond debounce window).
        let mut summary = make_summary("ctx", &[], &[], &[]);
        summary.last_disconnect_at = Some(Utc::now() - chrono::Duration::seconds(60));
        db.put(TBL_HANDOFF, "current", &summary).unwrap();

        assert!(should_process_disconnect(&db));
    }

    #[test]
    fn should_process_disconnect_returns_false_within_debounce_window() {
        let db = KiraDb::open_temp().unwrap();
        // Set last_disconnect_at to 5 seconds ago (within debounce window).
        let mut summary = make_summary("ctx", &[], &[], &[]);
        summary.last_disconnect_at = Some(Utc::now() - chrono::Duration::seconds(5));
        db.put(TBL_HANDOFF, "current", &summary).unwrap();

        assert!(!should_process_disconnect(&db));
    }

    #[test]
    fn should_process_disconnect_updates_last_disconnect_at() {
        let db = KiraDb::open_temp().unwrap();
        let before = Utc::now();

        should_process_disconnect(&db);

        let loaded = get_handoff(&db).unwrap();
        assert!(loaded.last_disconnect_at.is_some());
        let disconnect_at = loaded.last_disconnect_at.unwrap();
        assert!(disconnect_at >= before);
    }

    #[test]
    fn should_process_disconnect_debounce_boundary_at_30s() {
        let db = KiraDb::open_temp().unwrap();
        // Exactly at 30 seconds — should process (not within window).
        let mut summary = make_summary("ctx", &[], &[], &[]);
        summary.last_disconnect_at = Some(Utc::now() - chrono::Duration::seconds(30));
        db.put(TBL_HANDOFF, "current", &summary).unwrap();

        assert!(should_process_disconnect(&db));
    }
}
