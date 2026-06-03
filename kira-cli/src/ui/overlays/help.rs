#![forbid(unsafe_code)]

//! Help overlay — full-screen keybinding reference organized by context
//! (global, chat panel, input area, sidebar, overlays).

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::input::keybindings::{KeyBinding, KeyContext, default_keybindings};
use crossterm::event::{KeyCode, KeyModifiers};

/// Render the help overlay into the given area.
///
/// Displays all keybindings organized by context (Global, Chat Panel,
/// Input Area, Sidebar, Overlays) with key combination, command name,
/// and brief description. Shows "Press Escape or ? to close" at the bottom.
pub fn render_help(frame: &mut Frame, area: Rect) {
    // Clear the area behind the overlay
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(
            Line::from(" Help — Keybindings ").style(
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let registry = default_keybindings();

    let contexts = [
        (KeyContext::Global, "Global"),
        (KeyContext::ChatPanel, "Chat Panel"),
        (KeyContext::InputArea, "Input Area"),
        (KeyContext::Sidebar, "Sidebar"),
        (KeyContext::Overlay, "Overlays"),
    ];

    let mut lines: Vec<Line<'static>> = Vec::new();

    for (context, header) in &contexts {
        let bindings = registry.all_for_context(*context);
        if bindings.is_empty() {
            continue;
        }

        // Section header
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("── {header} ──"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        // Each binding: key combination (bold), command name, description
        for binding in &bindings {
            let key_str = format_key_combination(binding);
            let line = Line::from(vec![
                Span::styled(
                    format!("  {key_str:<16}"),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:<20}", binding.command),
                    Style::default().fg(Color::Green),
                ),
                Span::styled(
                    binding.description.clone(),
                    Style::default().fg(Color::Gray),
                ),
            ]);
            lines.push(line);
        }
    }

    // ── Layers View section (Requirement 12.1) ─────────────────────────────
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "── Layers View ──",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    let layers_bindings: &[(&str, &str, &str)] = &[
        ("Ctrl+L", "toggle_layers", "Toggle Layers View"),
        ("1-4", "layer_detail", "Drill into layer detail"),
        ("Escape", "dismiss_layers", "Dismiss / collapse detail"),
    ];

    for &(key_str, command, description) in layers_bindings {
        let line = Line::from(vec![
            Span::styled(
                format!("  {key_str:<16}"),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{command:<20}"),
                Style::default().fg(Color::Green),
            ),
            Span::styled(
                description.to_string(),
                Style::default().fg(Color::Gray),
            ),
        ]);
        lines.push(line);
    }

    // Footer hint
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Press Escape or ? to close",
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
    )));

    let content = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });

    frame.render_widget(content, area);
}

/// Format a key binding's key sequence as a human-readable string.
///
/// Examples: "Ctrl+C", "Tab", "g g", "Shift+Enter", "PageUp"
fn format_key_combination(binding: &KeyBinding) -> String {
    binding
        .keys
        .iter()
        .map(format_single_key)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Format a single `KeyEvent` as a human-readable string.
fn format_single_key(key: &crossterm::event::KeyEvent) -> String {
    let mut parts = Vec::new();

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        parts.push("Ctrl");
    }
    if key.modifiers.contains(KeyModifiers::ALT) {
        parts.push("Alt");
    }
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        parts.push("Shift");
    }

    let key_name = match key.code {
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::Esc => "Escape".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Delete => "Delete".to_string(),
        KeyCode::Left => "←".to_string(),
        KeyCode::Right => "→".to_string(),
        KeyCode::Up => "↑".to_string(),
        KeyCode::Down => "↓".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::PageUp => "PageUp".to_string(),
        KeyCode::PageDown => "PageDown".to_string(),
        KeyCode::Insert => "Insert".to_string(),
        KeyCode::F(n) => format!("F{n}"),
        _ => "?".to_string(),
    };

    parts.push(&key_name);
    parts.join("+")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn format_ctrl_c() {
        let binding = KeyBinding {
            keys: vec![KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)],
            command: "quit".into(),
            description: "Quit".into(),
            context: KeyContext::Global,
        };
        assert_eq!(format_key_combination(&binding), "Ctrl+c");
    }

    #[test]
    fn format_tab() {
        let binding = KeyBinding {
            keys: vec![KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)],
            command: "toggle_sidebar".into(),
            description: "Toggle sidebar".into(),
            context: KeyContext::Global,
        };
        assert_eq!(format_key_combination(&binding), "Tab");
    }

    #[test]
    fn format_multi_key_gg() {
        let binding = KeyBinding {
            keys: vec![
                KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
                KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
            ],
            command: "go_top".into(),
            description: "Go to top".into(),
            context: KeyContext::ChatPanel,
        };
        assert_eq!(format_key_combination(&binding), "g g");
    }

    #[test]
    fn format_shift_enter() {
        let binding = KeyBinding {
            keys: vec![KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT)],
            command: "newline".into(),
            description: "Insert newline".into(),
            context: KeyContext::InputArea,
        };
        assert_eq!(format_key_combination(&binding), "Shift+Enter");
    }

    #[test]
    fn format_page_up() {
        let binding = KeyBinding {
            keys: vec![KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE)],
            command: "page_up".into(),
            description: "Page up".into(),
            context: KeyContext::ChatPanel,
        };
        assert_eq!(format_key_combination(&binding), "PageUp");
    }

    #[test]
    fn format_escape() {
        let binding = KeyBinding {
            keys: vec![KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)],
            command: "close".into(),
            description: "Close overlay".into(),
            context: KeyContext::Overlay,
        };
        assert_eq!(format_key_combination(&binding), "Escape");
    }

    #[test]
    fn render_help_does_not_panic() {
        use ratatui::{Terminal, backend::TestBackend};

        let backend = TestBackend::new(80, 40);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let area = frame.area();
                render_help(frame, area);
            })
            .unwrap();
    }

    #[test]
    fn render_help_small_area_does_not_panic() {
        use ratatui::{Terminal, backend::TestBackend};

        let backend = TestBackend::new(20, 5);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let area = frame.area();
                render_help(frame, area);
            })
            .unwrap();
    }
}
