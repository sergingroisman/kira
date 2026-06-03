#![forbid(unsafe_code)]

//! Header bar widget — renders connection status, mode badge, tier badge,
//! model name, session cost, and approval count in a single line.

use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::{AppState, CoreStatus, Mode, Tier};
use crate::ui::theme;

/// Render the header bar into the given area.
///
/// The header is a single-line bar with a `DarkGray` background displaying:
/// 1. Connection status dot (green/red/yellow)
/// 2. Mode badge (`[PLAN]` or `[YOLO]`) in cyan+bold
/// 3. Tier badge (`[heavy]`, `[standard]`, `[light]`) in magenta
/// 4. Active model name
/// 5. Session cost in USD (e.g., `"$0.42"`)
/// 6. Approval count badge when pending (yellow+bold)
pub fn render_header(frame: &mut Frame, state: &AppState, area: Rect) {
    let mut spans: Vec<Span<'_>> = Vec::with_capacity(8);

    // 1. Connection status dot
    let conn_color = match &state.core_status {
        CoreStatus::Connected { .. } => theme::SUCCESS,
        CoreStatus::Disconnected => theme::DANGER,
        CoreStatus::Unknown => theme::WARNING,
    };
    spans.push(Span::styled("●", Style::default().fg(conn_color)));
    spans.push(Span::raw(" "));

    // 2. Mode pill
    let mode_label = match state.mode {
        Mode::Auto => "AUTO",
        Mode::Plan => "PLAN",
        Mode::Yolo => "YOLO",
    };
    spans.push(theme::pill(mode_label, theme::ON_FILL, theme::ACCENT));
    spans.push(Span::raw(" "));

    // 3. Tier pill
    let tier_label = match state.tier {
        Tier::Heavy => "heavy",
        Tier::Standard => "standard",
        Tier::Light => "light",
    };
    spans.push(theme::pill(tier_label, theme::ON_FILL, theme::ACCENT_ALT));
    spans.push(Span::raw(" "));

    // 4. Active model name (only if non-empty)
    if !state.active_model.is_empty() {
        spans.push(Span::styled(
            state.active_model.as_str(),
            Style::default().fg(theme::TEXT),
        ));
        spans.push(Span::raw(" "));
    }

    // 5. Session cost in USD
    spans.push(Span::styled(
        format!("${:.2}", state.session_cost_usd),
        Style::default().fg(theme::TEXT),
    ));

    // 6. Approval count pill (only when there are pending approvals)
    let pending_count = state.approval_queue.len() + usize::from(state.approval_active.is_some());
    if pending_count > 0 {
        spans.push(Span::raw(" "));
        spans.push(theme::pill(
            &format!("⚠ {pending_count} pending"),
            theme::ON_FILL,
            theme::WARNING,
        ));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).style(Style::default().bg(theme::SURFACE));
    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
    use std::collections::VecDeque;

    use crate::app::AppState;
    use crate::config::Config;

    /// Helper to render the header into a test buffer and return the buffer.
    fn render_header_to_buffer(state: &AppState, width: u16) -> Buffer {
        let backend = TestBackend::new(width, 1);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|_| panic!("terminal init"));
        terminal
            .draw(|frame| {
                let area = Rect::new(0, 0, width, 1);
                render_header(frame, state, area);
            })
            .unwrap_or_else(|_| panic!("draw"));
        terminal.backend().buffer().clone()
    }

    fn default_state() -> AppState {
        AppState::new(Config::default(), Mode::Plan, Tier::Standard)
    }

    #[test]
    fn header_shows_green_dot_when_connected() {
        let mut state = default_state();
        state.core_status = CoreStatus::Connected {
            version: "1.0.0".to_string(),
        };
        let buf = render_header_to_buffer(&state, 80);
        // The first cell should be the green dot character
        let cell = &buf[(0, 0)];
        assert_eq!(cell.symbol(), "●");
        assert_eq!(cell.fg, Color::Green);
    }

    #[test]
    fn header_shows_red_dot_when_disconnected() {
        let mut state = default_state();
        state.core_status = CoreStatus::Disconnected;
        let buf = render_header_to_buffer(&state, 80);
        let cell = &buf[(0, 0)];
        assert_eq!(cell.symbol(), "●");
        assert_eq!(cell.fg, Color::Red);
    }

    #[test]
    fn header_shows_yellow_dot_when_unknown() {
        let state = default_state(); // CoreStatus::Unknown by default
        let buf = render_header_to_buffer(&state, 80);
        let cell = &buf[(0, 0)];
        assert_eq!(cell.symbol(), "●");
        assert_eq!(cell.fg, Color::Yellow);
    }

    #[test]
    fn header_shows_mode_badge() {
        let mut state = default_state();
        state.mode = Mode::Yolo;
        let buf = render_header_to_buffer(&state, 80);
        // Convert buffer to string and check for [YOLO]
        let content = buffer_to_string(&buf);
        assert!(content.contains("YOLO"));
    }

    #[test]
    fn header_shows_tier_badge() {
        let mut state = default_state();
        state.tier = Tier::Heavy;
        let buf = render_header_to_buffer(&state, 80);
        let content = buffer_to_string(&buf);
        assert!(content.contains("heavy"));
    }

    #[test]
    fn header_shows_model_name() {
        let mut state = default_state();
        state.active_model = "deepseek-chat".to_string();
        let buf = render_header_to_buffer(&state, 80);
        let content = buffer_to_string(&buf);
        assert!(content.contains("deepseek-chat"));
    }

    #[test]
    fn header_shows_session_cost() {
        let mut state = default_state();
        state.session_cost_usd = 1.5;
        let buf = render_header_to_buffer(&state, 80);
        let content = buffer_to_string(&buf);
        assert!(content.contains("$1.50"));
    }

    #[test]
    fn header_shows_approval_badge_when_pending() {
        let mut state = default_state();
        // Add some approval requests to the queue
        state.approval_queue = VecDeque::from(vec![
            crate::messages::ApprovalRequest {
                agent_id: "abc12345-1234-1234-1234-123456789012".to_string(),
                tool_name: "bash".to_string(),
                risk_level: "HIGH".to_string(),
                arguments_preview: "rm -rf /tmp/test".to_string(),
                timeout_secs: 30,
                received_at: tokio::time::Instant::now(),
            },
            crate::messages::ApprovalRequest {
                agent_id: "def12345-1234-1234-1234-123456789012".to_string(),
                tool_name: "bash".to_string(),
                risk_level: "CRITICAL".to_string(),
                arguments_preview: "deploy".to_string(),
                timeout_secs: 60,
                received_at: tokio::time::Instant::now(),
            },
        ]);
        let buf = render_header_to_buffer(&state, 80);
        let content = buffer_to_string(&buf);
        assert!(content.contains("⚠ 2 pending"));
    }

    #[test]
    fn header_no_approval_badge_when_empty() {
        let state = default_state();
        let buf = render_header_to_buffer(&state, 80);
        let content = buffer_to_string(&buf);
        assert!(!content.contains("pending"));
    }

    #[test]
    fn header_background_is_dark_gray() {
        let state = default_state();
        let buf = render_header_to_buffer(&state, 80);
        // Check that the bar background is DarkGray on a trailing (empty) cell.
        // Pills paint their own bg, so sample near the right edge instead.
        let cell = &buf[(79, 0)];
        assert_eq!(cell.bg, Color::DarkGray);
    }

    /// Convert a single-row buffer to a string for content assertions.
    fn buffer_to_string(buf: &Buffer) -> String {
        let area = buf.area();
        let mut s = String::new();
        for x in 0..area.width {
            s.push_str(buf[(x, 0)].symbol());
        }
        s
    }
}
