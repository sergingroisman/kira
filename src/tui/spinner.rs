//! Spinner de execução (PR-UX2): ciclo de glifos PURO sobre o tick.
//!
//! Regra do dono, e por isso ela mora aqui e não no widget: **só aparece com
//! [`ExecStatus::Running`]**. Tela parada com glifo piscando finge atividade que
//! não existe — e num dashboard de diagnóstico isso é mentira sobre o estado do
//! processo. [`for_status`] é o único caminho de acesso: quem não tem filho
//! rodando recebe `None`.
//!
//! Puro sobre o tick pelo mesmo motivo de [`crate::tui::viewport`]: o `app`
//! incrementa um contador no loop (o `draw` já roda a cada `poll`), e o desenho
//! não chama relógio nem sorteia nada — o mesmo tick dá o mesmo frame.
//!
//! `KIRA_GLYPHS=ascii` usa os quatro frames de 7 bits: braille (`⠋`) em console
//! de fábrica vira quadradinho, e quadradinho no lugar do spinner é ruído.

use crate::tui::action::ExecStatus;
use crate::tui::glyph::Glyphs;

/// Frames do terminal moderno (braille, 10 posições — ciclo lento o bastante
/// para não virar estroboscópio no `poll` de 150 ms).
pub const UNICODE: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Frames ASCII (`KIRA_GLYPHS=ascii`): 4 posições, todas de 7 bits.
pub const ASCII: [&str; 4] = ["|", "/", "-", "\\"];

/// Série de frames do modo de glifos da sessão.
pub fn frames(glyphs: Glyphs) -> &'static [&'static str] {
    if glyphs.mode().is_ascii() {
        &ASCII
    } else {
        &UNICODE
    }
}

/// Frame para o tick informado. Função pura: mesmo tick, mesmo glifo.
pub fn frame(tick: u64, glyphs: Glyphs) -> &'static str {
    let frames = frames(glyphs);
    // `frames` nunca é vazio (constantes acima), mas a divisão é barata e o
    // `get` evita pânico se alguém zerar a série no futuro.
    let index = (tick as usize) % frames.len();
    frames.get(index).copied().unwrap_or(frames[0])
}

/// Glifo a desenhar DADO o estado da execução: `None` fora de execução.
///
/// É o gate do critério de aceite — sem filho rodando, nada de glifo.
pub fn for_status(status: ExecStatus, tick: u64, glyphs: Glyphs) -> Option<&'static str> {
    match status {
        ExecStatus::Running => Some(frame(tick, glyphs)),
        ExecStatus::Exited { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::glyph::{Glyphs, Mode, has_special_glyphs};

    #[test]
    fn unicode_cycle_has_ten_frames_and_wraps() {
        let glyphs = Glyphs::new(Mode::Unicode);
        assert_eq!(frames(glyphs).len(), 10);
        // Anda um por tick e volta ao começo no fim do ciclo.
        for (tick, expected) in UNICODE.iter().enumerate() {
            assert_eq!(frame(tick as u64, glyphs), *expected, "tick {tick}");
        }
        assert_eq!(frame(10, glyphs), UNICODE[0]);
        assert_eq!(frame(23, glyphs), UNICODE[3]);
    }

    /// Critério de aceite: em ASCII são os QUATRO frames ascii, sem nenhum
    /// glifo unicode vazando no frame desenhado.
    #[test]
    fn ascii_mode_uses_four_frames_and_leaks_no_unicode() {
        let glyphs = Glyphs::new(Mode::Ascii);
        assert_eq!(frames(glyphs).len(), 4);
        assert_eq!(frames(glyphs), &ASCII);

        let mut drawn = String::new();
        for tick in 0..40 {
            let glyph = frame(tick, glyphs);
            assert!(glyph.is_ascii(), "frame {tick} não é ascii: {glyph:?}");
            assert!(
                !has_special_glyphs(glyph),
                "frame {tick} tem glifo especial: {glyph:?}"
            );
            drawn.push_str(glyph);
        }
        // Nenhum caractere fora de `|/-\` em 40 ticks (cobre o ciclo inteiro).
        assert!(
            drawn
                .chars()
                .all(|c| ASCII.contains(&c.to_string().as_str())),
            "só frames ascii: {drawn}"
        );
    }

    #[test]
    fn spinner_is_none_unless_the_child_is_running() {
        let glyphs = Glyphs::default();
        assert_eq!(
            for_status(ExecStatus::Running, 3, glyphs),
            Some(UNICODE[3]),
            "rodando: desenha"
        );
        assert_eq!(
            for_status(ExecStatus::Exited { code: Some(0) }, 3, glyphs),
            None,
            "terminou bem: para"
        );
        assert_eq!(
            for_status(ExecStatus::Exited { code: Some(7) }, 3, glyphs),
            None,
            "terminou mal: para também"
        );
        assert_eq!(
            for_status(ExecStatus::Exited { code: None }, 3, glyphs),
            None,
            "encerrado sem código: para"
        );
    }

    #[test]
    fn the_same_tick_always_paints_the_same_frame() {
        for glyphs in [Glyphs::new(Mode::Unicode), Glyphs::new(Mode::Ascii)] {
            for tick in [0_u64, 1, 7, 9, 10, 999] {
                assert_eq!(frame(tick, glyphs), frame(tick, glyphs));
            }
        }
    }
}
