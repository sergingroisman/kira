#![forbid(unsafe_code)]

//! Startup splash screen — a full-screen overlay shown while kira-cored is
//! connecting. Dismissed automatically on connect, or on the first keypress.
//!
//! Renders a hand-crafted block-font KIRA banner, connection status, and a
//! dismissal hint. No external widget crates — zero extra dep.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
};

use crate::app::{AppState, CoreStatus};
use crate::ui::theme;

/// Hand-crafted block-font KIRA banner (5 rows, ~40 chars wide per row).
/// Uses block characters for a "big text" effect without extra deps.
const KIRA_BANNER: &[&str] = &[
    "██╗  ██╗██╗██████╗  █████╗",
    "██║ ██╔╝██║██╔══██╗██╔══██╗",
    "█████╔╝ ██║██████╔╝███████║",
    "██╔═██╗ ██║██╔══██╗██╔══██║",
    "██║  ██╗██║██║  ██║██║  ██║",
    "╚═╝  ╚═╝╚═╝╚═╝  ╚═╝╚═╝  ╚═╝",
];

/// How many 100ms ticks before the splash auto-dismisses even without a
/// connect event (safety valve — should not normally trigger).
const SPLASH_MAX_TICKS: u32 = 60; // 6 seconds

/// Render the splash screen.
///
/// Fills the entire terminal with a dark background, a centered KIRA banner,
/// a connection status line, and a dismissal hint at the bottom.
pub fn render_splash(frame: &mut Frame, state: &AppState) {
    let area = frame.area();

    // Clear the entire frame first.
    frame.render_widget(Clear, area);

    // Dark background.
    let bg = Block::default().style(Style::default().bg(Color::Black));
    frame.render_widget(bg, area);

    #[allow(clippy::cast_possible_truncation)] // const array, always 6 rows
    let banner_rows = KIRA_BANNER.len() as u16;

    // Vertical layout: top padding | banner | spacer | status | hint | bottom padding.
    let [_, banner_area, _, status_area, hint_area, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(banner_rows),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(area);

    // ── Banner ────────────────────────────────────────────────────────────────

    let banner_color = match &state.core_status {
        CoreStatus::Connected { .. } => theme::SUCCESS,
        CoreStatus::Disconnected => theme::DANGER,
        CoreStatus::Unknown => theme::ACCENT,
    };

    let banner_style = Style::default()
        .fg(banner_color)
        .add_modifier(Modifier::BOLD);

    let banner_lines: Vec<Line<'_>> = KIRA_BANNER
        .iter()
        .map(|row| Line::from(Span::styled(*row, banner_style)))
        .collect();

    let banner_para = Paragraph::new(banner_lines).alignment(Alignment::Center);
    frame.render_widget(banner_para, banner_area);

    // ── Status line ───────────────────────────────────────────────────────────

    let status_text = match &state.core_status {
        CoreStatus::Connected { version } => Line::from(vec![
            Span::styled("● ", Style::default().fg(theme::SUCCESS)),
            Span::styled(
                format!("Connected to kira-cored {version}"),
                Style::default().fg(theme::TEXT).add_modifier(Modifier::BOLD),
            ),
        ]),
        CoreStatus::Disconnected => Line::from(vec![
            Span::styled("● ", Style::default().fg(theme::DANGER)),
            Span::styled(
                "Core unreachable — retrying…",
                Style::default().fg(theme::DANGER),
            ),
        ]),
        CoreStatus::Unknown => {
            let spinner = theme::spinner_char(state.spinner_frame);
            Line::from(vec![
                Span::styled(
                    format!("{spinner} "),
                    Style::default()
                        .fg(theme::ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "Connecting to kira-cored…",
                    Style::default().fg(theme::TEXT_DIM),
                ),
            ])
        }
    };

    let status_para = Paragraph::new(status_text).alignment(Alignment::Center);
    frame.render_widget(status_para, status_area);

    // ── Dismissal hint ────────────────────────────────────────────────────────

    let hint = Paragraph::new(Line::from(Span::styled(
        "Press any key to continue",
        theme::dim(),
    )))
    .alignment(Alignment::Center);
    frame.render_widget(hint, hint_area);
}

/// Whether the splash should be dismissed.
///
/// Returns `true` if: connected to Core, or max tick budget exceeded.
/// Caller should set `state.splash_visible = false` when this returns `true`.
pub const fn should_dismiss(state: &AppState) -> bool {
    matches!(state.core_status, CoreStatus::Connected { .. })
        || state.splash_ticks >= SPLASH_MAX_TICKS
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AppState, Mode, Tier};
    use crate::config::Config;

    fn state() -> AppState {
        AppState::new(Config::default(), Mode::Plan, Tier::Standard)
    }

    #[test]
    fn splash_visible_on_new_state() {
        assert!(state().splash_visible);
    }

    #[test]
    fn splash_ticks_zero_on_new_state() {
        assert_eq!(state().splash_ticks, 0);
    }

    #[test]
    fn should_dismiss_when_connected() {
        let mut s = state();
        s.core_status = CoreStatus::Connected { version: "1.0".to_string() };
        assert!(should_dismiss(&s));
    }

    #[test]
    fn should_not_dismiss_when_unknown() {
        let s = state();
        assert!(!should_dismiss(&s));
    }

    #[test]
    fn should_dismiss_on_tick_budget() {
        let mut s = state();
        s.splash_ticks = SPLASH_MAX_TICKS;
        assert!(should_dismiss(&s));
    }
}
