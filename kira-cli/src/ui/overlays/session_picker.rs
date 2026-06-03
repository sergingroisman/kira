#![forbid(unsafe_code)]

//! Session picker overlay — full-screen overlay for browsing and switching
//! between sessions with text search filtering.
//!
//! Displays all sessions in a bordered list with title, creation date,
//! message count, and cost. Supports text filtering, workspace scope toggle
//! (Ctrl+A), Enter to select, and Escape to close.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Row, Table},
};

use crate::models::{SessionEntry, SessionPickerState};

// ── Public API ─────────────────────────────────────────────────────────────

/// Render the session picker overlay into the given area.
///
/// The overlay is full-screen with a bordered block titled "Sessions".
/// Layout from top to bottom:
/// 1. Scope indicator ("All Workspaces" or "Current Workspace") + filter input
/// 2. Session list (table with title, date, messages, cost)
/// 3. Highlighted selection row
pub fn render_session_picker(frame: &mut Frame, state: &SessionPickerState, area: Rect) {
    // Clear the area behind the overlay
    frame.render_widget(Clear, area);

    // Build the title with scope indicator
    let scope_label = if state.show_all_workspaces {
        "All Workspaces"
    } else {
        "Current Workspace"
    };

    let title = format!(" Sessions [{scope_label}] ");

    let block = Block::default()
        .title(
            Line::from(title).style(
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    // Compute inner area (inside the border)
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 2 || inner.width < 10 {
        return;
    }

    // Split inner area: filter input (1 line) + gap (1 line) + session list (rest)
    let chunks = Layout::vertical([
        Constraint::Length(1), // filter input
        Constraint::Length(1), // spacer
        Constraint::Min(1),    // session list
    ])
    .split(inner);

    // ── Filter input line ──────────────────────────────────────────────────
    let filter_line = render_filter_line(&state.filter);
    frame.render_widget(filter_line, chunks[0]);

    // ── Session list ───────────────────────────────────────────────────────
    let filtered = filter_sessions(&state.sessions, &state.filter);

    if filtered.is_empty() {
        let empty_msg = Paragraph::new(Line::from(Span::styled(
            "No sessions found",
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(empty_msg, chunks[2]);
        return;
    }

    // Build table rows
    let rows: Vec<Row> = filtered
        .iter()
        .enumerate()
        .map(|(i, session)| {
            let style = if i == state.selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            Row::new(vec![
                format_title(&session.title, chunks[2].width),
                format_date(&session.created_at),
                format_message_count(session.message_count),
                format_cost(session.cost_usd),
            ])
            .style(style)
        })
        .collect();

    let header = Row::new(vec!["Title", "Created", "Messages", "Cost"])
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .bottom_margin(1);

    // Column widths: title gets remaining space, date ~12, messages ~8, cost ~10
    let widths = [
        Constraint::Min(10),
        Constraint::Length(12),
        Constraint::Length(8),
        Constraint::Length(10),
    ];

    let table = Table::new(rows, widths).header(header).column_spacing(1);

    frame.render_widget(table, chunks[2]);
}

/// Filter sessions by case-insensitive substring match on title.
///
/// Returns references to sessions whose title contains the filter string
/// (case-insensitive). If the filter is empty, all sessions are returned.
pub fn filter_sessions<'a>(sessions: &'a [SessionEntry], filter: &str) -> Vec<&'a SessionEntry> {
    if filter.is_empty() {
        return sessions.iter().collect();
    }

    let filter_lower = filter.to_lowercase();
    sessions
        .iter()
        .filter(|s| s.title.to_lowercase().contains(&filter_lower))
        .collect()
}

// ── Private helpers ────────────────────────────────────────────────────────

/// Render the filter input line with a search prompt.
fn render_filter_line(filter: &str) -> Paragraph<'static> {
    let prompt = Span::styled("Filter: ", Style::default().fg(Color::Yellow));
    let text = Span::styled(
        if filter.is_empty() {
            String::from("(type to search)")
        } else {
            filter.to_string()
        },
        if filter.is_empty() {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::White)
        },
    );

    Paragraph::new(Line::from(vec![prompt, text]))
}

/// Format a session title, truncating if necessary.
fn format_title(title: &str, max_width: u16) -> String {
    // Reserve space for other columns (~32 chars) and some padding
    let available = max_width.saturating_sub(35) as usize;
    let available = available.max(10);

    if title.len() <= available {
        title.to_string()
    } else {
        let truncated = &title[..available.saturating_sub(1)];
        format!("{truncated}…")
    }
}

/// Format a creation date as YYYY-MM-DD.
fn format_date(dt: &chrono::DateTime<chrono::Utc>) -> String {
    dt.format("%Y-%m-%d").to_string()
}

/// Format message count.
fn format_message_count(count: u32) -> String {
    format!("{count}")
}

/// Format cost in USD with 2 decimal places.
fn format_cost(cost: f64) -> String {
    format!("${cost:.2}")
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_session(title: &str, messages: u32, cost: f64) -> SessionEntry {
        SessionEntry {
            id: String::from("test-id"),
            title: String::from(title),
            created_at: Utc::now(),
            message_count: messages,
            cost_usd: cost,
        }
    }

    #[test]
    fn filter_sessions_empty_filter_returns_all() {
        let sessions = vec![
            make_session("Alpha", 5, 0.10),
            make_session("Beta", 3, 0.05),
        ];
        let result = filter_sessions(&sessions, "");
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn filter_sessions_case_insensitive() {
        let sessions = vec![
            make_session("Hello World", 5, 0.10),
            make_session("Goodbye", 3, 0.05),
        ];
        let result = filter_sessions(&sessions, "hello");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, "Hello World");
    }

    #[test]
    fn filter_sessions_no_match() {
        let sessions = vec![
            make_session("Alpha", 5, 0.10),
            make_session("Beta", 3, 0.05),
        ];
        let result = filter_sessions(&sessions, "gamma");
        assert!(result.is_empty());
    }

    #[test]
    fn filter_sessions_substring_match() {
        let sessions = vec![
            make_session("Refactor auth module", 10, 0.50),
            make_session("Fix auth bug", 2, 0.02),
            make_session("Add logging", 7, 0.30),
        ];
        let result = filter_sessions(&sessions, "auth");
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn format_cost_two_decimals() {
        assert_eq!(format_cost(1.5), "$1.50");
        assert_eq!(format_cost(0.0), "$0.00");
        assert_eq!(format_cost(12.345), "$12.35");
    }

    #[test]
    fn format_date_iso() {
        use chrono::TimeZone;
        let dt = chrono::Utc
            .with_ymd_and_hms(2024, 3, 15, 10, 30, 0)
            .unwrap();
        assert_eq!(format_date(&dt), "2024-03-15");
    }

    #[test]
    fn format_title_no_truncation() {
        let title = "Short title";
        let result = format_title(title, 80);
        assert_eq!(result, "Short title");
    }

    #[test]
    fn format_title_truncation() {
        let title = "This is a very long session title that should be truncated";
        let result = format_title(title, 50);
        assert!(result.len() < title.len());
        assert!(result.ends_with('…'));
    }

    #[test]
    fn format_message_count_display() {
        assert_eq!(format_message_count(0), "0");
        assert_eq!(format_message_count(42), "42");
        assert_eq!(format_message_count(1000), "1000");
    }
}
