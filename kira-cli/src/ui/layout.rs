#![forbid(unsafe_code)]

//! Layout engine — computes adaptive panel zones based on terminal dimensions
//! and sidebar visibility state.
//!
//! The layout divides the terminal into vertical zones:
//! - Header (1 row at top)
//! - Main area (sidebar + chat, fills remaining space)
//! - Input area (3–10 rows above footer)
//! - Footer (1 row at bottom)
//!
//! If the terminal is too small (width < 60 or height < 8), all zones are
//! zero-sized and `terminal_too_small` is set to `true`.

use ratatui::layout::Rect;

/// The computed layout zones for a single frame.
///
/// Each field is a `Rect` describing the position and size of that zone.
/// When `terminal_too_small` is `true`, all rects are zero-sized and the
/// caller should render a "terminal too small" message instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutZones {
    /// Top bar — 1 row showing connection, mode, tier, model, cost, approvals.
    pub header: Rect,
    /// Optional sidebar — 42 columns on the left of the main area.
    pub sidebar: Option<Rect>,
    /// Chat panel — occupies the main area (or main area minus sidebar).
    pub chat: Rect,
    /// Input area — 3–10 rows above the footer.
    pub input: Rect,
    /// Bottom bar — 1 row showing workspace, git, LSP, MCP, keybindings.
    pub footer: Rect,
    /// When `true`, the terminal is too small to render the layout.
    pub terminal_too_small: bool,
}

/// Sidebar width in columns (fixed).
const SIDEBAR_WIDTH: u16 = 42;

/// Minimum terminal width to render the full layout.
const MIN_WIDTH: u16 = 60;

/// Minimum terminal height to render the full layout.
const MIN_HEIGHT: u16 = 8;

/// Minimum input height (clamped lower bound).
const INPUT_MIN: u16 = 3;

/// Maximum input height (clamped upper bound).
const INPUT_MAX: u16 = 10;

/// Compute the layout zones for the given terminal area.
///
/// # Arguments
///
/// * `area` — The full terminal rectangle.
/// * `sidebar_visible` — Whether the sidebar should be shown. The caller
///   resolves auto-threshold vs manual toggle before calling this function.
/// * `input_height` — Desired input area height (will be clamped to 3–10).
///
/// # Returns
///
/// A `LayoutZones` struct with all zone rectangles computed. If the terminal
/// is too small, `terminal_too_small` is `true` and all rects are zero-sized.
pub fn compute_layout(area: Rect, sidebar_visible: bool, input_height: u16) -> LayoutZones {
    // Terminal too small — bail with zero-sized zones.
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        return LayoutZones {
            header: Rect::default(),
            sidebar: None,
            chat: Rect::default(),
            input: Rect::default(),
            footer: Rect::default(),
            terminal_too_small: true,
        };
    }

    // Clamp input height to [3, 10], then further constrain so that
    // header (1) + footer (1) + input + at least 0 main rows ≤ area.height.
    // This guarantees all zone heights always sum to the terminal height.
    let max_input_for_area = area.height.saturating_sub(2); // room after header+footer
    let clamped_input_height = input_height
        .clamp(INPUT_MIN, INPUT_MAX)
        .min(max_input_for_area);

    // Fixed zones: header (1 row) + footer (1 row) + input (clamped).
    let fixed_height = 1u16.saturating_add(1).saturating_add(clamped_input_height);

    // Main area height is whatever remains.
    let main_height = area.height.saturating_sub(fixed_height);

    // Header: 1 row at the top.
    let header = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: 1,
    };

    // Footer: 1 row at the bottom.
    let footer = Rect {
        x: area.x,
        y: area.y.saturating_add(area.height.saturating_sub(1)),
        width: area.width,
        height: 1,
    };

    // Input: clamped_input_height rows above the footer.
    let input = Rect {
        x: area.x,
        y: footer.y.saturating_sub(clamped_input_height),
        width: area.width,
        height: clamped_input_height,
    };

    // Main area starts right below the header.
    let main_y = area.y.saturating_add(1);

    // Compute sidebar and chat within the main area.
    let (sidebar, chat) = if sidebar_visible {
        // Sidebar occupies SIDEBAR_WIDTH columns on the left.
        // If the terminal is narrower than the sidebar, chat gets 0 width.
        let sidebar_w = SIDEBAR_WIDTH.min(area.width);
        let chat_w = area.width.saturating_sub(sidebar_w);

        let sidebar_rect = Rect {
            x: area.x,
            y: main_y,
            width: sidebar_w,
            height: main_height,
        };

        let chat_rect = Rect {
            x: area.x.saturating_add(sidebar_w),
            y: main_y,
            width: chat_w,
            height: main_height,
        };

        (Some(sidebar_rect), chat_rect)
    } else {
        // No sidebar — chat takes the full width.
        let chat_rect = Rect {
            x: area.x,
            y: main_y,
            width: area.width,
            height: main_height,
        };

        (None, chat_rect)
    };

    LayoutZones {
        header,
        sidebar,
        chat,
        input,
        footer,
        terminal_too_small: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // Feature: tui-layout-redesign, Property 1: Layout zone computation is consistent
    // Validates: Requirements 1.1, 1.2, 1.3, 1.5, 1.6, 1.7
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_layout_zone_computation_is_consistent(
            w in 1u16..=500,
            h in 1u16..=200,
            sidebar_visible in proptest::bool::ANY,
            input_height in 0u16..=20,
        ) {
            let area = Rect::new(0, 0, w, h);
            let zones = compute_layout(area, sidebar_visible, input_height);

            if w < 60 || h < 8 {
                // Terminal too small — no zones computed.
                prop_assert!(zones.terminal_too_small);
                prop_assert_eq!(zones.header, Rect::default());
                prop_assert_eq!(zones.chat, Rect::default());
                prop_assert_eq!(zones.input, Rect::default());
                prop_assert_eq!(zones.footer, Rect::default());
                prop_assert_eq!(zones.sidebar, None);
            } else {
                prop_assert!(!zones.terminal_too_small);

                // Header is 1 row.
                prop_assert_eq!(zones.header.height, 1);

                // Footer is 1 row.
                prop_assert_eq!(zones.footer.height, 1);

                // Input height is clamped to [3, 10].
                prop_assert!(zones.input.height >= 3);
                prop_assert!(zones.input.height <= 10);

                // All zone heights sum to total terminal height.
                let total_height = zones.header.height
                    + zones.chat.height
                    + zones.input.height
                    + zones.footer.height;
                prop_assert_eq!(total_height, h);

                // Sidebar and chat width assertions.
                if sidebar_visible {
                    // Sidebar is shown when sidebar_visible is true.
                    let sidebar = zones.sidebar.unwrap();
                    prop_assert_eq!(sidebar.width, 42);
                    prop_assert_eq!(zones.chat.width, w - 42);
                } else {
                    // No sidebar — chat takes full width.
                    prop_assert_eq!(zones.sidebar, None);
                    prop_assert_eq!(zones.chat.width, w);
                }
            }
        }
    }

    #[test]
    fn too_small_width() {
        let area = Rect::new(0, 0, 59, 20);
        let zones = compute_layout(area, false, 3);
        assert!(zones.terminal_too_small);
        assert_eq!(zones.header, Rect::default());
        assert_eq!(zones.sidebar, None);
    }

    #[test]
    fn too_small_height() {
        let area = Rect::new(0, 0, 80, 7);
        let zones = compute_layout(area, false, 3);
        assert!(zones.terminal_too_small);
    }

    #[test]
    fn exactly_minimum_size() {
        let area = Rect::new(0, 0, 60, 8);
        let zones = compute_layout(area, false, 3);
        assert!(!zones.terminal_too_small);
        assert_eq!(zones.header.height, 1);
        assert_eq!(zones.footer.height, 1);
        assert_eq!(zones.input.height, 3);
        // main_height = 8 - 1 - 1 - 3 = 3
        assert_eq!(zones.chat.height, 3);
    }

    #[test]
    fn sidebar_visible_allocates_42_cols() {
        let area = Rect::new(0, 0, 150, 30);
        let zones = compute_layout(area, true, 3);
        assert!(!zones.terminal_too_small);
        let sidebar = zones.sidebar.expect("sidebar should be Some");
        assert_eq!(sidebar.width, 42);
        assert_eq!(zones.chat.width, 150 - 42);
        assert_eq!(zones.chat.x, 42);
    }

    #[test]
    fn sidebar_hidden_chat_full_width() {
        let area = Rect::new(0, 0, 150, 30);
        let zones = compute_layout(area, false, 3);
        assert_eq!(zones.sidebar, None);
        assert_eq!(zones.chat.width, 150);
        assert_eq!(zones.chat.x, 0);
    }

    #[test]
    fn sidebar_visible_narrow_terminal() {
        // sidebar_visible=true even on narrow terminal (manual toggle)
        let area = Rect::new(0, 0, 80, 20);
        let zones = compute_layout(area, true, 3);
        assert!(!zones.terminal_too_small);
        let sidebar = zones.sidebar.expect("sidebar should be Some");
        assert_eq!(sidebar.width, 42);
        assert_eq!(zones.chat.width, 80 - 42);
    }

    #[test]
    fn input_height_clamped_below() {
        let area = Rect::new(0, 0, 100, 20);
        let zones = compute_layout(area, false, 1);
        assert_eq!(zones.input.height, 3);
    }

    #[test]
    fn input_height_clamped_above() {
        let area = Rect::new(0, 0, 100, 30);
        let zones = compute_layout(area, false, 15);
        assert_eq!(zones.input.height, 10);
    }

    #[test]
    fn input_height_within_range() {
        let area = Rect::new(0, 0, 100, 30);
        let zones = compute_layout(area, false, 7);
        assert_eq!(zones.input.height, 7);
    }

    #[test]
    fn zones_sum_to_total_height() {
        let area = Rect::new(0, 0, 120, 40);
        let zones = compute_layout(area, true, 5);
        let total =
            zones.header.height + zones.chat.height + zones.input.height + zones.footer.height;
        assert_eq!(total, 40);
    }

    #[test]
    fn zones_y_positions_are_contiguous() {
        let area = Rect::new(0, 0, 100, 25);
        let zones = compute_layout(area, false, 4);
        // header starts at 0
        assert_eq!(zones.header.y, 0);
        // chat starts right after header
        assert_eq!(zones.chat.y, 1);
        // input starts right after chat
        assert_eq!(zones.input.y, zones.chat.y + zones.chat.height);
        // footer starts right after input
        assert_eq!(zones.footer.y, zones.input.y + zones.input.height);
        // footer ends at the terminal bottom
        assert_eq!(zones.footer.y + zones.footer.height, 25);
    }

    #[test]
    fn sidebar_and_chat_same_height() {
        let area = Rect::new(0, 0, 200, 50);
        let zones = compute_layout(area, true, 6);
        let sidebar = zones.sidebar.expect("sidebar should be Some");
        assert_eq!(sidebar.height, zones.chat.height);
        assert_eq!(sidebar.y, zones.chat.y);
    }

    #[test]
    fn nonzero_origin() {
        // Verify layout works with non-zero origin (e.g., nested rects)
        let area = Rect::new(5, 3, 100, 25);
        let zones = compute_layout(area, false, 3);
        assert!(!zones.terminal_too_small);
        assert_eq!(zones.header.x, 5);
        assert_eq!(zones.header.y, 3);
        assert_eq!(zones.footer.y, 3 + 25 - 1);
    }
}
