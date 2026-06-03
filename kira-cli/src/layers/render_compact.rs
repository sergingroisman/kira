//! Compact-mode renderer — single line/column for constrained viewports.
//!
//! - Vertical compact (width < 30, height >= 12): single column of P/R/S/E with status icons, one per row.
//! - Horizontal compact (height < 12): single row of P→R→S→E with status icons separated by →.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::Frame;

use super::indicators::{indicator_for, layer_color};
use super::state::{ConceptualLayer, LayersState};

/// Layer labels used in compact mode (single character per layer).
const LAYER_LABELS: [(ConceptualLayer, &str); 4] = [
    (ConceptualLayer::Perception, "P"),
    (ConceptualLayer::Reasoning, "R"),
    (ConceptualLayer::Security, "S"),
    (ConceptualLayer::Execution, "E"),
];

/// Render the Layers View in compact mode.
///
/// When `vertical` is true: render one layer per row (P, R, S, E) with status icon.
/// When `vertical` is false: render all 4 on a single row separated by → arrows.
///
/// No flow arrows, throughput indicators, or duration counters are shown in compact mode.
pub fn render_compact(
    frame: &mut Frame,
    state: &LayersState,
    area: Rect,
    spinner_frame: u8,
    vertical: bool,
) {
    if vertical {
        render_vertical(frame, state, area, spinner_frame);
    } else {
        render_horizontal(frame, state, area, spinner_frame);
    }
}

/// Vertical compact: one layer per row, each showing "L indicator" where L is the label.
fn render_vertical(frame: &mut Frame, state: &LayersState, area: Rect, spinner_frame: u8) {
    for (i, &(layer, label)) in LAYER_LABELS.iter().enumerate() {
        // LAYER_LABELS has exactly 4 entries, so i is always 0..3 — safe truncation.
        #[allow(clippy::cast_possible_truncation)]
        let row = i as u16;
        if row >= area.height {
            break;
        }

        let layer_state = &state.layers[layer as usize];
        let indicator = indicator_for(layer_state.status, spinner_frame);
        let color = layer_color(layer);

        let line = Line::from(vec![
            Span::styled(label, Style::default().fg(color)),
            Span::raw(" "),
            Span::styled(indicator.symbol, indicator.style),
        ]);

        let row_rect = Rect::new(area.x, area.y + row, area.width, 1);
        frame.render_widget(line, row_rect);
    }
}

/// Horizontal compact: single row showing "P● → R● → S● → E●" with status icons.
fn render_horizontal(frame: &mut Frame, state: &LayersState, area: Rect, spinner_frame: u8) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let mut spans: Vec<Span<'_>> = Vec::with_capacity(4 * 3 + 3 * 3);

    for (i, &(layer, label)) in LAYER_LABELS.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" → "));
        }

        let layer_state = &state.layers[layer as usize];
        let indicator = indicator_for(layer_state.status, spinner_frame);
        let color = layer_color(layer);

        spans.push(Span::styled(label, Style::default().fg(color)));
        spans.push(Span::styled(indicator.symbol, indicator.style));
    }

    let line = Line::from(spans);
    let row_rect = Rect::new(area.x, area.y, area.width, 1);
    frame.render_widget(line, row_rect);
}
