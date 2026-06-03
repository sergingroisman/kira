#![forbid(unsafe_code)]

//! Slash-command dispatcher — split into sync (local) and async (remote).
//!
//! Local commands (`/mode`, `/tier`, `/help`, `/quit`, `/agents`) are handled
//! synchronously and return a `DispatchResult` immediately.
//!
//! Remote commands (`/queue`, `/queue cancel <id>`, `/memory search <q>`,
//! `/memory <run_id>`, `/cancel <prefix>`, `/lessons`) spawn an async task
//! that calls `CoreClient` and sends results via the mpsc channel.

pub mod mode;
pub mod tier;

use std::fmt::Write as _;
use tokio::sync::mpsc;

use crate::app::{AppState, OutputLineKind};
use crate::core_client::CoreClient;
use crate::messages::UiMessage;

/// All recognized slash-commands.
pub const COMMANDS: &[&str] = &[
    "/mode",
    "/tier",
    "/agent",
    "/agents",
    "/cancel",
    "/plan",
    "/plans",
    "/approve",
    "/layers",
    "/lessons",
    "/worktrees",
    "/detective",
    "/workflow",
    "/workflows",
    "/waves",
    "/queue",
    "/memory",
    "/help",
    "/quit",
];

/// Result of dispatching a slash-command synchronously.
#[derive(Debug, PartialEq, Eq)]
pub enum DispatchResult {
    /// Command executed locally with optional output message.
    Immediate(Option<String>),
    /// Quit requested.
    Quit,
    /// Command not recognized.
    Unknown(String),
    /// Error parsing or executing command.
    Error(String),
    /// Command dispatched asynchronously (result will arrive via channel).
    Async,
}

/// Parse and dispatch a slash-command synchronously.
///
/// Handles local commands (`/mode`, `/tier`, `/help`, `/quit`, `/agents`).
/// Returns `DispatchResult::Async` for commands that need Core access —
/// the caller should then call `dispatch_async`.
///
/// `input` must start with `/`.
pub fn dispatch_sync(input: &str, state: &mut AppState) -> DispatchResult {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return DispatchResult::Unknown(trimmed.to_string());
    }

    let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
    let cmd = parts[0].to_lowercase();
    let args = parts.get(1).unwrap_or(&"").trim();

    match cmd.as_str() {
        "/quit" => DispatchResult::Quit,
        "/help" => DispatchResult::Immediate(Some(help_text())),
        "/mode" => {
            let result = mode::execute(args);
            // Apply mode change to state if successful
            if let DispatchResult::Immediate(Some(_)) = &result {
                match args.to_lowercase().as_str() {
                    "yolo" => state.mode = crate::app::Mode::Yolo,
                    "plan" => state.mode = crate::app::Mode::Plan,
                    _ => {}
                }
            }
            result
        }
        "/tier" => {
            let result = tier::execute(args);
            // Apply tier change to state if successful
            if let DispatchResult::Immediate(Some(_)) = &result
                && let Ok(new_tier) = args.parse::<crate::app::Tier>()
            {
                state.tier = new_tier;
            }
            result
        }
        "/layers" => {
            toggle_layers(state);
            DispatchResult::Immediate(None)
        }
        "/agents" | "/agent" => {
            let agents = &state.active_agents;
            if agents.is_empty() {
                DispatchResult::Immediate(Some("No active agents.".to_string()))
            } else {
                use std::fmt::Write;
                let mut out = String::from("Active agents:\n");
                for agent in agents {
                    let last = agent.last_output.as_deref().unwrap_or("(no output yet)");
                    let _ = writeln!(
                        out,
                        "  [{short_id}] status={status} | {last}",
                        short_id = agent.short_id,
                        status = agent.status,
                        last = last,
                    );
                }
                DispatchResult::Immediate(Some(out))
            }
        }
        // Remote commands — signal that async dispatch is needed
        "/queue" | "/memory" | "/cancel" | "/lessons" | "/detective" | "/workflows" => DispatchResult::Async,
        // Other known commands that aren't yet wired
        "/plan" | "/plans" => {
            DispatchResult::Immediate(Some("plans view: use /plans to list".to_string()))
        }
        "/approve" => DispatchResult::Immediate(Some("approve: pending plan approved".to_string())),
        "/worktrees" => DispatchResult::Immediate(Some("worktrees: fetching list...".to_string())),
        "/workflow" => {
            if args.is_empty() {
                DispatchResult::Error("usage: /workflow <goal> [--wf sdd|gsd2]".to_string())
            } else {
                // Intentionally same as the remote commands arm — needs validation first
                DispatchResult::Async
            }
        }
        "/waves" => DispatchResult::Immediate(Some("waves: fetching wave plan...".to_string())),
        _ => DispatchResult::Unknown(cmd),
    }
}

/// Dispatch a remote command asynchronously.
///
/// Spawns a tokio task that calls the appropriate `CoreClient` method and
/// sends the result back via the channel as `UiMessage::CommandResult` or
/// `UiMessage::CommandError`.
///
/// For `/cancel <prefix>`, the prefix matching is done against `active_agents`
/// passed in. This function should only be called for commands that returned
/// `DispatchResult::Async` from `dispatch_sync`.
pub fn dispatch_async(
    input: &str,
    state: &AppState,
    client: CoreClient,
    tx: mpsc::Sender<UiMessage>,
) {
    let trimmed = input.trim();
    let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
    let cmd = parts[0].to_lowercase();
    let args = parts.get(1).unwrap_or(&"").trim().to_string();

    match cmd.as_str() {
        "/queue" => {
            if args.starts_with("cancel ") {
                let id = args
                    .strip_prefix("cancel ")
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if id.is_empty() {
                    tokio::spawn(async move {
                        let _ = tx
                            .send(UiMessage::CommandError {
                                message: "usage: /queue cancel <id>".to_string(),
                            })
                            .await;
                    });
                } else {
                    tokio::spawn(async move {
                        match client.queue_cancel(&id).await {
                            Ok(()) => {
                                let _ = tx
                                    .send(UiMessage::CommandResult {
                                        output: format!("Queue entry '{id}' cancelled."),
                                        kind: OutputLineKind::System,
                                    })
                                    .await;
                            }
                            Err(e) => {
                                let _ = tx
                                    .send(UiMessage::CommandError {
                                        message: format!("queue cancel failed: {e}"),
                                    })
                                    .await;
                            }
                        }
                    });
                }
            } else {
                // /queue — list queue entries
                tokio::spawn(async move {
                    match client.queue_list().await {
                        Ok(entries) => {
                            let output = if entries.is_empty() {
                                "Queue is empty.".to_string()
                            } else {
                                use std::fmt::Write;
                                let mut out = format!("Queue ({} entries):\n", entries.len());
                                for entry in entries.iter().take(200) {
                                    let _ = writeln!(
                                        out,
                                        "  [{id}] {status} — {preview}",
                                        id = entry.id,
                                        status = entry.status,
                                        preview = entry.message_preview,
                                    );
                                }
                                if entries.len() > 200 {
                                    let _ = writeln!(out, "  ... and {} more", entries.len() - 200);
                                }
                                out
                            };
                            let _ = tx
                                .send(UiMessage::CommandResult {
                                    output,
                                    kind: OutputLineKind::System,
                                })
                                .await;
                        }
                        Err(e) => {
                            let _ = tx
                                .send(UiMessage::CommandError {
                                    message: format!("queue list failed: {e}"),
                                })
                                .await;
                        }
                    }
                });
            }
        }
        "/memory" => {
            if args.starts_with("search ") {
                let query = args
                    .strip_prefix("search ")
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if query.is_empty() {
                    tokio::spawn(async move {
                        let _ = tx
                            .send(UiMessage::CommandError {
                                message: "usage: /memory search <query>".to_string(),
                            })
                            .await;
                    });
                } else {
                    tokio::spawn(async move {
                        match client.memory_search(&query).await {
                            Ok(fragments) => {
                                let output = if fragments.is_empty() {
                                    "No memory fragments found.".to_string()
                                } else {
                                    use std::fmt::Write;
                                    let mut out =
                                        format!("Memory search ({} results):\n", fragments.len());
                                    for frag in fragments.iter().take(50) {
                                        let _ = writeln!(
                                            out,
                                            "  [{id}] (sim={sim:.2}) {content}",
                                            id = frag.id,
                                            sim = frag.similarity,
                                            content = truncate(&frag.content, 120),
                                        );
                                    }
                                    if fragments.len() > 50 {
                                        let _ = writeln!(
                                            out,
                                            "  ... and {} more",
                                            fragments.len() - 50
                                        );
                                    }
                                    out
                                };
                                let _ = tx
                                    .send(UiMessage::CommandResult {
                                        output,
                                        kind: OutputLineKind::System,
                                    })
                                    .await;
                            }
                            Err(e) => {
                                let _ = tx
                                    .send(UiMessage::CommandError {
                                        message: format!("memory search failed: {e}"),
                                    })
                                    .await;
                            }
                        }
                    });
                }
            } else if args.is_empty() {
                tokio::spawn(async move {
                    let _ = tx
                        .send(UiMessage::CommandError {
                            message: "usage: /memory <run-id> or /memory search <query>"
                                .to_string(),
                        })
                        .await;
                });
            } else {
                // /memory <run_id>
                let run_id = args;
                tokio::spawn(async move {
                    match client.memory_context(&run_id).await {
                        Ok(fragments) => {
                            let output = if fragments.is_empty() {
                                format!("No context fragments for run '{run_id}'.")
                            } else {
                                use std::fmt::Write;
                                let mut out = format!(
                                    "Memory context for '{run_id}' ({} fragments):\n",
                                    fragments.len()
                                );
                                for frag in &fragments {
                                    let _ = writeln!(
                                        out,
                                        "  [{id}] {content}",
                                        id = frag.id,
                                        content = truncate(&frag.content, 120),
                                    );
                                }
                                out
                            };
                            let _ = tx
                                .send(UiMessage::CommandResult {
                                    output,
                                    kind: OutputLineKind::System,
                                })
                                .await;
                        }
                        Err(e) => {
                            let _ = tx
                                .send(UiMessage::CommandError {
                                    message: format!("memory context failed: {e}"),
                                })
                                .await;
                        }
                    }
                });
            }
        }
        "/cancel" => {
            if args.is_empty() {
                tokio::spawn(async move {
                    let _ = tx
                        .send(UiMessage::CommandError {
                            message: "usage: /cancel <agent-id-prefix>".to_string(),
                        })
                        .await;
                });
                return;
            }

            // Prefix matching against active agents using extracted helper
            match find_by_prefix(&state.active_agents, &args) {
                PrefixMatch::NotFound => {
                    let msg = format!("No active agent matches prefix '{args}'.");
                    tokio::spawn(async move {
                        let _ = tx.send(UiMessage::CommandError { message: msg }).await;
                    });
                }
                PrefixMatch::Found(agent_id) => {
                    let short_id: String = agent_id.chars().take(8).collect();
                    tokio::spawn(async move {
                        match client.agent_cancel(&agent_id).await {
                            Ok(()) => {
                                let _ = tx
                                    .send(UiMessage::CommandResult {
                                        output: format!("Cancel sent to agent {short_id}."),
                                        kind: OutputLineKind::System,
                                    })
                                    .await;
                            }
                            Err(e) => {
                                let _ = tx
                                    .send(UiMessage::CommandError {
                                        message: format!("cancel failed: {e}"),
                                    })
                                    .await;
                            }
                        }
                    });
                }
                PrefixMatch::Ambiguous(n) => {
                    let msg =
                        format!("Ambiguous prefix '{args}' matches {n} agents. Be more specific.");
                    tokio::spawn(async move {
                        let _ = tx.send(UiMessage::CommandError { message: msg }).await;
                    });
                }
            }
        }
        "/workflows" => {
            tokio::spawn(async move {
                match client.workflow_ids().await {
                    Ok(ids) => {
                        let output = if ids.is_empty() {
                            "no workflows loaded".to_string()
                        } else {
                            format!("workflows: {}", ids.join(", "))
                        };
                        let _ = tx.send(UiMessage::CommandResult { output, kind: OutputLineKind::System }).await;
                    }
                    Err(e) => {
                        let _ = tx.send(UiMessage::CommandError { message: format!("workflows: {e}") }).await;
                    }
                }
            });
        }
        "/workflow" => {
            // args = "<goal>" or "<goal> --wf <id>"
            #[allow(clippy::option_if_let_else)]
            let (goal, workflow) = if let Some(rest) = args.strip_suffix(" --wf gsd2") {
                (rest.trim().to_string(), "gsd2".to_string())
            } else if let Some(rest) = args.strip_suffix(" --wf sdd") {
                (rest.trim().to_string(), "sdd".to_string())
            } else {
                (args, "sdd".to_string())
            };
            tokio::spawn(async move {
                match client.workflow_start(&goal, &workflow, None, false).await {
                    Ok(run) => {
                        let output = format!(
                            "▸ workflow iniciado: {} | wf={} | fase={}",
                            run.id.chars().take(8).collect::<String>(),
                            run.workflow_id,
                            run.current_phase_id
                        );
                        let _ = tx.send(UiMessage::CommandResult { output, kind: OutputLineKind::System }).await;

                        // Poll the run for progress (phases, questions, done)
                        // until terminal. Wave agents' own SSE events show the
                        // per-task activity chips/approvals separately.
                        let run_id = run.id.clone();
                        loop {
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                            match client.workflow_get(&run_id).await {
                                Ok(view) => {
                                    let done = view.finished_at.is_some()
                                        || view.current_phase_id == "done"
                                        || view.current_phase_id == "failed";
                                    let _ = tx.send(UiMessage::WorkflowUpdate(view)).await;
                                    if done {
                                        break;
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(UiMessage::CommandError { message: format!("workflow: {e}") }).await;
                    }
                }
            });
        }
        "/detective" => {
            // args = optional extra flags (future: --spec <path>)
            tokio::spawn(async move {
                let _ = tx.send(UiMessage::CommandResult {
                    output: "detective: running 6-lens review (this may take a minute)…".to_string(),
                    kind: OutputLineKind::System,
                }).await;
                match client.detective_run(None, None, None, None).await {
                    Ok(result) => {
                        let icon = match result.verdict.as_str() {
                            "pass"  => "✅",
                            "warn"  => "⚠️ ",
                            "block" => "🚨",
                            _       => "❓",
                        };
                        let mut output = format!(
                            "{icon} detective: {} — {} ({} findings)\n",
                            result.verdict.to_uppercase(),
                            result.verdict_reason,
                            result.summary.total_findings
                        );
                        for f in result.findings.iter().take(5) {
                            let loc = f.path.as_deref().unwrap_or("?");
                            let _ = writeln!(output, "  [{sev}] {loc} — {title}",
                                sev = f.severity, title = f.title);
                        }
                        if result.summary.total_findings > 5 {
                            let remaining = result.summary.total_findings - 5;
                            let _ = writeln!(output, "  … +{remaining} more — run `kira detective` for full output");
                        }
                        let _ = tx.send(UiMessage::CommandResult { output, kind: OutputLineKind::System }).await;
                    }
                    Err(e) => {
                        let _ = tx.send(UiMessage::CommandError { message: format!("detective: {e}") }).await;
                    }
                }
            });
        }
        "/lessons" => {
            tokio::spawn(async move {
                match client.lessons_list().await {
                    Ok(lessons) => {
                        let output = if lessons.is_empty() {
                            "No lessons recorded.".to_string()
                        } else {
                            use std::fmt::Write;
                            let mut out = format!("Lessons ({}):\n", lessons.len());
                            for lesson in &lessons {
                                let _ = writeln!(
                                    out,
                                    "  [{id}] [{level}] {content}",
                                    id = lesson.id,
                                    level = lesson.level,
                                    content = truncate(&lesson.content, 100),
                                );
                            }
                            out
                        };
                        let _ = tx
                            .send(UiMessage::CommandResult {
                                output,
                                kind: OutputLineKind::System,
                            })
                            .await;
                    }
                    Err(e) => {
                        let _ = tx
                            .send(UiMessage::CommandError {
                                message: format!("lessons list failed: {e}"),
                            })
                            .await;
                    }
                }
            });
        }
        _ => {
            // Should not reach here if dispatch_sync returned Async correctly
            let cmd_owned = cmd.clone();
            tokio::spawn(async move {
                let _ = tx
                    .send(UiMessage::CommandError {
                        message: format!("unhandled async command: {cmd_owned}"),
                    })
                    .await;
            });
        }
    }
}

/// Result of prefix-matching against a set of agent IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrefixMatch {
    /// Exactly one agent matched.
    Found(String),
    /// No agents matched.
    NotFound,
    /// Multiple agents matched (count provided).
    Ambiguous(usize),
}

/// Find an agent by prefix, matching against both full `id` and `short_id`.
///
/// Returns `Found(full_id)` if exactly one agent matches, `NotFound` if zero
/// match, and `Ambiguous(n)` if more than one match.
///
/// Only non-terminal (active) agents are considered — the caller is responsible
/// for passing only active agents.
pub fn find_by_prefix(agents: &[crate::messages::AgentThread], prefix: &str) -> PrefixMatch {
    let matches: Vec<&str> = agents
        .iter()
        .filter(|a| a.id.starts_with(prefix) || a.short_id.starts_with(prefix))
        .map(|a| a.id.as_str())
        .collect();

    match matches.len() {
        0 => PrefixMatch::NotFound,
        1 => PrefixMatch::Found(matches[0].to_string()),
        n => PrefixMatch::Ambiguous(n),
    }
}

/// Return autocomplete suggestions for a partial slash-command.
pub fn autocomplete(partial: &str) -> Vec<&'static str> {
    if partial.is_empty() || !partial.starts_with('/') {
        return Vec::new();
    }
    let lower = partial.to_lowercase();
    COMMANDS
        .iter()
        .filter(|cmd| cmd.starts_with(&lower))
        .take(10)
        .copied()
        .collect()
}

/// Check if a command name is a recognized command.
pub fn is_known_command(cmd: &str) -> bool {
    let lower = cmd.to_lowercase();
    COMMANDS.iter().any(|c| *c == lower)
}

fn help_text() -> String {
    use std::fmt::Write;
    let mut text = String::from("Available commands:\n");
    for cmd in COMMANDS {
        let _ = writeln!(text, "  {cmd}");
    }
    text
}

/// Truncate a string to `max_len` characters, appending "..." if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

// ── Backward-compatible dispatch (delegates to dispatch_sync with dummy state) ──

/// Legacy dispatch function — kept for backward compatibility with existing tests.
/// Prefer `dispatch_sync` for new code.
pub fn dispatch(input: &str) -> DispatchResult {
    // Create a minimal state for sync dispatch
    let config = crate::config::Config::default();
    let mut state = AppState::new(config, crate::app::Mode::Yolo, crate::app::Tier::Standard);
    dispatch_sync(input, &mut state)
}

/// Attempt to submit a chat message. Returns `true` if submission was accepted
/// (input_locked was false), `false` if rejected (input_locked was true).
/// When rejected, the input field remains unchanged.
pub fn try_submit_message(state: &mut AppState, message: &str) -> bool {
    if state.input_locked {
        // Reject: input remains unchanged
        state.push_toast(
            crate::app::ToastKind::Warning,
            "Message already in flight".to_string(),
        );
        false
    } else {
        // Accept: lock input and clear
        state.input_locked = true;
        state.pending_input = Some(message.to_string());
        state.input.clear();
        state.input_cursor = 0;
        true
    }
}

/// Toggle the Layers View visibility.
///
/// Flips `layers_visible` and, on first show within the session, emits an
/// info toast with usage hints. This logic is shared between the `/layers`
/// slash command and the Ctrl+L keybinding.
pub fn toggle_layers(state: &mut AppState) {
    state.layers_visible = !state.layers_visible;
    if state.layers_visible && !state.layers_first_shown {
        state.layers_first_shown = true;
        state.push_toast(
            crate::app::ToastKind::Info,
            "Layers View: press 1-4 to drill into a layer".to_string(),
        );
    }
}

/// Classification of raw user input before dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputClass {
    /// Input starts with `/` — route to command dispatcher.
    Command,
    /// Non-empty input that doesn't start with `/` — submit as chat message.
    ChatMessage,
    /// Empty or whitespace-only — reject (no action).
    Rejected,
}

/// Classify raw user input into one of three categories.
///
/// This is the pure-function equivalent of the classification logic in the
/// event loop's Enter handler.
pub fn classify_input(raw: &str) -> InputClass {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        InputClass::Rejected
    } else if trimmed.starts_with('/') {
        InputClass::Command
    } else {
        InputClass::ChatMessage
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_quit() {
        assert_eq!(dispatch("/quit"), DispatchResult::Quit);
    }

    #[test]
    fn dispatch_help() {
        match dispatch("/help") {
            DispatchResult::Immediate(Some(text)) => assert!(text.contains("/mode")),
            _ => panic!("expected Immediate with help text"),
        }
    }

    #[test]
    fn dispatch_unknown_command() {
        match dispatch("/nonexistent") {
            DispatchResult::Unknown(cmd) => assert_eq!(cmd, "/nonexistent"),
            _ => panic!("expected Unknown"),
        }
    }

    #[test]
    fn dispatch_mode_yolo() {
        match dispatch("/mode yolo") {
            DispatchResult::Immediate(Some(msg)) => assert!(msg.contains("yolo")),
            _ => panic!("expected Immediate"),
        }
    }

    #[test]
    fn dispatch_tier_heavy() {
        match dispatch("/tier heavy") {
            DispatchResult::Immediate(Some(msg)) => assert!(msg.contains("heavy")),
            _ => panic!("expected Immediate"),
        }
    }

    #[test]
    fn dispatch_workflow_no_args() {
        match dispatch("/workflow") {
            DispatchResult::Error(msg) => assert!(msg.contains("usage")),
            _ => panic!("expected Error"),
        }
    }

    #[test]
    fn dispatch_cancel_is_async() {
        // /cancel with args should be dispatched asynchronously
        match dispatch("/cancel abc123") {
            DispatchResult::Async => {}
            _ => panic!("expected Async"),
        }
    }

    #[test]
    fn dispatch_cancel_no_args_is_async() {
        // /cancel without args still routes to async (error sent via channel)
        match dispatch("/cancel") {
            DispatchResult::Async => {}
            _ => panic!("expected Async"),
        }
    }

    #[test]
    fn dispatch_queue_is_async() {
        match dispatch("/queue") {
            DispatchResult::Async => {}
            _ => panic!("expected Async"),
        }
    }

    #[test]
    fn dispatch_queue_cancel_is_async() {
        match dispatch("/queue cancel abc") {
            DispatchResult::Async => {}
            _ => panic!("expected Async"),
        }
    }

    #[test]
    fn dispatch_memory_is_async() {
        match dispatch("/memory search test") {
            DispatchResult::Async => {}
            _ => panic!("expected Async"),
        }
    }

    #[test]
    fn dispatch_lessons_is_async() {
        match dispatch("/lessons") {
            DispatchResult::Async => {}
            _ => panic!("expected Async"),
        }
    }

    #[test]
    fn dispatch_agents_no_active() {
        match dispatch("/agents") {
            DispatchResult::Immediate(Some(msg)) => assert!(msg.contains("No active agents")),
            _ => panic!("expected Immediate"),
        }
    }

    #[test]
    fn dispatch_agents_with_active() {
        use crate::messages::AgentThread;
        let config = crate::config::Config::default();
        let mut state = AppState::new(config, crate::app::Mode::Yolo, crate::app::Tier::Standard);
        state.active_agents.push(AgentThread {
            id: "abcdef12-3456-7890-abcd-ef1234567890".to_string(),
            short_id: "abcdef12".to_string(),
            kind: "writer".to_string(),
            status: "running".to_string(),
            cost_usd: 0.0,
            last_output: Some("working on it...".to_string()),
        });
        match dispatch_sync("/agents", &mut state) {
            DispatchResult::Immediate(Some(msg)) => {
                assert!(msg.contains("abcdef12"));
                assert!(msg.contains("running"));
            }
            _ => panic!("expected Immediate with agent info"),
        }
    }

    #[test]
    fn dispatch_mode_updates_state() {
        let config = crate::config::Config::default();
        let mut state = AppState::new(config, crate::app::Mode::Yolo, crate::app::Tier::Standard);
        assert_eq!(state.mode, crate::app::Mode::Yolo);
        dispatch_sync("/mode plan", &mut state);
        assert_eq!(state.mode, crate::app::Mode::Plan);
    }

    #[test]
    fn dispatch_tier_updates_state() {
        let config = crate::config::Config::default();
        let mut state = AppState::new(config, crate::app::Mode::Yolo, crate::app::Tier::Standard);
        assert_eq!(state.tier, crate::app::Tier::Standard);
        dispatch_sync("/tier heavy", &mut state);
        assert_eq!(state.tier, crate::app::Tier::Heavy);
    }

    #[test]
    fn autocomplete_partial() {
        let results = autocomplete("/mo");
        assert!(results.contains(&"/mode"));
        assert!(!results.contains(&"/tier"));
    }

    #[test]
    fn autocomplete_slash_only() {
        let results = autocomplete("/");
        assert_eq!(results.len(), 10); // capped at 10
    }

    #[test]
    fn autocomplete_empty() {
        let results = autocomplete("");
        assert!(results.is_empty());
    }

    #[test]
    fn autocomplete_no_slash() {
        let results = autocomplete("mode");
        assert!(results.is_empty());
    }

    #[test]
    fn is_known_command_works() {
        assert!(is_known_command("/quit"));
        assert!(is_known_command("/mode"));
        assert!(!is_known_command("/nonexistent"));
    }

    // ── Property-Based Tests ──────────────────────────────────────────────────

    use proptest::prelude::*;

    // Feature: tui-wiring, Property 6: Input Classification
    // **Validates: Requirements 4.1, 5.1**

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property 6a: Any non-empty trimmed string starting with `/` is classified
        /// as a Command.
        #[test]
        fn prop_slash_prefix_classified_as_command(
            suffix in "[a-zA-Z0-9_ ]{1,50}"
        ) {
            let input = format!("/{suffix}");
            prop_assert_eq!(classify_input(&input), InputClass::Command);
        }

        /// Property 6b: Any non-empty trimmed string NOT starting with `/` is
        /// classified as a ChatMessage.
        #[test]
        fn prop_non_slash_classified_as_chat(
            // Generate strings that don't start with `/` and aren't all whitespace
            s in "[a-zA-Z0-9][a-zA-Z0-9 /]{0,50}"
        ) {
            prop_assert_eq!(classify_input(&s), InputClass::ChatMessage);
        }

        /// Property 6c: Empty or whitespace-only strings are classified as Rejected.
        #[test]
        fn prop_empty_or_whitespace_rejected(
            spaces in "[ \\t\\n\\r]{0,20}"
        ) {
            prop_assert_eq!(classify_input(&spaces), InputClass::Rejected);
        }

        /// Property 6d: Leading/trailing whitespace around a slash command doesn't
        /// change classification — still classified as Command.
        #[test]
        fn prop_whitespace_around_slash_still_command(
            leading in "[ \\t]{0,5}",
            cmd in "[a-zA-Z]{1,10}",
            trailing in "[ \\t]{0,5}",
        ) {
            let input = format!("{leading}/{cmd}{trailing}");
            prop_assert_eq!(classify_input(&input), InputClass::Command);
        }

        /// Property 6e: Leading/trailing whitespace around a chat message doesn't
        /// change classification — still classified as ChatMessage.
        #[test]
        fn prop_whitespace_around_chat_still_chat(
            leading in "[ \\t]{0,5}",
            msg in "[a-zA-Z][a-zA-Z0-9 ]{0,30}",
            trailing in "[ \\t]{0,5}",
        ) {
            let input = format!("{leading}{msg}{trailing}");
            prop_assert_eq!(classify_input(&input), InputClass::ChatMessage);
        }
    }

    // Feature: tui-wiring, Property 7: Input Lock Prevents Duplicate Submission
    // Feature: tui-wiring, Property 9: Unknown Command Detection
    // **Validates: Requirements 4.4, 5.9**

    /// Strategy that generates strings starting with `/` that do NOT match any
    /// registered command. We generate a `/` followed by 1-30 alphanumeric chars
    /// and filter out any that happen to match a known command prefix.
    fn unknown_command_strategy() -> impl Strategy<Value = String> {
        "[a-z]{1,30}".prop_map(|s| format!("/{s}")).prop_filter(
            "must not match any registered command",
            |cmd| {
                let lower = cmd.to_lowercase();
                // Extract just the command part (before any space)
                let cmd_part = lower.split_whitespace().next().unwrap_or(&lower);
                !COMMANDS.contains(&cmd_part)
            },
        )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property 7: When input_locked is true, try_submit_message rejects
        /// the submission and the input field remains unchanged.
        #[test]
        fn prop_input_lock_prevents_duplicate_submission(
            message in "[a-zA-Z0-9 ]{1,100}"
        ) {
            let config = crate::config::Config::default();
            let mut state = AppState::new(
                config,
                crate::app::Mode::Yolo,
                crate::app::Tier::Standard,
            );

            // Pre-set some input text to verify it remains unchanged
            state.input = "original text".to_string();
            state.input_cursor = 13;
            state.input_locked = true;

            let original_input = state.input.clone();
            let original_cursor = state.input_cursor;

            // Attempt submission — should be rejected
            let accepted = try_submit_message(&mut state, &message);

            // Verify rejection
            prop_assert!(!accepted, "submission should be rejected when input_locked == true");
            // Verify input field unchanged
            prop_assert_eq!(&state.input, &original_input, "input text must remain unchanged");
            prop_assert_eq!(state.input_cursor, original_cursor, "cursor must remain unchanged");
            // Verify input_locked remains true
            prop_assert!(state.input_locked, "input_locked must remain true");
            // Verify a warning toast was produced
            prop_assert!(!state.toasts.is_empty(), "a warning toast should be produced");
        }

        /// Property 9: For any string starting with `/` that does not match a
        /// registered command name, dispatch_sync returns Unknown with the
        /// unrecognized command string.
        #[test]
        fn prop_unknown_command_detection(
            cmd in unknown_command_strategy()
        ) {
            let config = crate::config::Config::default();
            let mut state = AppState::new(
                config,
                crate::app::Mode::Yolo,
                crate::app::Tier::Standard,
            );

            let result = dispatch_sync(&cmd, &mut state);

            match result {
                DispatchResult::Unknown(returned_cmd) => {
                    // The returned command should be the lowercased command part
                    let expected = cmd.trim().split(' ').next().unwrap().to_lowercase();
                    prop_assert_eq!(
                        returned_cmd, expected,
                        "Unknown should contain the unrecognized command string"
                    );
                }
                other => {
                    prop_assert!(
                        false,
                        "expected Unknown for '{}', got {:?}",
                        cmd,
                        other
                    );
                }
            }
        }
    }
}
