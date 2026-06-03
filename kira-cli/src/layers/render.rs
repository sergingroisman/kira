#![allow(unused)]
//! Full-mode renderer — 4 layer boxes, flow arrows, detail panel.

use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Widget};
use ratatui::Frame;

use super::config::LayersConfig;
use super::indicators::{indicator_for, layer_color};
use super::state::{ConceptualLayer, FlowState, LayerDetail, LayerStatus, LayersState};

// ---------------------------------------------------------------------------
// Render Mode
// ---------------------------------------------------------------------------

/// Determine the render mode based on available dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    /// Full mode: 4 boxes (3 rows each) + 3 flow arrows (1 row each) = 15 rows minimum.
    Full,
    /// Vertical compact: single column of 4 status icons (width < 30, height >= 12).
    VerticalCompact,
    /// Horizontal compact: single row of 4 status icons (height < 12).
    HorizontalCompact,
}

/// Determine render mode from available area.
///
/// - Full if width >= 30 AND height >= 12
/// - VerticalCompact if width < 30 AND height >= 12
/// - HorizontalCompact if height < 12 (regardless of width)
pub const fn determine_mode(area: Rect) -> RenderMode {
    if area.width >= 30 && area.height >= 12 {
        RenderMode::Full
    } else if area.height >= 12 {
        RenderMode::VerticalCompact
    } else {
        RenderMode::HorizontalCompact
    }
}

// ---------------------------------------------------------------------------
// Placement
// ---------------------------------------------------------------------------

/// Placement of the Layers View in the TUI layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// Rendered as a section within the sidebar (max 12 rows).
    SidebarSection,
    /// Rendered as a floating panel (36 cols, 60% height, top-right).
    FloatingPanel,
}

/// Determine placement based on sidebar visibility and terminal width.
///
/// Returns `SidebarSection` if sidebar is visible AND terminal width > 160;
/// otherwise returns `FloatingPanel`.
pub const fn determine_placement(sidebar_visible: bool, terminal_width: u16) -> Placement {
    if sidebar_visible && terminal_width > 160 {
        Placement::SidebarSection
    } else {
        Placement::FloatingPanel
    }
}

// ---------------------------------------------------------------------------
// Floating Panel Rect
// ---------------------------------------------------------------------------

/// Compute the floating panel Rect (36 cols, 60% height, top-right, 1-row margin from header).
///
/// - Width: 36 columns (clamped to terminal width)
/// - Height: 60% of terminal height (minimum 6 rows, clamped to terminal height - 2)
/// - Position: top-right corner, 1-row margin from header (y=1)
pub fn floating_panel_rect(terminal: Rect) -> Rect {
    let width = 36u16.min(terminal.width);
    let height = {
        let sixty_pct = u16::try_from(u32::from(terminal.height) * 60 / 100).unwrap_or(u16::MAX);
        let min_height = 6u16;
        let max_height = terminal.height.saturating_sub(2);
        sixty_pct.max(min_height).min(max_height)
    };
    let x = terminal.width.saturating_sub(width);
    let y = 1; // 1-row margin from header
    Rect::new(x, y, width, height)
}

// ---------------------------------------------------------------------------
// Detail Panel Height
// ---------------------------------------------------------------------------

/// Compute the detail panel height (capped at 50% of available height).
pub const fn detail_panel_height(available_height: u16) -> u16 {
    available_height / 2
}

// ---------------------------------------------------------------------------
// Animation Rate
// ---------------------------------------------------------------------------

/// Compute animation cycle rate (cycles per second) based on throughput.
///
/// Linear interpolation:
/// - throughput 0: 0.0 (no animation, dim)
/// - throughput 1: 1.0 cycle/sec
/// - throughput >= 10: 4.0 cycles/sec (capped)
/// - throughput 1–10: linear interpolation between 1.0 and 4.0
#[allow(clippy::cast_precision_loss)]
pub fn animation_rate_cps(throughput: usize) -> f32 {
    if throughput == 0 {
        return 0.0;
    }
    let t = (throughput as f32).min(10.0);
    // Linear: 1.0 at t=1, 4.0 at t=10 → slope = 3.0/9.0 = 1/3
    (t - 1.0).mul_add(3.0 / 9.0, 1.0)
}

// ---------------------------------------------------------------------------
// Flow Staleness
// ---------------------------------------------------------------------------

/// Determine if a flow arrow should render as dim (stale).
///
/// A flow arrow is stale if no flow event has occurred in the last 5 seconds,
/// or if no flow event has ever occurred (last_flow_at is None).
pub fn is_flow_stale(flow: &FlowState, now: Instant) -> bool {
    flow.last_flow_at
        .is_none_or(|last| now.duration_since(last).as_secs() > 5)
}

// ---------------------------------------------------------------------------
// Layer Labels
// ---------------------------------------------------------------------------

/// Human-readable labels for each conceptual layer.
const LAYER_LABELS: [&str; 4] = ["Perception", "Reasoning", "Security", "Execution"];

/// The ordered list of conceptual layers (top-to-bottom).
const LAYERS_ORDERED: [ConceptualLayer; 4] = [
    ConceptualLayer::Perception,
    ConceptualLayer::Reasoning,
    ConceptualLayer::Security,
    ConceptualLayer::Execution,
];

// ---------------------------------------------------------------------------
// Full Mode Renderer
// ---------------------------------------------------------------------------

/// Render the full Layers View (4 boxes + flow arrows + return arrow + detail panel).
///
/// Layout (15 rows total for the stack):
/// - 4 layer boxes × 3 rows each = 12 rows
/// - 3 flow arrows × 1 row each = 3 rows
/// - Total: 15 rows
///
/// When `state.detail_expanded` is `Some(idx)`, the area is split:
/// - Top 50%: compressed layer stack
/// - Bottom 50%: detail panel for the selected layer
///
/// If the area is smaller than 15 rows, layers/arrows are truncated gracefully.
/// The rightmost column is reserved for the return arrow (▲) from Execution to Perception.
pub fn render_layers_full(
    frame: &mut Frame,
    state: &LayersState,
    config: &LayersConfig,
    area: Rect,
    spinner_frame: u8,
) {
    if area.width < 3 || area.height < 1 {
        return;
    }

    let now = Instant::now();

    // Determine if we need to split for detail panel.
    let (stack_area, detail_area) = state.detail_expanded.map_or((area, None), |idx| {
        if idx < 4 {
            let detail_h = detail_panel_height(area.height);
            let stack_h = area.height.saturating_sub(detail_h);
            let stack = Rect::new(area.x, area.y, area.width, stack_h);
            let detail = Rect::new(area.x, area.y + stack_h, area.width, detail_h);
            (stack, Some((detail, idx)))
        } else {
            (area, None)
        }
    });

    // Render the layer stack in the stack_area.
    render_layer_stack(frame, state, config, stack_area, spinner_frame, now);

    // Render the detail panel if expanded.
    if let Some((detail_rect, idx)) = detail_area {
        render_detail_panel(frame, state, detail_rect, idx);
    }
}

/// Render the layer stack (4 boxes + flow arrows + return arrow) within the given area.
fn render_layer_stack(
    frame: &mut Frame,
    state: &LayersState,
    config: &LayersConfig,
    area: Rect,
    spinner_frame: u8,
    now: Instant,
) {
    if area.width < 3 || area.height < 1 {
        return;
    }

    // Reserve 1 column on the right for the return arrow, and 1 row at top for header.
    let header_height: u16 = 1;
    let return_arrow_col = area.width.saturating_sub(1);

    // Render panel header (row 0 of area).
    render_panel_header(frame, state, area);

    // Content area starts after header.
    let content_y = area.y + header_height;
    let content_height = area.height.saturating_sub(header_height);
    let content_width = area.width.saturating_sub(2); // leave space for return arrow

    if content_height == 0 || content_width < 3 {
        return;
    }

    // Track the y positions of each layer box (for return arrow rendering).
    let mut layer_y_positions: [u16; 4] = [0; 4];
    let mut current_y = content_y;

    for (i, &layer) in LAYERS_ORDERED.iter().enumerate() {
        // Check if we have room for this layer box (3 rows).
        if current_y + 3 > area.y + area.height {
            break;
        }

        layer_y_positions[i] = current_y;

        let layer_state = &state.layers[i];
        let threshold = config.threshold_for(layer);
        let duration = compute_display_duration(layer_state, now);
        let is_bottleneck = layer_state.status == LayerStatus::Active && duration >= threshold;

        // Render the 3-row layer box.
        render_layer_box(
            frame,
            area.x,
            current_y,
            content_width,
            layer,
            layer_state.status,
            duration,
            is_bottleneck,
            spinner_frame,
        );

        current_y += 3;

        // Render flow arrow between this layer and the next (except after the last).
        if i < 3 {
            if current_y >= area.y + area.height {
                break;
            }

            let flow = &state.flows[i];
            let throughput = state.throughput(i);
            let is_stale = is_flow_stale(flow, now);
            let flow_bottleneck = is_flow_bottleneck(flow, state, now);

            render_flow_arrow(
                frame,
                area.x,
                current_y,
                content_width,
                throughput,
                is_stale,
                flow_bottleneck,
            );

            current_y += 1;
        }
    }

    // Render return arrow (▲) in the rightmost column from Execution back to Perception.
    render_return_arrow(
        frame,
        area.x + return_arrow_col,
        layer_y_positions[0], // Perception y
        layer_y_positions[3], // Execution y
        area.y + area.height,
    );
}

// ---------------------------------------------------------------------------
// Panel Header
// ---------------------------------------------------------------------------

/// Render the panel header row with title and optional disconnected indicator.
fn render_panel_header(frame: &mut Frame, state: &LayersState, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let buf = frame.buffer_mut();
    let y = area.y;

    let title = if state.disconnected {
        "⚡ Layers [DISCONNECTED]"
    } else {
        "⚡ Layers"
    };

    let title_style = if state.disconnected {
        Style::default()
            .fg(Color::Red)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    };

    let max_len = area.width as usize;
    let display_title: String = title.chars().take(max_len).collect();

    for (i, ch) in display_title.chars().enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        let x = area.x + (i as u16);
        if x < area.x + area.width {
            buf[(x, y)].set_char(ch).set_style(title_style);
        }
    }
}

// ---------------------------------------------------------------------------
// Layer Box Rendering
// ---------------------------------------------------------------------------

/// Render a single layer box (3 rows: top border, content, bottom border).
///
/// Box-drawing characters: ┌─┐ (top), │ │ (sides), └─┘ (bottom).
/// Content line: │ <indicator> <label> <duration> │
#[allow(clippy::too_many_arguments)]
fn render_layer_box(
    frame: &mut Frame,
    x: u16,
    y: u16,
    width: u16,
    layer: ConceptualLayer,
    status: LayerStatus,
    duration: u64,
    is_bottleneck: bool,
    spinner_frame: u8,
) {
    let buf = frame.buffer_mut();

    if width < 3 {
        return;
    }

    let border_style = if is_bottleneck {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    // Top border: ┌───...───┐
    render_horizontal_border(buf, x, y, width, '┌', '┐', '─', border_style);

    // Content line: │ <indicator> <label> <duration> │
    let content_y = y + 1;
    // Left border
    buf[(x, content_y)].set_char('│').set_style(border_style);
    // Right border
    if width > 1 {
        buf[(x + width - 1, content_y)]
            .set_char('│')
            .set_style(border_style);
    }

    // Content area (between borders)
    let inner_x = x + 1;
    let inner_width = width.saturating_sub(2) as usize;

    if inner_width > 0 {
        let indicator = indicator_for(status, spinner_frame);
        let label = LAYER_LABELS[layer as usize];
        let label_color = layer_color(layer);
        let duration_str = format!("{duration}s");

        // Clear the inner area first
        for i in 0..inner_width {
            #[allow(clippy::cast_possible_truncation)]
            let cx = inner_x + (i as u16);
            buf[(cx, content_y)]
                .set_char(' ')
                .set_style(Style::default());
        }

        // Write indicator symbol
        let mut col = inner_x;
        for ch in indicator.symbol.chars() {
            if (col - inner_x) as usize >= inner_width {
                break;
            }
            buf[(col, content_y)]
                .set_char(ch)
                .set_style(indicator.style);
            col += 1;
        }

        // Space after indicator
        if ((col - inner_x) as usize) < inner_width {
            col += 1;
        }

        // Write label
        let label_style = Style::default().fg(label_color);
        for ch in label.chars() {
            if (col - inner_x) as usize >= inner_width {
                break;
            }
            buf[(col, content_y)]
                .set_char(ch)
                .set_style(label_style);
            col += 1;
        }

        // Write duration right-aligned (if there's room)
        let dur_len = duration_str.len();
        if inner_width > dur_len + 1 {
            #[allow(clippy::cast_possible_truncation)]
            let dur_x = inner_x + ((inner_width - dur_len) as u16);
            let dur_style = if is_bottleneck {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            for (i, ch) in duration_str.chars().enumerate() {
                #[allow(clippy::cast_possible_truncation)]
                let offset = i as u16;
                buf[(dur_x + offset, content_y)]
                    .set_char(ch)
                    .set_style(dur_style);
            }
        }
    }

    // Bottom border: └───...───┘
    render_horizontal_border(buf, x, y + 2, width, '└', '┘', '─', border_style);
}

/// Render a horizontal border line with given corner and fill characters.
fn render_horizontal_border(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    width: u16,
    left_corner: char,
    right_corner: char,
    fill: char,
    style: Style,
) {
    if width == 0 {
        return;
    }
    buf[(x, y)].set_char(left_corner).set_style(style);
    for i in 1..width.saturating_sub(1) {
        buf[(x + i, y)].set_char(fill).set_style(style);
    }
    if width > 1 {
        buf[(x + width - 1, y)]
            .set_char(right_corner)
            .set_style(style);
    }
}

// ---------------------------------------------------------------------------
// Flow Arrow Rendering
// ---------------------------------------------------------------------------

/// Render a flow arrow row between two layer boxes.
///
/// Shows: centered ▼ with throughput count to the right.
/// Applies dim styling if stale (>5s no flow).
/// Applies yellow background if bottleneck (>10s zero throughput while active).
fn render_flow_arrow(
    frame: &mut Frame,
    x: u16,
    y: u16,
    width: u16,
    throughput: usize,
    stale: bool,
    bottleneck: bool,
) {
    let buf = frame.buffer_mut();

    if width < 3 {
        return;
    }

    let arrow_style = if bottleneck {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else if stale {
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM)
    } else {
        Style::default().fg(Color::Green)
    };

    // Clear the row
    for i in 0..width {
        buf[(x + i, y)]
            .set_char(' ')
            .set_style(Style::default());
    }

    // Center the arrow character
    let center = width / 2;
    buf[(x + center, y)]
        .set_char('▼')
        .set_style(arrow_style);

    // Throughput count to the right of the arrow
    let throughput_str = format!(" {throughput}");
    let throughput_style = if bottleneck {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else if stale {
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let start_col = x + center + 1;
    for (i, ch) in throughput_str.chars().enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        let col = start_col + (i as u16);
        if col < x + width {
            buf[(col, y)]
                .set_char(ch)
                .set_style(throughput_style);
        }
    }
}

// ---------------------------------------------------------------------------
// Return Arrow Rendering
// ---------------------------------------------------------------------------

/// Render the return arrow (▲) in the rightmost column from Execution back to Perception.
///
/// Draws vertical line characters (│) from Execution box top to Perception box bottom,
/// with ▲ at the top (Perception level).
fn render_return_arrow(
    frame: &mut Frame,
    x: u16,
    perception_y: u16,
    execution_y: u16,
    max_y: u16,
) {
    if perception_y == 0 && execution_y == 0 {
        return; // No layers rendered
    }
    if execution_y <= perception_y {
        return; // Not enough layers rendered
    }

    let buf = frame.buffer_mut();
    let style = Style::default().fg(Color::DarkGray);

    // Draw from perception box (middle row) down to execution box (middle row).
    let start_y = perception_y + 1; // Middle of perception box
    let end_y = (execution_y + 1).min(max_y); // Middle of execution box

    // Arrow head at the top (pointing up = return flow)
    if start_y < max_y {
        buf[(x, start_y)]
            .set_char('▲')
            .set_style(Style::default().fg(Color::Cyan));
    }

    // Vertical line from below the arrow head to the execution layer
    for row in (start_y + 1)..end_y {
        if row < max_y {
            buf[(x, row)].set_char('│').set_style(style);
        }
    }
}

// ---------------------------------------------------------------------------
// Helper Functions
// ---------------------------------------------------------------------------

/// Compute the display duration for a layer.
///
/// If the layer is active, compute elapsed from active_since.
/// Otherwise, use the last recorded duration.
fn compute_display_duration(
    layer_state: &super::state::SingleLayerState,
    now: Instant,
) -> u64 {
    if layer_state.status == LayerStatus::Active {
        layer_state
            .active_since
            .map_or(layer_state.last_duration_secs, |start| {
                now.duration_since(start).as_secs()
            })
    } else {
        layer_state.last_duration_secs
    }
}

/// Determine if a flow arrow should show bottleneck highlight.
///
/// A flow arrow is bottlenecked if:
/// - At least one layer is active (pipeline is running)
/// - The flow has had zero throughput for more than 10 seconds
/// - The flow's last_flow_at is either None or more than 10s ago
fn is_flow_bottleneck(flow: &FlowState, state: &LayersState, now: Instant) -> bool {
    // Check if any layer is active
    let any_active = state
        .layers
        .iter()
        .any(|l| l.status == LayerStatus::Active);

    if !any_active {
        return false;
    }

    // Check if throughput is zero and last flow was >10s ago
    let stalled = flow
        .last_flow_at
        .is_none_or(|last| now.duration_since(last).as_secs() > 10);

    stalled && flow.flow_times.is_empty()
}

// ---------------------------------------------------------------------------
// Detail Panel Rendering
// ---------------------------------------------------------------------------

/// Maximum number of events to display in the detail panel.
const DETAIL_MAX_EVENTS: usize = 20;

/// Render the detail panel for a specific layer.
///
/// Shows:
/// - Header with layer name
/// - Layer-specific metadata fields (with dash for missing values)
/// - Last 20 events (timestamp, phase, message)
fn render_detail_panel(frame: &mut Frame, state: &LayersState, area: Rect, layer_idx: usize) {
    if area.width < 5 || area.height < 3 {
        return;
    }

    let buf = frame.buffer_mut();

    // Draw a separator line at the top of the detail panel.
    let sep_style = Style::default().fg(Color::DarkGray);
    for col in area.x..area.x + area.width {
        buf[(col, area.y)].set_char('─').set_style(sep_style);
    }

    let content_y = area.y + 1;
    let content_height = area.height.saturating_sub(1);
    if content_height == 0 {
        return;
    }

    let layer = ConceptualLayer::from_index(layer_idx);
    let layer_state = &state.layers[layer_idx];
    let label = LAYER_LABELS[layer_idx];
    let color = layer_color(layer);

    // Row 0: Detail panel header.
    let header_style = Style::default().fg(color).add_modifier(Modifier::BOLD);
    let header_text = format!("▸ {label} Detail");
    render_text_line(buf, area.x, content_y, area.width, &header_text, header_style);

    let mut row = content_y + 1;
    let max_row = area.y + area.height;

    // Render layer-specific metadata.
    row = render_layer_metadata(buf, area.x, row, area.width, max_row, layer, layer_state);

    // Blank separator row between metadata and events.
    if row < max_row {
        row += 1;
    }

    // Render event list header.
    if row < max_row {
        let events_header = "Time     Phase    Message";
        let hdr_style = Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM);
        render_text_line(buf, area.x, row, area.width, events_header, hdr_style);
        row += 1;
    }

    // Render last 20 events (most recent at bottom).
    let events = &layer_state.events;
    let event_count = events.len().min(DETAIL_MAX_EVENTS);
    let available_rows = max_row.saturating_sub(row) as usize;
    let display_count = event_count.min(available_rows);

    // Show the most recent events that fit.
    let display_start = events.len().saturating_sub(display_count);
    for event in events.iter().skip(display_start) {
        if row >= max_row {
            break;
        }
        let line = format_event_line(event, area.width as usize);
        let evt_style = Style::default().fg(Color::White);
        render_text_line(buf, area.x, row, area.width, &line, evt_style);
        row += 1;
    }
}

/// Render layer-specific metadata fields. Returns the next available row.
fn render_layer_metadata(
    buf: &mut Buffer,
    x: u16,
    start_row: u16,
    width: u16,
    max_row: u16,
    layer: ConceptualLayer,
    layer_state: &super::state::SingleLayerState,
) -> u16 {
    let value_style = Style::default().fg(Color::White);
    let dash = "─";

    let mut row = start_row;

    match layer {
        ConceptualLayer::Perception => {
            let (mem_count, files_count, rag_query) = match &layer_state.detail {
                Some(LayerDetail::Perception {
                    memory_fragments_count,
                    files_read_count,
                    rag_query,
                }) => (
                    format!("{memory_fragments_count}"),
                    format!("{files_read_count}"),
                    rag_query
                        .as_deref()
                        .unwrap_or(dash)
                        .to_string(),
                ),
                _ => (dash.to_string(), dash.to_string(), dash.to_string()),
            };

            if row < max_row {
                let line = format!("Memory: {mem_count}  Files: {files_count}");
                render_text_line(buf, x, row, width, &line, value_style);
                row += 1;
            }
            if row < max_row {
                let rag_display: String = rag_query.chars().take(40).collect();
                let line = format!("RAG: {rag_display}");
                render_text_line(buf, x, row, width, &line, value_style);
                row += 1;
            }
        }
        ConceptualLayer::Reasoning => {
            let (sdd_phase, intent, model) = match &layer_state.detail {
                Some(LayerDetail::Reasoning {
                    sdd_phase,
                    last_intent,
                    active_model,
                }) => (
                    sdd_phase.as_deref().unwrap_or(dash).to_string(),
                    last_intent.as_deref().unwrap_or(dash).to_string(),
                    active_model.as_deref().unwrap_or(dash).to_string(),
                ),
                _ => (dash.to_string(), dash.to_string(), dash.to_string()),
            };

            if row < max_row {
                let line = format!("SDD: {sdd_phase}  Model: {model}");
                render_text_line(buf, x, row, width, &line, value_style);
                row += 1;
            }
            if row < max_row {
                let intent_display: String = intent.chars().take(60).collect();
                let line = format!("Intent: {intent_display}");
                render_text_line(buf, x, row, width, &line, value_style);
                row += 1;
            }
        }
        ConceptualLayer::Security => {
            let (reason, risk, hitl) = match &layer_state.detail {
                Some(LayerDetail::Security {
                    last_guardrail_reason,
                    current_risk,
                    hitl_status,
                }) => (
                    last_guardrail_reason
                        .as_deref()
                        .unwrap_or(dash)
                        .to_string(),
                    current_risk.as_deref().unwrap_or(dash).to_string(),
                    hitl_status.as_deref().unwrap_or(dash).to_string(),
                ),
                _ => (dash.to_string(), dash.to_string(), dash.to_string()),
            };

            if row < max_row {
                let line = format!("Risk: {risk}  HITL: {hitl}");
                render_text_line(buf, x, row, width, &line, value_style);
                row += 1;
            }
            if row < max_row {
                let reason_display: String = reason.chars().take(60).collect();
                let line = format!("Guardrail: {reason_display}");
                render_text_line(buf, x, row, width, &line, value_style);
                row += 1;
            }
        }
        ConceptualLayer::Execution => {
            let (tool, agent_id, elapsed) = match &layer_state.detail {
                Some(LayerDetail::Execution {
                    current_tool,
                    agent_short_id,
                    elapsed_secs,
                }) => (
                    current_tool.as_deref().unwrap_or(dash).to_string(),
                    agent_short_id.as_deref().unwrap_or(dash).to_string(),
                    elapsed_secs
                        .map_or_else(|| dash.to_string(), |s| format!("{s}s")),
                ),
                _ => (dash.to_string(), dash.to_string(), dash.to_string()),
            };

            if row < max_row {
                let tool_display: String = tool.chars().take(40).collect();
                let line = format!("Tool: {tool_display}  Agent: {agent_id}");
                render_text_line(buf, x, row, width, &line, value_style);
                row += 1;
            }
            if row < max_row {
                let line = format!("Elapsed: {elapsed}");
                render_text_line(buf, x, row, width, &line, value_style);
                row += 1;
            }
        }
    }

    row
}

/// Format a single event line for the detail panel.
fn format_event_line(event: &super::state::PipelineEvent, max_width: usize) -> String {
    // Format: "HH:MM:SS phase    message..."
    let phase_width = 8;
    let timestamp = &event.timestamp;
    let phase: String = event.phase.chars().take(phase_width).collect();
    let phase_padded = format!("{phase:<phase_width$}");

    // Remaining space for message (timestamp=8 + space + phase=8 + space = 18 chars overhead).
    let overhead = 8 + 1 + phase_width + 1;
    let msg_width = max_width.saturating_sub(overhead);
    let message: String = event.message.chars().take(msg_width).collect();

    format!("{timestamp} {phase_padded} {message}")
}

/// Render a single line of text into the buffer, truncating to width.
fn render_text_line(buf: &mut Buffer, x: u16, y: u16, width: u16, text: &str, style: Style) {
    let max_chars = width as usize;
    for (i, ch) in text.chars().take(max_chars).enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        let col = x + (i as u16);
        if col < x + width {
            buf[(col, y)].set_char(ch).set_style(style);
        }
    }
}

// ---------------------------------------------------------------------------
// Sidebar Section Renderer
// ---------------------------------------------------------------------------

/// Maximum rows for the sidebar section (including border).
const MAX_SECTION_ROWS: u16 = 12;

/// Render the Layers View as a sidebar section (constrained to max 12 rows).
///
/// Draws a bordered block with title "Layers", then delegates to
/// `render_layers_full()` within the inner area of the block.
pub fn render_layers_section(
    frame: &mut Frame,
    state: &LayersState,
    config: &LayersConfig,
    area: Rect,
    spinner_frame: u8,
) {
    // Constrain height to max 12 rows.
    let constrained_area = Rect {
        height: area.height.min(MAX_SECTION_ROWS),
        ..area
    };

    if constrained_area.width < 3 || constrained_area.height < 3 {
        return;
    }

    // Render the bordered block with "Layers" title.
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Layers ")
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(constrained_area);
    frame.render_widget(block, constrained_area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    // Delegate to the full renderer within the inner area.
    render_layers_full(frame, state, config, inner, spinner_frame);
}

// ---------------------------------------------------------------------------
// Floating Panel Renderer
// ---------------------------------------------------------------------------

/// Render the Layers View as a floating panel (with border and title "⚡ Layers").
///
/// - Computes the floating panel rect via `floating_panel_rect(terminal)`
/// - Clears the background area (z-order above base, below overlays)
/// - Renders a Block with borders and title "⚡ Layers"
/// - Delegates to `render_layers_full()` for the inner content
pub fn render_layers_float(
    frame: &mut Frame,
    state: &LayersState,
    config: &LayersConfig,
    terminal: Rect,
    spinner_frame: u8,
) {
    let panel_rect = floating_panel_rect(terminal);

    // Clear the background area before rendering (z-order above base, below overlays).
    frame.render_widget(Clear, panel_rect);

    // Build the block with border and title.
    let title = if state.disconnected {
        "⚡ Layers [DISCONNECTED]"
    } else {
        "⚡ Layers"
    };

    let title_style = if state.disconnected {
        Style::default()
            .fg(Color::Red)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Line::styled(title, title_style))
        .border_style(Style::default().fg(Color::DarkGray));

    // Compute the inner area (inside the block borders).
    let inner_area = block.inner(panel_rect);

    // Render the block border and title.
    frame.render_widget(block, panel_rect);

    // Render the layers content inside the block's inner area.
    render_layers_full(frame, state, config, inner_area, spinner_frame);
}
