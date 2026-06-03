#![forbid(unsafe_code)]

//! Input area module — wraps `tui-textarea` with auto-expanding height,
//! history navigation, and completion popup support.

pub mod completion;
pub mod history;

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
};
use tui_textarea::TextArea;

use crate::app::AppState;

/// Pure function: compute the input area height based on the number of content
/// lines, clamped to `[min, max]`.
///
/// - `line_count`: the number of lines in the text area (newline_count + 1).
/// - `min`: minimum height (typically 3).
/// - `max`: maximum height (typically 10).
///
/// Returns `clamp(line_count as u16, min, max)`.
pub fn compute_input_height(line_count: usize, min: u16, max: u16) -> u16 {
    // Saturating cast: if line_count > u16::MAX, saturate to u16::MAX
    // which will then be clamped to max anyway.
    #[allow(clippy::cast_possible_truncation)]
    let count = if line_count > u16::MAX as usize {
        u16::MAX
    } else {
        line_count as u16
    };
    count.clamp(min, max)
}

/// Render the input area into the given `area`.
///
/// Creates a `tui-textarea::TextArea` widget configured with the current input
/// text from `AppState`, applies styling, and renders it. The TextArea provides:
/// - Multi-line editing with cursor movement (arrows, Home, End, Ctrl+A, Ctrl+E)
/// - Vertical scrolling when content exceeds the visible area
///
/// Note: Actual key event handling (Shift+Enter for newline, Enter for submit)
/// is wired in the event loop (task 13.2). This function only handles rendering.
pub fn render_input(frame: &mut Frame, state: &AppState, area: Rect) {
    // Remember the widget rect so the mouse handler can map clicks to cursor.
    state.input_area.set(area);

    // Build lines from the current input text.
    // Split on newlines to get individual lines for the TextArea.
    let lines: Vec<String> = if state.input.is_empty() {
        vec![String::new()]
    } else {
        state.input.split('\n').map(String::from).collect()
    };

    let mut textarea = TextArea::new(lines);

    // Style the text area
    textarea.set_block(
        crate::ui::theme::bordered()
            .title(" input ")
            .title_style(
                Style::default()
                    .fg(crate::ui::theme::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
    );

    // Cursor styling
    textarea.set_cursor_line_style(Style::default());
    textarea.set_cursor_style(Style::default().fg(Color::Black).bg(Color::White));

    // If input is locked, show a placeholder style
    if state.input_locked {
        textarea.set_placeholder_text("Sending...");
        textarea.set_placeholder_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::ITALIC),
        );
    }

    // Position the cursor at `input_cursor` (a char index into the flat input).
    // Walk the lines counting chars (+1 per newline) to map it to (row, col).
    let (row, col) = char_index_to_row_col(&state.input, state.input_cursor);
    textarea.move_cursor(tui_textarea::CursorMove::Jump(row, col));

    frame.render_widget(&textarea, area);
}

/// Map a flat char index into `text` to a zero-based `(row, col)` for the
/// TextArea (rows split on `\n`; the newline counts as one position).
#[allow(clippy::cast_possible_truncation)]
pub fn char_index_to_row_col(text: &str, char_index: usize) -> (u16, u16) {
    let mut remaining = char_index;
    let mut row = 0u16;
    for line in text.split('\n') {
        let line_chars = line.chars().count();
        if remaining <= line_chars {
            return (row, remaining as u16);
        }
        remaining -= line_chars + 1; // consume the line + its '\n'
        row += 1;
    }
    (row, 0)
}

/// Inverse of [`char_index_to_row_col`]: map a `(row, col)` click position back
/// to a flat char index, clamped to the actual content.
pub fn row_col_to_char_index(text: &str, row: usize, col: usize) -> usize {
    let mut idx = 0usize;
    for (r, line) in text.split('\n').enumerate() {
        let line_chars = line.chars().count();
        if r == row {
            return idx + col.min(line_chars);
        }
        idx += line_chars + 1;
    }
    text.chars().count()
}

/// Extract text from the input, validate it contains non-whitespace, clear the
/// input buffer, and return `Some(text)` if valid or `None` if whitespace-only.
///
/// When submission succeeds:
/// - `state.input` is cleared to empty string.
/// - `state.input_cursor` is reset to 0.
/// - `state.input_height` is reset to the configured minimum (default 3).
///
/// When submission is rejected (whitespace-only or empty):
/// - `state.input` remains unchanged.
/// - Returns `None`.
pub fn submit_input(state: &mut AppState) -> Option<String> {
    let text = state.input.trim().to_string();

    if text.is_empty() {
        return None;
    }

    // Capture the full input (preserving original whitespace/newlines)
    let submitted = std::mem::take(&mut state.input);
    state.input_cursor = 0;
    state.input_height = state.config.ui.input_min_height;

    Some(submitted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AppState, Mode, Tier};
    use crate::config::Config;
    use proptest::prelude::*;

    fn test_state() -> AppState {
        AppState::new(Config::default(), Mode::Plan, Tier::Standard)
    }

    // ── compute_input_height unit tests ────────────────────────────────────

    #[test]
    fn height_at_minimum_when_few_lines() {
        // 1 line → clamped to min=3
        assert_eq!(compute_input_height(1, 3, 10), 3);
        assert_eq!(compute_input_height(2, 3, 10), 3);
        assert_eq!(compute_input_height(3, 3, 10), 3);
    }

    #[test]
    fn height_expands_with_content() {
        assert_eq!(compute_input_height(4, 3, 10), 4);
        assert_eq!(compute_input_height(7, 3, 10), 7);
        assert_eq!(compute_input_height(10, 3, 10), 10);
    }

    #[test]
    fn height_capped_at_maximum() {
        assert_eq!(compute_input_height(11, 3, 10), 10);
        assert_eq!(compute_input_height(100, 3, 10), 10);
        assert_eq!(compute_input_height(1000, 3, 10), 10);
    }

    #[test]
    fn height_zero_lines_clamps_to_min() {
        // Edge case: 0 lines (shouldn't happen in practice, but handle gracefully)
        assert_eq!(compute_input_height(0, 3, 10), 3);
    }

    #[test]
    fn height_large_line_count_saturates() {
        // Very large line count should still clamp to max
        assert_eq!(compute_input_height(usize::MAX, 3, 10), 10);
    }

    // ── submit_input unit tests ────────────────────────────────────────────

    #[test]
    fn submit_non_empty_returns_text_and_clears() {
        let mut state = test_state();
        // char/row-col round-trip mapping.
        assert_eq!(super::char_index_to_row_col("hello", 3), (0, 3));
        assert_eq!(super::char_index_to_row_col("ab\ncd", 4), (1, 1));
        assert_eq!(super::row_col_to_char_index("ab\ncd", 1, 1), 4);
        assert_eq!(super::row_col_to_char_index("hello", 0, 99), 5); // col clamps

        state.input = "hello world".to_string();
        state.input_cursor = 5;
        state.input_height = 4;

        let result = submit_input(&mut state);

        assert_eq!(result, Some("hello world".to_string()));
        assert!(state.input.is_empty());
        assert_eq!(state.input_cursor, 0);
        assert_eq!(state.input_height, 3); // reset to min
    }

    #[test]
    fn submit_multiline_returns_full_text() {
        let mut state = test_state();
        state.input = "line1\nline2\nline3".to_string();

        let result = submit_input(&mut state);

        assert_eq!(result, Some("line1\nline2\nline3".to_string()));
        assert!(state.input.is_empty());
    }

    #[test]
    fn submit_whitespace_only_returns_none() {
        let mut state = test_state();
        state.input = "   \n  \t  \n  ".to_string();
        state.input_cursor = 5;

        let result = submit_input(&mut state);

        assert_eq!(result, None);
        // Input should remain unchanged
        assert_eq!(state.input, "   \n  \t  \n  ");
        assert_eq!(state.input_cursor, 5);
    }

    #[test]
    fn submit_empty_returns_none() {
        let mut state = test_state();
        state.input = String::new();

        let result = submit_input(&mut state);

        assert_eq!(result, None);
        assert!(state.input.is_empty());
    }

    #[test]
    fn submit_with_leading_trailing_whitespace_preserves_original() {
        let mut state = test_state();
        state.input = "  hello  ".to_string();

        let result = submit_input(&mut state);

        // Returns the original text (not trimmed), since trim is only for validation
        assert_eq!(result, Some("  hello  ".to_string()));
    }

    // ── Property-based tests ───────────────────────────────────────────────

    /// Generate a string with 0–20 newlines for input height testing.
    fn arb_multiline_input() -> impl Strategy<Value = String> {
        // Generate 0 to 20 newlines embedded in arbitrary text
        (0usize..=20).prop_flat_map(|newline_count| {
            // Generate segments between newlines
            prop::collection::vec("[a-zA-Z0-9 ]{0,20}", newline_count + 1)
                .prop_map(move |segments| segments.join("\n"))
        })
    }

    /// Generate a string that contains at least one non-whitespace character.
    fn arb_non_whitespace_input() -> impl Strategy<Value = String> {
        // Ensure at least one non-whitespace char exists
        ("[\\s]{0,10}[a-zA-Z0-9][\\s\\S]{0,50}").prop_filter("must contain non-whitespace", |s| {
            s.chars().any(|c| !c.is_whitespace())
        })
    }

    /// Generate a string composed entirely of whitespace characters (spaces, tabs, newlines).
    fn arb_whitespace_only_input() -> impl Strategy<Value = String> {
        prop::collection::vec(prop_oneof![Just(' '), Just('\t'), Just('\n')], 0..=50)
            .prop_map(|chars| chars.into_iter().collect::<String>())
    }

    // Feature: tui-layout-redesign, Property 5: Input height clamps to [3, 10]
    // Validates: Requirements 5.1, 5.2, 5.3, 5.4
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_input_height_clamps_to_3_10(
            input in arb_multiline_input()
        ) {
            let line_count = input.chars().filter(|&c| c == '\n').count() + 1;
            let height = compute_input_height(line_count, 3, 10);

            // Height must be within [3, 10]
            prop_assert!(height >= 3, "height {} < min 3", height);
            prop_assert!(height <= 10, "height {} > max 10", height);

            // Height must equal clamp(line_count, 3, 10)
            let expected = (line_count as u16).clamp(3, 10);
            prop_assert_eq!(height, expected,
                "for {} lines, expected height {} but got {}",
                line_count, expected, height);
        }
    }

    // Feature: tui-layout-redesign, Property 6: Enter on non-empty input submits and clears
    // Validates: Requirements 5.7
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_submit_non_empty_input_returns_some_and_clears(
            input in arb_non_whitespace_input()
        ) {
            let mut state = test_state();
            state.input = input.clone();
            state.input_height = 5; // arbitrary non-default height

            let result = submit_input(&mut state);

            // Must produce Some containing the original input
            prop_assert!(result.is_some(),
                "submit_input should return Some for non-whitespace input: {:?}", input);
            prop_assert_eq!(result.unwrap(), input);

            // Input buffer must be cleared
            prop_assert!(state.input.is_empty(),
                "input buffer should be empty after submit");

            // Input height must be reset to min (3)
            prop_assert_eq!(state.input_height, 3,
                "input height should reset to 3 after submit, got {}", state.input_height);
        }
    }

    // Feature: tui-layout-redesign, Property 7: Whitespace-only input is not submitted
    // Validates: Requirements 5.8
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_whitespace_only_input_not_submitted(
            input in arb_whitespace_only_input()
        ) {
            let mut state = test_state();
            state.input = input.clone();
            let original_cursor = state.input_cursor;

            let result = submit_input(&mut state);

            // Must return None (no submission)
            prop_assert!(result.is_none(),
                "submit_input should return None for whitespace-only input: {:?}", input);

            // Input buffer must remain unchanged
            prop_assert_eq!(&state.input, &input,
                "input buffer should remain unchanged for whitespace-only input");

            // Cursor should remain unchanged
            prop_assert_eq!(state.input_cursor, original_cursor,
                "cursor should remain unchanged for whitespace-only input");
        }
    }
}
