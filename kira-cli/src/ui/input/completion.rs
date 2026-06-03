#![forbid(unsafe_code)]

//! Completion popup — handles slash command completion (triggered by `/`)
//! and file mention completion (triggered by `@`) with fuzzy matching.
//!
//! Available slash commands: `/sessions`, `/models`, `/agents`, `/status`,
//! `/help`, `/quit`, `/waves`, `/budget`.

use crate::models::{CompletionItem, CompletionKind, CompletionState};

/// Maximum number of completion items to display.
const MAX_COMPLETION_ITEMS: usize = 10;

/// Available slash commands.
pub const SLASH_COMMANDS: &[&str] = &[
    "/sessions",
    "/models",
    "/agents",
    "/status",
    "/help",
    "/quit",
    "/waves",
    "/budget",
];

/// Filters available slash commands by prefix match, returning max 10 items.
///
/// The `prefix` should include the leading `/`. Commands whose name starts
/// with the given prefix (case-insensitive) are included in the result.
pub fn filter_slash_commands(commands: &[&str], prefix: &str) -> Vec<CompletionItem> {
    let prefix_lower = prefix.to_lowercase();
    commands
        .iter()
        .filter(|cmd| cmd.to_lowercase().starts_with(&prefix_lower))
        .take(MAX_COMPLETION_ITEMS)
        .map(|cmd| CompletionItem {
            label: cmd.to_string(),
            description: None,
        })
        .collect()
}

/// Fuzzy matches file paths against a query string, returning max 10 items
/// sorted by relevance (shorter matches and earlier substring positions rank higher).
///
/// Uses simple case-insensitive substring matching. Files that contain the
/// query as a substring are included. Results are sorted by:
/// 1. Position of the match (earlier is better)
/// 2. Length of the path (shorter is better)
pub fn filter_file_mentions(files: &[&str], query: &str) -> Vec<CompletionItem> {
    if query.is_empty() {
        return Vec::new();
    }

    let query_lower = query.to_lowercase();

    let mut scored: Vec<(usize, usize, &str)> = files
        .iter()
        .filter_map(|file| {
            let file_lower = file.to_lowercase();
            file_lower
                .find(&query_lower)
                .map(|pos| (pos, file.len(), *file))
        })
        .collect();

    // Sort by match position first, then by path length (shorter is better).
    scored.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    scored
        .into_iter()
        .take(MAX_COMPLETION_ITEMS)
        .map(|(_, _, file)| CompletionItem {
            label: file.to_string(),
            description: None,
        })
        .collect()
}

/// Determines if completion should be triggered based on input content and cursor position.
///
/// Returns `Some(CompletionKind::SlashCommand)` if the input starts with `/`
/// and the cursor is within the first word (no spaces before cursor).
///
/// Returns `Some(CompletionKind::FileMention)` if there is an `@` followed by
/// one or more non-whitespace characters before the cursor position.
///
/// Returns `None` otherwise.
pub fn should_trigger_completion(input: &str, cursor_pos: usize) -> Option<CompletionKind> {
    if input.is_empty() || cursor_pos == 0 {
        return None;
    }

    // Clamp cursor_pos to input length to avoid out-of-bounds.
    let pos = cursor_pos.min(input.len());
    let before_cursor = &input[..pos];

    // Check for slash command: input starts with `/` and no spaces before cursor.
    if before_cursor.starts_with('/') && !before_cursor.contains(' ') {
        return Some(CompletionKind::SlashCommand);
    }

    // Check for file mention: find the last `@` before cursor, followed by non-whitespace chars.
    if let Some(at_pos) = before_cursor.rfind('@') {
        let after_at = &before_cursor[at_pos + 1..];
        if !after_at.is_empty() && !after_at.contains(char::is_whitespace) {
            return Some(CompletionKind::FileMention);
        }
    }

    None
}

/// Moves the completion selection up (-1) or down (+1), wrapping around.
///
/// A `direction` of -1 moves up (toward index 0), and +1 moves down.
/// When at the top and moving up, wraps to the last item.
/// When at the bottom and moving down, wraps to the first item.
#[allow(clippy::missing_const_for_fn)]
pub fn navigate_completion(state: &mut CompletionState, direction: i8) {
    if state.items.is_empty() {
        return;
    }

    let len = state.items.len();

    match direction {
        -1 => {
            // Move up (wrap to end if at top).
            if state.selected == 0 {
                state.selected = len - 1;
            } else {
                state.selected -= 1;
            }
        }
        1 => {
            // Move down (wrap to start if at end).
            if state.selected >= len - 1 {
                state.selected = 0;
            } else {
                state.selected += 1;
            }
        }
        _ => {}
    }
}

/// Returns the label of the currently selected completion item, or `None`
/// if the state has no items or the selection is out of bounds.
pub fn accept_completion(state: &CompletionState) -> Option<String> {
    state
        .items
        .get(state.selected)
        .map(|item| item.label.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── filter_slash_commands tests ────────────────────────────────────────

    #[test]
    fn filter_slash_commands_full_prefix() {
        let results = filter_slash_commands(SLASH_COMMANDS, "/s");
        assert!(results.iter().all(|r| r.label.starts_with("/s")));
        assert!(results.iter().any(|r| r.label == "/sessions"));
        assert!(results.iter().any(|r| r.label == "/status"));
    }

    #[test]
    fn filter_slash_commands_exact_match() {
        let results = filter_slash_commands(SLASH_COMMANDS, "/quit");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].label, "/quit");
    }

    #[test]
    fn filter_slash_commands_no_match() {
        let results = filter_slash_commands(SLASH_COMMANDS, "/xyz");
        assert!(results.is_empty());
    }

    #[test]
    fn filter_slash_commands_empty_prefix_returns_all() {
        let results = filter_slash_commands(SLASH_COMMANDS, "");
        // Empty string is a prefix of everything.
        assert_eq!(results.len(), SLASH_COMMANDS.len());
    }

    #[test]
    fn filter_slash_commands_case_insensitive() {
        let results = filter_slash_commands(SLASH_COMMANDS, "/S");
        assert!(results.iter().any(|r| r.label == "/sessions"));
        assert!(results.iter().any(|r| r.label == "/status"));
    }

    #[test]
    fn filter_slash_commands_max_10_items() {
        // Create more than 10 commands.
        let many_commands: Vec<&str> = (0..15).map(|_| "/test").collect();
        let results = filter_slash_commands(&many_commands, "/t");
        assert!(results.len() <= MAX_COMPLETION_ITEMS);
    }

    // ── filter_file_mentions tests ─────────────────────────────────────────

    #[test]
    fn filter_file_mentions_substring_match() {
        let files = &["src/main.rs", "src/lib.rs", "tests/test.rs"];
        let results = filter_file_mentions(files, "main");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].label, "src/main.rs");
    }

    #[test]
    fn filter_file_mentions_empty_query_returns_empty() {
        let files = &["src/main.rs", "src/lib.rs"];
        let results = filter_file_mentions(files, "");
        assert!(results.is_empty());
    }

    #[test]
    fn filter_file_mentions_case_insensitive() {
        let files = &["src/Main.rs", "src/lib.rs"];
        let results = filter_file_mentions(files, "main");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].label, "src/Main.rs");
    }

    #[test]
    fn filter_file_mentions_sorted_by_position() {
        let files = &["zzz/foo.rs", "foo/bar.rs", "abc/foo/baz.rs"];
        let results = filter_file_mentions(files, "foo");
        // "foo/bar.rs" has "foo" at position 0, others have it later.
        assert_eq!(results[0].label, "foo/bar.rs");
    }

    #[test]
    fn filter_file_mentions_max_10_items() {
        let files: Vec<&str> = (0..20).map(|_| "match.rs").collect();
        let results = filter_file_mentions(&files, "match");
        assert!(results.len() <= MAX_COMPLETION_ITEMS);
    }

    #[test]
    fn filter_file_mentions_no_match() {
        let files = &["src/main.rs", "src/lib.rs"];
        let results = filter_file_mentions(files, "xyz");
        assert!(results.is_empty());
    }

    // ── should_trigger_completion tests ────────────────────────────────────

    #[test]
    fn trigger_slash_command_at_start() {
        let result = should_trigger_completion("/ses", 4);
        assert_eq!(result, Some(CompletionKind::SlashCommand));
    }

    #[test]
    fn trigger_slash_command_just_slash() {
        let result = should_trigger_completion("/", 1);
        assert_eq!(result, Some(CompletionKind::SlashCommand));
    }

    #[test]
    fn no_trigger_slash_with_space() {
        let result = should_trigger_completion("/cmd arg", 8);
        assert_eq!(result, None);
    }

    #[test]
    fn trigger_file_mention() {
        let result = should_trigger_completion("hello @src/m", 12);
        assert_eq!(result, Some(CompletionKind::FileMention));
    }

    #[test]
    fn no_trigger_at_sign_alone() {
        // `@` with no chars after it should not trigger.
        let result = should_trigger_completion("hello @", 7);
        assert_eq!(result, None);
    }

    #[test]
    fn no_trigger_empty_input() {
        let result = should_trigger_completion("", 0);
        assert_eq!(result, None);
    }

    #[test]
    fn no_trigger_cursor_at_zero() {
        let result = should_trigger_completion("/hello", 0);
        assert_eq!(result, None);
    }

    #[test]
    fn no_trigger_file_mention_with_space_after_at() {
        let result = should_trigger_completion("hello @ world", 8);
        assert_eq!(result, None);
    }

    // ── navigate_completion tests ──────────────────────────────────────────

    #[test]
    fn navigate_down_increments() {
        let mut state = make_state(5, 0);
        navigate_completion(&mut state, 1);
        assert_eq!(state.selected, 1);
    }

    #[test]
    fn navigate_down_wraps_at_end() {
        let mut state = make_state(5, 4);
        navigate_completion(&mut state, 1);
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn navigate_up_decrements() {
        let mut state = make_state(5, 3);
        navigate_completion(&mut state, -1);
        assert_eq!(state.selected, 2);
    }

    #[test]
    fn navigate_up_wraps_at_start() {
        let mut state = make_state(5, 0);
        navigate_completion(&mut state, -1);
        assert_eq!(state.selected, 4);
    }

    #[test]
    fn navigate_empty_items_is_noop() {
        let mut state = CompletionState {
            kind: CompletionKind::SlashCommand,
            items: Vec::new(),
            selected: 0,
            filter: String::new(),
        };
        navigate_completion(&mut state, 1);
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn navigate_invalid_direction_is_noop() {
        let mut state = make_state(5, 2);
        navigate_completion(&mut state, 0);
        assert_eq!(state.selected, 2);
    }

    // ── accept_completion tests ────────────────────────────────────────────

    #[test]
    fn accept_returns_selected_label() {
        let state = make_state(3, 1);
        let result = accept_completion(&state);
        assert_eq!(result, Some("item_1".to_string()));
    }

    #[test]
    fn accept_empty_items_returns_none() {
        let state = CompletionState {
            kind: CompletionKind::SlashCommand,
            items: Vec::new(),
            selected: 0,
            filter: String::new(),
        };
        let result = accept_completion(&state);
        assert_eq!(result, None);
    }

    #[test]
    fn accept_out_of_bounds_returns_none() {
        let state = CompletionState {
            kind: CompletionKind::SlashCommand,
            items: vec![CompletionItem {
                label: "only".to_string(),
                description: None,
            }],
            selected: 5, // out of bounds
            filter: String::new(),
        };
        let result = accept_completion(&state);
        assert_eq!(result, None);
    }

    // ── Test helpers ───────────────────────────────────────────────────────

    fn make_state(item_count: usize, selected: usize) -> CompletionState {
        let items = (0..item_count)
            .map(|i| CompletionItem {
                label: format!("item_{i}"),
                description: None,
            })
            .collect();

        CompletionState {
            kind: CompletionKind::SlashCommand,
            items,
            selected,
            filter: String::new(),
        }
    }

    // ── Property-based tests ───────────────────────────────────────────────

    mod property_tests {
        use super::*;
        use proptest::prelude::*;

        // Feature: tui-layout-redesign, Property 8: Slash command completion filters by prefix
        // Validates: Requirements 5.11
        //
        // For any prefix string starting with `/`, the completion popup SHALL
        // contain only commands whose name starts with that prefix, the result
        // count SHALL not exceed 10, and the result set SHALL be a subset of
        // the available commands.
        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            #[test]
            fn prop_slash_command_completion_filters_by_prefix(
                suffix in "[a-z]{0,10}"
            ) {
                let prefix = format!("/{suffix}");
                let results = filter_slash_commands(SLASH_COMMANDS, &prefix);

                // Assertion 1: Result count ≤ 10
                prop_assert!(
                    results.len() <= MAX_COMPLETION_ITEMS,
                    "Result count {} exceeds max {}",
                    results.len(),
                    MAX_COMPLETION_ITEMS
                );

                // Assertion 2: All results start with the given prefix (case-insensitive)
                let prefix_lower = prefix.to_lowercase();
                for item in &results {
                    prop_assert!(
                        item.label.to_lowercase().starts_with(&prefix_lower),
                        "Item '{}' does not start with prefix '{}'",
                        item.label,
                        prefix
                    );
                }

                // Assertion 3: All results are a subset of the input commands
                for item in &results {
                    prop_assert!(
                        SLASH_COMMANDS.contains(&item.label.as_str()),
                        "Item '{}' is not in the available commands",
                        item.label
                    );
                }
            }
        }
    }
}
