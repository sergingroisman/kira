#![forbid(unsafe_code)]

//! Tool call renderers — type-specific rendering for bash commands, file edits,
//! file reads, and unknown tool types.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::models::ToolCallKind;
use crate::ui::theme;

/// Render a tool call into a vector of styled lines for display in the chat panel.
///
/// Each variant of `ToolCallKind` gets type-specific rendering:
/// - **Bash**: terminal-style block with dark-gray background, command prefixed with "$ "
/// - **FileEdit**: unified diff with green additions (+) and red deletions (-)
/// - **FileRead**: single summary line in format `"{path} ({N} lines)"`
/// - **Unknown**: plain text with tool type label prefix
pub fn render_tool_call(kind: &ToolCallKind, _area_width: u16) -> Vec<Line<'static>> {
    match kind {
        ToolCallKind::Bash { command, output } => render_bash(command, output),
        ToolCallKind::FileEdit { path, diff } => render_file_edit(path, diff),
        ToolCallKind::FileRead { path, line_count } => render_file_read(path, *line_count),
        ToolCallKind::Unknown { tool_type, content } => render_unknown(tool_type, content),
    }
}

/// Render a bash tool call as a terminal-style block.
///
/// First line shows the command prefixed with "$ ", followed by output lines.
/// All lines use monospace styling with a dark-gray background.
fn render_bash(command: &str, output: &str) -> Vec<Line<'static>> {
    let style = Style::default()
        .bg(theme::SURFACE)
        .fg(theme::TEXT)
        .add_modifier(Modifier::empty());

    let mut lines = Vec::new();

    // Command line prefixed with "$ "
    lines.push(Line::from(Span::styled(format!("$ {command}"), style)));

    // Output lines
    for line in output.lines() {
        lines.push(Line::from(Span::styled(line.to_owned(), style)));
    }

    lines
}

/// Render a file edit tool call as a unified diff.
///
/// Lines starting with "+" get green foreground, lines starting with "-" get red
/// foreground, and all other lines (context, headers) get dim styling.
fn render_file_edit(path: &str, diff: &str) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Header line showing the file path
    lines.push(Line::from(Span::styled(
        path.to_owned(),
        Style::default()
            .fg(theme::ACCENT)
            .add_modifier(Modifier::BOLD),
    )));

    for line in diff.lines() {
        let styled_line = if line.starts_with('+') {
            Line::from(Span::styled(
                line.to_owned(),
                Style::default().fg(theme::SUCCESS),
            ))
        } else if line.starts_with('-') {
            Line::from(Span::styled(
                line.to_owned(),
                Style::default().fg(theme::DANGER),
            ))
        } else {
            Line::from(Span::styled(
                line.to_owned(),
                Style::default().add_modifier(Modifier::DIM),
            ))
        };
        lines.push(styled_line);
    }

    lines
}

/// Render a file read tool call as a single summary line.
///
/// Format: `"{path} ({N} lines)"` with dim styling.
fn render_file_read(path: &str, line_count: usize) -> Vec<Line<'static>> {
    vec![Line::from(Span::styled(
        format!("{path} ({line_count} lines)"),
        Style::default().add_modifier(Modifier::DIM),
    ))]
}

/// Render an unknown tool call as plain text with a tool type label prefix.
///
/// Format: `"[{tool_type}] {content}"`
fn render_unknown(tool_type: &str, content: &str) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    for (i, line) in content.lines().enumerate() {
        let text = if i == 0 {
            format!("[{tool_type}] {line}")
        } else {
            line.to_owned()
        };
        lines.push(Line::from(Span::styled(text, Style::default())));
    }

    // Handle empty content case
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            format!("[{tool_type}]"),
            Style::default(),
        )));
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use ratatui::style::Color;

    /// Generate a random diff line: either starts with "+", "-", or some other prefix.
    fn arb_diff_line() -> impl Strategy<Value = String> {
        prop_oneof![
            // Addition line
            "[^\n]{0,80}".prop_map(|s| format!("+{s}")),
            // Deletion line
            "[^\n]{0,80}".prop_map(|s| format!("-{s}")),
            // Context line (starts with space or other char)
            " [^\n]{0,80}".prop_map(|s| s),
        ]
    }

    /// Generate a multi-line diff string from random diff lines.
    fn arb_diff_string() -> impl Strategy<Value = String> {
        prop::collection::vec(arb_diff_line(), 1..=20).prop_map(|lines| lines.join("\n"))
    }

    /// Generate a non-empty file path string.
    fn arb_file_path() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9_/\\.]{1,100}"
    }

    // Feature: tui-layout-redesign, Property 9: Diff line coloring matches prefix
    // Validates: Requirements 6.6
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_diff_line_coloring_matches_prefix(
            diff in arb_diff_string(),
            path in arb_file_path(),
        ) {
            let kind = ToolCallKind::FileEdit {
                path: path.clone(),
                diff: diff.clone(),
            };
            let lines = render_tool_call(&kind, 80);

            // First line is the header (path), skip it
            // Remaining lines correspond to diff lines
            let diff_lines: Vec<&str> = diff.lines().collect();
            // rendered lines = 1 header + diff_lines.len()
            prop_assert_eq!(lines.len(), 1 + diff_lines.len());

            for (i, diff_line) in diff_lines.iter().enumerate() {
                let rendered = &lines[i + 1]; // skip header
                if diff_line.starts_with('+') {
                    // Should have green foreground
                    prop_assert_eq!(
                        rendered.spans[0].style.fg,
                        Some(Color::Green),
                        "Line starting with '+' should have green fg: {:?}",
                        diff_line
                    );
                } else if diff_line.starts_with('-') {
                    // Should have red foreground
                    prop_assert_eq!(
                        rendered.spans[0].style.fg,
                        Some(Color::Red),
                        "Line starting with '-' should have red fg: {:?}",
                        diff_line
                    );
                }
            }
        }
    }

    // Feature: tui-layout-redesign, Property 10: File read summary format
    // Validates: Requirements 6.7
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_file_read_summary_format(
            path in arb_file_path(),
            line_count in 0usize..100_000,
        ) {
            let kind = ToolCallKind::FileRead {
                path: path.clone(),
                line_count,
            };
            let lines = render_tool_call(&kind, 80);

            // Should produce exactly one line
            prop_assert_eq!(lines.len(), 1);

            // Extract text content
            let content: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();

            // Should match the pattern "{path} ({N} lines)"
            let expected = format!("{path} ({line_count} lines)");
            prop_assert_eq!(content, expected);
        }
    }

    #[test]
    fn bash_renders_command_with_dollar_prefix() {
        let kind = ToolCallKind::Bash {
            command: "ls -la".to_owned(),
            output: "file1.rs\nfile2.rs".to_owned(),
        };
        let lines = render_tool_call(&kind, 80);
        assert_eq!(lines.len(), 3);

        // First line is the command
        let first_line_content: String =
            lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(first_line_content, "$ ls -la");

        // Verify dark-gray background on command line
        let cmd_style = lines[0].spans[0].style;
        assert_eq!(cmd_style.bg, Some(Color::DarkGray));
    }

    #[test]
    fn bash_empty_output_renders_only_command() {
        let kind = ToolCallKind::Bash {
            command: "echo hello".to_owned(),
            output: String::new(),
        };
        let lines = render_tool_call(&kind, 80);
        // Empty string has no lines from .lines(), so only the command line
        assert_eq!(lines.len(), 1);
        let content: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(content, "$ echo hello");
    }

    #[test]
    fn file_edit_colors_additions_green_and_deletions_red() {
        let kind = ToolCallKind::FileEdit {
            path: "src/main.rs".to_owned(),
            diff: "+added line\n-removed line\n context line".to_owned(),
        };
        let lines = render_tool_call(&kind, 80);
        // 1 header + 3 diff lines
        assert_eq!(lines.len(), 4);

        // Addition line (index 1) should be green
        let add_style = lines[1].spans[0].style;
        assert_eq!(add_style.fg, Some(Color::Green));

        // Deletion line (index 2) should be red
        let del_style = lines[2].spans[0].style;
        assert_eq!(del_style.fg, Some(Color::Red));

        // Context line (index 3) should be dim
        let ctx_style = lines[3].spans[0].style;
        assert!(ctx_style.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn file_read_renders_summary_format() {
        let kind = ToolCallKind::FileRead {
            path: "src/lib.rs".to_owned(),
            line_count: 42,
        };
        let lines = render_tool_call(&kind, 80);
        assert_eq!(lines.len(), 1);
        let content: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(content, "src/lib.rs (42 lines)");
    }

    #[test]
    fn file_read_zero_lines() {
        let kind = ToolCallKind::FileRead {
            path: "/tmp/empty.txt".to_owned(),
            line_count: 0,
        };
        let lines = render_tool_call(&kind, 80);
        assert_eq!(lines.len(), 1);
        let content: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(content, "/tmp/empty.txt (0 lines)");
    }

    #[test]
    fn unknown_renders_with_type_prefix() {
        let kind = ToolCallKind::Unknown {
            tool_type: "web_search".to_owned(),
            content: "Results found".to_owned(),
        };
        let lines = render_tool_call(&kind, 80);
        assert_eq!(lines.len(), 1);
        let content: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(content, "[web_search] Results found");
    }

    #[test]
    fn unknown_empty_content_renders_type_only() {
        let kind = ToolCallKind::Unknown {
            tool_type: "mystery".to_owned(),
            content: String::new(),
        };
        let lines = render_tool_call(&kind, 80);
        assert_eq!(lines.len(), 1);
        let content: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(content, "[mystery]");
    }

    #[test]
    fn unknown_multiline_content_prefixes_first_line_only() {
        let kind = ToolCallKind::Unknown {
            tool_type: "custom".to_owned(),
            content: "line one\nline two\nline three".to_owned(),
        };
        let lines = render_tool_call(&kind, 80);
        assert_eq!(lines.len(), 3);
        let first: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        let second: String = lines[1].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(first, "[custom] line one");
        assert_eq!(second, "line two");
    }
}
