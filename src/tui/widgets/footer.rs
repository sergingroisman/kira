//! Rodapé de 1 linha com teclas em **chip** — assinatura do statui.
//!
//! Adaptado de statui (MIT) — `ui/widgets/footer.rs` e `ui/theme.rs`
//! (`footer_keys`/`footer_desc`). Ver `third_party/statui/NOTICE.md`.
//!
//! Regra do KIRA (persona §7): toda tela mostra as teclas que valem nela; em
//! `mono()` o chip usa `REVERSED` para continuar distinguível sem cor.
//!
//! As teclas de AÇÃO (`s`/`t`/`d`) só aparecem quando existem de verdade:
//! `--read-only` some com elas e o `o` só entra depois da primeira execução —
//! rodapé que promete tecla inexistente é mentira de UI.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::tui::theme::{self, Palette};

/// Uma tecla e o que ela faz, já no vocabulário do domínio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    pub key: &'static str,
    pub desc: &'static str,
}

const QUIT: Key = Key {
    key: "q",
    desc: "Sair",
};
const MOVE: Key = Key {
    key: "j/k",
    desc: "Mover",
};
const TAB: Key = Key {
    key: "Tab",
    desc: "Tela",
};
const HELP: Key = Key {
    key: "?",
    desc: "Ajuda",
};
const INSPECT: Key = Key {
    key: "i",
    desc: "Detalhe",
};

/// Teclas de ação, na ordem da matriz (scan, triage, rascunho de PR).
const ACTIONS: [Key; 3] = [
    Key {
        key: "s",
        desc: "Scan",
    },
    Key {
        key: "t",
        desc: "Triage",
    },
    Key {
        key: "d",
        desc: "PR",
    },
];

const EXECUTION: Key = Key {
    key: "o",
    desc: "Saída",
};

/// Teclas por tela (rodapé contextual), com as flags do estado real.
///
/// `actions` = a tela pode pedir ação (`!read_only`); `execution` = existe
/// execução em andamento/última para abrir com `o`.
pub fn keys_for(screen: usize, actions: bool, execution: bool) -> Vec<Key> {
    // 0 Resumo · 1 Achados · 2 Sessões · 3 Wiki
    let mut keys = Vec::with_capacity(10);
    keys.push(QUIT);
    if actions {
        keys.extend(ACTIONS);
    }
    match screen {
        1 => {
            keys.push(MOVE);
            keys.push(INSPECT);
            keys.push(Key {
                key: "f",
                desc: "Sev",
            });
            keys.push(Key {
                key: "m",
                desc: "Ferram",
            });
        }
        2 => {
            keys.push(MOVE);
            keys.push(INSPECT);
        }
        3 => {
            keys.push(MOVE);
            keys.push(Key {
                key: "i",
                desc: "Ler",
            });
            keys.push(Key {
                key: "/",
                desc: "Busca",
            });
        }
        // O Resumo não é lista: nem `j/k` nem `i` valem aqui.
        _ => {}
    }
    keys.push(TAB);
    if execution {
        keys.push(EXECUTION);
    }
    keys.push(HELP);
    keys
}

/// Spans do rodapé: ` key ` com fundo de chip + descrição.
pub fn spans(keys: &[Key], palette: Palette) -> Vec<Span<'static>> {
    let colored = theme::is_colorful(&palette);
    let mut spans = Vec::with_capacity(keys.len() * 3);
    for key in keys {
        let chip = Style::default().add_modifier(Modifier::BOLD);
        let chip = if colored {
            chip.fg(palette.key_fg).bg(palette.key_bg)
        } else {
            // Sem cor: `REVERSED` é o chip — legível em qualquer terminal.
            chip.add_modifier(Modifier::REVERSED)
        };
        spans.push(Span::styled(format!(" {} ", key.key), chip));
        spans.push(Span::styled(
            format!(" {} ", key.desc),
            Style::default().fg(palette.desc),
        ));
        spans.push(Span::raw(" "));
    }
    spans
}

/// Texto puro do rodapé (o mesmo conteúdo, sem cor).
pub fn text(keys: &[Key]) -> String {
    keys.iter()
        .map(|key| format!("{} {}", key.key, key.desc))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Desenha o rodapé na última linha do chrome.
pub fn render(frame: &mut Frame, area: Rect, keys: &[Key], palette: Palette) {
    if area.height == 0 {
        return;
    }
    frame.render_widget(Paragraph::new(Line::from(spans(keys, palette))), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn render_text(screen: usize, actions: bool, execution: bool) -> String {
        let backend = TestBackend::new(120, 1);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
        let keys = keys_for(screen, actions, execution);
        terminal
            .draw(|frame| render(frame, frame.area(), &keys, theme::mono()))
            .unwrap_or_else(|err| panic!("draw: {err}"));
        let buffer = terminal.backend().buffer();
        let mut text = String::new();
        for x in 0..120 {
            text.push(buffer[(x, 0)].symbol().chars().next().unwrap_or(' '));
        }
        text
    }

    #[test]
    fn footer_shows_sair_first_and_ajuda_last() {
        let keys = keys_for(0, false, false);
        assert_eq!(keys.first().map(|key| key.key), Some("q"));
        assert_eq!(keys.last().map(|key| key.key), Some("?"));
        let text = render_text(0, false, false);
        assert!(text.contains("Sair"), "{text}");
        assert!(text.contains("Ajuda"), "{text}");
    }

    #[test]
    fn detail_key_only_appears_where_a_record_can_be_opened() {
        assert!(keys_for(1, false, false).iter().any(|key| key.key == "i"));
        assert!(keys_for(2, false, false).iter().any(|key| key.key == "i"));
        assert!(keys_for(3, false, false).iter().any(|key| key.key == "i"));
        assert!(!keys_for(0, false, false).iter().any(|key| key.key == "i"));
    }

    #[test]
    fn action_and_execution_chips_follow_the_real_state() {
        // `--read-only`: nenhuma tecla de ação no rodapé.
        assert!(
            !keys_for(0, false, false)
                .iter()
                .any(|key| key.key == "s" || key.key == "t" || key.key == "d")
        );
        let acting = keys_for(0, true, false);
        assert_eq!(
            acting.iter().map(|key| key.key).collect::<Vec<_>>(),
            vec!["q", "s", "t", "d", "Tab", "?"],
            "ação logo depois de sair, sem prometer execução"
        );
        // `o` só existe quando há execução para abrir.
        assert!(keys_for(0, true, true).iter().any(|key| key.key == "o"));
        assert!(!keys_for(0, true, false).iter().any(|key| key.key == "o"));
    }

    #[test]
    fn footer_text_is_copyable_plain_text() {
        let text = text(&keys_for(1, true, false));
        assert!(text.contains("q Sair"), "{text}");
        assert!(text.contains("i Detalhe"), "{text}");
        assert!(text.contains("s Scan"), "{text}");
        assert!(!text.contains('\u{1b}'), "{text}");
    }
}
