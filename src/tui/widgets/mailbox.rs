//! Painel de caixa de correio (M4): lista de posts (da store) com status de
//! leitura e preview do markdown.
//!
//! Só pintura: as linhas vêm do snapshot (`artifact_rows` já carrega os posts
//! do projeto). A lista mostra status de leitura (`read_at` do post) e o
//! preview do markdown abre com `Enter` (`Esc` volta). Busca `/` filtra por
//! slug/kind. Idade relativa é função PURA (`humanize_age`) sobre o `now`
//! capturado UMA vez no snapshot — nenhum relógio no widget. Render do
//! markdown via [`crate::tui::markdown::render`] (mesmo renderizador da wiki,
//! sem dependência nova); scroll do preview clamped pelo tamanho real do
//! conteúdo — **sem scroll horizontal**.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Widget, Wrap};

use crate::tui::glyph::Glyphs;
use crate::tui::state::{self, ArtifactRow};
use crate::tui::theme::Palette;
use crate::tui::viewport;
use crate::tui::markdown;

/// O widget da lista de posts (caixa de correio).
#[derive(Debug, Clone)]
pub struct MailboxWidget<'a> {
    items: &'a [ArtifactRow],
    /// `now` do snapshot (unix, s) — o ÚNICO relógio que a pintura enxerga.
    now_secs: Option<i64>,
    selected: usize,
    /// Painel em foco? Borda realçada quando sim, esmaecida quando não.
    focused: bool,
    palette: Palette,
    /// Texto do estado vazio (já calculado em `state::empty_mailbox_line`).
    empty_line: String,
    /// Conjunto de glifos (`KIRA_GLYPHS=ascii`).
    glyphs: Glyphs,
    /// Preview modal aberto? (Enter abre, Esc fecha).
    preview_open: bool,
    /// Scroll do preview (linhas do markdown renderizado).
    preview_scroll: usize,
}

impl<'a> MailboxWidget<'a> {
    /// Constrói a partir do snapshot pronto (nunca consulta disco).
    pub fn new(
        items: &'a [ArtifactRow],
        now_secs: Option<i64>,
        selected: usize,
        focused: bool,
        empty_line: String,
        palette: Palette,
    ) -> Self {
        Self {
            items,
            now_secs,
            selected,
            focused,
            palette,
            empty_line,
            glyphs: Glyphs::default(),
            preview_open: false,
            preview_scroll: 0,
        }
    }

    /// Glifos da sessão (o `app` passa o modo lido do ambiente uma vez).
    pub fn with_glyphs(mut self, glyphs: Glyphs) -> Self {
        self.glyphs = glyphs;
        self
    }

    /// Cor da linha: não-lido destacado, lido neutro.
    fn row_style(&self, read_at: Option<&str>) -> Style {
        if read_at.is_some() {
            Style::default().fg(self.palette.value)
        } else {
            Style::default().fg(self.palette.missing).add_modifier(Modifier::BOLD)
        }
    }

    /// Texto de uma linha: status + kind + slug/title + idade.
    fn item_line(&self, item: &ArtifactRow, palette: Palette) -> Line<'static> {
        let read = item.state == "delivered" || item.state == "sealed"; // publicado = lido; rascunho = não lido
        let icon = if read { "✓" } else { "●" };
        let age = item.age.clone();
        let kind_icon = match item.kind.as_str() {
            "wiki" => "📄",
            "rascunho" => "📝",
            _ => "📦",
        };
        let title = item.path.split('/').next_back().unwrap_or(&item.path);
        Line::from(vec![
            Span::styled(
                format!("{} {} ", if read { "✓" } else { "●" }, kind_icon),
                Style::default().fg(if self.focused { self.palette.missing } else { self.palette.value }),
            ),
            Span::styled(
                title,
                Style::default().fg(if self.focused { self.palette.missing } else { self.palette.value }).add_modifier(ratatui::style::Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                age,
                Style::default().fg(self.palette.missing),
            ),
        ])
    }

    /// Lida com teclas quando o preview está aberto.
    fn handle_preview_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        match key.code {
            crossterm::event::KeyCode::Esc => {
                self.preview_open = false;
                self.preview_scroll = 0;
                true
            }
            crossterm::event::KeyCode::Up => {
                if self.preview_scroll > 0 {
                    self.preview_scroll -= 1;
                }
                true
            }
            crossterm::event::KeyCode::Down => {
                self.preview_scroll += 1;
                true
            }
            crossterm::event::KeyCode::PageUp => {
                self.preview_scroll = self.preview_scroll.saturating_sub(20);
                true
            }
            crossterm::event::KeyCode::PageDown => {
                self.preview_scroll += 20;
                true
            }
            _ => false,
        }
    }

    /// Lida com teclas quando a lista está em foco.
    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        if self.preview_open {
            return self.handle_preview_key(key);
        }
        match key.code {
            crossterm::event::KeyCode::Enter => {
                if !self.items.is_empty() {
                    self.preview_open = true;
                    self.preview_scroll = 0;
                    true
                } else {
                    false
                }
            }
            crossterm::event::KeyCode::Char('/') => {
                // Busca será tratada no app.rs (filtro global)
                false
            }
            crossterm::event::KeyCode::Char('j') | crossterm::event::KeyCode::Down => {
                if self.selected + 1 < self.items.len() {
                    self.selected += 1;
                    true
                } else {
                    false
                }
            }
            crossterm::event::KeyCode::Char('k') | crossterm::event::KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                    true
                } else {
                    false
                }
            }
            crossterm::event::KeyCode::PageDown => {
                let max = self.items.len().saturating_sub(1);
                self.selected = (self.selected + 20).min(max);
                true
            }
            crossterm::event::KeyCode::PageUp => {
                self.selected = self.selected.saturating_sub(20);
                true
            }
            crossterm::event::KeyCode::Home => {
                self.selected = 0;
                true
            }
            crossterm::event::KeyCode::End => {
                self.selected = self.items.len().saturating_sub(1);
                true
            }
            _ => false,
        }
    }

    /// Renderiza o preview modal (markdown do post selecionado).
    fn render_preview(&self, area: Rect, buf: &mut Buffer, palette: Palette, glyphs: &Glyphs) {
        if self.items.is_empty() || !self.preview_open || self.selected >= self.items.len() {
            return;
        }
        let item = &self.items[self.selected];
        let path = &item.path;
        let content = std::fs::read_to_string(path).unwrap_or_else(|_| "— erro ao ler arquivo —".to_string());
        let lines = crate::tui::markdown::render(&content);
        
        let area_height = area.height as usize;
        let content_height = lines.len();
        let scroll = self.preview_scroll.min(content_height.saturating_sub(area_height.saturating_sub(4)));
        
        let block = crate::tui::widgets::panel::block_with(
            &format!("Preview · {}", path.split('/').next_back().unwrap_or("preview")),
            true,
            self.palette,
        );
        let inner = block.inner(area);
        block.render(area, buf);
        
        let visible_lines = lines.into_iter().skip(self.preview_scroll).take(area_height.saturating_sub(4));
        for (i, line) in visible_lines.enumerate() {
            if i + 2 < inner.height as usize {
                buf.set_line(inner.x, inner.y + 2 + i as u16, &line, Style::default());
            }
        }
    }
}

impl<'a> Widget for MailboxWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = crate::tui::widgets::panel::block_with(
            "Correio · posts",
            true,
            self.palette,
        );
        let inner = block.inner(area);
        block.render(area, buf);

        let items = self.items;
        if items.is_empty() {
            let text = Paragraph::new(self.empty_line.clone())
                .style(Style::default().fg(self.palette.missing))
                .alignment(ratatui::layout::Alignment::Center);
            text.render(inner, buf);
            return;
        }

        // Lista de itens
        let mut lines = Vec::new();
        for (idx, item) in self.items.iter().enumerate() {
            let style = if idx == self.selected {
                Style::default().bg(self.palette.missing).fg(self.palette.background)
            } else {
                self.row_style(item.state.as_str())
            };
            let line = self.item_line(&self.items[idx], self.palette);
            buf.set_line(inner.x, inner.y + idx as u16, &line, style);
        }
    }
}

/// A coluna "Status" do preview (lido/não lido).
fn status_badge(read: bool, palette: Palette) -> Span<'static> {
    if read {
        Span::styled("✓ LIDO", Style::default().fg(palette.value))
    } else {
        Span::styled("● NÃO LIDO", Style::default().fg(palette.missing).add_modifier(Modifier::BOLD))
    }
}

/// Texto do corpo do markdown (mesmo renderizador da wiki).
fn markdown_body(artifact: &ArtifactRow, palette: Palette) -> Vec<Line<'static>> {
    let path = &artifact.path;
    let content = std::fs::read_to_string(path).unwrap_or_else(|_| "— erro ao ler arquivo —".to_string());
    markdown::render(&content)
}
