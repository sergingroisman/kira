#![forbid(unsafe_code)]

//! Input event routing and keybinding management.
//!
//! This module provides focus-aware input routing, a keybinding registry
//! for multi-key sequences, and the `InputResult` enum that represents
//! the outcome of processing a key event.

pub mod focus;
pub mod keybindings;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub use focus::{FocusStack, is_global_keybinding, should_route_to_overlay};
pub use keybindings::{KeyBinding, KeyContext, KeymapRegistry};

use crate::app::AppState;

/// The result of processing a key event through the input router.
///
/// Each variant represents a distinct action the event loop should take
/// after the input system has consumed a key event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputResult {
    /// The key was consumed by the current handler; no further action needed.
    Consumed,
    /// The operator requested to quit the application (e.g., Ctrl+C).
    Quit,
    /// The operator submitted a chat message.
    SubmitMessage(String),
    /// The operator entered a slash command (e.g., `/sessions`).
    SlashCommand(String),
    /// Toggle sidebar visibility (Tab key).
    ToggleSidebar,
    /// Toggle between Plan and Yolo modes (Ctrl+M).
    ToggleMode,
    /// Toggle the Layers View visibility (Ctrl+L).
    ToggleLayers,
    /// Open an overlay by name.
    OpenOverlay(OverlayKind),
    /// Close the topmost overlay.
    CloseOverlay,
    /// Scroll the chat panel by the given number of lines (positive = down, negative = up).
    ScrollChat(i16),
    /// Navigate history (positive = forward, negative = backward).
    HistoryNavigate(i16),
    /// Trigger completion popup.
    TriggerCompletion,
    /// Accept the currently selected completion item.
    AcceptCompletion,
}

/// Identifies which overlay to open, without carrying full state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayKind {
    CommandPalette,
    SessionPicker,
    TaskBrowser,
    Help,
}

/// Route a key event through the focus stack and keybinding registry.
///
/// This is the main entry point for the event loop to dispatch key events.
/// It checks the overlay stack first, then delegates to the focused base panel,
/// with global keybindings handled before panel-specific bindings.
///
/// Routing order:
/// 1. Global keybindings (Ctrl+C, Ctrl+P, Ctrl+M, Tab) — always checked first
/// 2. If overlay stack is non-empty → route to overlay handler
/// 3. Otherwise → route to base panel handler (input area by default)
pub fn handle_key(key: KeyEvent, state: &mut AppState) -> InputResult {
    // ── Step 1: Check global keybindings ──────────────────────────────────
    if is_global_keybinding(&key) {
        return handle_global(key, state);
    }

    // ── Step 2: If overlay stack is non-empty, route to overlay handler ───
    if should_route_to_overlay(&state.overlay_stack) {
        return handle_overlay_key(key, state);
    }

    // ── Step 3: Route to base panel handler (input area) ──────────────────
    handle_base_panel_key(key, state)
}

/// Handle a global keybinding. Called when `is_global_keybinding` returns true.
#[allow(clippy::missing_const_for_fn)]
fn handle_global(key: KeyEvent, _state: &AppState) -> InputResult {
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => InputResult::Quit,
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            InputResult::OpenOverlay(OverlayKind::CommandPalette)
        }
        // NOTE: Ctrl+M is indistinguishable from Enter in a terminal (both are
        // CR / 0x0D), so mode toggle is bound to Ctrl+O instead.
        KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            InputResult::ToggleMode
        }
        KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            InputResult::ToggleLayers
        }
        KeyCode::Tab if key.modifiers.is_empty() => InputResult::ToggleSidebar,
        _ => InputResult::Consumed,
    }
}

/// Handle key events when an overlay is active (top of overlay stack).
///
/// Overlays consume Escape to close, and pass through other keys to
/// overlay-specific logic. For now, overlays handle navigation (Up/Down),
/// selection (Enter), and text input for filtering.
#[allow(clippy::missing_const_for_fn, clippy::needless_pass_by_ref_mut)]
fn handle_overlay_key(key: KeyEvent, state: &mut AppState) -> InputResult {
    match key.code {
        KeyCode::Esc => InputResult::CloseOverlay,
        KeyCode::Up => {
            // Navigate up in overlay list (placeholder)
            let _ = state;
            InputResult::Consumed
        }
        KeyCode::Down => {
            // Navigate down in overlay list (placeholder)
            let _ = state;
            InputResult::Consumed
        }
        KeyCode::Enter => {
            // Select current item in overlay — for now just close
            InputResult::CloseOverlay
        }
        KeyCode::Char(_c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            // Text input for overlay filtering (placeholder)
            let _ = state;
            InputResult::Consumed
        }
        KeyCode::Backspace => {
            // Backspace for overlay filter editing (placeholder)
            let _ = state;
            InputResult::Consumed
        }
        _ => InputResult::Consumed,
    }
}

/// Handle key events for the base panel (input area + scroll).
///
/// This preserves the existing behavior:
/// - Escape: if detail panel expanded → collapse it; else if layers visible → dismiss layers
/// - Enter submits message or slash command
/// - Up/Down navigates history
/// - Backspace deletes character
/// - PageUp/PageDown scrolls output
/// - 1-4 (when layers visible and input empty): toggle layer detail drill-down
/// - Character keys insert into input buffer
fn handle_base_panel_key(key: KeyEvent, state: &mut AppState) -> InputResult {
    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Esc) if state.layers_visible => {
            // If detail panel is expanded, collapse it first
            if state.layers_state.detail_expanded.is_some() {
                state.layers_state.detail_expanded = None;
                InputResult::Consumed
            } else {
                // Dismiss layers view when Escape is pressed and no overlay is active
                InputResult::ToggleLayers
            }
        }
        (KeyModifiers::NONE, KeyCode::Enter) => handle_submit(state),
        (KeyModifiers::NONE, KeyCode::Up) => {
            handle_history_up(state);
            InputResult::Consumed
        }
        (KeyModifiers::NONE, KeyCode::Down) => {
            handle_history_down(state);
            InputResult::Consumed
        }
        (KeyModifiers::NONE, KeyCode::Backspace) => {
            handle_backspace(state);
            InputResult::Consumed
        }
        // Cursor navigation within the input — lets the user go back to edit
        // mid-text instead of being stuck at the end.
        (m, KeyCode::Left) if m.contains(KeyModifiers::CONTROL) || m.contains(KeyModifiers::ALT) => {
            word_left(state);
            InputResult::Consumed
        }
        (m, KeyCode::Right) if m.contains(KeyModifiers::CONTROL) || m.contains(KeyModifiers::ALT) => {
            word_right(state);
            InputResult::Consumed
        }
        (KeyModifiers::NONE, KeyCode::Left) => {
            state.input_cursor = state.input_cursor.saturating_sub(1);
            InputResult::Consumed
        }
        (KeyModifiers::NONE, KeyCode::Right) => {
            state.input_cursor = (state.input_cursor + 1).min(state.input.chars().count());
            InputResult::Consumed
        }
        (KeyModifiers::NONE, KeyCode::Home) => {
            state.input_cursor = 0;
            InputResult::Consumed
        }
        (KeyModifiers::NONE, KeyCode::End) => {
            state.input_cursor = state.input.chars().count();
            InputResult::Consumed
        }
        (KeyModifiers::NONE, KeyCode::Delete) => {
            delete_forward(state);
            InputResult::Consumed
        }
        (KeyModifiers::NONE, KeyCode::PageUp) => {
            handle_page_up(state);
            InputResult::Consumed
        }
        (KeyModifiers::NONE, KeyCode::PageDown) => {
            handle_page_down(state);
            InputResult::Consumed
        }
        (mods, KeyCode::Char(c)) if !mods.contains(KeyModifiers::CONTROL) => {
            // Route number keys 1-4 to layers drill-down when layers are visible
            // and the input area does not have active cursor focus (input is empty).
            if state.layers_visible
                && state.input.is_empty()
                && let Some(layer_idx) = layer_index_from_char(c)
            {
                handle_layer_drilldown(state, layer_idx);
                return InputResult::Consumed;
            }
            handle_char_input(state, c);
            InputResult::Consumed
        }
        _ => InputResult::Consumed,
    }
}

// ── Base panel key handlers ──────────────────────────────────────────────────

/// Map a character '1'-'4' to a layer index (0-3).
/// Returns None for characters outside this range.
const fn layer_index_from_char(c: char) -> Option<usize> {
    match c {
        '1' => Some(0),
        '2' => Some(1),
        '3' => Some(2),
        '4' => Some(3),
        _ => None,
    }
}

/// Handle layer drill-down: toggle the detail panel for the given layer index.
///
/// - If the detail panel is already expanded for this layer, collapse it (toggle off).
/// - If the detail panel is collapsed or expanded for a different layer, expand it for this layer.
fn handle_layer_drilldown(state: &mut AppState, layer_idx: usize) {
    if state.layers_state.detail_expanded == Some(layer_idx) {
        // Same layer pressed again — collapse
        state.layers_state.detail_expanded = None;
    } else {
        // Expand for the requested layer
        state.layers_state.detail_expanded = Some(layer_idx);
    }
}

/// Handle Enter key: submit message or slash command.
///
/// Returns `SubmitMessage` or `SlashCommand` if input is non-empty,
/// otherwise `Consumed` (no-op for empty/whitespace-only input).
fn handle_submit(state: &mut AppState) -> InputResult {
    let input = state.input.trim().to_string();
    if input.is_empty() {
        return InputResult::Consumed;
    }

    // Save to history
    state.history.push(input.clone());
    if state.history.len() > 50 {
        state.history.remove(0);
    }
    state.history_pos = None;

    // Clear input buffer
    state.input.clear();
    state.input_cursor = 0;

    if input.starts_with('/') {
        InputResult::SlashCommand(input)
    } else {
        InputResult::SubmitMessage(input)
    }
}

/// Handle Up arrow: navigate to previous history entry.
fn handle_history_up(state: &mut AppState) {
    let len = state.history.len();
    if len > 0 {
        let pos = state.history_pos.map_or(len - 1, |p| p.saturating_sub(1));
        state.history_pos = Some(pos);
        if let Some(entry) = state.history.get(pos) {
            state.input = entry.clone();
            state.input_cursor = state.input.len();
        }
    }
}

/// Handle Down arrow: navigate to next history entry.
fn handle_history_down(state: &mut AppState) {
    if let Some(pos) = state.history_pos {
        if pos + 1 < state.history.len() {
            let next = pos + 1;
            state.history_pos = Some(next);
            if let Some(entry) = state.history.get(next) {
                state.input = entry.clone();
                state.input_cursor = state.input.len();
            }
        } else {
            state.history_pos = None;
            state.input.clear();
            state.input_cursor = 0;
        }
    }
}

/// Handle Backspace: delete character before cursor.
fn handle_backspace(state: &mut AppState) {
    if state.input_cursor > 0 {
        let char_pos = state.input_cursor - 1;
        let byte_pos = state
            .input
            .char_indices()
            .nth(char_pos)
            .map_or(state.input.len(), |(i, _)| i);
        state.input.remove(byte_pos);
        state.input_cursor = char_pos;
    }
}

/// Handle PageUp: scroll output panel up by 10 lines.
#[allow(clippy::cast_possible_truncation)]
fn handle_page_up(state: &mut AppState) {
    // Clamp against RENDERED rows (markdown/wrap/cards expand one source line
    // into many) — `output.len()` was far too small, killing scroll.
    #[allow(clippy::cast_possible_truncation)]
    let total = state.rendered_rows.get().max(state.output.len());
    let max_offset = total.saturating_sub(1).min(u16::MAX as usize) as u16;
    state.scroll_offset = state.scroll_offset.saturating_add(10).min(max_offset);
    state.auto_scroll = false;
}

/// Handle PageDown: scroll output panel down by 10 lines.
#[allow(clippy::missing_const_for_fn)]
fn handle_page_down(state: &mut AppState) {
    state.scroll_offset = state.scroll_offset.saturating_sub(10);
    if state.scroll_offset == 0 {
        state.auto_scroll = true;
    }
}

/// Delete the character AT the cursor (forward delete / Del key).
fn delete_forward(state: &mut AppState) {
    let n = state.input.chars().count();
    if state.input_cursor < n {
        let byte_pos = state
            .input
            .char_indices()
            .nth(state.input_cursor)
            .map_or(state.input.len(), |(i, _)| i);
        state.input.remove(byte_pos);
    }
}

/// Move the cursor left by one word (skip whitespace, then the word).
fn word_left(state: &mut AppState) {
    let chars: Vec<char> = state.input.chars().collect();
    let mut i = state.input_cursor.min(chars.len());
    while i > 0 && chars[i - 1].is_whitespace() {
        i -= 1;
    }
    while i > 0 && !chars[i - 1].is_whitespace() {
        i -= 1;
    }
    state.input_cursor = i;
}

/// Move the cursor right by one word (skip whitespace, then the word).
fn word_right(state: &mut AppState) {
    let chars: Vec<char> = state.input.chars().collect();
    let n = chars.len();
    let mut i = state.input_cursor.min(n);
    while i < n && chars[i].is_whitespace() {
        i += 1;
    }
    while i < n && !chars[i].is_whitespace() {
        i += 1;
    }
    state.input_cursor = i;
}

/// Handle character input: insert character at cursor position.
fn handle_char_input(state: &mut AppState, c: char) {
    let byte_pos = state
        .input
        .char_indices()
        .nth(state.input_cursor)
        .map_or(state.input.len(), |(i, _)| i);
    state.input.insert(byte_pos, c);
    state.input_cursor += 1;
}

// ── Overlay key handlers ─────────────────────────────────────────────────────
// Overlay-specific state management is handled by the overlay widgets themselves.
// The input router delegates navigation and filtering to the overlay stack,
// which will be fully implemented when overlay widgets gain mutable state access.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AppState, Mode, Tier};
    use crate::config::Config;
    use crossterm::event::{KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    /// Helper to create a test AppState.
    fn test_state() -> AppState {
        AppState::new(Config::default(), Mode::Plan, Tier::Standard)
    }

    #[test]
    fn cursor_nav_left_right_home_end() {
        let mut s = test_state();
        s.input = "hello".to_string();
        s.input_cursor = 5;
        handle_base_panel_key(key_event(KeyCode::Left, KeyModifiers::NONE), &mut s);
        assert_eq!(s.input_cursor, 4);
        handle_base_panel_key(key_event(KeyCode::Home, KeyModifiers::NONE), &mut s);
        assert_eq!(s.input_cursor, 0);
        handle_base_panel_key(key_event(KeyCode::Right, KeyModifiers::NONE), &mut s);
        assert_eq!(s.input_cursor, 1);
        handle_base_panel_key(key_event(KeyCode::End, KeyModifiers::NONE), &mut s);
        assert_eq!(s.input_cursor, 5);
    }

    #[test]
    fn cursor_left_clamps_at_zero_and_right_at_len() {
        let mut s = test_state();
        s.input = "ab".to_string();
        s.input_cursor = 0;
        handle_base_panel_key(key_event(KeyCode::Left, KeyModifiers::NONE), &mut s);
        assert_eq!(s.input_cursor, 0);
        s.input_cursor = 2;
        handle_base_panel_key(key_event(KeyCode::Right, KeyModifiers::NONE), &mut s);
        assert_eq!(s.input_cursor, 2);
    }

    #[test]
    fn insert_then_left_inserts_midtext() {
        let mut s = test_state();
        s.input = "ac".to_string();
        s.input_cursor = 1; // between a and c
        handle_char_input(&mut s, 'b');
        assert_eq!(s.input, "abc");
        assert_eq!(s.input_cursor, 2);
    }

    #[test]
    fn word_jump_left_right() {
        let mut s = test_state();
        s.input = "foo bar baz".to_string();
        s.input_cursor = 11;
        word_left(&mut s);
        assert_eq!(s.input_cursor, 8); // start of "baz"
        word_left(&mut s);
        assert_eq!(s.input_cursor, 4); // start of "bar"
        word_right(&mut s);
        assert_eq!(s.input_cursor, 7); // end of "bar"
    }

    #[test]
    fn delete_forward_removes_at_cursor() {
        let mut s = test_state();
        s.input = "abc".to_string();
        s.input_cursor = 1;
        delete_forward(&mut s);
        assert_eq!(s.input, "ac");
        assert_eq!(s.input_cursor, 1);
    }

    /// Helper to create a key event.
    fn key_event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    // ── layer_index_from_char tests ──────────────────────────────────────

    #[test]
    fn layer_index_from_char_maps_1_to_0() {
        assert_eq!(layer_index_from_char('1'), Some(0));
    }

    #[test]
    fn layer_index_from_char_maps_2_to_1() {
        assert_eq!(layer_index_from_char('2'), Some(1));
    }

    #[test]
    fn layer_index_from_char_maps_3_to_2() {
        assert_eq!(layer_index_from_char('3'), Some(2));
    }

    #[test]
    fn layer_index_from_char_maps_4_to_3() {
        assert_eq!(layer_index_from_char('4'), Some(3));
    }

    #[test]
    fn layer_index_from_char_returns_none_for_5() {
        assert_eq!(layer_index_from_char('5'), None);
    }

    #[test]
    fn layer_index_from_char_returns_none_for_0() {
        assert_eq!(layer_index_from_char('0'), None);
    }

    #[test]
    fn layer_index_from_char_returns_none_for_letter() {
        assert_eq!(layer_index_from_char('a'), None);
    }

    // ── Number key drill-down routing tests ──────────────────────────────

    #[test]
    fn number_key_expands_detail_when_layers_visible_and_input_empty() {
        let mut state = test_state();
        state.layers_visible = true;
        state.input.clear();

        let key = key_event(KeyCode::Char('1'), KeyModifiers::NONE);
        let result = handle_key(key, &mut state);

        assert_eq!(result, InputResult::Consumed);
        assert_eq!(state.layers_state.detail_expanded, Some(0));
    }

    #[test]
    fn number_key_routes_to_input_when_input_non_empty() {
        let mut state = test_state();
        state.layers_visible = true;
        state.input = "hello".to_string();
        state.input_cursor = 5;

        let key = key_event(KeyCode::Char('1'), KeyModifiers::NONE);
        let result = handle_key(key, &mut state);

        assert_eq!(result, InputResult::Consumed);
        // Should be inserted into input, not routed to layers
        assert_eq!(state.layers_state.detail_expanded, None);
        assert!(state.input.contains('1'));
    }

    #[test]
    fn number_key_routes_to_input_when_layers_not_visible() {
        let mut state = test_state();
        state.layers_visible = false;
        state.input.clear();

        let key = key_event(KeyCode::Char('2'), KeyModifiers::NONE);
        let result = handle_key(key, &mut state);

        assert_eq!(result, InputResult::Consumed);
        // Should be inserted into input
        assert_eq!(state.layers_state.detail_expanded, None);
        assert_eq!(state.input, "2");
    }

    #[test]
    fn same_number_key_toggles_detail_panel_off() {
        let mut state = test_state();
        state.layers_visible = true;
        state.input.clear();
        state.layers_state.detail_expanded = Some(2);

        let key = key_event(KeyCode::Char('3'), KeyModifiers::NONE);
        let result = handle_key(key, &mut state);

        assert_eq!(result, InputResult::Consumed);
        assert_eq!(state.layers_state.detail_expanded, None);
    }

    #[test]
    fn different_number_key_switches_detail_panel() {
        let mut state = test_state();
        state.layers_visible = true;
        state.input.clear();
        state.layers_state.detail_expanded = Some(0);

        let key = key_event(KeyCode::Char('3'), KeyModifiers::NONE);
        let result = handle_key(key, &mut state);

        assert_eq!(result, InputResult::Consumed);
        assert_eq!(state.layers_state.detail_expanded, Some(2));
    }

    // ── Escape key behavior tests ────────────────────────────────────────

    #[test]
    fn escape_collapses_detail_panel_when_expanded() {
        let mut state = test_state();
        state.layers_visible = true;
        state.layers_state.detail_expanded = Some(1);

        let key = key_event(KeyCode::Esc, KeyModifiers::NONE);
        let result = handle_key(key, &mut state);

        assert_eq!(result, InputResult::Consumed);
        assert_eq!(state.layers_state.detail_expanded, None);
        // Layers should still be visible (not dismissed)
        assert!(state.layers_visible);
    }

    #[test]
    fn escape_dismisses_layers_when_no_detail_expanded() {
        let mut state = test_state();
        state.layers_visible = true;
        state.layers_state.detail_expanded = None;

        let key = key_event(KeyCode::Esc, KeyModifiers::NONE);
        let result = handle_key(key, &mut state);

        assert_eq!(result, InputResult::ToggleLayers);
    }

    #[test]
    fn number_5_through_9_route_to_input_even_with_layers_visible() {
        let mut state = test_state();
        state.layers_visible = true;
        state.input.clear();

        for c in ['5', '6', '7', '8', '9', '0'] {
            state.input.clear();
            state.input_cursor = 0;
            let key = key_event(KeyCode::Char(c), KeyModifiers::NONE);
            let result = handle_key(key, &mut state);
            assert_eq!(result, InputResult::Consumed);
            assert_eq!(state.layers_state.detail_expanded, None);
            assert_eq!(state.input, c.to_string());
        }
    }

    #[test]
    fn number_key_4_expands_execution_layer() {
        let mut state = test_state();
        state.layers_visible = true;
        state.input.clear();

        let key = key_event(KeyCode::Char('4'), KeyModifiers::NONE);
        let result = handle_key(key, &mut state);

        assert_eq!(result, InputResult::Consumed);
        assert_eq!(state.layers_state.detail_expanded, Some(3));
    }
}
