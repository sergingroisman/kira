#![forbid(unsafe_code)]

//! Data models for the TUI layout redesign.
//!
//! These structures support the sidebar, chat panel, input area, overlays,
//! and HUD layers. They are referenced by `AppState` and rendered by the
//! widget tree.

use crossterm::event::KeyEvent;

// ── Budget & Provider ──────────────────────────────────────────────────────

/// Budget information for sidebar display.
#[derive(Debug, Clone)]
pub struct BudgetInfo {
    pub tokens_used: u64,
    pub usage_pct: u8,
    pub cost_usd: f64,
    pub max_budget_usd: f64,
}

impl Default for BudgetInfo {
    fn default() -> Self {
        Self {
            tokens_used: 0,
            usage_pct: 0,
            cost_usd: 0.0,
            max_budget_usd: 0.0,
        }
    }
}

/// Provider connection status.
#[derive(Debug, Clone)]
pub struct ProviderStatus {
    pub name: String,
    pub status: ConnectionStatus,
}

/// Connection state for a provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
    Unknown,
}

// ── SDD Pipeline ───────────────────────────────────────────────────────────

/// SDD pipeline phase tracking.
#[derive(Debug, Clone)]
pub struct SddPipelineState {
    pub current_phase: SddPhase,
}

/// The four SDD phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SddPhase {
    Specify,
    Design,
    Tasks,
    Execute,
}

// ── Waves ──────────────────────────────────────────────────────────────────

/// Wave execution state for sidebar display.
#[derive(Debug, Clone)]
pub struct WaveState {
    pub wave_number: u16,
    pub status: WaveStatus,
    pub completed: u16,
    pub total: u16,
}

/// Status of a wave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaveStatus {
    Pending,
    InProgress,
    Complete,
}

// ── Modified Files ─────────────────────────────────────────────────────────

/// Modified file entry for sidebar display.
#[derive(Debug, Clone)]
pub struct ModifiedFile {
    pub path: String,
    pub insertions: u32,
    pub deletions: u32,
}

// ── Chat Messages ──────────────────────────────────────────────────────────

/// Rich chat message (replaces flat `OutputLine` for the new renderer).
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub id: String,
    pub kind: MessageKind,
    pub content: MessageContent,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub agent_short_id: Option<String>,
}

/// The origin/type of a chat message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageKind {
    User,
    Agent,
    System,
    Error,
}

/// Content variants for a chat message.
#[derive(Debug, Clone)]
pub enum MessageContent {
    Plain(String),
    Markdown(String),
    ToolCall(ToolCallKind),
    Thinking { content: String, collapsed: bool },
    Streaming { partial: String },
}

/// Type-specific tool call data for rendering.
#[derive(Debug, Clone)]
pub enum ToolCallKind {
    Bash { command: String, output: String },
    FileEdit { path: String, diff: String },
    FileRead { path: String, line_count: usize },
    Unknown { tool_type: String, content: String },
}

// ── Activity Timeline ──────────────────────────────────────────────────────

/// A structured agent action shown in the kiro-style activity timeline.
///
/// Derived from `AgentOutput` lines via heuristic parsing (CLI-only fallback)
/// or from typed Core SSE events when available.
#[derive(Debug, Clone)]
pub struct ActivityItem {
    /// Unicode icon glyph for this action type.
    pub icon: &'static str,
    /// Short verb describing the action (e.g. "Read", "Creating", "Ran").
    pub verb: &'static str,
    /// Primary target — filename, query, or command (truncated for display).
    pub target: String,
    /// Optional subtitle shown below the main chip row (e.g. search query).
    pub subtitle: Option<String>,
    /// Completion state of this action.
    pub status: ActivityStatus,
    /// Expandable detail body (Kiro-style ▶/▼). Empty = no expand affordance.
    pub detail: Vec<String>,
    /// Whether the detail is currently hidden (collapsed).
    pub collapsed: bool,
}

/// Completion state of an activity item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivityStatus {
    /// Spinner shown — action is in progress.
    Running,
    /// Green tick — action completed successfully.
    Done,
    /// Red cross — action failed.
    Error,
}

// ── Completion ─────────────────────────────────────────────────────────────

/// Completion popup state.
#[derive(Debug, Clone)]
pub struct CompletionState {
    pub kind: CompletionKind,
    pub items: Vec<CompletionItem>,
    pub selected: usize,
    pub filter: String,
}

/// What kind of completion is active.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionKind {
    SlashCommand,
    FileMention,
}

/// A single completion item.
#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub label: String,
    pub description: Option<String>,
}

// ── Which-Key ──────────────────────────────────────────────────────────────

/// Which-key panel state (shows available continuations after a prefix key).
#[derive(Debug, Clone)]
pub struct WhichKeyState {
    pub prefix: Vec<KeyEvent>,
    pub continuations: Vec<KeyContinuation>,
    pub shown_at: std::time::Instant,
}

/// A single continuation entry in the which-key panel.
#[derive(Debug, Clone)]
pub struct KeyContinuation {
    pub key: String,
    pub description: String,
}

// ── Overlay States ─────────────────────────────────────────────────────────

/// Command palette state.
#[derive(Debug, Clone, Default)]
pub struct CommandPaletteState {
    pub filter: String,
    pub results: Vec<PaletteEntry>,
    pub selected: usize,
}

/// A single entry in the command palette results.
#[derive(Debug, Clone)]
pub struct PaletteEntry {
    pub command: String,
    pub description: String,
    pub score: u32,
}

/// Session picker state.
#[derive(Debug, Clone, Default)]
pub struct SessionPickerState {
    pub sessions: Vec<SessionEntry>,
    pub filter: String,
    pub selected: usize,
    pub show_all_workspaces: bool,
}

/// A single session entry in the session picker.
#[derive(Debug, Clone)]
pub struct SessionEntry {
    pub id: String,
    pub title: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub message_count: u32,
    pub cost_usd: f64,
}

/// Task browser state.
#[derive(Debug, Clone)]
pub struct TaskBrowserState {
    pub waves: Vec<TaskWave>,
    pub selected_task: Option<usize>,
    pub detail_scroll: u16,
    pub last_refresh: std::time::Instant,
}

impl Default for TaskBrowserState {
    fn default() -> Self {
        Self {
            waves: Vec::new(),
            selected_task: None,
            detail_scroll: 0,
            last_refresh: std::time::Instant::now(),
        }
    }
}

/// A wave grouping in the task browser.
#[derive(Debug, Clone)]
pub struct TaskWave {
    pub wave_number: u16,
    pub tasks: Vec<TaskEntry>,
}

/// A single task entry in the task browser.
#[derive(Debug, Clone)]
pub struct TaskEntry {
    pub id: String,
    pub title: String,
    pub status: TaskStatus,
    pub agent_short_id: Option<String>,
    pub elapsed: std::time::Duration,
    pub description: Option<String>,
    pub output: Vec<String>,
    pub error: Option<String>,
}

/// Status of a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

// ── Overlay Stack ──────────────────────────────────────────────────────────

/// The set of possible overlays that can be pushed onto the overlay stack.
#[derive(Debug, Clone)]
pub enum Overlay {
    CommandPalette(CommandPaletteState),
    SessionPicker(SessionPickerState),
    TaskBrowser(TaskBrowserState),
    Help,
    Approval,
}

/// A z-ordered stack of overlays. The top overlay captures input focus.
#[derive(Debug, Clone, Default)]
pub struct OverlayStack {
    stack: Vec<Overlay>,
}

impl OverlayStack {
    /// Create a new empty overlay stack.
    pub const fn new() -> Self {
        Self { stack: Vec::new() }
    }

    /// Push an overlay onto the top of the stack.
    pub fn push(&mut self, overlay: Overlay) {
        self.stack.push(overlay);
    }

    /// Pop the top overlay off the stack.
    pub fn pop(&mut self) -> Option<Overlay> {
        self.stack.pop()
    }

    /// Peek at the top overlay without removing it.
    pub fn top(&self) -> Option<&Overlay> {
        self.stack.last()
    }

    /// Peek at the top overlay mutably.
    pub fn top_mut(&mut self) -> Option<&mut Overlay> {
        self.stack.last_mut()
    }

    /// Returns `true` if no overlays are active.
    pub const fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_stack_push_pop() {
        let mut stack = OverlayStack::new();
        assert!(stack.is_empty());
        assert!(stack.top().is_none());

        stack.push(Overlay::Help);
        assert!(!stack.is_empty());
        assert!(matches!(stack.top(), Some(Overlay::Help)));

        stack.push(Overlay::CommandPalette(CommandPaletteState::default()));
        assert!(matches!(stack.top(), Some(Overlay::CommandPalette(_))));

        let popped = stack.pop();
        assert!(matches!(popped, Some(Overlay::CommandPalette(_))));
        assert!(matches!(stack.top(), Some(Overlay::Help)));

        let popped = stack.pop();
        assert!(matches!(popped, Some(Overlay::Help)));
        assert!(stack.is_empty());
    }

    #[test]
    fn overlay_stack_pop_empty_returns_none() {
        let mut stack = OverlayStack::new();
        assert!(stack.pop().is_none());
    }

    #[test]
    fn budget_info_default() {
        let info = BudgetInfo::default();
        assert_eq!(info.tokens_used, 0);
        assert_eq!(info.usage_pct, 0);
        assert!((info.cost_usd - 0.0).abs() < f64::EPSILON);
        assert!((info.max_budget_usd - 0.0).abs() < f64::EPSILON);
    }
}
