#![forbid(unsafe_code)]

//! Legacy approval module — thin re-export shim.
//!
//! This module is **deprecated**. All approval logic and rendering has been
//! migrated to [`crate::ui::overlays::approval`]. This shim re-exports the
//! public API for backward compatibility during the transition period.
//!
//! New code should import directly from `crate::ui::overlays::approval`.

// Re-export all public items from the new module location.
pub use crate::ui::overlays::approval::{
    ApprovalAction, check_approval_timeout, handle_approval_feedback_backspace,
    handle_approval_feedback_cancel, handle_approval_feedback_char,
    handle_approval_feedback_submit, handle_approval_key, render_approval, resolve_approval,
    ring_terminal_bell,
};

use ratatui::{Frame, layout::Rect};

use crate::app::AppState;

/// Render the approval modal as a centered overlay.
///
/// # Deprecated
///
/// Use [`crate::ui::overlays::approval::render_approval`] instead.
/// This function delegates to the new implementation.
#[deprecated(
    since = "0.5.0",
    note = "Use crate::ui::overlays::approval::render_approval instead"
)]
pub fn render_approval_modal(frame: &mut Frame, state: &AppState, area: Rect) {
    // Delegate to the new overlay renderer which handles its own centering
    // when called from the overlay stack. For backward compatibility, we
    // pass the full area and let render_approval handle the layout.
    render_approval(frame, state, area);
}
