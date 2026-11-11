//! Painel de sessões — tabela da trilha (`sessions`) + detalhe do summary
//! (`Enter`) — W6-PR3.
//!
//! Só pintura, como os outros painéis: as linhas vêm de
//! [`crate::tui::state::session_rows_from`] via snapshot. A idade relativa é
//! função PURA ([`crate::tui::state::humanize_age`]) sobre o `now` capturado
//! UMA vez no snapshot — nenhum relógio dentro do widget (determinismo da
//! spec W6). Colunas de largura da área e célula truncada com `…` — **sem
//! scroll horizontal**; seleção com `➤` (padrão do statui, ADR-0014).
//! Cores: sessão que FALHOU destacada, sucesso neutro — a mesma informação em
//! texto no modo mono.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};

use crate::tui::state::{self, SessionLine};
use crate::tui::theme::Palette;
use crate::tui::widgets::findings::cell;

/// O widget da tabela de sessões.
#[derive(Debug, Clone)]
pub struct SessionsWidget<'a> {
    rows: &'a [SessionLine],
    /// `now` do snapshot (unix, s) — o ÚNICO relógio que a pintura enxerga.
    now_secs: Option<i64>,
    selected: usize,
    /// Painel em foco? Borda realçada quando sim, esmaecida quando não.
    focused: bool,
    palette: Palette,
    /// Texto do estado vazio (já calculado em `state::empty_sessions_line`).
    empty_line: String,
}

impl<'a> SessionsWidget<'a> {
    /// Constrói a partir do snapshot pronto (nunca consulta disco).
    pub fn new(
        rows: &'a [SessionLine],
        now_secs: Option<i64>,
        selected: usize,
        focused: bool,
        empty_line: String,
        palette: Palette,
    ) -> Self {
        Self {
            rows,
            now_secs,
            selected,
            focused,
            palette,
            empty_line,
        }
    }

    /// Cor da linha: falha destacada, sucesso neutro — a informação é a MESMA
    /// no texto (o mono preserva a célula, a cor era só semântica visual).
    fn row_style(&self, ok: bool) -> Style {
        if ok {
            Style::default().fg(self.palette.value)
        } else {
            Style::default().fg(self.palette.critical)
        }
    }
}

/// Larguras das colunas a partir da área interna, SEM scroll horizontal:
/// #ID e QUANDO são curtas e fixas; COMANDO/MODELO/DURAÇÃO têm teto; o RESUMO
/// leva o resto. Em área estreita tudo encolhe (coluna pode sumir), nada
/// estoura a borda — a soma nunca passa da área interna.
pub fn column_widths(inner_width: u16) -> (usize, usize, usize, usize, usize, usize) {
    // Orçamento: marcador `➤ ` (2) + 5 separadores de 1 espaço entre as 6
    // células — nada disso pode estourar a borda.
    let total = usize::from(inner_width).saturating_sub(2 + 5);
    let id = 8.min(total);
    let quando = 10.min(total.saturating_sub(id));
    let comando = 12.min(total.saturating_sub(id + quando));
    let modelo = 12.min(total.saturating_sub(id + quando + comando));
    let duracao = 8.min(total.saturating_sub(id + quando + comando + modelo));
    // RESUMO leva o resto (pode ser 0 em área minúscula — nada estoura).
    let resumo = total.saturating_sub(id + quando + comando + modelo + duracao);
    (id, quando, comando, modelo, duracao, resumo)
}

impl Widget for SessionsWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Título do bloco: quantas sessões a store tem para mostrar (o limite
        // de 50 da spec já foi aplicado em `state::session_rows_from`).
        let title = if self.rows.is_empty() {
            " sessões ".to_string()
        } else {
            format!(" sessões — últimas {} ", self.rows.len())
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                title,
                Style::default()
                    .fg(self.palette.title)
                    .add_modifier(Modifier::BOLD),
            ))
            .border_style(Style::default().fg(if self.focused {
                self.palette.border_focus
            } else {
                self.palette.border_idle
            }));
        let inner = block.inner(area);
        block.render(area, buf);

        if self.rows.is_empty() {
            // Estado vazio honesto: `—` com a causa (store sem sessões etc.).
            Paragraph::new(Line::from(Span::styled(
                self.empty_line.clone(),
                Style::default().fg(self.palette.missing),
            )))
            .block(Block::default())
            .wrap(Wrap { trim: false })
            .render(inner, buf);
            return;
        }

        let (id_w, quando_w, comando_w, modelo_w, duracao_w, resumo_w) = column_widths(inner.width);
        let marker_w = 2usize;
        let selected = self.selected.min(self.rows.len().saturating_sub(1));

        // Cabeçalho das colunas: uma linha fixa no topo da tabela.
        let header_line = format!(
            "{}{} {} {} {} {} {}",
            cell("", marker_w),
            cell("#ID", id_w),
            cell("QUANDO", quando_w),
            cell("COMANDO", comando_w),
            cell("MODELO", modelo_w),
            cell("DURAÇÃO", duracao_w),
            cell("RESUMO", resumo_w),
        );
        Paragraph::new(Line::from(Span::styled(
            header_line,
            Style::default().fg(self.palette.missing),
        )))
        .render(inner, buf);

        // Linhas da tabela: y fixo por índice — tabela de verdade, não fluxo.
        let row_area = Rect {
            x: inner.x,
            y: inner.y + 1,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };
        for (index, row) in self.rows.iter().enumerate() {
            if index as u16 >= row_area.height {
                break; // sem scroll horizontal e sem página: o que não couber
                // espera o terminal maior (denso, não escondido).
            }
            let is_selected = index == selected;
            let marker = if is_selected { "➤ " } else { "  " };
            let body = format!(
                "{}{} {} {} {} {} {}",
                cell(marker, marker_w),
                cell(&format!("#{}", row.id), id_w),
                cell(&state::session_age_line(row, self.now_secs), quando_w),
                cell(&row.command, comando_w),
                cell(row.model.as_deref().unwrap_or("—"), modelo_w),
                cell(&state::format_duration_ms(row.duration_ms), duracao_w),
                cell(&row.summary_first_line, resumo_w),
            );
            let mut line = Line::from(Span::styled(body, self.row_style(row.ok)));
            if is_selected {
                // Destaque de seleção que sobrevive a `NO_COLOR`: REVERSED é
                // suportado por qualquer terminal, mono incluso.
                for span in &mut line.spans {
                    span.style = span.style.add_modifier(Modifier::REVERSED);
                }
            }
            let row_rect = Rect {
                y: row_area.y + index as u16,
                height: 1,
                ..row_area
            };
            Paragraph::new(line).render(row_rect, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::state::{SessionLine, humanize_age};
    use crate::tui::theme;
    use ratatui::{Terminal, backend::TestBackend};

    /// Linha de sessão de referência (o que `state::session_rows_from` devolve).
    fn session_line(
        id: i64,
        command: &str,
        model: Option<&str>,
        ok: bool,
        summary: &str,
    ) -> SessionLine {
        SessionLine {
            id,
            command: command.to_string(),
            model: model.map(|model| model.to_string()),
            started_unix: Some(1_762_861_200),
            started_at: "2026-11-11T09:00:00Z".to_string(),
            local_started_at: Some("2026-11-11 09:00:00".to_string()),
            duration_ms: 65_000,
            ok,
            summary_first_line: summary.lines().next().unwrap_or_default().to_string(),
            summary: summary.to_string(),
        }
    }

    /// Renderiza num buffer de teste e devolve o texto (smith determinístico).
    fn render_text(
        rows: &[SessionLine],
        now_secs: Option<i64>,
        selected: usize,
        focused: bool,
        width: u16,
        height: u16,
    ) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
        let widget = SessionsWidget::new(
            rows,
            now_secs,
            selected,
            focused,
            "sessões: — (store sem sessões)".to_string(),
            theme::mono(),
        );
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
    fn table_renders_id_when_command_model_duration_and_first_summary_line() {
        let rows = vec![
            session_line(
                3,
                "scan",
                None,
                true,
                "scan fixture: 3 achados\nlinha 2 interna",
            ),
            session_line(2, "triage", Some("qwen2.5:7b"), true, "nota gravada"),
            session_line(1, "doctor", None, false, "ollama não respondeu"),
        ];
        // now = started + 180s ⇒ "há 3 min" determinístico (o caso da spec).
        let now = 1_762_861_200 + 180;
        let text = render_text(&rows, Some(now), 0, true, 100, 12);
        assert!(text.contains("sessões — últimas 3"), "{text}");
        assert!(text.contains("#ID"), "{text}");
        assert!(text.contains("QUANDO"), "{text}");
        assert!(text.contains("COMANDO"), "{text}");
        assert!(text.contains("MODELO"), "{text}");
        assert!(text.contains("DURAÇÃO"), "{text}");
        assert!(text.contains("RESUMO"), "{text}");
        assert!(text.contains("#3"), "{text}");
        assert!(text.contains("scan"), "{text}");
        assert!(text.contains("qwen2.5:7b"), "{text}");
        assert!(text.contains("1m5s"), "{text}");
        assert!(text.contains("há 3 min"), "idade relativa pura: {text}");
        // Mais recente primeiro (id 3 antes do 2 e do 1).
        let id3 = text.find("#3").unwrap_or(usize::MAX);
        let id2 = text.find("#2").unwrap_or(usize::MAX);
        let id1 = text.find("#1").unwrap_or(usize::MAX);
        assert!(
            id3 < id2 && id2 < id1,
            "ordem mais-recente-primeiro: {text}"
        );
        // Só a 1ª linha do summary na tabela.
        assert!(text.contains("scan fixture: 3 achados"), "{text}");
        assert!(!text.contains("linha 2 interna"), "{text}");
        assert!(text.contains("➤"), "seleção visível: {text}");
    }

    #[test]
    fn failed_session_is_highlighted_and_success_is_neutral() {
        let rows = vec![
            session_line(1, "scan", None, true, "ok"),
            session_line(2, "doctor", None, false, "boom"),
        ];
        let backend = TestBackend::new(80, 8);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
        let palette = theme::colorful();
        let widget = SessionsWidget::new(
            &rows,
            Some(1_762_861_200 + 180),
            0,
            true,
            String::new(),
            palette,
        );
        terminal
            .draw(|frame| frame.render_widget(widget, frame.area()))
            .unwrap_or_else(|err| panic!("draw: {err}"));
        let buffer = terminal.backend().buffer();
        // A linha da falha (comando doctor) recebe a cor crítica; a do
        // sucesso recebe a cor neutra de valor presente.
        let mut failed_color = None;
        let mut ok_color = None;
        for y in 0..8 {
            let mut line = String::new();
            for x in 0..80 {
                line.push_str(buffer[(x, y)].symbol());
            }
            if line.contains("doctor") {
                failed_color = Some(buffer[(3, y)].style().fg);
            }
            if line.contains("#1") {
                ok_color = Some(buffer[(3, y)].style().fg);
            }
        }
        assert_eq!(
            failed_color,
            Some(Some(palette.critical)),
            "falha destacada"
        );
        assert_eq!(ok_color, Some(Some(palette.value)), "sucesso neutro");
    }

    #[test]
    fn long_cells_are_truncated_never_scrolled() {
        let long_command = "comando-gigante-que-estouraria-qualquer-coluna-do-painel";
        let long_summary = "summary gigante que estouraria qualquer coluna de resumo do painel";
        let rows = vec![session_line(
            1,
            long_command,
            Some("modelo-tambem-gigante"),
            true,
            long_summary,
        )];
        let text = render_text(&rows, Some(1_762_861_300), 0, true, 40, 6);
        // Nada cruza a borda direita nas linhas internas.
        for line in text.lines().skip(2).take(1) {
            let border = line.chars().last().unwrap_or(' ');
            assert_eq!(border, '│', "borda preservada em toda linha: {line:?}");
        }
        assert!(text.contains('…'), "corte com reticência: {text}");
    }

    #[test]
    fn empty_list_renders_the_honest_empty_state() {
        let text = render_text(&[], None, 0, true, 50, 5);
        assert!(
            text.contains("sessões: — (store sem sessões)"),
            "estado vazio honesto: {text}"
        );
        assert!(
            text.contains("sessões "),
            "título do bloco presente: {text}"
        );
    }

    #[test]
    fn tiny_terminal_still_renders_without_panic() {
        let rows = vec![session_line(1, "scan", None, true, "ok")];
        let text = render_text(&rows, Some(1_762_861_300), 0, false, 20, 4);
        assert!(!text.is_empty(), "{text}");
    }

    #[test]
    fn column_widths_never_exceed_the_area() {
        // Área normal: DURAÇÃO capped em 8, RESUMO leva o resto.
        let (id, quando, comando, modelo, duracao, resumo) = column_widths(100);
        assert_eq!((id, quando, comando, modelo, duracao), (8, 10, 12, 12, 8));
        assert!(resumo > 0, "{resumo}");
        // Área estreita: tudo encolhe sem pânico nem scroll.
        let (id, quando, comando, modelo, duracao, resumo) = column_widths(12);
        assert!(id + quando + comando + modelo + duracao + resumo <= 5);
        let (id, quando, comando, modelo, duracao, resumo) = column_widths(0);
        assert_eq!((id + quando + comando + modelo + duracao + resumo), 0);
    }

    #[test]
    fn the_widget_never_owns_a_clock() {
        // O contrato do determinismo: a idade vem de humanize_age(now, then)
        // com `now` do snapshot — a função é pura e o widget não tem relógio.
        assert_eq!(humanize_age(1_762_861_380, 1_762_861_200), "há 3 min");
        assert_eq!(humanize_age(1_762_861_200, 1_762_861_380), "há 0s");
    }
}
