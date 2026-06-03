#![forbid(unsafe_code)]

//! Top-level UI module — dispatches the 3-layer render pipeline.
//!
//! The render pipeline executes in order:
//! 1. **Layout Engine**: Compute zones from terminal area + state
//! 2. **Base layer**: header, sidebar, chat panel, input area, footer
//! 3. **Overlay layer**: command palette, session picker, task browser, help, approval
//! 4. **HUD layer**: toasts and which-key panel

pub mod chat;
pub mod footer;
pub mod header;
pub mod hud;
pub mod input;
pub mod layout;
pub mod overlays;
pub mod sidebar;
pub mod splash;
pub mod theme;

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::AppState;
use crate::layers::render::{determine_mode, determine_placement, Placement, RenderMode};
use crate::layers::render_compact::render_compact;

/// Top-level render — called every tick from the event loop.
///
/// Implements the 3-layer pipeline:
/// 0. Splash screen (when still connecting — short-circuits the rest)
/// 1. Layout engine computes zones
/// 2. Base layer renders header, sidebar, chat, input, footer
/// 3. Overlay layer renders top-of-stack overlay (if any)
/// 4. HUD layer renders toasts and which-key panel
pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();

    // 0. Splash screen — shown until Core connects or first keypress.
    if state.splash_visible {
        splash::render_splash(frame, state);
        return;
    }

    // Determine sidebar visibility from state:
    // Show sidebar if manually toggled OR if terminal is wide enough (auto-threshold).
    let sidebar_visible = state.sidebar_visible
        && (state.sidebar_manual_toggle || area.width > state.config.ui.sidebar_auto_threshold);

    // 1. Layout Engine — compute zones
    let zones = layout::compute_layout(area, sidebar_visible, state.input_height);

    // 2. Terminal too small — render single-line message and bail
    if zones.terminal_too_small {
        render_terminal_too_small(frame, area);
        return;
    }

    // 3. Base Layer — render all base panels using computed zones
    header::render_header(frame, state, zones.header);

    if let Some(sidebar_area) = zones.sidebar {
        sidebar::render_sidebar(frame, state, sidebar_area);
    }

    chat::render_chat(frame, state, zones.chat);
    input::render_input(frame, state, zones.input);
    footer::render_footer(frame, state, zones.footer);

    // 3.5. Layers View — renders between base layer and overlays.
    //       Suppressed when any focus-capturing overlay is active.
    if state.layers_visible && state.overlay_stack.is_empty() {
        render_layers_view(frame, state, area);
    }

    // 4. Overlay Layer — render top-of-stack overlay (if any)
    overlays::render_overlay(frame, state, area);

    // 5. HUD Layer — toasts and which-key panel (always on top)
    hud::toast::render_toasts(frame, state, area);

    if let Some(wk_state) = &state.which_key_state {
        hud::which_key::render_which_key(frame, wk_state, area);
    }
}

/// Render a single-line "Terminal too small" message centered in the area.
///
/// Displayed when the terminal dimensions are below the minimum threshold
/// (width < 60 or height < 8).
fn render_terminal_too_small(frame: &mut Frame, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let msg = "Terminal too small (min 60×8)";
    let line = Line::from(vec![Span::styled(
        msg,
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )]);

    // Center vertically: pick the middle row
    let y = area.y.saturating_add(area.height / 2);
    let msg_area = Rect::new(area.x, y, area.width, 1);

    let paragraph = Paragraph::new(line).alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(paragraph, msg_area);
}

/// Render the Layers View based on placement and render mode.
///
/// Called between the base layer and overlay layer in the render pipeline.
/// Determines placement (sidebar section vs floating panel) and render mode
/// (full, vertical compact, horizontal compact), then delegates to the
/// appropriate renderer.
fn render_layers_view(frame: &mut Frame, state: &AppState, terminal_area: Rect) {
    let placement = determine_placement(state.sidebar_visible, terminal_area.width);

    match placement {
        Placement::SidebarSection => {
            // Sidebar section rendering is handled by task 11.2 in sidebar.rs.
            // When placement is SidebarSection, the sidebar module calls
            // render_layers_section() directly. Nothing to do here.
        }
        Placement::FloatingPanel => {
            // Determine render mode from the floating panel area.
            let panel_rect =
                crate::layers::render::floating_panel_rect(terminal_area);
            let mode = determine_mode(panel_rect);

            match mode {
                RenderMode::Full => {
                    crate::layers::render::render_layers_float(
                        frame,
                        &state.layers_state,
                        &state.layers_config,
                        terminal_area,
                        state.spinner_frame,
                    );
                }
                RenderMode::VerticalCompact => {
                    // Clear and render compact in the floating panel area.
                    frame.render_widget(ratatui::widgets::Clear, panel_rect);
                    render_compact(
                        frame,
                        &state.layers_state,
                        panel_rect,
                        state.spinner_frame,
                        true,
                    );
                }
                RenderMode::HorizontalCompact => {
                    // Clear and render compact in the floating panel area.
                    frame.render_widget(ratatui::widgets::Clear, panel_rect);
                    render_compact(
                        frame,
                        &state.layers_state,
                        panel_rect,
                        state.spinner_frame,
                        false,
                    );
                }
            }
        }
    }
}
