//! Geometria do dashboard — funções PURAS sobre `Rect` (casca do statui).
//!
//! O statui monta a tela em duas faixas (`Min(0)` + rodapé de 1 linha) e troca
//! a área de trabalho entre 100% e 50–50 quando o inspetor abre
//! (`third_party/statui/NOTICE.md`). Aqui a mesma geometria vira função pura e
//! testável: nenhum widget calcula layout por conta própria.

use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Largura mínima do layout denso (spec W6). Abaixo disso a casca degrada para
/// lista única com aviso — nunca quebra.
pub const MIN_WIDTH: u16 = 80;

/// Altura mínima do layout denso (spec W6).
pub const MIN_HEIGHT: u16 = 24;

/// Terminal pequeno demais para o layout denso?
pub fn is_degraded(area: Rect) -> bool {
    area.width < MIN_WIDTH || area.height < MIN_HEIGHT
}

/// As quatro faixas fixas do chrome: status (1), abas (1), trabalho (resto) e
/// rodapé de 1 linha — o mesmo desenho do statui, com uma linha de abas a mais.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shell {
    pub status: Rect,
    pub tabs: Rect,
    pub workspace: Rect,
    pub footer: Rect,
}

/// Divide a tela nas faixas do chrome. Altura minúscula não panica: as faixas
/// que não cabem viram `Rect` vazio (altura 0) e o desenho segue.
pub fn shell(area: Rect) -> Shell {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);
    Shell {
        status: rows[0],
        tabs: rows[1],
        workspace: rows[2],
        footer: rows[3],
    }
}

/// Área de trabalho: 100% quando o inspetor está fechado, 50–50 quando aberto
/// (proporção do statui). `inspector` é `None` no modo normal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Workspace {
    pub primary: Rect,
    pub inspector: Option<Rect>,
}

/// Divide a área de trabalho entre tabela e inspetor.
pub fn workspace(area: Rect, inspecting: bool) -> Workspace {
    if !inspecting {
        return Workspace {
            primary: area,
            inspector: None,
        };
    }
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    Workspace {
        primary: cols[0],
        inspector: Some(cols[1]),
    }
}

/// Retângulo centralizado por porcentagem (popup de confirmação/estado vazio),
/// mesma técnica de `util::centered_rect` do statui, recalculada como pura.
pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let percent_x = percent_x.min(100);
    let percent_y = percent_y.min(100);
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1]);
    horizontal[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(width: u16, height: u16) -> Rect {
        Rect::new(0, 0, width, height)
    }

    #[test]
    fn shell_reserves_one_row_for_status_tabs_and_footer() {
        let shell = shell(area(120, 40));
        assert_eq!(shell.status.height, 1);
        assert_eq!(shell.tabs.height, 1);
        assert_eq!(shell.footer.height, 1);
        assert_eq!(shell.workspace.height, 37);
        assert_eq!(shell.footer.y, 39);
    }

    #[test]
    fn tiny_terminal_degenerates_without_panicking() {
        // 4×3 não cabe nem o chrome: as faixas que sobram ficam vazias.
        let shell = shell(area(4, 3));
        assert_eq!(shell.workspace.height, 0);
        assert!(is_degraded(area(4, 3)));
        let ws = workspace(shell.workspace, true);
        // Nunca pede área maior que a disponível (aqui, zero).
        assert!(ws.primary.width + ws.inspector.unwrap_or_default().width <= 4);
    }

    #[test]
    fn inspecting_splits_the_workspace_in_half() {
        let ws = workspace(area(120, 30), true);
        assert!(ws.inspector.is_some());
        assert_eq!(ws.primary.width, 60);
        assert_eq!(ws.inspector.unwrap_or_default().width, 60);
        let closed = workspace(area(120, 30), false);
        assert!(closed.inspector.is_none());
        assert_eq!(closed.primary.width, 120);
    }

    #[test]
    fn threshold_is_eighty_by_twenty_four() {
        assert!(!is_degraded(area(80, 24)));
        assert!(is_degraded(area(79, 24)));
        assert!(is_degraded(area(80, 23)));
    }

    #[test]
    fn centered_rect_stays_inside_the_area() {
        let popup = centered_rect(70, 60, area(120, 40));
        assert!(popup.x + popup.width <= 120);
        assert!(popup.y + popup.height <= 40);
        assert!(popup.width > 0 && popup.height > 0);
        // Porcentagem absurda satura em 100%, nunca estoura.
        let full = centered_rect(200, 200, area(40, 20));
        assert!(full.width <= 40 && full.height <= 20);
    }
}
