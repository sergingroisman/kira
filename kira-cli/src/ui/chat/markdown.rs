#![forbid(unsafe_code)]

//! Markdown renderer — converts markdown text to Ratatui `Line`/`Span`
//! sequences using `pulldown-cmark` for parsing and `syntect` for syntax
//! highlighting of fenced code blocks.

use std::sync::OnceLock;

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;

/// Lazily-initialized syntax set for code highlighting.
static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();

/// Lazily-initialized theme for code highlighting.
static THEME: OnceLock<Theme> = OnceLock::new();

fn get_syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn get_theme() -> &'static Theme {
    THEME.get_or_init(|| {
        let ts = ThemeSet::load_defaults();
        ts.themes
            .get("base16-ocean.dark")
            .cloned()
            .unwrap_or_else(|| {
                // Fallback: use any available theme
                ts.themes.values().next().cloned().unwrap_or_default()
            })
    })
}

/// Render a markdown string into a vector of styled Ratatui `Line`s.
///
/// Uses `pulldown-cmark` to parse the markdown and converts events into
/// styled spans. Fenced code blocks are highlighted via `syntect`.
///
/// Falls back to plain text rendering on any parse failure.
pub fn render_markdown(raw: &str, area_width: u16) -> Vec<Line<'static>> {
    if raw.is_empty() {
        return vec![Line::from("")];
    }

    let options =
        Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES | Options::ENABLE_HEADING_ATTRIBUTES;

    let parser = Parser::new_ext(raw, options);
    let events: Vec<Event<'_>> = parser.collect();

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut current_spans: Vec<Span<'static>> = Vec::new();

    // Style state tracking
    let mut bold = false;
    let mut italic = false;
    let mut in_code_block = false;
    let mut code_block_lang = String::new();
    let mut code_block_content = String::new();
    let mut heading_level: Option<HeadingLevel> = None;
    let mut in_list = false;
    let mut list_ordered = false;
    let mut list_item_index: u64 = 0;
    let mut list_item_started = false;
    // Table accumulation (GFM tables → aligned grid).
    let mut in_table_cell = false;
    let mut table_rows: Vec<Vec<String>> = Vec::new();
    let mut table_row: Vec<String> = Vec::new();
    let mut table_cell = String::new();

    for event in events {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    // Flush current line before heading
                    flush_line(&mut lines, &mut current_spans);
                    heading_level = Some(level);
                }
                Tag::Emphasis => {
                    italic = true;
                }
                Tag::Strong => {
                    bold = true;
                }
                Tag::CodeBlock(kind) => {
                    flush_line(&mut lines, &mut current_spans);
                    in_code_block = true;
                    code_block_content.clear();
                    code_block_lang = match kind {
                        pulldown_cmark::CodeBlockKind::Fenced(lang) => lang.to_string(),
                        pulldown_cmark::CodeBlockKind::Indented => String::new(),
                    };
                }
                Tag::List(start) => {
                    in_list = true;
                    if let Some(n) = start {
                        list_ordered = true;
                        list_item_index = n;
                    } else {
                        list_ordered = false;
                        list_item_index = 0;
                    }
                }
                Tag::Item => {
                    flush_line(&mut lines, &mut current_spans);
                    list_item_started = true;
                }
                Tag::Paragraph => {
                    // Start a new paragraph — flush any pending line
                    flush_line(&mut lines, &mut current_spans);
                }
                Tag::Table(_) => {
                    flush_line(&mut lines, &mut current_spans);
                    table_rows.clear();
                }
                Tag::TableHead | Tag::TableRow => {
                    table_row.clear();
                }
                Tag::TableCell => {
                    in_table_cell = true;
                    table_cell.clear();
                }
                _ => {}
            },
            Event::End(tag_end) => match tag_end {
                TagEnd::Heading(_) => {
                    // Apply heading style to accumulated spans and flush
                    let level = heading_level.take();
                    let indent = heading_indent(level);
                    let styled_spans = apply_heading_style(&current_spans, indent);
                    lines.push(Line::from(styled_spans));
                    current_spans.clear();
                }
                TagEnd::Emphasis => {
                    italic = false;
                }
                TagEnd::Strong => {
                    bold = false;
                }
                TagEnd::CodeBlock => {
                    // Render the accumulated code block
                    let highlighted = highlight_code_block(&code_block_lang, &code_block_content);
                    lines.extend(highlighted);
                    in_code_block = false;
                    code_block_lang.clear();
                    code_block_content.clear();
                }
                TagEnd::List(_) => {
                    in_list = false;
                    list_ordered = false;
                    list_item_index = 0;
                }
                TagEnd::Item | TagEnd::Paragraph => {
                    flush_line(&mut lines, &mut current_spans);
                }
                TagEnd::TableCell => {
                    in_table_cell = false;
                    table_row.push(std::mem::take(&mut table_cell));
                }
                TagEnd::TableHead | TagEnd::TableRow => {
                    table_rows.push(std::mem::take(&mut table_row));
                }
                TagEnd::Table => {
                    lines.extend(render_table(&table_rows, area_width as usize));
                    table_rows.clear();
                }
                _ => {}
            },
            Event::Text(text) => {
                if in_table_cell {
                    table_cell.push_str(&text);
                } else if in_code_block {
                    code_block_content.push_str(&text);
                } else {
                    let style = compute_inline_style(bold, italic);

                    // If this is the start of a list item, prepend the marker
                    if list_item_started && in_list {
                        let marker = if list_ordered {
                            let m = format!("{list_item_index}. ");
                            list_item_index = list_item_index.saturating_add(1);
                            m
                        } else {
                            "• ".to_string()
                        };
                        current_spans.push(Span::styled(format!("  {marker}"), Style::default()));
                        list_item_started = false;
                    }

                    current_spans.push(Span::styled(text.to_string(), style));
                }
            }
            Event::Code(code) => {
                if in_table_cell {
                    table_cell.push_str(&code);
                } else {
                    // Inline code — distinct background color
                    let style = Style::default().bg(Color::DarkGray).fg(Color::White);
                    current_spans.push(Span::styled(code.to_string(), style));
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                flush_line(&mut lines, &mut current_spans);
            }
            _ => {}
        }
    }

    // Flush any remaining spans
    flush_line(&mut lines, &mut current_spans);

    // If we produced no lines, return the raw text as plain
    if lines.is_empty() {
        return raw.lines().map(|l| Line::from(l.to_owned())).collect();
    }

    lines
}

/// Render accumulated GFM table rows into an aligned grid. First row is the
/// header (bold + accent); a separator follows it. Columns are padded to the
/// widest cell (capped) and joined with vertical bars.
fn render_table(rows: &[Vec<String>], avail_width: usize) -> Vec<Line<'static>> {
    if rows.is_empty() {
        return Vec::new();
    }
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    if cols == 0 {
        return Vec::new();
    }
    // Natural width = widest cell per column.
    let mut widths = vec![0usize; cols];
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }
    // Shrink to fit the panel: reserve " │ " (3) between columns. If the natural
    // grid overflows, cap each column to an even share so it never spills past
    // the right border.
    let sep_total = 3 * cols.saturating_sub(1);
    let budget = avail_width.saturating_sub(sep_total).max(cols);
    if widths.iter().sum::<usize>() > budget {
        let per_col = (budget / cols).max(6);
        for w in &mut widths {
            *w = (*w).min(per_col);
        }
    }

    let mut out: Vec<Line<'static>> = Vec::new();
    for (ri, row) in rows.iter().enumerate() {
        let mut spans: Vec<Span<'static>> = Vec::new();
        for (i, w) in widths.iter().enumerate() {
            let raw = row.get(i).map_or("", String::as_str);
            let cell: String = raw.chars().take(*w).collect();
            let pad = w.saturating_sub(cell.chars().count());
            let style = if ri == 0 {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            spans.push(Span::styled(format!("{cell}{}", " ".repeat(pad)), style));
            if i + 1 < widths.len() {
                spans.push(Span::styled(" │ ".to_string(), Style::default().fg(Color::DarkGray)));
            }
        }
        out.push(Line::from(spans));
        if ri == 0 {
            let sep = widths
                .iter()
                .map(|w| "─".repeat(*w))
                .collect::<Vec<_>>()
                .join("─┼─");
            out.push(Line::from(Span::styled(sep, Style::default().fg(Color::DarkGray))));
        }
    }
    out
}

/// Wrap already-styled `Line`s to `width` columns, preserving each span's
/// style across the break. Char-based (keeps code indentation intact; safe for
/// UTF-8 / accented text). A `List` does not wrap on its own, so markdown
/// output must be pre-wrapped or it overflows the right border.
pub fn wrap_lines(lines: Vec<Line<'static>>, width: usize) -> Vec<Line<'static>> {
    if width == 0 {
        return lines;
    }
    let mut out: Vec<Line<'static>> = Vec::new();
    for line in lines {
        let total: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
        if total <= width {
            out.push(line);
            continue;
        }
        let mut cur: Vec<Span<'static>> = Vec::new();
        let mut col = 0usize;
        for span in line.spans {
            let style = span.style;
            let chars: Vec<char> = span.content.chars().collect();
            let mut i = 0;
            while i < chars.len() {
                let take = (width - col).min(chars.len() - i);
                let chunk: String = chars[i..i + take].iter().collect();
                cur.push(Span::styled(chunk, style));
                col += take;
                i += take;
                if col >= width {
                    out.push(Line::from(std::mem::take(&mut cur)));
                    col = 0;
                }
            }
        }
        if !cur.is_empty() {
            out.push(Line::from(cur));
        }
    }
    out
}

/// Flush accumulated spans into a completed line.
fn flush_line(lines: &mut Vec<Line<'static>>, spans: &mut Vec<Span<'static>>) {
    if !spans.is_empty() {
        lines.push(Line::from(std::mem::take(spans)));
    }
}

/// Compute the inline style based on bold/italic state.
fn compute_inline_style(bold: bool, italic: bool) -> Style {
    let mut style = Style::default();
    if bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    if italic {
        style = style.add_modifier(Modifier::ITALIC);
    }
    style
}

/// Get the indentation prefix for a heading level.
const fn heading_indent(level: Option<HeadingLevel>) -> &'static str {
    match level {
        Some(HeadingLevel::H1) => "",
        Some(HeadingLevel::H2) => " ",
        // h3+ treated same as h3
        _ => "  ",
    }
}

/// Apply heading style (bold + indentation) to spans.
fn apply_heading_style(spans: &[Span<'static>], indent: &str) -> Vec<Span<'static>> {
    let mut result = Vec::with_capacity(spans.len() + 1);

    // Add indentation prefix if non-empty
    if !indent.is_empty() {
        result.push(Span::styled(indent.to_owned(), Style::default()));
    }

    // Apply bold modifier to all spans
    for span in spans {
        let mut style = span.style;
        style = style.add_modifier(Modifier::BOLD);
        result.push(Span::styled(span.content.to_string(), style));
    }

    result
}

/// Highlight a fenced code block using syntect.
///
/// If the language is not recognized or highlighting fails, falls back to
/// plain monospace rendering with a DarkGray background.
fn highlight_code_block(lang: &str, code: &str) -> Vec<Line<'static>> {
    // Git-diff blocks get line-level background tints (+ green, - red) instead
    // of syntect token highlighting — reads like `git diff`.
    let l = lang.to_ascii_lowercase();
    if l == "diff" || l == "patch" {
        return render_diff_block(code);
    }

    let ss = get_syntax_set();
    let theme = get_theme();

    // Try to find syntax for the language
    let syntax = if lang.is_empty() {
        None
    } else {
        ss.find_syntax_by_token(lang)
            .or_else(|| ss.find_syntax_by_extension(lang))
    };

    syntax.map_or_else(
        || render_plain_code_block(code),
        |syn| highlight_with_syntect(ss, theme, syn, code),
    )
}

/// Highlight code using syntect and convert to Ratatui spans.
fn highlight_with_syntect(
    ss: &SyntaxSet,
    theme: &Theme,
    syntax: &syntect::parsing::SyntaxReference,
    code: &str,
) -> Vec<Line<'static>> {
    use syntect::highlighting::{HighlightIterator, HighlightState, Highlighter};
    use syntect::parsing::{ParseState, ScopeStack};
    use syntect::util::LinesWithEndings;

    let mut parse_state = ParseState::new(syntax);
    let highlighter = Highlighter::new(theme);
    let mut highlight_state = HighlightState::new(&highlighter, ScopeStack::new());
    let mut lines: Vec<Line<'static>> = Vec::new();

    for line in LinesWithEndings::from(code) {
        let Ok(ops) = parse_state.parse_line(line, ss) else {
            // On parse error, fall back to plain rendering for remaining lines
            return render_plain_code_block(code);
        };

        let mut spans: Vec<Span<'static>> = Vec::new();

        // Use the highlight ranges from syntect
        let regions = HighlightIterator::new(&mut highlight_state, &ops, line, &highlighter);

        for (style, text) in regions {
            let fg = syntect_color_to_ratatui(style.foreground);
            let ratatui_style = Style::default().fg(fg).bg(Color::DarkGray);
            spans.push(Span::styled(
                text.trim_end_matches('\n').to_owned(),
                ratatui_style,
            ));
        }

        if spans.is_empty() {
            // Empty line in code block
            spans.push(Span::styled(
                String::new(),
                Style::default().bg(Color::DarkGray),
            ));
        }

        lines.push(Line::from(spans));
    }

    // Handle trailing empty content
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            String::new(),
            Style::default().bg(Color::DarkGray),
        )));
    }

    lines
}

/// Convert a syntect color to the nearest Ratatui color.
const fn syntect_color_to_ratatui(color: syntect::highlighting::Color) -> Color {
    Color::Rgb(color.r, color.g, color.b)
}

/// Render a unified-diff / patch block with git-style line tinting:
/// additions get a green background, deletions red, hunk headers cyan, and
/// file headers (`+++`/`---`) bold.
pub fn render_diff_block(code: &str) -> Vec<Line<'static>> {
    // Subtle backgrounds so text stays readable in any terminal theme.
    let add_bg = Color::Rgb(20, 60, 25);
    let del_bg = Color::Rgb(70, 25, 25);

    let mut lines: Vec<Line<'static>> = Vec::new();
    for raw in code.lines() {
        let style = if raw.starts_with("+++") || raw.starts_with("---") {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else if raw.starts_with("@@") {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else if raw.starts_with('+') {
            Style::default().fg(Color::Rgb(180, 255, 180)).bg(add_bg)
        } else if raw.starts_with('-') {
            Style::default().fg(Color::Rgb(255, 180, 180)).bg(del_bg)
        } else {
            Style::default().fg(Color::Gray)
        };
        lines.push(Line::from(Span::styled(raw.to_owned(), style)));
    }
    if lines.is_empty() {
        lines.push(Line::from(""));
    }
    lines
}

/// Render a code block as plain monospace text with DarkGray background.
/// Used when language is not recognized or highlighting fails.
fn render_plain_code_block(code: &str) -> Vec<Line<'static>> {
    let style = Style::default().bg(Color::DarkGray).fg(Color::White);

    if code.is_empty() {
        return vec![Line::from(Span::styled(String::new(), style))];
    }

    code.lines()
        .map(|line| Line::from(Span::styled(line.to_owned(), style)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_returns_single_empty_line() {
        let result = render_markdown("", 80);
        assert_eq!(result.len(), 1);
        let content: String = result[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(content, "");
    }

    #[test]
    fn plain_text_renders_as_is() {
        let result = render_markdown("Hello, world!", 80);
        assert!(!result.is_empty());
        let content: String = result
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.as_ref())
            .collect();
        assert!(content.contains("Hello, world!"));
    }

    #[test]
    fn bold_text_has_bold_modifier() {
        let result = render_markdown("**bold text**", 80);
        assert!(!result.is_empty());

        let bold_span = result
            .iter()
            .flat_map(|l| l.spans.iter())
            .find(|s| s.content.contains("bold text"));
        assert!(bold_span.is_some());
        let span = bold_span.unwrap_or_else(|| unreachable!());
        assert!(span.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn italic_text_has_italic_modifier() {
        let result = render_markdown("*italic text*", 80);
        assert!(!result.is_empty());

        let italic_span = result
            .iter()
            .flat_map(|l| l.spans.iter())
            .find(|s| s.content.contains("italic text"));
        assert!(italic_span.is_some());
        let span = italic_span.unwrap_or_else(|| unreachable!());
        assert!(span.style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn inline_code_has_dark_gray_background() {
        let result = render_markdown("Use `foo()` here", 80);
        assert!(!result.is_empty());

        let code_span = result
            .iter()
            .flat_map(|l| l.spans.iter())
            .find(|s| s.content.contains("foo()"));
        assert!(code_span.is_some());
        let span = code_span.unwrap_or_else(|| unreachable!());
        assert_eq!(span.style.bg, Some(Color::DarkGray));
    }

    #[test]
    fn h1_heading_is_bold_no_indent() {
        let result = render_markdown("# Title", 80);
        assert!(!result.is_empty());

        // Find the line containing "Title"
        let heading_line = result
            .iter()
            .find(|l| l.spans.iter().any(|s| s.content.contains("Title")));
        assert!(heading_line.is_some());

        let line = heading_line.unwrap_or_else(|| unreachable!());
        // Should have bold modifier
        let title_span = line.spans.iter().find(|s| s.content.contains("Title"));
        assert!(title_span.is_some());
        assert!(
            title_span
                .unwrap_or_else(|| unreachable!())
                .style
                .add_modifier
                .contains(Modifier::BOLD)
        );

        // H1 should have no indentation prefix
        let full_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(!full_text.starts_with(' '));
    }

    #[test]
    fn h2_heading_has_one_space_indent() {
        let result = render_markdown("## Subtitle", 80);
        assert!(!result.is_empty());

        let heading_line = result
            .iter()
            .find(|l| l.spans.iter().any(|s| s.content.contains("Subtitle")));
        assert!(heading_line.is_some());

        let line = heading_line.unwrap_or_else(|| unreachable!());
        let full_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(full_text.starts_with(' '));
        assert!(!full_text.starts_with("  ")); // only one space
    }

    #[test]
    fn h3_heading_has_two_space_indent() {
        let result = render_markdown("### Section", 80);
        assert!(!result.is_empty());

        let heading_line = result
            .iter()
            .find(|l| l.spans.iter().any(|s| s.content.contains("Section")));
        assert!(heading_line.is_some());

        let line = heading_line.unwrap_or_else(|| unreachable!());
        let full_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(full_text.starts_with("  "));
    }

    #[test]
    fn bullet_list_has_bullet_marker() {
        let result = render_markdown("- item one\n- item two", 80);
        assert!(!result.is_empty());

        let all_text: String = result
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.as_ref())
            .collect();
        assert!(all_text.contains('•'));
        assert!(all_text.contains("item one"));
        assert!(all_text.contains("item two"));
    }

    #[test]
    fn numbered_list_has_number_markers() {
        let result = render_markdown("1. first\n2. second\n3. third", 80);
        assert!(!result.is_empty());

        let all_text: String = result
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.as_ref())
            .collect();
        assert!(all_text.contains("1. "));
        assert!(all_text.contains("first"));
        assert!(all_text.contains("2. "));
        assert!(all_text.contains("second"));
    }

    #[test]
    fn fenced_code_block_has_dark_gray_background() {
        let md = "```rust\nfn main() {}\n```";
        let result = render_markdown(md, 80);
        assert!(!result.is_empty());

        // All code block lines should have DarkGray background
        let code_lines: Vec<&Line> = result
            .iter()
            .filter(|l| l.spans.iter().any(|s| s.style.bg == Some(Color::DarkGray)))
            .collect();
        assert!(!code_lines.is_empty());
    }

    #[test]
    fn unknown_language_code_block_renders_plain() {
        let md = "```unknownlang123\nsome code here\n```";
        let result = render_markdown(md, 80);
        assert!(!result.is_empty());

        // Should still have DarkGray background (plain code block fallback)
        let has_code_bg = result
            .iter()
            .any(|l| l.spans.iter().any(|s| s.style.bg == Some(Color::DarkGray)));
        assert!(has_code_bg);

        // Should contain the code text
        let all_text: String = result
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.as_ref())
            .collect();
        assert!(all_text.contains("some code here"));
    }

    #[test]
    fn code_block_without_language_renders_plain() {
        let md = "```\nplain code\n```";
        let result = render_markdown(md, 80);
        assert!(!result.is_empty());

        let has_code_bg = result
            .iter()
            .any(|l| l.spans.iter().any(|s| s.style.bg == Some(Color::DarkGray)));
        assert!(has_code_bg);
    }

    #[test]
    fn multiline_paragraph_renders_correctly() {
        let result = render_markdown("Line one\nLine two", 80);
        let all_text: String = result
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.as_ref())
            .collect();
        // Soft break within a paragraph — text should be present
        assert!(all_text.contains("Line one"));
        assert!(all_text.contains("Line two"));
    }

    #[test]
    fn mixed_bold_and_italic() {
        let result = render_markdown("***bold italic***", 80);
        assert!(!result.is_empty());

        // Find span with both bold and italic
        let bi_span = result
            .iter()
            .flat_map(|l| l.spans.iter())
            .find(|s| s.content.contains("bold italic"));
        assert!(bi_span.is_some());
        let span = bi_span.unwrap_or_else(|| unreachable!());
        assert!(span.style.add_modifier.contains(Modifier::BOLD));
        assert!(span.style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn diff_block_tints_add_and_del_lines() {
        let lines = render_diff_block("+adicionado\n-removido\n@@ -1 +1 @@\ncontexto");
        assert_eq!(lines.len(), 4);
        // '+' line → green-ish background.
        assert_eq!(lines[0].spans[0].style.bg, Some(Color::Rgb(20, 60, 25)));
        // '-' line → red-ish background.
        assert_eq!(lines[1].spans[0].style.bg, Some(Color::Rgb(70, 25, 25)));
        // hunk header → cyan fg, no bg.
        assert_eq!(lines[2].spans[0].style.fg, Some(Color::Cyan));
        // context → no background tint.
        assert_eq!(lines[3].spans[0].style.bg, None);
    }

    #[test]
    fn diff_lang_routes_to_diff_renderer() {
        let md = "```diff\n+new line\n-old line\n```";
        let out = render_markdown(md, 80);
        assert!(
            out.iter()
                .any(|l| l.spans.iter().any(|s| s.style.bg == Some(Color::Rgb(20, 60, 25)))),
            "diff fence should produce green-tinted add lines"
        );
    }

    #[test]
    fn long_line_wraps_to_width() {
        let lines = wrap_lines(vec![Line::from("a".repeat(100))], 20);
        assert!(lines.len() >= 5, "100 chars / width 20 → ≥5 rows");
        for l in &lines {
            let w: usize = l.spans.iter().map(|s| s.content.chars().count()).sum();
            assert!(w <= 20, "no wrapped row exceeds width");
        }
    }
}
