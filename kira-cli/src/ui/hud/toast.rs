#![forbid(unsafe_code)]

//! Toast notification renderer — displays severity-colored notifications
//! in the bottom-right corner, stacked vertically (max 5 simultaneous).

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::{AppState, ToastKind};
use crate::ui::theme;

/// Maximum width of a toast notification (characters).
const TOAST_MAX_WIDTH: u16 = 50;

/// Maximum number of simultaneous toasts displayed.
const MAX_VISIBLE_TOASTS: usize = 5;

/// Render all active toasts in the bottom-right corner of the given area,
/// stacked vertically (newest at bottom, oldest at top).
///
/// Each toast is a single-line widget with a severity prefix and color:
/// - Success: green, prefix "✓ "
/// - Warning: yellow, prefix "⚠ "
/// - Error: red, prefix "✗ "
/// - Info: blue, prefix "ℹ "
///
/// Messages longer than the available width are truncated with "…".
pub fn render_toasts(frame: &mut Frame, state: &AppState, area: Rect) {
    let toast_count = state.toasts.len().min(MAX_VISIBLE_TOASTS);
    if toast_count == 0 {
        return;
    }

    // Compute toast width: min of TOAST_MAX_WIDTH and available area width
    let toast_width = TOAST_MAX_WIDTH.min(area.width);
    if toast_width == 0 || area.height == 0 {
        return;
    }

    // Stack toasts from bottom up: newest at bottom
    // We iterate the last `toast_count` toasts (they are in insertion order,
    // oldest first in the VecDeque).
    let start_idx = state.toasts.len().saturating_sub(MAX_VISIBLE_TOASTS);
    let visible_toasts: Vec<_> = state.toasts.iter().skip(start_idx).collect();

    let num_toasts = visible_toasts.len().min(area.height as usize);

    for (i, toast) in visible_toasts.iter().take(num_toasts).enumerate() {
        let (prefix, color) = toast_style(toast.kind.clone());

        // Compute available space for the message text (subtract prefix width)
        #[allow(clippy::cast_possible_truncation)]
        let prefix_len = prefix.len() as u16;
        let max_msg_len = toast_width.saturating_sub(prefix_len) as usize;

        let truncated_msg = truncate_message(&toast.message, max_msg_len);

        let line = Line::from(vec![
            Span::styled(prefix, Style::default().fg(color)),
            Span::styled(truncated_msg, Style::default().fg(color)),
        ]);

        // Position: bottom-right corner, stacked from bottom up
        // The i-th toast (0 = oldest visible) goes at row:
        //   area.y + area.height - num_toasts + i
        #[allow(clippy::cast_possible_truncation)]
        let y = area.y + area.height - (num_toasts as u16) + (i as u16);
        let x = area.x + area.width.saturating_sub(toast_width);

        let toast_area = Rect::new(x, y, toast_width, 1);
        let paragraph = Paragraph::new(line);
        frame.render_widget(paragraph, toast_area);
    }
}

/// Return the prefix string and color for a given toast severity.
fn toast_style(kind: ToastKind) -> (String, Color) {
    match kind {
        ToastKind::Success => ("✓ ".to_string(), theme::SUCCESS),
        ToastKind::Warning => ("⚠ ".to_string(), theme::WARNING),
        ToastKind::Error => ("✗ ".to_string(), theme::DANGER),
        ToastKind::Info => ("ℹ ".to_string(), theme::INFO),
    }
}

/// Truncate a message to fit within `max_len` characters.
/// If truncation is needed, the last visible character is replaced with "…".
fn truncate_message(msg: &str, max_len: usize) -> String {
    if max_len == 0 {
        return String::new();
    }

    let char_count = msg.chars().count();
    if char_count <= max_len {
        msg.to_string()
    } else {
        // Truncate and append ellipsis
        let truncated: String = msg.chars().take(max_len.saturating_sub(1)).collect();
        format!("{truncated}…")
    }
}

/// Push a toast into the queue, evicting the oldest if the queue exceeds 5.
///
/// This is a free function operating on the toast `VecDeque` directly,
/// useful for contexts where `AppState` methods are not accessible.
pub fn push_toast(
    toasts: &mut std::collections::VecDeque<crate::app::Toast>,
    kind: ToastKind,
    message: String,
    persistent: bool,
    duration_secs: u64,
) {
    if toasts.len() >= MAX_VISIBLE_TOASTS {
        toasts.pop_front();
    }
    toasts.push_back(crate::app::Toast {
        kind,
        message,
        created_at: std::time::Instant::now(),
        duration_secs,
        persistent,
    });
}

/// Remove expired non-persistent toasts from the queue.
///
/// Persistent toasts are never auto-dismissed. Non-persistent toasts are
/// removed once their elapsed time exceeds `duration_secs`.
pub fn tick_toasts(
    toasts: &mut std::collections::VecDeque<crate::app::Toast>,
    _duration_secs: u64,
) {
    let now = std::time::Instant::now();
    toasts.retain(|t| t.persistent || now.duration_since(t.created_at).as_secs() < t.duration_secs);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Toast, ToastKind};
    use std::collections::VecDeque;
    use std::time::Instant;

    use proptest::prelude::*;

    #[test]
    fn truncate_message_short_message_unchanged() {
        let result = truncate_message("hello", 10);
        assert_eq!(result, "hello");
    }

    #[test]
    fn truncate_message_exact_length_unchanged() {
        let result = truncate_message("hello", 5);
        assert_eq!(result, "hello");
    }

    #[test]
    fn truncate_message_too_long_gets_ellipsis() {
        let result = truncate_message("hello world", 8);
        // 7 chars + "…" = 8 display chars
        assert_eq!(result, "hello w…");
    }

    #[test]
    fn truncate_message_zero_max_returns_empty() {
        let result = truncate_message("hello", 0);
        assert_eq!(result, "");
    }

    #[test]
    fn toast_style_success_is_green() {
        let (prefix, color) = toast_style(ToastKind::Success);
        assert_eq!(prefix, "✓ ");
        assert_eq!(color, Color::Green);
    }

    #[test]
    fn toast_style_warning_is_yellow() {
        let (prefix, color) = toast_style(ToastKind::Warning);
        assert_eq!(prefix, "⚠ ");
        assert_eq!(color, Color::Yellow);
    }

    #[test]
    fn toast_style_error_is_red() {
        let (prefix, color) = toast_style(ToastKind::Error);
        assert_eq!(prefix, "✗ ");
        assert_eq!(color, Color::Red);
    }

    #[test]
    fn toast_style_info_is_blue() {
        let (prefix, color) = toast_style(ToastKind::Info);
        assert_eq!(prefix, "ℹ ");
        assert_eq!(color, Color::Blue);
    }

    #[test]
    fn push_toast_evicts_oldest_when_full() {
        let mut toasts: VecDeque<Toast> = VecDeque::new();
        for i in 0..5 {
            push_toast(&mut toasts, ToastKind::Info, format!("msg {i}"), false, 5);
        }
        assert_eq!(toasts.len(), 5);

        // Push one more — oldest should be evicted
        push_toast(&mut toasts, ToastKind::Success, "new".to_string(), false, 5);
        assert_eq!(toasts.len(), 5);
        // First toast should now be "msg 1" (msg 0 was evicted)
        assert_eq!(toasts.front().map(|t| t.message.as_str()), Some("msg 1"));
        assert_eq!(toasts.back().map(|t| t.message.as_str()), Some("new"));
    }

    #[test]
    fn tick_toasts_removes_expired_non_persistent() {
        let mut toasts: VecDeque<Toast> = VecDeque::new();
        // Create an expired toast (duration_secs = 0 means it expires immediately)
        toasts.push_back(Toast {
            kind: ToastKind::Info,
            message: "expired".to_string(),
            created_at: Instant::now() - std::time::Duration::from_secs(10),
            duration_secs: 5,
            persistent: false,
        });
        // Create a fresh toast
        toasts.push_back(Toast {
            kind: ToastKind::Success,
            message: "fresh".to_string(),
            created_at: Instant::now(),
            duration_secs: 5,
            persistent: false,
        });

        tick_toasts(&mut toasts, 5);

        assert_eq!(toasts.len(), 1);
        assert_eq!(toasts.front().map(|t| t.message.as_str()), Some("fresh"));
    }

    #[test]
    fn tick_toasts_keeps_persistent_toasts() {
        let mut toasts: VecDeque<Toast> = VecDeque::new();
        // Create a persistent toast with old timestamp
        toasts.push_back(Toast {
            kind: ToastKind::Warning,
            message: "budget warning".to_string(),
            created_at: Instant::now() - std::time::Duration::from_secs(3600),
            duration_secs: 0,
            persistent: true,
        });

        tick_toasts(&mut toasts, 5);

        assert_eq!(toasts.len(), 1);
        assert_eq!(
            toasts.front().map(|t| t.message.as_str()),
            Some("budget warning")
        );
    }

    #[test]
    fn push_persistent_toast_sets_persistent_flag() {
        let mut toasts: VecDeque<Toast> = VecDeque::new();
        push_toast(
            &mut toasts,
            ToastKind::Warning,
            "budget >80%".to_string(),
            true,
            0,
        );

        assert_eq!(toasts.len(), 1);
        let toast = toasts.front().unwrap_or_else(|| unreachable!());
        assert!(toast.persistent);
        assert_eq!(toast.kind, ToastKind::Warning);
    }

    /// Generate a random ToastKind.
    fn arb_toast_kind() -> impl Strategy<Value = ToastKind> {
        prop_oneof![
            Just(ToastKind::Success),
            Just(ToastKind::Warning),
            Just(ToastKind::Error),
            Just(ToastKind::Info),
        ]
    }

    // Feature: tui-layout-redesign, Property 14: Toast queue never exceeds 5
    // Validates: Requirements 13.6
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_toast_queue_never_exceeds_5(
            pushes in prop::collection::vec(
                (arb_toast_kind(), ".*", any::<bool>(), 1u64..=60),
                0..=100
            )
        ) {
            let mut toasts: VecDeque<Toast> = VecDeque::new();

            for (kind, message, persistent, duration_secs) in pushes {
                push_toast(&mut toasts, kind, message, persistent, duration_secs);
                // After every push, the queue length must never exceed 5
                prop_assert!(
                    toasts.len() <= 5,
                    "Toast queue exceeded 5: got {}",
                    toasts.len()
                );
            }
        }
    }
}
