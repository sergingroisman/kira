#![forbid(unsafe_code)]

//! Task browser overlay — full-screen overlay displaying SDD pipeline progress,
//! wave execution status, and per-task detail with scrollable output.
//!
//! Layout:
//! ```text
//! ┌─────────────────── Task Browser ───────────────────┐
//! │ Header (1 line): title + disconnected indicator     │
//! ├────────────────────┬───────────────────────────────┤
//! │ Task List (50%)    │ Detail Panel (50%)            │
//! │ - Wave 1           │ Title: ...                    │
//! │   ✓ Task A  ag1..  │ Description: ...              │
//! │   ⠋ Task B  ag2..  │ Output (scrollable):          │
//! │ - Wave 2           │   line 1                      │
//! │   ○ Task C         │   line 2                      │
//! │                    │ Errors: ...                    │
//! ├────────────────────┴───────────────────────────────┤
//! │ Footer (1 line): keybinding hints                   │
//! └────────────────────────────────────────────────────┘
//! ```

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::models::{TaskBrowserState, TaskStatus, TaskWave};

// ── Public API ─────────────────────────────────────────────────────────────

/// Render the task browser overlay into the given area.
///
/// Displays a full-screen overlay with header, task list (left 50%),
/// detail panel (right 50%), and footer. Tasks are grouped by wave in
/// ascending order.
pub fn render_task_browser(frame: &mut Frame, state: &TaskBrowserState, area: Rect) {
    // Clear the area behind the overlay
    frame.render_widget(Clear, area);

    // Determine if Core is disconnected based on staleness of last_refresh.
    // If last_refresh is older than 4 seconds (2x the refresh interval),
    // we consider the connection stale.
    let is_disconnected = state.last_refresh.elapsed().as_secs() > 4;

    // Split into header (1), main body, footer (1)
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    let header_area = vertical[0];
    let body_area = vertical[1];
    let footer_area = vertical[2];

    // Render header
    render_header(frame, is_disconnected, header_area);

    // Check for empty state
    if state.waves.is_empty() {
        render_empty_state(frame, body_area);
    } else {
        // Split body into task list (left 50%) and detail panel (right 50%)
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(body_area);

        let list_area = horizontal[0];
        let detail_area = horizontal[1];

        render_task_list(frame, state, list_area);
        render_detail_panel(frame, state, detail_area);
    }

    // Render footer
    render_footer(frame, footer_area);
}

/// Format a duration as MM:SS or HH:MM:SS if duration exceeds 59 minutes.
pub fn format_duration(duration: std::time::Duration) -> String {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

/// Returns the status icon for a given task status.
pub const fn status_icon(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Running => "⠋",
        TaskStatus::Completed => "✓",
        TaskStatus::Failed => "✗",
        TaskStatus::Skipped => "–",
        TaskStatus::Pending => "○",
    }
}

/// Returns waves sorted in ascending order by wave_number.
///
/// This is the pure function used by the property test to verify
/// wave ordering correctness.
pub fn group_by_wave(waves: &[TaskWave]) -> Vec<&TaskWave> {
    let mut sorted: Vec<&TaskWave> = waves.iter().collect();
    sorted.sort_by_key(|w| w.wave_number);
    sorted
}

// ── Private Rendering Helpers ──────────────────────────────────────────────

/// Render the header line with title and optional disconnected indicator.
fn render_header(frame: &mut Frame, is_disconnected: bool, area: Rect) {
    let mut spans = vec![Span::styled(
        " Task Browser ",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )];

    if is_disconnected {
        spans.push(Span::styled(
            " [DISCONNECTED] ",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));
    }

    let header = Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::DarkGray));

    frame.render_widget(header, area);
}

/// Render the footer line with keybinding hints.
fn render_footer(frame: &mut Frame, area: Rect) {
    let hints = Line::from(vec![
        Span::styled(" ↑/↓ ", Style::default().fg(Color::Cyan)),
        Span::raw("navigate  "),
        Span::styled("Enter ", Style::default().fg(Color::Cyan)),
        Span::raw("select  "),
        Span::styled("Esc ", Style::default().fg(Color::Cyan)),
        Span::raw("close"),
    ]);

    let footer = Paragraph::new(hints).style(Style::default().bg(Color::DarkGray));
    frame.render_widget(footer, area);
}

/// Render the empty state when no waves/tasks exist.
fn render_empty_state(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let message = Paragraph::new(Line::from(vec![Span::styled(
        "No SDD pipeline active",
        Style::default()
            .fg(Color::Gray)
            .add_modifier(Modifier::ITALIC),
    )]))
    .block(block)
    .wrap(Wrap { trim: true });

    frame.render_widget(message, area);
}

/// Render the task list panel (left side), grouped by wave.
fn render_task_list(frame: &mut Frame, state: &TaskBrowserState, area: Rect) {
    let block = Block::default()
        .title(Line::from(" Tasks ").style(Style::default().fg(Color::Cyan)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let sorted_waves = group_by_wave(&state.waves);

    let mut items: Vec<ListItem> = Vec::new();
    let mut task_index: usize = 0;

    for wave in &sorted_waves {
        // Wave header line
        let completed = wave
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Completed)
            .count();
        let total = wave.tasks.len();

        let wave_header = Line::from(vec![
            Span::styled(
                format!("── Wave {} ", wave.wave_number),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("({completed}/{total})"),
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        items.push(ListItem::new(wave_header));

        // Task entries within this wave
        for task in &wave.tasks {
            let is_selected = state.selected_task == Some(task_index);

            let icon = status_icon(&task.status);
            let icon_color = status_color(task.status);

            let title = truncate_title(&task.title, 80);
            let agent_id = task.agent_short_id.as_deref().unwrap_or_default();
            let elapsed = format_duration(task.elapsed);

            let mut spans = vec![
                Span::raw("  "),
                Span::styled(icon, Style::default().fg(icon_color)),
                Span::raw(" "),
                Span::styled(
                    title,
                    if is_selected {
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    },
                ),
            ];

            if !agent_id.is_empty() {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    agent_id.to_string(),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            spans.push(Span::raw("  "));
            spans.push(Span::styled(elapsed, Style::default().fg(Color::DarkGray)));

            let style = if is_selected {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };

            items.push(ListItem::new(Line::from(spans)).style(style));
            task_index += 1;
        }
    }

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

/// Render the detail panel (right side) for the selected task.
fn render_detail_panel(frame: &mut Frame, state: &TaskBrowserState, area: Rect) {
    let block = Block::default()
        .title(Line::from(" Details ").style(Style::default().fg(Color::Cyan)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    // Find the selected task
    let selected_task = state.selected_task.and_then(|idx| {
        let mut count = 0usize;
        for wave in &state.waves {
            for task in &wave.tasks {
                if count == idx {
                    return Some(task);
                }
                count += 1;
            }
        }
        None
    });

    let Some(task) = selected_task else {
        // No task selected — show hint
        let hint = Paragraph::new(Line::from(Span::styled(
            "Select a task to view details",
            Style::default().fg(Color::Gray),
        )))
        .block(block)
        .wrap(Wrap { trim: true });
        frame.render_widget(hint, area);
        return;
    };

    // Build detail content lines
    let mut lines: Vec<Line> = Vec::new();

    // Title
    lines.push(Line::from(vec![
        Span::styled("Title: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(&task.title),
    ]));

    // Status
    lines.push(Line::from(vec![
        Span::styled("Status: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(
            status_label(task.status),
            Style::default().fg(status_color(task.status)),
        ),
    ]));

    // Agent
    if let Some(ref agent_id) = task.agent_short_id {
        lines.push(Line::from(vec![
            Span::styled("Agent: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(agent_id.as_str()),
        ]));
    }

    // Elapsed
    lines.push(Line::from(vec![
        Span::styled("Elapsed: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(format_duration(task.elapsed)),
    ]));

    lines.push(Line::from(""));

    // Description
    if let Some(ref desc) = task.description {
        lines.push(Line::from(Span::styled(
            "Description:",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        for line in desc.lines() {
            lines.push(Line::from(Span::raw(line.to_string())));
        }
        lines.push(Line::from(""));
    }

    // Errors
    if let Some(ref error) = task.error {
        lines.push(Line::from(Span::styled(
            "Error:",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )));
        for line in error.lines() {
            lines.push(Line::from(Span::styled(
                line.to_string(),
                Style::default().fg(Color::Red),
            )));
        }
        lines.push(Line::from(""));
    }

    // Output (scrollable, up to 10k lines)
    if !task.output.is_empty() {
        lines.push(Line::from(Span::styled(
            format!("Output ({} lines):", task.output.len()),
            Style::default().add_modifier(Modifier::BOLD),
        )));

        let scroll = state.detail_scroll as usize;
        let visible_output = task.output.iter().skip(scroll);

        for line in visible_output {
            lines.push(Line::from(Span::raw(line.clone())));
        }
    }

    let detail = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((state.detail_scroll, 0));

    frame.render_widget(detail, area);
}

// ── Utility Functions ──────────────────────────────────────────────────────

/// Returns the color associated with a task status.
const fn status_color(status: TaskStatus) -> Color {
    match status {
        TaskStatus::Running => Color::Cyan,
        TaskStatus::Completed => Color::Green,
        TaskStatus::Failed => Color::Red,
        TaskStatus::Skipped => Color::DarkGray,
        TaskStatus::Pending => Color::Gray,
    }
}

/// Returns a human-readable label for a task status.
const fn status_label(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Running => "Running",
        TaskStatus::Completed => "Completed",
        TaskStatus::Failed => "Failed",
        TaskStatus::Skipped => "Skipped",
        TaskStatus::Pending => "Pending",
    }
}

/// Truncate a title to `max_chars` characters, appending "…" if truncated.
fn truncate_title(title: &str, max_chars: usize) -> String {
    if title.chars().count() <= max_chars {
        title.to_string()
    } else {
        let truncated: String = title.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{truncated}…")
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{TaskEntry, TaskStatus, TaskWave};
    use proptest::prelude::*;

    #[test]
    fn format_duration_seconds_only() {
        let d = std::time::Duration::from_secs(45);
        assert_eq!(format_duration(d), "00:45");
    }

    #[test]
    fn format_duration_minutes_and_seconds() {
        let d = std::time::Duration::from_secs(125); // 2:05
        assert_eq!(format_duration(d), "02:05");
    }

    #[test]
    fn format_duration_exactly_one_hour() {
        let d = std::time::Duration::from_secs(3600);
        assert_eq!(format_duration(d), "01:00:00");
    }

    #[test]
    fn format_duration_over_one_hour() {
        let d = std::time::Duration::from_secs(3661); // 1:01:01
        assert_eq!(format_duration(d), "01:01:01");
    }

    #[test]
    fn format_duration_zero() {
        let d = std::time::Duration::from_secs(0);
        assert_eq!(format_duration(d), "00:00");
    }

    #[test]
    fn format_duration_59_minutes_59_seconds() {
        let d = std::time::Duration::from_secs(59 * 60 + 59); // 59:59
        assert_eq!(format_duration(d), "59:59");
    }

    #[test]
    fn format_duration_60_minutes() {
        // 60 minutes = 1 hour, should switch to HH:MM:SS
        let d = std::time::Duration::from_secs(60 * 60);
        assert_eq!(format_duration(d), "01:00:00");
    }

    #[test]
    fn status_icon_mapping() {
        assert_eq!(status_icon(&TaskStatus::Running), "⠋");
        assert_eq!(status_icon(&TaskStatus::Completed), "✓");
        assert_eq!(status_icon(&TaskStatus::Failed), "✗");
        assert_eq!(status_icon(&TaskStatus::Skipped), "–");
        assert_eq!(status_icon(&TaskStatus::Pending), "○");
    }

    #[test]
    fn group_by_wave_sorts_ascending() {
        let waves = vec![
            TaskWave {
                wave_number: 3,
                tasks: vec![],
            },
            TaskWave {
                wave_number: 1,
                tasks: vec![],
            },
            TaskWave {
                wave_number: 2,
                tasks: vec![],
            },
        ];

        let sorted = group_by_wave(&waves);
        let numbers: Vec<u16> = sorted.iter().map(|w| w.wave_number).collect();
        assert_eq!(numbers, vec![1, 2, 3]);
    }

    #[test]
    fn group_by_wave_empty() {
        let waves: Vec<TaskWave> = vec![];
        let sorted = group_by_wave(&waves);
        assert!(sorted.is_empty());
    }

    #[test]
    fn group_by_wave_single() {
        let waves = vec![TaskWave {
            wave_number: 5,
            tasks: vec![],
        }];

        let sorted = group_by_wave(&waves);
        assert_eq!(sorted.len(), 1);
        assert_eq!(sorted[0].wave_number, 5);
    }

    #[test]
    fn truncate_title_short() {
        let title = "Short title";
        assert_eq!(truncate_title(title, 80), "Short title");
    }

    #[test]
    fn truncate_title_exact_80() {
        let title: String = "a".repeat(80);
        assert_eq!(truncate_title(&title, 80), title);
    }

    #[test]
    fn truncate_title_over_80() {
        let title: String = "a".repeat(100);
        let result = truncate_title(&title, 80);
        // 79 chars + "…" = 80 display chars
        assert_eq!(result.chars().count(), 80);
        assert!(result.ends_with('…'));
    }

    #[test]
    fn truncate_title_unicode() {
        // Unicode characters should be counted by char, not byte
        let title = "日本語のタイトルがとても長い場合にはトランケートされるべきです";
        let result = truncate_title(title, 10);
        assert_eq!(result.chars().count(), 10);
        assert!(result.ends_with('…'));
    }

    #[test]
    fn status_color_mapping() {
        assert_eq!(status_color(TaskStatus::Running), Color::Cyan);
        assert_eq!(status_color(TaskStatus::Completed), Color::Green);
        assert_eq!(status_color(TaskStatus::Failed), Color::Red);
        assert_eq!(status_color(TaskStatus::Skipped), Color::DarkGray);
        assert_eq!(status_color(TaskStatus::Pending), Color::Gray);
    }

    #[test]
    fn status_label_mapping() {
        assert_eq!(status_label(TaskStatus::Running), "Running");
        assert_eq!(status_label(TaskStatus::Completed), "Completed");
        assert_eq!(status_label(TaskStatus::Failed), "Failed");
        assert_eq!(status_label(TaskStatus::Skipped), "Skipped");
        assert_eq!(status_label(TaskStatus::Pending), "Pending");
    }

    #[test]
    fn group_by_wave_preserves_tasks() {
        let waves = vec![
            TaskWave {
                wave_number: 2,
                tasks: vec![TaskEntry {
                    id: "t1".to_string(),
                    title: "Task 1".to_string(),
                    status: TaskStatus::Completed,
                    agent_short_id: Some("abc12345".to_string()),
                    elapsed: std::time::Duration::from_secs(30),
                    description: Some("A task".to_string()),
                    output: vec!["line1".to_string()],
                    error: None,
                }],
            },
            TaskWave {
                wave_number: 1,
                tasks: vec![TaskEntry {
                    id: "t2".to_string(),
                    title: "Task 2".to_string(),
                    status: TaskStatus::Pending,
                    agent_short_id: None,
                    elapsed: std::time::Duration::from_secs(0),
                    description: None,
                    output: vec![],
                    error: None,
                }],
            },
        ];

        let sorted = group_by_wave(&waves);
        assert_eq!(sorted[0].wave_number, 1);
        assert_eq!(sorted[0].tasks.len(), 1);
        assert_eq!(sorted[0].tasks[0].id, "t2");
        assert_eq!(sorted[1].wave_number, 2);
        assert_eq!(sorted[1].tasks.len(), 1);
        assert_eq!(sorted[1].tasks[0].id, "t1");
    }

    // ── Property-Based Tests ───────────────────────────────────────────────

    // Feature: tui-layout-redesign, Property 15: Task waves are ordered ascending
    //
    // **Validates: Requirements 11.2**
    //
    // For any set of tasks with associated wave numbers, the task browser
    // SHALL group tasks by wave and display wave groups in strictly ascending
    // wave number order.
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_task_waves_are_ordered_ascending(
            wave_specs in prop::collection::vec(
                (1u16..=10, 0usize..=5),
                1..=20,
            )
        ) {
            // Generate TaskWave instances from the random specs
            let waves: Vec<TaskWave> = wave_specs
                .into_iter()
                .map(|(wave_number, task_count)| {
                    let tasks: Vec<TaskEntry> = (0..task_count)
                        .map(|i| TaskEntry {
                            id: format!("task-{wave_number}-{i}"),
                            title: format!("Task {i} in wave {wave_number}"),
                            status: TaskStatus::Pending,
                            agent_short_id: None,
                            elapsed: std::time::Duration::from_secs(0),
                            description: None,
                            output: vec![],
                            error: None,
                        })
                        .collect();
                    TaskWave { wave_number, tasks }
                })
                .collect();

            let sorted = group_by_wave(&waves);

            // The result must be in strictly ascending order by wave_number
            // (strictly ascending between consecutive distinct elements;
            // duplicate wave_numbers are allowed in input and appear adjacent
            // in non-decreasing order).
            for window in sorted.windows(2) {
                prop_assert!(
                    window[0].wave_number <= window[1].wave_number,
                    "Wave ordering violated: wave {} appeared before wave {}",
                    window[0].wave_number,
                    window[1].wave_number,
                );
            }

            // All input waves must be present in the output
            prop_assert_eq!(sorted.len(), waves.len());
        }
    }
}
