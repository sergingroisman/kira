#![forbid(unsafe_code)]

//! Sidebar widget — renders session title, workspace info, budget, provider
//! status, SDD pipeline, waves progress, layers view, active agents, modified
//! files, and version footer in a 42-column panel.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::AppState;
use crate::layers::render::{Placement, determine_placement, render_layers_section};
use crate::messages::AgentThread;
use crate::models::{ConnectionStatus, ModifiedFile, SddPhase, WaveStatus};

// ── Constants ──────────────────────────────────────────────────────────────

const MAX_AGENTS: usize = 6;
const MAX_FILES: usize = 8;
const PATH_MAX_LEN: usize = 30;
/// Maximum rows allocated for the Layers View section in the sidebar.
const LAYERS_SECTION_MAX_ROWS: u16 = 12;

// ── Public API ─────────────────────────────────────────────────────────────

/// Render the sidebar into the given area.
pub fn render_sidebar(frame: &mut Frame, state: &AppState, area: Rect) {
    // Determine if the Layers View should render as a sidebar section.
    let show_layers_in_sidebar = state.layers_visible
        && determine_placement(state.sidebar_visible, frame.area().width) == Placement::SidebarSection;

    // Reserve space for the layers section at the bottom if needed.
    let (paragraph_area, layers_area) = if show_layers_in_sidebar {
        let layers_height = LAYERS_SECTION_MAX_ROWS.min(area.height.saturating_sub(6));
        if layers_height >= 3 {
            let para_height = area.height.saturating_sub(layers_height);
            let para = Rect::new(area.x, area.y, area.width, para_height);
            let layers = Rect::new(area.x, area.y + para_height, area.width, layers_height);
            (para, Some(layers))
        } else {
            (area, None)
        }
    } else {
        (area, None)
    };

    let mut lines: Vec<Line<'static>> = Vec::new();

    // 1. Session title
    render_session_title(state, &mut lines);
    lines.push(Line::raw(""));

    // 2. Workspace info
    render_workspace_info(state, &mut lines);
    lines.push(Line::raw(""));

    // 3. Budget
    render_budget(state, &mut lines);
    lines.push(Line::raw(""));

    // 4. Provider dots
    render_providers(state, &mut lines);
    lines.push(Line::raw(""));

    // 5. SDD pipeline
    render_sdd_pipeline(state, &mut lines);
    lines.push(Line::raw(""));

    // 6. Waves progress
    render_waves(state, &mut lines);
    lines.push(Line::raw(""));

    // 7. Active agents (max 6 + overflow)
    render_agents(state, &mut lines);
    lines.push(Line::raw(""));

    // 8. Modified files (max 8 + overflow)
    render_files(state, &mut lines);
    lines.push(Line::raw(""));

    // 9. Version footer
    render_version_footer(&mut lines);

    let block = crate::ui::theme::block(" Sidebar ");

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, paragraph_area);

    // 10. Layers View section (after the paragraph, when visible and placement is SidebarSection)
    if let Some(layers_rect) = layers_area {
        render_layers_section(
            frame,
            &state.layers_state,
            &state.layers_config,
            layers_rect,
            state.spinner_frame,
        );
    }
}

/// Cap the agents list to `max` entries, returning the visible slice and overflow count.
pub fn cap_agents(agents: &[AgentThread], max: usize) -> (&[AgentThread], usize) {
    if agents.len() <= max {
        (agents, 0)
    } else {
        (&agents[..max], agents.len() - max)
    }
}

/// Cap the files list to `max` entries, returning the visible slice and overflow count.
pub fn cap_files(files: &[ModifiedFile], max: usize) -> (&[ModifiedFile], usize) {
    if files.len() <= max {
        (files, 0)
    } else {
        (&files[..max], files.len() - max)
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Format a number with thousand separators (e.g. 1_234_567 → "1,234,567").
fn format_thousands(n: u64) -> String {
    let s = n.to_string();
    let len = s.len();
    if len <= 3 {
        return s;
    }
    let mut result = String::with_capacity(len + len / 3);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(ch);
    }
    result
}

/// Truncate a path from the left with ellipsis if longer than `max_len`.
fn truncate_path_left(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        path.to_string()
    } else {
        // Show "…" + last (max_len - 1) chars
        let start = path.len() - (max_len - 1);
        format!("…{}", &path[start..])
    }
}

// ── Section renderers ──────────────────────────────────────────────────────

fn render_session_title(state: &AppState, lines: &mut Vec<Line<'static>>) {
    let title = state
        .session_title
        .as_ref()
        .map_or_else(|| "No active session".to_string(), String::clone);
    lines.push(Line::from(vec![Span::styled(
        title,
        Style::default().add_modifier(Modifier::BOLD),
    )]));
}

fn render_workspace_info(state: &AppState, lines: &mut Vec<Line<'static>>) {
    let ws = truncate_path_left(&state.workspace_path, PATH_MAX_LEN);
    lines.push(Line::from(vec![
        Span::styled("ws: ", Style::default().fg(Color::DarkGray)),
        Span::raw(ws),
    ]));
    if let Some(branch) = &state.git_branch {
        lines.push(Line::from(vec![
            Span::styled("git: ", Style::default().fg(Color::DarkGray)),
            Span::styled(branch.clone(), Style::default().fg(Color::Magenta)),
        ]));
    }
}

fn render_budget(state: &AppState, lines: &mut Vec<Line<'static>>) {
    let info = &state.budget_info;
    lines.push(Line::from(vec![Span::styled(
        "Budget",
        Style::default().add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from(vec![
        Span::styled("  tokens: ", Style::default().fg(Color::DarkGray)),
        Span::raw(format_thousands(info.tokens_used)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  usage:  ", Style::default().fg(Color::DarkGray)),
        Span::raw(format!("{}%", info.usage_pct)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  cost:   ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("${:.2}", info.cost_usd),
            Style::default().fg(Color::Yellow),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  max:    ", Style::default().fg(Color::DarkGray)),
        Span::raw(format!("${:.2}", info.max_budget_usd)),
    ]));
}

fn render_providers(state: &AppState, lines: &mut Vec<Line<'static>>) {
    lines.push(Line::from(vec![Span::styled(
        "Providers",
        Style::default().add_modifier(Modifier::BOLD),
    )]));
    if state.provider_statuses.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "  (none)",
            Style::default().fg(Color::DarkGray),
        )]));
        return;
    }
    for provider in &state.provider_statuses {
        let (dot_color, dot_char) = match provider.status {
            ConnectionStatus::Connected => (Color::Green, "●"),
            ConnectionStatus::Disconnected => (Color::Red, "●"),
            ConnectionStatus::Unknown => (Color::Yellow, "●"),
        };
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(dot_char, Style::default().fg(dot_color)),
            Span::raw(format!(" {}", provider.name)),
        ]));
    }
}

fn render_sdd_pipeline(state: &AppState, lines: &mut Vec<Line<'static>>) {
    lines.push(Line::from(vec![Span::styled(
        "SDD Pipeline",
        Style::default().add_modifier(Modifier::BOLD),
    )]));

    let Some(pipeline) = &state.sdd_pipeline else {
        lines.push(Line::from(vec![Span::styled(
            "  (inactive)",
            Style::default().fg(Color::DarkGray),
        )]));
        return;
    };

    let phases = [
        (SddPhase::Specify, "SPECIFY"),
        (SddPhase::Design, "DESIGN"),
        (SddPhase::Tasks, "TASKS"),
        (SddPhase::Execute, "EXECUTE"),
    ];

    let mut spans: Vec<Span<'static>> = vec![Span::raw("  ")];
    for (i, (phase, label)) in phases.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" → ", Style::default().fg(Color::DarkGray)));
        }
        if *phase == pipeline.current_phase {
            spans.push(Span::styled(
                (*label).to_string(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                (*label).to_string(),
                Style::default().add_modifier(Modifier::DIM),
            ));
        }
    }
    lines.push(Line::from(spans));
}

fn render_waves(state: &AppState, lines: &mut Vec<Line<'static>>) {
    lines.push(Line::from(vec![Span::styled(
        "Waves",
        Style::default().add_modifier(Modifier::BOLD),
    )]));

    if state.waves.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "  (none)",
            Style::default().fg(Color::DarkGray),
        )]));
        return;
    }

    for wave in &state.waves {
        let (icon, detail) = match wave.status {
            WaveStatus::Complete => ("✓".to_string(), String::new()),
            WaveStatus::InProgress => (
                "◑".to_string(),
                format!(" {}/{}", wave.completed, wave.total),
            ),
            WaveStatus::Pending => ("○".to_string(), String::new()),
        };

        let icon_color = match wave.status {
            WaveStatus::Complete => Color::Green,
            WaveStatus::InProgress => Color::Yellow,
            WaveStatus::Pending => Color::DarkGray,
        };

        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(icon, Style::default().fg(icon_color)),
            Span::raw(format!(" Wave {}", wave.wave_number)),
            Span::styled(detail, Style::default().fg(Color::DarkGray)),
        ]));
    }
}

fn render_agents(state: &AppState, lines: &mut Vec<Line<'static>>) {
    lines.push(Line::from(vec![Span::styled(
        "Agents",
        Style::default().add_modifier(Modifier::BOLD),
    )]));

    if state.active_agents.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "  (none)",
            Style::default().fg(Color::DarkGray),
        )]));
        return;
    }

    let (visible, overflow) = cap_agents(&state.active_agents, MAX_AGENTS);

    for agent in visible {
        let status_color = match agent.status.as_str() {
            "running" => Color::Green,
            "waiting" => Color::Yellow,
            "failed" => Color::Red,
            _ => Color::DarkGray,
        };

        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                agent.short_id.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(agent.kind.clone(), Style::default().fg(Color::Magenta)),
            Span::raw(" "),
            Span::styled(agent.status.clone(), Style::default().fg(status_color)),
            Span::raw(" "),
            Span::styled(
                format!("${:.2}", agent.cost_usd),
                Style::default().fg(Color::Yellow),
            ),
        ]));
    }

    if overflow > 0 {
        lines.push(Line::from(vec![Span::styled(
            format!("  +{overflow} more"),
            Style::default().fg(Color::DarkGray),
        )]));
    }
}

fn render_files(state: &AppState, lines: &mut Vec<Line<'static>>) {
    lines.push(Line::from(vec![Span::styled(
        "Files",
        Style::default().add_modifier(Modifier::BOLD),
    )]));

    if state.modified_files.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "  (none)",
            Style::default().fg(Color::DarkGray),
        )]));
        return;
    }

    let (visible, overflow) = cap_files(&state.modified_files, MAX_FILES);

    for file in visible {
        let path_display = truncate_path_left(&file.path, PATH_MAX_LEN);
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::raw(path_display),
            Span::raw(" "),
            Span::styled(
                format!("+{}", file.insertions),
                Style::default().fg(Color::Green),
            ),
            Span::raw("/"),
            Span::styled(
                format!("-{}", file.deletions),
                Style::default().fg(Color::Red),
            ),
        ]));
    }

    if overflow > 0 {
        lines.push(Line::from(vec![Span::styled(
            format!("  +{overflow} more"),
            Style::default().fg(Color::DarkGray),
        )]));
    }
}

fn render_version_footer(lines: &mut Vec<Line<'static>>) {
    lines.push(Line::from(vec![Span::styled(
        format!("kira v{}", env!("CARGO_PKG_VERSION")),
        Style::default().fg(Color::DarkGray),
    )]));
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_thousands_small() {
        assert_eq!(format_thousands(0), "0");
        assert_eq!(format_thousands(999), "999");
    }

    #[test]
    fn format_thousands_medium() {
        assert_eq!(format_thousands(1_000), "1,000");
        assert_eq!(format_thousands(12_345), "12,345");
        assert_eq!(format_thousands(999_999), "999,999");
    }

    #[test]
    fn format_thousands_large() {
        assert_eq!(format_thousands(1_000_000), "1,000,000");
        assert_eq!(format_thousands(1_234_567_890), "1,234,567,890");
    }

    #[test]
    fn truncate_path_short() {
        let path = "src/main.rs";
        assert_eq!(truncate_path_left(path, 30), "src/main.rs");
    }

    #[test]
    fn truncate_path_exact() {
        let path = "a".repeat(30);
        assert_eq!(truncate_path_left(&path, 30), path);
    }

    #[test]
    fn truncate_path_long() {
        let path = "a".repeat(40);
        let result = truncate_path_left(&path, 30);
        // The result should have 30 chars: "…" (1 char) + 29 'a' chars
        assert_eq!(result.chars().count(), 30);
        assert!(result.starts_with('…'));
        // Last 29 chars of the original path
        assert_eq!(&result[3..], &path[11..]); // "…" is 3 bytes in UTF-8
    }

    #[test]
    fn cap_agents_under_max() {
        let agents: Vec<AgentThread> = (0..3)
            .map(|i| AgentThread {
                id: format!("agent-{i}"),
                short_id: format!("agent-{i}"),
                kind: "writer".to_string(),
                status: "running".to_string(),
                cost_usd: 0.01,
                last_output: None,
            })
            .collect();
        let (visible, overflow) = cap_agents(&agents, 6);
        assert_eq!(visible.len(), 3);
        assert_eq!(overflow, 0);
    }

    #[test]
    fn cap_agents_over_max() {
        let agents: Vec<AgentThread> = (0..10)
            .map(|i| AgentThread {
                id: format!("agent-{i}"),
                short_id: format!("agent-{i}"),
                kind: "writer".to_string(),
                status: "running".to_string(),
                cost_usd: 0.01,
                last_output: None,
            })
            .collect();
        let (visible, overflow) = cap_agents(&agents, 6);
        assert_eq!(visible.len(), 6);
        assert_eq!(overflow, 4);
    }

    #[test]
    fn cap_files_under_max() {
        let files: Vec<ModifiedFile> = (0..5)
            .map(|i| ModifiedFile {
                path: format!("src/file{i}.rs"),
                insertions: 10,
                deletions: 2,
            })
            .collect();
        let (visible, overflow) = cap_files(&files, 8);
        assert_eq!(visible.len(), 5);
        assert_eq!(overflow, 0);
    }

    #[test]
    fn cap_files_over_max() {
        let files: Vec<ModifiedFile> = (0..12)
            .map(|i| ModifiedFile {
                path: format!("src/file{i}.rs"),
                insertions: 10,
                deletions: 2,
            })
            .collect();
        let (visible, overflow) = cap_files(&files, 8);
        assert_eq!(visible.len(), 8);
        assert_eq!(overflow, 4);
    }

    #[test]
    fn cap_agents_empty() {
        let agents: Vec<AgentThread> = Vec::new();
        let (visible, overflow) = cap_agents(&agents, 6);
        assert_eq!(visible.len(), 0);
        assert_eq!(overflow, 0);
    }

    #[test]
    fn cap_files_empty() {
        let files: Vec<ModifiedFile> = Vec::new();
        let (visible, overflow) = cap_files(&files, 8);
        assert_eq!(visible.len(), 0);
        assert_eq!(overflow, 0);
    }

    // ── Property-Based Tests ───────────────────────────────────────────────

    use proptest::prelude::*;

    fn arb_agent_thread() -> impl Strategy<Value = AgentThread> {
        (
            "[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}",
            "[a-f0-9]{8}",
            prop_oneof!["writer", "deployer", "judge"],
            prop_oneof!["running", "waiting", "failed", "completed"],
            0.0f64..100.0f64,
        )
            .prop_map(|(id, short_id, kind, status, cost_usd)| AgentThread {
                id,
                short_id,
                kind,
                status,
                cost_usd,
                last_output: None,
            })
    }

    fn arb_modified_file() -> impl Strategy<Value = ModifiedFile> {
        (
            "src/[a-z]{1,20}/[a-z]{1,15}\\.[a-z]{1,4}",
            0u32..1000,
            0u32..1000,
        )
            .prop_map(|(path, insertions, deletions)| ModifiedFile {
                path,
                insertions,
                deletions,
            })
    }

    // Feature: tui-layout-redesign, Property 3: Sidebar agent list is capped at 6
    // Validates: Requirements 2.9
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_agent_list_capped_at_6(
            agents in prop::collection::vec(arb_agent_thread(), 0..=20)
        ) {
            let n = agents.len();
            let (visible, overflow) = cap_agents(&agents, MAX_AGENTS);

            // Visible count is min(N, 6)
            let expected_visible = n.min(MAX_AGENTS);
            prop_assert_eq!(visible.len(), expected_visible);

            // Overflow count is correct
            if n > MAX_AGENTS {
                prop_assert_eq!(overflow, n - MAX_AGENTS);
            } else {
                prop_assert_eq!(overflow, 0);
            }

            // Visible slice contains the first min(N, 6) agents
            for (i, agent) in visible.iter().enumerate() {
                prop_assert_eq!(&agent.id, &agents[i].id);
            }
        }
    }

    // Feature: tui-layout-redesign, Property 4: Sidebar modified files list is capped at 8
    // Validates: Requirements 2.10
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_modified_files_capped_at_8(
            files in prop::collection::vec(arb_modified_file(), 0..=20)
        ) {
            let n = files.len();
            let (visible, overflow) = cap_files(&files, MAX_FILES);

            // Visible count is min(N, 8)
            let expected_visible = n.min(MAX_FILES);
            prop_assert_eq!(visible.len(), expected_visible);

            // Overflow count is correct
            if n > MAX_FILES {
                prop_assert_eq!(overflow, n - MAX_FILES);
            } else {
                prop_assert_eq!(overflow, 0);
            }

            // Visible slice contains the first min(N, 8) files
            for (i, file) in visible.iter().enumerate() {
                prop_assert_eq!(&file.path, &files[i].path);
            }
        }
    }
}
