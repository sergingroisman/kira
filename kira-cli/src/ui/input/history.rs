#![forbid(unsafe_code)]

//! History navigation — cycles through up to 100 stored input entries
//! using Up/Down arrow keys when the cursor is on the first/last line.
//!
//! History is stored newest-first (index 0 = most recent).
//! `current_pos = None` means the user is at the live input (not navigating history).

/// Maximum number of history entries stored.
pub const MAX_HISTORY_ENTRIES: usize = 100;

/// Navigate to the previous (older) history entry.
///
/// # Rules
/// - From `None` (live input) → position 0 (most recent entry)
/// - From position N → position N+1 (older), capped at `history.len() - 1`
/// - If history is empty → no-op (returns `(None, None)`)
///
/// Returns `(new_position, text_to_display)`.
/// `text_to_display` is `Some(&str)` with the history entry content,
/// or `None` if navigation is a no-op.
pub fn navigate_history_up(
    history: &[String],
    current_pos: Option<usize>,
) -> (Option<usize>, Option<&str>) {
    if history.is_empty() {
        return (None, None);
    }

    let new_pos = match current_pos {
        None => 0,
        Some(pos) => {
            let max_idx = history.len().saturating_sub(1);
            if pos >= max_idx {
                // Already at the oldest entry, no-op
                return (Some(pos), None);
            }
            pos + 1
        }
    };

    history.get(new_pos).map_or((current_pos, None), |entry| {
        (Some(new_pos), Some(entry.as_str()))
    })
}

/// Navigate to the next (newer) history entry.
///
/// # Rules
/// - From `None` (live input) → no-op (stay at live input)
/// - From position 0 → `None` (back to live input), returns `None` text
///   to signal the caller should restore the original input
/// - From position N → position N-1 (newer entry)
///
/// Returns `(new_position, text_to_display)`.
/// `text_to_display` is `Some(&str)` with the history entry content,
/// or `None` meaning "restore original input" (when returning to live input).
pub fn navigate_history_down(
    history: &[String],
    current_pos: Option<usize>,
) -> (Option<usize>, Option<&str>) {
    match current_pos {
        None => {
            // Already at live input, no-op
            (None, None)
        }
        Some(0) => {
            // Moving from most recent entry back to live input
            (None, None)
        }
        Some(pos) => {
            let new_pos = pos - 1;
            history.get(new_pos).map_or((current_pos, None), |entry| {
                (Some(new_pos), Some(entry.as_str()))
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── navigate_history_up tests ───

    #[test]
    fn up_empty_history_is_noop() {
        let history: Vec<String> = vec![];
        let (pos, text) = navigate_history_up(&history, None);
        assert_eq!(pos, None);
        assert_eq!(text, None);
    }

    #[test]
    fn up_from_live_input_goes_to_most_recent() {
        let history = vec!["recent".to_string(), "older".to_string()];
        let (pos, text) = navigate_history_up(&history, None);
        assert_eq!(pos, Some(0));
        assert_eq!(text, Some("recent"));
    }

    #[test]
    fn up_from_pos_0_goes_to_pos_1() {
        let history = vec![
            "recent".to_string(),
            "older".to_string(),
            "oldest".to_string(),
        ];
        let (pos, text) = navigate_history_up(&history, Some(0));
        assert_eq!(pos, Some(1));
        assert_eq!(text, Some("older"));
    }

    #[test]
    fn up_at_oldest_entry_is_noop() {
        let history = vec!["recent".to_string(), "oldest".to_string()];
        let (pos, text) = navigate_history_up(&history, Some(1));
        assert_eq!(pos, Some(1));
        assert_eq!(text, None);
    }

    #[test]
    fn up_single_entry_from_none_goes_to_0() {
        let history = vec!["only".to_string()];
        let (pos, text) = navigate_history_up(&history, None);
        assert_eq!(pos, Some(0));
        assert_eq!(text, Some("only"));
    }

    #[test]
    fn up_single_entry_at_0_is_noop() {
        let history = vec!["only".to_string()];
        let (pos, text) = navigate_history_up(&history, Some(0));
        assert_eq!(pos, Some(0));
        assert_eq!(text, None);
    }

    // ─── navigate_history_down tests ───

    #[test]
    fn down_from_live_input_is_noop() {
        let history = vec!["recent".to_string(), "older".to_string()];
        let (pos, text) = navigate_history_down(&history, None);
        assert_eq!(pos, None);
        assert_eq!(text, None);
    }

    #[test]
    fn down_from_pos_0_returns_to_live_input() {
        let history = vec!["recent".to_string(), "older".to_string()];
        let (pos, text) = navigate_history_down(&history, Some(0));
        assert_eq!(pos, None);
        assert_eq!(text, None);
    }

    #[test]
    fn down_from_pos_1_goes_to_pos_0() {
        let history = vec!["recent".to_string(), "older".to_string()];
        let (pos, text) = navigate_history_down(&history, Some(1));
        assert_eq!(pos, Some(0));
        assert_eq!(text, Some("recent"));
    }

    #[test]
    fn down_from_pos_2_goes_to_pos_1() {
        let history = vec![
            "newest".to_string(),
            "middle".to_string(),
            "oldest".to_string(),
        ];
        let (pos, text) = navigate_history_down(&history, Some(2));
        assert_eq!(pos, Some(1));
        assert_eq!(text, Some("middle"));
    }

    #[test]
    fn down_empty_history_from_none_is_noop() {
        let history: Vec<String> = vec![];
        let (pos, text) = navigate_history_down(&history, None);
        assert_eq!(pos, None);
        assert_eq!(text, None);
    }

    // ─── round-trip tests ───

    #[test]
    fn full_cycle_up_and_back_down() {
        let history = vec![
            "first".to_string(),
            "second".to_string(),
            "third".to_string(),
        ];

        // Start at live input, go up through all entries
        let (pos, text) = navigate_history_up(&history, None);
        assert_eq!(pos, Some(0));
        assert_eq!(text, Some("first"));

        let (pos, text) = navigate_history_up(&history, pos);
        assert_eq!(pos, Some(1));
        assert_eq!(text, Some("second"));

        let (pos, text) = navigate_history_up(&history, pos);
        assert_eq!(pos, Some(2));
        assert_eq!(text, Some("third"));

        // At oldest, up is noop
        let (pos, text) = navigate_history_up(&history, pos);
        assert_eq!(pos, Some(2));
        assert_eq!(text, None);

        // Now go back down
        let (pos, text) = navigate_history_down(&history, Some(2));
        assert_eq!(pos, Some(1));
        assert_eq!(text, Some("second"));

        let (pos, text) = navigate_history_down(&history, pos);
        assert_eq!(pos, Some(0));
        assert_eq!(text, Some("first"));

        // Back to live input
        let (pos, text) = navigate_history_down(&history, pos);
        assert_eq!(pos, None);
        assert_eq!(text, None);

        // Down from live input is noop
        let (pos, text) = navigate_history_down(&history, pos);
        assert_eq!(pos, None);
        assert_eq!(text, None);
    }

    // ─── MAX_HISTORY_ENTRIES constant test ───

    #[test]
    fn max_history_entries_is_100() {
        assert_eq!(MAX_HISTORY_ENTRIES, 100);
    }

    #[test]
    fn up_navigates_through_large_history() {
        let history: Vec<String> = (0..MAX_HISTORY_ENTRIES)
            .map(|i| format!("entry_{}", i))
            .collect();

        // Navigate all the way to the oldest
        let mut pos = None;
        for i in 0..MAX_HISTORY_ENTRIES {
            let (new_pos, text) = navigate_history_up(&history, pos);
            assert_eq!(new_pos, Some(i));
            assert_eq!(text, Some(format!("entry_{}", i).as_str()));
            pos = new_pos;
        }

        // At the end, up is noop
        let (final_pos, text) = navigate_history_up(&history, pos);
        assert_eq!(final_pos, Some(MAX_HISTORY_ENTRIES - 1));
        assert_eq!(text, None);
    }
}
