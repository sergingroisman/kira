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
//! mode/alternate screen no Drop (erro de desenho no meio do loop incluso) e
//! o hook de pânico instalado em [`run`] restaura o terminal ANTES do
//! backtrace — panic/erro nunca deixa o terminal quebrado.

use std::io::{self};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
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
    /// Índice de seleção do painel de achados (o ➤ da tabela). UM POR PAINEL:
    /// o índice único compartilhado fazia ↓ em achados mover a seleção das
    /// sessões/wiki junto (defeito real do smoke do dono).
    pub findings_selected: usize,
    /// Índice de seleção do painel de sessões — próprio, nunca compartilhado.
    pub sessions_selected: usize,
    /// Índice de seleção do painel de wiki (sobre a lista FILTRADA).
    pub wiki_selected: usize,
    /// Índice de scroll da lista ÚNICA do modo degradado (não há painéis).
    pub degraded_selected: usize,
    /// Modo degradado ativo (terminal menor que 80×24)? `run` atualiza a cada
    /// frame; `Move` roteia para a lista única em vez do painel em foco.
    pub degraded: bool,
    /// Detalhe de achado aberto (Enter abre, Enter volta — Esc agora sai).
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
    /// Preview de página aberto em tela cheia (Enter abre, Enter volta).
    pub page_open: bool,
    /// Scroll vertical do preview (linhas; o widget clampa pelo tamanho real).
    pub preview_scroll: usize,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            panel: Panel::Header,
            findings_selected: 0,
            sessions_selected: 0,
            wiki_selected: 0,
            degraded_selected: 0,
            degraded: false,
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

/// Executa um [`Outcome`] sobre o estado de TELA — a mutação EXTRAÍDA do loop,
/// função pura de mutação unit-testável sem terminal (mesma disciplina do
/// [`decide`]): é aqui que o ESCOPO POR PAINEL do `Move` acontece.
///
/// `Quit` e `Reload` NÃO são daqui: o primeiro devolve o controle ao `run`
/// (saída/restauro do terminal) e o segundo relê o disco (I/O fica no loop).
pub fn apply_outcome(screen: &mut AppState, outcome: Outcome) {
    match outcome {
        // Redraw: o próximo passo do loop desenha de novo — nada a mutar.
        Outcome::Redraw => {}
        Outcome::ToggleDetail => screen.detail_open = !screen.detail_open,
        Outcome::Move(offset) => {
            // ESCOPO POR PAINEL (defeito real do smoke do dono): mover muta
            // SOMENTE o índice do painel em foco — ↓ em achados nunca mais
            // mexe na seleção de sessões ou wiki (cada painel tem o seu
            // índice e o seu scroll).
            if screen.degraded {
                // Degradado não há painéis visíveis: a lista única tem o
                // próprio índice de scroll.
                screen.degraded_selected = apply_move(screen.degraded_selected, offset);
            } else {
                match screen.panel {
                    Panel::Findings => {
                        screen.findings_selected = apply_move(screen.findings_selected, offset)
                    }
                    Panel::Sessions => {
                        screen.sessions_selected = apply_move(screen.sessions_selected, offset)
                    }
                    Panel::Wiki => screen.wiki_selected = apply_move(screen.wiki_selected, offset),
                    // O cabeçalho denso não tem linhas para mover.
                    Panel::Header => {}
                }
            }
        }
        Outcome::CyclePanel(direction) => screen.panel = next_panel(screen.panel, direction),
        // W6-PR4: busca incremental e preview em tela cheia.
        Outcome::SearchWiki => {
            screen.wiki_search_open = true;
            screen.wiki_query.clear();
            // A lista filtrada recomeça do topo.
            screen.wiki_selected = 0;
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
        // O scroll do preview é clamped DENTRO do widget (a largura/altura
        // reais são dele) — aqui só aplica o movimento.
        Outcome::ScrollPage(offset) => {
            screen.preview_scroll = apply_move(screen.preview_scroll, offset)
        }
        Outcome::Help => screen.help_open = !screen.help_open,
        Outcome::Ignore => {}
        // Quit e Reload pertencem ao loop (saída/restauro e leitura do disco).
        Outcome::Quit | Outcome::Reload => {}
    }
}

/// DECISÃO PURA da tecla: estado de tela + `KeyEvent` → [`Outcome`].
///
/// Toda a semântica de interação do loop mora aqui, sem terminal, sem I/O —
/// o loop só executa o outcome. A SAÍDA (`q`, `Esc`, Ctrl+C) é a primeira
/// checagem, ANTES de qualquer modal — contrato do dono: sair em qualquer
/// estado, inclusive ajuda/detalhe/busca abertos. Setas E vim (persona §7)
/// chegam pelo `keymap`; Enter/TAB têm semântica dependente do estado (painel
/// em foco, detalhe aberto) e por isso são decididos AQUI, não no keymap sem
/// estado do W6-PR1. Nenhuma tecla escreve: não existe outcome de escrita.
pub fn decide(state: &AppState, key: KeyEvent) -> Outcome {
    // SAÍDA ANTES DE TUDO: `q`, `Esc` e Ctrl+C encerram o dashboard em
    // QUALQUER estado (ajuda aberta, detalhe aberto, busca ativa, preview de
    // página). Nenhum modal pode engolir a tecla de sair — por isso a
    // checagem mora na primeira linha do decide().
    match key.code {
        KeyCode::Esc => return Outcome::Quit,
        KeyCode::Char('q') => return Outcome::Quit,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            return Outcome::Quit;
        }
        _ => {}
    }
    // Busca aberta é modal: toda tecla vira texto no rodapé; Enter CONFIRMA e
    // fecha a caixa (o filtro é incremental, a agulha fica). Só caracteres
    // sem CONTROL entram; SHIFT entra (maiúscula é busca). Esc NÃO cancela
    // mais — agora sai do app (checagem acima).
    if state.wiki_search_open {
        return match key.code {
            KeyCode::Enter => Outcome::CloseSearch,
            KeyCode::Backspace => Outcome::SearchBackspace,
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                Outcome::SearchChar(c)
            }
            _ => Outcome::Ignore,
        };
    }
    // Preview de página aberto é modal: Enter volta à lista, j/k/PgUp/PgDn
    // rolam e `r` relê o disco (o mesmo serviço do modal de detalhe). Esc NÃO
    // fecha mais — sai do app (checagem de saída acima). Mover/Tab por baixo
    // do preview confundiria o foco.
    if state.page_open {
        return match key.code {
            KeyCode::Enter => Outcome::ClosePage,
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
    // Detalhe aberto é modal: Enter fecha (o detalhe é toggle) e `r` relê o
    // disco. Esc NÃO fecha mais — sai do app (checagem de saída acima).
    // Mover/Tab por baixo do detalhe confundiria o foco.
    if state.detail_open {
        return match key.code {
            KeyCode::Enter => Outcome::ToggleDetail,
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
        // Esc NÃO chega aqui: a checagem de saída no topo do decide() já o
        // converteu em Quit (contrato do dono — Esc sai em qualquer estado).
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

    // Restauro do terminal GARANTIDO até em pânico: o hook devolve raw
    // mode/alternate screen ANTES do backtrace. O Drop do guard já cobre erro
    // e return; o hook cobre o caminho de panic — o dono nunca fica com o
    // terminal quebrado (o defeito "não consigo fechar" não pode virar
    // "terminal destruído").
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        default_hook(info);
    }));

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

        // Clamp de TODOS os índices pela quantidade REAL de linhas visíveis:
        // o ➤ nunca aponta para fora da lista, nem depois de um `r` que
        // encolheu os dados, nem ao voltar para um painel. Um índice por
        // painel, mais o scroll da lista única degradada.
        let degraded_total_lines = lines.len() + findings.len() + sessions.len() + 1;
        screen.degraded = degraded;
        screen.findings_selected = screen.findings_selected.min(max_selected_for(
            Panel::Findings,
            screen.findings_len,
            screen.sessions_len,
            screen.wiki_len,
            degraded_total_lines,
            degraded,
        ));
        screen.sessions_selected = screen.sessions_selected.min(max_selected_for(
            Panel::Sessions,
            screen.findings_len,
            screen.sessions_len,
            screen.wiki_len,
            degraded_total_lines,
            degraded,
        ));
        screen.wiki_selected = screen.wiki_selected.min(max_selected_for(
            Panel::Wiki,
            screen.findings_len,
            screen.sessions_len,
            screen.wiki_len,
            degraded_total_lines,
            degraded,
        ));
        screen.degraded_selected = screen.degraded_selected.min(max_selected_for(
            Panel::Header,
            screen.findings_len,
            screen.sessions_len,
            screen.wiki_len,
            degraded_total_lines,
            degraded,
        ));

        terminal
            .draw(|frame| render_frame(frame, &snapshot, &screen, palette))
            .map_err(|err| format!("falha ao desenhar o dashboard: {err}"))?;

        // Poll: decisão por tecla via função pura; sem tecla, nada acontece
        // (sem spinner eterno — a persona proíbe; o dashboard só muda quando
        // o mundo muda).
        if let Event::Key(key) =
            event::read().map_err(|err| format!("falha ao ler teclado: {err}"))?
        {
            // O Windows entrega Press E Release para a MESMA tecla (no Unix só
            // existe Press): tratar os dois repetia cada tecla — o Tab pulava
            // painel (o Release repetia o ciclo e a wiki vazia nunca ficava
            // em foco) e o ↓ andava dois passos. Só Press executa a decisão.
            if key.kind == KeyEventKind::Press {
                match decide(&screen, key) {
                    Outcome::Quit => return Ok(()),
                    // `r` relê o disco (read-only), nunca outra coisa — I/O
                    // fica no loop; toda mutação de tela é função pura.
                    Outcome::Reload => {
                        snapshot = state::load();
                    }
                    outcome => apply_outcome(&mut screen, outcome),
                }
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
    // dela é a barra de teclas com o painel N/4. EXTRA: painel focado vazio ⇒
    // a dica do que fazer entra no rodapé (uma linha, causa honesta — o dev
    // vê que o painel existe e está vazio).
    let hint = if screen.degraded {
        // Degradado não há painel em foco visível: sem dica.
        None
    } else {
        state::empty_panel_hint(
            snapshot,
            screen.panel,
            screen.findings_len,
            screen.sessions_len,
            screen.wiki_len,
        )
    };
    let footer = if screen.wiki_search_open {
        format!(
            "busca wiki: {}▌ (j/k mover · Enter confirma · q/Esc saem)",
            screen.wiki_query
        )
    } else {
        match hint {
            Some(hint) => format!(
                "painel {}/{} · {} · {}",
                screen.panel.index() + 1,
                PANEL_COUNT,
                hint,
                state::footer_line()
            ),
            None => format!(
                "painel {}/{} · {}",
                screen.panel.index() + 1,
                PANEL_COUNT,
                state::footer_line()
            ),
        }
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
            // `degraded_selected` vira scroll da lista única: as teclas j/k
            // continuam fazendo algo útil na degradação.
            .scroll((
                u16::try_from(screen.degraded_selected.min(narrow.len().saturating_sub(1)))
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
                        screen.findings_selected,
                        true,
                        empty_line.clone(),
                        palette,
                    ),
                    detail_cols[0],
                );
                if let Some(row) = snapshot.finding_rows.get(screen.findings_selected) {
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
                        screen.sessions_selected,
                        true,
                        empty_sessions_line.clone(),
                        palette,
                    ),
                    detail_cols[0],
                );
                if let Some(row) = snapshot.session_rows.get(screen.sessions_selected) {
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
                        screen.findings_selected,
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
                        screen.sessions_selected,
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
                        screen.wiki_selected,
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
        && let Some(page) = filtered_wiki.get(screen.wiki_selected)
    {
        frame.render_widget(
            WikiPageWidget::new(page, screen.preview_scroll, palette),
            area,
        );
    }
    if screen.help_open {
        // As pendências (causas integrais) vêm do snapshot — mesma função
        // pura que alimenta a seção do `--snapshot`.
        frame.render_widget(
            help_paragraph(&palette, &state::pending_causes(snapshot)),
            area,
        );
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

/// Overlay de ajuda: os mesmos atalhos do rodapé, com o que cada um faz —
/// mais a seção de pendências com a causa INTEGRAL de cada dado ausente (o
/// cabeçalho corta o valor em 100 chars; o `?` mostra o passo a passo todo,
/// inclusive a dica do `kira doctor`, em várias linhas — é texto, não painel).
fn help_paragraph(palette: &Palette, pendings: &[String]) -> Paragraph<'static> {
    let mut text = vec![
        "ajuda do kira dashboard (read-only — nenhuma tecla escreve)".to_string(),
        String::new(),
        "q                sair".to_string(),
        "Esc              sair (em qualquer estado; Ctrl+C idem)".to_string(),
        "Tab / Shift+Tab  ciclar painel (cabeçalho ↔ achados ↔ sessões ↔ wiki)".to_string(),
        "j / k ou ↑ / ↓   mover linha do painel em foco".to_string(),
        "g / G            topo / fim".to_string(),
        "Enter            abrir/fechar detalhe (achado ou sessão) ou página da wiki".to_string(),
        "/                busca incremental na wiki (Enter confirma)".to_string(),
        "j/k, PgUp/PgDn   rolar a página da wiki aberta em tela cheia".to_string(),
        "r                reler o disco (sem rede, sem escrita)".to_string(),
        "?                abrir/fechar esta ajuda".to_string(),
    ];
    if !pendings.is_empty() {
        text.push(String::new());
        text.push(
            "pendências (cabeçalho corta em uma linha; aqui vai a causa inteira):".to_string(),
        );
        text.extend(pendings.iter().cloned());
    }
    text.push(String::new());
    text.push("Dado ausente aparece como — com a causa; nada é inventado.".to_string());
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
    fn esc_quits_in_every_state_and_so_do_q_and_ctrl_c() {
        // Contrato do dono (defeito real do smoke): q, Esc e Ctrl+C saem do
        // app em QUALQUER estado — a checagem é a primeira do decide(),
        // antes de ajuda/detalhe/busca/preview engolirem a tecla.
        let normal = findings_state();
        let with_help = AppState {
            help_open: true,
            ..findings_state()
        };
        let with_detail = AppState {
            detail_open: true,
            ..findings_state()
        };
        let searching = AppState {
            wiki_search_open: true,
            wiki_query: "se".to_string(),
            ..AppState::default()
        };
        let with_page = AppState {
            page_open: true,
            ..AppState::default()
        };
        let ctrl_c = KeyEvent {
            modifiers: KeyModifiers::CONTROL,
            ..KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE)
        };
        for state in [&normal, &with_help, &with_detail, &searching, &with_page] {
            assert_eq!(
                decide(state, plain(KeyCode::Esc)),
                Outcome::Quit,
                "{state:?}"
            );
            assert_eq!(
                decide(state, plain(KeyCode::Char('q'))),
                Outcome::Quit,
                "{state:?}"
            );
            assert_eq!(decide(state, ctrl_c), Outcome::Quit, "{state:?}");
        }
    }

    #[test]
    fn enter_closes_the_modals_because_esc_now_quits() {
        // Com Esc dedicado a SAIR, fechar overlay virou trabalho do Enter
        // (detalhe/página são toggle) e da confirmação da busca.
        let with_detail = AppState {
            detail_open: true,
            ..findings_state()
        };
        assert_eq!(
            decide(&with_detail, plain(KeyCode::Enter)),
            Outcome::ToggleDetail
        );
        let with_page = AppState {
            panel: Panel::Wiki,
            wiki_len: 2,
            page_open: true,
            ..AppState::default()
        };
        assert_eq!(
            decide(&with_page, plain(KeyCode::Enter)),
            Outcome::ClosePage
        );
        let searching = AppState {
            wiki_search_open: true,
            wiki_query: "se".to_string(),
            ..AppState::default()
        };
        assert_eq!(
            decide(&searching, plain(KeyCode::Enter)),
            Outcome::CloseSearch
        );
        // A ajuda continua fechando com `?` (toggle).
        let with_help = AppState {
            help_open: true,
            ..findings_state()
        };
        assert_eq!(decide(&with_help, plain(KeyCode::Char('?'))), Outcome::Help);
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
    fn move_is_scoped_to_the_focused_panel_never_the_others() {
        // Defeito real do smoke do dono (bug 2): com foco em achados, ↓
        // movia também a seleção da lista de sessões — o índice era único e
        // compartilhado. Agora cada painel tem o seu índice e o Move muta
        // SOMENTE o painel em foco.
        let mut screen = AppState {
            panel: Panel::Findings,
            findings_selected: 0,
            sessions_selected: 2,
            wiki_selected: 1,
            findings_len: 3,
            sessions_len: 3,
            wiki_len: 2,
            ..AppState::default()
        };
        let outcome = decide(&screen, plain(KeyCode::Down));
        apply_outcome(&mut screen, outcome);
        assert_eq!(screen.findings_selected, 1, "achados anda 1 linha");
        assert_eq!(screen.sessions_selected, 2, "sessões NÃO anda (bug 2)");
        assert_eq!(screen.wiki_selected, 1, "wiki NÃO anda (bug 2)");
        // E vice-versa: mover em sessões não mexe em achados.
        let mut other = AppState {
            panel: Panel::Sessions,
            findings_selected: 2,
            sessions_selected: 0,
            findings_len: 3,
            sessions_len: 3,
            ..AppState::default()
        };
        apply_outcome(&mut other, Outcome::Move(1));
        assert_eq!(other.sessions_selected, 1, "sessões anda 1 linha");
        assert_eq!(other.findings_selected, 2, "achados NÃO anda");
        // g/G satura no índice do próprio painel, sem tocar nos outros.
        let mut third = AppState {
            panel: Panel::Wiki,
            wiki_selected: 0,
            ..AppState::default()
        };
        apply_outcome(&mut third, Outcome::Move(isize::MAX));
        assert_eq!(
            third.wiki_selected,
            isize::MAX as usize,
            "wiki vai ao fim (saturação do apply_move)"
        );
        assert_eq!(third.findings_selected, 0, "achados intocado");
        assert_eq!(third.sessions_selected, 0, "sessões intocada");
        // Degradado: a lista única tem o próprio índice de scroll.
        let mut degraded = AppState {
            degraded: true,
            degraded_selected: 0,
            ..AppState::default()
        };
        apply_outcome(&mut degraded, Outcome::Move(1));
        assert_eq!(degraded.degraded_selected, 1, "lista única anda");
        assert_eq!(degraded.findings_selected, 0);
    }

    #[test]
    fn tab_visits_the_empty_wiki_panel_full_cycle_of_four() {
        // Defeito real do smoke no Windows (bug 3): o Release repetia o ciclo
        // e a wiki vazia nunca ficava em foco. Aqui o ciclo é provado passo a
        // passo COM a wiki vazia: painel vazio existe e nunca é pulado.
        let mut screen = AppState::default(); // cabeçalho; wiki_len = 0
        assert_eq!(screen.panel, Panel::Header);
        let outcome = decide(&screen, plain(KeyCode::Tab));
        apply_outcome(&mut screen, outcome);
        assert_eq!(screen.panel, Panel::Findings);
        let outcome = decide(&screen, plain(KeyCode::Tab));
        apply_outcome(&mut screen, outcome);
        assert_eq!(screen.panel, Panel::Sessions);
        let outcome = decide(&screen, plain(KeyCode::Tab));
        apply_outcome(&mut screen, outcome);
        assert_eq!(
            screen.panel,
            Panel::Wiki,
            "a wiki VAZIA é visitada pelo Tab"
        );
        let outcome = decide(&screen, plain(KeyCode::Tab));
        apply_outcome(&mut screen, outcome);
        assert_eq!(screen.panel, Panel::Header, "o ciclo fecha em 4");
        // Shift+Tab também volta passando pela wiki vazia.
        let outcome = decide(&screen, plain(KeyCode::BackTab));
        apply_outcome(&mut screen, outcome);
        assert_eq!(screen.panel, Panel::Wiki, "Shift+Tab visita a wiki vazia");
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
        assert!(
            text.contains("Enter            abrir/fechar detalhe"),
            "{text}"
        );
        assert!(
            text.contains("Esc              sair"),
            "o Esc documentado na ajuda (agora sai do app): {text}"
        );
    }

    #[test]
    fn help_overlay_lists_the_full_pat_cause_with_the_doctor_hint() {
        // A causa de DUAS linhas do PAT mora agora no `?` (texto) — o
        // cabeçalho mostra só o ponteiro curto, nunca a causa estourada.
        let snapshot = Snapshot {
            pat_note: Some(
                "PAT não encontrado — defina AZURE_DEVOPS_PAT (ex.: export AZURE_DEVOPS_PAT=...)\n\
                 para diagnosticar a configuração desta máquina, rode `kira doctor`"
                    .to_string(),
            ),
            ..Snapshot::default()
        };
        let screen = AppState {
            panel: Panel::Header,
            help_open: true,
            ..AppState::default()
        };
        let text = render_full(&snapshot, &screen, 120, 40);
        assert!(text.contains("pendências"), "seção na ajuda: {text}");
        assert!(
            text.contains("kira doctor"),
            "a dica do doctor INTEIRA no overlay: {text}"
        );
        assert!(
            text.contains("defina AZURE_DEVOPS_PAT"),
            "o passo a passo completo no overlay: {text}"
        );
        // Sem pendências, a seção não existe (ajuda limpa).
        let clean = render_full(&Snapshot::default(), &screen, 120, 40);
        assert!(
            !clean.contains("pendências"),
            "nada de seção vazia: {clean}"
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
    fn tab_cycle_renders_each_panel_title_with_the_wiki_empty() {
        // TestBackend (bug 3): o ciclo de Tab pelos 4 painéis COM a wiki
        // vazia — o rodapé muda em cada passo e, no passo da wiki, o painel
        // vazio aparece com a causa honesta (nunca pulado, nunca escondido).
        let snapshot = Snapshot::default();
        let mut screen = AppState::default();
        for expected in ["painel 1/4", "painel 2/4", "painel 3/4", "painel 4/4"] {
            let text = render_full(&snapshot, &screen, 100, 30);
            assert!(
                text.contains(expected),
                "o título do rodapé muda a cada passo ({expected}): {text}"
            );
            if expected == "painel 4/4" {
                assert!(
                    text.contains("wiki: — (wiki vazia — rode kira wiki reindex)"),
                    "o painel de wiki VAZIO está visível com a causa: {text}"
                );
                assert!(text.contains(" wiki "), "título do bloco da wiki: {text}");
            }
            let outcome = decide(&screen, plain(KeyCode::Tab));
            apply_outcome(&mut screen, outcome);
        }
        // O ciclo fechou: de volta ao cabeçalho.
        assert_eq!(screen.panel, Panel::Header);
    }

    #[test]
    fn footer_shows_the_hint_when_the_focused_panel_is_empty() {
        // EXTRA: painel focado vazio ⇒ o rodapé mostra a dica do que fazer em
        // uma linha, e `q sair` continua sendo o primeiro item das teclas.
        let snapshot = Snapshot::default();
        let screen = AppState {
            panel: Panel::Wiki,
            ..AppState::default()
        };
        let text = render_full(&snapshot, &screen, 100, 30);
        assert!(
            text.contains("painel 4/4 · wiki vazia — rode kira wiki reindex"),
            "dica do painel vazio no rodapé: {text}"
        );
        assert!(
            text.contains("teclas: q sair"),
            "`q sair` segue primeiro no rodapé: {text}"
        );
        // Painel focado COM conteúdo: sem dica (o rodapé volta ao padrão).
        let full = snapshot_with_wiki_pages();
        let screen = AppState {
            panel: Panel::Wiki,
            wiki_len: 2,
            ..AppState::default()
        };
        let text = render_full(&full, &screen, 100, 30);
        assert!(
            text.contains("painel 4/4 · teclas: q sair"),
            "sem dica quando o painel tem conteúdo: {text}"
        );
        // Cabeçalho em foco nunca tem dica (não é lista).
        let screen = AppState::default();
        let text = render_full(&full, &screen, 100, 30);
        assert!(
            text.contains("painel 1/4 · teclas: q sair"),
            "cabeçalho não ganha dica de lista vazia: {text}"
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
    fn search_mode_turns_keys_into_text_and_enter_confirms() {
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
        // Backspace apaga; Enter confirma e fecha a caixa (agulha fica no
        // filtro). Esc NÃO cancela mais: sai do app (contrato do dono).
        assert_eq!(
            decide(&searching, plain(KeyCode::Backspace)),
            Outcome::SearchBackspace
        );
        assert_eq!(
            decide(&searching, plain(KeyCode::Enter)),
            Outcome::CloseSearch
        );
        assert_eq!(decide(&searching, plain(KeyCode::Esc)), Outcome::Quit);
        // E a própria busca não engole a tecla de sair: `q` na caixa de texto
        // sai do app (a checagem vem antes do modal).
        assert_eq!(decide(&searching, plain(KeyCode::Char('q'))), Outcome::Quit);
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
    fn page_view_is_modal_enter_backs_and_scroll_by_line_and_page() {
        let open = AppState {
            panel: Panel::Wiki,
            wiki_len: 2,
            page_open: true,
            ..AppState::default()
        };
        // Esc agora SAI do app (contrato do dono); Enter volta à lista.
        assert_eq!(decide(&open, plain(KeyCode::Esc)), Outcome::Quit);
        assert_eq!(decide(&open, plain(KeyCode::Enter)), Outcome::ClosePage);
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
