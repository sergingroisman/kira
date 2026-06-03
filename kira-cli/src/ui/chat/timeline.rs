#![forbid(unsafe_code)]

//! Action timeline — kiro-style activity feed for the chat panel.
//!
//! Each agent tool call is shown as an icon + verb + target chip row, with a
//! running spinner (→ done ✓ → error ✗). This module provides:
//!
//! - [`parse_activity`]: heuristic parse of a raw `AgentOutput` line into an
//!   [`ActivityItem`] (CLI-only fallback; upgrade to typed Core SSE events later).
//! - [`render_activity_item`]: render one `ActivityItem` as a styled `ListItem`.

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::ListItem,
};

use crate::models::{ActivityItem, ActivityStatus};
use crate::ui::theme;

// ── Parsing ────────────────────────────────────────────────────────────────

/// Try to parse a raw agent output `line` into a structured `ActivityItem`.
///
/// Returns `Some` on a recognized pattern, `None` for plain text that should
/// fall through to the normal output renderer.
///
/// Patterns recognized (case-insensitive prefix matching):
/// | Prefix (starts_with)              | Icon | Verb      |
/// |-----------------------------------|------|-----------|
/// | `read file`/`reading file`        | `◎`  | Read      |
/// | `read (`/`read [`                 | `◎`  | Read      |
/// | `search`/`searching`/`searched`   | `⌕`  | Searched  |
/// | `edit`/`editing`/`wrote`          | `✎`  | Edit      |
/// | `create`/`creating`/`created`     | `✎`  | Creating  |
/// | `write`/`writing`/`written`       | `✎`  | Write     |
/// | `run`/`running`/`ran`/`bash`/`$`  | `❯`  | Ran       |
/// | `glob`/`find file`                | `⊛`  | Glob      |
/// | `grep`/`ripgrep`/`rg `            | `⌕`  | Grep      |
/// | `done`/`finished`/`completed`     | `✓`  | Done      |
/// | `error`/`failed`/`fail`           | `✗`  | Error     |
/// Build an activity chip from a typed tool event (Kiro-style). Deterministic
/// mapping tool→verb/icon; status from whether the call succeeded.
/// Icon + verb for a tool name (single source of truth for chip identity).
pub fn tool_icon_verb(tool: &str) -> (&'static str, &'static str) {
    match tool {
        "file_read" | "read_symbol" => ("◎", "Read"),
        "code_search" => ("⌕", "Searched"),
        "glob" | "find" => ("⊛", "Globbed"),
        "web_search" => ("⌕", "Web search"),
        "web_fetch" => ("↗", "Fetched"),
        "file_write" => ("✎", "Wrote"),
        "edit" => ("✎", "Edited"),
        "bash" => ("❯", "Ran"),
        _ => ("•", "Tool"),
    }
}

/// A Running (spinner) chip for a tool that just started.
pub fn chip_running(tool: &str, target: &str) -> ActivityItem {
    let (icon, verb) = tool_icon_verb(tool);
    ActivityItem {
        icon,
        verb,
        target: target.to_string(),
        subtitle: None,
        status: ActivityStatus::Running,
        collapsed: true,
        detail: Vec::new(),
    }
}

pub fn chip_for_tool(tool: &str, target: &str, ok: bool, detail: Vec<String>) -> ActivityItem {
    let (icon, verb) = tool_icon_verb(tool);
    ActivityItem {
        icon,
        verb,
        target: target.to_string(),
        subtitle: None,
        status: if ok { ActivityStatus::Done } else { ActivityStatus::Error },
        collapsed: true, // start collapsed, like Kiro
        detail,
    }
}

pub fn parse_activity(line: &str) -> Option<ActivityItem> {
    let lower = line.trim().to_lowercase();

    if lower.is_empty() {
        return None;
    }

    // --- Read ---
    if lower.starts_with("read file")
        || lower.starts_with("reading file")
        || lower.starts_with("read (")
        || lower.starts_with("read [")
    {
        let target = extract_target(line, &["read file", "reading file", "read"]);
        return Some(ActivityItem {
            icon: "◎",
            verb: "Read",
            target,
            subtitle: None,
            status: ActivityStatus::Done,
            detail: Vec::new(),
            collapsed: false,
        });
    }

    // --- Grep / search ---
    if lower.starts_with("grep")
        || lower.starts_with("ripgrep")
        || lower.starts_with("rg ")
        || lower.starts_with("rg\t")
    {
        let target = extract_target(line, &["ripgrep", "grep", "rg"]);
        return Some(ActivityItem {
            icon: "⌕",
            verb: "Grep",
            target,
            subtitle: None,
            status: ActivityStatus::Done,
            detail: Vec::new(),
            collapsed: false,
        });
    }

    // --- Search ---
    if lower.starts_with("search")
        || lower.starts_with("searching")
        || lower.starts_with("searched")
    {
        let target = extract_target(line, &["searched workspace", "searching", "searched", "search"]);
        return Some(ActivityItem {
            icon: "⌕",
            verb: "Searched",
            target,
            subtitle: None,
            status: ActivityStatus::Done,
            detail: Vec::new(),
            collapsed: false,
        });
    }

    // --- Glob / find ---
    if lower.starts_with("glob") || lower.starts_with("find file") {
        let target = extract_target(line, &["find file", "glob"]);
        return Some(ActivityItem {
            icon: "⊛",
            verb: "Glob",
            target,
            subtitle: None,
            status: ActivityStatus::Done,
            detail: Vec::new(),
            collapsed: false,
        });
    }

    // --- Create ---
    if lower.starts_with("creat") {
        let status = if lower.starts_with("creating") {
            ActivityStatus::Running
        } else {
            ActivityStatus::Done
        };
        let target = extract_target(line, &["creating", "created", "create"]);
        return Some(ActivityItem {
            icon: "✎",
            verb: if status == ActivityStatus::Running { "Creating" } else { "Created" },
            target,
            subtitle: None,
            status,
            detail: Vec::new(),
            collapsed: false,
        });
    }

    // --- Write ---
    if lower.starts_with("writ") {
        let status = if lower.starts_with("writing") {
            ActivityStatus::Running
        } else {
            ActivityStatus::Done
        };
        let target = extract_target(line, &["writing", "written", "wrote", "write"]);
        return Some(ActivityItem {
            icon: "✎",
            verb: if status == ActivityStatus::Running { "Writing" } else { "Write" },
            target,
            subtitle: None,
            status,
            detail: Vec::new(),
            collapsed: false,
        });
    }

    // --- Edit ---
    if lower.starts_with("edit") {
        let status = if lower.starts_with("editing") {
            ActivityStatus::Running
        } else {
            ActivityStatus::Done
        };
        let target = extract_target(line, &["editing", "edited", "edit"]);
        return Some(ActivityItem {
            icon: "✎",
            verb: if status == ActivityStatus::Running { "Editing" } else { "Edit" },
            target,
            subtitle: None,
            status,
            detail: Vec::new(),
            collapsed: false,
        });
    }

    // --- Bash / run ---
    if lower.starts_with("bash")
        || lower.starts_with("run ")
        || lower.starts_with("running ")
        || lower.starts_with("ran ")
        || lower.starts_with("$ ")
    {
        let status = if lower.starts_with("running ") {
            ActivityStatus::Running
        } else {
            ActivityStatus::Done
        };
        let target = extract_target(line, &["running", "bash", "run", "ran", "$"]);
        return Some(ActivityItem {
            icon: "❯",
            verb: if status == ActivityStatus::Running { "Running" } else { "Ran" },
            target,
            subtitle: None,
            status,
            detail: Vec::new(),
            collapsed: false,
        });
    }

    // --- Done / completed ---
    if lower.starts_with("done") || lower.starts_with("finished") || lower.starts_with("completed") {
        let target = extract_target(line, &["completed", "finished", "done"]);
        return Some(ActivityItem {
            icon: "✓",
            verb: "Done",
            target,
            subtitle: None,
            status: ActivityStatus::Done,
            detail: Vec::new(),
            collapsed: false,
        });
    }

    // --- Error / failed ---
    if lower.starts_with("error") || lower.starts_with("failed") || lower.starts_with("fail ") {
        let target = extract_target(line, &["failed", "error", "fail"]);
        return Some(ActivityItem {
            icon: "✗",
            verb: "Error",
            target,
            subtitle: None,
            status: ActivityStatus::Error,
            detail: Vec::new(),
            collapsed: false,
        });
    }

    None
}

/// Strip the recognized prefix from `line`, trim, and return as the target
/// string (capped to 60 chars with "…" if needed).
fn extract_target(line: &str, prefixes: &[&str]) -> String {
    let lower = line.to_lowercase();
    for prefix in prefixes {
        if lower.starts_with(prefix) {
            let rest = line[prefix.len()..].trim_start_matches([' ', ':', '-', '\t']).trim();
            return cap(rest, 60);
        }
    }
    cap(line, 60)
}

/// Cap a string to `max` chars, appending "…" when truncated.
fn cap(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let head: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{head}…")
    }
}

// ── Rendering ──────────────────────────────────────────────────────────────

/// Build the styled `Line` for an activity item.
///
/// Layout: `  {status_glyph} {icon} {verb}  [{target}]  {subtitle?}`
///
/// | State   | Glyph | Color   |
/// |---------|-------|---------|
/// | Running | ⠹     | Accent  |
/// | Done    | ✓     | Success |
/// | Error   | ✗     | Danger  |
pub fn activity_line(item: &ActivityItem, spinner_frame: u8) -> Line<'static> {
    let (status_glyph, status_color) = match &item.status {
        ActivityStatus::Running => (
            theme::spinner_char(spinner_frame).to_string(),
            theme::ACCENT,
        ),
        ActivityStatus::Done => ("✓".to_string(), theme::SUCCESS),
        ActivityStatus::Error => ("✗".to_string(), theme::DANGER),
    };

    // Disclosure triangle (Kiro ▶/▼) when the chip has an expandable detail.
    let disclosure = if item.detail.is_empty() {
        "  "
    } else if item.collapsed {
        "▶ "
    } else {
        "▼ "
    };
    let mut spans: Vec<Span<'static>> = vec![
        Span::styled(disclosure.to_string(), theme::dim()),
        Span::styled(
            format!("{status_glyph} "),
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{} ", item.icon),
            Style::default().fg(theme::TEXT_DIM),
        ),
        Span::styled(
            format!("{} ", item.verb),
            Style::default().fg(theme::TEXT).add_modifier(Modifier::BOLD),
        ),
    ];

    if !item.target.is_empty() {
        spans.push(Span::styled(
            format!("[{}]", item.target),
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ));
    }

    if let Some(subtitle) = &item.subtitle {
        spans.push(Span::styled(
            format!("  {}", cap(subtitle, 40)),
            theme::dim(),
        ));
    }

    Line::from(spans)
}

/// Render one `ActivityItem` as a `ListItem` for the chat panel. When expanded
/// (`!collapsed`), the detail lines are shown indented below the chip.
pub fn render_activity_item(item: &ActivityItem, spinner_frame: u8) -> ListItem<'static> {
    let mut lines = vec![activity_line(item, spinner_frame)];
    if !item.collapsed {
        for d in &item.detail {
            lines.push(Line::from(Span::styled(format!("      {d}"), theme::dim())));
        }
    }
    ListItem::new(lines)
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn line_text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    // ── Parser tests ──────────────────────────────────────────────────────

    #[test]
    fn chip_for_tool_starts_collapsed_with_detail() {
        let c = chip_for_tool("bash", "ls", true, vec!["a".into(), "b".into()]);
        assert_eq!(c.verb, "Ran");
        assert!(c.collapsed);
        assert_eq!(c.detail.len(), 2);
        assert_eq!(c.status, ActivityStatus::Done);
    }

    #[test]
    fn collapsed_chip_renders_one_line_expanded_shows_detail() {
        let mut c = chip_for_tool("file_read", "x.rs", true, vec!["line1".into(), "line2".into()]);
        // Collapsed: just the chip row.
        let collapsed = render_activity_item(&c, 0);
        // Expanded: chip + 2 detail rows.
        c.collapsed = false;
        let expanded = render_activity_item(&c, 0);
        // ListItem height = number of lines; expanded must be taller.
        assert!(expanded.height() > collapsed.height());
        assert_eq!(expanded.height(), collapsed.height() + 2);
    }

    #[test]
    fn parse_read_file() {
        let a = parse_activity("Read file: src/main.rs").expect("should parse");
        assert_eq!(a.verb, "Read");
        assert_eq!(a.icon, "◎");
        assert!(a.target.contains("src/main.rs"));
    }

    #[test]
    fn parse_search_workspace() {
        let a = parse_activity("Searched workspace for token definitions").expect("should parse");
        assert_eq!(a.verb, "Searched");
        assert_eq!(a.icon, "⌕");
    }

    #[test]
    fn parse_creating_file() {
        let a = parse_activity("Creating requirements.md").expect("should parse");
        assert_eq!(a.verb, "Creating");
        assert_eq!(a.status, ActivityStatus::Running);
        assert!(a.target.contains("requirements.md"));
    }

    #[test]
    fn parse_bash_dollar() {
        let a = parse_activity("$ cargo build --release").expect("should parse");
        assert_eq!(a.verb, "Ran");
        assert_eq!(a.icon, "❯");
    }

    #[test]
    fn parse_running_keeps_status() {
        let a = parse_activity("Running cargo test").expect("should parse");
        assert_eq!(a.status, ActivityStatus::Running);
        assert_eq!(a.verb, "Running");
    }

    #[test]
    fn parse_error_line() {
        let a = parse_activity("Error: file not found").expect("should parse");
        assert_eq!(a.status, ActivityStatus::Error);
        assert_eq!(a.verb, "Error");
    }

    #[test]
    fn parse_plain_text_returns_none() {
        assert!(parse_activity("This is agent reasoning output").is_none());
        assert!(parse_activity("I will now analyze the codebase").is_none());
        assert!(parse_activity("").is_none());
    }

    #[test]
    fn parse_grep_recognized() {
        let a = parse_activity("grep -r 'main' src/").expect("should parse");
        assert_eq!(a.verb, "Grep");
        assert_eq!(a.icon, "⌕");
    }

    // ── Renderer tests (use activity_line, not ListItem internals) ────────

    #[test]
    fn render_done_item_contains_check() {
        let item = ActivityItem {
            icon: "◎",
            verb: "Read",
            target: "store.rs".to_string(),
            subtitle: None,
            status: ActivityStatus::Done,
            detail: Vec::new(),
            collapsed: false,
        };
        let t = line_text(&activity_line(&item, 0));
        assert!(t.contains('✓'));
        assert!(t.contains("Read"));
        assert!(t.contains("store.rs"));
    }

    #[test]
    fn render_error_item_contains_cross() {
        let item = ActivityItem {
            icon: "✗",
            verb: "Error",
            target: "build failed".to_string(),
            subtitle: None,
            status: ActivityStatus::Error,
            detail: Vec::new(),
            collapsed: false,
        };
        let t = line_text(&activity_line(&item, 0));
        assert!(t.contains('✗'));
        assert!(t.contains("Error"));
    }

    #[test]
    fn render_running_item_uses_spinner() {
        let item = ActivityItem {
            icon: "✎",
            verb: "Creating",
            target: "PLAN.md".to_string(),
            subtitle: None,
            status: ActivityStatus::Running,
            detail: Vec::new(),
            collapsed: false,
        };
        let t = line_text(&activity_line(&item, 0));
        // Frame 0 spinner is "⠋"
        assert!(t.contains("⠋"));
        assert!(t.contains("PLAN.md"));
    }

    #[test]
    fn render_subtitle_shown() {
        let item = ActivityItem {
            icon: "⌕",
            verb: "Searched",
            target: "workspace".to_string(),
            subtitle: Some("pattern: agent pool".to_string()),
            status: ActivityStatus::Done,
            detail: Vec::new(),
            collapsed: false,
        };
        let t = line_text(&activity_line(&item, 3));
        assert!(t.contains("agent pool"));
    }

    #[test]
    fn cap_long_string() {
        let long = "a".repeat(80);
        let capped = cap(&long, 60);
        assert_eq!(capped.chars().count(), 60);
        assert!(capped.ends_with('…'));
    }

    #[test]
    fn cap_short_string_unchanged() {
        assert_eq!(cap("hello", 60), "hello");
    }
}
