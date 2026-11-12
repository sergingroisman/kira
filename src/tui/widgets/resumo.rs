//! Tela Resumo: faixa visual de risco + seções de texto do inspetor do statui.
//!
//! Adaptado de statui (MIT) — `ui/widgets/inspector/mod.rs` (bloco externo +
//! seções com régua). Ver `third_party/statui/NOTICE.md`.
//!
//! Duas camadas, de propósito:
//!
//! 1. **Faixa visual** (PR-UX1): barras de severidade, tendência de achados por
//!    scan e uso do índice. É o que se lê em dois segundos — a comparação que o
//!    texto sozinho obriga a fazer de cabeça.
//! 2. **Seções de texto**: continuam vindo de
//!    [`crate::tui::state::resumo_sections`], função pura sobre o snapshot, e
//!    são a MESMA saída do `--snapshot` em pipe/CI. Com a faixa visual
//!    presente, a seção "Achados" é omitida aqui (as barras já dizem a mesma
//!    contagem) — duplicar número no mesmo painel gasta linha sem informar.
//!
//! Nenhuma frase de análise é gerada: são contagens, campos do snapshot e a
//! causa honesta do dado ausente.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::tui::glyph::Glyphs;
use crate::tui::theme::Palette;
use crate::tui::widgets::{gauge, section, severity, trend};

/// Dados da faixa visual do Resumo (todos reais, todos do snapshot).
#[derive(Debug, Clone, Copy)]
pub struct Visual<'a> {
    /// Severidades PRESENTES com a contagem (`severity_bars`).
    pub severities: &'a [(String, usize)],
    /// Série de achados por scan, mais antigo primeiro.
    pub trend: &'a [u64],
    /// Uso do índice: `Some((arquivos, teto))` quando há contagem.
    pub index: Option<(usize, usize)>,
    /// Texto da linha do índice quando não há contagem (já com a causa).
    pub index_fallback: &'a str,
}

/// Widget do Resumo: painel com borda dupla, faixa visual e seções.
#[derive(Debug, Clone)]
pub struct ResumoWidget<'a> {
    sections: &'a [(String, Vec<String>)],
    /// Painel em foco? O Resumo perde a borda realçada quando o usuário está
    /// em outra fonte.
    focused: bool,
    palette: Palette,
    /// Glifos da sessão (`KIRA_GLYPHS=ascii`).
    glyphs: Glyphs,
    /// Faixa visual; `None` = só texto (é o caso dos testes de seção).
    visual: Option<Visual<'a>>,
}

impl<'a> ResumoWidget<'a> {
    pub fn new(sections: &'a [(String, Vec<String>)], focused: bool, palette: Palette) -> Self {
        Self {
            sections,
            focused,
            palette,
            glyphs: Glyphs::default(),
            visual: None,
        }
    }

    /// Liga a faixa visual (barras + tendência + índice).
    pub fn with_visual(mut self, visual: Visual<'a>) -> Self {
        self.visual = Some(visual);
        self
    }

    /// Glifos da sessão (o `app` passa o modo lido do ambiente uma vez).
    pub fn with_glyphs(mut self, glyphs: Glyphs) -> Self {
        self.glyphs = glyphs;
        self
    }

    /// Desenha a faixa visual e devolve o `y` seguinte.
    fn render_visual(&self, inner: Rect, buf: &mut Buffer, mut y: u16) -> u16 {
        let Some(visual) = &self.visual else {
            return y;
        };
        let bottom = inner.y.saturating_add(inner.height);
        let row = |y: u16| Rect {
            x: inner.x,
            y,
            width: inner.width,
            height: 1,
        };

        if y < bottom {
            section::render_with(row(y), buf, "Risco", self.palette, self.glyphs);
            y = y.saturating_add(1);
        }
        let bars = u16::try_from(visual.severities.len().min(4)).unwrap_or(0);
        if bars > 0 && y < bottom {
            let budget = bars.min(bottom.saturating_sub(y));
            severity::render(
                Rect {
                    x: inner.x,
                    y,
                    width: inner.width,
                    height: budget,
                },
                buf,
                visual.severities,
                self.palette,
                self.glyphs,
            );
            y = y.saturating_add(budget);
        }
        if visual.trend.len() >= 2 && y < bottom {
            trend::render(row(y), buf, visual.trend, self.palette, self.glyphs);
            y = y.saturating_add(1);
        }
        if y < bottom {
            match visual.index {
                Some((used, cap)) => {
                    gauge::render(row(y), buf, "índice", used, cap, self.palette, self.glyphs)
                }
                None => {
                    Paragraph::new(Line::from(Span::styled(
                        visual.index_fallback.to_string(),
                        Style::default().fg(self.palette.missing),
                    )))
                    .render(row(y), buf);
                }
            }
            y = y.saturating_add(1);
        }
        y
    }
}

impl Widget for ResumoWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = crate::tui::widgets::panel::block_with(
            "Resumo",
            self.focused,
            self.palette,
            self.glyphs,
        );
        let inner = block.inner(area);
        block.render(area, buf);
        if inner.height == 0 || inner.width == 0 {
            return;
        }
        let bottom = inner.y.saturating_add(inner.height);
        let mut y = self.render_visual(inner, buf, inner.y);
        for (title, lines) in self.sections {
            // Com a faixa visual, "Achados" repetiria as barras na mesma tela.
            if self.visual.is_some() && title == "Achados" {
                continue;
            }
            if y >= bottom {
                break;
            }
            section::render_with(
                Rect {
                    x: inner.x,
                    y,
                    width: inner.width,
                    height: 1,
                },
                buf,
                title,
                self.palette,
                self.glyphs,
            );
            y += 1;
            for line in lines {
                if y >= bottom {
                    break;
                }
                let rect = Rect {
                    x: inner.x,
                    y,
                    width: inner.width,
                    height: 1,
                };
                // Sem wrap: a linha já vem em colunas fixas do `state` (o
                // corte é explícito, nunca um scroll horizontal escondido).
                Paragraph::new(Line::from(Span::styled(
                    line.clone(),
                    Style::default().fg(self.palette.value),
                )))
                .render(rect, buf);
                y += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::glyph::{Glyphs, Mode};
    use crate::tui::theme;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn render_text(width: u16, height: u16) -> String {
        let sections = vec![
            (
                "Achados".to_string(),
                vec!["critical 3 · high 7 · total 10".to_string()],
            ),
            (
                "Pendências".to_string(),
                vec!["— nenhuma pendência registrada".to_string()],
            ),
        ];
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
        let widget = ResumoWidget::new(&sections, true, theme::mono());
        terminal
            .draw(|frame| frame.render_widget(widget, frame.area()))
            .unwrap_or_else(|err| panic!("draw: {err}"));
        let buffer = terminal.backend().buffer();
        let mut text = String::new();
        for y in 0..height {
            for x in 0..width {
                text.push(buffer[(x, y)].symbol().chars().next().unwrap_or(' '));
            }
            text.push('\n');
        }
        text
    }

    /// Mesmo painel, agora com a faixa visual ligada.
    fn render_visual(width: u16, height: u16, glyphs: Glyphs) -> String {
        let sections = vec![
            (
                "Achados".to_string(),
                vec!["critical 3 · high 7 · total 10".to_string()],
            ),
            (
                "Sessões recentes".to_string(),
                vec!["QUANDO  COMANDO".to_string()],
            ),
        ];
        let severities = vec![("critical".to_string(), 3), ("high".to_string(), 7)];
        let trend = [3u64, 5, 4, 7];
        let visual = Visual {
            severities: &severities,
            trend: &trend,
            index: Some((812, 5000)),
            index_fallback: "índice do repositório: — (índice vazio)",
        };
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
        let widget = ResumoWidget::new(&sections, true, theme::mono())
            .with_visual(visual)
            .with_glyphs(glyphs);
        terminal
            .draw(|frame| frame.render_widget(widget, frame.area()))
            .unwrap_or_else(|err| panic!("draw: {err}"));
        let buffer = terminal.backend().buffer();
        let mut text = String::new();
        for y in 0..height {
            for x in 0..width {
                text.push(buffer[(x, y)].symbol().chars().next().unwrap_or(' '));
            }
            text.push('\n');
        }
        text
    }

    #[test]
    fn summary_renders_panel_and_sections() {
        let text = render_text(80, 12);
        assert!(text.contains("| Resumo |"), "{text}");
        assert!(text.contains("| Achados |"), "{text}");
        assert!(text.contains("| Pendências |"), "{text}");
        assert!(text.contains("critical 3"), "{text}");
        assert!(text.contains('╔'), "borda dupla: {text}");
    }

    #[test]
    fn tiny_area_does_not_panic_or_overflow() {
        let text = render_text(12, 4);
        assert!(!text.is_empty(), "{text}");
        for line in text.lines() {
            assert_eq!(line.chars().count(), 12, "{line:?}");
        }
    }

    #[test]
    fn visual_strip_shows_risk_trend_and_index_before_the_text_sections() {
        let text = render_visual(80, 20, Glyphs::default());
        assert!(text.contains("| Risco |"), "{text}");
        assert!(text.contains("critical"), "{text}");
        assert!(text.contains("tendência (4 scans):"), "{text}");
        assert!(text.contains("812/5000"), "{text}");
        // "Achados" some: as barras já dizem a mesma contagem.
        assert!(
            !text.contains("| Achados |"),
            "sem contagem duplicada: {text}"
        );
        // Ordem: risco acima das seções de texto.
        let risk = text.find("| Risco |").unwrap_or(usize::MAX);
        let sessions = text.find("| Sessões recentes |").unwrap_or(usize::MAX);
        assert!(risk < sessions, "{text}");
    }

    #[test]
    fn visual_strip_survives_a_small_terminal_without_overflow() {
        // 80×24 é o piso do layout denso; a faixa não pode empurrar o texto
        // para fora nem estourar a borda.
        let text = render_visual(80, 24, Glyphs::default());
        // Só as linhas INTERNAS têm borda vertical; a primeira e a última são
        // o topo e a base do painel.
        for line in text.lines().skip(1).take(22) {
            let border = line.chars().last().unwrap_or(' ');
            assert_eq!(border, '║', "borda preservada: {line:?}");
        }
        // 20 colunas: a faixa encolhe, o gauge perde a barra e mantém números.
        let tiny = render_visual(20, 10, Glyphs::default());
        assert!(!tiny.is_empty(), "{tiny}");
    }

    #[test]
    fn ascii_mode_resumo_has_no_box_drawing_or_block_glyph() {
        use crate::tui::glyph::has_special_glyphs;
        let text = render_visual(80, 20, Glyphs::new(Mode::Ascii));
        // Critério 4: nenhum glifo especial no Resumo inteiro (borda, barras,
        // sparkline, gauge). Acento de palavra não conta como glifo.
        assert!(!has_special_glyphs(&text), "modo ascii no Resumo: {text}");
        assert!(text.contains("| Risco |"), "título preservado: {text}");
        assert!(text.contains('#'), "barras em ascii: {text}");
    }
}
