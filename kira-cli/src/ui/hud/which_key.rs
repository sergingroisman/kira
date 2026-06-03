#![forbid(unsafe_code)]

//! Which-key panel — shows available key continuations after a partial
//! key sequence, anchored above the footer bar.
//!
//! The panel appears after a 500ms delay when the operator presses a key
//! that is a prefix of one or more multi-key bindings. It lists available
//! continuation keys with their command descriptions and is dismissed on
//! continuation key press, Escape, or an unrecognized key.

use std::time::Duration;

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::models::WhichKeyState;

/// Delay before the which-key panel becomes visible (500ms).
const WHICH_KEY_DELAY: Duration = Duration::from_millis(500);

/// Minimum panel width (to avoid overly narrow popups).
const MIN_PANEL_WIDTH: u16 = 20;

/// Padding added to content width for the panel border and spacing.
const HORIZONTAL_PADDING: u16 = 4;

/// Returns `true` if the which-key panel should be visible.
///
/// The panel is shown only after the configured delay (500ms) has elapsed
/// since the prefix key was pressed, and there is at least one continuation
/// to display.
pub fn should_show_which_key(state: &WhichKeyState) -> bool {
    !state.continuations.is_empty() && state.shown_at.elapsed() >= WHICH_KEY_DELAY
}

/// Render the which-key panel anchored to the bottom of `area` (above footer).
///
/// The panel is a bordered popup listing each continuation as:
///   key (bold cyan) — description (gray)
///
/// The panel width is determined by the longest continuation entry plus
/// padding, and the height is the number of continuations plus 2 for borders.
/// It is horizontally centered within the given area.
pub fn render_which_key(frame: &mut Frame, state: &WhichKeyState, area: Rect) {
    if !should_show_which_key(state) {
        return;
    }

    if state.continuations.is_empty() {
        return;
    }

    // Compute content width: max(key.len + " → " + description.len) across all entries
    let max_content_len = state
        .continuations
        .iter()
        .map(|c| c.key.len() + 3 + c.description.len()) // " → " is 3 display chars
        .max()
        .unwrap_or(0);
    #[allow(clippy::cast_possible_truncation)]
    let content_width = max_content_len as u16;

    // Panel dimensions (including border)
    let panel_width = content_width
        .saturating_add(HORIZONTAL_PADDING)
        .max(MIN_PANEL_WIDTH)
        .min(area.width);

    // Height = number of continuations + 2 (top and bottom border)
    #[allow(clippy::cast_possible_truncation)]
    let continuations_count = state.continuations.len() as u16;
    let panel_height = continuations_count.saturating_add(2).min(area.height);

    // If the area is too small to render anything meaningful, bail out
    if panel_height < 3 || panel_width < MIN_PANEL_WIDTH || area.height < panel_height {
        return;
    }

    // Anchor to bottom of area, horizontally centered
    let x = area
        .x
        .saturating_add(area.width.saturating_sub(panel_width) / 2);
    let y = area
        .y
        .saturating_add(area.height.saturating_sub(panel_height));

    let panel_rect = Rect::new(x, y, panel_width, panel_height);

    // Build the continuation lines
    let lines: Vec<Line<'_>> = state
        .continuations
        .iter()
        .map(|c| {
            Line::from(vec![
                Span::styled(
                    c.key.as_str(),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" → ", Style::default().fg(Color::DarkGray)),
                Span::styled(c.description.as_str(), Style::default().fg(Color::Gray)),
            ])
        })
        .collect();

    // Build the title from the prefix keys
    let title = if state.prefix.is_empty() {
        String::from(" Keys ")
    } else {
        let mut prefix_str = String::new();
        for k in &state.prefix {
            use std::fmt::Write;
            let _ = write!(prefix_str, "{}", KeyEventDisplay(k));
        }
        format!(" {prefix_str}… ")
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(title)
        .title_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    let paragraph = Paragraph::new(lines).block(block);

    // Clear the area behind the popup so it doesn't blend with underlying content
    frame.render_widget(Clear, panel_rect);
    frame.render_widget(paragraph, panel_rect);
}

/// Helper to display a `KeyEvent` in a human-readable format.
struct KeyEventDisplay<'a>(&'a crossterm::event::KeyEvent);

impl std::fmt::Display for KeyEventDisplay<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use crossterm::event::{KeyCode, KeyModifiers};

        let key = self.0;
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            write!(f, "C-")?;
        }
        if key.modifiers.contains(KeyModifiers::ALT) {
            write!(f, "M-")?;
        }
        if key.modifiers.contains(KeyModifiers::SHIFT) {
            write!(f, "S-")?;
        }

        match key.code {
            KeyCode::Char(c) => write!(f, "{c}"),
            KeyCode::Enter => write!(f, "Enter"),
            KeyCode::Esc => write!(f, "Esc"),
            KeyCode::Tab => write!(f, "Tab"),
            KeyCode::Backspace => write!(f, "BS"),
            KeyCode::Delete => write!(f, "Del"),
            KeyCode::Up => write!(f, "↑"),
            KeyCode::Down => write!(f, "↓"),
            KeyCode::Left => write!(f, "←"),
            KeyCode::Right => write!(f, "→"),
            KeyCode::Home => write!(f, "Home"),
            KeyCode::End => write!(f, "End"),
            KeyCode::PageUp => write!(f, "PgUp"),
            KeyCode::PageDown => write!(f, "PgDn"),
            KeyCode::F(n) => write!(f, "F{n}"),
            _ => write!(f, "?"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
    use std::time::Instant;

    use crate::models::{KeyContinuation, WhichKeyState};

    /// Create a `WhichKeyState` with the given continuations and a `shown_at`
    /// that is already past the 500ms delay.
    fn state_past_delay(continuations: Vec<KeyContinuation>) -> WhichKeyState {
        WhichKeyState {
            prefix: vec![KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE)],
            continuations,
            // Set shown_at far enough in the past to exceed the delay
            shown_at: Instant::now() - Duration::from_millis(600),
        }
    }

    /// Create a `WhichKeyState` that has NOT yet exceeded the delay.
    fn state_before_delay(continuations: Vec<KeyContinuation>) -> WhichKeyState {
        WhichKeyState {
            prefix: vec![KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE)],
            continuations,
            shown_at: Instant::now(),
        }
    }

    fn sample_continuations() -> Vec<KeyContinuation> {
        vec![
            KeyContinuation {
                key: "g".to_string(),
                description: "go to top".to_string(),
            },
            KeyContinuation {
                key: "G".to_string(),
                description: "go to bottom".to_string(),
            },
            KeyContinuation {
                key: "d".to_string(),
                description: "go to definition".to_string(),
            },
        ]
    }

    fn render_to_buffer(state: &WhichKeyState, width: u16, height: u16) -> Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|_| panic!("terminal init"));
        terminal
            .draw(|frame| {
                let area = Rect::new(0, 0, width, height);
                render_which_key(frame, state, area);
            })
            .unwrap_or_else(|_| panic!("draw"));
        terminal.backend().buffer().clone()
    }

    fn buffer_to_string(buf: &Buffer) -> String {
        let area = buf.area();
        let mut s = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                s.push_str(buf[(x, y)].symbol());
            }
            s.push('\n');
        }
        s
    }

    #[test]
    fn should_show_returns_false_before_delay() {
        let state = state_before_delay(sample_continuations());
        assert!(!should_show_which_key(&state));
    }

    #[test]
    fn should_show_returns_true_after_delay() {
        let state = state_past_delay(sample_continuations());
        assert!(should_show_which_key(&state));
    }

    #[test]
    fn should_show_returns_false_with_empty_continuations() {
        let state = state_past_delay(vec![]);
        assert!(!should_show_which_key(&state));
    }

    #[test]
    fn panel_renders_continuations_after_delay() {
        let state = state_past_delay(sample_continuations());
        let buf = render_to_buffer(&state, 60, 20);
        let content = buffer_to_string(&buf);

        // Should contain the continuation keys and descriptions
        assert!(content.contains("go to top"));
        assert!(content.contains("go to bottom"));
        assert!(content.contains("go to definition"));
    }

    #[test]
    fn panel_does_not_render_before_delay() {
        let state = state_before_delay(sample_continuations());
        let buf = render_to_buffer(&state, 60, 20);
        let content = buffer_to_string(&buf);

        // Should NOT contain continuation descriptions
        assert!(!content.contains("go to top"));
    }

    #[test]
    fn panel_is_anchored_to_bottom() {
        let state = state_past_delay(sample_continuations());
        // 3 continuations + 2 border = 5 rows panel height
        // In a 20-row area, the panel should start at row 15
        let buf = render_to_buffer(&state, 60, 20);

        // Row 14 (0-indexed) should be empty (above the panel)
        let row_14: String = (0..60).map(|x| buf[(x, 14)].symbol().to_string()).collect();
        let row_14_trimmed = row_14.trim();
        assert!(
            row_14_trimmed.is_empty(),
            "Row 14 should be empty but was: '{row_14_trimmed}'"
        );

        // Row 15 should contain the top border of the panel
        let row_15: String = (0..60).map(|x| buf[(x, 15)].symbol().to_string()).collect();
        assert!(
            row_15.contains("─") || row_15.contains("┐") || row_15.contains("┌"),
            "Row 15 should contain border characters but was: '{row_15}'"
        );
    }

    #[test]
    fn panel_key_is_bold_cyan() {
        let state = state_past_delay(vec![KeyContinuation {
            key: "g".to_string(),
            description: "go to top".to_string(),
        }]);
        // Panel: 1 continuation + 2 border = 3 rows, anchored at bottom of 10-row area
        // Panel starts at row 7, content at row 8
        let backend = TestBackend::new(40, 10);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|_| panic!("terminal init"));
        terminal
            .draw(|frame| {
                let area = Rect::new(0, 0, 40, 10);
                render_which_key(frame, &state.clone(), area);
            })
            .unwrap_or_else(|_| panic!("draw"));
        let buf = terminal.backend().buffer().clone();

        // Find the 'g' character in the content row (row 8)
        // The panel is centered, so we need to find where 'g' is
        let content_row = 8u16;
        let mut found_key = false;
        for x in 0..40u16 {
            let cell = &buf[(x, content_row)];
            if cell.symbol() == "g" {
                assert_eq!(cell.fg, Color::Cyan, "Key should be cyan");
                assert!(cell.modifier.contains(Modifier::BOLD), "Key should be bold");
                found_key = true;
                break;
            }
        }
        assert!(found_key, "Should find the key 'g' in the content row");
    }

    #[test]
    fn panel_does_not_render_in_tiny_area() {
        let state = state_past_delay(sample_continuations());
        // Area too small (height=2 cannot fit border + content)
        let buf = render_to_buffer(&state, 60, 2);
        let content = buffer_to_string(&buf);
        // Should not render any continuation content
        assert!(!content.contains("go to top"));
    }
}
