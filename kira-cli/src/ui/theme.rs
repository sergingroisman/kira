#![forbid(unsafe_code)]

//! Central theme — the single source of truth for colors, styles, and shared
//! widget primitives across the TUI.
//!
//! Before this module, every widget hardcoded its own `Color::Green`,
//! `Color::DarkGray`, etc., and the braille spinner frames were duplicated in
//! three places. Routing everything through `theme` keeps the look cohesive and
//! retunable from one spot.
//!
//! Tokens are *semantic* (`ACCENT`, `SUCCESS`, `DANGER`, …) rather than raw
//! color names, so a palette change here propagates everywhere.

use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders},
};

// ── Semantic color tokens ───────────────────────────────────────────────────

/// Primary brand / interactive accent (badges, focus, links).
pub const ACCENT: Color = Color::Cyan;
/// Secondary accent (tier, alt highlights).
pub const ACCENT_ALT: Color = Color::Magenta;
/// Positive / completed / agent output.
pub const SUCCESS: Color = Color::Green;
/// Caution / pending / system notices.
pub const WARNING: Color = Color::Yellow;
/// Errors / destructive / failures.
pub const DANGER: Color = Color::Red;
/// Informational notices.
pub const INFO: Color = Color::Blue;

/// Primary foreground text.
pub const TEXT: Color = Color::White;
/// De-emphasized text (timestamps, hints, context lines).
pub const TEXT_DIM: Color = Color::Gray;
/// Muted / disabled / idle elements.
pub const MUTED: Color = Color::DarkGray;

/// Bar / panel background (header, footer).
pub const SURFACE: Color = Color::DarkGray;
/// Contrasting foreground used on filled pills/badges.
pub const ON_FILL: Color = Color::Black;

// ── Domain colors ───────────────────────────────────────────────────────────

/// Color for the conceptual pipeline layers (kept distinct for accessibility).
pub mod layer {
    use super::{ACCENT, ACCENT_ALT, SUCCESS, WARNING};
    use ratatui::style::Color;

    pub const PERCEPTION: Color = ACCENT; // Cyan
    pub const REASONING: Color = ACCENT_ALT; // Magenta
    pub const SECURITY: Color = WARNING; // Yellow
    pub const EXECUTION: Color = SUCCESS; // Green
}

// ── Shared spinner ──────────────────────────────────────────────────────────

/// Braille spinner frames. Single source of truth — previously duplicated in
/// `chat/mod.rs`, `chat/streaming.rs`, and `layers/indicators.rs`.
///
/// With 10 frames at the ~100ms render tick, a full cycle takes ~1s.
pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Current spinner glyph for a frame index (wraps via modulo, so any `u8` is safe).
#[must_use]
pub fn spinner_char(frame_idx: u8) -> &'static str {
    SPINNER_FRAMES[(frame_idx as usize) % SPINNER_FRAMES.len()]
}

// ── Style helpers ─────────────────────────────────────────────────────────────

/// A filled "pill" badge: ` LABEL ` with a colored background and bold fg.
///
/// Used for mode/tier/status chips in the header instead of flat `[BRACKET]`
/// text, giving the UI a modern, professional look.
#[must_use]
pub fn pill(label: &str, fg: Color, bg: Color) -> Span<'static> {
    Span::styled(
        format!(" {label} "),
        Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
    )
}

/// A rounded, themed bordered block with an optional title.
///
/// Replaces ad-hoc `Block::default().borders(Borders::ALL)` calls so every
/// panel shares the same rounded-corner look and border color.
#[must_use]
pub fn block(title: impl Into<String>) -> Block<'static> {
    let title: String = title.into();
    let mut b = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(MUTED));
    if !title.is_empty() {
        b = b.title(Span::styled(
            title,
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        ));
    }
    b
}

/// A rounded, themed bordered block with no title.
///
/// Callers chain their own `.title()` / `.border_style()`. Gives every panel
/// the same rounded-corner look without overriding per-panel accent colors.
#[must_use]
pub fn bordered() -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(MUTED))
}

/// Style for de-emphasized text.
#[must_use]
pub fn dim() -> Style {
    Style::default().fg(TEXT_DIM).add_modifier(Modifier::DIM)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spinner_char_wraps() {
        assert_eq!(spinner_char(0), SPINNER_FRAMES[0]);
        assert_eq!(spinner_char(10), SPINNER_FRAMES[0]);
        assert_eq!(spinner_char(u8::MAX), SPINNER_FRAMES[(u8::MAX as usize) % 10]);
    }

    #[test]
    fn pill_pads_and_fills() {
        let span = pill("PLAN", ON_FILL, ACCENT);
        assert_eq!(span.content, " PLAN ");
        assert_eq!(span.style.bg, Some(ACCENT));
        assert_eq!(span.style.fg, Some(ON_FILL));
        assert!(span.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn block_is_rounded() {
        let _b = block("kira");
        // Construction must not panic; BorderType::Rounded set above.
    }
}
