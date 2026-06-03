#![forbid(unsafe_code)]

//! Chat panel orchestrator — renders messages with color-coded styling,
//! delegates to sub-modules for markdown, tool calls, and streaming indicators.

pub mod markdown;
pub mod streaming;
pub mod timeline;
pub mod tool_call;

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph},
};

use crate::app::{AppState, OutputLineKind};
use crate::ui::theme;

/// Wrap `text` to `width` columns, word-aware, char-based (UTF-8 safe).
///
/// Splits on existing newlines first, then greedily packs words. Words longer
/// than `width` are hard-split on char boundaries (never byte-sliced, so
/// accented Portuguese text never panics). Returns at least one line per
/// input newline segment so blank lines are preserved.
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut out: Vec<String> = Vec::new();

    for raw in text.split('\n') {
        if raw.is_empty() {
            out.push(String::new());
            continue;
        }
        let mut line = String::new();
        let mut line_len = 0usize;
        let mut emitted = false;

        for word in raw.split(' ') {
            let wlen = word.chars().count();

            // Word longer than the whole width → flush, then hard-split.
            if wlen > width {
                if !line.is_empty() {
                    out.push(std::mem::take(&mut line));
                    line_len = 0;
                    emitted = true;
                }
                let chars: Vec<char> = word.chars().collect();
                let mut i = 0;
                while i < chars.len() {
                    let stop = (i + width).min(chars.len());
                    out.push(chars[i..stop].iter().collect());
                    i = stop;
                    emitted = true;
                }
                continue;
            }

            let add = if line.is_empty() { wlen } else { wlen + 1 };
            if line_len + add > width {
                out.push(std::mem::take(&mut line));
                emitted = true;
                line.push_str(word);
                line_len = wlen;
            } else {
                if !line.is_empty() {
                    line.push(' ');
                }
                line.push_str(word);
                line_len += add;
            }
        }
        // Push the trailing partial line. Only emit an empty line when nothing
        // else was produced for this segment (preserves intentional blanks
        // without appending a spurious empty row after a hard-split word).
        if !line.is_empty() || !emitted {
            out.push(line);
        }
    }

    out
}

/// Render a thinking/reasoning block as a collapsible section.
///
/// When collapsed, shows a single line: "▶ Thinking... (press t to expand)" in dim/gray style.
/// When expanded, shows:
/// - Header: "▼ Thinking" in dim style
/// - Content lines in dim/italic style
/// - Footer: "── end thinking ──" in dim style
pub fn render_thinking_block(
    content: &str,
    collapsed: bool,
    area_width: u16,
) -> Vec<Line<'static>> {
    let dim_style = theme::dim();

    if collapsed {
        let line = Line::from(vec![Span::styled(
            "▶ Thinking... (press t to expand)".to_string(),
            dim_style,
        )]);
        return vec![line];
    }

    // Expanded state
    let mut lines: Vec<Line<'static>> = Vec::new();

    // Header
    lines.push(Line::from(vec![Span::styled(
        "▼ Thinking".to_string(),
        dim_style,
    )]));

    // Content lines in dim + italic
    let content_style = theme::dim().add_modifier(Modifier::ITALIC);

    let usable_width = if area_width > 2 {
        area_width as usize - 2
    } else {
        1
    };

    // Account for the 2-space indent applied to each content line. Empty
    // content yields no content rows (header + footer only).
    if !content.is_empty() {
        let wrap_width = usable_width.saturating_sub(2).max(1);
        for chunk in wrap_text(content, wrap_width) {
            lines.push(Line::from(vec![Span::styled(
                format!("  {chunk}"),
                content_style,
            )]));
        }
    }

    // Footer
    lines.push(Line::from(vec![Span::styled(
        "── end thinking ──".to_string(),
        dim_style,
    )]));

    lines
}

/// Render the chat panel within the given area.
///
/// Displays messages from `state.output` with color-coded styling per
/// `OutputLineKind`, supports scroll offset navigation, and shows a streaming
/// spinner when an agent is actively streaming.
pub fn render_chat(frame: &mut Frame, state: &AppState, area: Rect) {
    // Border takes 2 rows (top + bottom), so visible content height is reduced.
    let visible_height = area.height.saturating_sub(2) as usize;

    if visible_height == 0 {
        // Not enough space to render anything meaningful — just draw the block.
        frame.render_widget(theme::block(" kira "), area);
        return;
    }

    // The live status strip (spinner + "what it's doing") gets its own reserved
    // row at the bottom so it never paints over the last line of the answer.
    let status_line = build_status_line(state);
    let reserved = usize::from(status_line.is_some());
    let content_height = visible_height.saturating_sub(reserved).max(1);

    // Expand EVERY source line into rendered rows first (markdown, wrapping,
    // cards). Scrolling then operates in rendered-row space below — one source
    // line can be dozens of rows, so windowing in source space would make
    // scroll jump over whole answers.
    let usable_width = area.width.saturating_sub(2) as usize;
    // Gutter-style "cards": each message gets a colored left bar (▌) + a role
    // header, visually separating the user's question from Kira's answer. The
    // 2-col gutter eats into the content width.
    let gutter = "▌ ";
    let content_width = usable_width.saturating_sub(gutter.chars().count());
    let mut items: Vec<ListItem> = Vec::new();
    let mut prev_role: Option<u8> = None; // 0 = user, 1 = agent

    for (out_idx, line) in state.output.iter().enumerate() {
        match &line.kind {
            OutputLineKind::Activity(item) => {
                prev_role = None;
                let mut chip = timeline::render_activity_item(item, state.spinner_frame);
                // Highlight the keyboard-focused chip (Kiro navigation).
                if state.activity_focus == Some(out_idx) {
                    chip = chip.style(
                        Style::default()
                            .bg(theme::ACCENT)
                            .fg(ratatui::style::Color::Black),
                    );
                }
                items.push(chip);
            }
            // Agent answers are markdown — headings, bold, tables, code blocks.
            OutputLineKind::Agent => {
                if prev_role != Some(1) {
                    items.push(ListItem::new(Line::from("")));
                    items.push(ListItem::new(Line::from(Span::styled(
                        "▌ Kira",
                        Style::default().fg(theme::SUCCESS).add_modifier(Modifier::BOLD),
                    ))));
                    prev_role = Some(1);
                }
                #[allow(clippy::cast_possible_truncation)]
                let rendered = markdown::wrap_lines(
                    markdown::render_markdown(&line.text, content_width as u16),
                    content_width,
                );
                for mdline in rendered {
                    let mut spans = vec![Span::styled(gutter, Style::default().fg(theme::SUCCESS))];
                    spans.extend(mdline.spans);
                    items.push(ListItem::new(Line::from(spans)));
                }
            }
            OutputLineKind::User => {
                if prev_role != Some(0) {
                    items.push(ListItem::new(Line::from("")));
                    items.push(ListItem::new(Line::from(Span::styled(
                        "▌ Você",
                        Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD),
                    ))));
                    prev_role = Some(0);
                }
                for chunk in wrap_text(&line.text, content_width) {
                    let spans = vec![
                        Span::styled(gutter, Style::default().fg(theme::ACCENT)),
                        Span::styled(chunk, Style::default().fg(theme::TEXT)),
                    ];
                    items.push(ListItem::new(Line::from(spans)));
                }
            }
            kind => {
                prev_role = None;
                let style = style_for_kind(kind);
                for chunk in wrap_text(&line.text, usable_width) {
                    items.push(ListItem::new(chunk).style(style));
                }
            }
        }
    }

    // Publish the full rendered-row count so the scroll clamp spans the whole
    // expanded view (read by `scroll_max_offset`).
    let total_rows = items.len();
    state.rendered_rows.set(total_rows);

    // Window in rendered-row space: offset 0 = bottom (newest). Clamp the
    // offset so it can't scroll past the top.
    if total_rows > content_height {
        let max_off = total_rows - content_height;
        let off = (state.scroll_offset as usize).min(max_off);
        let end = total_rows - off;
        let start = end.saturating_sub(content_height);
        items = items.split_off(start);
        items.truncate(end - start);
    }

    // Build the block title — show scroll offset when scrolled up.
    let title = build_title(state);
    let block = theme::block(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Content fills the inner area minus the reserved strip row.
    let content_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: inner.height.saturating_sub(reserved as u16),
    };
    frame.render_widget(List::new(items), content_area);

    // Live activity strip on its own reserved bottom row — no overlap.
    if let Some(line) = status_line {
        let strip = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        frame.render_widget(Paragraph::new(line), strip);
    }
}

/// Build the live activity status line — answers "what is the agent doing now".
///
/// Priority:
/// 1. An active pipeline layer + its latest event message (layer-colored).
/// 2. An active agent + its kind/short id + last output snippet.
/// 3. A generic "streaming…" while a response streams.
/// 4. `None` when idle (nothing rendered).
fn build_status_line(state: &AppState) -> Option<Line<'static>> {
    let spinner = Span::styled(
        format!("{} ", theme::spinner_char(state.spinner_frame)),
        Style::default()
            .fg(theme::ACCENT)
            .add_modifier(Modifier::BOLD),
    );

    // 1. Active pipeline layer.
    if let Some((layer, event)) = state.layers_state.latest_active() {
        let color = crate::layers::indicators::layer_color(layer);
        let mut spans = vec![
            spinner,
            Span::styled(
                layer.label().to_string(),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
        ];
        if !event.message.is_empty() {
            spans.push(Span::styled(
                format!("  {}", truncate(&event.message, 60)),
                theme::dim(),
            ));
        }
        return Some(Line::from(spans));
    }

    // 2. Active agent.
    if let Some(agent) = state.active_agents.last() {
        let mut spans = vec![
            spinner,
            Span::styled(
                format!("{} ", agent.kind),
                Style::default()
                    .fg(theme::SUCCESS)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("[{}]", agent.short_id), Style::default().fg(theme::ACCENT)),
        ];
        if let Some(out) = &agent.last_output {
            spans.push(Span::styled(format!("  {}", truncate(out, 50)), theme::dim()));
        }
        return Some(Line::from(spans));
    }

    // 3. Generic streaming.
    if state.streaming_active {
        return Some(Line::from(vec![
            spinner,
            Span::styled("streaming…", Style::default().fg(theme::ACCENT)),
        ]));
    }

    None
}

/// Truncate a string to `max` chars, appending "…" when cut.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let head: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{head}…")
    }
}

/// Map `OutputLineKind` to a foreground color style.
fn style_for_kind(kind: &OutputLineKind) -> Style {
    match kind {
        OutputLineKind::User => Style::default().fg(theme::TEXT),
        OutputLineKind::Agent => Style::default().fg(theme::SUCCESS),
        OutputLineKind::System => Style::default().fg(theme::WARNING),
        OutputLineKind::Error => Style::default().fg(theme::DANGER),
        // Activity lines are handled before style_for_kind is called.
        OutputLineKind::Activity(_) => Style::default().fg(theme::TEXT_DIM),
    }
}

/// Build the block title string.
/// Shows scroll offset indicator when the user has scrolled up.
fn build_title(state: &AppState) -> String {
    if state.scroll_offset > 0 {
        format!(" kira [↑{}] ", state.scroll_offset)
    } else {
        " kira ".to_string()
    }
}

/// Compute the clamped scroll offset after a scroll action.
///
/// Returns the new scroll offset clamped to `[0, max(0, total_lines - visible_height)]`.
/// The `delta` is positive for scrolling up (increasing offset) and negative for
/// scrolling down (decreasing offset).
pub fn compute_scroll_offset(
    current_offset: u16,
    delta: i16,
    total_lines: usize,
    visible_height: usize,
) -> u16 {
    let max_offset = total_lines.saturating_sub(visible_height);

    // Apply delta to current offset (handle negative by using saturating ops).
    let new_offset = if delta >= 0 {
        #[allow(clippy::cast_sign_loss)]
        let d = delta as u16;
        current_offset.saturating_add(d)
    } else {
        #[allow(clippy::cast_sign_loss)]
        let d = delta.unsigned_abs();
        current_offset.saturating_sub(d)
    };

    // Clamp to [0, max_offset].
    #[allow(clippy::cast_possible_truncation)]
    let max_offset_u16 = max_offset.min(u16::MAX as usize) as u16;
    new_offset.min(max_offset_u16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AppState, Mode, OutputLineKind, Tier};
    use crate::config::Config;
    use ratatui::style::Color;

    /// Helper to create a default AppState for testing.
    fn test_state() -> AppState {
        AppState::new(Config::default(), Mode::Plan, Tier::Standard)
    }

    #[test]
    fn style_for_user_is_white() {
        let style = style_for_kind(&OutputLineKind::User);
        assert_eq!(style.fg, Some(Color::White));
    }

    #[test]
    fn style_for_agent_is_green() {
        let style = style_for_kind(&OutputLineKind::Agent);
        assert_eq!(style.fg, Some(Color::Green));
    }

    #[test]
    fn style_for_system_is_yellow() {
        let style = style_for_kind(&OutputLineKind::System);
        assert_eq!(style.fg, Some(Color::Yellow));
    }

    #[test]
    fn style_for_error_is_red() {
        let style = style_for_kind(&OutputLineKind::Error);
        assert_eq!(style.fg, Some(Color::Red));
    }

    fn line_text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn status_line_none_when_idle() {
        let state = test_state();
        assert!(build_status_line(&state).is_none());
    }

    #[test]
    fn status_line_shows_streaming_when_active() {
        let mut state = test_state();
        state.streaming_active = true;
        let line = build_status_line(&state).expect("status line");
        assert!(line_text(&line).contains("streaming"));
    }

    #[test]
    fn status_line_shows_active_agent() {
        let mut state = test_state();
        state.active_agents.push(crate::messages::AgentThread {
            id: "abc12345-0000-0000-0000-000000000000".to_string(),
            short_id: "abc12345".to_string(),
            kind: "writer".to_string(),
            status: "running".to_string(),
            cost_usd: 0.0,
            last_output: Some("editing main.rs".to_string()),
        });
        let line = build_status_line(&state).expect("status line");
        let text = line_text(&line);
        assert!(text.contains("writer"));
        assert!(text.contains("abc12345"));
        assert!(text.contains("editing main.rs"));
    }

    #[test]
    fn truncate_cuts_long_strings() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 5), "hell…");
    }

    #[test]
    fn title_shows_offset_when_scrolled() {
        let mut state = test_state();
        state.scroll_offset = 5;
        let title = build_title(&state);
        assert_eq!(title, " kira [↑5] ");
    }

    #[test]
    fn title_plain_when_at_bottom() {
        let state = test_state();
        let title = build_title(&state);
        assert_eq!(title, " kira ");
    }

    #[test]
    fn compute_scroll_offset_clamps_to_max() {
        // 100 total lines, 20 visible → max offset = 80
        let result = compute_scroll_offset(0, 100, 100, 20);
        assert_eq!(result, 80);
    }

    #[test]
    fn compute_scroll_offset_clamps_to_zero() {
        // Scrolling down past zero stays at zero
        let result = compute_scroll_offset(5, -10, 100, 20);
        assert_eq!(result, 0);
    }

    #[test]
    fn compute_scroll_offset_no_scroll_when_content_fits() {
        // 10 total lines, 20 visible → max offset = 0
        let result = compute_scroll_offset(0, 5, 10, 20);
        assert_eq!(result, 0);
    }

    #[test]
    fn compute_scroll_offset_positive_delta() {
        // 100 total, 20 visible, currently at 10, scroll up by 5 → 15
        let result = compute_scroll_offset(10, 5, 100, 20);
        assert_eq!(result, 15);
    }

    #[test]
    fn compute_scroll_offset_negative_delta() {
        // Currently at 10, scroll down by 3 → 7
        let result = compute_scroll_offset(10, -3, 100, 20);
        assert_eq!(result, 7);
    }

    // ── Thinking block tests ───────────────────────────────────────────────

    #[test]
    fn thinking_block_collapsed_renders_single_line() {
        let lines = render_thinking_block("some reasoning content", true, 80);
        assert_eq!(lines.len(), 1);
        let text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("▶ Thinking..."));
        assert!(text.contains("press t to expand"));
    }

    #[test]
    fn thinking_block_collapsed_has_dim_style() {
        let lines = render_thinking_block("content", true, 80);
        assert_eq!(lines.len(), 1);
        let span = &lines[0].spans[0];
        assert_eq!(span.style.fg, Some(Color::Gray));
        assert!(span.style.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn thinking_block_expanded_has_header_content_footer() {
        let content = "Line one\nLine two\nLine three";
        let lines = render_thinking_block(content, false, 80);

        // Should have: 1 header + 3 content lines + 1 footer = 5 lines
        assert_eq!(lines.len(), 5);

        // Header
        let header_text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(header_text.contains("▼ Thinking"));

        // Content lines are indented
        let content_line_1: String = lines[1].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(content_line_1.contains("Line one"));

        let content_line_2: String = lines[2].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(content_line_2.contains("Line two"));

        let content_line_3: String = lines[3].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(content_line_3.contains("Line three"));

        // Footer
        let footer_text: String = lines[4].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(footer_text.contains("── end thinking ──"));
    }

    #[test]
    fn thinking_block_expanded_content_is_dim_italic() {
        let lines = render_thinking_block("some thought", false, 80);
        // lines[1] is the first content line
        let span = &lines[1].spans[0];
        assert_eq!(span.style.fg, Some(Color::Gray));
        assert!(span.style.add_modifier.contains(Modifier::DIM));
        assert!(span.style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn thinking_block_expanded_header_is_dim_not_italic() {
        let lines = render_thinking_block("thought", false, 80);
        let header_span = &lines[0].spans[0];
        assert_eq!(header_span.style.fg, Some(Color::Gray));
        assert!(header_span.style.add_modifier.contains(Modifier::DIM));
        assert!(!header_span.style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn thinking_block_expanded_footer_is_dim_not_italic() {
        let lines = render_thinking_block("thought", false, 80);
        let footer_span = lines.last().map(|l| &l.spans[0]);
        let footer_span = footer_span.unwrap_or_else(|| panic!("expected footer line"));
        assert_eq!(footer_span.style.fg, Some(Color::Gray));
        assert!(footer_span.style.add_modifier.contains(Modifier::DIM));
        assert!(!footer_span.style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn thinking_block_empty_content_expanded() {
        let lines = render_thinking_block("", false, 80);
        // Header + footer = 2 (empty string produces no content lines via .lines())
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn thinking_block_wraps_long_lines() {
        // Create a line longer than the available width
        let long_line = "a".repeat(100);
        let lines = render_thinking_block(&long_line, false, 30);
        // With area_width=30, usable_width=28, wrap_width=26
        // 100 chars / 26 = 4 wrapped lines (ceil)
        // Total: 1 header + 4 content lines + 1 footer = 6
        assert!(lines.len() > 3); // More than header + 1 line + footer
    }

    #[test]
    fn thinking_block_collapsed_empty_content() {
        let lines = render_thinking_block("", true, 80);
        assert_eq!(lines.len(), 1);
        let text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("▶ Thinking..."));
    }

    #[test]
    fn thinking_block_narrow_width() {
        // Very narrow width should still produce valid output
        let lines = render_thinking_block("hello world", false, 5);
        // Should not panic and should produce at least header + content + footer
        assert!(lines.len() >= 3);
    }

    #[test]
    fn wrap_text_word_aware() {
        let out = wrap_text("the quick brown fox", 10);
        assert_eq!(out, vec!["the quick", "brown fox"]);
        // Every produced line fits within the width.
        assert!(out.iter().all(|l| l.chars().count() <= 10));
    }

    #[test]
    fn wrap_text_hard_splits_long_word() {
        let out = wrap_text(&"a".repeat(25), 10);
        assert_eq!(out, vec!["aaaaaaaaaa", "aaaaaaaaaa", "aaaaa"]);
    }

    #[test]
    fn wrap_text_multibyte_no_panic() {
        // Portuguese accents are multi-byte: char-based wrapping must never
        // slice mid-codepoint (byte slicing would panic here).
        let out = wrap_text("configuração graciosa açúcar coração ação", 8);
        assert!(out.iter().all(|l| l.chars().count() <= 8));
        // All original words still present across the wrapped output.
        let joined: String = out.join("");
        for w in ["graciosa", "açúcar", "coração", "ação"] {
            assert!(joined.contains(w), "missing {w}");
        }
    }

    #[test]
    fn wrap_text_preserves_blank_lines() {
        let out = wrap_text("a\n\nb", 10);
        assert_eq!(out, vec!["a", "", "b"]);
    }
}
