#![forbid(unsafe_code)]

//! Approval modal overlay — enhanced approval dialog with risk-level coloring,
//! countdown timer, and pending queue count.
//!
//! This module contains both the rendering logic (the enhanced approval modal)
//! and the approval interaction logic (key handling, feedback mode, queue management).
//!
//! Displays tool name, risk level badge, agent short ID, arguments preview,
//! countdown timer, and action footer. Border color reflects risk level:
//! red for CRITICAL, yellow for HIGH, default for others.
//!
//! Key handling:
//! - `y` → approve
//! - `n` → reject
//! - `f` → enter feedback mode (inline text input, max 500 chars)
//! - In feedback mode: text input, Enter to submit rejection with feedback, Esc to cancel

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::app::AppState;

/// Maximum length of feedback text input.
const MAX_FEEDBACK_LEN: usize = 500;

/// Maximum length of arguments preview displayed in the modal.
const MAX_ARGS_PREVIEW_LEN: usize = 2048;

/// Result of handling a key event in the approval modal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalAction {
    /// Operator approved the tool call.
    Approve { agent_id: String },
    /// Operator rejected the tool call (with optional feedback).
    Reject {
        agent_id: String,
        feedback: Option<String>,
    },
    /// Key was consumed by the modal (no action to send to Core).
    Consumed,
    /// Key was not handled by the modal (pass through).
    NotHandled,
}

/// Render the approval modal overlay into the given area.
///
/// Displays the currently active approval request with risk-level styling,
/// countdown timer, and pending queue count. If no approval is active,
/// renders nothing (early return).
pub fn render_approval(frame: &mut Frame, state: &AppState, area: Rect) {
    let Some(request) = state.approval_active.as_ref() else {
        return;
    };

    // Clear the area behind the overlay
    frame.render_widget(Clear, area);

    // Determine border color based on risk level
    let border_color = match request.risk_level.as_str() {
        "CRITICAL" => Color::Red,
        "HIGH" => Color::Yellow,
        _ => Color::Cyan,
    };

    // Build title with optional pending count
    let pending_count = state.approval_queue.len();
    let title = if pending_count > 0 {
        format!(" Approval Required [+{pending_count} pending] ")
    } else {
        " Approval Required ".to_string()
    };

    let block = Block::default()
        .title(
            Line::from(title).style(
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    // Compute inner area for content
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // If inner area is too small, bail out
    if inner.height < 3 || inner.width < 10 {
        return;
    }

    // Build content lines
    let mut lines: Vec<Line<'static>> = Vec::new();

    // Blank line for spacing
    lines.push(Line::from(""));

    // Action summary — Claude-Code-style one-liner (✎ Write path / $ cmd / …)
    // instead of raw "Tool:" + cryptic args.
    let action_width = inner.width.saturating_sub(4) as usize;
    let action = action_summary(&request.tool_name, &request.arguments_preview);
    let action = if action.chars().count() > action_width.max(8) {
        let kept: String = action.chars().take(action_width.saturating_sub(1)).collect();
        format!("{kept}…")
    } else {
        action
    };
    lines.push(Line::from(Span::styled(
        format!("  {action}"),
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
    )));

    // Risk badge + dim agent id on one line.
    let (badge_foreground, badge_background) = match request.risk_level.as_str() {
        "CRITICAL" => (Color::White, Color::Red),
        "HIGH" => (Color::Black, Color::Yellow),
        _ => (Color::White, Color::DarkGray),
    };
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            format!(" {} ", request.risk_level),
            Style::default()
                .fg(badge_foreground)
                .bg(badge_background)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  agent {}", request.short_agent_id()),
            Style::default().fg(Color::DarkGray),
        ),
    ]));

    // Blank separator
    lines.push(Line::from(""));

    // File-changing tools (file_write / edit) get a git-style colored diff so
    // the operator sees exactly what will change before approving. Other tools
    // (bash, etc.) fall back to a plain arguments preview.
    let diff_budget = (inner.height as usize).saturating_sub(12).max(3);
    let diff_width = (inner.width as usize).saturating_sub(2).max(20);
    if let Some(diff_lines) = diff_preview_lines(
        &request.tool_name,
        &request.arguments_preview,
        diff_budget,
        diff_width,
    ) {
        lines.push(Line::from(Span::styled(
            "  Mudanças:",
            Style::default().fg(Color::Gray),
        )));
        lines.extend(diff_lines);
    } else {
        let max_preview_width = inner.width.saturating_sub(4) as usize;
        let args_preview = if request.arguments_preview.len() > max_preview_width {
            let truncated: String = request
                .arguments_preview
                .chars()
                .take(max_preview_width.saturating_sub(3))
                .collect();
            format!("{truncated}...")
        } else {
            request.arguments_preview.clone()
        };
        lines.push(Line::from(vec![
            Span::styled("  Args: ", Style::default().fg(Color::Gray)),
            Span::styled(args_preview, Style::default().fg(Color::White)),
        ]));
    }

    // Blank separator
    lines.push(Line::from(""));

    // Countdown as a depleting health bar.
    let remaining = request.seconds_remaining();
    let bar_width = (inner.width.saturating_sub(10) as usize).clamp(8, 40);
    lines.push(timer_bar(remaining, request.timeout_secs, bar_width));

    // Blank separator
    lines.push(Line::from(""));

    // Footer with key actions
    lines.push(Line::from(vec![
        Span::styled(
            "  [y]",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" approve  ", Style::default().fg(Color::Gray)),
        Span::styled(
            "[n]",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" reject  ", Style::default().fg(Color::Gray)),
        Span::styled(
            "[f]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" feedback", Style::default().fg(Color::Gray)),
    ]));

    // Split inner area: content takes all available space
    let content_chunks = Layout::vertical([Constraint::Min(1)]).split(inner);

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });

    frame.render_widget(paragraph, content_chunks[0]);
}

/// Parse the (possibly double-encoded) tool arguments JSON. The Core sends
/// `serde_json::to_string(arguments)`, which can yield a JSON string wrapping
/// the real object — unwrap one level if so.
fn parse_tool_args(raw: &str) -> Option<serde_json::Value> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    match v {
        serde_json::Value::String(s) => serde_json::from_str(&s).ok(),
        other => Some(other),
    }
}

/// Build a Claude-Code-style one-line action summary from the tool + args,
/// e.g. `✎ Write  .kira/runs/…/spec.md`, `$ bun -e …`, `🌐 Fetch  https://…`.
/// Falls back to the bare tool name when args can't be parsed.
fn action_summary(tool: &str, args_raw: &str) -> String {
    let parsed = parse_tool_args(args_raw);
    let field = |key: &str| -> Option<String> {
        parsed
            .as_ref()
            .and_then(|v| v.get(key).and_then(|x| x.as_str()).map(str::to_string))
            .or_else(|| lenient_str_field(args_raw, key))
    };
    match tool {
        "file_write" => format!("✎ Write  {}", field("path").unwrap_or_else(|| "?".into())),
        "edit" => format!("✎ Edit  {}", field("path").unwrap_or_else(|| "?".into())),
        "file_read" => format!("📖 Read  {}", field("path").unwrap_or_else(|| "?".into())),
        "web_fetch" => format!("🌐 Fetch  {}", field("url").unwrap_or_else(|| "?".into())),
        "web_search" => format!("🔎 Search  {}", field("query").unwrap_or_else(|| "?".into())),
        "bash" => {
            // argv is an array; join for a shell-like preview.
            let argv = parsed
                .as_ref()
                .and_then(|v| v.get("argv").and_then(|a| a.as_array()))
                .map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                });
            format!("$ {}", argv.unwrap_or_else(|| "(comando)".into()))
        }
        other => other.to_string(),
    }
}

/// Render a depleting "health bar" for the approval countdown. Full + green
/// when fresh, shrinks + turns yellow/red as the deadline approaches.
fn timer_bar(remaining: u64, total: u64, width: usize) -> Line<'static> {
    let width = width.max(8);
    let ratio = if total == 0 { 0.0 } else { remaining as f64 / total as f64 };
    let filled = ((ratio * width as f64).round() as usize).min(width);
    let empty = width - filled;
    let color = if remaining <= 10 {
        Color::Red
    } else if remaining <= 30 {
        Color::Yellow
    } else {
        Color::Green
    };
    Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled("█".repeat(filled), Style::default().fg(color)),
        Span::styled("░".repeat(empty), Style::default().fg(Color::DarkGray)),
        Span::styled(format!("  {remaining}s"), Style::default().fg(color)),
    ])
}

/// Leniently extract a string field's value from a raw (possibly truncated or
/// malformed) JSON args blob. Used as a fallback when `parse_tool_args` fails —
/// e.g. the Core truncated a large `content` field mid-string, leaving invalid
/// JSON. Without this the modal would dump the raw escaped blob (`{\"path\":…`).
///
/// Scans for `"<key>"` then the opening quote of its value, then decodes the
/// string body (handling `\"`, `\\`, `\n`, `\t`, `\r`, `\/`) until the closing
/// unescaped quote or end of input (tolerating truncation).
fn lenient_str_field(raw: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let after_key = &raw[raw.find(&needle)? + needle.len()..];
    // Skip whitespace + the ':' + whitespace, then require the opening quote.
    let rest = after_key.trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let body = rest.strip_prefix('"')?;

    let mut out = String::new();
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out), // closing quote
            '\\' => match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('/') => out.push('/'),
                Some(other) => out.push(other), // \uXXXX etc. — best effort
                None => break,                  // truncated mid-escape
            },
            _ => out.push(c),
        }
    }
    // Reached end without a closing quote → truncated, but return what we got.
    Some(out)
}

/// Build a git-style colored diff preview for file-changing tools.
/// `file_write` → all-green new/overwritten content; `edit` → red old / green
/// new. Returns `None` for tools without a meaningful diff (e.g. bash).
fn diff_preview_lines(
    tool: &str,
    args_raw: &str,
    max_lines: usize,
    max_width: usize,
) -> Option<Vec<Line<'static>>> {
    // Only file-changing tools get a diff.
    if tool != "file_write" && tool != "edit" {
        return None;
    }
    // Prefer a clean parse; fall back to lenient field extraction when the
    // args blob is truncated/malformed (large `content` cut by the Core).
    let parsed = parse_tool_args(args_raw);
    let field = |key: &str| -> String {
        parsed
            .as_ref()
            .and_then(|v| v.get(key).and_then(|x| x.as_str()).map(str::to_string))
            .or_else(|| lenient_str_field(args_raw, key))
            .unwrap_or_default()
    };

    // Char-safe horizontal truncation (UTF-8: never byte-slice arbitrary text).
    // Reserve 1 col for the diff prefix (+/-).
    let body_w = max_width.saturating_sub(1).max(8);
    let trunc = |l: &str| -> String {
        if l.chars().count() > body_w {
            let kept: String = l.chars().take(body_w.saturating_sub(1)).collect();
            format!("{kept}…")
        } else {
            l.to_string()
        }
    };

    let text = match tool {
        "file_write" => {
            let path = field("path");
            let path = if path.is_empty() { "arquivo".to_string() } else { path };
            let content = field("content");
            let mut s = format!("+++ {}\n", trunc(&format!("{path} (novo / sobrescrito)")));
            for l in content.lines() {
                s.push_str(&format!("+{}\n", trunc(l)));
            }
            s
        }
        "edit" => {
            let path = field("path");
            let path = if path.is_empty() { "arquivo".to_string() } else { path };
            let old = field("old_string");
            let new = field("new_string");
            let mut s = format!("--- {0}\n+++ {0}\n", trunc(&path));
            for l in old.lines() {
                s.push_str(&format!("-{}\n", trunc(l)));
            }
            for l in new.lines() {
                s.push_str(&format!("+{}\n", trunc(l)));
            }
            s
        }
        _ => return None,
    };

    let mut lines = crate::ui::chat::markdown::render_diff_block(&text);
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        lines.push(Line::from(Span::styled(
            "  … (diff truncado)",
            Style::default().fg(Color::DarkGray),
        )));
    }
    Some(lines)
}

/// Emit a terminal bell character (BEL, 0x07) to alert the operator.
///
/// Called when a new approval request arrives. This writes the bell character
/// directly to stdout. If the write fails, it is silently ignored (non-critical).
pub fn ring_terminal_bell() {
    use std::io::Write;
    let _ = std::io::stdout().write_all(b"\x07");
    let _ = std::io::stdout().flush();
}

// ── Approval interaction logic ─────────────────────────────────────────────

/// Handle a key event while the approval modal is active.
///
/// Returns an `ApprovalAction` indicating what happened:
/// - `Approve` / `Reject`: the operator made a decision, send to Core and resolve
/// - `Consumed`: the key was handled (e.g., entering feedback mode, typing)
/// - `NotHandled`: no approval modal is active
///
/// The caller is responsible for:
/// 1. Sending the approval/rejection to Core via `CoreClient::agent_approve()`
/// 2. Calling `resolve_approval(state)` to clear the active request and dequeue next
pub fn handle_approval_key(state: &mut AppState, key_code: char) -> ApprovalAction {
    let Some(request) = &state.approval_active else {
        return ApprovalAction::NotHandled;
    };

    if state.approval_feedback_input.is_some() {
        // In feedback mode — regular chars are handled by handle_approval_feedback_input
        return ApprovalAction::NotHandled;
    }

    let agent_id = request.agent_id.clone();

    match key_code {
        'y' | 'Y' => ApprovalAction::Approve { agent_id },
        'n' | 'N' => ApprovalAction::Reject {
            agent_id,
            feedback: None,
        },
        'f' | 'F' => {
            // Enter feedback mode
            state.approval_feedback_input = Some(String::new());
            ApprovalAction::Consumed
        }
        _ => ApprovalAction::Consumed, // Consume all other keys while modal is active
    }
}

/// Handle a character input in feedback mode.
///
/// Returns `None` if not in feedback mode. Otherwise appends the character
/// (up to MAX_FEEDBACK_LEN) and returns `Some(Consumed)`.
pub fn handle_approval_feedback_char(state: &mut AppState, c: char) -> Option<ApprovalAction> {
    let feedback = state.approval_feedback_input.as_mut()?;
    if feedback.len() < MAX_FEEDBACK_LEN {
        feedback.push(c);
    }
    Some(ApprovalAction::Consumed)
}

/// Handle backspace in feedback mode.
pub fn handle_approval_feedback_backspace(state: &mut AppState) -> Option<ApprovalAction> {
    let feedback = state.approval_feedback_input.as_mut()?;
    feedback.pop();
    Some(ApprovalAction::Consumed)
}

/// Handle Enter in feedback mode — submit rejection with feedback.
///
/// Returns `Some(Reject { ... })` with the feedback text, or `None` if not in feedback mode.
pub fn handle_approval_feedback_submit(state: &mut AppState) -> Option<ApprovalAction> {
    let Some(request) = &state.approval_active else {
        return None;
    };
    let feedback_text = state.approval_feedback_input.take()?;
    let agent_id = request.agent_id.clone();
    let feedback = if feedback_text.is_empty() {
        None
    } else {
        Some(feedback_text)
    };
    Some(ApprovalAction::Reject { agent_id, feedback })
}

/// Handle Esc in feedback mode — cancel feedback and return to normal modal.
pub fn handle_approval_feedback_cancel(state: &mut AppState) -> Option<ApprovalAction> {
    if state.approval_feedback_input.is_some() {
        state.approval_feedback_input = None;
        Some(ApprovalAction::Consumed)
    } else {
        None
    }
}

/// Resolve the current approval (approve/reject/timeout) and dequeue the next request.
///
/// Clears `approval_active` and `approval_feedback_input`, then pops the next
/// request from `approval_queue` into `approval_active`.
pub fn resolve_approval(state: &mut AppState) {
    state.approval_active = state.approval_queue.pop_front();
    state.approval_feedback_input = None;
    // No more pending approvals → close the modal overlay and clear the
    // Security-blocked indicator. If another is queued, keep the overlay up.
    if state.approval_active.is_none() {
        if matches!(state.overlay_stack.top(), Some(crate::models::Overlay::Approval)) {
            state.overlay_stack.pop();
        }
        state.layers_state.clear_security_blocked();
    }
}

/// Check if the active approval has expired and return the agent_id if so.
///
/// The caller should auto-reject (send POST with approved=false, feedback=null)
/// and then call `resolve_approval(state)`.
pub fn check_approval_timeout(state: &AppState) -> Option<String> {
    state
        .approval_active
        .as_ref()
        .filter(|req| req.is_expired())
        .map(|req| req.agent_id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};
    use tokio::time::Instant;

    use crate::app::{AppState, Mode, Tier};
    use crate::config::Config;
    use crate::messages::ApprovalRequest;

    /// Helper to create a test AppState with an active approval request.
    fn state_with_approval(risk_level: &str, pending_count: usize) -> AppState {
        let mut state = AppState::new(Config::default(), Mode::Plan, Tier::Standard);
        state.approval_active = Some(ApprovalRequest {
            agent_id: "abcdef12-3456-7890-abcd-ef1234567890".to_string(),
            tool_name: "bash".to_string(),
            risk_level: risk_level.to_string(),
            arguments_preview: r#"{"argv":["rm","-rf","/tmp/workspace"]}"#.to_string(),
            timeout_secs: 30,
            received_at: Instant::now(),
        });
        // Add pending requests to the queue
        for _ in 0..pending_count {
            state.approval_queue.push_back(ApprovalRequest {
                agent_id: "00000000-0000-0000-0000-000000000000".to_string(),
                tool_name: "file_write".to_string(),
                risk_level: "HIGH".to_string(),
                arguments_preview: "...".to_string(),
                timeout_secs: 30,
                received_at: Instant::now(),
            });
        }
        state
    }

    #[test]
    fn render_approval_no_active_request_is_noop() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|_| unreachable!());
        let state = AppState::new(Config::default(), Mode::Plan, Tier::Standard);

        terminal
            .draw(|frame| {
                let area = frame.area();
                render_approval(frame, &state, area);
            })
            .unwrap_or_else(|_| unreachable!());

        // Should not panic and buffer should be empty (no widgets rendered)
        let buf = terminal.backend().buffer().clone();
        // All cells should be default (space)
        for cell in buf.content().iter() {
            assert_eq!(cell.symbol(), " ");
        }
    }

    #[test]
    fn render_approval_critical_shows_red_border() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|_| unreachable!());
        let state = state_with_approval("CRITICAL", 0);

        terminal
            .draw(|frame| {
                let area = frame.area();
                render_approval(frame, &state, area);
            })
            .unwrap_or_else(|_| unreachable!());

        // Check that the border cells have red foreground
        let buf = terminal.backend().buffer().clone();
        // Top-left corner should be a border character with red fg
        let top_left = &buf[(0, 0)];
        assert_eq!(top_left.fg, Color::Red);
    }

    #[test]
    fn render_approval_high_shows_yellow_border() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|_| unreachable!());
        let state = state_with_approval("HIGH", 0);

        terminal
            .draw(|frame| {
                let area = frame.area();
                render_approval(frame, &state, area);
            })
            .unwrap_or_else(|_| unreachable!());

        let buf = terminal.backend().buffer().clone();
        let top_left = &buf[(0, 0)];
        assert_eq!(top_left.fg, Color::Yellow);
    }

    #[test]
    fn render_approval_shows_pending_count_in_title() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|_| unreachable!());
        let state = state_with_approval("HIGH", 3);

        terminal
            .draw(|frame| {
                let area = frame.area();
                render_approval(frame, &state, area);
            })
            .unwrap_or_else(|_| unreachable!());

        // Check that the buffer contains the pending count text
        let buf = terminal.backend().buffer().clone();
        let content: String = (0..buf.area.width)
            .map(|x| buf[(x, 0)].symbol().to_string())
            .collect();
        assert!(
            content.contains("+3 pending"),
            "Title should contain '+3 pending', got: {content}"
        );
    }

    #[test]
    fn render_approval_shows_tool_name() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|_| unreachable!());
        let state = state_with_approval("CRITICAL", 0);

        terminal
            .draw(|frame| {
                let area = frame.area();
                render_approval(frame, &state, area);
            })
            .unwrap_or_else(|_| unreachable!());

        // The bash action summary ("$ rm -rf …") should appear.
        let buf = terminal.backend().buffer().clone();
        let mut found = false;
        for y in 0..buf.area.height {
            let row: String = (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect();
            if row.contains("rm -rf") {
                found = true;
                break;
            }
        }
        assert!(
            found,
            "bash action summary ('$ rm -rf …') should appear in the rendered modal"
        );
    }

    #[test]
    fn render_approval_shows_agent_short_id() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|_| unreachable!());
        let state = state_with_approval("HIGH", 0);

        terminal
            .draw(|frame| {
                let area = frame.area();
                render_approval(frame, &state, area);
            })
            .unwrap_or_else(|_| unreachable!());

        let buf = terminal.backend().buffer().clone();
        let mut found = false;
        for y in 0..buf.area.height {
            let row: String = (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect();
            if row.contains("abcdef12") {
                found = true;
                break;
            }
        }
        assert!(
            found,
            "Agent short ID 'abcdef12' should appear in the rendered modal"
        );
    }

    #[test]
    fn render_approval_shows_countdown() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|_| unreachable!());
        let state = state_with_approval("CRITICAL", 0);

        terminal
            .draw(|frame| {
                let area = frame.area();
                render_approval(frame, &state, area);
            })
            .unwrap_or_else(|_| unreachable!());

        let buf = terminal.backend().buffer().clone();
        let mut found = false;
        for y in 0..buf.area.height {
            let row: String = (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect();
            // Health bar shows remaining seconds ("30s") + bar glyphs.
            if row.contains("30s") || row.contains('█') || row.contains('░') {
                found = true;
                break;
            }
        }
        assert!(found, "Countdown health bar should appear in the rendered modal");
    }

    #[test]
    fn render_approval_shows_key_actions() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|_| unreachable!());
        let state = state_with_approval("HIGH", 0);

        terminal
            .draw(|frame| {
                let area = frame.area();
                render_approval(frame, &state, area);
            })
            .unwrap_or_else(|_| unreachable!());

        let buf = terminal.backend().buffer().clone();
        let mut found_approve = false;
        let mut found_reject = false;
        let mut found_feedback = false;
        for y in 0..buf.area.height {
            let row: String = (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect();
            if row.contains("approve") {
                found_approve = true;
            }
            if row.contains("reject") {
                found_reject = true;
            }
            if row.contains("feedback") {
                found_feedback = true;
            }
        }
        assert!(found_approve, "[y] approve should appear");
        assert!(found_reject, "[n] reject should appear");
        assert!(found_feedback, "[f] feedback should appear");
    }

    #[test]
    fn render_approval_small_area_does_not_panic() {
        let backend = TestBackend::new(5, 3);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|_| unreachable!());
        let state = state_with_approval("CRITICAL", 0);

        // Should not panic even with a tiny area
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_approval(frame, &state, area);
            })
            .unwrap_or_else(|_| unreachable!());
    }

    #[test]
    fn render_approval_long_args_truncated() {
        let backend = TestBackend::new(60, 20);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|_| unreachable!());
        let mut state = AppState::new(Config::default(), Mode::Plan, Tier::Standard);
        state.approval_active = Some(ApprovalRequest {
            agent_id: "abcdef12-3456-7890-abcd-ef1234567890".to_string(),
            tool_name: "bash".to_string(),
            risk_level: "HIGH".to_string(),
            arguments_preview: "a".repeat(200),
            timeout_secs: 30,
            received_at: Instant::now(),
        });

        // Should not panic with very long arguments
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_approval(frame, &state, area);
            })
            .unwrap_or_else(|_| unreachable!());
    }

    // ── Approval logic tests ───────────────────────────────────────────────

    fn make_request(timeout_secs: u64) -> ApprovalRequest {
        ApprovalRequest {
            agent_id: "abcdef12-3456-7890-abcd-ef1234567890".to_string(),
            tool_name: "bash".to_string(),
            risk_level: "HIGH".to_string(),
            arguments_preview: r#"{"command": "rm -rf ./build"}"#.to_string(),
            timeout_secs,
            received_at: Instant::now(),
        }
    }

    fn make_state_with_active_approval() -> AppState {
        let mut state = AppState::new(Config::default(), Mode::Plan, Tier::Standard);
        state.approval_active = Some(make_request(120));
        state
    }

    #[test]
    fn handle_y_approves() {
        let mut state = make_state_with_active_approval();
        let action = handle_approval_key(&mut state, 'y');
        assert_eq!(
            action,
            ApprovalAction::Approve {
                agent_id: "abcdef12-3456-7890-abcd-ef1234567890".to_string()
            }
        );
    }

    #[test]
    fn handle_n_rejects() {
        let mut state = make_state_with_active_approval();
        let action = handle_approval_key(&mut state, 'n');
        assert_eq!(
            action,
            ApprovalAction::Reject {
                agent_id: "abcdef12-3456-7890-abcd-ef1234567890".to_string(),
                feedback: None,
            }
        );
    }

    #[test]
    fn handle_f_enters_feedback_mode() {
        let mut state = make_state_with_active_approval();
        let action = handle_approval_key(&mut state, 'f');
        assert_eq!(action, ApprovalAction::Consumed);
        assert_eq!(state.approval_feedback_input, Some(String::new()));
    }

    #[test]
    fn feedback_mode_accepts_chars() {
        let mut state = make_state_with_active_approval();
        state.approval_feedback_input = Some(String::new());
        let action = handle_approval_feedback_char(&mut state, 'h');
        assert_eq!(action, Some(ApprovalAction::Consumed));
        assert_eq!(state.approval_feedback_input, Some("h".to_string()));
    }

    #[test]
    fn feedback_mode_respects_max_len() {
        let mut state = make_state_with_active_approval();
        state.approval_feedback_input = Some("x".repeat(MAX_FEEDBACK_LEN));
        handle_approval_feedback_char(&mut state, 'z');
        assert_eq!(
            state.approval_feedback_input.as_ref().map(|s| s.len()),
            Some(MAX_FEEDBACK_LEN)
        );
    }

    #[test]
    fn feedback_submit_returns_reject_with_text() {
        let mut state = make_state_with_active_approval();
        state.approval_feedback_input = Some("bad tool".to_string());
        let action = handle_approval_feedback_submit(&mut state);
        assert_eq!(
            action,
            Some(ApprovalAction::Reject {
                agent_id: "abcdef12-3456-7890-abcd-ef1234567890".to_string(),
                feedback: Some("bad tool".to_string()),
            })
        );
        assert_eq!(state.approval_feedback_input, None);
    }

    #[test]
    fn feedback_cancel_returns_to_normal_mode() {
        let mut state = make_state_with_active_approval();
        state.approval_feedback_input = Some("partial".to_string());
        let action = handle_approval_feedback_cancel(&mut state);
        assert_eq!(action, Some(ApprovalAction::Consumed));
        assert_eq!(state.approval_feedback_input, None);
    }

    #[test]
    fn resolve_dequeues_next() {
        let mut state = make_state_with_active_approval();
        let next_req = ApprovalRequest {
            agent_id: "next1234-5678-9012-3456-abcdefghijkl".to_string(),
            tool_name: "file_write".to_string(),
            risk_level: "CRITICAL".to_string(),
            arguments_preview: "{}".to_string(),
            timeout_secs: 60,
            received_at: Instant::now(),
        };
        state.approval_queue.push_back(next_req);

        resolve_approval(&mut state);

        assert!(state.approval_active.is_some());
        assert_eq!(
            state.approval_active.as_ref().map(|r| r.agent_id.as_str()),
            Some("next1234-5678-9012-3456-abcdefghijkl")
        );
        assert!(state.approval_queue.is_empty());
    }

    #[test]
    fn resolve_pops_approval_overlay_when_done() {
        use crate::models::Overlay;
        let mut state = make_state_with_active_approval();
        state.overlay_stack.push(Overlay::Approval);
        // No queued approvals → resolving should close the overlay.
        resolve_approval(&mut state);
        assert!(state.approval_active.is_none());
        assert!(
            state.overlay_stack.is_empty(),
            "Approval overlay must be popped so the modal disappears"
        );
    }

    #[test]
    fn resolve_keeps_overlay_when_more_queued() {
        use crate::models::Overlay;
        let mut state = make_state_with_active_approval();
        state.overlay_stack.push(Overlay::Approval);
        state.approval_queue.push_back(make_request(120));
        resolve_approval(&mut state);
        assert!(state.approval_active.is_some());
        assert!(matches!(state.overlay_stack.top(), Some(Overlay::Approval)));
    }

    #[test]
    fn resolve_clears_when_queue_empty() {
        let mut state = make_state_with_active_approval();
        resolve_approval(&mut state);
        assert!(state.approval_active.is_none());
        assert!(state.approval_queue.is_empty());
    }

    #[test]
    fn check_timeout_returns_none_when_not_expired() {
        let state = make_state_with_active_approval();
        assert_eq!(check_approval_timeout(&state), None);
    }

    #[test]
    fn diff_preview_file_write_emits_green_adds() {
        // Double-encoded (as the Core sends it).
        let raw = serde_json::to_string(r#"{"path":"a.txt","content":"l1\nl2"}"#).unwrap();
        let lines = diff_preview_lines("file_write", &raw, 50, 80).expect("file_write → diff");
        // header (+++ path) + 2 content add lines.
        assert_eq!(lines.len(), 3);
        // content lines carry the add background.
        assert_eq!(lines[1].spans[0].style.bg, Some(Color::Rgb(20, 60, 25)));
    }

    #[test]
    fn diff_preview_edit_has_del_and_add() {
        let raw = r#"{"path":"a.txt","old_string":"velho","new_string":"novo"}"#;
        let lines = diff_preview_lines("edit", raw, 50, 80).expect("edit → diff");
        let has_del = lines.iter().any(|l| l.spans[0].style.bg == Some(Color::Rgb(70, 25, 25)));
        let has_add = lines.iter().any(|l| l.spans[0].style.bg == Some(Color::Rgb(20, 60, 25)));
        assert!(has_del && has_add);
    }

    #[test]
    fn diff_preview_bash_is_none() {
        assert!(diff_preview_lines("bash", r#"{"command":"ls"}"#, 50, 80).is_none());
    }

    #[test]
    fn diff_preview_recovers_from_truncated_json() {
        // Large content cut mid-string by the Core → invalid JSON. Must still
        // render a clean diff (path + the partial content), not a raw blob.
        let raw = r#"{"path":".kira/runs/x/tasks.json","content":"{\"tasks\":[\"t1"#;
        let lines = diff_preview_lines("file_write", raw, 50, 80)
            .expect("truncated file_write should still diff");
        // header line shows the recovered path.
        let header = lines[0].spans.iter().map(|s| s.content.as_ref()).collect::<String>();
        assert!(header.contains("tasks.json"), "header was: {header}");
        assert!(lines.len() >= 2, "expected header + content line");
    }

    #[test]
    fn lenient_str_field_extracts_path() {
        let raw = r#"{"path":"a/b.rs","content":"x"}"#;
        assert_eq!(lenient_str_field(raw, "path").as_deref(), Some("a/b.rs"));
    }

    #[test]
    fn lenient_str_field_handles_truncation() {
        // No closing quote → returns what it has.
        let raw = r#"{"content":"line1\nline2 and then cut"#;
        let got = lenient_str_field(raw, "content").unwrap();
        assert!(got.starts_with("line1\nline2"), "got: {got:?}");
    }

    #[test]
    fn not_handled_when_no_modal() {
        let mut state = AppState::new(Config::default(), Mode::Plan, Tier::Standard);
        let action = handle_approval_key(&mut state, 'y');
        assert_eq!(action, ApprovalAction::NotHandled);
    }
}
