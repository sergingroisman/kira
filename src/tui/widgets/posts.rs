//! Caixa de correio (M4, ADR-0038): a lista da tabela `posts` e a leitura do
//! rascunho entregue — tudo sobre o snapshot read-only do `kira digest`.
//!
//! Só pintura: as linhas vêm de [`crate::tui::state::post_rows_from`] e o
//! corpo é o arquivo `.kira/digest/<slug>.md` já lido no snapshot (a fonte de
//! verdade é o disco; o store guarda proveniência e `read_at`). O marcador de
//! **não lido** vive na própria célula do estado (dado visível sem depender de
//! cor, a regra dos chips da W6), e a idade é a função PURA
//! [`crate::tui::state::humanize_age`] sobre o `now` capturado UMA vez no
//! snapshot — nenhum relógio dentro do widget. O preview usa o renderizador
//! nativo [`crate::tui::markdown::render_with`] (sem `tui-markdown`; ADR-0036
//! e o ledger do M4 mandam) e o scroll é clampado pelo tamanho REAL do corpo.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Widget, Wrap};

use crate::tui::glyph::Glyphs;
use crate::tui::state::{self, PostEntry};
use crate::tui::theme::Palette;
use crate::tui::viewport;
use crate::tui::widgets::findings::cell;

/// O widget da LISTA de posts (painel 5).
#[derive(Debug, Clone)]
pub struct PostsWidget<'a> {
    posts: &'a [PostEntry],
    /// `now` do snapshot (unix, s) — o ÚNICO relógio que a pintura enxerga.
    now_secs: Option<i64>,
    selected: usize,
    /// Painel em foco? Borda realçada quando sim, esmaecida quando não.
    focused: bool,
    palette: Palette,
    /// Texto do estado vazio (já calculado em `state::empty_posts_line`).
    empty_line: String,
    /// Conjunto de glifos (`KIRA_GLYPHS=ascii`).
    glyphs: Glyphs,
}

impl<'a> PostsWidget<'a> {
    /// Constrói a partir do snapshot pronto (nunca consulta disco nem store).
    pub fn new(
        posts: &'a [PostEntry],
        now_secs: Option<i64>,
        selected: usize,
        focused: bool,
        empty_line: String,
        palette: Palette,
    ) -> Self {
        Self {
            posts,
            now_secs,
            selected,
            focused,
            palette,
            empty_line,
            glyphs: Glyphs::default(),
        }
    }

    /// Glifos da sessão (o `app` passa o modo lido do ambiente uma vez).
    pub fn with_glyphs(mut self, glyphs: Glyphs) -> Self {
        self.glyphs = glyphs;
        self
    }
}

/// Larguras das colunas sem scroll horizontal: marcador `➤ ` (2) + ESTADO
/// (20 — é a coluna mais longa: `entregue [não lido]`) + SLUG (24) e o resto
/// se divide entre TIPO e CRIADO. Área estreita encolhe tudo, nada estoura.
pub fn column_widths(inner_width: u16) -> (usize, usize, usize, usize) {
    let total = usize::from(inner_width).saturating_sub(2 + 4);
    let state = 20.min(total);
    let slug = 24.min(total.saturating_sub(state));
    let rest = total.saturating_sub(state + slug);
    let kind = 12.min(rest);
    let age = rest.saturating_sub(kind);
    (state, slug, kind, age)
}

impl Widget for PostsWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = match self.posts.len() {
            0 => "Posts".to_string(),
            1 => "Posts — 1 artigo".to_string(),
            n => format!("Posts — {n} artigos"),
        };
        let block =
            crate::tui::widgets::panel::block_with(&title, self.focused, self.palette, self.glyphs);
        let inner = block.inner(area);
        block.render(area, buf);

        if self.posts.is_empty() {
            // Estado vazio honesto: `—` com a causa (sem posts ⇒ `kira digest`).
            Paragraph::new(Line::from(Span::styled(
                self.empty_line.clone(),
                Style::default().fg(self.palette.missing),
            )))
            .block(Block::default())
            .wrap(Wrap { trim: false })
            .render(inner, buf);
            return;
        }

        let selected = self.selected.min(self.posts.len().saturating_sub(1));
        let list_height = inner.height.saturating_sub(1);
        let visible = usize::from(list_height);
        let offset = viewport::keep_visible(selected, visible, self.posts.len());
        let label = viewport::position_label(selected, visible, self.posts.len());
        let with_scrollbar = label.is_some() && inner.width >= 24;
        let body_width = if with_scrollbar {
            inner.width.saturating_sub(1)
        } else {
            inner.width
        };

        let (state_w, slug_w, kind_w, age_w) = column_widths(body_width);
        let marker_w = 2usize;

        let header_line = format!(
            "{}{} {} {} {}",
            cell("", marker_w),
            cell("ESTADO", state_w),
            cell("SLUG", slug_w),
            cell("TIPO", kind_w),
            cell("CRIADO", age_w),
        );
        let header_line = match &label {
            Some(label) => viewport::overlay_right(&header_line, label, usize::from(body_width)),
            None => header_line,
        };
        Paragraph::new(Line::from(Span::styled(
            header_line,
            Style::default().fg(self.palette.missing),
        )))
        .render(
            Rect {
                x: inner.x,
                y: inner.y,
                width: body_width,
                height: 1,
            },
            buf,
        );

        let (start, end) = viewport::slice(offset, visible, self.posts.len());
        for (line_index, post) in self.posts[start..end].iter().enumerate() {
            let rect = Rect {
                x: inner.x,
                y: inner.y + 1 + line_index as u16,
                width: body_width,
                height: 1,
            };
            let is_selected = (start + line_index) == selected;
            let marker = if is_selected {
                self.glyphs.selection()
            } else {
                "  "
            };
            let created = post.created_unix.map_or_else(
                || "—".to_string(),
                |then| state::humanize_age(self.now_secs.unwrap_or(then), then),
            );
            let body = format!(
                "{}{} {} {} {}",
                cell(marker, marker_w),
                cell(&state::post_state_cell(post), state_w),
                cell(&post.slug, slug_w),
                cell(&format!("{} · {}", post.kind, post.lang), kind_w),
                cell(&created, age_w),
            );
            let mut line = Line::from(Span::styled(body, Style::default().fg(self.palette.value)));
            if is_selected {
                // Destaque que sobrevive a `NO_COLOR`: REVERSED.
                for span in &mut line.spans {
                    span.style = span.style.add_modifier(Modifier::REVERSED);
                }
            }
            Paragraph::new(line).render(rect, buf);
        }

        if with_scrollbar {
            crate::tui::widgets::scrollbar::render(
                buf,
                Rect {
                    x: inner.x + body_width,
                    y: inner.y + 1,
                    width: 1,
                    height: list_height,
                },
                offset,
                self.posts.len(),
                self.palette,
                self.glyphs,
            );
        }
    }
}

/// O widget da LEITURA de um post em tela cheia (`Enter` na lista — a tecla
/// que também grava `read_at`, M4). Sem corpo ainda (fila/rota/perdida) a tela
/// diz por quê, com o comando que resolve; nunca texto inventado.
#[derive(Debug, Clone)]
pub struct PostsPageWidget<'a> {
    post: &'a PostEntry,
    scroll: usize,
    palette: Palette,
    glyphs: Glyphs,
}

impl<'a> PostsPageWidget<'a> {
    pub fn new(post: &'a PostEntry, scroll: usize, palette: Palette) -> Self {
        Self {
            post,
            scroll,
            palette,
            glyphs: Glyphs::default(),
        }
    }

    /// Glifos da sessão: o markdown rendido também respeita o modo ASCII.
    pub fn with_glyphs(mut self, glyphs: Glyphs) -> Self {
        self.glyphs = glyphs;
        self
    }
}

impl Widget for PostsPageWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = crate::tui::widgets::panel::block_with(
            &format!("Post: {} — Esc volta", self.post.slug),
            true,
            self.palette,
            self.glyphs,
        );
        let inner = block.inner(area);
        // Tela cheia de verdade: limpa antes de pintar (nada de vazamento).
        Clear.render(inner, buf);
        block.render(area, buf);

        let rendered: Vec<String> = crate::tui::markdown::render_with(
            &strip_draft_header(&self.post.body),
            inner.width as usize,
            self.glyphs,
        );
        if rendered.is_empty() {
            let note = if self.post.state == "delivered" {
                "(rascunho ausente em .kira/digest/ — rode kira digest de novo)"
            } else {
                "(sem corpo: o post ainda não foi digerido)"
            };
            Paragraph::new(Line::from(Span::styled(
                note,
                Style::default().fg(self.palette.missing),
            )))
            .block(Block::default())
            .wrap(Wrap { trim: false })
            .render(inner, buf);
            return;
        }
        let total = rendered.len();
        let visible = usize::from(inner.height).max(1);
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

/// O cabeçalho-comentário do rascunho (`<!-- gerado por kira digest … -->`) é
/// metadado do arquivo, não prosa: a leitura não o mostra — os dados dele já
/// estão no título e nas colunas do painel.
fn strip_draft_header(body: &str) -> String {
    body.lines()
        .filter(|line| !line.trim_start().starts_with("<!--"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::theme;
    use ratatui::{Terminal, backend::TestBackend};

    fn palette() -> Palette {
        theme::mono()
    }

    fn post(slug: &str, state: &str, read_at: Option<&str>) -> PostEntry {
        PostEntry {
            id: format!("uuid-{slug}"),
            slug: slug.to_string(),
            kind: "intro".to_string(),
            lang: "en".to_string(),
            state: state.to_string(),
            read_at: read_at.map(str::to_string),
            path: (state == "delivered").then(|| format!(".kira/wiki/{slug}.md")),
            created_at: "2026-11-10T08:00:00Z".to_string(),
            created_unix: Some(1_762_771_200),
            body: if state == "delivered" {
                format!(
                    "<!-- gerado por `kira digest`. slug: {slug} -->\n\n# O projeto\n\nO que ele faz.\n"
                )
            } else {
                String::new()
            },
        }
    }

    fn render_text(widget: impl Widget, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
        terminal
            .draw(|frame| widget.render(frame.area(), frame.buffer_mut()))
            .expect("desenhar");
        // Por LINHAS (chunks da largura), como no buffer_text da wiki: join
        // sem separador colaria tudo e quebraria asserts de coluna.
        terminal
            .backend()
            .buffer()
            .content()
            .chunks(width as usize)
            .map(|row| {
                row.iter()
                    .map(ratatui::buffer::Cell::symbol)
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn list_renders_state_slug_and_the_unread_marker_in_the_cell() {
        let posts = [
            post("intro", "delivered", None),
            post("c4-container", "sealed", None),
        ];
        let widget = PostsWidget::new(
            &posts,
            Some(1_762_771_200),
            0,
            true,
            state::empty_posts_line(&state::Snapshot::default()),
            palette(),
        );
        let text = render_text(widget, 80, 8);
        assert!(text.contains("ESTADO"), "{text}");
        // O marcador vive NA CÉLULA do estado (regra dos chips): legível sem cor.
        assert!(text.contains("entregue [não lido]"), "{text}");
        assert!(text.contains("intro"), "{text}");
        assert!(text.contains("c4-container"), "{text}");
        assert!(text.contains("na fila"), "{text}");
    }

    #[test]
    fn read_posts_have_no_unread_marker() {
        let posts = [post("intro", "delivered", Some("2026-11-11T08:00:00Z"))];
        let widget = PostsWidget::new(
            &posts,
            Some(1_762_771_200),
            0,
            true,
            String::new(),
            palette(),
        );
        let text = render_text(widget, 80, 6);
        assert!(!text.contains("não lido"), "lido não pode gritar: {text}");
        assert!(text.contains("entregue"), "{text}");
    }

    #[test]
    fn empty_list_renders_the_honest_empty_state() {
        let posts: Vec<PostEntry> = Vec::new();
        let snapshot = state::Snapshot::default();
        let widget = PostsWidget::new(
            &posts,
            snapshot.now_secs,
            0,
            true,
            state::empty_posts_line(&snapshot),
            palette(),
        );
        let text = render_text(widget, 60, 5);
        assert!(text.contains("posts: —"), "{text}");
        assert!(text.contains("kira digest"), "{text}");
    }

    #[test]
    fn page_view_renders_markdown_without_the_draft_header_or_hash_markers() {
        let delivered = post("intro", "delivered", None);
        let widget = PostsPageWidget::new(&delivered, 0, palette());
        let text = render_text(widget, 60, 10);
        assert!(text.contains("O projeto"), "{text}");
        assert!(!text.contains('#'), "markdown cru não vaza: {text}");
        assert!(
            !text.contains("gerado por"),
            "cabeçalho-do-arquivo não é prosa: {text}"
        );
        assert!(text.contains("Post: intro"), "{text}");
    }

    #[test]
    fn page_view_shows_the_honest_note_when_there_is_no_body_yet() {
        let sealed = post("c4-container", "sealed", None);
        let widget = PostsPageWidget::new(&sealed, 0, palette());
        let text = render_text(widget, 60, 6);
        assert!(text.contains("ainda não foi digerido"), "{text}");
    }

    #[test]
    fn page_view_note_distinguishes_a_missing_draft_from_an_undigested_one() {
        let mut delivered = post("intro", "delivered", None);
        delivered.body = String::new();
        let widget = PostsPageWidget::new(&delivered, 0, palette());
        let text = render_text(widget, 60, 6);
        assert!(text.contains(".kira/digest/"), "{text}");
    }

    #[test]
    fn ascii_mode_has_no_special_glyph_in_either_view() {
        let posts = [post("intro", "delivered", None)];
        let glyphs = Glyphs::new(crate::tui::glyph::Mode::Ascii);
        let list = PostsWidget::new(
            &posts,
            Some(1_762_771_200),
            0,
            true,
            String::new(),
            palette(),
        )
        .with_glyphs(glyphs);
        let text = render_text(list, 80, 6);
        assert!(
            !crate::tui::glyph::has_special_glyphs(&text),
            "ascii não pode desenhar glifo especial: {text}"
        );
        let page = PostsPageWidget::new(&posts[0], 0, palette()).with_glyphs(glyphs);
        let text = render_text(page, 70, 8);
        assert!(!crate::tui::glyph::has_special_glyphs(&text), "{text}");
    }

    #[test]
    fn tiny_terminal_still_renders_without_panic() {
        let posts = [post("intro", "delivered", None)];
        let widget = PostsWidget::new(&posts, None, 0, true, String::new(), palette());
        let _ = render_text(widget, 20, 4);
    }

    #[test]
    fn column_widths_never_exceed_the_area() {
        for width in [10u16, 24, 40, 60, 120, 200] {
            let (state_w, slug_w, kind_w, age_w) = column_widths(width);
            let used = 2 + state_w + 1 + slug_w + 1 + kind_w + 1 + age_w;
            assert!(used <= usize::from(width), "{width} → soma {used}");
        }
    }

    #[test]
    fn selection_scrolls_into_view_never_off_the_screen() {
        let posts: Vec<PostEntry> = (0..12)
            .map(|i| post(&format!("p{i}"), "delivered", None))
            .collect();
        let widget = PostsWidget::new(
            &posts,
            Some(1_762_771_200),
            11,
            true,
            String::new(),
            palette(),
        );
        let text = render_text(widget, 80, 6);
        assert!(text.contains("p11"), "último selecionado aparece: {text}");
    }
}
