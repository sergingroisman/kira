#![forbid(unsafe_code)]

//! Overlay stack manager — manages a z-ordered stack of overlays (command palette,
//! session picker, task browser, help, approval) rendered above the base layer.

pub mod approval;
pub mod command_palette;
pub mod help;
pub mod session_picker;
pub mod task_browser;

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::AppState;
use crate::models::Overlay;

// ── Public API ─────────────────────────────────────────────────────────────

/// Render the top-of-stack overlay (if any) into the given area.
///
/// When the overlay stack is empty, this is a no-op. Otherwise, the top
/// overlay is matched and delegated to the appropriate sub-module renderer.
pub fn render_overlay(frame: &mut Frame, state: &AppState, area: Rect) {
    let Some(overlay) = state.overlay_stack.top() else {
        return;
    };

    match overlay {
        Overlay::CommandPalette(palette_state) => {
            let overlay_area = centered_rect(60, 50, area);
            command_palette::render_command_palette(frame, palette_state, overlay_area);
        }
        Overlay::SessionPicker(picker_state) => {
            session_picker::render_session_picker(frame, picker_state, area);
        }
        Overlay::TaskBrowser(browser_state) => {
            task_browser::render_task_browser(frame, browser_state, area);
        }
        Overlay::Help => {
            help::render_help(frame, area);
        }
        Overlay::Approval => {
            let overlay_area = centered_rect(70, 50, area);
            approval::render_approval(frame, state, overlay_area);
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Compute a centered rectangle within `area` using percentage-based sizing.
///
/// `percent_x` and `percent_y` are clamped to [1, 100].
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let px = percent_x.clamp(1, 100);
    let py = percent_y.clamp(1, 100);

    let width = area.width.saturating_mul(px) / 100;
    let height = area.height.saturating_mul(py) / 100;

    // Ensure at least 1×1
    let width = width.max(1);
    let height = height.max(1);

    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;

    Rect::new(x, y, width, height)
}

/// Render a placeholder overlay block with a title, clearing the area first.
///
/// Used by stub sub-module renderers during incremental development.
pub(crate) fn render_placeholder(frame: &mut Frame, title: &str, area: Rect) {
    // Clear the area behind the overlay
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(
            Line::from(format!(" {title} ")).style(
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let content = Paragraph::new(Line::from(format!("[{title}]")))
        .block(block)
        .style(Style::default().fg(Color::Gray));

    frame.render_widget(content, area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centered_rect_basic() {
        let area = Rect::new(0, 0, 100, 50);
        let result = centered_rect(60, 50, area);

        assert_eq!(result.width, 60);
        assert_eq!(result.height, 25);
        assert_eq!(result.x, 20);
        assert_eq!(result.y, 12);
    }

    #[test]
    fn centered_rect_full_size() {
        let area = Rect::new(0, 0, 80, 40);
        let result = centered_rect(100, 100, area);

        assert_eq!(result.width, 80);
        assert_eq!(result.height, 40);
        assert_eq!(result.x, 0);
        assert_eq!(result.y, 0);
    }

    #[test]
    fn centered_rect_clamps_to_minimum() {
        let area = Rect::new(0, 0, 2, 2);
        let result = centered_rect(1, 1, area);

        // 1% of 2 = 0, clamped to 1
        assert!(result.width >= 1);
        assert!(result.height >= 1);
    }

    #[test]
    fn centered_rect_zero_area() {
        let area = Rect::new(0, 0, 0, 0);
        let result = centered_rect(50, 50, area);

        // Should still produce at least 1×1
        assert_eq!(result.width, 1);
        assert_eq!(result.height, 1);
    }

    #[test]
    fn centered_rect_with_offset() {
        let area = Rect::new(10, 5, 100, 50);
        let result = centered_rect(60, 50, area);

        assert_eq!(result.width, 60);
        assert_eq!(result.height, 25);
        assert_eq!(result.x, 30); // 10 + (100-60)/2
        assert_eq!(result.y, 17); // 5 + (50-25)/2
    }
}
