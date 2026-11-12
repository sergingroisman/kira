//! Loop de eventos do `kira dashboard` — o coração da casca (ADR-0014).
//!
//! Ciclo: draw (ratatui) → poll de tecla (crossterm) → [`decide`] → mutação
//! mínima de estado de tela (painel/scroll/detalhe/ajuda) ou releitura do
//! disco (`r`). Nenhuma ação escreve em disco, rede ou LLM; o snapshot é
//! recomposto apenas em `Reload` — nunca por frame.
//!
//! A decisão está EXTRAÍDA em função pura ([`decide`] + [`apply_move`] +
//! [`next_panel`]): a cobertura de `tui/app.rs` estava em 0% no W6-PR1 porque
//! o `match` vivia dentro do loop impensável — agora cada transição é
//! unit-testável sem terminal (meta do W6-PR2: >60% de linhas).
//!
//! A tela NUNCA é deixada suja: o guard [`ScreenGuard`] devolve raw
//! mode/alternate screen no Drop, mesmo em erro de desenho no meio do loop.

use std::io::{self};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::tui::keymap::{self, Action};
use crate::tui::state::{self, Snapshot, WikiPage};
use crate::tui::theme::{self, Palette};
use crate::tui::widgets::findings::{DetailWidget, FindingsWidget};
use crate::tui::widgets::header::HeaderWidget;
use crate::tui::widgets::sessions::SessionsWidget;
use crate::tui::widgets::wiki::{WikiPageWidget, WikiWidget};

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

/// Painel em foco (W6-PR4: os QUATRO da spec).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    /// Cabeçalho: identidade do projeto e saúde básica (W6-PR1).
    Header,
    /// Tabela de achados de `findings.json` (W6-PR2).
    Findings,
    /// Tabela das últimas 50 sessões da store (W6-PR3).
    Sessions,
    /// Lista de páginas da wiki + busca `/` + preview em tela cheia (W6-PR4).
    Wiki,
}

impl Panel {
    /// Posição do painel no ciclo (1-based para o rodapé `painel 1/4`).
    pub fn index(self) -> usize {
        match self {
            Panel::Header => 0,
            Panel::Findings => 1,
            Panel::Sessions => 2,
            Panel::Wiki => 3,
        }
    }
}

/// Quantidade de painéis do ciclo de Tab nesta PR.
pub const PANEL_COUNT: usize = 4;

/// Estado de TELA (não de domínio): o que o loop muta por tecla. O snapshot de
/// dados vive à parte (`Snapshot`) e só muda em `Reload`.
///
/// NÃO é `Copy` (a agulha da busca é `String`) — o loop sempre passa por
/// referência e muta campos individualmente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppState {
    /// Painel em foco.
    pub panel: Panel,
    /// Linha selecionada (tabela de achados ou scroll da lista degradada).
    pub selected: usize,
    /// Detalhe de achado aberto (Enter abre, Esc volta).
    pub detail_open: bool,
    /// Overlay de ajuda aberto (?).
    pub help_open: bool,
    /// Quantidade de linhas de achados que o loop enxerga — decide precisa
    /// dela para saber se `Enter` tem o que abrir (lista vazia não abre).
    pub findings_len: usize,
    /// Quantidade de linhas de sessões que o loop enxerga — idem, para o
    /// `Enter` do painel de sessões (W6-PR3).
    pub sessions_len: usize,
    /// Quantidade de páginas FILTRADAS que o loop enxerga no painel de wiki
    /// (W6-PR4) — o `Enter` precisa saber se há página para abrir e o clamp
    /// da seleção usa a lista filtrada, não a do índice inteiro.
    pub wiki_len: usize,
    /// Busca incremental da wiki aberta (`/`): as teclas viram texto no
    /// rodapé até `Esc` cancelar.
    pub wiki_search_open: bool,
    /// Agulha da busca incremental (aplicada por `state::filter_pages`).
    pub wiki_query: String,
    /// Preview de página aberto em tela cheia (`Enter` abre, `Esc` volta).
    pub page_open: bool,
    /// Scroll vertical do preview (linhas; o widget clampa pelo tamanho real).
    pub preview_scroll: usize,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            panel: Panel::Header,
            selected: 0,
            detail_open: false,
            help_open: false,
            findings_len: 0,
            sessions_len: 0,
            wiki_len: 0,
            wiki_search_open: false,
            wiki_query: String::new(),
            page_open: false,
            preview_scroll: 0,
        }
    }
}

/// O que o loop executa depois de [`decide`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Sair do dashboard.
    Quit,
    /// Redesenhar sem mudar estado (Ctrl+L, clássico de shell).
    Redraw,
    /// Reler o disco e recompor o snapshot (`r`) — sem rede, sem escrita.
    Reload,
    /// Abrir/fechar o detalhe do achado (Enter/Esc).
    ToggleDetail,
    /// Mover a seleção (+1 desce; `isize::MIN`/`MAX` = topo/fim de `g`/`G`).
    Move(isize),
    /// Ciclar painel (+1 = Tab, -1 = Shift+Tab), com wrap.
    CyclePanel(i32),
    /// Abrir a busca incremental da wiki (`/`).
    SearchWiki,
    /// Digitar um caractere na busca aberta (query é estado de tela).
    SearchChar(char),
    /// Apagar o último caractere da busca (Backspace).
    SearchBackspace,
    /// Fechar a busca (Esc cancela a agulha).
    CloseSearch,
    /// Abrir a página selecionada da wiki em tela cheia (Enter).
    OpenPage,
    /// Fechar o preview em tela cheia (Esc volta à lista).
    ClosePage,
    /// Rolar o preview em tela cheia (j/k por linha, PgUp/PgDn por página).
    ScrollPage(isize),
    /// Alternar o overlay de ajuda (?).
    Help,
    /// Tecla ignorada (nada a fazer).
    Ignore,
}

/// Próximo painel do ciclo, com wrap nas duas direções. Com 4 painéis o ciclo
/// é cabeçalho → achados → sessões → wiki (W6-PR4).
pub fn next_panel(panel: Panel, direction: i32) -> Panel {
    let index = panel.index();
    let count = PANEL_COUNT;
    let next = (index as i32 + direction).rem_euclid(count as i32) as usize;
    match next {
        1 => Panel::Findings,
        2 => Panel::Sessions,
        3 => Panel::Wiki,
        _ => Panel::Header,
    }
}

/// Aplica um offset de movimento com saturação nas duas pontas: `g`/`G`
/// chegam como `isize::MIN`/`MAX` e nunca desvaziam o `usize`.
pub fn apply_move(selected: usize, offset: isize) -> usize {
    if offset < 0 {
        selected.saturating_sub(offset.unsigned_abs())
    } else {
        selected.saturating_add(offset as usize)
    }
}

/// DECISÃO PURA da tecla: estado de tela + `KeyEvent` → [`Outcome`].
///
/// Toda a semântica de interação do loop mora aqui, sem terminal, sem I/O —
/// o loop só executa o outcome. Setas E vim (persona §7) chegam pelo
/// `keymap`; Enter/Esc/TAB têm semântica dependente do estado (painel em
/// foco, detalhe aberto) e por isso são decididos AQUI, não no keymap sem
/// estado do W6-PR1. Nenhuma tecla escreve: não existe outcome de escrita.
pub fn decide(state: &AppState, key: KeyEvent) -> Outcome {
    // Busca aberta é modal: toda tecla vira texto no rodapé até `Esc`
    // cancelar (Enter mantém a agulha — o filtro já é incremental). Só
    // caracteres sem CONTROL entram; SHIFT entra (maiúscula é busca).
    if state.wiki_search_open {
        return match key.code {
            KeyCode::Esc => Outcome::CloseSearch,
            KeyCode::Backspace => Outcome::SearchBackspace,
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                Outcome::SearchChar(c)
            }
            _ => Outcome::Ignore,
        };
    }
    // Preview de página aberto é modal: Esc volta à lista, j/k/PgUp/PgDn
    // rolam, `q` sai do app e `r` relê o disco (os mesmos dois serviços do
    // modal de detalhe). Mover/Tab por baixo do preview confundiria o foco.
    if state.page_open {
        return match key.code {
            KeyCode::Esc => Outcome::ClosePage,
            KeyCode::Char('q') => Outcome::Quit,
            KeyCode::Char('r') => Outcome::Reload,
            KeyCode::Char('j') | KeyCode::Down | KeyCode::PageDown => {
                Outcome::ScrollPage(if key.code == KeyCode::PageDown {
                    SCROLL_PAGE_STEP
                } else {
                    1
                })
            }
            KeyCode::Char('k') | KeyCode::Up | KeyCode::PageUp => {
                Outcome::ScrollPage(if key.code == KeyCode::PageUp {
                    -SCROLL_PAGE_STEP
                } else {
                    -1
                })
            }
            KeyCode::Char('g') => Outcome::ScrollPage(isize::MIN),
            KeyCode::Char('G') => Outcome::ScrollPage(isize::MAX),
            _ => Outcome::Ignore,
        };
    }
    // Detalhe aberto é modal: sai com Esc (fecha), `q` (sai do app) ou `r`
    // (relê disco). Mover/Tab por baixo do detalhe confundiria o foco.
    if state.detail_open {
        return match key.code {
            KeyCode::Esc => Outcome::ToggleDetail,
            KeyCode::Char('q') => Outcome::Quit,
            KeyCode::Char('r') => Outcome::Reload,
            _ => Outcome::Ignore,
        };
    }
    match key.code {
        KeyCode::Enter => {
            // Enter só abre detalhe com linha para abrir: lista vazia é
            // Ignore (e no cabeçalho Enter nunca teve significado). W6-PR3:
            // o painel de sessões abre o MESMO modal com o summary da
            // sessão selecionada. W6-PR4: o painel de wiki abre a página
            // em tela cheia (`OpenPage`).
            match state.panel {
                Panel::Findings if state.findings_len > 0 => Outcome::ToggleDetail,
                Panel::Sessions if state.sessions_len > 0 => Outcome::ToggleDetail,
                Panel::Wiki if state.wiki_len > 0 => Outcome::OpenPage,
                _ => Outcome::Ignore,
            }
        }
        KeyCode::Esc => {
            // "Esc volta": fecha o overlay aberto (ajuda). Sem overlay aberto
            // não há para onde voltar — NUNCA sai do app por engano.
            if state.help_open {
                Outcome::Help
            } else {
                Outcome::Ignore
            }
        }
        // Ctrl+L: redesenhar sem mudar nada (a tela de um terminal bagunçado
        // por outra aplicação volta sozinha).
        KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => Outcome::Redraw,
        _ => match keymap::map(key) {
            Some(Action::Quit) => Outcome::Quit,
            Some(Action::NextPanel) => Outcome::CyclePanel(1),
            Some(Action::PrevPanel) => Outcome::CyclePanel(-1),
            Some(Action::Down) => Outcome::Move(1),
            Some(Action::Up) => Outcome::Move(-1),
            // g/G = extremos: satura via [`apply_move`], o loop clampa pela
            // quantidade real de linhas.
            Some(Action::Top) => Outcome::Move(isize::MIN),
            Some(Action::Bottom) => Outcome::Move(isize::MAX),
            Some(Action::Reload) => Outcome::Reload,
            Some(Action::Help) => Outcome::Help,
            // `/` só significa algo no painel de wiki: nos outros, Ignore.
            Some(Action::SearchWiki) if state.panel == Panel::Wiki => Outcome::SearchWiki,
            Some(Action::SearchWiki) => Outcome::Ignore,
            None => Outcome::Ignore,
        },
    }
}

/// Passo de PgUp/PgDn no preview (linhas por "página" de scroll).
pub const SCROLL_PAGE_STEP: isize = 10;

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

    let palette = theme::palette_from_env();
    let mut screen = AppState {
        findings_len: initial.finding_rows.len(),
        sessions_len: initial.session_rows.len(),
        wiki_len: initial.wiki_rows.len(),
        ..AppState::default()
    };
    let mut snapshot = initial;

    loop {
        screen.findings_len = snapshot.finding_rows.len();
        screen.sessions_len = snapshot.session_rows.len();
        // A lista da wiki é FILTRADA por frame (é texto puro sobre o
        // snapshot, nunca I/O): a agulha vive no estado de tela.
        let filtered_wiki: Vec<&WikiPage> =
            state::filter_pages(&snapshot.wiki_rows, &screen.wiki_query);
        screen.wiki_len = filtered_wiki.len();
        let lines = state::header_lines(&snapshot);
        let findings = state::findings_lines(&snapshot);
        let sessions = state::sessions_lines(&snapshot);

        // Degradado? O tamanho vem do backend (cache do ratatui), não do frame.
        let size = terminal
            .size()
            .map_err(|err| format!("falha ao medir a tela: {err}"))?;
        let degraded = size.width < MIN_WIDTH || size.height < MIN_HEIGHT;

        // Clamp da seleção pela quantidade REAL de linhas visíveis: o ➤ nunca
        // aponta para fora da lista.
        let max_selected = max_selected_for(
            screen.panel,
            screen.findings_len,
            screen.sessions_len,
            screen.wiki_len,
            lines.len() + findings.len() + sessions.len() + 1,
            degraded,
        );
        screen.selected = screen.selected.min(max_selected.max(1) - 1);

        terminal
            .draw(|frame| render_frame(frame, &snapshot, &screen, palette))
            .map_err(|err| format!("falha ao desenhar o dashboard: {err}"))?;

        // Poll: decisão por tecla via função pura; sem tecla, nada acontece
        // (sem spinner eterno — a persona proíbe; o dashboard só muda quando
        // o mundo muda).
        if let Event::Key(key) =
            event::read().map_err(|err| format!("falha ao ler teclado: {err}"))?
        {
            match decide(&screen, key) {
                Outcome::Quit => return Ok(()),
                // Redraw: o próximo passo do loop desenha de novo — nada a mutar.
                Outcome::Redraw => {}
                Outcome::Reload => {
                    // `r` = reler o disco (read-only), nunca outra coisa.
                    snapshot = state::load();
                }
                Outcome::ToggleDetail => screen.detail_open = !screen.detail_open,
                Outcome::Move(offset) => screen.selected = apply_move(screen.selected, offset),
                Outcome::CyclePanel(direction) => {
                    screen.panel = next_panel(screen.panel, direction)
                }
                // W6-PR4: busca incremental e preview em tela cheia.
                Outcome::SearchWiki => {
                    screen.wiki_search_open = true;
                    screen.wiki_query.clear();
                    screen.selected = 0;
                }
                Outcome::SearchChar(c) => screen.wiki_query.push(c),
                Outcome::SearchBackspace => {
                    screen.wiki_query.pop();
                }
                Outcome::CloseSearch => screen.wiki_search_open = false,
                Outcome::OpenPage => {
                    screen.page_open = true;
                    screen.preview_scroll = 0;
                    screen.wiki_search_open = false;
                }
                Outcome::ClosePage => screen.page_open = false,
                // O scroll do preview é clamped DENTRO do widget (a largura/
                // altura reais são dele) — o loop só aplica o movimento.
                Outcome::ScrollPage(offset) => {
                    screen.preview_scroll = apply_move(screen.preview_scroll, offset)
                }
                Outcome::Help => screen.help_open = !screen.help_open,
                Outcome::Ignore => {}
            }
        }
    }
}

/// Limite de seleção pela quantidade REAL de linhas visíveis: o ➤ nunca
/// aponta para fora da lista. Função pura (unit-testada) — o loop só aplica.
pub fn max_selected_for(
    panel: Panel,
    findings_len: usize,
    sessions_len: usize,
    wiki_len: usize,
    degraded_total_lines: usize,
    degraded: bool,
) -> usize {
    if degraded {
        degraded_total_lines.max(1) - 1
    } else {
        match panel {
            Panel::Findings => findings_len.max(1) - 1,
            Panel::Sessions => sessions_len.max(1) - 1,
            Panel::Wiki => wiki_len.max(1) - 1,
            Panel::Header => 0,
        }
    }
}

/// Desenha UM frame completo do dashboard: cabeçalho + painel de achados +
/// painel de sessões (com o detalhe lateral quando `Enter` o abriu) + rodapé
/// (+ overlay de ajuda). Função pura sobre o frame — o loop só a chama, e os
/// testes a exercitam com `TestBackend`, o que dá cobertura real ao desenho
/// sem TTY.
pub fn render_frame(frame: &mut Frame, snapshot: &Snapshot, screen: &AppState, palette: Palette) {
    let area = frame.area();
    let degraded = area.width < MIN_WIDTH || area.height < MIN_HEIGHT;
    let lines = state::header_lines(snapshot);
    let findings = state::findings_lines(snapshot);
    let sessions = state::sessions_lines(snapshot);
    let wiki = state::wiki_lines(snapshot);
    let empty_line = state::empty_findings_line(snapshot);
    let empty_sessions_line = state::empty_sessions_line(snapshot);
    let empty_wiki_line = state::empty_wiki_line(snapshot);
    // A lista de páginas mostrada é a FILTRADA pela agulha da busca (pura);
    // sem busca aberta, a agulha está vazia e o filtro devolve tudo.
    let filtered_wiki: Vec<&WikiPage> =
        state::filter_pages(&snapshot.wiki_rows, &screen.wiki_query);
    // Rodapé: na busca aberta vira a caixa de texto (cursor no fim); fora
    // dela é a barra de teclas com o painel N/4.
    let footer = if screen.wiki_search_open {
        format!(
            "busca wiki: {}▌ (j/k mover · Enter abrir · Esc cancela)",
            screen.wiki_query
        )
    } else {
        format!(
            "painel {}/{} · {}",
            screen.panel.index() + 1,
            PANEL_COUNT,
            state::footer_line()
        )
    };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if degraded {
            // Degradado: lista única ocupa a tela inteira (menos o rodapé).
            vec![Constraint::Min(1), Constraint::Length(1)]
        } else {
            // Denso: cabeçalho + achados + sessões + wiki + rodapé de 1 linha.
            vec![
                Constraint::Length(lines.len() as u16 + 2),
                Constraint::Min(1),
                Constraint::Min(1),
                Constraint::Min(1),
                Constraint::Length(1),
            ]
        })
        .split(area);
    let (list_area, footer_area) = if degraded {
        (rows[0], rows[1])
    } else {
        (rows[0], rows[4])
    };
    if degraded {
        // Lista única com aviso: mesma informação, sem layout denso.
        let mut narrow = vec![format!(
            "aviso: terminal {}×{} menor que {MIN_WIDTH}×{MIN_HEIGHT} — layout de lista única",
            area.width, area.height
        )];
        narrow.extend(lines.iter().cloned());
        narrow.extend(findings.iter().cloned());
        narrow.extend(sessions.iter().cloned());
        narrow.extend(wiki.iter().cloned());
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
            // `selected` vira scroll da lista única: as teclas j/k continuam
            // fazendo algo útil na degradação.
            .scroll((
                u16::try_from(screen.selected.min(narrow.len().saturating_sub(1)))
                    .unwrap_or(u16::MAX),
                0,
            )),
            list_area,
        );
    } else {
        let header_focused = screen.panel == Panel::Header;
        let title = snapshot
            .project
            .as_ref()
            .map(|project| project.name.as_str());
        frame.render_widget(
            HeaderWidget::new(&lines, title, palette).focused(header_focused),
            rows[0],
        );
        match screen.panel {
            Panel::Findings if screen.detail_open => {
                // Detalhe lateral do achado: tabela à esquerda, achado
                // inteiro à direita — o contexto nunca desaparece ao abrir.
                let detail_cols = split_detail(rows[1]);
                frame.render_widget(
                    FindingsWidget::new(
                        &snapshot.finding_rows,
                        screen.selected,
                        true,
                        empty_line.clone(),
                        palette,
                    ),
                    detail_cols[0],
                );
                if let Some(row) = snapshot.finding_rows.get(screen.selected) {
                    let detail = state::detail_lines(row);
                    frame.render_widget(DetailWidget::new(&detail, palette), detail_cols[1]);
                }
            }
            Panel::Sessions if screen.detail_open => {
                // Detalhe lateral da sessão (W6-PR3): tabela à esquerda,
                // summary inteiro (≤2000 chars gravados) à direita.
                let detail_cols = split_detail(rows[2]);
                frame.render_widget(
                    SessionsWidget::new(
                        &snapshot.session_rows,
                        snapshot.now_secs,
                        screen.selected,
                        true,
                        empty_sessions_line.clone(),
                        palette,
                    ),
                    detail_cols[0],
                );
                if let Some(row) = snapshot.session_rows.get(screen.selected) {
                    let detail = state::session_detail_lines(row, snapshot.now_secs);
                    frame.render_widget(DetailWidget::new(&detail, palette), detail_cols[1]);
                }
            }
            _ => {
                // Sem detalhe: os painéis de tabela empilhados verticalmente
                // (achados, sessões, wiki) — a wiki sempre fica visível.
                frame.render_widget(
                    FindingsWidget::new(
                        &snapshot.finding_rows,
                        screen.selected,
                        screen.panel == Panel::Findings,
                        empty_line.clone(),
                        palette,
                    ),
                    rows[1],
                );
                frame.render_widget(
                    SessionsWidget::new(
                        &snapshot.session_rows,
                        snapshot.now_secs,
                        screen.selected,
                        screen.panel == Panel::Sessions,
                        empty_sessions_line.clone(),
                        palette,
                    ),
                    rows[2],
                );
                frame.render_widget(
                    WikiWidget::new(
                        &filtered_wiki,
                        snapshot.now_secs,
                        screen.selected,
                        screen.panel == Panel::Wiki,
                        empty_wiki_line.clone(),
                        palette,
                    ),
                    rows[3],
                );
            }
        }
    }
    frame.render_widget(
        Paragraph::new(Span::styled(footer, Style::default().fg(palette.footer))),
        footer_area,
    );
    // Preview em tela cheia da página (W6-PR4): cobre o dashboard inteiro,
    // como um "blog" — `Enter` abriu, `Esc` volta.
    if screen.page_open
        && let Some(page) = filtered_wiki.get(screen.selected)
    {
        frame.render_widget(
            WikiPageWidget::new(page, screen.preview_scroll, palette),
            area,
        );
    }
    if screen.help_open {
        frame.render_widget(help_paragraph(&palette), area);
    }
}

/// Divide a área de um painel em tabela (60%) + detalhe lateral (40%) — a
/// mesma proporção dos achados no W6-PR2, agora usada pelas sessões também.
fn split_detail(area: Rect) -> std::rc::Rc<[Rect]> {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area)
}

/// Overlay de ajuda: os mesmos atalhos do rodapé, com o que cada um faz.
fn help_paragraph(palette: &Palette) -> Paragraph<'static> {
    let text = [
        "ajuda do kira dashboard (read-only — nenhuma tecla escreve)".to_string(),
        String::new(),
        "q                sair".to_string(),
        "Tab / Shift+Tab  ciclar painel (cabeçalho ↔ achados ↔ sessões ↔ wiki)".to_string(),
        "j / k ou ↑ / ↓   mover linha".to_string(),
        "g / G            topo / fim".to_string(),
        "Enter            abrir detalhe (achado ou sessão) ou página da wiki".to_string(),
        "Esc              voltar (fecha detalhe/página/ajuda)".to_string(),
        "/                busca incremental na wiki (Esc cancela)".to_string(),
        "j/k, PgUp/PgDn   rolar a página da wiki aberta em tela cheia".to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    /// Helper: monta o KeyEvent do jeito que o crossterm entrega teclas nuas.
    fn plain(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// Estado de referência: painel de achados em foco, 3 linhas, nada aberto.
    fn findings_state() -> AppState {
        AppState {
            panel: Panel::Findings,
            findings_len: 3,
            ..AppState::default()
        }
    }

    #[test]
    fn quit_reload_redraw_and_help_map_from_their_keys() {
        let state = findings_state();
        assert_eq!(decide(&state, plain(KeyCode::Char('q'))), Outcome::Quit);
        assert_eq!(decide(&state, plain(KeyCode::Char('r'))), Outcome::Reload);
        assert_eq!(decide(&state, plain(KeyCode::Char('?'))), Outcome::Help);
        // Ctrl+L: redesenhar sem mudar estado (e `l` solto é Ignore).
        let ctrl_l = KeyEvent {
            modifiers: KeyModifiers::CONTROL,
            ..KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE)
        };
        assert_eq!(decide(&state, ctrl_l), Outcome::Redraw);
        assert_eq!(decide(&state, plain(KeyCode::Char('l'))), Outcome::Ignore);
    }

    #[test]
    fn enter_opens_detail_only_on_findings_with_rows() {
        // Painel de achados com linhas: Enter abre o detalhe.
        assert_eq!(
            decide(&findings_state(), plain(KeyCode::Enter)),
            Outcome::ToggleDetail
        );
        // Lista vazia: Enter não tem o que abrir.
        assert_eq!(
            decide(
                &AppState {
                    panel: Panel::Findings,
                    ..AppState::default()
                },
                plain(KeyCode::Enter)
            ),
            Outcome::Ignore
        );
        // Painel de cabeçalho: Enter nunca teve significado aqui.
        assert_eq!(
            decide(&AppState::default(), plain(KeyCode::Enter)),
            Outcome::Ignore
        );
        // W6-PR3: o mesmo Enter abre o detalhe da sessão no painel de
        // sessões (com linhas) e é Ignore na lista vazia.
        let sessions_state = AppState {
            panel: Panel::Sessions,
            sessions_len: 3,
            ..AppState::default()
        };
        assert_eq!(
            decide(&sessions_state, plain(KeyCode::Enter)),
            Outcome::ToggleDetail
        );
        assert_eq!(
            decide(
                &AppState {
                    panel: Panel::Sessions,
                    ..AppState::default()
                },
                plain(KeyCode::Enter)
            ),
            Outcome::Ignore
        );
    }

    #[test]
    fn esc_goes_back_closing_the_open_overlay_never_the_app() {
        // Sem nada aberto: Esc não faz nada (menos sair do app por engano).
        assert_eq!(
            decide(&findings_state(), plain(KeyCode::Esc)),
            Outcome::Ignore
        );
        // Ajuda aberta: Esc volta fechando a ajuda.
        let with_help = AppState {
            help_open: true,
            ..findings_state()
        };
        assert_eq!(decide(&with_help, plain(KeyCode::Esc)), Outcome::Help);
        // Detalhe aberto: Esc volta fechando o detalhe — e o detalhe é modal.
        let with_detail = AppState {
            detail_open: true,
            ..findings_state()
        };
        assert_eq!(
            decide(&with_detail, plain(KeyCode::Esc)),
            Outcome::ToggleDetail
        );
        // Modal: mover/Tab/ajuda não vazam por baixo do detalhe…
        assert_eq!(decide(&with_detail, plain(KeyCode::Tab)), Outcome::Ignore);
        assert_eq!(
            decide(&with_detail, plain(KeyCode::Char('j'))),
            Outcome::Ignore
        );
        assert_eq!(
            decide(&with_detail, plain(KeyCode::Char('?'))),
            Outcome::Ignore
        );
        // …mas `q` continua saindo do app e `r` relendo o disco.
        assert_eq!(
            decide(&with_detail, plain(KeyCode::Char('q'))),
            Outcome::Quit
        );
        assert_eq!(
            decide(&with_detail, plain(KeyCode::Char('r'))),
            Outcome::Reload
        );
    }

    #[test]
    fn tab_cycles_panels_in_both_directions_with_wrap() {
        let state = findings_state();
        assert_eq!(decide(&state, plain(KeyCode::Tab)), Outcome::CyclePanel(1));
        assert_eq!(
            decide(&state, plain(KeyCode::BackTab)),
            Outcome::CyclePanel(-1)
        );
        // O ciclo de 4 painéis (W6-PR4): cabeçalho → achados → sessões →
        // wiki → cabeçalho, com wrap nas duas direções.
        assert_eq!(next_panel(Panel::Header, 1), Panel::Findings);
        assert_eq!(next_panel(Panel::Findings, 1), Panel::Sessions);
        assert_eq!(next_panel(Panel::Sessions, 1), Panel::Wiki);
        assert_eq!(next_panel(Panel::Wiki, 1), Panel::Header);
        assert_eq!(next_panel(Panel::Header, -1), Panel::Wiki);
        assert_eq!(next_panel(Panel::Findings, -1), Panel::Header);
        assert_eq!(next_panel(Panel::Sessions, -1), Panel::Findings);
        assert_eq!(next_panel(Panel::Wiki, -1), Panel::Sessions);
    }

    #[test]
    fn movement_covers_arrows_vim_and_extremes() {
        let state = findings_state();
        assert_eq!(decide(&state, plain(KeyCode::Char('j'))), Outcome::Move(1));
        assert_eq!(decide(&state, plain(KeyCode::Down)), Outcome::Move(1));
        assert_eq!(decide(&state, plain(KeyCode::Char('k'))), Outcome::Move(-1));
        assert_eq!(decide(&state, plain(KeyCode::Up)), Outcome::Move(-1));
        // g/G = extremos via saturação, nunca desvaziar o usize.
        assert_eq!(
            decide(&state, plain(KeyCode::Char('g'))),
            Outcome::Move(isize::MIN)
        );
        assert_eq!(
            decide(&state, plain(KeyCode::Char('G'))),
            Outcome::Move(isize::MAX)
        );
    }

    #[test]
    fn apply_move_saturates_and_clamps_to_the_real_list() {
        assert_eq!(apply_move(2, 1), 3);
        assert_eq!(apply_move(2, -1), 1);
        // Extremos: saturação nas duas pontas (o loop clampa pelo tamanho).
        assert_eq!(apply_move(2, isize::MIN), 0);
        assert_eq!(apply_move(2, isize::MAX), (isize::MAX as usize) + 2);
        assert_eq!(apply_move(0, -5), 0);
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let state = findings_state();
        assert_eq!(decide(&state, plain(KeyCode::Char('x'))), Outcome::Ignore);
        assert_eq!(decide(&state, plain(KeyCode::F(1))), Outcome::Ignore);
    }

    #[test]
    fn panel_index_numbers_the_footer_cycle() {
        assert_eq!(Panel::Header.index(), 0);
        assert_eq!(Panel::Findings.index(), 1);
        assert_eq!(Panel::Sessions.index(), 2);
        assert_eq!(Panel::Wiki.index(), 3);
        assert_eq!(PANEL_COUNT, 4);
    }

    #[test]
    fn max_selected_clamps_per_panel_and_degradation() {
        // Painel de achados: limite = nº de achados - 1.
        assert_eq!(max_selected_for(Panel::Findings, 3, 5, 7, 0, false), 2);
        // Painel de sessões: limite = nº de sessões - 1 (W6-PR3).
        assert_eq!(max_selected_for(Panel::Sessions, 3, 5, 7, 0, false), 4);
        // Painel de wiki: limite = nº de páginas FILTRADAS - 1 (W6-PR4).
        assert_eq!(max_selected_for(Panel::Wiki, 3, 5, 7, 0, false), 6);
        // Listas vazias: ainda assim nunca negativo (0).
        assert_eq!(max_selected_for(Panel::Findings, 0, 0, 0, 0, false), 0);
        assert_eq!(max_selected_for(Panel::Sessions, 0, 0, 0, 0, false), 0);
        assert_eq!(max_selected_for(Panel::Wiki, 0, 0, 0, 0, false), 0);
        // Cabeçalho denso: seleção não usada, limite 0.
        assert_eq!(max_selected_for(Panel::Header, 5, 5, 5, 0, false), 0);
        // Degradado: limite = total de linhas visíveis - 1 (com aviso).
        assert_eq!(max_selected_for(Panel::Header, 3, 3, 3, 10, true), 9);
    }

    /// Renderiza um frame completo com `TestBackend` e devolve o texto — o
    /// mesmo smith determinístico do widget, agora sobre o frame inteiro.
    fn render_full(snapshot: &Snapshot, screen: &AppState, width: u16, height: u16) -> String {
        use ratatui::backend::TestBackend;
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
        terminal
            .draw(|frame| render_frame(frame, snapshot, screen, theme::mono()))
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

    /// Snapshot com 1 achado de referência (caminho do fluxo feliz).
    fn snapshot_with_one_finding() -> Snapshot {
        Snapshot {
            findings: Some(1),
            finding_rows: state::rows_from(vec![crate::scan::Finding {
                tool: "audit".to_string(),
                rule: "vulnerable".to_string(),
                package: "lodash".to_string(),
                version: "4.17.19".to_string(),
                severity: "critical".to_string(),
                path: "src/lib.rs:42".to_string(),
                message: "Prototype Pollution\nCVE-2020-8203".to_string(),
                fingerprint: "0a1b2c3d4e5f6001".to_string(),
            }]),
            ..Snapshot::default()
        }
    }

    /// Snapshot com 3 sessões de referência (W6-PR3): duas ok, uma falha —
    /// o caminho feliz do painel de sessões, com `now` fixo (determinismo).
    fn snapshot_with_three_sessions() -> Snapshot {
        let sessions: Vec<crate::store::SessionRow> = [
            (
                3,
                "scan",
                None,
                true,
                "scan fixture: 3 achados\nlinha interna",
            ),
            (2, "triage", Some("qwen2.5:7b"), true, "nota gravada"),
            (1, "doctor", None, false, "ollama não respondeu"),
        ]
        .into_iter()
        .map(
            |(id, command, model, ok, summary)| crate::store::SessionRow {
                id,
                command: command.to_string(),
                args: String::new(),
                model: model.map(|model| model.to_string()),
                started_at: "2026-11-11T09:00:00Z".to_string(),
                local_started_at: None,
                duration_ms: 65_000,
                ok,
                summary: summary.to_string(),
            },
        )
        .collect();
        // now = started_at (2026-11-11T09:00:00Z) + 3 min ⇒ "há 3 min"
        // determinístico, sem nenhum relógio do sistema no teste.
        let now_secs = 1_794_387_600 + 180;
        Snapshot {
            session_rows: state::session_rows_from(&sessions),
            now_secs: Some(now_secs),
            ..Snapshot::default()
        }
    }

    #[test]
    fn full_frame_renders_header_findings_and_footer() {
        let snapshot = snapshot_with_one_finding();
        let screen = AppState {
            panel: Panel::Findings,
            findings_len: 1,
            ..AppState::default()
        };
        let text = render_full(&snapshot, &screen, 100, 30);
        assert!(
            text.contains("painel 2/4"),
            "rodapé no painel de achados: {text}"
        );
        assert!(
            text.contains("critical 1 · high 0 · medium 0 · low 0"),
            "contagem no cabeçalho do painel: {text}"
        );
        assert!(text.contains("src/lib.rs:42"), "{text}");
        assert!(text.contains("➤"), "seleção visível: {text}");
    }

    #[test]
    fn detail_frame_shows_the_whole_finding_beside_the_table() {
        let snapshot = snapshot_with_one_finding();
        let screen = AppState {
            panel: Panel::Findings,
            findings_len: 1,
            detail_open: true,
            ..AppState::default()
        };
        // Frame mais alto: com 4 painéis (W6-PR4) o modal de detalhe precisa
        // de mais altura para caber a mensagem/summary INTEIRO visível.
        let text = render_full(&snapshot, &screen, 100, 50);
        assert!(text.contains("detalhe — Esc volta"), "{text}");
        assert!(
            text.contains("fingerprint: 0a1b2c3d4e5f6001"),
            "fingerprint completo no detalhe: {text}"
        );
        assert!(
            text.contains("CVE-2020-8203"),
            "mensagem inteira no detalhe: {text}"
        );
        // A tabela continua à esquerda (contexto nunca some).
        assert!(text.contains("src/lib.rs:42"), "{text}");
    }

    #[test]
    fn help_overlay_and_header_focus_render() {
        let snapshot = Snapshot::default();
        let screen = AppState {
            panel: Panel::Header,
            help_open: true,
            ..AppState::default()
        };
        let text = render_full(&snapshot, &screen, 100, 24);
        assert!(text.contains("ajuda do kira dashboard"), "{text}");
        assert!(text.contains("Enter            abrir detalhe"), "{text}");
        assert!(
            text.contains("Esc              voltar"),
            "o Esc documentado na ajuda: {text}"
        );
    }

    #[test]
    fn header_focus_shows_panel_one_of_three_footer() {
        let snapshot = Snapshot::default();
        let screen = AppState {
            panel: Panel::Header,
            ..AppState::default()
        };
        let text = render_full(&snapshot, &screen, 100, 24);
        assert!(text.contains("painel 1/4"), "{text}");
        assert!(
            text.contains("achados: — (nunca rodou kira scan)"),
            "estado vazio honesto: {text}"
        );
        // Os TRÊS painéis de tabela ficam vazios de forma honesta.
        assert!(
            text.contains("sessões: — (store sem sessões)"),
            "estado vazio honesto das sessões: {text}"
        );
        assert!(
            text.contains("wiki: — (wiki vazia — rode kira wiki reindex)"),
            "estado vazio honesto da wiki: {text}"
        );
    }

    #[test]
    fn sessions_frame_renders_the_three_row_table_with_selection() {
        let snapshot = snapshot_with_three_sessions();
        let screen = AppState {
            panel: Panel::Sessions,
            sessions_len: 3,
            ..AppState::default()
        };
        let text = render_full(&snapshot, &screen, 100, 30);
        assert!(
            text.contains("painel 3/4"),
            "rodapé no painel de sessões: {text}"
        );
        assert!(
            text.contains("sessões — últimas 3"),
            "título do bloco com contagem: {text}"
        );
        assert!(text.contains("#ID"), "{text}");
        assert!(text.contains("QUANDO"), "{text}");
        assert!(text.contains("#3"), "{text}");
        assert!(text.contains("triage"), "{text}");
        assert!(text.contains("qwen2.5:7b"), "{text}");
        assert!(text.contains("1m5s"), "{text}");
        assert!(text.contains("há 3 min"), "idade relativa pura: {text}");
        // Mais recente primeiro (id 3 antes do 1) e seleção visível.
        let id3 = text.find("#3").unwrap_or(usize::MAX);
        let id1 = text.find("#1").unwrap_or(usize::MAX);
        assert!(id3 < id1, "ordem mais-recente-primeiro: {text}");
        assert!(text.contains("➤"), "seleção visível: {text}");
        // Só a 1ª linha do summary na tabela; o resto é do detalhe.
        assert!(text.contains("scan fixture: 3 achados"), "{text}");
        assert!(!text.contains("linha interna"), "{text}");
    }

    #[test]
    fn session_detail_frame_shows_the_whole_summary_beside_the_table() {
        let snapshot = snapshot_with_three_sessions();
        let screen = AppState {
            panel: Panel::Sessions,
            sessions_len: 3,
            detail_open: true,
            ..AppState::default()
        };
        // Frame mais alto: com 3 painéis o modal de detalhe precisa de mais
        // altura para caber o summary INTEIRO visível.
        let text = render_full(&snapshot, &screen, 100, 50);
        assert!(text.contains("detalhe — Esc volta"), "{text}");
        // Seleção 0 = a sessão mais recente (id 3, comando scan).
        assert!(text.contains("comando: scan"), "{text}");
        assert!(text.contains("modelo: —"), "{text}");
        assert!(text.contains("duração: 1m5s"), "{text}");
        assert!(text.contains("status: ok"), "{text}");
        // O summary inteiro aparece no detalhe (a tabela cortou a 2ª linha).
        assert!(text.contains("linha interna"), "{text}");
        // A tabela continua à esquerda (contexto nunca some).
        assert!(text.contains("#3"), "{text}");
    }

    // ------------------------------------------------------------------
    // Painel de wiki (W6-PR4)
    // ------------------------------------------------------------------

    /// Snapshot com 2 páginas de wiki de referência (o caminho feliz).
    fn snapshot_with_wiki_pages() -> Snapshot {
        Snapshot {
            wiki_rows: vec![
                state::WikiPage {
                    slug: "contexto".to_string(),
                    kind: "contexto".to_string(),
                    title: "Contexto do projeto".to_string(),
                    path: String::new(),
                    updated_at: "2026-11-11T09:00:00Z".to_string(),
                    updated_unix: Some(1_794_387_600),
                    content: "# Contexto\n\n- item um\n\n**decisão** com `código`".to_string(),
                },
                state::WikiPage {
                    slug: "bugs/4821".to_string(),
                    kind: "bug".to_string(),
                    title: "Sessão 4821".to_string(),
                    path: String::new(),
                    updated_at: "2026-11-10T09:00:00Z".to_string(),
                    updated_unix: Some(1_794_387_600),
                    content: "token expirado em produção".to_string(),
                },
            ],
            now_secs: Some(1_794_387_600 + 180),
            ..Snapshot::default()
        }
    }

    #[test]
    fn wiki_enter_opens_the_page_and_empty_list_is_ignore() {
        // Painel de wiki com páginas: Enter abre o preview em tela cheia.
        let wiki_state = AppState {
            panel: Panel::Wiki,
            wiki_len: 2,
            ..AppState::default()
        };
        assert_eq!(
            decide(&wiki_state, plain(KeyCode::Enter)),
            Outcome::OpenPage
        );
        // Lista vazia (ou filtro sem casamento): Enter não tem o que abrir.
        assert_eq!(
            decide(
                &AppState {
                    panel: Panel::Wiki,
                    ..AppState::default()
                },
                plain(KeyCode::Enter)
            ),
            Outcome::Ignore
        );
    }

    #[test]
    fn slash_opens_the_search_only_in_the_wiki_panel() {
        let wiki_state = AppState {
            panel: Panel::Wiki,
            wiki_len: 2,
            ..AppState::default()
        };
        assert_eq!(
            decide(&wiki_state, plain(KeyCode::Char('/'))),
            Outcome::SearchWiki
        );
        // Nos outros painéis `/` não significa nada (ainda).
        assert_eq!(
            decide(&findings_state(), plain(KeyCode::Char('/'))),
            Outcome::Ignore
        );
    }

    #[test]
    fn search_mode_turns_keys_into_text_and_esc_cancels() {
        let searching = AppState {
            panel: Panel::Wiki,
            wiki_search_open: true,
            wiki_query: "se".to_string(),
            ..AppState::default()
        };
        // Caracteres comuns (e maiúsculas com SHIFT) entram na agulha.
        assert_eq!(
            decide(&searching, plain(KeyCode::Char('s'))),
            Outcome::SearchChar('s')
        );
        let shift_s = KeyEvent {
            modifiers: KeyModifiers::SHIFT,
            ..KeyEvent::new(KeyCode::Char('S'), KeyModifiers::NONE)
        };
        assert_eq!(decide(&searching, shift_s), Outcome::SearchChar('S'));
        // Backspace apaga; Esc cancela a busca (modal fecha).
        assert_eq!(
            decide(&searching, plain(KeyCode::Backspace)),
            Outcome::SearchBackspace
        );
        assert_eq!(
            decide(&searching, plain(KeyCode::Esc)),
            Outcome::CloseSearch
        );
        // Teclas de navegação NÃO vazam por baixo da busca: modal.
        assert_eq!(decide(&searching, plain(KeyCode::Tab)), Outcome::Ignore);
        assert_eq!(decide(&searching, plain(KeyCode::Down)), Outcome::Ignore);
        // Ctrl+ combinações não viram texto.
        let ctrl_s = KeyEvent {
            modifiers: KeyModifiers::CONTROL,
            ..KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE)
        };
        assert_eq!(decide(&searching, ctrl_s), Outcome::Ignore);
    }

    #[test]
    fn page_view_is_modal_esc_back_and_scroll_by_line_and_page() {
        let open = AppState {
            panel: Panel::Wiki,
            wiki_len: 2,
            page_open: true,
            ..AppState::default()
        };
        assert_eq!(decide(&open, plain(KeyCode::Esc)), Outcome::ClosePage);
        assert_eq!(
            decide(&open, plain(KeyCode::Char('j'))),
            Outcome::ScrollPage(1)
        );
        assert_eq!(
            decide(&open, plain(KeyCode::Char('k'))),
            Outcome::ScrollPage(-1)
        );
        assert_eq!(
            decide(&open, plain(KeyCode::PageDown)),
            Outcome::ScrollPage(SCROLL_PAGE_STEP)
        );
        assert_eq!(
            decide(&open, plain(KeyCode::PageUp)),
            Outcome::ScrollPage(-SCROLL_PAGE_STEP)
        );
        assert_eq!(
            decide(&open, plain(KeyCode::Char('g'))),
            Outcome::ScrollPage(isize::MIN)
        );
        assert_eq!(
            decide(&open, plain(KeyCode::Char('G'))),
            Outcome::ScrollPage(isize::MAX)
        );
        // Modal: mover/Tab/ajuda não vazam por baixo do preview…
        assert_eq!(decide(&open, plain(KeyCode::Tab)), Outcome::Ignore);
        assert_eq!(decide(&open, plain(KeyCode::Char('?'))), Outcome::Ignore);
        // …mas `q` sai do app e `r` relê o disco (mesmo contrato do detalhe).
        assert_eq!(decide(&open, plain(KeyCode::Char('q'))), Outcome::Quit);
        assert_eq!(decide(&open, plain(KeyCode::Char('r'))), Outcome::Reload);
    }

    #[test]
    fn wiki_frame_renders_the_list_with_selection_and_age() {
        let snapshot = snapshot_with_wiki_pages();
        let screen = AppState {
            panel: Panel::Wiki,
            wiki_len: 2,
            ..AppState::default()
        };
        let text = render_full(&snapshot, &screen, 100, 30);
        assert!(text.contains("painel 4/4"), "{text}");
        assert!(text.contains("wiki — 2 páginas"), "{text}");
        assert!(text.contains("TIPO"), "{text}");
        assert!(text.contains("Contexto do projeto"), "{text}");
        assert!(text.contains("Sessão 4821"), "{text}");
        assert!(text.contains("há 3 min"), "{text}");
        assert!(text.contains("➤"), "seleção visível: {text}");
    }

    #[test]
    fn wiki_page_overlay_renders_the_markdown_in_full_screen() {
        let snapshot = snapshot_with_wiki_pages();
        let screen = AppState {
            panel: Panel::Wiki,
            wiki_len: 2,
            page_open: true,
            ..AppState::default()
        };
        let text = render_full(&snapshot, &screen, 100, 30);
        // Preview em tela cheia: título da página + markdown rendido (sem
        // `#`, sem `**`, sem code span cru).
        assert!(
            text.contains("wiki: Contexto do projeto — Esc volta"),
            "{text}"
        );
        assert!(text.contains("▌ Contexto"), "{text}");
        assert!(text.contains("• item um"), "{text}");
        assert!(text.contains("decisão"), "{text}");
        assert!(!text.contains("**"), "{text}");
        // A lista NÃO aparece por baixo do preview (tela cheia de verdade).
        assert!(!text.contains("Sessão 4821"), "{text}");
    }

    #[test]
    fn wiki_search_footer_shows_the_query_with_cursor() {
        let snapshot = snapshot_with_wiki_pages();
        let screen = AppState {
            panel: Panel::Wiki,
            wiki_search_open: true,
            wiki_query: "sess".to_string(),
            wiki_len: 1,
            ..AppState::default()
        };
        let text = render_full(&snapshot, &screen, 100, 30);
        // Rodapé vira caixa de texto com cursor ▌.
        assert!(text.contains("busca wiki: sess▌"), "{text}");
        // E a lista FILTRADA (só a página que casa) está visível.
        assert!(text.contains("Sessão 4821"), "{text}");
        assert!(
            !text.contains("wiki — 2 páginas"),
            "com filtro, só as casadas: {text}"
        );
        assert!(text.contains("wiki — 1 página"), "{text}");
    }

    #[test]
    fn wiki_search_filters_by_content_folded() {
        // A função pura do filtro é a mesma que o widget recebe — e o
        // render_frame FILTRA de verdade (agulha "token" acha só a 4821).
        let snapshot = snapshot_with_wiki_pages();
        let filtered = state::filter_pages(&snapshot.wiki_rows, "token");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].slug, "bugs/4821");
    }

    #[test]
    fn degraded_frame_keeps_everything_in_one_list_with_the_warning() {
        let mut snapshot = snapshot_with_one_finding();
        let sessions = snapshot_with_three_sessions();
        snapshot.session_rows = sessions.session_rows;
        snapshot.now_secs = sessions.now_secs;
        let screen = AppState {
            panel: Panel::Findings,
            findings_len: 1,
            sessions_len: 3,
            ..AppState::default()
        };
        let text = render_full(&snapshot, &screen, 60, 20);
        assert!(
            text.contains("aviso: terminal 60×20 menor que 80×24"),
            "aviso de degradação: {text}"
        );
        assert!(
            text.contains("src/lib.rs:42"),
            "achado na lista única: {text}"
        );
        assert!(
            text.contains("#3"),
            "sessão na lista única (mesma informação degradada): {text}"
        );
        assert!(text.contains("painel achados"), "{text}");
        assert!(text.contains("painel sessões"), "{text}");
    }
}
