#![forbid(unsafe_code)]

//! Keybinding registry and keymap lookup.
//!
//! Provides the `KeymapRegistry` for storing, querying, and discovering
//! keybindings. Used by the which-key panel to show continuations and
//! by the help overlay to list all bindings.

use crossterm::event::KeyEvent;

/// The context in which a keybinding is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyContext {
    /// Bindings available everywhere (e.g., Ctrl+C, Ctrl+P, Tab).
    Global,
    /// Bindings active when the chat panel has focus.
    ChatPanel,
    /// Bindings active when the input area has focus.
    InputArea,
    /// Bindings active when the sidebar has focus.
    Sidebar,
    /// Bindings active inside any overlay.
    Overlay,
}

/// A single keybinding definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBinding {
    /// The key sequence that triggers this binding (supports multi-key chords).
    pub keys: Vec<KeyEvent>,
    /// The command identifier (e.g., "toggle_sidebar", "open_palette").
    pub command: String,
    /// Human-readable description shown in help and which-key panel.
    pub description: String,
    /// The context in which this binding is active.
    pub context: KeyContext,
}

/// Registry of all keybindings, supporting prefix lookup for which-key
/// and context-filtered queries for the help overlay.
#[derive(Debug, Clone, Default)]
pub struct KeymapRegistry {
    /// All registered keybindings.
    bindings: Vec<KeyBinding>,
}

impl KeymapRegistry {
    /// Create a new empty registry.
    pub const fn new() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }

    /// Register a keybinding.
    pub fn register(&mut self, binding: KeyBinding) {
        self.bindings.push(binding);
    }

    /// Look up bindings that match the given key prefix within a context.
    ///
    /// Returns all bindings whose key sequence starts with `prefix` and
    /// are active in the given context (or Global context).
    pub fn lookup(&self, prefix: &[KeyEvent], context: KeyContext) -> Vec<&KeyBinding> {
        self.bindings
            .iter()
            .filter(|b| {
                (b.context == context || b.context == KeyContext::Global)
                    && b.keys.len() >= prefix.len()
                    && b.keys[..prefix.len()] == *prefix
            })
            .collect()
    }

    /// Check if the given key sequence is a prefix of any registered binding.
    ///
    /// Used to determine whether to start the which-key delay timer.
    pub fn is_prefix(&self, keys: &[KeyEvent]) -> bool {
        self.bindings
            .iter()
            .any(|b| b.keys.len() > keys.len() && b.keys[..keys.len()] == *keys)
    }

    /// Return all bindings active in the given context.
    ///
    /// Used by the help overlay to display context-grouped keybindings.
    pub fn all_for_context(&self, context: KeyContext) -> Vec<&KeyBinding> {
        self.bindings
            .iter()
            .filter(|b| b.context == context)
            .collect()
    }
}

/// Creates a `KeymapRegistry` pre-populated with all default keybindings.
///
/// This is the single source of truth for keybinding definitions. Used by:
/// - The input router for key dispatch
/// - The which-key panel for showing continuations
/// - The help overlay for listing all bindings
pub fn default_keybindings() -> KeymapRegistry {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut registry = KeymapRegistry::new();

    // ── Global keybindings ──
    registry.register(KeyBinding {
        keys: vec![KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)],
        command: "quit".into(),
        description: "Quit Kira".into(),
        context: KeyContext::Global,
    });
    registry.register(KeyBinding {
        keys: vec![KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL)],
        command: "open_palette".into(),
        description: "Open command palette".into(),
        context: KeyContext::Global,
    });
    registry.register(KeyBinding {
        keys: vec![KeyEvent::new(KeyCode::Char('m'), KeyModifiers::CONTROL)],
        command: "toggle_mode".into(),
        description: "Toggle PLAN/YOLO mode".into(),
        context: KeyContext::Global,
    });
    registry.register(KeyBinding {
        keys: vec![KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)],
        command: "toggle_sidebar".into(),
        description: "Toggle sidebar".into(),
        context: KeyContext::Global,
    });
    registry.register(KeyBinding {
        keys: vec![KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE)],
        command: "help".into(),
        description: "Show help overlay".into(),
        context: KeyContext::Global,
    });

    // ── ChatPanel keybindings ──
    registry.register(KeyBinding {
        keys: vec![KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE)],
        command: "scroll_down".into(),
        description: "Scroll down".into(),
        context: KeyContext::ChatPanel,
    });
    registry.register(KeyBinding {
        keys: vec![KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE)],
        command: "scroll_up".into(),
        description: "Scroll up".into(),
        context: KeyContext::ChatPanel,
    });
    registry.register(KeyBinding {
        keys: vec![KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)],
        command: "scroll_down".into(),
        description: "Scroll down".into(),
        context: KeyContext::ChatPanel,
    });
    registry.register(KeyBinding {
        keys: vec![KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)],
        command: "scroll_up".into(),
        description: "Scroll up".into(),
        context: KeyContext::ChatPanel,
    });
    registry.register(KeyBinding {
        keys: vec![
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
        ],
        command: "go_top".into(),
        description: "Go to top".into(),
        context: KeyContext::ChatPanel,
    });
    registry.register(KeyBinding {
        keys: vec![KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT)],
        command: "go_bottom".into(),
        description: "Go to bottom".into(),
        context: KeyContext::ChatPanel,
    });
    registry.register(KeyBinding {
        keys: vec![KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE)],
        command: "page_up".into(),
        description: "Page up".into(),
        context: KeyContext::ChatPanel,
    });
    registry.register(KeyBinding {
        keys: vec![KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE)],
        command: "page_down".into(),
        description: "Page down".into(),
        context: KeyContext::ChatPanel,
    });

    // ── InputArea keybindings ──
    registry.register(KeyBinding {
        keys: vec![KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)],
        command: "submit".into(),
        description: "Submit message".into(),
        context: KeyContext::InputArea,
    });
    registry.register(KeyBinding {
        keys: vec![KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT)],
        command: "newline".into(),
        description: "Insert newline".into(),
        context: KeyContext::InputArea,
    });
    registry.register(KeyBinding {
        keys: vec![KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)],
        command: "history_prev".into(),
        description: "Previous history entry".into(),
        context: KeyContext::InputArea,
    });
    registry.register(KeyBinding {
        keys: vec![KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)],
        command: "history_next".into(),
        description: "Next history entry".into(),
        context: KeyContext::InputArea,
    });

    // ── Overlay keybindings ──
    registry.register(KeyBinding {
        keys: vec![KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)],
        command: "close".into(),
        description: "Close overlay".into(),
        context: KeyContext::Overlay,
    });
    registry.register(KeyBinding {
        keys: vec![KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)],
        command: "select".into(),
        description: "Select item".into(),
        context: KeyContext::Overlay,
    });
    registry.register(KeyBinding {
        keys: vec![KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)],
        command: "navigate_up".into(),
        description: "Navigate up".into(),
        context: KeyContext::Overlay,
    });
    registry.register(KeyBinding {
        keys: vec![KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)],
        command: "navigate_down".into(),
        description: "Navigate down".into(),
        context: KeyContext::Overlay,
    });

    registry
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};

    fn key_event(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl_key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    #[test]
    fn lookup_returns_matching_bindings() {
        let mut registry = KeymapRegistry::new();
        registry.register(KeyBinding {
            keys: vec![ctrl_key(KeyCode::Char('p'))],
            command: "open_palette".into(),
            description: "Open command palette".into(),
            context: KeyContext::Global,
        });
        registry.register(KeyBinding {
            keys: vec![key_event(KeyCode::Char('j'))],
            command: "scroll_down".into(),
            description: "Scroll down".into(),
            context: KeyContext::ChatPanel,
        });

        let results = registry.lookup(&[ctrl_key(KeyCode::Char('p'))], KeyContext::Global);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].command, "open_palette");
    }

    #[test]
    fn lookup_includes_global_bindings_in_any_context() {
        let mut registry = KeymapRegistry::new();
        registry.register(KeyBinding {
            keys: vec![ctrl_key(KeyCode::Char('c'))],
            command: "quit".into(),
            description: "Quit".into(),
            context: KeyContext::Global,
        });

        let results = registry.lookup(&[ctrl_key(KeyCode::Char('c'))], KeyContext::ChatPanel);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].command, "quit");
    }

    #[test]
    fn is_prefix_detects_multi_key_sequences() {
        let mut registry = KeymapRegistry::new();
        registry.register(KeyBinding {
            keys: vec![key_event(KeyCode::Char('g')), key_event(KeyCode::Char('g'))],
            command: "go_top".into(),
            description: "Go to top".into(),
            context: KeyContext::ChatPanel,
        });

        assert!(registry.is_prefix(&[key_event(KeyCode::Char('g'))]));
        assert!(!registry.is_prefix(&[key_event(KeyCode::Char('x'))]));
    }

    #[test]
    fn all_for_context_filters_correctly() {
        let mut registry = KeymapRegistry::new();
        registry.register(KeyBinding {
            keys: vec![ctrl_key(KeyCode::Char('c'))],
            command: "quit".into(),
            description: "Quit".into(),
            context: KeyContext::Global,
        });
        registry.register(KeyBinding {
            keys: vec![key_event(KeyCode::Char('j'))],
            command: "scroll_down".into(),
            description: "Scroll down".into(),
            context: KeyContext::ChatPanel,
        });

        let global = registry.all_for_context(KeyContext::Global);
        assert_eq!(global.len(), 1);
        assert_eq!(global[0].command, "quit");

        let chat = registry.all_for_context(KeyContext::ChatPanel);
        assert_eq!(chat.len(), 1);
        assert_eq!(chat[0].command, "scroll_down");
    }

    #[test]
    fn default_keybindings_has_expected_binding_counts() {
        let registry = default_keybindings();

        // Global: quit, open_palette, toggle_mode, toggle_sidebar, help = 5
        let global = registry.all_for_context(KeyContext::Global);
        assert_eq!(global.len(), 5);

        // ChatPanel: j, k, Down, Up, g+g, G, PageUp, PageDown = 8
        let chat = registry.all_for_context(KeyContext::ChatPanel);
        assert_eq!(chat.len(), 8);

        // InputArea: Enter (submit), Shift+Enter (newline), Up (history_prev), Down (history_next) = 4
        let input = registry.all_for_context(KeyContext::InputArea);
        assert_eq!(input.len(), 4);

        // Sidebar: none = 0
        let sidebar = registry.all_for_context(KeyContext::Sidebar);
        assert_eq!(sidebar.len(), 0);

        // Overlay: Escape (close), Enter (select), Up (navigate_up), Down (navigate_down) = 4
        let overlay = registry.all_for_context(KeyContext::Overlay);
        assert_eq!(overlay.len(), 4);

        // Total: 5 + 8 + 4 + 0 + 4 = 21
        let total = global.len() + chat.len() + input.len() + sidebar.len() + overlay.len();
        assert_eq!(total, 21);
    }

    #[test]
    fn default_keybindings_gg_is_prefix() {
        let registry = default_keybindings();
        // 'g' alone should be a prefix (for the g+g binding)
        assert!(registry.is_prefix(&[key_event(KeyCode::Char('g'))]));
    }

    #[test]
    fn default_keybindings_lookup_global_in_chat_context() {
        let registry = default_keybindings();
        // Global bindings should be visible from ChatPanel context
        let results = registry.lookup(&[ctrl_key(KeyCode::Char('c'))], KeyContext::ChatPanel);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].command, "quit");
    }
}
