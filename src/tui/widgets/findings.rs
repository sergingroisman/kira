//! Painel de achados — tabela de `findings.json` + inspetor do `Enter`.
//!
//! Só pintura, como os outros painéis: as linhas vêm de
//! [`crate::tui::state::rows_from`] via snapshot. O título do bloco carrega a
//! contagem por severidade (`Achados — critical N · high N · …`) e o filtro
//! ativo, quando existe. Colunas com largura da área e célula truncada com `…`
//! — **sem scroll horizontal** (persona §7: denso, tudo visível por linha).
//! Seleção com `➤` (padrão do statui, ADR-0014) e linha destacada por
//! `REVERSED`, que existe em qualquer terminal inclusive mono.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget, Wrap};

use crate::tui::glyph::Glyphs;
use crate::tui::state::{self, FindingRow};
use crate::tui::theme::Palette;
use crate::tui::viewport;

/// Larguras efetivas das colunas. Coluna com largura 0 desaparece inteira —
/// em área estreita o painel perde FERRAMENTA antes de espremer o resumo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Columns {
    pub severity: usize,
    pub tool: usize,
    pub package: usize,
    pub location: usize,
    pub message: usize,
}

impl Columns {
    /// Quantas colunas (além do marcador) estão visíveis.
    pub fn visible(&self) -> usize {
        [
            self.severity,
            self.tool,
            self.package,
            self.location,
            self.message,
        ]
        .iter()
        .filter(|width| **width > 0)
        .count()
    }
}

/// Larguras a partir da área interna, SEM scroll horizontal.
///
/// 80 colunas: SEVERIDADE, PACOTE, LOCAL, RESUMO (a ferramenta vive no filtro e
/// no inspetor). 100+: entra FERRAMENTA e o pacote ganha espaço.
pub fn column_widths(inner_width: u16) -> Columns {
    let total = usize::from(inner_width);
    if total < 24 {
        // Área minúscula: só severidade e resumo, nada estoura a borda.
        let severity = 8.min(total);
        let message = total.saturating_sub(severity + 2);
        return Columns {
            severity,
            tool: 0,
            package: 0,
            location: 0,
            message,
        };
    }
    if total >= 100 {
        let severity = 10;
        let tool = 10;
        let package = 24;
        let location = 22;
        // marcador `➤ ` (2) + 1 espaço + 4 separadores entre 5 células.
        let used = 2 + 1 + 4 + severity + tool + package + location;
        return Columns {
            severity,
            tool,
            package,
            location,
            message: total.saturating_sub(used),
        };
    }
    let severity = 10;
    let package = 22.min(total.saturating_sub(severity + 8));
    let location = 18.min(total.saturating_sub(severity + package + 8));
    let used = 2 + 1 + 3 + severity + package + location;
    Columns {
        severity,
        tool: 0,
        package,
        location,
        message: total.saturating_sub(used),
    }
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

/// Junta as células visíveis com um espaço; coluna de largura 0 é omitida.
fn row_line(marker: &str, cells: &[(&str, usize)]) -> String {
    let mut parts = vec![cell(marker, 2)];
    for (text, width) in cells {
        if *width == 0 {
            continue;
        }
        parts.push(cell(text, *width));
    }
    parts.join(" ")
}

/// O widget da tabela de achados.
#[derive(Debug, Clone)]
pub struct FindingsWidget<'a> {
    /// Lista VISÍVEL (já filtrada por `f`/`m`), por referência — a tabela não
    /// clona linhas a cada frame.
    rows: &'a [&'a FindingRow],
    selected: usize,
    /// Painel em foco? Borda realçada quando sim, esmaecida quando não.
    focused: bool,
    palette: Palette,
    /// Texto do estado vazio (já calculado em `state::empty_findings_line`).
    empty_line: String,
    /// Filtro ativo, já em texto (`critical` / `audit`), ou `None`.
    filter: Option<String>,
    /// Conjunto de glifos (`KIRA_GLYPHS=ascii` troca marcador e scrollbar).
    glyphs: Glyphs,
}

impl<'a> FindingsWidget<'a> {
    /// Constrói a partir do snapshot pronto (nunca consulta disco).
    pub fn new(
        rows: &'a [&'a FindingRow],
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
            filter: None,
            glyphs: Glyphs::default(),
        }
    }

    /// Glifos da sessão (o `app` passa o modo lido do ambiente uma vez).
    pub fn with_glyphs(mut self, glyphs: Glyphs) -> Self {
        self.glyphs = glyphs;
        self
    }

    /// Filtro em memória (`f`/`m`): o mesmo texto aparece no título do bloco.
    pub fn with_filter(mut self, filter: Option<String>) -> Self {
        self.filter = filter;
        self
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

    /// Título do bloco: contagem canônica + filtro ativo.
    fn block_title(&self) -> String {
        let mut title = if self.rows.is_empty() {
            "Achados".to_string()
        } else {
            format!("Achados — {}", state::count_header_line_refs(self.rows))
        };
        if let Some(filter) = &self.filter {
            title.push_str(" · filtro: ");
            title.push_str(filter);
        }
        title
    }
}

impl Widget for FindingsWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = crate::tui::widgets::panel::block_with(
            &self.block_title(),
            self.focused,
            self.palette,
            self.glyphs,
        );
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

        // Duas linhas fixas no topo (ferramentas + colunas); o resto é lista.
        let list_height = inner.height.saturating_sub(2);
        let visible = usize::from(list_height);
        let selected = self.selected.min(self.rows.len().saturating_sub(1));
        let offset = viewport::keep_visible(selected, visible, self.rows.len());
        // O rótulo é a posição do CURSOR (`12/87`), não o topo da janela: é o
        // número que o dono acompanha ao descer a lista.
        let label = viewport::position_label(selected, visible, self.rows.len());
        // Coluna extra só para o scrollbar: o texto NUNCA fica embaixo dele.
        let with_scrollbar = label.is_some() && inner.width >= 24;
        let body_width = if with_scrollbar {
            inner.width.saturating_sub(1)
        } else {
            inner.width
        };

        let cols = column_widths(body_width);
        if cols.package == 0 {
            // Sem largura para uma tabela legível: uma linha por achado
            // (severidade + resumo), sem esconder dado. A rolagem vale aqui
            // também — é a única forma de chegar ao fim da lista.
            let (start, end) = viewport::slice(offset, visible, self.rows.len());
            for (line_index, index) in (start..end).enumerate() {
                let row = self.rows[index];
                let marker = if index == selected {
                    self.glyphs.selection()
                } else {
                    "  "
                };
                let body = format!("{marker}{} {}", row.severity, row.message_first_line);
                Paragraph::new(Line::from(Span::styled(
                    body,
                    self.severity_style(&row.severity),
                )))
                .render(
                    Rect {
                        y: inner.y + line_index as u16,
                        height: 1,
                        ..inner
                    },
                    buf,
                );
            }
            return;
        }

        let tools = state::tools_line_refs(self.rows);
        let first_header = match &label {
            Some(label) => viewport::overlay_right(&tools, label, usize::from(body_width)),
            None => tools,
        };
        let header_rows = [
            first_header,
            row_line(
                "",
                &[
                    ("SEVERIDADE", cols.severity),
                    ("FERRAMENTA", cols.tool),
                    ("PACOTE", cols.package),
                    ("LOCAL", cols.location),
                    ("RESUMO", cols.message),
                ],
            ),
        ];
        for (index, line) in header_rows.iter().enumerate() {
            Paragraph::new(Line::from(Span::styled(
                line.clone(),
                Style::default().fg(self.palette.missing),
            )))
            .render(
                Rect {
                    y: inner.y + index as u16,
                    height: 1,
                    width: body_width,
                    ..inner
                },
                buf,
            );
        }

        let row_area = Rect {
            x: inner.x,
            y: inner.y + 2,
            width: body_width,
            height: list_height,
        };
        let (start, end) = viewport::slice(offset, visible, self.rows.len());
        for (line_index, index) in (start..end).enumerate() {
            let Some(row) = self.rows.get(index) else {
                continue;
            };
            let is_selected = index == selected;
            let marker = if is_selected {
                self.glyphs.selection()
            } else {
                "  "
            };
            let package = row.package_line();
            let body = row_line(
                marker,
                &[
                    (&row.severity, cols.severity),
                    (&row.tool, cols.tool),
                    (&package, cols.package),
                    (&row.location(), cols.location),
                    (&row.message_first_line, cols.message),
                ],
            );
            let mut line = Line::from(Span::styled(body, self.severity_style(&row.severity)));
            if is_selected {
                // Destaque que sobrevive a `NO_COLOR`: REVERSED é universal.
                for span in &mut line.spans {
                    span.style = span.style.add_modifier(Modifier::REVERSED);
                }
            }
            Paragraph::new(line).render(
                Rect {
                    y: row_area.y + line_index as u16,
                    height: 1,
                    ..row_area
                },
                buf,
            );
        }

        if with_scrollbar {
            crate::tui::widgets::scrollbar::render(
                buf,
                Rect {
                    x: inner.x + body_width,
                    y: row_area.y,
                    width: 1,
                    height: list_height,
                },
                offset,
                self.rows.len(),
                self.palette,
                self.glyphs,
            );
        }
    }
}

/// O inspetor do achado (`i`/`Enter`): severidade, ferramenta, regra, pacote,
/// LOCAL, fingerprint completo e mensagem inteira — nada truncado aqui.
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
        let block = crate::tui::widgets::panel::block("Detalhe — Esc volta", true, self.palette);
        let inner = block.inner(area);
        block.render(area, buf);
        Paragraph::new(
            self.lines
                .iter()
                .map(|line| {
                    // Dado real do disco recebe a cor de valor; nada de aviso.
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
            rule: "vulnerable".to_string(),
            package: "minimist".to_string(),
            version: "1.2.5".to_string(),
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
        let rows: Vec<&FindingRow> = rows.iter().collect();
        let widget = FindingsWidget::new(
            &rows,
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
    fn table_renders_columns_severities_in_canonical_order_and_selection() {
        let rows = rows_from(vec![
            finding("dotnet", "low", "App.csproj", "pacote deprecado"),
            finding("audit", "high", "package.json", "advisory de segurança"),
            finding("audit", "critical", "src/lib.rs:42", "Prototype Pollution"),
            finding("npm", "medium", "package.json", "revisão pendente"),
        ]);
        let text = render_text(&rows, 0, true, 120, 12);
        assert!(
            text.contains("Achados — critical 1 · high 1 · medium 1 · low 1"),
            "contagem no título: {text}"
        );
        assert!(text.contains("SEVERIDADE"), "{text}");
        assert!(text.contains("FERRAMENTA"), "{text}");
        assert!(text.contains("PACOTE"), "{text}");
        assert!(text.contains("LOCAL"), "{text}");
        assert!(text.contains("RESUMO"), "{text}");
        assert!(text.contains("minimist 1.2.5"), "pacote + versão: {text}");
        assert!(text.contains("➤"), "seleção visível: {text}");
        let critical = text.find("critical").unwrap_or(usize::MAX);
        let high = text.find("high").unwrap_or(usize::MAX);
        let low = text.find("low").unwrap_or(usize::MAX);
        assert!(critical < high, "{text}");
        assert!(high < low, "{text}");
        assert!(text.contains("src/lib.rs:42"), "{text}");
        assert!(text.contains("Prototype Pollution"), "{text}");
    }

    #[test]
    fn narrow_table_drops_the_tool_column_instead_of_crushing_the_summary() {
        let rows = rows_from(vec![finding(
            "audit",
            "critical",
            "package.json",
            "Prototype",
        )]);
        let text = render_text(&rows, 0, true, 80, 8);
        assert!(text.contains("PACOTE"), "{text}");
        assert!(!text.contains("FERRAMENTA"), "80 colunas: {text}");
        assert!(text.contains("Prototype"), "{text}");
        // O dado continua alcançável no inspetor.
        let detail = state::detail_lines(&rows[0]);
        assert!(
            detail
                .iter()
                .any(|line| line.starts_with("ferramenta: audit"))
        );
    }

    #[test]
    fn filter_text_appears_in_the_block_title() {
        let rows = rows_from(vec![finding("audit", "critical", "package.json", "x")]);
        let refs: Vec<&FindingRow> = rows.iter().collect();
        let backend = TestBackend::new(100, 8);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
        let widget = FindingsWidget::new(&refs, 0, true, String::new(), theme::mono())
            .with_filter(Some("critical · audit".to_string()));
        terminal
            .draw(|frame| frame.render_widget(widget, frame.area()))
            .unwrap_or_else(|err| panic!("draw: {err}"));
        let buffer = terminal.backend().buffer();
        let mut text = String::new();
        for x in 0..100 {
            text.push(buffer[(x, 0)].symbol().chars().next().unwrap_or(' '));
        }
        assert!(text.contains("filtro: critical · audit"), "{text}");
    }

    #[test]
    fn long_cells_are_truncated_never_scrolled() {
        let long_message = "mensagem gigante que estouraria qualquer coluna de resumo do painel";
        let long_path = "diretorio/muito/comprido/que/estoura/a/coluna/local/e/nao/role.rs:7";
        let rows = rows_from(vec![finding("audit", "critical", long_path, long_message)]);
        let text = render_text(&rows, 0, true, 40, 6);
        for line in text.lines().skip(2).take(2) {
            let border = line.chars().last().unwrap_or(' ');
            assert_eq!(border, '║', "borda dupla preservada: {line:?}");
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
        assert!(text.contains("Achados"), "título do bloco: {text}");
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
        let backend = TestBackend::new(60, 14);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
        let widget = DetailWidget::new(&detail, theme::mono());
        terminal
            .draw(|frame| frame.render_widget(widget, frame.area()))
            .unwrap_or_else(|err| panic!("draw: {err}"));
        let buffer = terminal.backend().buffer();
        let mut text = String::new();
        for y in 0..14 {
            for x in 0..60 {
                text.push(buffer[(x, y)].symbol().chars().next().unwrap_or(' '));
            }
            text.push('\n');
        }
        assert!(text.contains("severidade: critical"), "{text}");
        assert!(text.contains("ferramenta: audit"), "{text}");
        assert!(text.contains("regra: vulnerable"), "{text}");
        assert!(text.contains("pacote: minimist 1.2.5"), "{text}");
        assert!(text.contains("local: src/lib.rs:42"), "{text}");
        assert!(text.contains("fingerprint: 0123456789abcdef"), "{text}");
        assert!(text.contains("CVE-2020-8203"), "mensagem inteira: {text}");
        assert!(text.contains("Esc volta"), "{text}");
    }

    #[test]
    fn column_widths_never_exceed_the_area() {
        let wide = column_widths(120);
        assert_eq!(wide.severity, 10);
        assert_eq!(wide.tool, 10);
        assert!(wide.message > 20, "{wide:?}");
        let narrow = column_widths(76);
        assert_eq!(narrow.tool, 0);
        assert!(narrow.message > 0, "{narrow:?}");
        let total = narrow.severity + narrow.package + narrow.location + narrow.message;
        assert!(total + 2 + 1 + 3 <= 76, "{narrow:?}");
        // Área minúscula: nada estoura e nada vira pânico.
        let tiny = column_widths(12);
        assert!(tiny.message + tiny.severity <= 12, "{tiny:?}");
        assert_eq!(column_widths(0).message, 0);
    }

    #[test]
    fn column_visibility_counts_what_is_shown() {
        let wide = column_widths(120);
        assert_eq!(wide.visible(), 5);
        let narrow = column_widths(76);
        assert_eq!(narrow.visible(), 4);
    }

    #[test]
    fn cell_truncates_with_ellipsis_and_pads() {
        assert_eq!(cell("abc", 5), "abc  ");
        assert_eq!(cell("abcdef", 5), "abcd…");
        assert_eq!(cell("crítico", 4), "crí…");
    }

    /// Defeito que o viewport corrige: com `j` além da altura visível a
    /// seleção saía da tela. Agora a janela acompanha o cursor.
    #[test]
    fn selection_scrolls_into_view_instead_of_leaving_the_screen() {
        let findings = (0..20)
            .map(|n| {
                finding(
                    "audit",
                    "high",
                    &format!("src/f{n:02}.rs"),
                    &format!("achado {n:02}"),
                )
            })
            .collect();
        let rows = rows_from(findings);
        let selected_message = rows[15].message_first_line.clone();
        let first_message = rows[0].message_first_line.clone();
        let text = render_text(&rows, 15, true, 100, 10);
        assert!(
            text.contains(&selected_message),
            "a linha selecionada está visível: {text}"
        );
        assert!(text.contains("➤"), "marcador visível: {text}");
        assert!(
            !text.contains(&first_message),
            "o topo rolou para fora (janela acompanhou o cursor): {text}"
        );
        // Rótulo de posição e scrollbar só existem quando a lista transborda.
        assert!(text.contains("16/20"), "posição do cursor: {text}");
        assert!(
            text.contains('║') || text.contains('█'),
            "scrollbar desenhado: {text}"
        );
    }

    #[test]
    fn a_list_that_fits_has_no_scrollbar_and_no_position_label() {
        let rows = rows_from(vec![finding("audit", "high", "src/f.rs", "achado")]);
        let text = render_text(&rows, 0, true, 100, 12);
        assert!(!text.contains("1/1"), "sem rótulo inútil: {text}");
        assert!(!text.contains('█'), "sem scrollbar: {text}");
    }

    #[test]
    fn ascii_mode_uses_a_plain_marker_and_plain_borders() {
        use crate::tui::glyph::{Glyphs, Mode, has_special_glyphs};
        let rows = rows_from(vec![
            finding("audit", "high", "src/a.rs", "primeiro"),
            finding("dotnet", "low", "App.csproj", "segundo"),
        ]);
        let refs: Vec<&FindingRow> = rows.iter().collect();
        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
        terminal
            .draw(|frame| {
                frame.render_widget(
                    FindingsWidget::new(&refs, 0, true, String::new(), theme::mono())
                        .with_glyphs(Glyphs::new(Mode::Ascii)),
                    frame.area(),
                );
            })
            .unwrap_or_else(|err| panic!("draw: {err}"));
        let buffer = terminal.backend().buffer();
        let mut text = String::new();
        for y in 0..10 {
            for x in 0..80 {
                text.push(buffer[(x, y)].symbol().chars().next().unwrap_or(' '));
            }
            text.push('\n');
        }
        assert!(text.contains("> "), "marcador em ascii: {text}");
        assert!(!has_special_glyphs(&text), "sem glifo especial: {text}");
    }
}
