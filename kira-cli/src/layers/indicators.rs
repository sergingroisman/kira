//! Layer indicator styling — spinner, dot, pause, cross per status.

use ratatui::style::{Color, Modifier, Style};

use super::state::{ConceptualLayer, LayerStatus};
use crate::ui::theme;

/// Visual indicator for a layer's current status.
pub struct LayerIndicator {
    /// The character(s) to display for this indicator.
    pub symbol: &'static str,
    /// The ratatui Style to apply when rendering.
    pub style: Style,
    /// Whether this indicator should animate (cycle frames).
    pub animated: bool,
}

/// Get the indicator for a given layer status and spinner frame index.
///
/// - Active → green bold spinner (animated)
/// - Idle → dim dot (static)
/// - Blocked → yellow pause (static)
/// - Errored → red bold cross (static)
pub fn indicator_for(status: LayerStatus, spinner_frame: u8) -> LayerIndicator {
    match status {
        LayerStatus::Active => LayerIndicator {
            symbol: theme::spinner_char(spinner_frame),
            style: Style::default()
                .fg(theme::SUCCESS)
                .add_modifier(Modifier::BOLD),
            animated: true,
        },
        LayerStatus::Idle => LayerIndicator {
            symbol: "●",
            style: Style::default()
                .fg(theme::MUTED)
                .add_modifier(Modifier::DIM),
            animated: false,
        },
        LayerStatus::Blocked => LayerIndicator {
            symbol: "⏸",
            style: Style::default().fg(theme::WARNING),
            animated: false,
        },
        LayerStatus::Errored => LayerIndicator {
            symbol: "✗",
            style: Style::default()
                .fg(theme::DANGER)
                .add_modifier(Modifier::BOLD),
            animated: false,
        },
    }
}

/// Layer label colors (distinct per layer for accessibility).
///
/// - Perception → Cyan
/// - Reasoning → Magenta
/// - Security → Yellow
/// - Execution → Green
pub const fn layer_color(layer: ConceptualLayer) -> Color {
    match layer {
        ConceptualLayer::Perception => theme::layer::PERCEPTION,
        ConceptualLayer::Reasoning => theme::layer::REASONING,
        ConceptualLayer::Security => theme::layer::SECURITY,
        ConceptualLayer::Execution => theme::layer::EXECUTION,
    }
}
