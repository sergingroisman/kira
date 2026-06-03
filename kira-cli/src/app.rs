#![forbid(unsafe_code)]

//! Application state — shared between the TUI render loop and async tasks.

use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::Config;
use crate::layers::config::LayersConfig;
use crate::layers::state::LayersState;
use crate::messages::{AgentThread, ApprovalRequest, PendingMessage};
use crate::models::{
    BudgetInfo, ChatMessage, CompletionState, ModifiedFile, OverlayStack, ProviderStatus,
    SddPipelineState, WaveState, WhichKeyState,
};

/// Execution mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// KIRA decides Plan vs Yolo per message based on task complexity.
    Auto,
    Yolo,
    Plan,
}

/// Inline clarification Q&A for a workflow run — answered one question at a
/// time via the input bar.
#[derive(Debug, Clone)]
pub struct WorkflowQa {
    pub run_id: String,
    /// `(question_id, question_text)` still to ask.
    pub questions: Vec<(String, String)>,
    /// Collected answers, parallel to `questions` by index.
    pub answers: Vec<String>,
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auto => write!(f, "AUTO"),
            Self::Yolo => write!(f, "YOLO"),
            Self::Plan => write!(f, "PLAN"),
        }
    }
}

/// LLM tier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tier {
    Heavy,
    Standard,
    Light,
}

impl std::fmt::Display for Tier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Heavy => write!(f, "heavy"),
            Self::Standard => write!(f, "standard"),
            Self::Light => write!(f, "light"),
        }
    }
}

impl std::str::FromStr for Tier {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "heavy" => Ok(Self::Heavy),
            "standard" => Ok(Self::Standard),
            "light" => Ok(Self::Light),
            other => anyhow::bail!("invalid tier: {other}; valid: heavy, standard, light"),
        }
    }
}

/// Connection status to kira-cored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreStatus {
    Unknown,
    Connected { version: String },
    Disconnected,
}

/// A toast notification.
#[derive(Debug, Clone)]
pub struct Toast {
    pub kind: ToastKind,
    pub message: String,
    pub created_at: std::time::Instant,
    pub duration_secs: u64,
    pub persistent: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToastKind {
    Success,
    Warning,
    Error,
    Info,
}

/// A line in the main chat/output panel.
#[derive(Debug, Clone)]
pub struct OutputLine {
    pub text: String,
    pub kind: OutputLineKind,
}

#[derive(Debug, Clone)]
pub enum OutputLineKind {
    User,
    Agent,
    System,
    Error,
    /// A structured kiro-style timeline chip (icon + verb + target).
    Activity(crate::models::ActivityItem),
}

/// Shared application state (wrapped in Arc<Mutex> for async access).
#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct AppState {
    pub config: Config,
    pub mode: Mode,
    pub tier: Tier,
    pub core_status: CoreStatus,
    pub output: Vec<OutputLine>,
    pub toasts: VecDeque<Toast>,
    pub input: String,
    pub input_cursor: usize,
    pub history: Vec<String>,
    pub history_pos: Option<usize>,
    pub should_quit: bool,
    pub budget_stopped: bool,
    pub queue_depth: u32,

    // ── Connection resilience ──────────────────────────────────────────────
    /// Messages queued locally while Core is unreachable (max 64).
    pub outgoing_queue: VecDeque<PendingMessage>,
    /// Prevents duplicate submissions while a message is in-flight.
    pub input_locked: bool,
    /// Holds the input text while a submission is in-flight (restored on failure).
    pub pending_input: Option<String>,

    // ── HITL ───────────────────────────────────────────────────────────────
    /// The currently displayed approval modal, if any.
    pub approval_active: Option<ApprovalRequest>,
    /// Queued approval requests waiting for the current modal to resolve (FIFO).
    pub approval_queue: VecDeque<ApprovalRequest>,
    /// Text input buffer for feedback mode (Some when feedback mode is active).
    pub approval_feedback_input: Option<String>,

    // ── Scroll state ───────────────────────────────────────────────────────
    /// Rows scrolled up from the bottom of the output panel. Counted in
    /// *rendered* rows (one source line may expand into many — markdown,
    /// wrapping, card headers), not source `output` indices.
    pub scroll_offset: u16,
    /// Whether auto-scroll is active (true when scroll_offset == 0).
    pub auto_scroll: bool,
    /// Total rendered rows produced by the last chat draw. Set by the renderer
    /// (interior-mutable so it can run on `&AppState`), read by the scroll
    /// clamp so scrolling spans the fully-expanded view.
    pub rendered_rows: std::cell::Cell<usize>,
    /// Screen rect of the input widget from the last draw. Set by the renderer,
    /// read by the mouse handler to map a click to a cursor position.
    pub input_area: std::cell::Cell<ratatui::layout::Rect>,
    /// The plan matrix awaiting operator approval ([y]/[n]) at the workflow gate.
    pub pending_plan: Option<crate::core_client::PlanMatrixView>,
    /// Index into `output` of the activity chip currently focused for
    /// keyboard navigation (Kiro-style). `None` = not in activity-focus mode.
    pub activity_focus: Option<usize>,
    /// Rolling conversation turns `(role, content)` for multi-turn context.
    /// Sent with each submit so follow-ups see prior turns. Capped.
    pub conversation: Vec<(String, String)>,
    /// Active workflow run id being polled for progress. `None` = no run.
    pub workflow_run_id: Option<String>,
    /// Last phase reported for the active workflow run (dedup progress lines).
    pub workflow_last_phase: Option<String>,
    /// Pending clarification Q&A for a workflow run: answer one at a time.
    pub workflow_qa: Option<WorkflowQa>,

    // ── Agent tracking ─────────────────────────────────────────────────────
    /// Currently active (non-terminal) agent threads.
    pub active_agents: Vec<AgentThread>,

    // ── Layout state ───────────────────────────────────────────────────────
    /// Whether the sidebar is currently visible.
    pub sidebar_visible: bool,
    /// True if the user manually toggled the sidebar via Tab.
    pub sidebar_manual_toggle: bool,
    /// Current input area height (clamped to 3–10).
    pub input_height: u16,

    // ── Overlay state ──────────────────────────────────────────────────────
    /// Z-ordered overlay stack; top overlay captures input focus.
    pub overlay_stack: OverlayStack,

    // ── Sidebar data ───────────────────────────────────────────────────────
    /// Title of the active session (None if no session).
    pub session_title: Option<String>,
    /// Current workspace path.
    pub workspace_path: String,
    /// Current git branch name.
    pub git_branch: Option<String>,
    /// Budget usage information.
    pub budget_info: BudgetInfo,
    /// Provider connection statuses.
    pub provider_statuses: Vec<ProviderStatus>,
    /// SDD pipeline state (None if no pipeline active).
    pub sdd_pipeline: Option<SddPipelineState>,
    /// Wave execution states.
    pub waves: Vec<WaveState>,
    /// Modified files in the workspace.
    pub modified_files: Vec<ModifiedFile>,
    /// Count of connected LSP servers.
    pub lsp_count: u16,
    /// Count of connected MCP servers.
    pub mcp_count: u16,

    // ── Chat state ─────────────────────────────────────────────────────────
    /// Rich chat messages (new renderer model).
    pub chat_messages: Vec<ChatMessage>,
    /// Whether an agent is actively streaming a response.
    pub streaming_active: bool,
    /// Current spinner animation frame index.
    pub spinner_frame: u8,

    // ── Input state ────────────────────────────────────────────────────────
    /// Active completion popup state (None if no popup visible).
    pub completion_state: Option<CompletionState>,
    /// Which-key panel state (None if not showing).
    pub which_key_state: Option<WhichKeyState>,

    // ── Model info ─────────────────────────────────────────────────────────
    /// Name of the currently active LLM model.
    pub active_model: String,
    /// Cumulative session cost in USD.
    pub session_cost_usd: f64,

    // ── Splash screen ──────────────────────────────────────────────────────
    /// Whether the startup splash screen is still showing.
    pub splash_visible: bool,
    /// Elapsed tick count while splash is shown (each tick = 100ms).
    pub splash_ticks: u32,

    // ── Layers View state ──────────────────────────────────────────────────
    /// Whether the Layers View is currently visible.
    pub layers_visible: bool,
    /// Layers View state (always maintained regardless of visibility).
    pub layers_state: LayersState,
    /// Layers View configuration (loaded from tokens.toml).
    pub layers_config: LayersConfig,
    /// Whether the first-show toast has been displayed this session.
    pub layers_first_shown: bool,
}

pub type SharedState = Arc<Mutex<AppState>>;

impl AppState {
    #[allow(clippy::missing_const_for_fn)]
    pub fn new(config: Config, mode: Mode, tier: Tier) -> Self {
        let mut layers_config = config.ui.layers.clone();
        let clamped_fields = layers_config.validate_and_clamp();
        let layers_visible = config.ui.layers.visible_on_start;

        let mut state = Self {
            config,
            mode,
            tier,
            core_status: CoreStatus::Unknown,
            output: Vec::new(),
            toasts: VecDeque::new(),
            input: String::new(),
            input_cursor: 0,
            history: Vec::new(),
            history_pos: None,
            should_quit: false,
            budget_stopped: false,
            queue_depth: 0,
            outgoing_queue: VecDeque::new(),
            input_locked: false,
            pending_input: None,
            approval_active: None,
            approval_queue: VecDeque::new(),
            approval_feedback_input: None,
            scroll_offset: 0,
            rendered_rows: std::cell::Cell::new(0),
            input_area: std::cell::Cell::new(ratatui::layout::Rect::new(0, 0, 0, 0)),
            pending_plan: None,
            activity_focus: None,
            conversation: Vec::new(),
            workflow_run_id: None,
            workflow_last_phase: None,
            workflow_qa: None,
            auto_scroll: true,
            active_agents: Vec::new(),

            // Layout state
            sidebar_visible: false,
            sidebar_manual_toggle: false,
            input_height: 3,

            // Overlay state
            overlay_stack: OverlayStack::new(),

            // Sidebar data
            session_title: None,
            workspace_path: String::new(),
            git_branch: None,
            budget_info: BudgetInfo::default(),
            provider_statuses: Vec::new(),
            sdd_pipeline: None,
            waves: Vec::new(),
            modified_files: Vec::new(),
            lsp_count: 0,
            mcp_count: 0,

            // Chat state
            chat_messages: Vec::new(),
            streaming_active: false,
            spinner_frame: 0,

            // Input state
            completion_state: None,
            which_key_state: None,

            // Model info
            active_model: String::new(),
            session_cost_usd: 0.0,

            // Splash screen
            splash_visible: true,
            splash_ticks: 0,

            // Layers View state
            layers_visible,
            layers_state: LayersState::new(),
            layers_config,
            layers_first_shown: false,
        };

        // Emit warning toasts for any clamped config fields
        for field in clamped_fields {
            tracing::warn!(field, "layers config value out of range, reset to default");
            state.push_toast(
                ToastKind::Warning,
                format!("Layers config: {field} was out of range, reset to default"),
            );
        }

        state
    }

    pub fn push_toast(&mut self, kind: ToastKind, message: String) {
        const MAX_TOASTS: usize = 5;
        if self.toasts.len() >= MAX_TOASTS {
            self.toasts.pop_front();
        }
        self.toasts.push_back(Toast {
            kind,
            message,
            created_at: std::time::Instant::now(),
            duration_secs: self.config.ui.toast_duration_secs,
            persistent: false,
        });
    }

    /// Push a toast with a custom duration in seconds.
    pub fn push_toast_with_duration(&mut self, kind: ToastKind, message: String, duration_secs: u64) {
        const MAX_TOASTS: usize = 5;
        if self.toasts.len() >= MAX_TOASTS {
            self.toasts.pop_front();
        }
        self.toasts.push_back(Toast {
            kind,
            message,
            created_at: std::time::Instant::now(),
            duration_secs,
            persistent: false,
        });
    }

    /// Push a persistent toast that will not auto-dismiss.
    pub fn push_persistent_toast(&mut self, kind: ToastKind, message: String) {
        const MAX_TOASTS: usize = 5;
        if self.toasts.len() >= MAX_TOASTS {
            self.toasts.pop_front();
        }
        self.toasts.push_back(Toast {
            kind,
            message,
            created_at: std::time::Instant::now(),
            duration_secs: 0,
            persistent: true,
        });
    }

    /// Maximum number of lines retained in the output buffer.
    pub const OUTPUT_BUFFER_CAP: usize = 10_000;

    /// Push a line to the output buffer, enforcing the 10,000-line cap.
    ///
    /// When the buffer exceeds the cap, the oldest lines are evicted using
    /// `drain(0..excess)` for efficient bulk removal. If `auto_scroll` is
    /// active (scroll_offset == 0), the offset stays at 0 so the latest
    /// line remains visible.
    /// Toggle collapse on every expandable activity chip (Kiro ▶/▼). If any
    /// is currently collapsed, expand all; otherwise collapse all.
    pub fn toggle_all_activity(&mut self) {
        let any_collapsed = self.output.iter().any(|o| {
            matches!(&o.kind, OutputLineKind::Activity(a) if !a.detail.is_empty() && a.collapsed)
        });
        for o in &mut self.output {
            if let OutputLineKind::Activity(a) = &mut o.kind {
                if !a.detail.is_empty() {
                    a.collapsed = !any_collapsed;
                }
            }
        }
    }

    /// Record a conversation turn (role = "user" | "assistant"), capping the
    /// rolling window so context stays bounded.
    pub fn push_turn(&mut self, role: &str, content: String) {
        if content.trim().is_empty() {
            return;
        }
        self.conversation.push((role.to_string(), content));
        const MAX_TURNS: usize = 16;
        if self.conversation.len() > MAX_TURNS {
            let drop = self.conversation.len() - MAX_TURNS;
            self.conversation.drain(0..drop);
        }
    }

    /// `output` indices that are activity chips (for keyboard navigation).
    fn activity_indices(&self) -> Vec<usize> {
        self.output
            .iter()
            .enumerate()
            .filter(|(_, o)| matches!(o.kind, OutputLineKind::Activity(_)))
            .map(|(i, _)| i)
            .collect()
    }

    /// Enter activity-focus mode on the last chip. No-op if there are none.
    pub fn activity_focus_enter(&mut self) {
        self.activity_focus = self.activity_indices().last().copied();
    }

    /// Move the activity selection by one chip. `forward` = toward newest.
    pub fn activity_focus_move(&mut self, forward: bool) {
        let idxs = self.activity_indices();
        if idxs.is_empty() {
            self.activity_focus = None;
            return;
        }
        let cur = self.activity_focus.and_then(|f| idxs.iter().position(|&i| i == f));
        let next = match cur {
            Some(p) if forward => (p + 1).min(idxs.len() - 1),
            Some(p) => p.saturating_sub(1),
            None => idxs.len() - 1,
        };
        self.activity_focus = Some(idxs[next]);
    }

    /// Flip the most-recent Running chip with matching `verb` to Done/Error and
    /// attach its detail. Returns false if no matching Running chip exists
    /// (caller should push a fresh chip then).
    pub fn finish_running_chip(&mut self, verb: &str, ok: bool, detail: Vec<String>) -> bool {
        use crate::models::ActivityStatus;
        for o in self.output.iter_mut().rev() {
            if let OutputLineKind::Activity(a) = &mut o.kind {
                if a.status == ActivityStatus::Running && a.verb == verb {
                    a.status = if ok { ActivityStatus::Done } else { ActivityStatus::Error };
                    a.detail = detail;
                    return true;
                }
            }
        }
        false
    }

    /// Toggle collapse on the focused chip.
    pub fn toggle_focused_activity(&mut self) {
        if let Some(i) = self.activity_focus
            && let Some(OutputLine { kind: OutputLineKind::Activity(a), .. }) = self.output.get_mut(i)
            && !a.detail.is_empty()
        {
            a.collapsed = !a.collapsed;
        }
    }

    pub fn push_output(&mut self, text: String, kind: OutputLineKind) {
        self.output.push(OutputLine { text, kind });

        // Enforce buffer cap — evict oldest lines
        if self.output.len() > Self::OUTPUT_BUFFER_CAP {
            let excess = self.output.len() - Self::OUTPUT_BUFFER_CAP;
            self.output.drain(0..excess);

            // Adjust scroll_offset so the user's view doesn't jump
            if self.scroll_offset > 0 {
                #[allow(clippy::cast_possible_truncation)]
                let excess_u16 = excess.min(u16::MAX as usize) as u16;
                self.scroll_offset = self.scroll_offset.saturating_sub(excess_u16);
            }
        }
    }

    /// Toggle sidebar visibility (Tab key).
    ///
    /// Flips `sidebar_visible` and sets `sidebar_manual_toggle = true` to
    /// indicate the user explicitly toggled via Tab (as opposed to auto-hide
    /// based on terminal width).
    pub const fn toggle_sidebar(&mut self) {
        self.sidebar_visible = !self.sidebar_visible;
        self.sidebar_manual_toggle = true;
    }

    /// Toggle execution mode between Plan and Yolo (Ctrl+M).
    ///
    /// Flips `self.mode` and pushes an info toast confirming the new mode.
    /// The header bar renders `state.mode` each frame, so it updates
    /// automatically on the next render cycle.
    pub fn toggle_mode(&mut self) {
        // Cycle Auto → Yolo → Plan → Auto.
        self.mode = match self.mode {
            Mode::Auto => Mode::Yolo,
            Mode::Yolo => Mode::Plan,
            Mode::Plan => Mode::Auto,
        };
        self.push_toast(ToastKind::Info, format!("Mode: {}", self.mode));
    }

    /// Remove expired toasts (persistent toasts are never auto-dismissed).
    pub fn tick_toasts(&mut self) {
        let now = std::time::Instant::now();
        self.toasts.retain(|t| {
            t.persistent || now.duration_since(t.created_at).as_secs() < t.duration_secs
        });
    }

    // ── Scroll offset logic ────────────────────────────────────────────────

    /// Compute the maximum scroll offset given the visible height.
    ///
    /// Returns `max(0, total_lines - visible_height)` as a `u16`.
    /// If total_lines ≤ visible_height, returns 0 (no scrolling possible).
    #[allow(clippy::cast_possible_truncation)]
    pub fn scroll_max_offset(&self, visible_height: u16) -> u16 {
        // Clamp against *rendered* rows (markdown/wrapping/cards expand one
        // source line into many). Falls back to source count before the first
        // draw populates `rendered_rows`.
        let total = self.rendered_rows.get().max(self.output.len());
        let max_raw = total.saturating_sub(visible_height as usize);
        max_raw.min(u16::MAX as usize) as u16
    }

    /// Scroll up (increase offset) by `increment` lines, clamped to the
    /// maximum offset for the given visible height.
    ///
    /// Uses `config.ui.scroll_lines_per_key` as the default increment when
    /// called from the event loop. The `increment` parameter allows callers
    /// to override (e.g., page-up).
    pub fn scroll_up(&mut self, increment: u16, visible_height: u16) {
        let max_offset = self.scroll_max_offset(visible_height);
        self.scroll_offset = self.scroll_offset.saturating_add(increment).min(max_offset);
        self.auto_scroll = self.scroll_offset == 0;
    }

    /// Scroll down (decrease offset) by `increment` lines, clamped to 0.
    ///
    /// When scroll_offset reaches 0, auto_scroll is re-enabled so new
    /// output lines are immediately visible.
    #[allow(clippy::missing_const_for_fn)]
    pub fn scroll_down(&mut self, increment: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(increment);
        self.auto_scroll = self.scroll_offset == 0;
    }
}

/// Pure function for computing a new scroll offset given a delta.
///
/// - `current`: the current scroll offset.
/// - `delta`: positive means scroll up (increase offset), negative means scroll down (decrease offset).
/// - `total_lines`: total number of output lines in the buffer.
/// - `visible_height`: number of lines visible in the chat panel.
///
/// Returns the new offset clamped to `[0, max(0, total_lines - visible_height)]`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn compute_scroll_offset(
    current: u16,
    delta: i16,
    total_lines: usize,
    visible_height: u16,
) -> u16 {
    let max_offset = total_lines
        .saturating_sub(visible_height as usize)
        .min(u16::MAX as usize) as u16;

    let new_offset = if delta >= 0 {
        current.saturating_add(delta as u16)
    } else {
        // delta is negative; negate safely (i16::MIN edge case handled by unsigned_abs)
        current.saturating_sub(delta.unsigned_abs())
    };

    new_offset.min(max_offset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Helper to create a default AppState for testing.
    fn test_state() -> AppState {
        AppState::new(crate::config::Config::default(), Mode::Plan, Tier::Standard)
    }

    fn chip(detail: bool) -> crate::models::ActivityItem {
        crate::models::ActivityItem {
            icon: "◎",
            verb: "Read",
            target: "x".into(),
            subtitle: None,
            status: crate::models::ActivityStatus::Done,
            detail: if detail { vec!["d".into()] } else { vec![] },
            collapsed: true,
        }
    }

    #[test]
    fn activity_focus_enter_move_toggle() {
        let mut s = test_state();
        s.push_output("hi".into(), OutputLineKind::User);
        s.push_output(String::new(), OutputLineKind::Activity(chip(true))); // idx 1
        s.push_output(String::new(), OutputLineKind::Activity(chip(true))); // idx 2

        s.activity_focus_enter();
        assert_eq!(s.activity_focus, Some(2)); // last chip
        s.activity_focus_move(false); // up
        assert_eq!(s.activity_focus, Some(1));
        s.activity_focus_move(false); // clamp at top
        assert_eq!(s.activity_focus, Some(1));

        // Toggle the focused chip's collapse.
        s.toggle_focused_activity();
        if let OutputLineKind::Activity(a) = &s.output[1].kind {
            assert!(!a.collapsed, "should be expanded after toggle");
        } else {
            panic!("expected activity");
        }
    }

    #[test]
    fn activity_focus_enter_noop_without_chips() {
        let mut s = test_state();
        s.push_output("hi".into(), OutputLineKind::User);
        s.activity_focus_enter();
        assert_eq!(s.activity_focus, None);
    }

    // Feature: tui-wiring, Property 10: Output Buffer Cap Invariant
    // **Validates: Requirements 6.6**

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property 10a: For any sequence of output line additions (0..=15000),
        /// the output buffer length SHALL never exceed 10,000.
        #[test]
        fn prop_output_buffer_never_exceeds_cap(
            count in 0usize..=15000
        ) {
            let mut state = test_state();
            for i in 0..count {
                state.push_output(format!("l{i}"), OutputLineKind::Agent);
                prop_assert!(
                    state.output.len() <= AppState::OUTPUT_BUFFER_CAP,
                    "Buffer length {} exceeds cap {} after push",
                    state.output.len(),
                    AppState::OUTPUT_BUFFER_CAP
                );
            }
        }

        /// Property 10b: When the buffer is at capacity and a new line is added,
        /// the oldest line SHALL be evicted (buffer stays at cap).
        #[test]
        fn prop_output_buffer_evicts_oldest(
            extra_lines in 1usize..=5000
        ) {
            let mut state = test_state();

            // Fill buffer to capacity with numbered lines
            for i in 0..AppState::OUTPUT_BUFFER_CAP {
                state.push_output(format!("line-{i}"), OutputLineKind::Agent);
            }
            prop_assert_eq!(state.output.len(), AppState::OUTPUT_BUFFER_CAP);

            // Push extra lines beyond capacity
            for i in 0..extra_lines {
                state.push_output(format!("extra-{i}"), OutputLineKind::Agent);
            }

            // Buffer must still be at cap
            prop_assert_eq!(state.output.len(), AppState::OUTPUT_BUFFER_CAP);

            // The oldest lines should have been evicted — the first line in the
            // buffer should be "line-{extra_lines}" (the original lines shifted out)
            let expected_first = format!("line-{extra_lines}");
            prop_assert_eq!(
                &state.output[0].text,
                &expected_first,
                "Expected first line to be '{}', got '{}'",
                expected_first,
                state.output[0].text
            );
        }
    }

    // Feature: tui-wiring, Property 11: Auto-Scroll Invariant
    // **Validates: Requirements 6.3, 6.4**

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property 11a: When scroll_offset == 0 (auto-scroll active), pushing a
        /// new output line SHALL keep scroll_offset at 0.
        #[test]
        fn prop_auto_scroll_stays_at_zero(
            count in 1usize..=500
        ) {
            let mut state = test_state();
            // Ensure auto-scroll is active
            state.scroll_offset = 0;
            state.auto_scroll = true;

            for i in 0..count {
                state.push_output(format!("l{i}"), OutputLineKind::Agent);
                prop_assert_eq!(
                    state.scroll_offset, 0,
                    "scroll_offset should remain 0 when auto-scroll is active"
                );
            }
        }

        /// Property 11b: When scroll_offset transitions from non-zero back to 0,
        /// auto_scroll SHALL be re-enabled.
        #[test]
        fn prop_auto_scroll_re_enabled_on_return_to_zero(
            initial_offset in 1u16..=500
        ) {
            let mut state = test_state();

            // Simulate user scrolling up (auto_scroll disabled)
            state.scroll_offset = initial_offset;
            state.auto_scroll = false;

            // Simulate user scrolling back to bottom
            state.scroll_offset = 0;
            state.auto_scroll = true;

            // Now push a new line — scroll_offset should stay at 0
            state.push_output("new line".to_string(), OutputLineKind::Agent);
            prop_assert_eq!(
                state.scroll_offset, 0,
                "scroll_offset should remain 0 after re-enabling auto-scroll"
            );
            prop_assert!(
                state.auto_scroll,
                "auto_scroll should remain true after returning to offset 0"
            );
        }
    }

    // Feature: tui-layout-redesign, Property 2: Sidebar toggle flips visibility
    // **Validates: Requirements 2.1**

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// For any current sidebar visibility state (true or false), applying the
        /// Tab toggle action SHALL produce the opposite visibility state and set
        /// sidebar_manual_toggle to true.
        #[test]
        fn prop_sidebar_toggle_flips_visibility(
            initial_visible in proptest::bool::ANY
        ) {
            let mut state = test_state();
            state.sidebar_visible = initial_visible;

            state.toggle_sidebar();

            prop_assert_eq!(
                state.sidebar_visible, !initial_visible,
                "Expected sidebar_visible to flip from {} to {}, got {}",
                initial_visible, !initial_visible, state.sidebar_visible
            );
            prop_assert!(
                state.sidebar_manual_toggle,
                "sidebar_manual_toggle should be true after toggle"
            );
        }
    }

    // ── Mode toggle unit tests ─────────────────────────────────────────────
    // **Validates: Requirements 9.1, 9.2, 9.3**

    #[test]
    fn toggle_mode_plan_to_yolo() {
        let mut state = test_state(); // starts in Plan
        assert_eq!(state.mode, Mode::Plan);

        state.toggle_mode(); // Plan → Auto (3-mode cycle)

        assert_eq!(state.mode, Mode::Auto);
        // Should have pushed an info toast
        assert_eq!(state.toasts.len(), 1);
        let toast = state.toasts.back().unwrap_or_else(|| unreachable!());
        assert_eq!(toast.kind, ToastKind::Info);
        assert_eq!(toast.message, "Mode: AUTO");
    }

    #[test]
    fn toggle_mode_yolo_to_plan() {
        let mut state = test_state();
        state.mode = Mode::Yolo;

        state.toggle_mode();

        assert_eq!(state.mode, Mode::Plan);
        assert_eq!(state.toasts.len(), 1);
        let toast = state.toasts.back().unwrap_or_else(|| unreachable!());
        assert_eq!(toast.kind, ToastKind::Info);
        assert_eq!(toast.message, "Mode: PLAN");
    }

    #[test]
    fn toggle_mode_triple_toggle_returns_to_original() {
        let mut state = test_state();
        let original = state.mode.clone();

        // 3-mode cycle (Auto→Yolo→Plan→Auto): three toggles return to origin.
        state.toggle_mode();
        state.toggle_mode();
        state.toggle_mode();

        assert_eq!(state.mode, original);
        assert_eq!(state.toasts.len(), 3);
    }

    // ── Scroll offset unit tests ───────────────────────────────────────────

    #[test]
    fn scroll_max_offset_empty_buffer() {
        let state = test_state();
        assert_eq!(state.scroll_max_offset(20), 0);
    }

    #[test]
    fn scroll_max_offset_fewer_lines_than_visible() {
        let mut state = test_state();
        for i in 0..5 {
            state.push_output(format!("line {i}"), OutputLineKind::Agent);
        }
        // 5 lines, visible_height=20 → max offset = 0
        assert_eq!(state.scroll_max_offset(20), 0);
    }

    #[test]
    fn scroll_max_offset_more_lines_than_visible() {
        let mut state = test_state();
        for i in 0..100 {
            state.push_output(format!("line {i}"), OutputLineKind::Agent);
        }
        // 100 lines, visible_height=20 → max offset = 80
        assert_eq!(state.scroll_max_offset(20), 80);
    }

    #[test]
    fn scroll_up_clamps_to_max() {
        let mut state = test_state();
        for i in 0..50 {
            state.push_output(format!("line {i}"), OutputLineKind::Agent);
        }
        // 50 lines, visible_height=20 → max offset = 30
        state.scroll_offset = 25;
        state.auto_scroll = false;
        state.scroll_up(10, 20); // 25 + 10 = 35, clamped to 30
        assert_eq!(state.scroll_offset, 30);
        assert!(!state.auto_scroll);
    }

    #[test]
    fn scroll_down_clamps_to_zero_and_enables_auto_scroll() {
        let mut state = test_state();
        state.scroll_offset = 5;
        state.auto_scroll = false;
        state.scroll_down(10); // 5 - 10 saturates to 0
        assert_eq!(state.scroll_offset, 0);
        assert!(state.auto_scroll);
    }

    #[test]
    fn scroll_down_partial() {
        let mut state = test_state();
        state.scroll_offset = 10;
        state.auto_scroll = false;
        state.scroll_down(3); // 10 - 3 = 7
        assert_eq!(state.scroll_offset, 7);
        assert!(!state.auto_scroll);
    }

    #[test]
    fn compute_scroll_offset_positive_delta_clamped() {
        // 100 lines, visible 20 → max = 80
        // current=75, delta=+10 → 85, clamped to 80
        let result = compute_scroll_offset(75, 10, 100, 20);
        assert_eq!(result, 80);
    }

    #[test]
    fn compute_scroll_offset_negative_delta_clamped_to_zero() {
        // current=5, delta=-10 → saturates to 0
        let result = compute_scroll_offset(5, -10, 100, 20);
        assert_eq!(result, 0);
    }

    #[test]
    fn compute_scroll_offset_no_scrollable_content() {
        // 10 lines, visible 20 → max = 0
        // Any delta should result in 0
        let result = compute_scroll_offset(0, 5, 10, 20);
        assert_eq!(result, 0);
    }

    #[test]
    fn compute_scroll_offset_delta_zero() {
        // current=15, delta=0 → stays at 15 (within max=80)
        let result = compute_scroll_offset(15, 0, 100, 20);
        assert_eq!(result, 15);
    }

    #[test]
    fn compute_scroll_offset_i16_min_edge_case() {
        // delta = i16::MIN (-32768) — tests unsigned_abs handling
        let result = compute_scroll_offset(100, i16::MIN, 1000, 20);
        // 100 - 32768 saturates to 0
        assert_eq!(result, 0);
    }

    // Feature: tui-layout-redesign, Property 11: Scroll offset stays within bounds
    // **Validates: Requirements 6.11**

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// For any current scroll offset, total output line count, visible height,
        /// and scroll increment (1–10), applying a scroll action SHALL produce a
        /// new offset clamped to [0, max(0, total_lines - visible_height)].
        #[test]
        fn prop_scroll_offset_stays_within_bounds(
            current in 0u16..1000,
            delta in -100i16..100,
            total_lines in 0usize..20000,
            visible_height in 1u16..200,
        ) {
            let result = compute_scroll_offset(current, delta, total_lines, visible_height);

            let max_offset = total_lines.saturating_sub(visible_height as usize);

            prop_assert!(
                result as usize <= max_offset,
                "Scroll offset {} exceeds max {} (total_lines={}, visible_height={})",
                result, max_offset, total_lines, visible_height
            );
        }
    }

    // Feature: tui-layout-redesign, Property 13: Mode toggle is an involution
    // **Validates: Requirements 9.1**

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Mode toggle is a 3-cycle Auto → Yolo → Plan → Auto. Toggling three
        /// times from any starting mode returns to the original.
        #[test]
        fn prop_mode_toggle_is_three_cycle(
            start in 0u8..3
        ) {
            let mut state = test_state();
            let initial_mode = match start {
                0 => Mode::Auto,
                1 => Mode::Yolo,
                _ => Mode::Plan,
            };
            state.mode = initial_mode.clone();

            // One toggle follows the cycle.
            state.toggle_mode();
            let expected = match initial_mode {
                Mode::Auto => Mode::Yolo,
                Mode::Yolo => Mode::Plan,
                Mode::Plan => Mode::Auto,
            };
            prop_assert_eq!(&state.mode, &expected);

            // Three toggles total return to the origin.
            state.toggle_mode();
            state.toggle_mode();
            prop_assert_eq!(&state.mode, &initial_mode);
        }
    }
}
