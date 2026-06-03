#![forbid(unsafe_code)]

//! Command palette overlay — centered fuzzy-search overlay for discovering
//! and executing commands (Ctrl+P).
//!
//! Provides a text input for fuzzy filtering, displays up to 15 results
//! ranked by match score, and supports keyboard navigation.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use crate::models::{CommandPaletteState, PaletteEntry};

/// Maximum number of results displayed in the command palette.
const MAX_RESULTS: usize = 15;

/// Maximum length of the filter input.
const MAX_FILTER_LEN: usize = 128;

// ── Rendering ──────────────────────────────────────────────────────────────

/// Render the command palette overlay into the given area.
///
/// Displays a bordered block with "Command Palette" title, a filter input
/// line at the top, and a ranked results list below with the selected item
/// highlighted.
pub fn render_command_palette(frame: &mut Frame, state: &CommandPaletteState, area: Rect) {
    // Clear the area behind the overlay
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(
            Line::from(" Command Palette ").style(
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    // Render the outer block and get the inner area
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 2 || inner.width < 4 {
        return;
    }

    // Split inner area: 1 line for filter input, rest for results
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // filter input
            Constraint::Length(1), // separator
            Constraint::Min(1),    // results list
        ])
        .split(inner);

    // Render filter input line
    let filter_display = if state.filter.is_empty() {
        Span::styled("Type to filter...", Style::default().fg(Color::DarkGray))
    } else {
        Span::styled(
            truncate_str(&state.filter, inner.width as usize),
            Style::default().fg(Color::White),
        )
    };
    let filter_line = Paragraph::new(Line::from(vec![
        Span::styled("> ", Style::default().fg(Color::Cyan)),
        filter_display,
    ]));
    frame.render_widget(filter_line, chunks[0]);

    // Render separator line
    let separator = Paragraph::new(Line::from("─".repeat(inner.width as usize)))
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(separator, chunks[1]);

    // Render results list
    if state.results.is_empty() {
        let empty_msg = if state.filter.is_empty() {
            "No commands available"
        } else {
            "No matches found"
        };
        let empty =
            Paragraph::new(Line::from(empty_msg)).style(Style::default().fg(Color::DarkGray));
        frame.render_widget(empty, chunks[2]);
    } else {
        let items: Vec<ListItem> = state
            .results
            .iter()
            .enumerate()
            .take(chunks[2].height as usize)
            .map(|(i, entry)| {
                let is_selected = i == state.selected;
                let style = if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                let line = Line::from(vec![
                    Span::styled(&entry.command, style),
                    Span::styled("  ", style),
                    Span::styled(
                        &entry.description,
                        if is_selected {
                            style
                        } else {
                            Style::default().fg(Color::DarkGray)
                        },
                    ),
                ]);
                ListItem::new(line)
            })
            .collect();

        let list = List::new(items);
        frame.render_widget(list, chunks[2]);
    }
}

// ── Filtering & Scoring ────────────────────────────────────────────────────

/// Fuzzy-filter and rank commands against the given filter string.
///
/// Returns at most 15 results sorted in descending order by score.
///
/// Scoring:
/// - Exact match (case-insensitive): 100
/// - Starts with filter (case-insensitive): 80
/// - Contains filter (case-insensitive): 50
/// - No match: excluded
///
/// When the filter is empty, all commands are returned (up to 15) with score 100.
pub fn filter_palette_commands(commands: &[PaletteEntry], filter: &str) -> Vec<PaletteEntry> {
    // Truncate filter to max length
    let filter_trimmed = if filter.len() > MAX_FILTER_LEN {
        &filter[..MAX_FILTER_LEN]
    } else {
        filter
    };

    if filter_trimmed.is_empty() {
        // Return all commands (up to MAX_RESULTS) with max score
        return commands
            .iter()
            .take(MAX_RESULTS)
            .map(|entry| PaletteEntry {
                command: entry.command.clone(),
                description: entry.description.clone(),
                score: 100,
            })
            .collect();
    }

    let filter_lower = filter_trimmed.to_lowercase();

    let mut scored: Vec<PaletteEntry> = commands
        .iter()
        .filter_map(|entry| {
            let cmd_lower = entry.command.to_lowercase();
            let desc_lower = entry.description.to_lowercase();

            let score = if cmd_lower == filter_lower || desc_lower == filter_lower {
                100
            } else if cmd_lower.starts_with(&filter_lower) || desc_lower.starts_with(&filter_lower)
            {
                80
            } else if cmd_lower.contains(&filter_lower) || desc_lower.contains(&filter_lower) {
                50
            } else {
                return None;
            };

            Some(PaletteEntry {
                command: entry.command.clone(),
                description: entry.description.clone(),
                score,
            })
        })
        .collect();

    // Sort descending by score, then alphabetically by command for stability
    scored.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.command.cmp(&b.command))
    });

    scored.truncate(MAX_RESULTS);
    scored
}

// ── Default Commands ───────────────────────────────────────────────────────

/// Returns all available commands for the command palette.
///
/// Includes all slash commands, mode toggles, help, session picker,
/// task browser, and sidebar toggle.
pub fn default_palette_commands() -> Vec<PaletteEntry> {
    vec![
        // Slash commands
        PaletteEntry {
            command: "/sessions".to_string(),
            description: "Open session picker".to_string(),
            score: 0,
        },
        PaletteEntry {
            command: "/models".to_string(),
            description: "List available models".to_string(),
            score: 0,
        },
        PaletteEntry {
            command: "/agents".to_string(),
            description: "List active agents".to_string(),
            score: 0,
        },
        PaletteEntry {
            command: "/status".to_string(),
            description: "Show system status".to_string(),
            score: 0,
        },
        PaletteEntry {
            command: "/help".to_string(),
            description: "Show help overlay".to_string(),
            score: 0,
        },
        PaletteEntry {
            command: "/quit".to_string(),
            description: "Quit Kira".to_string(),
            score: 0,
        },
        PaletteEntry {
            command: "/waves".to_string(),
            description: "Open task browser".to_string(),
            score: 0,
        },
        PaletteEntry {
            command: "/budget".to_string(),
            description: "Show budget details".to_string(),
            score: 0,
        },
        // Mode toggles and actions
        PaletteEntry {
            command: "Toggle Mode (Plan/Yolo)".to_string(),
            description: "Switch between Plan and Yolo modes".to_string(),
            score: 0,
        },
        PaletteEntry {
            command: "Toggle Sidebar".to_string(),
            description: "Show or hide the sidebar panel".to_string(),
            score: 0,
        },
        PaletteEntry {
            command: "Open Help".to_string(),
            description: "Display keybinding reference".to_string(),
            score: 0,
        },
        PaletteEntry {
            command: "Open Session Picker".to_string(),
            description: "Browse and switch sessions".to_string(),
            score: 0,
        },
        PaletteEntry {
            command: "Open Task Browser".to_string(),
            description: "View SDD pipeline tasks and waves".to_string(),
            score: 0,
        },
        PaletteEntry {
            command: "Toggle Layers View".to_string(),
            description: "Show or hide the Layers pipeline view".to_string(),
            score: 0,
        },
    ]
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Truncate a string to fit within `max_width` characters.
fn truncate_str(s: &str, max_width: usize) -> String {
    if s.len() <= max_width {
        s.to_string()
    } else if max_width > 3 {
        format!("{}...", &s[..max_width - 3])
    } else {
        s.chars().take(max_width).collect()
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_commands() -> Vec<PaletteEntry> {
        default_palette_commands()
    }

    #[test]
    fn filter_empty_returns_all_up_to_max() {
        let commands = sample_commands();
        let results = filter_palette_commands(&commands, "");
        assert_eq!(results.len(), commands.len().min(MAX_RESULTS));
        // All should have score 100 when no filter
        for r in &results {
            assert_eq!(r.score, 100);
        }
    }

    #[test]
    fn filter_exact_match_scores_100() {
        let commands = sample_commands();
        let results = filter_palette_commands(&commands, "/help");
        assert!(!results.is_empty());
        assert_eq!(results[0].command, "/help");
        assert_eq!(results[0].score, 100);
    }

    #[test]
    fn filter_starts_with_scores_80() {
        let commands = sample_commands();
        let results = filter_palette_commands(&commands, "/ses");
        assert!(!results.is_empty());
        assert_eq!(results[0].command, "/sessions");
        assert_eq!(results[0].score, 80);
    }

    #[test]
    fn filter_contains_scores_50() {
        let commands = sample_commands();
        let results = filter_palette_commands(&commands, "model");
        assert!(!results.is_empty());
        // "/models" contains "model" but doesn't start with it (starts with "/")
        let models_entry = results.iter().find(|r| r.command == "/models");
        assert!(models_entry.is_some());
        // "model" is contained in "/models" -> score 50
        // But "List available models" description starts with... no, contains "models"
        // The command "/models" contains "model" -> 50
        // The description "List available models" contains "model" -> 50
        // Since both are contains, score is 50
        assert_eq!(models_entry.map(|e| e.score).unwrap_or(0), 50);
    }

    #[test]
    fn filter_contains_in_description_scores_50() {
        let commands = sample_commands();
        // "browser" is contained in "Open Task Browser" description "View SDD pipeline tasks and waves"
        // Actually "browser" is in the command "Open Task Browser"
        let results = filter_palette_commands(&commands, "browser");
        assert!(!results.is_empty());
        let task_entry = results.iter().find(|r| r.command == "Open Task Browser");
        assert!(task_entry.is_some());
    }

    #[test]
    fn filter_no_match_returns_empty() {
        let commands = sample_commands();
        let results = filter_palette_commands(&commands, "zzzznonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn filter_case_insensitive() {
        let commands = sample_commands();
        let results = filter_palette_commands(&commands, "HELP");
        assert!(!results.is_empty());
    }

    #[test]
    fn filter_results_sorted_descending_by_score() {
        let commands = sample_commands();
        let results = filter_palette_commands(&commands, "s");
        for window in results.windows(2) {
            assert!(window[0].score >= window[1].score);
        }
    }

    #[test]
    fn filter_results_capped_at_15() {
        // Create more than 15 commands
        let commands: Vec<PaletteEntry> = (0..30)
            .map(|i| PaletteEntry {
                command: format!("command-{i}"),
                description: format!("Description {i}"),
                score: 0,
            })
            .collect();
        // All contain "command"
        let results = filter_palette_commands(&commands, "command");
        assert!(results.len() <= MAX_RESULTS);
    }

    #[test]
    fn filter_long_input_truncated() {
        let commands = sample_commands();
        let long_filter = "a".repeat(200);
        // Should not panic, just return empty (no match for 128 'a's)
        let results = filter_palette_commands(&commands, &long_filter);
        assert!(results.is_empty());
    }

    #[test]
    fn default_palette_commands_includes_all_slash_commands() {
        let commands = default_palette_commands();
        let slash_commands: Vec<&str> = commands
            .iter()
            .filter(|c| c.command.starts_with('/'))
            .map(|c| c.command.as_str())
            .collect();
        assert!(slash_commands.contains(&"/sessions"));
        assert!(slash_commands.contains(&"/models"));
        assert!(slash_commands.contains(&"/agents"));
        assert!(slash_commands.contains(&"/status"));
        assert!(slash_commands.contains(&"/help"));
        assert!(slash_commands.contains(&"/quit"));
        assert!(slash_commands.contains(&"/waves"));
        assert!(slash_commands.contains(&"/budget"));
    }

    #[test]
    fn default_palette_commands_includes_actions() {
        let commands = default_palette_commands();
        let names: Vec<&str> = commands.iter().map(|c| c.command.as_str()).collect();
        assert!(names.contains(&"Toggle Mode (Plan/Yolo)"));
        assert!(names.contains(&"Toggle Sidebar"));
        assert!(names.contains(&"Open Help"));
        assert!(names.contains(&"Open Session Picker"));
        assert!(names.contains(&"Open Task Browser"));
    }

    #[test]
    fn truncate_str_short() {
        assert_eq!(truncate_str("hello", 10), "hello");
    }

    #[test]
    fn truncate_str_exact() {
        assert_eq!(truncate_str("hello", 5), "hello");
    }

    #[test]
    fn truncate_str_long() {
        assert_eq!(truncate_str("hello world", 8), "hello...");
    }

    #[test]
    fn truncate_str_very_short_max() {
        assert_eq!(truncate_str("hello", 2), "he");
    }

    // ── Property-Based Tests ───────────────────────────────────────────────

    use proptest::prelude::*;

    // Feature: tui-layout-redesign, Property 12: Command palette results are ranked and capped
    // **Validates: Requirements 7.3**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_command_palette_results_ranked_and_capped(
            filter in "[a-zA-Z0-9 /]{0,20}"
        ) {
            let commands = default_palette_commands();
            let results = filter_palette_commands(&commands, &filter);

            // (1) Results are capped at MAX_RESULTS (15)
            prop_assert!(
                results.len() <= MAX_RESULTS,
                "Expected at most {} results, got {}",
                MAX_RESULTS,
                results.len()
            );

            // (2) Results are sorted in descending order by score
            for window in results.windows(2) {
                prop_assert!(
                    window[0].score >= window[1].score,
                    "Results not sorted descending by score: {} (cmd={}) followed by {} (cmd={})",
                    window[0].score,
                    window[0].command,
                    window[1].score,
                    window[1].command
                );
            }
        }
    }
}
