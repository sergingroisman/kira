#![forbid(unsafe_code)]

//! Streaming indicator — animated spinner displayed while an agent is actively
//! streaming a response.
//!
//! The spinner frame index is stored in `AppState.spinner_frame: u8` and
//! advanced by the event loop on each tick (typically 100ms). This module
//! provides only the rendering logic.

use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::ui::theme;

/// Get the current spinner frame based on the frame index.
///
/// Delegates to the shared [`theme::SPINNER_FRAMES`]; wraps via modulo so any
/// `u8` value is safe.
pub fn spinner_char(frame_idx: u8) -> &'static str {
    theme::spinner_char(frame_idx)
}

/// Render the streaming indicator as a `Line` with the spinner character
/// styled in the accent color to stand out from surrounding content.
///
/// This `Line` is intended to be appended on the same line as the last
/// streamed token in the chat panel.
pub fn render_streaming_indicator(frame_idx: u8) -> Line<'static> {
    let ch = spinner_char(frame_idx);
    Line::from(Span::styled(ch, Style::default().fg(theme::ACCENT)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spinner_char_returns_valid_frame_for_zero() {
        assert_eq!(spinner_char(0), "⠋");
    }

    #[test]
    fn spinner_char_wraps_around() {
        // Frame 10 should wrap to index 0
        assert_eq!(spinner_char(10), "⠋");
        // Frame 11 should wrap to index 1
        assert_eq!(spinner_char(11), "⠙");
    }

    #[test]
    fn spinner_char_handles_max_u8() {
        // Should not panic — wraps via modulo
        let ch = spinner_char(u8::MAX);
        assert!(theme::SPINNER_FRAMES.contains(&ch));
    }

    #[test]
    fn all_frames_are_distinct() {
        let mut seen = std::collections::HashSet::new();
        for i in 0..10u8 {
            assert!(seen.insert(spinner_char(i)));
        }
    }

    #[test]
    fn render_streaming_indicator_returns_cyan_styled_line() {
        let line = render_streaming_indicator(3);
        assert_eq!(line.spans.len(), 1);
        let span = &line.spans[0];
        assert_eq!(span.content, "⠸");
        assert_eq!(span.style.fg, Some(theme::ACCENT));
    }

    #[test]
    fn render_streaming_indicator_cycles_through_frames() {
        // Verify that different frame indices produce different characters
        let line_a = render_streaming_indicator(0);
        let line_b = render_streaming_indicator(5);
        assert_ne!(line_a.spans[0].content, line_b.spans[0].content);
    }
}
