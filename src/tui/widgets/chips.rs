//! Chips de estado das sessões (PR-UX2): `[ok]`, `[falhou]`, `[exec]`.
//!
//! Regra de honestidade: o estado do chip vem de `SessionLine::ok`, que veio do
//! `ok` da trilha — a ÚNICA memória de resultado que existe hoje. Não há
//! "aguardando"/"pausado" aqui porque a store não grava isso para comando: chip
//! inventado seria estado de processo que ninguém observou. O `[exec]` é
//! diferente: ele existe só enquanto uma execução viva está na tela
//! ([`crate::tui::action::ExecStatus::Running`]), e desaparece com ela.
//!
//! Por que chip e não só cor: em `mono()` (e em terminal sem cor) a cor some; o
//! rótulo entre colchetes continua dizendo o estado na célula.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::tui::glyph::Glyphs;
use crate::tui::spinner;
use crate::tui::state::{self, SessionLine};
use crate::tui::theme::Palette;

/// Estado desenhado no chip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipState {
    /// A trilha gravou `ok = true`.
    Ok,
    /// A trilha gravou `ok = false`.
    Failed,
    /// Execução viva na tela agora (`ExecStatus::Running`).
    Running,
}

impl ChipState {
    /// Rótulo do chip: entre colchetes, igual em unicode e ascii.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ok => "[ok]",
            Self::Failed => "[falhou]",
            Self::Running => "[exec]",
        }
    }
}

/// Estado de uma sessão da trilha. Nunca "rodando": sessão gravada já terminou.
pub fn from_session(row: &SessionLine) -> ChipState {
    if row.ok {
        ChipState::Ok
    } else {
        ChipState::Failed
    }
}

/// Cor semântica do chip (a mesma da paleta de resultado).
pub fn color(state: ChipState, palette: Palette) -> ratatui::style::Color {
    match state {
        ChipState::Ok => palette.ok,
        ChipState::Failed => palette.failed,
        ChipState::Running => palette.running,
    }
}

/// Linha de uma sessão: `[ok] scan há 12 min 1.2s achados: 10`.
pub fn session_line(
    row: &SessionLine,
    now_secs: Option<i64>,
    palette: Palette,
    glyphs: Glyphs,
) -> Line<'static> {
    let state = from_session(row);
    let when = state::session_age_line(row, now_secs);
    let duration = state::format_duration_ms(row.duration_ms);
    let mut spans = vec![
        Span::styled(
            format!("{} ", state.label()),
            Style::default()
                .fg(color(state, palette))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:<12}", state::one_line(&row.command, 12)),
            Style::default().fg(palette.value),
        ),
        Span::styled(
            format!("{:<12}", state::one_line(&when, 12)),
            Style::default().fg(palette.footer),
        ),
        Span::styled(
            format!("{duration:>6} "),
            Style::default().fg(palette.footer),
        ),
    ];
    spans.push(Span::styled(
        state::one_line(&row.summary_first_line, 40),
        Style::default().fg(palette.value),
    ));
    // `glyphs` entra na assinatura para o dia em que o chip ganhar marcador; o
    // rótulo é ASCII por construção, então hoje ele já vale nos dois modos.
    let _ = glyphs;
    Line::from(spans)
}

/// Linha da execução viva: `[exec] triage 4821  executando há 4s  ⠙`.
pub fn running_line(
    action_label: &str,
    elapsed: &str,
    tick: u64,
    palette: Palette,
    glyphs: Glyphs,
) -> Line<'static> {
    let spinner = spinner::frame(tick, glyphs);
    Line::from(vec![
        Span::styled(
            format!("{} ", ChipState::Running.label()),
            Style::default()
                .fg(palette.running)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:<16}", crate::tui::widgets::card::crop(action_label, 16)),
            Style::default().fg(palette.value),
        ),
        Span::styled(format!("{elapsed} "), Style::default().fg(palette.footer)),
        Span::styled(spinner.to_string(), Style::default().fg(palette.running)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::glyph::{Glyphs, Mode, has_special_glyphs};
    use crate::tui::theme;

    fn session(ok: bool) -> SessionLine {
        SessionLine {
            id: 7,
            command: "scan".to_string(),
            args: String::new(),
            model: None,
            started_unix: Some(1_000),
            started_at: String::new(),
            local_started_at: None,
            duration_ms: 1_200,
            ok,
            summary_first_line: "achados: 10".to_string(),
            summary: String::new(),
        }
    }

    fn flat(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }

    /// O estado do chip vem do `ok` da trilha — nunca de palpite.
    #[test]
    fn chip_state_comes_from_the_session_ok_flag() {
        assert_eq!(from_session(&session(true)), ChipState::Ok);
        assert_eq!(from_session(&session(false)), ChipState::Failed);
        assert_eq!(from_session(&session(true)).label(), "[ok]");
        assert_eq!(from_session(&session(false)).label(), "[falhou]");
    }

    #[test]
    fn session_line_carries_command_age_duration_and_summary() {
        let palette = theme::mono();
        let text = flat(&session_line(
            &session(true),
            Some(1_720),
            palette,
            Glyphs::default(),
        ));
        assert!(text.contains("[ok]"), "{text}");
        assert!(text.contains("scan"), "{text}");
        assert!(text.contains("há 12 min"), "{text}");
        // `format_duration_ms` arredonda para segundos inteiros acima de 1s.
        assert!(text.contains("1s"), "{text}");
        assert!(text.contains("achados: 10"), "{text}");
    }

    #[test]
    fn failed_chip_is_marked_in_the_cell_not_only_by_colour() {
        let palette = theme::colorful();
        let state = from_session(&session(false));
        assert_eq!(state.label(), "[falhou]");
        assert_eq!(color(state, palette), palette.failed);
        // Em mono a cor some; o rótulo continua dizendo o estado.
        let mono = theme::mono();
        assert!(
            flat(&session_line(
                &session(false),
                Some(1_720),
                mono,
                Glyphs::default()
            ))
            .contains("[falhou]")
        );
    }

    #[test]
    fn running_chip_carries_the_spinner_and_the_elapsed_label() {
        let palette = theme::mono();
        let text = flat(&running_line(
            "triage 4821",
            "executando há 4s",
            3,
            palette,
            Glyphs::default(),
        ));
        assert!(text.contains("[exec]"), "{text}");
        assert!(text.contains("triage 4821"), "{text}");
        assert!(text.contains("executando há 4s"), "{text}");
        assert!(text.contains(spinner::UNICODE[3]), "{text}");
    }

    /// Em ASCII, o chip `[exec]` também não pode vazar braille.
    #[test]
    fn ascii_running_chip_uses_the_ascii_spinner() {
        let palette = theme::mono();
        let glyphs = Glyphs::new(Mode::Ascii);
        for tick in 0..12 {
            let text = flat(&running_line(
                "scan",
                "executando há 1s",
                tick,
                palette,
                glyphs,
            ));
            assert!(!has_special_glyphs(&text), "tick {tick}: {text}");
            assert!(
                spinner::ASCII.contains(&text.chars().last().unwrap_or(' ').to_string().as_str())
                    || text.ends_with(' '),
                "tick {tick}: {text}"
            );
        }
    }
}
