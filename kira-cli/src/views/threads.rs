#![forbid(unsafe_code)]

//! Threads View — displays parallel agents with status, cost, model.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

/// Data for a single agent thread displayed in the TUI.
#[derive(Debug, Clone)]
pub struct AgentThread {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub iterations: u32,
    pub cost_usd: f64,
    pub model: String,
    pub memory_fragments: u32,
}

impl AgentThread {
    /// Short ID (first 8 chars).
    pub fn short_id(&self) -> &str {
        if self.id.len() >= 8 {
            &self.id[..8]
        } else {
            &self.id
        }
    }

    /// Whether this thread is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status.as_str(),
            "completed" | "failed" | "cancelled" | "stopped"
        )
    }

    /// Status color for rendering.
    pub fn status_color(&self) -> Color {
        match self.status.as_str() {
            "completed" => Color::Green,
            "failed" => Color::Red,
            "cancelled" | "stopped" => Color::Yellow,
            _ => Color::Cyan,
        }
    }
}

/// Render the threads list into a frame area.
pub fn render(frame: &mut Frame, threads: &[AgentThread], area: Rect) {
    if threads.is_empty() {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" threads (none active) ");
        frame.render_widget(block, area);
        return;
    }

    let items: Vec<ListItem> = threads
        .iter()
        .map(|t| {
            let status_style = Style::default().fg(t.status_color());
            let line = Line::from(vec![
                Span::styled(
                    format!("{} ", t.short_id()),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:<8} ", t.kind),
                    Style::default().fg(Color::Magenta),
                ),
                Span::styled(format!("{:<10} ", t.status), status_style),
                Span::raw(format!("iter={:<3} ", t.iterations)),
                Span::styled(
                    format!("${:.4} ", t.cost_usd),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(t.model.clone(), Style::default().fg(Color::DarkGray)),
                if t.memory_fragments > 0 {
                    Span::styled(
                        format!(" mem={}", t.memory_fragments),
                        Style::default().fg(Color::Blue),
                    )
                } else {
                    Span::raw("")
                },
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" threads ({}) ", threads.len())),
    );
    frame.render_widget(list, area);
}

/// Find an agent by ID prefix. Returns the full ID if exactly one match.
pub fn find_by_prefix<'a>(threads: &'a [AgentThread], prefix: &str) -> FindResult<'a> {
    let matches: Vec<&AgentThread> = threads
        .iter()
        .filter(|t| t.id.starts_with(prefix) && !t.is_terminal())
        .collect();

    match matches.len() {
        0 => FindResult::NotFound,
        1 => FindResult::Found(matches[0]),
        _ => FindResult::Ambiguous(matches.len()),
    }
}

#[derive(Debug)]
pub enum FindResult<'a> {
    Found(&'a AgentThread),
    NotFound,
    Ambiguous(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_thread(id: &str, status: &str) -> AgentThread {
        AgentThread {
            id: id.to_string(),
            kind: "writer".to_string(),
            status: status.to_string(),
            iterations: 5,
            cost_usd: 0.0042,
            model: "deepseek-chat".to_string(),
            memory_fragments: 0,
        }
    }

    #[test]
    fn short_id_returns_8_chars() {
        let t = make_thread("abcdefgh-1234-5678", "running");
        assert_eq!(t.short_id(), "abcdefgh");
    }

    #[test]
    fn is_terminal_checks_statuses() {
        assert!(make_thread("x", "completed").is_terminal());
        assert!(make_thread("x", "failed").is_terminal());
        assert!(make_thread("x", "cancelled").is_terminal());
        assert!(make_thread("x", "stopped").is_terminal());
        assert!(!make_thread("x", "running").is_terminal());
    }

    #[test]
    fn find_by_prefix_exact_match() {
        let threads = vec![
            make_thread("abc12345-xxxx", "running"),
            make_thread("def67890-yyyy", "running"),
        ];
        match find_by_prefix(&threads, "abc") {
            FindResult::Found(t) => assert_eq!(t.id, "abc12345-xxxx"),
            _ => panic!("expected Found"),
        }
    }

    #[test]
    fn find_by_prefix_not_found() {
        let threads = vec![make_thread("abc12345", "running")];
        match find_by_prefix(&threads, "xyz") {
            FindResult::NotFound => {}
            _ => panic!("expected NotFound"),
        }
    }

    #[test]
    fn find_by_prefix_ambiguous() {
        let threads = vec![
            make_thread("abc12345", "running"),
            make_thread("abc67890", "running"),
        ];
        match find_by_prefix(&threads, "abc") {
            FindResult::Ambiguous(n) => assert_eq!(n, 2),
            _ => panic!("expected Ambiguous"),
        }
    }

    #[test]
    fn find_by_prefix_skips_terminal() {
        let threads = vec![
            make_thread("abc12345", "completed"), // terminal — skip
            make_thread("abc67890", "running"),
        ];
        match find_by_prefix(&threads, "abc") {
            FindResult::Found(t) => assert_eq!(t.id, "abc67890"),
            _ => panic!("expected Found (only active)"),
        }
    }
}
