#![forbid(unsafe_code)]

//! Footer bar widget — renders workspace path, git branch, LSP count,
//! MCP count, and context-sensitive keybinding hints.

use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::AppState;
use crate::ui::theme;

/// Abbreviate a workspace path if it exceeds 30 characters.
///
/// If the path length is > 30, returns "…" followed by the last 27 characters.
/// Otherwise returns the path unchanged.
fn abbreviate_path(path: &str) -> String {
    if path.len() > 30 {
        let suffix_start = path.len().saturating_sub(27);
        format!("…{}", &path[suffix_start..])
    } else {
        path.to_string()
    }
}

/// Render the footer bar into the given area.
///
/// Displays: workspace path (abbreviated), git branch, LSP count, MCP count,
/// and context-sensitive keybinding hints. Background is DarkGray for the
/// entire bar.
pub fn render_footer(frame: &mut Frame, state: &AppState, area: Rect) {
    let mut spans: Vec<Span> = Vec::new();

    // ── Workspace path ─────────────────────────────────────────────────────
    let path_display = abbreviate_path(&state.workspace_path);
    spans.push(Span::styled(
        format!(" {path_display}"),
        Style::default().fg(theme::TEXT),
    ));

    // ── Git branch ─────────────────────────────────────────────────────────
    if let Some(ref branch) = state.git_branch {
        spans.push(Span::styled(
            format!("  {branch}"),
            Style::default().fg(theme::ACCENT),
        ));
    }

    // ── LSP count ──────────────────────────────────────────────────────────
    spans.push(Span::styled(
        format!("  LSP:{}", state.lsp_count),
        Style::default().fg(theme::TEXT),
    ));

    // ── MCP count ──────────────────────────────────────────────────────────
    spans.push(Span::styled(
        format!("  MCP:{}", state.mcp_count),
        Style::default().fg(theme::TEXT),
    ));

    // ── Keybinding hints (dim/gray) ────────────────────────────────────────
    let hints = keybinding_hints(state);
    if !hints.is_empty() {
        spans.push(Span::styled(format!("  {hints}"), theme::dim()));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).style(Style::default().bg(theme::SURFACE));
    frame.render_widget(paragraph, area);
}

/// Return context-sensitive keybinding hints based on current state.
///
/// When no overlay is active and the Layers View is hidden, includes
/// a "^L layers" hint to aid discoverability (Requirement 12.3).
const fn keybinding_hints(state: &AppState) -> &'static str {
    if state.overlay_stack.is_empty() {
        if state.layers_visible {
            "^P palette  ^O mode  ^E expand  Tab sidebar  ? help"
        } else {
            "^P palette  ^O mode  ^L layers  ^E expand  Tab sidebar  ? help"
        }
    } else {
        "Esc close  ↑↓ navigate  Enter select"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abbreviate_short_path() {
        let path = "/home/user/project";
        assert_eq!(abbreviate_path(path), path);
    }

    #[test]
    fn abbreviate_exactly_30_chars() {
        // Exactly 30 chars should not be abbreviated
        let path = "123456789012345678901234567890";
        assert_eq!(path.len(), 30);
        assert_eq!(abbreviate_path(path), path);
    }

    #[test]
    fn abbreviate_long_path() {
        let path = "/Users/avanade/Workspace/kira/some/deep/nested/path";
        assert!(path.len() > 30);
        let result = abbreviate_path(path);
        // Should start with "…" and be 28 chars total (1 for "…" + 27 suffix)
        assert!(result.starts_with('…'));
        // The "…" character is 3 bytes in UTF-8, but we care about char count
        assert_eq!(result.chars().count(), 28);
    }

    #[test]
    fn abbreviate_empty_path() {
        assert_eq!(abbreviate_path(""), "");
    }

    #[test]
    fn abbreviate_31_char_path() {
        let path = "1234567890123456789012345678901";
        assert_eq!(path.len(), 31);
        let result = abbreviate_path(path);
        assert!(result.starts_with('…'));
        // Last 27 chars of the original
        assert!(result.ends_with("5678901"));
    }
}
