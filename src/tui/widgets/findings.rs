//! Painel de achados — tabela de `findings.json` + detalhe do `Enter` (W6-PR2).
//!
//! Só pintura, como o cabeçalho: as linhas vêm de
//! [`crate::tui::state::rows_from`] via snapshot. O cabeçalho do bloco carrega
//! a contagem por severidade (`critical N · high N · medium N · low N`); as
//! colunas têm largura fixa calculada da área e texto truncado com `…` —
//! **sem scroll horizontal** (persona §7: denso, tudo visível por linha).
//! Seleção com `➤` (padrão do statui, ADR-0014) e linha destacada por
//! `REVERSED`, que existe em qualquer terminal inclusive mono.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};

use crate::tui::state::{self, FindingRow};
use crate::tui::theme::Palette;

/// O widget da tabela de achados.
#[derive(Debug, Clone)]
pub struct FindingsWidget<'a> {
    rows: &'a [FindingRow],
    selected: usize,
    /// Painel em foco? Borda realçada quando sim, esmaecida quando não.
    focused: bool,
    palette: Palette,
    /// Texto do estado vazio (já calculado em `state::empty_findings_line`).
    empty_line: String,
}

impl<'a> FindingsWidget<'a> {
    /// Constrói a partir do snapshot pronto (nunca consulta disco).
    pub fn new(
        rows: &'a [FindingRow],
        selected: usize,
        focused: bool,
        empty_line: String,
        palette: Palette,
    ) -> Self {
        Self {
            rows,
            selected,
            focused,
            palette,
            empty_line,
        }
    }

    /// Cor semântica da severidade — a mesma informação em texto no modo mono.
    fn severity_style(&self, severity: &str) -> Style {
        let color = match severity {
            "critical" => self.palette.critical,
            "high" => self.palette.high,
            "medium" => self.palette.medium,
            "low" => self.palette.low,
            // Fora do padrão (ex.: `moderate` do npm): cor de valor presente.
            _ => self.palette.value,
        };
        Style::default().fg(color)
    }
}

/// Larguras das colunas a partir da área interna, SEM scroll horizontal:
/// SEVERIDADE e FERRAMENTA são curtas e fixas; LOCAL ocupa até 26 e o RESUMO
/// leva o resto. Em área estreita tudo encolhe (coluna pode sumir), nada
/// estoura a borda — a soma nunca passa da área interna.
pub fn column_widths(inner_width: u16) -> (usize, usize, usize) {
    // Orçamento: marcador `➤ ` (2) + 3 separadores de 1 espaço entre as 4
    // células — nada disso pode estourar a borda.
    let total = usize::from(inner_width).saturating_sub(2 + 3);
    let severity = 10.min(total);
    let tool = 12.min(total.saturating_sub(severity));
    let location = total.saturating_sub(severity + tool).min(26);
    (severity, tool, location)
}

/// Uma célula com largura exata: trunca com `…` (nunca sai da coluna) e
/// completa com espaços (alinhamento de coluna em texto puro). Largura 0 =
/// célula vazia (a coluna some inteira, sem estourar 1 char).
pub fn cell(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let chars: Vec<char> = text.chars().collect();
    if chars.len() > width {
        let mut cut: String = chars.into_iter().take(width.saturating_sub(1)).collect();
        cut.push('…');
        return cut;
    }
    format!("{text:<width$}")
}

impl Widget for FindingsWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Cabeçalho de contagem no título do bloco: a contagem é a primeira
        // informação do painel, antes de qualquer linha.
        let title = if self.rows.is_empty() {
            " achados ".to_string()
        } else {
            format!(" achados — {} ", state::count_header_line(self.rows))
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
            // Estado vazio honesto: `—` com a causa ou contagem zero explícita.
            Paragraph::new(Line::from(Span::styled(
                self.empty_line.clone(),
                Style::default().fg(self.palette.missing),
            )))
            .block(Block::default())
            .wrap(Wrap { trim: false })
            .render(inner, buf);
            return;
        }

        let (severity_w, tool_w, location_w) = column_widths(inner.width);
        let marker_w = 2usize;
        // 3 separadores de 1 espaço entre as 4 células (legibilidade; sem
        // scroll horizontal — o RESUMO encolhe antes).
        let separators = 3usize;
        let message_w = usize::from(inner.width)
            .saturating_sub(marker_w + severity_w + tool_w + location_w + separators);
        let selected = self.selected.min(self.rows.len().saturating_sub(1));

        // Linha de ferramentas (`ferramentas: audit 2, dotnet 1`) e cabeçalho
        // das colunas: duas linhas fixas no topo da tabela.
        let mut header_rows = vec![state::tools_line(self.rows)];
        header_rows.push(format!(
            "{}{} {} {} {}",
            cell("", marker_w),
            cell("SEVERIDADE", severity_w),
            cell("FERRAMENTA", tool_w),
            cell("LOCAL", location_w),
            cell("RESUMO", message_w),
        ));
        Paragraph::new(
            header_rows
                .iter()
                .map(|line| {
                    Line::from(Span::styled(
                        line.clone(),
                        Style::default().fg(self.palette.missing),
                    ))
                })
                .collect::<Vec<_>>(),
        )
        .render(inner, buf);

        // Linhas da tabela: y fixo por índice — tabela de verdade, não fluxo.
        let row_area = Rect {
            x: inner.x,
            y: inner.y + u16::try_from(header_rows.len()).unwrap_or(u16::MAX),
            width: inner.width,
            height: inner.height.saturating_sub(header_rows.len() as u16),
        };
        for (index, row) in self.rows.iter().enumerate() {
            if index as u16 >= row_area.height {
                break; // sem scroll horizontal e sem página: o que não couber
                // espera o terminal maior (denso, não escondido).
            }
            let is_selected = index == selected;
            let marker = if is_selected { "➤ " } else { "  " };
            let body = format!(
                "{}{} {} {} {}",
                cell(marker, marker_w),
                cell(&row.severity, severity_w),
                cell(&row.tool, tool_w),
                cell(&row.location(), location_w),
                cell(&row.message_first_line, message_w),
            );
            let mut line = Line::from(Span::styled(body, self.severity_style(&row.severity)));
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

/// O painel lateral do detalhe (`Enter`): severidade, ferramenta, LOCAL,
/// fingerprint completo e mensagem inteira — nada truncado aqui.
#[derive(Debug, Clone)]
pub struct DetailWidget<'a> {
    lines: &'a [String],
    palette: Palette,
}

impl<'a> DetailWidget<'a> {
    pub fn new(lines: &'a [String], palette: Palette) -> Self {
        Self { lines, palette }
    }
}

impl Widget for DetailWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                " detalhe — Esc volta ",
                Style::default()
                    .fg(self.palette.title)
                    .add_modifier(Modifier::BOLD),
            ))
            .border_style(Style::default().fg(self.palette.border_focus));
        let inner = block.inner(area);
        block.render(area, buf);
        Paragraph::new(
            self.lines
                .iter()
                .map(|line| {
                    // O `fingerprint:`/`severidade:` recebem a cor do valor:
                    // dado real do disco, não aviso.
                    Line::from(Span::styled(
                        line.clone(),
                        Style::default().fg(self.palette.value),
                    ))
                })
                .collect::<Vec<_>>(),
        )
        .block(Block::default())
        .wrap(Wrap { trim: false })
        .render(inner, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::Finding;
    use crate::tui::state::rows_from;
    use crate::tui::theme;
    use ratatui::{Terminal, backend::TestBackend};

    /// Achado de referência do widget (mesma família dos testes de state).
    fn finding(tool: &str, severity: &str, path: &str, message: &str) -> Finding {
        Finding {
            tool: tool.to_string(),
            rule: "outdated".to_string(),
            package: "pkg".to_string(),
            version: "1.0.0".to_string(),
            severity: severity.to_string(),
            path: path.to_string(),
            message: message.to_string(),
            fingerprint: "0123456789abcdef".to_string(),
        }
    }

    /// Renderiza num buffer de teste e devolve o texto (smith determinístico).
    fn render_text(
        rows: &[FindingRow],
        selected: usize,
        focused: bool,
        width: u16,
        height: u16,
    ) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
        let widget = FindingsWidget::new(
            rows,
            selected,
            focused,
            "achados: — (nunca rodou kira scan)".to_string(),
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
    fn table_renders_four_severities_in_canonical_order_with_selection() {
        let rows = rows_from(vec![
            finding("dotnet", "low", "App.csproj", "pacote deprecado"),
            finding("audit", "high", "package.json", "advisory de segurança"),
            finding("audit", "critical", "src/lib.rs:42", "Prototype Pollution"),
            finding("npm", "medium", "package.json", "revisão pendente"),
        ]);
        let text = render_text(&rows, 0, true, 80, 12);
        assert!(
            text.contains("achados — critical 1 · high 1 · medium 1 · low 1"),
            "contagem no cabeçalho do bloco: {text}"
        );
        assert!(text.contains("SEVERIDADE"), "{text}");
        assert!(text.contains("FERRAMENTA"), "{text}");
        assert!(text.contains("LOCAL"), "{text}");
        assert!(text.contains("RESUMO"), "{text}");
        assert!(text.contains("➤"), "seleção visível: {text}");
        // Ordem critical→low no buffer: critical aparece antes de high.
        let critical = text.find("critical").unwrap_or(usize::MAX);
        let high = text.find("high").unwrap_or(usize::MAX);
        let low = text.find("low").unwrap_or(usize::MAX);
        assert!(critical < high, "{text}");
        assert!(high < low, "{text}");
        // path:linha na coluna LOCAL e resumo na RESUMO.
        assert!(text.contains("src/lib.rs:42"), "{text}");
        assert!(text.contains("Prototype Pollution"), "{text}");
    }

    #[test]
    fn long_cells_are_truncated_never_scrolled() {
        let long_message = "mensagem gigante que estouraria qualquer coluna de resumo do painel";
        let long_path = "diretorio/muito/comprido/que/estoura/a/coluna/local/e/nao/role.rs:7";
        let rows = rows_from(vec![finding("audit", "critical", long_path, long_message)]);
        let text = render_text(&rows, 0, true, 40, 6);
        // Nada cruza a borda direita nas linhas internas: a última coluna é
        // a última coisa visível (as linhas de borda terminam em ┐/┘).
        for line in text.lines().skip(2).take(2) {
            let border = line.chars().last().unwrap_or(' ');
            assert_eq!(border, '│', "borda preservada em toda linha: {line:?}");
        }
        assert!(text.contains('…'), "corte com reticência: {text}");
    }

    #[test]
    fn empty_list_renders_the_honest_empty_state() {
        let text = render_text(&[], 0, true, 50, 5);
        assert!(
            text.contains("achados: — (nunca rodou kira scan)"),
            "estado vazio honesto: {text}"
        );
        assert!(
            text.contains("achados "),
            "título do bloco presente: {text}"
        );
    }

    #[test]
    fn tiny_terminal_still_renders_without_panic() {
        let rows = rows_from(vec![finding("audit", "high", "package.json", "advisory")]);
        let text = render_text(&rows, 0, false, 20, 4);
        assert!(!text.is_empty(), "{text}");
    }

    #[test]
    fn detail_widget_renders_every_field() {
        let rows = rows_from(vec![finding(
            "audit",
            "critical",
            "src/lib.rs:42",
            "Prototype Pollution\nCVE-2020-8203",
        )]);
        let detail = state::detail_lines(&rows[0]);
        let backend = TestBackend::new(50, 10);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
        let widget = DetailWidget::new(&detail, theme::mono());
        terminal
            .draw(|frame| frame.render_widget(widget, frame.area()))
            .unwrap_or_else(|err| panic!("draw: {err}"));
        let buffer = terminal.backend().buffer();
        let mut text = String::new();
        for y in 0..10 {
            for x in 0..50 {
                text.push(buffer[(x, y)].symbol().chars().next().unwrap_or(' '));
            }
            text.push('\n');
        }
        assert!(text.contains("severidade: critical"), "{text}");
        assert!(text.contains("ferramenta: audit"), "{text}");
        assert!(text.contains("local: src/lib.rs:42"), "{text}");
        assert!(text.contains("fingerprint: 0123456789abcdef"), "{text}");
        assert!(text.contains("CVE-2020-8203"), "mensagem inteira: {text}");
        assert!(text.contains("Esc volta"), "{text}");
    }

    #[test]
    fn column_widths_never_exceed_the_area() {
        // Área normal: LOCAL capped em 26, RESUMO leva o resto.
        let (severity, tool, location) = column_widths(80);
        assert_eq!((severity, tool, location), (10, 12, 26));
        // Área estreita: tudo encolhe sem panico nem scroll.
        let (severity, tool, location) = column_widths(12);
        assert!(
            severity + tool + location <= 10,
            "{severity} {tool} {location}"
        );
    }

    #[test]
    fn cell_truncates_with_ellipsis_and_pads() {
        assert_eq!(cell("abc", 5), "abc  ");
        assert_eq!(cell("abcdef", 5), "abcd…");
        // Unicode: corta por chars, não por bytes (acentos não viram lixo).
        assert_eq!(cell("crítico", 4), "crí…");
    }
}
