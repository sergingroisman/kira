#![forbid(unsafe_code)]

//! TUI entry point — initialises the terminal, runs the event loop.
//!
//! The event loop uses `tokio::select!` to multiplex:
//! 1. Terminal events (crossterm `EventStream`)
//! 2. Tick timer (100ms interval)
//! 3. `mpsc::Receiver<UiMessage>` for messages from background tasks
//!
//! All background tasks (health checker, SSE listener, agent pollers)
//! communicate exclusively through the bounded mpsc channel. The event loop
//! is the sole writer to `AppState`.

use anyhow::Result;
use chrono::Utc;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, EventStream},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use kira_cli::app::{AppState, CoreStatus, Mode, OutputLineKind, Tier, ToastKind, WorkflowQa};
use kira_cli::commands::{self, DispatchResult};
use kira_cli::config::Config;
use kira_cli::core_client::CoreClient;
use kira_cli::health::spawn_health_checker;
use kira_cli::input::{self, InputResult, OverlayKind};
use kira_cli::messages::{AgentThread, UiMessage};
use kira_cli::models::{
    BudgetInfo, ChatMessage, CommandPaletteState, ConnectionStatus, MessageContent, MessageKind,
    Overlay, ProviderStatus, SessionPickerState, TaskBrowserState,
};
use kira_cli::queue;
use kira_cli::sse::{PillarEventKind, spawn_listener};
use kira_cli::ui;
use kira_cli::views::approval::{
    ApprovalAction, check_approval_timeout, handle_approval_feedback_backspace,
    handle_approval_feedback_cancel, handle_approval_feedback_char,
    handle_approval_feedback_submit, handle_approval_key, resolve_approval,
};

/// Channel capacity for the event bus.
const CHANNEL_CAPACITY: usize = 256;

/// Tick interval for rendering.
const TICK_INTERVAL: Duration = Duration::from_millis(100);

/// Graceful shutdown timeout before force exit.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

/// Delay before restarting a failed background task.
const TASK_RESTART_DELAY: Duration = Duration::from_secs(1);

pub async fn run(config: Config, tier_str: String, mode_str: String) -> Result<()> {
    // Parse tier and mode
    let tier: Tier = tier_str.parse()?;
    let mode = match mode_str.as_str() {
        "plan" => Mode::Plan,
        "yolo" => Mode::Yolo,
        _ => Mode::Auto, // default: let KIRA decide per message
    };

    // Build state (owned by event loop — no Arc<Mutex>)
    let mut state = AppState::new(config.clone(), mode, tier);

    // Check minimum terminal size
    let (cols, rows) = crossterm::terminal::size()?;
    if cols < 80 || rows < 24 {
        anyhow::bail!("terminal too small: {cols}x{rows} — minimum required: 80x24");
    }

    // Build CoreClient
    let client = CoreClient::new(&config)?;

    // Create bounded mpsc channel (event bus)
    let (tx, rx) = mpsc::channel::<UiMessage>(CHANNEL_CAPACITY);

    // Create cancellation token for graceful shutdown
    let cancel = CancellationToken::new();

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the event loop
    let result = event_loop(&mut terminal, &mut state, rx, tx, client, cancel).await;

    // Restore terminal regardless of result
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

/// Apply a `UiMessage` to `AppState`, updating state for each variant.
///
/// This is the single point where background task results are integrated
/// into the application state. The event loop calls this for every message
/// received from the channel.
pub fn apply_message(msg: UiMessage, state: &mut AppState) {
    match msg {
        UiMessage::CoreConnected { version } => {
            state.core_status = CoreStatus::Connected {
                version: version.clone(),
            };
            state.layers_state.clear_disconnected();
            state.push_toast(
                ToastKind::Success,
                format!("Connected to Core (v{version})"),
            );
        }
        UiMessage::CoreDisconnected { reason } => {
            state.core_status = CoreStatus::Disconnected;
            state.layers_state.reset_all_idle();
            state.push_toast(ToastKind::Warning, format!("Core disconnected: {reason}"));
        }
        UiMessage::PillarEvent(kind) => {
            // Terminal pushed over SSE — render instantly (faster than the 2s
            // poll). Dedup in handle_agent_terminal guards against the poller
            // also reporting it.
            if let PillarEventKind::AgentTerminal { agent_id, status, result, error } = &kind {
                handle_agent_terminal(
                    state,
                    agent_id,
                    status,
                    error.as_deref(),
                    result.as_deref(),
                );
                return;
            }
            // Tool finished → push a Kiro-style activity chip into the chat.
            // Wave progress (SDD EXECUTE) → a header line "⌗ EXECUTE — wave N/M".
            if let PillarEventKind::Activity { phase, message, .. } = &kind
                && phase == "wave_progress"
            {
                state.push_output(format!("⌗ {message}"), OutputLineKind::System);
                return;
            }
            if let PillarEventKind::ToolActivityStart { tool, target } = &kind {
                let item = kira_cli::ui::chat::timeline::chip_running(tool, target);
                state.push_output(String::new(), OutputLineKind::Activity(item));
                return;
            }
            if let PillarEventKind::ToolActivity { tool, target, ok, detail } = &kind {
                // Flip the matching Running chip → Done/Error (in place). Fall
                // back to a fresh chip if the start event was missed.
                let (_, verb) = kira_cli::ui::chat::timeline::tool_icon_verb(tool);
                if !state.finish_running_chip(verb, *ok, detail.clone()) {
                    let item = kira_cli::ui::chat::timeline::chip_for_tool(
                        tool, target, *ok, detail.clone(),
                    );
                    state.push_output(String::new(), OutputLineKind::Activity(item));
                }
                return;
            }
            apply_pillar_event_to_layers(state, &kind);
            apply_pillar_event(kind, state);
        }
        UiMessage::AgentSpawned { agent_id } => {
            let short_id = agent_id.chars().take(8).collect::<String>();
            state.active_agents.push(AgentThread {
                id: agent_id,
                short_id: short_id.clone(),
                kind: String::new(),
                status: "running".to_string(),
                cost_usd: 0.0,
                last_output: None,
            });
            state.push_toast(ToastKind::Info, format!("Agent {short_id} spawned"));
        }
        UiMessage::AgentOutput { agent_id, line } => {
            // Update last_output for the agent
            if let Some(agent) = state.active_agents.iter_mut().find(|a| a.id == agent_id) {
                agent.last_output = Some(line.clone());
            }

            // Try to parse into a structured activity item first.
            if let Some(activity) = kira_cli::ui::chat::timeline::parse_activity(&line) {
                state.push_output(line, OutputLineKind::Activity(activity));
                return;
            }

            // Prefix with short ID if multiple agents active
            let formatted = if state.active_agents.len() > 1 {
                let short_id: String = agent_id.chars().take(8).collect();
                format!("[{short_id}] {line}")
            } else {
                line
            };

            state.push_output(formatted, OutputLineKind::Agent);
        }
        UiMessage::AgentTerminal {
            agent_id,
            status,
            error,
            result,
        } => {
            // Fallback delivery path (2s poller). Shares the deduped handler
            // with the instant SSE push.
            handle_agent_terminal(state, &agent_id, &status, error.as_deref(), result.as_deref());
        }
        UiMessage::ApprovalRequired(req) => {
            state.layers_state.set_security_blocked();
            if state.approval_active.is_some() {
                // Queue if modal already active
                state.approval_queue.push_back(req);
            } else {
                state.approval_active = Some(req);
                // Push the overlay so render_overlay actually draws the modal —
                // without this the gate is invisible and the tool auto-rejects
                // after the approval timeout (file never written).
                if !matches!(state.overlay_stack.top(), Some(Overlay::Approval)) {
                    state.overlay_stack.push(Overlay::Approval);
                }
            }
        }
        UiMessage::CommandResult { output, kind } => {
            state.push_output(output, kind);
        }
        UiMessage::CommandError { message } => {
            state.push_toast(ToastKind::Error, message);
        }
        UiMessage::MessageAccepted { agent_id, response } => {
            state.input_locked = false;
            state.pending_input = None;

            if let Some(content) = response {
                // Degraded inline fallback: Core answered without an agent.
                state.streaming_active = false;
                state.push_output(content, OutputLineKind::Agent);
            } else if !agent_id.is_empty() {
                // Agent spawned — show confirmation
                let short_id: String = agent_id.chars().take(8).collect();
                state.push_output(
                    format!("✓ Message submitted → agent {short_id}"),
                    OutputLineKind::System,
                );
            } else {
                state.push_output("✓ Message submitted".to_string(), OutputLineKind::System);
            }
        }
        UiMessage::MessageFailed { reason } => {
            state.input_locked = false;
            state.streaming_active = false;
            state.push_toast(ToastKind::Error, format!("Send failed: {reason}"));
            // Restore the pending input text so user can retry
            if let Some(text) = state.pending_input.take() {
                state.input = text;
                state.input_cursor = state.input.len();
            }
        }
        UiMessage::PlanReady(plan) => {
            // Workflow paused at the approval gate: render the task matrix
            // (tasks grouped by wave + owned files) and wait for [y]/[n].
            // Guard against duplicate poller emits.
            if state.pending_plan.is_some() {
                return;
            }
            state.input_locked = false;
            state.streaming_active = false;
            state.pending_input = None;
            state.push_output(
                format!("📋 Plano para: {}", plan.goal),
                OutputLineKind::System,
            );
            if plan.tasks.is_empty() {
                state.push_output(
                    "  (sem tasks — tasks.json vazio)".to_string(),
                    OutputLineKind::System,
                );
            }
            // Group tasks by wave (already sorted by wave from core).
            let mut current_wave: Option<u32> = None;
            for t in &plan.tasks {
                if current_wave != Some(t.wave) {
                    current_wave = Some(t.wave);
                    let n = plan
                        .tasks
                        .iter()
                        .filter(|x| x.wave == t.wave)
                        .count();
                    let parallel = if n > 1 { " (paralelo)" } else { "" };
                    state.push_output(
                        format!("▸ Wave {}{}", t.wave + 1, parallel),
                        OutputLineKind::System,
                    );
                }
                state.push_output(
                    format!("    {} — {}", t.id, t.title),
                    OutputLineKind::System,
                );
                if !t.owned_files.is_empty() {
                    state.push_output(
                        format!("        {}", t.owned_files.join(", ")),
                        OutputLineKind::System,
                    );
                }
            }
            state.push_output(
                format!(
                    "→ {} tasks · {} waves   [y] executar   [n] cancelar",
                    plan.tasks.len(),
                    plan.wave_count
                ),
                OutputLineKind::System,
            );
            state.pending_plan = Some(plan);
        }
        UiMessage::WorkflowUpdate(view) => {
            state.workflow_run_id = Some(view.id.clone());
            let phase = view.current_phase_id.clone();

            // Phase progression line (only on change).
            if state.workflow_last_phase.as_deref() != Some(phase.as_str()) {
                state.workflow_last_phase = Some(phase.clone());
                let glyph = match phase.as_str() {
                    "done" => "✓",
                    "failed" => "✗",
                    _ => "▸",
                };
                state.push_output(
                    format!("{glyph} workflow: fase {phase}"),
                    OutputLineKind::System,
                );
            }

            // Pending clarification questions → start inline Q&A (one at a time).
            if !view.pending_questions.is_empty() && state.workflow_qa.is_none() {
                let questions: Vec<(String, String)> = view
                    .pending_questions
                    .iter()
                    .map(|q| (q.id.clone(), q.question.clone()))
                    .collect();
                state.push_output(
                    format!("❓ {} pergunta(s) — responda cada uma e Enter:", questions.len()),
                    OutputLineKind::System,
                );
                if let Some((_, first)) = questions.first() {
                    state.push_output(format!("  1. {first}"), OutputLineKind::System);
                }
                state.workflow_qa = Some(WorkflowQa {
                    run_id: view.id.clone(),
                    questions,
                    answers: Vec::new(),
                });
            }

            // Terminal.
            if view.finished_at.is_some() || phase == "done" || phase == "failed" {
                if let Some(err) = view.error.as_deref().filter(|e| !e.is_empty()) {
                    state.push_output(format!("✗ workflow falhou: {err}"), OutputLineKind::Error);
                } else {
                    state.push_output("✓ workflow concluído".to_string(), OutputLineKind::System);
                }
                state.workflow_run_id = None;
                state.workflow_last_phase = None;
                state.workflow_qa = None;
            }
        }
        UiMessage::TaskFailed { task_name, error } => {
            state.push_toast(
                ToastKind::Error,
                format!("Task '{task_name}' failed: {error}"),
            );
        }
        UiMessage::SessionList { sessions } => {
            // Update the session picker overlay if it's on top of the stack
            if let Some(Overlay::SessionPicker(picker)) = state.overlay_stack.top_mut() {
                picker.sessions = sessions;
            }
        }
        UiMessage::TaskUpdate { waves } => {
            // Update the task browser overlay if it's on top of the stack
            if let Some(Overlay::TaskBrowser(browser)) = state.overlay_stack.top_mut() {
                browser.waves = waves;
                browser.last_refresh = std::time::Instant::now();
            }
        }
        UiMessage::SidebarRefresh {
            budget,
            providers,
            pipeline,
            waves,
            modified_files,
        } => {
            if let Some(b) = budget {
                state.budget_info = b;
            }
            if let Some(p) = providers {
                state.provider_statuses = p;
            }
            if let Some(pl) = pipeline {
                state.sdd_pipeline = Some(pl);
            }
            if let Some(w) = waves {
                state.waves = w;
            }
            if let Some(mf) = modified_files {
                state.modified_files = mf;
            }
        }
        UiMessage::StreamingToken { agent_id, token } => {
            state.streaming_active = true;
            // Append token to the last streaming message or create a new one
            if let Some(last) = state.chat_messages.last_mut() {
                if let MessageContent::Streaming { ref mut partial } = last.content {
                    partial.push_str(&token);
                } else {
                    // Start a new streaming message
                    let short_id: String = agent_id.chars().take(8).collect();
                    state.chat_messages.push(ChatMessage {
                        id: agent_id,
                        kind: MessageKind::Agent,
                        content: MessageContent::Streaming { partial: token },
                        timestamp: Utc::now(),
                        agent_short_id: Some(short_id),
                    });
                }
            } else {
                // No messages yet — create the first streaming message
                let short_id: String = agent_id.chars().take(8).collect();
                state.chat_messages.push(ChatMessage {
                    id: agent_id,
                    kind: MessageKind::Agent,
                    content: MessageContent::Streaming { partial: token },
                    timestamp: Utc::now(),
                    agent_short_id: Some(short_id),
                });
            }
        }
        UiMessage::StreamingDone { agent_id: _ } => {
            state.streaming_active = false;
        }
    }
}

/// Apply a pillar event to the Layers View state.
///
/// Maps `PillarEventKind` variants to `(Pillar, phase, message)` tuples,
/// classifies them into a conceptual layer, and updates `LayersState`.
/// Events that cannot be classified are silently ignored.
pub fn apply_pillar_event_to_layers(state: &mut AppState, kind: &PillarEventKind) {
    use kira_cli::layers::classify::{Pillar, classify};
    use kira_cli::layers::state::PipelineEvent;

    // Extract (pillar, phase, message) from the event kind.
    // Returns None for events that don't map to pipeline activity.
    let (pillar, phase, message) = match kind {
        PillarEventKind::GuardrailTriggered { level, reason, .. } => {
            (Pillar::P5Guardrails, "start".to_string(), format!("Guardrail [{level}]: {reason}"))
        }
        PillarEventKind::MemoryInjected { fragment_count, .. } => {
            if *fragment_count == 0 {
                return;
            }
            (Pillar::P7Cognition, "memory_done".to_string(), format!("{fragment_count} fragments injected"))
        }
        PillarEventKind::BudgetWarn { pct } => {
            (Pillar::P4Gates, "start".to_string(), format!("Budget: {pct}% consumed"))
        }
        PillarEventKind::BudgetStop => {
            (Pillar::P4Gates, "fail".to_string(), "Budget stop".to_string())
        }
        PillarEventKind::QueueDepthChanged { depth } => {
            (Pillar::P2Coordination, "step".to_string(), format!("Queue depth: {depth}"))
        }
        PillarEventKind::Activity { pillar, phase, message } => {
            // Generic pipeline activity from a raw PillarEvent. Map the wire
            // pillar string into the layer taxonomy; unknown pillars are dropped.
            let Some(p) = Pillar::from_wire(pillar) else {
                return;
            };
            (p, phase.clone(), message.clone())
        }
        // A HIGH-risk tool call awaiting operator approval is a Security-layer
        // activity — surface it so the Security layer lights up (was dropped,
        // leaving Security permanently idle).
        PillarEventKind::AwaitingApproval { tool_name, risk_level, .. } => {
            (Pillar::P5Guardrails, "start".to_string(), format!("aprovação [{risk_level}]: {tool_name}"))
        }
        PillarEventKind::Heartbeat { .. }
        | PillarEventKind::Unknown(_)
        | PillarEventKind::ToolActivityStart { .. }
        | PillarEventKind::ToolActivity { .. }
        | PillarEventKind::AgentTerminal { .. } => {
            return;
        }
    };

    // Classify into a conceptual layer — if None, ignore the event.
    let Some(layer) = classify(pillar, &phase) else {
        return;
    };

    // Construct PipelineEvent with HH:MM:SS timestamp.
    let now_utc = Utc::now();
    let timestamp = now_utc.format("%H:%M:%S").to_string();
    let now_instant = Instant::now();

    let event = PipelineEvent {
        timestamp,
        phase: phase.clone(),
        message: message.chars().take(60).collect(),
        raw_ts: now_instant,
    };

    // Apply event to layers state.
    state.layers_state.apply_event(layer, &phase, event);

    // Detect flow if this is a "start" phase.
    if phase == "start" {
        state.layers_state.detect_flow(layer, now_instant);
    }
}

/// Apply a pillar event to state.
fn apply_pillar_event(kind: PillarEventKind, state: &mut AppState) {
    match kind {
        PillarEventKind::BudgetWarn { pct } => {
            state.push_toast(ToastKind::Warning, format!("Budget: {pct}% consumed"));
        }
        PillarEventKind::BudgetStop => {
            state.budget_stopped = true;
            state.push_output(
                "⚠ BUDGET STOP — press any key to acknowledge".to_string(),
                OutputLineKind::Error,
            );
        }
        PillarEventKind::GuardrailTriggered { level, reason, .. } => {
            state.push_toast(ToastKind::Warning, format!("Guardrail [{level}]: {reason}"));
        }
        PillarEventKind::MemoryInjected { fragment_count, .. } => {
            if fragment_count > 0 {
                state.push_toast(
                    ToastKind::Info,
                    format!("Memory: {fragment_count} fragments injected"),
                );
            }
        }
        PillarEventKind::QueueDepthChanged { depth } => {
            state.queue_depth = depth;
        }
        PillarEventKind::Heartbeat { .. }
        | PillarEventKind::Unknown(_)
        | PillarEventKind::Activity { .. }
        | PillarEventKind::AwaitingApproval { .. }
        | PillarEventKind::ToolActivityStart { .. }
        | PillarEventKind::ToolActivity { .. }
        | PillarEventKind::AgentTerminal { .. } => {
            // No-op for toasts. Activity drives the Layers View; AgentTerminal
            // is intercepted in the PillarEvent handler before reaching here.
        }
    }
}

/// Heuristic for AUTO mode: does this request warrant a reviewed Plan (vs a
/// direct Yolo run)? Plan when it looks like a complex, multi-step build;
/// Yolo for questions and single simple actions.
/// Detect a single-deliverable research / documentation / question task.
/// These should run as ONE Explorer agent (cheap), NOT the 4-phase SDD
/// pipeline — running specify→design→tasks→execute on "study X / write a doc"
/// repeats web research 4× and burns ~60× the tokens for the same output.
fn is_research_doc(t: &str) -> bool {
    const RESEARCH: &[&str] = &[
        // PT
        "estud", "pesquis", "documenta", "explica", "explique", "resum",
        "o que é", "o que e", "como funciona", "como usar", "por que",
        "analise", "análise", "analisa", "tutorial", "compare", "leia",
        // EN
        "study", "research", "document", "explain", "summar", "what is",
        "how does", "how to", "why ", "analyze", "analyse", "compare ",
        "read ", "investigate", "overview",
    ];
    RESEARCH.iter().any(|k| t.contains(k))
}

fn wants_plan(text: &str) -> bool {
    let t = text.to_lowercase();
    // Research/doc/question → single Explorer agent, not the SDD pipeline.
    if is_research_doc(&t) {
        return false;
    }
    // Planning-worthy scope keywords (PT + EN).
    const HEAVY: &[&str] = &[
        "módulo", "modulo", "sistema", "projeto", "completo", "arquitetura",
        "pipeline", "refator", "refactor", "vários", "varios", "múltiplos",
        "multiplos", "endpoint", "integra", "migra", "module", "system",
        "architecture", "several", "migrate", "scaffold",
    ];
    if HEAVY.iter().any(|k| t.contains(k)) {
        return true;
    }
    // Multiple steps chained together (conjunctions / lists).
    let steps = t.matches(" e ").count()
        + t.matches(';').count()
        + t.matches(" then ").count()
        + t.matches(" depois ").count();
    if steps >= 2 {
        return true;
    }
    // Long, detailed action request.
    t.chars().count() > 160
}

/// Apply an agent's terminal state to the UI: render the final answer (once)
/// and toast. Idempotent across delivery paths — the SSE push and the 2s
/// poller can both report the same terminal; only the first (while the agent is
/// still tracked as active) renders, so the answer never appears twice.
fn handle_agent_terminal(
    state: &mut AppState,
    agent_id: &str,
    status: &str,
    error: Option<&str>,
    result: Option<&str>,
) {
    let was_active = state.active_agents.iter().any(|a| a.id == agent_id);
    if !was_active {
        // Already handled via the other delivery path — drop the duplicate.
        return;
    }
    state.active_agents.retain(|a| a.id != agent_id);
    if state.active_agents.is_empty() {
        state.streaming_active = false;
        // Stop the Layers timers — the run is over, nothing is "active".
        state.layers_state.finalize_all_active();
    }

    if status == "completed"
        && let Some(answer) = result.map(str::trim).filter(|s| !s.is_empty())
    {
        state.push_output(answer.to_string(), OutputLineKind::Agent);
        // Record the assistant turn so the next message carries this context.
        state.push_turn("assistant", answer.to_string());
    }

    let short_id: String = agent_id.chars().take(8).collect();
    let toast_msg = error.map_or_else(
        || format!("Agent {short_id} {status}"),
        |err| format!("Agent {short_id} {status}: {err}"),
    );
    let kind = match status {
        "completed" => ToastKind::Success,
        "failed" => ToastKind::Error,
        _ => ToastKind::Warning,
    };
    state.push_toast(kind, toast_msg);
}

/// Tracks background task handles for restart logic.
struct BackgroundTasks {
    health_checker: Option<JoinHandle<()>>,
    sse_listener: Option<JoinHandle<()>>,
    /// Periodic sidebar refresh poller (budget/providers via GET /v1/stats).
    sidebar_poller: Option<JoinHandle<()>>,
    /// Whether the SSE listener has been spawned at least once.
    sse_spawned: bool,
    /// Whether a health checker restart has been attempted.
    health_restart_attempted: bool,
    /// Whether an SSE listener restart has been attempted.
    sse_restart_attempted: bool,
}

#[allow(clippy::assigning_clones)]
async fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    state: &mut AppState,
    mut rx: mpsc::Receiver<UiMessage>,
    tx: mpsc::Sender<UiMessage>,
    client: CoreClient,
    cancel: CancellationToken,
) -> Result<()> {
    use crossterm::event::{Event, KeyCode, KeyModifiers};
    use futures::StreamExt;

    let mut events = EventStream::new();
    let mut tick = tokio::time::interval(TICK_INTERVAL);

    // Spawn health checker on startup
    let health_handle = spawn_health_checker(
        tx.clone(),
        client.clone(),
        state.config.time.health_interval_secs,
        cancel.clone(),
    );

    let mut tasks = BackgroundTasks {
        health_checker: Some(health_handle),
        sse_listener: None,
        sidebar_poller: None,
        sse_spawned: false,
        health_restart_attempted: false,
        sse_restart_attempted: false,
    };

    info!("Event loop started, health checker spawned");

    loop {
        tokio::select! {
            _ = tick.tick() => {
                // Periodic tick — expire toasts and render
                state.tick_toasts();

                // Animate the activity spinner (was never advanced, so the
                // "Thinking"/agent status indicator sat frozen on frame 0).
                if state.streaming_active || !state.active_agents.is_empty() {
                    state.spinner_frame = state.spinner_frame.wrapping_add(1);
                }

                // Splash: advance tick counter and auto-dismiss on connect.
                if state.splash_visible {
                    state.splash_ticks = state.splash_ticks.saturating_add(1);
                    if kira_cli::ui::splash::should_dismiss(state) {
                        state.splash_visible = false;
                    }
                }

                // Layers View: prune stale flow timestamps and update active durations
                let now = Instant::now();
                state.layers_state.prune_flow_windows(now);
                for layer in &mut state.layers_state.layers {
                    if let Some(since) = layer.active_since {
                        layer.last_duration_secs = now.duration_since(since).as_secs();
                    }
                }

                // Check approval timeout — auto-reject if expired
                if let Some(agent_id) = check_approval_timeout(state) {
                    // Auto-reject: send POST with approved=false, feedback=null
                    let client_timeout = client.clone();
                    let tx_timeout = tx.clone();
                    let tool_name = state
                        .approval_active
                        .as_ref()
                        .map(|r| r.tool_name.clone())
                        .unwrap_or_default();
                    resolve_approval(state);
                    state.layers_state.clear_security_blocked();
                    state.push_toast(
                        ToastKind::Warning,
                        format!("Auto-rejected '{tool_name}' (timeout)"),
                    );
                    tokio::spawn(async move {
                        if let Err(e) = client_timeout
                            .agent_approve(&agent_id, false, None)
                            .await
                        {
                            let _ = tx_timeout
                                .send(UiMessage::CommandError {
                                    message: format!("Auto-reject failed: {e}"),
                                })
                                .await;
                        }
                    });
                }

                let state_ref = &*state;
                terminal.draw(|frame| ui::render(frame, state_ref))?;

                if state.should_quit {
                    break;
                }

                // Check if background tasks have terminated unexpectedly
                check_task_health(&mut tasks, &tx, &client, &cancel, state).await;
            }
            Some(msg) = rx.recv() => {
                // Check if this is a CoreConnected that should trigger SSE spawn
                let should_spawn_sse = matches!(&msg, UiMessage::CoreConnected { .. })
                    && !tasks.sse_spawned;

                // Check if this is a CoreConnected that should trigger queue flush
                let should_flush_queue = matches!(&msg, UiMessage::CoreConnected { .. })
                    && !state.outgoing_queue.is_empty();

                // Check if this is a MessageAccepted that should trigger agent poller
                // Only spawn poller if agent_id is non-empty and no direct response
                let spawn_poller_agent_id = match &msg {
                    UiMessage::MessageAccepted { agent_id, response }
                        if !agent_id.is_empty() && response.is_none() => Some(agent_id.clone()),
                    _ => None,
                };

                // Drain the queue before applying the message (so flush uses current queue)
                let queued_messages = if should_flush_queue {
                    state.outgoing_queue.drain(..).collect::<Vec<_>>()
                } else {
                    Vec::new()
                };

                apply_message(msg, state);

                // Spawn SSE listener after first CoreConnected
                if should_spawn_sse {
                    let sse_handle = spawn_listener(
                        tx.clone(),
                        client.clone(),
                        cancel.clone(),
                    );
                    tasks.sse_listener = Some(sse_handle);
                    tasks.sse_spawned = true;
                    info!("SSE listener spawned after CoreConnected");

                    // Sidebar poller (budget/providers) — spawned alongside SSE.
                    if tasks.sidebar_poller.is_none() {
                        let interval_secs = state.config.ui.task_refresh_interval_secs;
                        let poller = spawn_sidebar_poller(
                            tx.clone(),
                            client.clone(),
                            interval_secs,
                            cancel.clone(),
                        );
                        tasks.sidebar_poller = Some(poller);
                        info!("Sidebar poller spawned after CoreConnected");
                    }
                }

                // Flush queued messages on reconnect (within 30s via spawned task)
                if !queued_messages.is_empty() {
                    let client_flush = client.clone();
                    let tx_flush = tx.clone();
                    let count = queued_messages.len();
                    tokio::spawn(async move {
                        queue::flush_queue(queued_messages, &client_flush, &tx_flush).await;
                    });
                    info!("Queue flush spawned for {count} pending messages after CoreConnected");
                }

                // Spawn agent poller after MessageAccepted
                if let Some(agent_id) = spawn_poller_agent_id {
                    let tx_poller = tx.clone();
                    let client_poller = client.clone();
                    let cancel_poller = cancel.clone();
                    tokio::spawn(async move {
                        spawn_agent_poller_channel(
                            agent_id,
                            client_poller,
                            tx_poller,
                            cancel_poller,
                        ).await;
                    });
                    info!("Agent poller spawned after MessageAccepted");
                }
            }
            maybe_event = events.next() => {
                let Some(Ok(event)) = maybe_event else { break };
                match event {
                    Event::Key(key) => {
                        // ── Splash dismiss ────────────────────────────────────
                        // Any keypress dismisses the splash; pass the key on to
                        // normal handling so e.g. Ctrl+C still quits.
                        if state.splash_visible {
                            state.splash_visible = false;
                        }

                        // ── Approval modal key handling ────────────────────────
                        // When the approval modal is active, intercept keys
                        // (except Ctrl+C which always triggers shutdown).
                        if state.approval_active.is_some()
                            && !(key.modifiers == KeyModifiers::CONTROL
                                && key.code == KeyCode::Char('c'))
                        {
                            let action = if state.approval_feedback_input.is_some() {
                                // In feedback mode
                                match key.code {
                                    KeyCode::Enter => {
                                        handle_approval_feedback_submit(state)
                                            .unwrap_or(ApprovalAction::Consumed)
                                    }
                                    KeyCode::Esc => {
                                        handle_approval_feedback_cancel(state)
                                            .unwrap_or(ApprovalAction::Consumed)
                                    }
                                    KeyCode::Backspace => {
                                        handle_approval_feedback_backspace(state)
                                            .unwrap_or(ApprovalAction::Consumed)
                                    }
                                    KeyCode::Char(c) => {
                                        handle_approval_feedback_char(state, c)
                                            .unwrap_or(ApprovalAction::Consumed)
                                    }
                                    _ => ApprovalAction::Consumed,
                                }
                            } else {
                                // Normal approval mode
                                match key.code {
                                    KeyCode::Char(c) => handle_approval_key(state, c),
                                    _ => ApprovalAction::Consumed,
                                }
                            };

                            // Process the action
                            match action {
                                ApprovalAction::Approve { agent_id } => {
                                    resolve_approval(state);
                                    state.layers_state.clear_security_blocked();
                                    let client_approve = client.clone();
                                    let tx_approve = tx.clone();
                                    tokio::spawn(async move {
                                        if let Err(e) = client_approve
                                            .agent_approve(&agent_id, true, None)
                                            .await
                                        {
                                            let _ = tx_approve
                                                .send(UiMessage::CommandError {
                                                    message: format!("Approval send failed: {e}"),
                                                })
                                                .await;
                                        }
                                    });
                                    state.push_toast(
                                        ToastKind::Success,
                                        "Tool call approved".to_string(),
                                    );
                                }
                                ApprovalAction::Reject { agent_id, feedback } => {
                                    resolve_approval(state);
                                    state.layers_state.clear_security_blocked();
                                    let client_reject = client.clone();
                                    let tx_reject = tx.clone();
                                    tokio::spawn(async move {
                                        let fb = feedback.as_deref();
                                        if let Err(e) = client_reject
                                            .agent_approve(&agent_id, false, fb)
                                            .await
                                        {
                                            let _ = tx_reject
                                                .send(UiMessage::CommandError {
                                                    message: format!("Rejection send failed: {e}"),
                                                })
                                                .await;
                                        }
                                    });
                                    state.push_toast(
                                        ToastKind::Warning,
                                        "Tool call rejected".to_string(),
                                    );
                                }
                                ApprovalAction::Consumed | ApprovalAction::NotHandled => {}
                            }
                            // Skip normal key handling — modal consumed the event
                            continue;
                        }

                        // ── Plan approval (PLAN mode) ──────────────────────────
                        // Workflow paused at the gate: [y] execute, [n]/[Esc] cancel.
                        // Resolving the gate on the daemon lets the run's existing
                        // poller continue delivering phase updates.
                        if state.pending_plan.is_some()
                            && !(key.modifiers == KeyModifiers::CONTROL
                                && key.code == KeyCode::Char('c'))
                        {
                            match key.code {
                                KeyCode::Char('y' | 'Y') => {
                                    if let Some(plan) = state.pending_plan.take() {
                                        state.streaming_active = true;
                                        let client_p = client.clone();
                                        let tx_p = tx.clone();
                                        let rid = plan.run_id.clone();
                                        tokio::spawn(async move {
                                            if let Err(e) = client_p.workflow_approve_plan(&rid, true).await {
                                                let _ = tx_p
                                                    .send(UiMessage::MessageFailed {
                                                        reason: e.to_string(),
                                                    })
                                                    .await;
                                            }
                                        });
                                        state.push_toast(
                                            ToastKind::Success,
                                            "Plano aprovado — executando".to_string(),
                                        );
                                    }
                                }
                                KeyCode::Char('n' | 'N') | KeyCode::Esc => {
                                    if let Some(plan) = state.pending_plan.take() {
                                        let client_p = client.clone();
                                        let rid = plan.run_id.clone();
                                        tokio::spawn(async move {
                                            let _ = client_p.workflow_approve_plan(&rid, false).await;
                                        });
                                    }
                                    state.push_output(
                                        "Plano cancelado.".to_string(),
                                        OutputLineKind::System,
                                    );
                                }
                                _ => {}
                            }
                            continue;
                        }

                        // ── Activity log expand/collapse (Kiro ▶/▼) ────────────
                        // Ctrl+E toggles all expandable tool chips.
                        if key.modifiers == KeyModifiers::CONTROL
                            && key.code == KeyCode::Char('e')
                        {
                            state.toggle_all_activity();
                            continue;
                        }

                        // ── Activity log per-item navigation (Kiro) ────────────
                        // Tab enters focus on the last chip (when chips exist);
                        // in focus mode ↑/↓ select, Enter/→ toggle, Esc exits.
                        let has_chips = state
                            .output
                            .iter()
                            .any(|o| matches!(o.kind, OutputLineKind::Activity(_)));
                        if state.activity_focus.is_some() {
                            match key.code {
                                KeyCode::Esc => {
                                    state.activity_focus = None;
                                    continue;
                                }
                                KeyCode::Up => {
                                    state.activity_focus_move(false);
                                    continue;
                                }
                                KeyCode::Down => {
                                    state.activity_focus_move(true);
                                    continue;
                                }
                                KeyCode::Enter | KeyCode::Right | KeyCode::Char(' ') => {
                                    state.toggle_focused_activity();
                                    continue;
                                }
                                KeyCode::Tab => {
                                    state.activity_focus_move(false);
                                    continue;
                                }
                                _ => {
                                    // Any other key exits focus and routes normally.
                                    state.activity_focus = None;
                                }
                            }
                        } else if key.code == KeyCode::Tab
                            && key.modifiers.is_empty()
                            && has_chips
                            && state.input.is_empty()
                        {
                            state.activity_focus_enter();
                            continue;
                        }

                        // ── Route through input router ─────────────────────────
                        let result = input::handle_key(key, state);
                        match result {
                            InputResult::Consumed
                            | InputResult::HistoryNavigate(_)
                            | InputResult::TriggerCompletion
                            | InputResult::AcceptCompletion => {
                                // These are handled internally by handle_key or are no-ops
                            }
                            InputResult::Quit => {
                                // Graceful shutdown: cancel all tasks, wait 2s, force exit
                                state.should_quit = true;
                                cancel.cancel();
                                info!("Ctrl+C received, initiating shutdown");

                                // Final render showing quit state
                                let state_ref = &*state;
                                terminal.draw(|frame| ui::render(frame, state_ref))?;

                                // Wait for tasks to finish (up to 2s)
                                let shutdown_result = tokio::time::timeout(
                                    SHUTDOWN_TIMEOUT,
                                    wait_for_tasks(&mut tasks),
                                ).await;

                                if shutdown_result.is_err() {
                                    warn!("Shutdown timeout exceeded, forcing exit");
                                    // Restore terminal before force exit
                                    disable_raw_mode()?;
                                    execute!(
                                        terminal.backend_mut(),
                                        LeaveAlternateScreen,
                                        DisableMouseCapture
                                    )?;
                                    terminal.show_cursor()?;
                                    std::process::exit(1);
                                }

                                break;
                            }
                            InputResult::SubmitMessage(input_text) => {
                                // Workflow clarification Q&A: route the input as
                                // the answer to the current question instead of chat.
                                if let Some(mut qa) = state.workflow_qa.take() {
                                    state.push_output(
                                        format!("  ↳ {input_text}"),
                                        OutputLineKind::User,
                                    );
                                    qa.answers.push(input_text.clone());
                                    if qa.answers.len() < qa.questions.len() {
                                        // Ask the next question.
                                        let (_, q) = &qa.questions[qa.answers.len()];
                                        state.push_output(
                                            format!("  {}. {q}", qa.answers.len() + 1),
                                            OutputLineKind::System,
                                        );
                                        state.workflow_qa = Some(qa);
                                    } else {
                                        // All answered → POST to Core, resume the run.
                                        let mut answers = std::collections::HashMap::new();
                                        for ((id, _), ans) in qa.questions.iter().zip(&qa.answers) {
                                            answers.insert(id.clone(), vec![ans.clone()]);
                                        }
                                        let run_id = qa.run_id.clone();
                                        let client_a = client.clone();
                                        let tx_a = tx.clone();
                                        tokio::spawn(async move {
                                            if let Err(e) = client_a.workflow_answer(&run_id, answers).await {
                                                let _ = tx_a
                                                    .send(UiMessage::CommandError {
                                                        message: format!("answer failed: {e}"),
                                                    })
                                                    .await;
                                            }
                                        });
                                        state.push_output(
                                            "✓ respostas enviadas — retomando workflow".to_string(),
                                            OutputLineKind::System,
                                        );
                                    }
                                    continue;
                                }
                                // Chat message submission
                                // Reject if input is locked (submission already in-flight)
                                if state.input_locked {
                                    state.push_toast(
                                        ToastKind::Warning,
                                        "Message already in flight".to_string(),
                                    );
                                } else if input_text.len() > 10_000 {
                                    // Reject if input exceeds max length
                                    state.push_toast(
                                        ToastKind::Error,
                                        "Message too long (max 10,000 characters)".to_string(),
                                    );
                                } else if matches!(state.core_status, CoreStatus::Disconnected) {
                                    // Core is disconnected — queue locally
                                    state.push_output(input_text.clone(), OutputLineKind::User);
                                    let tier_str = state.tier.to_string();
                                    match queue::enqueue_message(state, input_text.clone(), tier_str) {
                                        Ok(()) => {
                                            let queued_count = state.outgoing_queue.len();
                                            state.push_toast(
                                                ToastKind::Warning,
                                                format!("Queued locally ({queued_count} pending)"),
                                            );
                                        }
                                        Err(e) => {
                                            state.push_toast(
                                                ToastKind::Error,
                                                e.to_string(),
                                            );
                                        }
                                    }
                                } else {
                                    // Echo the user's message into the chat so they
                                    // see their own question as a line (was dead code).
                                    state.push_output(input_text.clone(), OutputLineKind::User);

                                    // Show the "thinking" indicator immediately, before
                                    // the first agent event arrives (~2s poll latency).
                                    state.streaming_active = true;

                                    // Lock input and save text for potential restore
                                    state.input_locked = true;
                                    state.pending_input = Some(input_text.clone());

                                    // Record the user turn so follow-ups carry
                                    // multi-turn context to the agent.
                                    state.push_turn("user", input_text.clone());
                                    let history = state.conversation.clone();

                                    // Spawn async submission task
                                    let tx_submit = tx.clone();
                                    let client_submit = client.clone();
                                    let tier_str = state.tier.to_string();
                                    // Resolve effective mode. AUTO lets KIRA pick:
                                    // complex/multi-step tasks → Plan (review
                                    // first); simple actions & questions → Yolo.
                                    let use_plan = match state.mode {
                                        Mode::Plan => true,
                                        Mode::Yolo => false,
                                        Mode::Auto => {
                                            let plan = wants_plan(&input_text);
                                            state.push_toast(
                                                ToastKind::Info,
                                                format!(
                                                    "Auto → {}",
                                                    if plan { "PLAN" } else { "YOLO" }
                                                ),
                                            );
                                            plan
                                        }
                                    };
                                    // PLAN / complex → drive the full SDD workflow
                                    // (spec→design→tasks→parallel waves) with live
                                    // phase progress + inline clarification Q&A.
                                    if use_plan {
                                        // Keep input usable — the run may pause for
                                        // questions the operator must answer.
                                        state.input_locked = false;
                                        state.streaming_active = false;
                                        let goal = input_text.clone();
                                        tokio::spawn(async move {
                                            match client_submit.workflow_start(&goal, "sdd", None, true).await {
                                                Ok(run) => {
                                                    let _ = tx_submit.send(UiMessage::CommandResult {
                                                        output: format!(
                                                            "▸ workflow iniciado: {} | fase={}",
                                                            run.id.chars().take(8).collect::<String>(),
                                                            run.current_phase_id
                                                        ),
                                                        kind: OutputLineKind::System,
                                                    }).await;
                                                    let rid = run.id.clone();
                                                    // Fetch the plan matrix once when the
                                                    // approval gate first trips.
                                                    let mut plan_sent = false;
                                                    loop {
                                                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                                                        match client_submit.workflow_get(&rid).await {
                                                            Ok(view) => {
                                                                let done = view.finished_at.is_some()
                                                                    || view.current_phase_id == "done"
                                                                    || view.current_phase_id == "failed";
                                                                let awaiting = view.awaiting_plan_approval;
                                                                let _ = tx_submit.send(UiMessage::WorkflowUpdate(view)).await;
                                                                if awaiting && !plan_sent {
                                                                    if let Ok(plan) = client_submit.workflow_plan(&rid).await {
                                                                        let _ = tx_submit.send(UiMessage::PlanReady(plan)).await;
                                                                        plan_sent = true;
                                                                    }
                                                                } else if !awaiting {
                                                                    plan_sent = false;
                                                                }
                                                                if done { break; }
                                                            }
                                                            Err(_) => break,
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    let _ = tx_submit.send(UiMessage::MessageFailed {
                                                        reason: e.to_string(),
                                                    }).await;
                                                }
                                            }
                                        });
                                    } else {
                                    tokio::spawn(async move {
                                        match client_submit.chat_submit(&history, &tier_str).await {
                                            Ok(accepted) => {
                                                // Tell the user why YOLO answered the way it did.
                                                if accepted.route == "lite" {
                                                    let _ = tx_submit
                                                        .send(UiMessage::CommandResult {
                                                            output: "🔍 modo leve — Explorer read-only (file_read/web) pesquisa antes de responder"
                                                                .to_string(),
                                                            kind: OutputLineKind::System,
                                                        })
                                                        .await;
                                                }
                                                let _ = tx_submit
                                                    .send(UiMessage::MessageAccepted {
                                                        agent_id: accepted.agent_id,
                                                        response: accepted.message,
                                                    })
                                                    .await;
                                            }
                                            Err(e) => {
                                                let _ = tx_submit
                                                    .send(UiMessage::MessageFailed {
                                                        reason: e.to_string(),
                                                    })
                                                    .await;
                                            }
                                        }
                                    });
                                    }
                                }
                            }
                            InputResult::SlashCommand(cmd) => {
                                match commands::dispatch_sync(&cmd, state) {
                                    DispatchResult::Immediate(Some(msg)) => {
                                        state.push_output(msg, OutputLineKind::System);
                                    }
                                    DispatchResult::Immediate(None) => {}
                                    DispatchResult::Quit => {
                                        state.should_quit = true;
                                        cancel.cancel();
                                    }
                                    DispatchResult::Unknown(c) => {
                                        state.push_output(
                                            format!("Unknown command: {c}. Type /help for available commands."),
                                            OutputLineKind::Error,
                                        );
                                    }
                                    DispatchResult::Error(msg) => {
                                        state.push_output(msg, OutputLineKind::Error);
                                    }
                                    DispatchResult::Async => {
                                        // Dispatch asynchronously — results come via channel
                                        commands::dispatch_async(
                                            &cmd,
                                            state,
                                            client.clone(),
                                            tx.clone(),
                                        );
                                    }
                                }
                            }
                            InputResult::ToggleSidebar => {
                                state.toggle_sidebar();
                            }
                            InputResult::ToggleMode => {
                                state.toggle_mode();
                            }
                            InputResult::ToggleLayers => {
                                state.layers_visible = !state.layers_visible;
                                // Emit first-show toast once per session
                                if state.layers_visible && !state.layers_first_shown {
                                    state.layers_first_shown = true;
                                    state.push_toast_with_duration(
                                        ToastKind::Info,
                                        "Layers View: press 1-4 to drill into a layer".to_string(),
                                        5,
                                    );
                                }
                            }
                            InputResult::OpenOverlay(kind) => {
                                let overlay = match kind {
                                    OverlayKind::CommandPalette => {
                                        Overlay::CommandPalette(CommandPaletteState::default())
                                    }
                                    OverlayKind::SessionPicker => {
                                        Overlay::SessionPicker(SessionPickerState::default())
                                    }
                                    OverlayKind::TaskBrowser => {
                                        Overlay::TaskBrowser(TaskBrowserState::default())
                                    }
                                    OverlayKind::Help => Overlay::Help,
                                };
                                state.overlay_stack.push(overlay);
                            }
                            InputResult::CloseOverlay => {
                                state.overlay_stack.pop();
                            }
                            InputResult::ScrollChat(delta) => {
                                if delta > 0 {
                                    #[allow(clippy::cast_sign_loss)]
                                    state.scroll_up(delta as u16, 20);
                                } else if delta < 0 {
                                    state.scroll_down(delta.unsigned_abs());
                                }
                            }
                        }
                    }
                    Event::Mouse(me) => {
                        // Left-click inside the input box → move the text cursor
                        // to the clicked position (the input is bordered, so the
                        // content starts one cell in/down).
                        use crossterm::event::{MouseButton, MouseEventKind};
                        // Mouse wheel scrolls the chat output (rendered-row space).
                        if matches!(me.kind, MouseEventKind::ScrollUp) {
                            #[allow(clippy::cast_possible_truncation)]
                            let total = state.rendered_rows.get().max(state.output.len());
                            let max_off = total.saturating_sub(1).min(u16::MAX as usize) as u16;
                            state.scroll_offset = state.scroll_offset.saturating_add(3).min(max_off);
                            state.auto_scroll = false;
                            continue;
                        }
                        if matches!(me.kind, MouseEventKind::ScrollDown) {
                            state.scroll_offset = state.scroll_offset.saturating_sub(3);
                            state.auto_scroll = state.scroll_offset == 0;
                            continue;
                        }
                        if matches!(me.kind, MouseEventKind::Down(MouseButton::Left)) {
                            let a = state.input_area.get();
                            let in_x = me.column > a.x && me.column < a.x + a.width.saturating_sub(1);
                            let in_y = me.row > a.y && me.row < a.y + a.height.saturating_sub(1);
                            if in_x && in_y {
                                let col = (me.column - a.x - 1) as usize;
                                let row = (me.row - a.y - 1) as usize;
                                state.input_cursor = kira_cli::ui::input::row_col_to_char_index(
                                    &state.input,
                                    row,
                                    col,
                                );
                            }
                        }
                    }
                    Event::Resize(_cols, _rows) => {
                        // Just note it — render will handle gracefully
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

/// Check if background tasks have terminated unexpectedly and attempt restart.
async fn check_task_health(
    tasks: &mut BackgroundTasks,
    tx: &mpsc::Sender<UiMessage>,
    client: &CoreClient,
    cancel: &CancellationToken,
    state: &mut AppState,
) {
    // Don't check if we're shutting down
    if cancel.is_cancelled() {
        return;
    }

    // Check health checker
    if let Some(ref handle) = tasks.health_checker
        && handle.is_finished()
    {
        tasks.health_checker = None;

        if tasks.health_restart_attempted {
            // Restart already attempted — operate in degraded mode
            state.push_toast(
                ToastKind::Warning,
                "Health checker failed permanently — degraded mode".to_string(),
            );
        } else {
            tasks.health_restart_attempted = true;
            let _ = tx
                .send(UiMessage::TaskFailed {
                    task_name: "health_checker".to_string(),
                    error: "terminated unexpectedly".to_string(),
                })
                .await;

            // Attempt single restart after delay
            let tx_clone = tx.clone();
            let client_clone = client.clone();
            let cancel_clone = cancel.clone();
            let interval = state.config.time.health_interval_secs;
            tokio::spawn(async move {
                tokio::time::sleep(TASK_RESTART_DELAY).await;
                if !cancel_clone.is_cancelled() {
                    let handle =
                        spawn_health_checker(tx_clone, client_clone, interval, cancel_clone);
                    // Fire-and-forget for the restart — it sends messages via channel.
                    drop(handle);
                }
            });
            warn!("Health checker terminated, attempting restart after 1s");
        }
    }

    // Check SSE listener
    if let Some(ref handle) = tasks.sse_listener
        && handle.is_finished()
    {
        tasks.sse_listener = None;

        if tasks.sse_restart_attempted {
            state.push_toast(
                ToastKind::Warning,
                "SSE listener failed permanently — degraded mode".to_string(),
            );
        } else {
            tasks.sse_restart_attempted = true;
            let _ = tx
                .send(UiMessage::TaskFailed {
                    task_name: "sse_listener".to_string(),
                    error: "terminated unexpectedly".to_string(),
                })
                .await;

            // Attempt single restart after delay
            let tx_clone = tx.clone();
            let client_clone = client.clone();
            let cancel_clone = cancel.clone();
            tokio::spawn(async move {
                tokio::time::sleep(TASK_RESTART_DELAY).await;
                if !cancel_clone.is_cancelled() {
                    let handle = spawn_listener(tx_clone, client_clone, cancel_clone);
                    drop(handle);
                }
            });
            warn!("SSE listener terminated, attempting restart after 1s");
        }
    }
}

/// Wait for all background tasks to complete (used during shutdown).
async fn wait_for_tasks(tasks: &mut BackgroundTasks) {
    if let Some(handle) = tasks.health_checker.take() {
        let _ = handle.await;
    }
    if let Some(handle) = tasks.sse_listener.take() {
        let _ = handle.await;
    }
    if let Some(handle) = tasks.sidebar_poller.take() {
        let _ = handle.await;
    }
}

/// Channel-based agent poller that sends status updates via the event bus.
///
/// Polls `GET /v1/agents/{id}` every 2 seconds until the agent reaches a
/// terminal status, then sends `UiMessage::AgentTerminal`.
async fn spawn_agent_poller_channel(
    agent_id: String,
    client: CoreClient,
    tx: mpsc::Sender<UiMessage>,
    cancel: CancellationToken,
) {
    use std::time::Duration;

    const POLL_INTERVAL: Duration = Duration::from_secs(2);
    const TERMINAL_STATUSES: &[&str] = &["completed", "failed", "cancelled", "stopped"];

    // Notify that the agent was spawned
    let _ = tx
        .send(UiMessage::AgentSpawned {
            agent_id: agent_id.clone(),
        })
        .await;

    loop {
        tokio::select! {
            () = cancel.cancelled() => break,
            () = tokio::time::sleep(POLL_INTERVAL) => {}
        }

        match client.agent_status(&agent_id).await {
            Ok(json_str) => {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&json_str) {
                    let status = v
                        .get("status")
                        .and_then(|s| s.as_str())
                        .unwrap_or("unknown")
                        .to_string();

                    if TERMINAL_STATUSES.contains(&status.as_str()) {
                        // Carry the final answer in AgentTerminal so the deduped
                        // handler renders it exactly once (the SSE push may have
                        // already delivered it).
                        let result = v
                            .get("result")
                            .and_then(|r| r.as_str())
                            .map(ToString::to_string);
                        let error = v
                            .get("error")
                            .and_then(|e| e.as_str())
                            .map(ToString::to_string);

                        let _ = tx
                            .send(UiMessage::AgentTerminal {
                                agent_id,
                                status,
                                error,
                                result,
                            })
                            .await;
                        break;
                    }

                    // Optionally send output lines if available
                    if let Some(output) = v.get("last_output").and_then(|o| o.as_str()) {
                        let _ = tx
                            .send(UiMessage::AgentOutput {
                                agent_id: agent_id.clone(),
                                line: output.to_string(),
                            })
                            .await;
                    }
                }
            }
            Err(e) => {
                tracing::warn!(agent_id = %agent_id, error = %e, "agent poll failed");
                // Continue polling on next cycle
            }
        }
    }
}

/// Periodically polls `GET /v1/stats` and pushes a `SidebarRefresh` so the
/// Budget and Providers sidebar sections reflect live daemon state. Previously
/// nothing ever sent `SidebarRefresh`, so the sidebar showed defaults forever.
fn spawn_sidebar_poller(
    tx: mpsc::Sender<UiMessage>,
    client: CoreClient,
    interval_secs: u64,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    use std::time::Duration;

    let interval = Duration::from_secs(interval_secs.max(1));
    tokio::spawn(async move {
        loop {
            tokio::select! {
                () = cancel.cancelled() => break,
                () = tokio::time::sleep(interval) => {}
            }

            match client.stats().await {
                Ok(s) => {
                    let budget = BudgetInfo {
                        tokens_used: s.tokens_used,
                        usage_pct: s.usage_pct,
                        cost_usd: s.usd_spent,
                        max_budget_usd: s.max_budget_usd,
                    };
                    let providers = s
                        .providers
                        .into_iter()
                        .map(|p| ProviderStatus {
                            name: p.name,
                            status: match p.status.as_str() {
                                "connected" => ConnectionStatus::Connected,
                                "disconnected" => ConnectionStatus::Disconnected,
                                _ => ConnectionStatus::Unknown,
                            },
                        })
                        .collect();

                    let _ = tx
                        .send(UiMessage::SidebarRefresh {
                            budget: Some(budget),
                            providers: Some(providers),
                            pipeline: None,
                            waves: None,
                            modified_files: None,
                        })
                        .await;
                }
                Err(e) => {
                    tracing::debug!(error = %e, "sidebar stats poll failed");
                }
            }
        }
    })
}
