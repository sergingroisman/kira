#![forbid(unsafe_code)]

//! Focus stack management for input routing.
//!
//! The focus stack determines which component receives key events.
//! When overlays are active, the top-of-stack overlay captures input.
//! Otherwise, input is routed to the base panel (input area or chat panel).
//!
//! Focus routing rules:
//! 1. If overlay stack is non-empty → route to top overlay handler
//! 2. Otherwise → route to base panel handler based on `FocusTarget`
//! 3. Global keybindings (Ctrl+C, Ctrl+P, Ctrl+M, Tab) are checked BEFORE
//!    panel-specific bindings

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::models::OverlayStack;

/// Identifies which base panel currently has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    /// The multi-line input area (default focus).
    InputArea,
    /// The chat panel (for scroll navigation).
    ChatPanel,
    /// The sidebar.
    Sidebar,
}

/// Manages focus routing between base panels.
///
/// Overlay focus is handled separately via the `OverlayStack` in `AppState`.
/// This struct tracks which base-layer panel has focus when no overlay is active.
#[derive(Debug, Clone)]
pub struct FocusStack {
    /// The currently focused base panel.
    current: FocusTarget,
}

impl Default for FocusStack {
    fn default() -> Self {
        Self {
            current: FocusTarget::InputArea,
        }
    }
}

impl FocusStack {
    /// Create a new focus stack with default focus on the input area.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the currently focused base panel.
    pub const fn current(&self) -> FocusTarget {
        self.current
    }

    /// Set focus to a specific base panel.
    pub const fn set_focus(&mut self, target: FocusTarget) {
        self.current = target;
    }
}

/// Returns `true` if input should be routed to the top overlay handler.
///
/// When the overlay stack is non-empty, the top overlay captures all input
/// (except global keybindings which are checked separately before this).
pub const fn should_route_to_overlay(overlay_stack: &OverlayStack) -> bool {
    !overlay_stack.is_empty()
}

/// Returns `true` if the given key event is a global keybinding.
///
/// Global keybindings are handled before panel-specific bindings regardless
/// of which panel or overlay currently has focus:
/// - `Ctrl+C` — quit
/// - `Ctrl+P` — open command palette
/// - `Ctrl+M` — toggle mode (Plan/Yolo)
/// - `Ctrl+L` — toggle Layers View
/// - `Tab` (without modifiers) — toggle sidebar
pub const fn is_global_keybinding(key: &KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('c' | 'p' | 'm' | 'l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            true
        }
        KeyCode::Tab if key.modifiers.is_empty() => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    /// Helper to create a key event with given code and modifiers.
    fn key_event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    // ── FocusStack tests ──────────────────────────────────────────────────

    #[test]
    fn default_focus_is_input_area() {
        let focus = FocusStack::new();
        assert_eq!(focus.current(), FocusTarget::InputArea);
    }

    #[test]
    fn set_focus_changes_target() {
        let mut focus = FocusStack::new();
        focus.set_focus(FocusTarget::ChatPanel);
        assert_eq!(focus.current(), FocusTarget::ChatPanel);

        focus.set_focus(FocusTarget::Sidebar);
        assert_eq!(focus.current(), FocusTarget::Sidebar);
    }

    // ── should_route_to_overlay tests ─────────────────────────────────────

    #[test]
    fn empty_overlay_stack_does_not_route_to_overlay() {
        let stack = OverlayStack::new();
        assert!(!should_route_to_overlay(&stack));
    }

    #[test]
    fn non_empty_overlay_stack_routes_to_overlay() {
        let mut stack = OverlayStack::new();
        stack.push(crate::models::Overlay::Help);
        assert!(should_route_to_overlay(&stack));
    }

    #[test]
    fn overlay_stack_after_pop_does_not_route() {
        let mut stack = OverlayStack::new();
        stack.push(crate::models::Overlay::Help);
        stack.pop();
        assert!(!should_route_to_overlay(&stack));
    }

    // ── is_global_keybinding tests ────────────────────────────────────────

    #[test]
    fn ctrl_c_is_global() {
        let key = key_event(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(is_global_keybinding(&key));
    }

    #[test]
    fn ctrl_p_is_global() {
        let key = key_event(KeyCode::Char('p'), KeyModifiers::CONTROL);
        assert!(is_global_keybinding(&key));
    }

    #[test]
    fn ctrl_m_is_global() {
        let key = key_event(KeyCode::Char('m'), KeyModifiers::CONTROL);
        assert!(is_global_keybinding(&key));
    }

    #[test]
    fn ctrl_l_is_global() {
        let key = key_event(KeyCode::Char('l'), KeyModifiers::CONTROL);
        assert!(is_global_keybinding(&key));
    }

    #[test]
    fn tab_is_global() {
        let key = key_event(KeyCode::Tab, KeyModifiers::NONE);
        assert!(is_global_keybinding(&key));
    }

    #[test]
    fn plain_char_is_not_global() {
        let key = key_event(KeyCode::Char('a'), KeyModifiers::NONE);
        assert!(!is_global_keybinding(&key));
    }

    #[test]
    fn shift_tab_is_not_global() {
        let key = key_event(KeyCode::Tab, KeyModifiers::SHIFT);
        assert!(!is_global_keybinding(&key));
    }

    #[test]
    fn ctrl_a_is_not_global() {
        let key = key_event(KeyCode::Char('a'), KeyModifiers::CONTROL);
        assert!(!is_global_keybinding(&key));
    }

    #[test]
    fn enter_is_not_global() {
        let key = key_event(KeyCode::Enter, KeyModifiers::NONE);
        assert!(!is_global_keybinding(&key));
    }

    #[test]
    fn escape_is_not_global() {
        let key = key_event(KeyCode::Esc, KeyModifiers::NONE);
        assert!(!is_global_keybinding(&key));
    }
}
