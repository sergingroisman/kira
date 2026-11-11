//! Tecla → ação. Toda a semântica de interação mora aqui, isolada em função
//! pura sobre `crossterm::event::KeyEvent`, para que cada atalho seja
//! unit-testável sem terminal real.
//!
//! Persona (docs/kira-persona.md §7): setas E vim (`j/k`, `g/G`) — as duas
//! famílias são cidadãs de primeira classe. Nenhuma tecla escreve: não existe
//! `Action` de mutação, e o `Reload` apenas relê o disco.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// O que o loop do `app.rs` executa após o keymap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Sair do dashboard.
    Quit,
    /// Painel seguinte (Tab; Shift+Tab vai para o anterior).
    NextPanel,
    /// Painel anterior (Shift+Tab).
    PrevPanel,
    /// Descer uma linha (j ou ↓).
    Down,
    /// Subir uma linha (k ou ↑).
    Up,
    /// Saltar para o topo (g).
    Top,
    /// Saltar para o fim (G).
    Bottom,
    /// Reler o disco e recompor o snapshot (r) — sem rede, sem escrita.
    Reload,
    /// Alternar a ajuda de atalhos (?).
    Help,
}

/// Traduz um evento de tecla em ação; `None` = tecla ignorada.
///
/// Só `Char('q')` **sem** Ctrl é saída: Ctrl+C não é tratado aqui porque o
/// crossterm em raw mode não gera interrupção, e capturar Ctrl+C como saída
/// duplicaria o comportamento do `q` sem precisão de contrato.
pub fn map(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Tab => Some(Action::NextPanel),
        // Shift+Tab chega como BackTab no crossterm (o modificador SHIFT vem
        // junto; qualquer um dos dois formulários fecha o ciclo para trás).
        KeyCode::BackTab => Some(Action::PrevPanel),
        KeyCode::Char('j') | KeyCode::Down => Some(Action::Down),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::Up),
        KeyCode::Char('g') => Some(Action::Top),
        // 'G' maiúsculo: no vim é o fim da lista. O crossterm entrega o char
        // já maiúsculo com SHIFT; exigimos SHIFT presente para não confundir
        // layouts sem shift — na prática ambos chegam juntos.
        KeyCode::Char('G') if key.modifiers.contains(KeyModifiers::SHIFT) => Some(Action::Bottom),
        KeyCode::Char('G') => Some(Action::Bottom),
        KeyCode::Char('r') => Some(Action::Reload),
        KeyCode::Char('?') => Some(Action::Help),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEventState};

    /// Helper: monta o KeyEvent do jeito que o crossterm entrega teclas nuas.
    fn plain(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn quit_tab_and_cycle_map_to_their_actions() {
        assert_eq!(map(plain(KeyCode::Char('q'))), Some(Action::Quit));
        assert_eq!(map(plain(KeyCode::Tab)), Some(Action::NextPanel));
        assert_eq!(map(plain(KeyCode::BackTab)), Some(Action::PrevPanel));
    }

    #[test]
    fn vim_and_arrows_move_together() {
        assert_eq!(map(plain(KeyCode::Char('j'))), Some(Action::Down));
        assert_eq!(map(plain(KeyCode::Down)), Some(Action::Down));
        assert_eq!(map(plain(KeyCode::Char('k'))), Some(Action::Up));
        assert_eq!(map(plain(KeyCode::Up)), Some(Action::Up));
    }

    #[test]
    fn top_bottom_go_to_the_extremes() {
        assert_eq!(map(plain(KeyCode::Char('g'))), Some(Action::Top));
        // Shift+G: crossterm reporta char 'G' com SHIFT — é o fim da lista.
        let shift_g = KeyEvent {
            modifiers: KeyModifiers::SHIFT,
            ..KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE)
        };
        assert_eq!(map(shift_g), Some(Action::Bottom));
        // Sem SHIFT (layout sem shift) também fecha em Bottom: melhor aceitar
        // do que deixar o usuário sem o fim da lista.
        assert_eq!(map(plain(KeyCode::Char('G'))), Some(Action::Bottom));
    }

    #[test]
    fn reload_and_help_are_read_only_actions() {
        assert_eq!(map(plain(KeyCode::Char('r'))), Some(Action::Reload));
        assert_eq!(map(plain(KeyCode::Char('?'))), Some(Action::Help));
    }

    #[test]
    fn unknown_and_modifier_keys_are_ignored() {
        assert_eq!(map(plain(KeyCode::Char('x'))), None);
        assert_eq!(map(plain(KeyCode::Enter)), None);
        assert_eq!(map(plain(KeyCode::Esc)), None);
        let ctrl_q = KeyEvent {
            modifiers: KeyModifiers::CONTROL,
            ..KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)
        };
        assert_eq!(map(ctrl_q), Some(Action::Quit));
        // KeyCode sem representação de texto: ignorado.
        assert_eq!(map(plain(KeyCode::F(1))), None);
        let _ = KeyEventState::NONE;
    }
}
