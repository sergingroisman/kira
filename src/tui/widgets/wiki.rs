//! Painel de wiki (W6-PR4): lista de páginas (índice `wiki_page` da store) +
//! preview do markdown renderizado em tela cheia (`Enter` abre, `Esc` volta).
//!
//! Só pintura, como os outros painéis: as páginas vêm de
//! [`crate::tui::state::wiki_rows_from`] via snapshot e a lista exibida é a
//! FILTRADA por [`crate::tui::state::filter_pages`] (busca `/`). A idade
//! relativa é função PURA ([`crate::tui::state::humanize_age`]) sobre o `now`
//! capturado UMA vez no snapshot — nenhum relógio dentro do widget. O markdown
//! rende por [`crate::tui::markdown::render`] (renderizador próprio, sem
//! dependência nova); o scroll do preview é clamped pelo tamanho REAL do
//! conteúdo (nunca desvaza a tela) — **sem scroll horizontal**.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap};

use crate::tui::state::{self, WikiPage};
use crate::tui::theme::Palette;
use crate::tui::widgets::findings::cell;

/// O widget da LISTA de páginas da wiki.
#[derive(Debug, Clone)]
pub struct WikiWidget<'a> {
    pages: &'a [&'a WikiPage],
    /// `now` do snapshot (unix, s) — o ÚNICO relógio que a pintura enxerga.
    now_secs: Option<i64>,
    selected: usize,
    /// Painel em foco? Borda realçada quando sim, esmaecida quando não.
    focused: bool,
    palette: Palette,
    /// Texto do estado vazio (já calculado em `state::empty_wiki_line`).
    empty_line: String,
}

impl<'a> WikiWidget<'a> {
    /// Constrói a partir do snapshot pronto (nunca consulta disco).
    pub fn new(
        pages: &'a [&'a WikiPage],
        now_secs: Option<i64>,
        selected: usize,
        focused: bool,
        empty_line: String,
        palette: Palette,
    ) -> Self {
        Self {
            pages,
            now_secs,
            selected,
            focused,
            palette,
            empty_line,
        }
    }
}

/// Larguras das colunas da lista a partir da área interna, SEM scroll
/// horizontal: marcador `➤ ` (2) + TIPO (8) + TÍTULO (teto 30) e a
/// ATUALIZAÇÃO leva o resto. Em área estreita tudo encolhe, nada estoura.
pub fn column_widths(inner_width: u16) -> (usize, usize, usize) {
    let total = usize::from(inner_width).saturating_sub(2 + 3);
    let kind = 8.min(total);
    let title = 30.min(total.saturating_sub(kind));
    let age = total.saturating_sub(kind + title);
    (kind, title, age)
}

impl Widget for WikiWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = match self.pages.len() {
            0 => " wiki ".to_string(),
            1 => " wiki — 1 página ".to_string(),
            n => format!(" wiki — {n} páginas "),
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

        if self.pages.is_empty() {
            // Estado vazio honesto: `—` com a causa (índice vazio ⇒ reindex).
            Paragraph::new(Line::from(Span::styled(
                self.empty_line.clone(),
                Style::default().fg(self.palette.missing),
            )))
            .block(Block::default())
            .wrap(Wrap { trim: false })
            .render(inner, buf);
            return;
        }

        let (kind_w, title_w, age_w) = column_widths(inner.width);
        let marker_w = 2usize;
        let selected = self.selected.min(self.pages.len().saturating_sub(1));

        // Cabeçalho das colunas: uma linha fixa no topo da lista.
        let header_line = format!(
            "{}{} {} {}",
            cell("", marker_w),
            cell("TIPO", kind_w),
            cell("TÍTULO", title_w),
            cell("ATUALIZAÇÃO", age_w),
        );
        Paragraph::new(Line::from(Span::styled(
            header_line,
            Style::default().fg(self.palette.missing),
        )))
        .render(inner, buf);

        // Linhas da lista: y fixo por índice — tabela de verdade, não fluxo.
        let row_area = Rect {
            x: inner.x,
            y: inner.y + 1,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };
        for (index, page) in self.pages.iter().enumerate() {
            if index as u16 >= row_area.height {
                break; // o que não couber espera o terminal maior (denso).
            }
            let is_selected = index == selected;
            let marker = if is_selected { "➤ " } else { "  " };
            let body = format!(
                "{}{} {} {}",
                cell(marker, marker_w),
                cell(&page.kind, kind_w),
                cell(&page.title, title_w),
                cell(&state::wiki_age_line(page, self.now_secs), age_w),
            );
            let mut line = Line::from(Span::styled(body, Style::default().fg(self.palette.value)));
            if is_selected {
                // Destaque de seleção que sobrevive a `NO_COLOR`: REVERSED.
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

/// O widget do PREVIEW de página em tela cheia (`Enter` na lista).
///
/// O scroll é clamped aqui: a quantidade real de linhas rendidas depende da
/// largura interna (só o widget conhece), então o clamp mora no widget —
/// `G` (scroll enorme) satura na última página visível, nunca além.
#[derive(Debug, Clone)]
pub struct WikiPageWidget<'a> {
    page: &'a WikiPage,
    scroll: usize,
    palette: Palette,
}

impl<'a> WikiPageWidget<'a> {
    pub fn new(page: &'a WikiPage, scroll: usize, palette: Palette) -> Self {
        Self {
            page,
            scroll,
            palette,
        }
    }
}

impl Widget for WikiPageWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                format!(" wiki: {} — Esc volta ", self.page.title),
                Style::default()
                    .fg(self.palette.title)
                    .add_modifier(Modifier::BOLD),
            ))
            .border_style(Style::default().fg(self.palette.border_focus));
        let inner = block.inner(area);
        // Tela cheia de verdade: limpa a área ANTES de pintar — as linhas
        // vazias do markdown não podem deixar o painel de fundo vazar.
        Clear.render(inner, buf);
        block.render(area, buf);
        let rendered = crate::tui::markdown::render(&self.page.content, inner.width as usize);
        if rendered.is_empty() {
            // Página ilegível/ausente no disco: causa honesta, nunca texto
            // inventado (o índice pode estar velho — `kira wiki reindex`).
            Paragraph::new(Line::from(Span::styled(
                "(conteúdo ausente em .kira/wiki/ — rode kira wiki reindex)",
                Style::default().fg(self.palette.missing),
            )))
            .block(Block::default())
            .wrap(Wrap { trim: false })
            .render(inner, buf);
            return;
        }
        let total = rendered.len();
        let visible = usize::from(inner.height).max(1);
        // Clamp: scroll máximo = total - visível (nunca além do conteúdo).
        let scroll = self.scroll.min(total.saturating_sub(visible));
        let lines: Vec<Line> = rendered
            .iter()
            .map(|line| {
                Line::from(Span::styled(
                    line.clone(),
                    Style::default().fg(self.palette.value),
                ))
            })
            .collect();
        let scroll_u16 = u16::try_from(scroll).unwrap_or(u16::MAX);
        Paragraph::new(lines)
            .block(Block::default())
            .scroll((scroll_u16, 0))
            .render(inner, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::theme;
    use ratatui::{Terminal, backend::TestBackend};

    /// Página de referência (mesma forma de `state::wiki_rows_from`).
    fn page(
        slug: &str,
        kind: &str,
        title: &str,
        updated_unix: Option<i64>,
        content: &str,
    ) -> WikiPage {
        WikiPage {
            slug: slug.to_string(),
            kind: kind.to_string(),
            title: title.to_string(),
            path: String::new(),
            updated_at: String::new(),
            updated_unix,
            content: content.to_string(),
        }
    }

    /// Renderiza num buffer de teste e devolve o texto (smith determinístico).
    fn render_text(
        pages: &[&WikiPage],
        now_secs: Option<i64>,
        selected: usize,
        focused: bool,
        empty_line: &str,
        width: u16,
        height: u16,
    ) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
        let widget = WikiWidget::new(
            pages,
            now_secs,
            selected,
            focused,
            empty_line.to_string(),
            theme::mono(),
        );
        terminal
            .draw(|frame| frame.render_widget(widget, frame.area()))
            .unwrap_or_else(|err| panic!("draw: {err}"));
        buffer_text(&terminal, width, height)
    }

    /// Texto do buffer do terminal de teste.
    fn buffer_text(terminal: &Terminal<TestBackend>, width: u16, height: u16) -> String {
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
    fn list_renders_kind_title_and_relative_age_with_selection() {
        let p1 = page(
            "contexto",
            "contexto",
            "Contexto do projeto",
            Some(1_762_861_200),
            "conteúdo",
        );
        let p2 = page(
            "bugs/4821",
            "bug",
            "Sessão 4821",
            Some(1_762_861_200),
            "outro",
        );
        // now = stamp + 180s ⇒ "há 3 min" determinístico.
        let pages = vec![&p1, &p2];
        let text = render_text(&pages, Some(1_762_861_200 + 180), 0, true, "", 100, 10);
        assert!(text.contains("wiki — 2 páginas"), "{text}");
        assert!(text.contains("TIPO"), "{text}");
        assert!(text.contains("TÍTULO"), "{text}");
        assert!(text.contains("ATUALIZAÇÃO"), "{text}");
        assert!(text.contains("Contexto do projeto"), "{text}");
        assert!(text.contains("bug"), "{text}");
        assert!(text.contains("há 3 min"), "{text}");
        assert!(text.contains("➤"), "seleção visível: {text}");
    }

    #[test]
    fn empty_list_renders_the_honest_empty_state() {
        let text = render_text(
            &[],
            None,
            0,
            true,
            "wiki: — (wiki vazia — rode kira wiki reindex)",
            50,
            5,
        );
        assert!(
            text.contains("wiki: — (wiki vazia — rode kira wiki reindex)"),
            "{text}"
        );
        assert!(text.contains("wiki "), "título do bloco presente: {text}");
    }

    #[test]
    fn tiny_terminal_still_renders_without_panic() {
        let p1 = page("x", "bug", "X", None, "c");
        let pages = vec![&p1];
        let text = render_text(&pages, None, 0, false, "", 20, 4);
        assert!(!text.is_empty(), "{text}");
    }

    #[test]
    fn column_widths_never_exceed_the_area() {
        let (kind, title, age) = column_widths(100);
        assert_eq!(kind, 8);
        assert_eq!(title, 30);
        assert!(age > 0);
        let (kind, title, age) = column_widths(12);
        assert!(kind + title + age <= 12, "{kind} {title} {age}");
        let (kind, title, age) = column_widths(0);
        assert_eq!(kind + title + age, 0);
    }

    #[test]
    fn page_view_renders_markdown_without_hash_or_bold_and_clamps_scroll() {
        // Página mais alta que a área: o scroll tem que saturar na última
        // linha visível, nunca além do conteúdo.
        let content = (1..=40)
            .map(|n| format!("linha {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let p1 = page(
            "contexto",
            "contexto",
            "Contexto",
            Some(1_762_861_200),
            &content,
        );
        let render_page = |scroll: usize, width: u16, height: u16| {
            let backend = TestBackend::new(width, height);
            let mut terminal =
                Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
            let widget = WikiPageWidget::new(&p1, scroll, theme::mono());
            terminal
                .draw(|frame| frame.render_widget(widget, frame.area()))
                .unwrap_or_else(|err| panic!("draw: {err}"));
            buffer_text(&terminal, width, height)
        };
        // Topo: título do bloco + primeiras linhas rendidas.
        let top = render_page(0, 80, 12);
        assert!(top.contains("wiki: Contexto — Esc volta"), "{top}");
        assert!(top.contains("linha 1"), "{top}");
        // Fim (scroll gigante de `G`): as ÚLTIMAS linhas, nunca além.
        let bottom = render_page(usize::MAX, 80, 12);
        assert!(
            bottom.contains("linha 40"),
            "última linha no fundo: {bottom}"
        );
        assert!(
            !bottom.contains("linha 1\n"),
            "topo rolou para fora: {bottom}"
        );
    }

    #[test]
    fn page_view_shows_honest_note_when_markdown_is_missing() {
        let p1 = page("ghost", "bug", "Fantasma", None, "");
        let backend = TestBackend::new(60, 6);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
        let widget = WikiPageWidget::new(&p1, 0, theme::mono());
        terminal
            .draw(|frame| frame.render_widget(widget, frame.area()))
            .unwrap_or_else(|err| panic!("draw: {err}"));
        let text = buffer_text(&terminal, 60, 6);
        assert!(text.contains("conteúdo ausente"), "{text}");
        assert!(text.contains("wiki: Fantasma — Esc volta"), "{text}");
    }

    #[test]
    fn page_view_never_leaks_raw_markdown_markers() {
        let p1 = page(
            "contexto",
            "contexto",
            "C",
            None,
            "# Título\n\n- item\n\n**negrito** [link](https://x)\n\n```rust\ncódigo\n```\n\n> citação\n\n---",
        );
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
        let widget = WikiPageWidget::new(&p1, 0, theme::mono());
        terminal
            .draw(|frame| frame.render_widget(widget, frame.area()))
            .unwrap_or_else(|err| panic!("draw: {err}"));
        let text = buffer_text(&terminal, 80, 20);
        // Renderizado: marcadores por nível, bala, ênfase desligada, link
        // expandido, barra de citação, régua — e nenhum `#`/`**`/`` ` `` cru.
        assert!(text.contains("▌ Título"), "{text}");
        assert!(text.contains("• item"), "{text}");
        assert!(text.contains("negrito"), "{text}");
        assert!(text.contains("link (https://x)"), "{text}");
        assert!(text.contains("│ código"), "{text}");
        assert!(text.contains("│ citação"), "{text}");
        assert!(text.contains("────"), "{text}");
        assert!(!text.contains("**"), "{text}");
        assert!(!text.contains('`'), "{text}");
    }
}
