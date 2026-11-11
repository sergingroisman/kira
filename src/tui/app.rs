//! Loop de eventos do `kira dashboard` — o coração da casca (ADR-0014).
//!
//! Ciclo: draw (ratatui) → poll de tecla (crossterm) → ação (keymap) →
//! mutação mínima de estado de tela (painel/scroll/ajuda) ou releitura do
//! disco (`r`). Nenhuma ação escreve em disco, rede ou LLM; o snapshot é
//! recomposto apenas em `Reload` — nunca por frame.
//!
//! A tela NUNCA é deixada suja: o guard [`ScreenGuard`] devolve raw
//! mode/alternate screen no Drop, mesmo em erro de desenho no meio do loop.

use std::io::{self};

use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::tui::keymap::{self, Action};
use crate::tui::state::{self, Snapshot};
use crate::tui::theme::{self, Palette};
use crate::tui::widgets::header::HeaderWidget;

/// Tamanho mínimo para o layout denso (spec W6). Abaixo disso a casca degrada
/// para lista única com aviso claro — nunca quebra.
const MIN_WIDTH: u16 = 80;
const MIN_HEIGHT: u16 = 24;

/// Guard de terminal: entra raw mode + alternate screen na criação e desfaz
/// TUDO no Drop — inclusive quando o loop sai por erro.
struct ScreenGuard;

impl ScreenGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        Ok(Self)
    }
}

impl Drop for ScreenGuard {
    fn drop(&mut self) {
        // Melhor esforço duplo: se um dos passos falhar, o outro ainda roda —
        // terminal sujo depois de um erro é pior que um aviso perdido.
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

/// Roda o modo interativo (só é chamado com stdout TTY). Erro é texto
/// acionável, nunca panic.
pub fn run(initial: Snapshot) -> Result<(), String> {
    let _guard = ScreenGuard::enter().map_err(|err| {
        format!(
            "não foi possível entrar no modo de tela do dashboard: {err} — \
             rode `kira dashboard --snapshot` para a versão de texto"
        )
    })?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))
        .map_err(|err| format!("não foi possível preparar a saída do dashboard: {err}"))?;

    // Estado de tela (não de domínio): scroll da lista e ajuda. Não existe
    // ainda um índice de painel focado: só o cabeçalho está nesta PR.
    let palette = theme::palette_from_env();
    let mut selected: usize = 0;
    let mut show_help = false;
    let mut snapshot = initial;

    loop {
        // Draw: cabeçalho + rodapé (e overlay de ajuda por cima, se aberto).
        let lines = state::header_lines(&snapshot);
        let footer = format!("painel 1/1 · {}", state::footer_line());
        terminal
            .draw(|frame| {
                let area = frame.area();
                let title = snapshot.project.as_ref().map(|project| project.name.as_str());
                let degraded = area.width < MIN_WIDTH || area.height < MIN_HEIGHT;
                let rows = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(1)])
                    .split(area);
                if degraded {
                    // Lista única com aviso: mesma informação, sem layout denso.
                    let mut narrow = vec![format!(
                        "aviso: terminal {}×{} menor que {MIN_WIDTH}×{MIN_HEIGHT} — layout de lista única",
                        area.width, area.height
                    )];
                    narrow.extend(lines.iter().cloned());
                    frame.render_widget(
                        Paragraph::new(
                            narrow
                                .iter()
                                .map(|line| {
                                    ratatui::text::Line::from(Span::styled(
                                        line.clone(),
                                        Style::default().fg(palette.value),
                                    ))
                                })
                                .collect::<Vec<_>>(),
                        )
                        .block(Block::default().borders(Borders::ALL))
                        // `selected` vira scroll da lista única: as teclas j/k
                        // continuam fazendo algo útil na degradação.
                        .scroll((
                            u16::try_from(selected.min(narrow.len().saturating_sub(1)))
                                .unwrap_or(u16::MAX),
                            0,
                        )),
                        rows[0],
                    );
                } else {
                    frame.render_widget(HeaderWidget::new(&lines, title, palette), rows[0]);
                }
                frame.render_widget(
                    Paragraph::new(Span::styled(
                        footer.clone(),
                        Style::default().fg(palette.footer),
                    )),
                    rows[1],
                );
                if show_help {
                    frame.render_widget(help_paragraph(&palette), area);
                }
            })
            .map_err(|err| format!("falha ao desenhar o dashboard: {err}"))?;

        // Poll: ação por tecla; sem tecla, nada acontece (sem spinner eterno —
        // a persona proíbe; o dashboard só muda quando o mundo muda).
        let action = match event::read().map_err(|err| format!("falha ao ler teclado: {err}"))? {
            Event::Key(key) => keymap::map(key),
            _ => None,
        };
        match action {
            Some(Action::Quit) => return Ok(()),
            // Nesta PR só existe o cabeçalho (PANEL_COUNT implícito = 1):
            // Tab/Shift+Tab são aceitos e não mudam o foco — os painéis dos
            // PRs seguintes entram no mesmo ciclo sem mexer no keymap.
            Some(Action::NextPanel) | Some(Action::PrevPanel) => {}
            Some(Action::Down) => selected = selected.saturating_add(1),
            Some(Action::Up) => selected = selected.saturating_sub(1),
            Some(Action::Top) => selected = 0,
            // O render satura o scroll pela quantidade real de linhas.
            Some(Action::Bottom) => selected = usize::MAX,
            Some(Action::Reload) => {
                // `r` = reler o disco (read-only), nunca outra coisa.
                snapshot = state::load();
            }
            Some(Action::Help) => show_help = !show_help,
            None => {}
        }
    }
}

/// Overlay de ajuda: os mesmos atalhos do rodapé, com o que cada um faz.
fn help_paragraph(palette: &Palette) -> Paragraph<'static> {
    let text = [
        "ajuda do kira dashboard (read-only — nenhuma tecla escreve)".to_string(),
        String::new(),
        "q                sair".to_string(),
        "Tab / Shift+Tab  ciclar painel".to_string(),
        "j / k ou ↑ / ↓   mover linha".to_string(),
        "g / G            topo / fim".to_string(),
        "r                reler o disco (sem rede, sem escrita)".to_string(),
        "?                abrir/fechar esta ajuda".to_string(),
        String::new(),
        "Dado ausente aparece como — com a causa; nada é inventado.".to_string(),
    ];
    Paragraph::new(
        text.into_iter()
            .map(|line| {
                ratatui::text::Line::from(Span::styled(line, Style::default().fg(palette.help)))
            })
            .collect::<Vec<_>>(),
    )
    .block(Block::default().borders(Borders::ALL).title(" ajuda "))
}
