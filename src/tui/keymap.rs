//! Tecla → ação. Toda a semântica de interação mora aqui, isolada em função
//! pura sobre `crossterm::event::KeyEvent`, para que cada atalho seja
//! unit-testável sem terminal real.
//!
//! Persona (docs/kira-persona.md §7): setas E vim (`j/k`, `g/G`) — as duas
//! famílias são cidadãs de primeira classe. Navegação não executa (ADR-0033):
//! `s`/`t`/`d` viram um PEDIDO (`Action::Request`), que só vira processo
//! depois da folha de alvo + popup de confirmação; `o` só abre o overlay do
//! que já está rodando. `Reload` apenas relê o disco.

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
    /// Abrir/fechar o inspetor 50–50 da tela atual (i) — tecla do statui.
    Inspect,
    /// Ciclar o filtro de severidade dos achados (f) — só em memória.
    FilterSeverity,
    /// Ciclar o filtro de ferramenta dos achados (m) — só em memória.
    FilterTool,
    /// Abrir a busca incremental da wiki (/) — o filtro é função pura.
    SearchWiki,
    /// Alternar a ajuda de atalhos (?).
    Help,
    /// Pedir uma AÇÃO (`s`/`t`/`d`) — pedido, não execução: quem monta o
    /// plano e confirma é o `app` (ADR-0033).
    Request(crate::tui::action::Request),
    /// Abrir/fechar o overlay da execução em andamento (`o`).
    Execution,
}

/// Traduz um evento de tecla em ação; `None` = tecla ignorada.
///
/// A SAÍDA (`q`, `Esc`, Ctrl+C) NÃO mora aqui: é a primeira checagem do
/// `app::decide` (contrato do dono — sair em QUALQUER estado, antes de
/// qualquer modal). Este mapa continua sem estado; o Ctrl+C cai no `_` e o
/// decide o converte em Quit antes de chegar aqui.
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
        // `i` é o inspetor do statui: abre/fecha o painel lateral 50–50.
        KeyCode::Char('i') => Some(Action::Inspect),
        // `f`/`m` filtram a tabela de achados em memória (nada é reescrito).
        KeyCode::Char('f') => Some(Action::FilterSeverity),
        KeyCode::Char('m') => Some(Action::FilterTool),
        // `/` abre a busca incremental do painel de wiki (W6-PR4): o
        // keymap só traduz a tecla; a aplicação decide se faz sentido no
        // painel em foco (só na wiki — nos outros é Ignore).
        KeyCode::Char('/') => Some(Action::SearchWiki),
        KeyCode::Char('?') => Some(Action::Help),
        // Teclas de AÇÃO: o keymap só diz o pedido; `decide` guarda
        // `read_only` e o loop monta/confirma o plano. Nenhuma delas spawna.
        KeyCode::Char('s') => Some(Action::Request(crate::tui::action::Request::Scan)),
        KeyCode::Char('t') => Some(Action::Request(crate::tui::action::Request::Triage)),
        KeyCode::Char('d') => Some(Action::Request(crate::tui::action::Request::PrDraft)),
        KeyCode::Char('o') => Some(Action::Execution),
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
        // W6-PR4: `/` abre a busca da wiki.
        assert_eq!(map(plain(KeyCode::Char('/'))), Some(Action::SearchWiki));
        assert_eq!(map(plain(KeyCode::Char('?'))), Some(Action::Help));
    }

    #[test]
    fn action_keys_map_to_requests_never_to_execution() {
        use crate::tui::action::Request;
        assert_eq!(
            map(plain(KeyCode::Char('s'))),
            Some(Action::Request(Request::Scan))
        );
        assert_eq!(
            map(plain(KeyCode::Char('t'))),
            Some(Action::Request(Request::Triage))
        );
        assert_eq!(
            map(plain(KeyCode::Char('d'))),
            Some(Action::Request(Request::PrDraft))
        );
        assert_eq!(map(plain(KeyCode::Char('o'))), Some(Action::Execution));
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
