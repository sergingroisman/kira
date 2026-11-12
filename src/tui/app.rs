//! Loop de eventos do `kira dashboard` — casca derivada do statui (ADR-0014).
//!
//! Ciclo: draw (ratatui) → poll de tecla (crossterm) → [`decide`] → mutação
//! mínima de estado de tela ou releitura do disco (`r`). O snapshot é
//! recomposto apenas em `Reload` — nunca por frame (spec W6).
//!
//! Quatro fontes (`1–4`/`Tab`): Resumo, Achados, Sessões, Wiki. Cada uma ocupa
//! a área de trabalho inteira; o inspetor abre ao lado em 50–50 (`i`/`Enter`),
//! como no statui (`third_party/statui/NOTICE.md`).
//!
//! Contrato (ADR-0033): navegar NUNCA executa. O [`Outcome`] não tem variante
//! de escrita — a camada de ação só entra com tecla explícita + confirmação.
//!
//! A tela NUNCA é deixada suja: o guard [`ScreenGuard`] devolve raw
//! mode/alternate screen no Drop e o hook de pânico restaura o terminal ANTES
//! do backtrace.

use std::io;
use std::time::Duration;

use crossterm::event::KeyModifiers as Mods;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::tui::action::{
    self, Action as StepAction, ActionCtx, ConfirmState, ExecState, Request, RunEvent, Runner,
    TargetSheet,
};
use crate::tui::chrome;
use crate::tui::glyph::Glyphs;
use crate::tui::keymap::{self, Action as Key};
use crate::tui::layout;
use crate::tui::state::{self, Snapshot, WikiPage};
use crate::tui::theme::{self, Palette};
use crate::tui::widgets::cards::{self, Live};
use crate::tui::widgets::findings::{DetailWidget, FindingsWidget};
use crate::tui::widgets::footer;
use crate::tui::widgets::sessions::SessionsWidget;
use crate::tui::widgets::wiki::{WikiPageWidget, WikiWidget};

/// Quantidade de fontes no ciclo de `Tab`/`1–4`.
pub const SCREEN_COUNT: usize = 4;

/// Passo de PgUp/PgDn no preview/lista (linhas por "página" de scroll).
pub const SCROLL_PAGE_STEP: isize = 10;

/// Quantos scans a ajuda lista antes de resumir o resto. A série inteira
/// continua no `--snapshot`; aqui o teto é a ALTURA da tela, não o dado.
pub const HELP_TREND_LINES: usize = 10;

/// Opções de abertura do dashboard (ADR-0033).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RunOptions {
    /// Desliga toda tecla de ação; navegação continua. É o modo do e2e de
    /// hash e de quem só quer olhar.
    pub read_only: bool,
}

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

/// Fonte de dados em foco (as quatro telas do dashboard).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    /// Home: risco, atividade recente e pendências.
    Resumo,
    /// Tabela de `findings.json`.
    Achados,
    /// Trilha de sessões da store.
    Sessoes,
    /// Páginas da wiki.
    Wiki,
}

impl Screen {
    /// Posição na ordem do ciclo (0-based; o chrome mostra 1-based nos rótulos).
    pub fn index(self) -> usize {
        match self {
            Screen::Resumo => 0,
            Screen::Achados => 1,
            Screen::Sessoes => 2,
            Screen::Wiki => 3,
        }
    }

    /// Constrói a partir da posição do ciclo, saturando em Resumo.
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Screen::Achados,
            2 => Screen::Sessoes,
            3 => Screen::Wiki,
            _ => Screen::Resumo,
        }
    }

    /// A tela tem registro que abre inspetor?
    pub fn has_inspector(self) -> bool {
        !matches!(self, Screen::Resumo)
    }
}

/// Próxima fonte do ciclo, com wrap nas duas direções.
pub fn next_screen(screen: Screen, direction: i32) -> Screen {
    let next = (screen.index() as i32 + direction).rem_euclid(SCREEN_COUNT as i32) as usize;
    Screen::from_index(next)
}

/// Estado de TELA (não de domínio): o que o loop muta por tecla. O snapshot de
/// dados vive à parte (`Snapshot`) e só muda em `Reload`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppState {
    /// Fonte em foco.
    pub screen: Screen,
    /// Contador do loop de eventos: o spinner avança com ele, nunca dentro do
    /// widget (mesmo `tick` = mesmo frame, o desenho continua determinístico).
    pub tick: u64,
    /// Inspetor 50–50 aberto na fonte atual.
    pub inspecting: bool,
    /// Seleção da tabela de achados (o ➤). UM POR TELA.
    pub findings_selected: usize,
    /// Filtro de severidade ativo (`f` cicla; `None` = todas).
    pub severity_filter: Option<String>,
    /// Filtro de ferramenta ativo (`m` cicla; `None` = todas).
    pub tool_filter: Option<String>,
    /// Valores de severidade PRESENTES no snapshot (ordem canônica).
    pub severity_values: Vec<String>,
    /// Ferramentas PRESENTES no snapshot (ordem alfabética).
    pub tool_values: Vec<String>,
    /// Seleção da tabela de sessões — própria, nunca compartilhada.
    pub sessions_selected: usize,
    /// Seleção da lista de wiki (sobre a lista FILTRADA).
    pub wiki_selected: usize,
    /// Scroll da lista ÚNICA do modo degradado.
    pub degraded_selected: usize,
    /// Modo degradado ativo (terminal menor que 80×24).
    pub degraded: bool,
    /// Overlay de ajuda aberto (?).
    pub help_open: bool,
    /// Quantidade de linhas que `decide` enxerga por fonte.
    pub findings_len: usize,
    pub sessions_len: usize,
    pub wiki_len: usize,
    /// Busca incremental da wiki aberta (`/`).
    pub wiki_search_open: bool,
    /// Agulha da busca (aplicada por `state::filter_pages`).
    pub wiki_query: String,
    /// Leitura de página em tela cheia (Enter na wiki).
    pub page_open: bool,
    /// Scroll vertical do preview (clampado pelo widget).
    pub preview_scroll: usize,
    /// `--read-only`: a UI mostra o estado e não oferece ação.
    pub read_only: bool,
    /// Ids de work item já conhecidos (args de sessões + wiki), recarregados a
    /// cada frame a partir do snapshot — a folha de alvo não inventa id.
    pub known_work_items: Vec<u32>,
    /// Folha de alvo aberta (`t`/`d` antes de qualquer execução).
    pub target: Option<TargetSheet>,
    /// Popup de confirmação aberto: plano montado, NADA executado.
    pub confirm: Option<ConfirmState>,
    /// Ação já com id, aguardando o `run` montar o plano (I/O fica lá).
    pub pending: Option<StepAction>,
    /// Execução em andamento (ou a última que terminou), para o overlay `o`.
    pub exec: Option<ExecState>,
    /// Overlay de execução aberto.
    pub exec_open: bool,
    /// Aviso de uma linha no rodapé (erro acionável, plano recusado).
    pub notice: Option<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            screen: Screen::Resumo,
            tick: 0,
            inspecting: false,
            findings_selected: 0,
            severity_filter: None,
            tool_filter: None,
            severity_values: Vec::new(),
            tool_values: Vec::new(),
            sessions_selected: 0,
            wiki_selected: 0,
            degraded_selected: 0,
            degraded: false,
            help_open: false,
            findings_len: 0,
            sessions_len: 0,
            wiki_len: 0,
            wiki_search_open: false,
            wiki_query: String::new(),
            page_open: false,
            preview_scroll: 0,
            read_only: false,
            known_work_items: Vec::new(),
            target: None,
            confirm: None,
            pending: None,
            exec: None,
            exec_open: false,
            notice: None,
        }
    }
}

/// O que o loop executa depois de [`decide`].
///
/// Não existe variante de escrita/rede/LLM: navegar nunca executa (ADR-0033).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Sair do dashboard.
    Quit,
    /// Redesenhar sem mudar estado (Ctrl+L).
    Redraw,
    /// Reler o dashboard e o store (`r`).
    Reload,
    /// Abrir/fechar o inspetor da fonte atual (`i`/`Enter`/`Esc`).
    ToggleInspector,
    /// Ir direto para uma fonte (`1`–`4`).
    GoScreen(Screen),
    /// Ciclar fonte (+1 = Tab, -1 = Shift+Tab), com wrap.
    CycleScreen(i32),
    /// Mover a seleção (+1 desce; `isize::MIN`/`MAX` = topo/fim).
    Move(isize),
    /// Ciclar o filtro de severidade dos achados (`f`).
    CycleSeverityFilter,
    /// Ciclar o filtro de ferramenta dos achados (`m`).
    CycleToolFilter,
    /// Limpar os dois filtros (`Esc` sem outra camada aberta).
    ClearFindingsFilters,
    /// Abrir a busca incremental da wiki (`/`).
    SearchWiki,
    /// Digitar um caractere na busca aberta.
    SearchChar(char),
    /// Apagar o último caractere da busca.
    SearchBackspace,
    /// Fechar a busca (`Esc` cancela a agulha).
    CloseSearch,
    /// Abrir a página selecionada da wiki em tela cheia.
    OpenPage,
    /// Fechar a leitura em tela cheia.
    ClosePage,
    /// Rolar a leitura (j/k por linha, PgUp/PgDn por página).
    ScrollPage(isize),
    /// Alternar o overlay de ajuda (?).
    Help,
    /// Pedido de ação (`s`/`t`/`d`): abre folha de alvo ou vira plano.
    RequestAction(Request),
    /// Dígito digitado na folha de alvo.
    TargetPush(char),
    /// Apagar o último dígito da folha de alvo.
    TargetBackspace,
    /// Andar na lista de ids conhecidos (Tab/Shift+Tab/↑/↓).
    TargetCycle(isize),
    /// Confirmar o id da folha (Enter) — ainda NÃO executa.
    TargetConfirm,
    /// Fechar a folha de alvo.
    TargetCancel,
    /// Executar o plano confirmado (só `Enter` no popup, com `read_only` off).
    ConfirmExecute,
    /// Fechar o popup sem executar.
    ConfirmCancel,
    /// Abrir/fechar o overlay da execução (`o`).
    ToggleExecution,
    /// Rolar o log da execução.
    ScrollExecution(isize),
    /// Religar o follow no fim do log (`G`).
    FollowExecution,
    /// Tecla ignorada.
    Ignore,
}

/// Aplica um offset de movimento com saturação nas duas pontas: `g`/`G`
/// chegam como `isize::MIN`/`MAX` e nunca dão a volta no `usize`.
pub fn apply_move(selected: usize, offset: isize) -> usize {
    if offset < 0 {
        selected.saturating_sub(offset.unsigned_abs())
    } else if offset == isize::MAX {
        // `G` = fim absoluto; o loop clampa pela quantidade real de linhas.
        usize::MAX
    } else {
        selected.saturating_add(offset as usize)
    }
}

/// Limite de seleção pela quantidade REAL de linhas visíveis.
pub fn max_selected_for(
    screen: Screen,
    findings_len: usize,
    sessions_len: usize,
    wiki_len: usize,
    degraded_total_lines: usize,
    degraded: bool,
) -> usize {
    if degraded {
        return degraded_total_lines.max(1) - 1;
    }
    match screen {
        Screen::Achados => findings_len.max(1) - 1,
        Screen::Sessoes => sessions_len.max(1) - 1,
        Screen::Wiki => wiki_len.max(1) - 1,
        Screen::Resumo => 0,
    }
}

/// A fonte atual tem registro para abrir?
fn screen_has_rows(screen: &AppState) -> bool {
    match screen.screen {
        Screen::Achados => screen.findings_len > 0,
        Screen::Sessoes => screen.sessions_len > 0,
        Screen::Wiki => screen.wiki_len > 0,
        Screen::Resumo => false,
    }
}

/// Próximo valor do ciclo de filtro: `None` → values[0] → … → `None`.
fn next_filter(current: Option<&str>, values: &[String]) -> Option<String> {
    let Some(current) = current else {
        return values.first().cloned();
    };
    let index = values.iter().position(|value| value == current);
    match index {
        Some(index) if index + 1 < values.len() => Some(values[index + 1].clone()),
        // Fim do ciclo volta para "todas" — o filtro nunca fica preso.
        Some(_) | None => None,
    }
}

/// Os dois filtros estão ativos? (`Esc` sem outra camada limpa).
pub fn filters_active(screen: &AppState) -> bool {
    screen.severity_filter.is_some() || screen.tool_filter.is_some()
}

/// Rótulo do filtro para o título do bloco (`critical` / `critical · audit`).
pub fn findings_filter_label(screen: &AppState) -> Option<String> {
    match (&screen.severity_filter, &screen.tool_filter) {
        (None, None) => None,
        (Some(severity), None) => Some(severity.clone()),
        (None, Some(tool)) => Some(tool.clone()),
        (Some(severity), Some(tool)) => Some(format!("{severity} · {tool}")),
    }
}

/// Lista VISÍVEL de achados (filtro em memória, puro sobre o snapshot).
pub fn filtered_findings<'a>(
    snapshot: &'a Snapshot,
    screen: &AppState,
) -> Vec<&'a state::FindingRow> {
    state::filter_findings(
        &snapshot.finding_rows,
        screen.severity_filter.as_deref(),
        screen.tool_filter.as_deref(),
    )
}

/// Executa um [`Outcome`] sobre o estado de TELA — função pura de mutação.
///
/// `Quit` e `Reload` NÃO são daqui: o primeiro devolve o controle ao `run`
/// (restauro do terminal), o segundo relê o disco (I/O fica no loop).
pub fn apply_outcome(screen: &mut AppState, outcome: Outcome) {
    match outcome {
        Outcome::Redraw => {}
        Outcome::ToggleInspector => {
            if screen.screen.has_inspector() && screen_has_rows(screen) {
                screen.inspecting = !screen.inspecting;
            }
        }
        Outcome::GoScreen(next) => {
            screen.screen = next;
            screen.inspecting = false;
            screen.page_open = false;
            screen.wiki_search_open = false;
        }
        Outcome::CycleScreen(direction) => {
            screen.screen = next_screen(screen.screen, direction);
            screen.inspecting = false;
            screen.page_open = false;
            screen.wiki_search_open = false;
        }
        Outcome::Move(offset) => {
            if screen.degraded {
                screen.degraded_selected = apply_move(screen.degraded_selected, offset);
            } else {
                match screen.screen {
                    Screen::Achados => {
                        screen.findings_selected = apply_move(screen.findings_selected, offset)
                    }
                    Screen::Sessoes => {
                        screen.sessions_selected = apply_move(screen.sessions_selected, offset)
                    }
                    Screen::Wiki => screen.wiki_selected = apply_move(screen.wiki_selected, offset),
                    // O Resumo não é lista: ↑/↓ não inventam seleção.
                    Screen::Resumo => {}
                }
            }
        }
        Outcome::CycleSeverityFilter => {
            screen.severity_filter =
                next_filter(screen.severity_filter.as_deref(), &screen.severity_values);
            // Lista nova começa do topo e fecha o inspetor do registro antigo.
            screen.findings_selected = 0;
            screen.inspecting = false;
        }
        Outcome::CycleToolFilter => {
            screen.tool_filter = next_filter(screen.tool_filter.as_deref(), &screen.tool_values);
            screen.findings_selected = 0;
            screen.inspecting = false;
        }
        Outcome::ClearFindingsFilters => {
            screen.severity_filter = None;
            screen.tool_filter = None;
            screen.findings_selected = 0;
        }
        Outcome::SearchWiki => {
            screen.wiki_search_open = true;
            screen.wiki_query.clear();
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
        Outcome::ScrollPage(offset) => {
            screen.preview_scroll = apply_move(screen.preview_scroll, offset)
        }
        Outcome::RequestAction(request) => {
            if !screen.read_only {
                if request.needs_work_item() {
                    // O id vem da lista conhecida ou do teclado (folha de alvo).
                    let known = screen.known_work_items.clone();
                    screen.target = Some(TargetSheet::new(request, known));
                } else {
                    screen.pending = request.into_action(None);
                }
            }
        }
        Outcome::TargetPush(value) => {
            if let Some(sheet) = &mut screen.target {
                sheet.push_digit(value);
            }
        }
        Outcome::TargetBackspace => {
            if let Some(sheet) = &mut screen.target {
                sheet.backspace();
            }
        }
        Outcome::TargetCycle(direction) => {
            if let Some(sheet) = &mut screen.target {
                sheet.cycle(direction);
            }
        }
        Outcome::TargetConfirm => {
            let chosen = screen.target.as_ref().and_then(TargetSheet::action);
            match chosen {
                Some(step) => {
                    screen.target = None;
                    screen.pending = Some(step);
                }
                // Sem id válido a folha CONTINUA aberta: Enter não inventa
                // work item nem fecha o que o dono ainda está digitando.
                None => {
                    screen.notice =
                        Some("work item inválido — digite um número maior que zero".to_string());
                }
            }
        }
        Outcome::TargetCancel => screen.target = None,
        // Quem spawna é o `run` (I/O fica lá); aqui o popup apenas fecha.
        Outcome::ConfirmExecute | Outcome::ConfirmCancel => screen.confirm = None,
        Outcome::ToggleExecution => {
            if screen.exec.is_some() {
                screen.exec_open = !screen.exec_open;
            }
        }
        Outcome::ScrollExecution(offset) => {
            if let Some(exec) = &mut screen.exec {
                exec.scroll_by(offset);
                screen.exec_open = true;
            }
        }
        Outcome::FollowExecution => {
            if let Some(exec) = &mut screen.exec {
                exec.follow = true;
                exec.scroll = 0;
                screen.exec_open = true;
            }
        }
        Outcome::Help => screen.help_open = !screen.help_open,
        Outcome::Ignore => {}
        Outcome::Quit | Outcome::Reload => {}
    }
}

/// A tecla é uma das três de AÇÃO? (`s`/`t`/`d` — `o` é só visualização).
fn is_action_key(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('s' | 't' | 'd'))
}

/// Monta o [`action::ActionPlan`] com o ambiente REAL (binário, PATH,
/// credenciais). É o único ponto com I/O — e roda só quando já existe um
/// pedido de ação, nunca por frame.
fn build_plan(step: StepAction, snapshot: &Snapshot) -> Result<action::ActionPlan, String> {
    let program = std::env::current_exe()
        .map_err(|err| format!("não foi possível localizar o binário do kira: {err}"))?;
    let org_project = credentials_org_project();
    let tools = |name: &str| {
        crate::binresolve::find_on_path(std::env::var_os("PATH").as_deref(), name).is_some()
    };
    let ctx = ActionCtx {
        root: snapshot.root.as_deref(),
        program: &program,
        org_project: org_project
            .as_ref()
            .map(|(org, project)| (org.as_str(), project.as_str())),
        pat_present: snapshot.pat_masked.is_some(),
        tools: &tools,
    };
    action::plan(step, &ctx).map_err(|err| err.message().to_string())
}

/// `(org, projeto)` resolvidos como o CLI resolveria (config/env), sem rede.
fn credentials_org_project() -> Option<(String, String)> {
    let org = crate::credentials::resolve_org_with_source().ok()?.value;
    let project = crate::credentials::resolve_project_with_source(None)
        .ok()?
        .value;
    Some((org, project))
}

/// Drena o canal da execução sem bloquear: aplica linha a linha e, quando o
/// processo sai, relê o disco (o filho gravou trilha/findings/wiki) e reancora
/// a seleção na MESMA linha.
fn drain_execution(
    state_screen: &mut AppState,
    runner: &mut Option<Runner>,
    snapshot: &mut Snapshot,
) {
    let Some(active) = runner.take() else {
        return;
    };
    let mut finished = false;
    while let Ok(event) = active.rx.try_recv() {
        finished |= matches!(&event, RunEvent::Exited { .. });
        if let Some(exec) = &mut state_screen.exec {
            exec.apply(event);
        }
    }
    if !finished {
        *runner = Some(active);
        return;
    }
    let key = selection_key(snapshot, state_screen);
    *snapshot = state::load();
    reanchor(state_screen, key, snapshot);
    // `active` cai aqui; o `Drop` só faz `join` (o filho já saiu).
}

/// Lista visível de achados (mesmo filtro da tabela) — base da reancoragem.
fn visible_findings<'a>(
    snapshot: &'a Snapshot,
    state_screen: &AppState,
) -> Vec<&'a state::FindingRow> {
    state::filter_findings(
        &snapshot.finding_rows,
        state_screen.severity_filter.as_deref(),
        state_screen.tool_filter.as_deref(),
    )
}

/// Chave estável do que está selecionado na tela atual.
fn selection_key(snapshot: &Snapshot, state_screen: &AppState) -> Option<state::SelectionKey> {
    match state_screen.screen {
        Screen::Achados => visible_findings(snapshot, state_screen)
            .get(state_screen.findings_selected)
            .map(|row| state::SelectionKey::Finding(row.fingerprint.clone())),
        Screen::Sessoes => snapshot
            .session_rows
            .get(state_screen.sessions_selected)
            .map(|row| state::SelectionKey::Session(row.id)),
        Screen::Wiki => {
            let filtered = state::filter_pages(&snapshot.wiki_rows, &state_screen.wiki_query);
            state::wiki_ordered(&filtered)
                .get(state_screen.wiki_selected)
                .map(|page| state::SelectionKey::Wiki(page.slug.clone()))
        }
        Screen::Resumo => None,
    }
}

/// Move a seleção para o registro da chave; se ele sumiu, mantém o índice
/// (o clamp do loop cuida do resto) — nunca "pula" para outro item.
fn reanchor(state_screen: &mut AppState, key: Option<state::SelectionKey>, snapshot: &Snapshot) {
    let Some(key) = key else {
        return;
    };
    match key {
        state::SelectionKey::Finding(fingerprint) => {
            if let Some(index) = visible_findings(snapshot, state_screen)
                .iter()
                .position(|row| row.fingerprint == fingerprint)
            {
                state_screen.findings_selected = index;
            }
        }
        state::SelectionKey::Session(id) => {
            if let Some(index) = snapshot.session_rows.iter().position(|row| row.id == id) {
                state_screen.sessions_selected = index;
            }
        }
        state::SelectionKey::Wiki(slug) => {
            let filtered = state::filter_pages(&snapshot.wiki_rows, &state_screen.wiki_query);
            if let Some(index) = state::wiki_ordered(&filtered)
                .iter()
                .position(|page| page.slug == slug)
            {
                state_screen.wiki_selected = index;
            }
        }
    }
}

/// DECISÃO PURA da tecla: estado de tela + `KeyEvent` → [`Outcome`].
///
/// Ordem dos modais: busca → leitura → ajuda. SAIR (Ctrl+C e `q` fora da
/// busca) vem antes de tudo. `Esc` é SEMPRE voltar (fecha a camada aberta),
/// nunca sai do app — contrato do dono. Nenhuma tecla escreve.
pub fn decide(state: &AppState, key: KeyEvent) -> Outcome {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Outcome::Quit;
    }
    // `--read-only` desliga a tecla de AÇÃO antes de qualquer modal: navegar
    // continua, pedir execução não (ADR-0033 §4). A guarda é aqui, não só no
    // rodapé — é o que o e2e de hash prova.
    if state.read_only && is_action_key(key) {
        return Outcome::Ignore;
    }
    match key.code {
        KeyCode::Esc => {
            // Cascata: o overlay da execução fecha primeiro, depois a folha de
            // alvo e o popup — nenhuma dessas camadas executa nada.
            if state.exec_open {
                return Outcome::ToggleExecution;
            }
            if state.target.is_some() {
                return Outcome::TargetCancel;
            }
            if state.confirm.is_some() {
                return Outcome::ConfirmCancel;
            }
            if state.wiki_search_open {
                return Outcome::CloseSearch;
            }
            if state.page_open {
                return Outcome::ClosePage;
            }
            if state.help_open {
                return Outcome::Help;
            }
            if state.inspecting {
                return Outcome::ToggleInspector;
            }
            // Esc sem camada aberta limpa os filtros de achados.
            if filters_active(state) {
                return Outcome::ClearFindingsFilters;
            }
            return Outcome::Ignore;
        }
        KeyCode::Char('q') if !state.wiki_search_open => return Outcome::Quit,
        _ => {}
    }
    // Busca aberta é modal: toda tecla vira texto no rodapé.
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
    // Leitura da wiki em tela cheia é modal.
    if state.page_open {
        return match key.code {
            KeyCode::Enter => Outcome::ClosePage,
            KeyCode::Char('r') => Outcome::Reload,
            KeyCode::Char('j') | KeyCode::Down => Outcome::ScrollPage(1),
            KeyCode::Char('k') | KeyCode::Up => Outcome::ScrollPage(-1),
            KeyCode::PageDown => Outcome::ScrollPage(SCROLL_PAGE_STEP),
            KeyCode::PageUp => Outcome::ScrollPage(-SCROLL_PAGE_STEP),
            KeyCode::Char('g') => Outcome::ScrollPage(isize::MIN),
            KeyCode::Char('G') => Outcome::ScrollPage(isize::MAX),
            _ => Outcome::Ignore,
        };
    }
    // Ajuda é modal: `?`/`Esc` fecham, o resto não navega por baixo.
    if state.help_open {
        return match key.code {
            KeyCode::Char('?') => Outcome::Help,
            _ => Outcome::Ignore,
        };
    }
    // Overlay da execução: rola o log, `G` religa o follow, `o`/`Esc` fecham.
    // Nenhuma tecla aqui mata o processo (ADR-0033: Esc não cancela).
    if state.exec_open {
        return match key.code {
            KeyCode::Enter | KeyCode::Char('o') => Outcome::ToggleExecution,
            KeyCode::Char('j') | KeyCode::Down => Outcome::ScrollExecution(1),
            KeyCode::Char('k') | KeyCode::Up => Outcome::ScrollExecution(-1),
            KeyCode::PageDown => Outcome::ScrollExecution(SCROLL_PAGE_STEP),
            KeyCode::PageUp => Outcome::ScrollExecution(-SCROLL_PAGE_STEP),
            KeyCode::Char('g') => Outcome::ScrollExecution(isize::MIN),
            KeyCode::Char('G') => Outcome::FollowExecution,
            _ => Outcome::Ignore,
        };
    }
    // Folha de alvo é modal: dígitos digitam o id, Tab/↑/↓ andam na lista.
    if state.target.is_some() {
        return match key.code {
            KeyCode::Enter => Outcome::TargetConfirm,
            KeyCode::Tab | KeyCode::Down => Outcome::TargetCycle(1),
            KeyCode::BackTab | KeyCode::Up => Outcome::TargetCycle(-1),
            KeyCode::Backspace => Outcome::TargetBackspace,
            KeyCode::Char(value) if value.is_ascii_digit() => Outcome::TargetPush(value),
            _ => Outcome::Ignore,
        };
    }
    // Popup de confirmação: Enter executa (o ÚNICO caminho), Esc cancela.
    if state.confirm.is_some() {
        return match key.code {
            KeyCode::Enter => Outcome::ConfirmExecute,
            _ => Outcome::Ignore,
        };
    }
    match key.code {
        KeyCode::Enter => match state.screen {
            // Enter abre o inspetor com o registro selecionado.
            Screen::Achados if state.findings_len > 0 => Outcome::ToggleInspector,
            Screen::Sessoes if state.sessions_len > 0 => Outcome::ToggleInspector,
            // Wiki: Enter abre a leitura integral (o `i` abre o preview ao lado).
            Screen::Wiki if state.wiki_len > 0 => Outcome::OpenPage,
            _ => Outcome::Ignore,
        },
        KeyCode::Char('1') => Outcome::GoScreen(Screen::Resumo),
        KeyCode::Char('2') => Outcome::GoScreen(Screen::Achados),
        KeyCode::Char('3') => Outcome::GoScreen(Screen::Sessoes),
        KeyCode::Char('4') => Outcome::GoScreen(Screen::Wiki),
        KeyCode::Char('l') if key.modifiers.contains(Mods::CONTROL) => Outcome::Redraw,
        _ => match keymap::map(key) {
            Some(Key::Quit) => Outcome::Quit,
            Some(Key::NextPanel) => Outcome::CycleScreen(1),
            Some(Key::PrevPanel) => Outcome::CycleScreen(-1),
            Some(Key::Down) => Outcome::Move(1),
            Some(Key::Up) => Outcome::Move(-1),
            Some(Key::Top) => Outcome::Move(isize::MIN),
            Some(Key::Bottom) => Outcome::Move(isize::MAX),
            Some(Key::Reload) => Outcome::Reload,
            Some(Key::Help) => Outcome::Help,
            Some(Key::Inspect) => Outcome::ToggleInspector,
            Some(Key::SearchWiki) if state.screen == Screen::Wiki => Outcome::SearchWiki,
            Some(Key::SearchWiki) => Outcome::Ignore,
            // Pedido de AÇÃO: nunca spawna aqui — o loop monta o plano e o
            // popup é quem pede o Enter. `read_only` já foi barrado acima.
            Some(Key::Request(request)) => {
                if state.read_only {
                    Outcome::Ignore
                } else {
                    Outcome::RequestAction(request)
                }
            }
            // `o` só vale quando existe execução para abrir.
            Some(Key::Execution) if state.exec.is_some() => Outcome::ToggleExecution,
            Some(Key::Execution) => Outcome::Ignore,
            // `f`/`m` só existem na tela de Achados.
            // Na lista única degradada o filtro não muda o que aparece:
            // melhor ignorar do que fingir que filtrou.
            Some(Key::FilterSeverity) if state.screen == Screen::Achados && !state.degraded => {
                Outcome::CycleSeverityFilter
            }
            Some(Key::FilterTool) if state.screen == Screen::Achados && !state.degraded => {
                Outcome::CycleToolFilter
            }
            Some(Key::FilterSeverity | Key::FilterTool) => Outcome::Ignore,
            None => Outcome::Ignore,
        },
    }
}

/// Roda o modo interativo (só é chamado com stdout TTY). Erro é texto
/// acionável, nunca panic.
pub fn run(initial: Snapshot, options: RunOptions) -> Result<(), String> {
    let _guard = ScreenGuard::enter().map_err(|err| {
        format!(
            "não foi possível entrar no modo de tela do dashboard: {err} — \
             rode `kira dashboard --snapshot` para a versão de texto"
        )
    })?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))
        .map_err(|err| format!("não foi possível preparar a saída do dashboard: {err}"))?;

    // Restauro do terminal GARANTIDO até em pânico.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        default_hook(info);
    }));

    let palette = theme::palette_from_env();
    let mut snapshot = initial;
    // A execução vive FORA do `AppState` (canal + thread não são dados de
    // tela); o `Drop` do `Runner` mata o filho ao sair.
    let mut runner: Option<Runner> = None;
    let mut state_screen = AppState {
        findings_len: snapshot.finding_rows.len(),
        sessions_len: snapshot.session_rows.len(),
        wiki_len: snapshot.wiki_rows.len(),
        read_only: options.read_only,
        ..AppState::default()
    };

    loop {
        // Valores de filtro são os PRESENTES no snapshot; se o valor escolhido
        // sumiu (novo scan), o filtro cai — nunca se filtra por fantasma.
        state_screen.severity_values = state::severities_present(&snapshot.finding_rows);
        state_screen.tool_values = state::tools_present(&snapshot.finding_rows);
        if let Some(severity) = &state_screen.severity_filter
            && !state_screen.severity_values.contains(severity)
        {
            state_screen.severity_filter = None;
        }
        if let Some(tool) = &state_screen.tool_filter
            && !state_screen.tool_values.contains(tool)
        {
            state_screen.tool_filter = None;
        }
        let visible_findings = filtered_findings(&snapshot, &state_screen).len();
        state_screen.findings_len = visible_findings;
        state_screen.sessions_len = snapshot.session_rows.len();
        let filtered_wiki: Vec<&WikiPage> =
            state::filter_pages(&snapshot.wiki_rows, &state_screen.wiki_query);
        state_screen.wiki_len = filtered_wiki.len();
        // Ids que a folha de alvo pode oferecer, do snapshot (sem rede).
        state_screen.known_work_items = action::known_work_items(&snapshot);

        let size = terminal
            .size()
            .map_err(|err| format!("falha ao medir a tela: {err}"))?;
        let degraded = layout::is_degraded(Rect::new(0, 0, size.width, size.height));
        state_screen.degraded = degraded;

        // Clamp de TODOS os índices pela quantidade REAL de linhas visíveis.
        let degraded_total_lines = degraded_lines(&snapshot, state_screen.screen).len();
        state_screen.findings_selected = state_screen.findings_selected.min(max_selected_for(
            Screen::Achados,
            state_screen.findings_len,
            state_screen.sessions_len,
            state_screen.wiki_len,
            degraded_total_lines,
            degraded,
        ));
        state_screen.sessions_selected = state_screen.sessions_selected.min(max_selected_for(
            Screen::Sessoes,
            state_screen.findings_len,
            state_screen.sessions_len,
            state_screen.wiki_len,
            degraded_total_lines,
            degraded,
        ));
        state_screen.wiki_selected = state_screen.wiki_selected.min(max_selected_for(
            Screen::Wiki,
            state_screen.findings_len,
            state_screen.sessions_len,
            state_screen.wiki_len,
            degraded_total_lines,
            degraded,
        ));
        state_screen.degraded_selected = state_screen.degraded_selected.min(max_selected_for(
            Screen::Resumo,
            state_screen.findings_len,
            state_screen.sessions_len,
            state_screen.wiki_len,
            degraded_total_lines,
            degraded,
        ));

        // Um tick por volta do loop: o spinner anda, o resto da tela não muda.
        state_screen.tick = state_screen.tick.wrapping_add(1);

        // Drena a execução em andamento (nunca bloqueia: `try_recv`).
        drain_execution(&mut state_screen, &mut runner, &mut snapshot);

        // Ação pedida e já com id vira PLANO aqui (único I/O de ambiente);
        // `decide`/`apply_outcome` continuam puros e testáveis.
        if let Some(step) = state_screen.pending.take() {
            match build_plan(step, &snapshot) {
                Ok(plan) => {
                    state_screen.notice = None;
                    state_screen.confirm = Some(ConfirmState { plan });
                }
                Err(message) => state_screen.notice = Some(message),
            }
        }

        terminal
            .draw(|frame| render_frame(frame, &snapshot, &state_screen, palette))
            .map_err(|err| format!("falha ao desenhar o dashboard: {err}"))?;

        // Poll com timeout: sem tecla, nada acontece (sem spinner eterno).
        let has_event = event::poll(Duration::from_millis(150))
            .map_err(|err| format!("falha ao esperar teclado: {err}"))?;
        if !has_event {
            continue;
        }
        if let Event::Key(key) =
            event::read().map_err(|err| format!("falha ao ler teclado: {err}"))?
        {
            // O Windows entrega Press E Release; só Press executa.
            if key.kind == KeyEventKind::Press {
                match decide(&state_screen, key) {
                    Outcome::Quit => return Ok(()),
                    Outcome::Reload => {
                        // Reler sem perder a linha selecionada (fingerprint/id/slug).
                        let key = selection_key(&snapshot, &state_screen);
                        snapshot = state::load();
                        reanchor(&mut state_screen, key, &snapshot);
                    }
                    // `Enter` no popup é o ÚNICO caminho que spawna — e o
                    // plano já nasceu com argv allowlist.
                    Outcome::ConfirmExecute => {
                        if let Some(confirm) = state_screen.confirm.take() {
                            if state_screen
                                .exec
                                .as_ref()
                                .is_some_and(ExecState::is_running)
                            {
                                state_screen.notice = Some(
                                    "já existe uma execução em andamento — espere terminar"
                                        .to_string(),
                                );
                            } else {
                                let action = confirm.plan.action;
                                match Runner::spawn(confirm.plan) {
                                    Ok(spawned) => {
                                        let now = snapshot.now_secs.unwrap_or(0);
                                        state_screen.exec = Some(ExecState::new(action, now));
                                        state_screen.exec_open = true;
                                        state_screen.notice = None;
                                        runner = Some(spawned);
                                    }
                                    Err(err) => state_screen.notice = Some(err.to_string()),
                                }
                            }
                        }
                    }
                    outcome => apply_outcome(&mut state_screen, outcome),
                }
            }
        }
    }
}

/// Linhas da lista única do modo degradado (função pura, testável).
pub fn degraded_lines(snapshot: &Snapshot, screen: Screen) -> Vec<String> {
    let mut lines = vec![format!(
        "aviso: terminal menor que {0}×{1} — lista única; aumente a janela para o layout denso",
        layout::MIN_WIDTH,
        layout::MIN_HEIGHT
    )];
    lines.push(chrome::status_line(snapshot));
    lines.push(chrome::tabs_text(snapshot, screen.index()));
    match screen {
        Screen::Resumo => {
            for (title, section_lines) in state::resumo_sections(snapshot) {
                lines.push(format!("| {title} |"));
                lines.extend(section_lines);
            }
        }
        Screen::Achados => lines.extend(state::findings_lines(snapshot)),
        Screen::Sessoes => lines.extend(state::sessions_lines(snapshot)),
        Screen::Wiki => lines.extend(state::wiki_lines(snapshot)),
    }
    lines
}

/// Desenha UM frame completo: chrome (status + abas), fonte ativa e rodapé.
pub fn render_frame(frame: &mut Frame, snapshot: &Snapshot, screen: &AppState, palette: Palette) {
    // Modo de glifo lido UMA vez por frame (`KIRA_GLYPHS=ascii`): todos os
    // painéis desta pintura usam o mesmo conjunto — misturar Unicode com ASCII
    // na mesma tela é pior que ficar no conjunto degradado inteiro.
    render_frame_with(frame, snapshot, screen, palette, Glyphs::from_env());
}

/// [`render_frame`] com o conjunto de glifos injetado.
///
/// Existe para o teste poder afirmar o critério "ASCII sem glifo especial" na
/// TELA INTEIRA: variável de ambiente é estado global do processo e teste que
/// muta env é proibido aqui (edition 2024 torna `set_var` unsafe).
pub fn render_frame_with(
    frame: &mut Frame,
    snapshot: &Snapshot,
    screen: &AppState,
    palette: Palette,
    glyphs: Glyphs,
) {
    let area = frame.area();
    let shell = layout::shell(area);

    let mut status = chrome::status_line(snapshot);
    if screen.read_only {
        status.push_str("   ·   leitura");
    }
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            status,
            Style::default().fg(palette.footer),
        ))),
        shell.status,
    );
    frame.render_widget(
        Paragraph::new(chrome::tabs_line(snapshot, screen.screen.index(), palette)),
        shell.tabs,
    );

    let filtered_wiki: Vec<&WikiPage> =
        state::filter_pages(&snapshot.wiki_rows, &screen.wiki_query);
    // Ordem canônica por tipo (contexto → … → bugs): a MESMA que a seleção
    // percorre, para `j`/`k` andar na ordem em que a tela desenha.
    let ordered_wiki = state::wiki_ordered(&filtered_wiki);
    let inspecting = screen.inspecting && screen_has_rows(screen) && !screen.degraded;

    if screen.degraded {
        let lines = degraded_lines(snapshot, screen.screen);
        let block = crate::tui::widgets::panel::block_with("lista única", true, palette, glyphs);
        let scroll = u16::try_from(screen.degraded_selected.min(lines.len().saturating_sub(1)))
            .unwrap_or(u16::MAX);
        frame.render_widget(
            Paragraph::new(
                lines
                    .iter()
                    .map(|line| {
                        Line::from(Span::styled(
                            line.clone(),
                            Style::default().fg(palette.value),
                        ))
                    })
                    .collect::<Vec<_>>(),
            )
            .block(block)
            .scroll((scroll, 0)),
            shell.workspace,
        );
    } else {
        let ws = layout::workspace(shell.workspace, inspecting);
        render_screen(
            frame,
            snapshot,
            screen,
            palette,
            glyphs,
            ws.primary,
            ws.inspector,
            &ordered_wiki,
        );
    }

    // Rodapé: aviso (erro acionável) > busca aberta > chips de tecla.
    if let Some(notice) = &screen.notice {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!(" {notice}"),
                Style::default().fg(palette.failed),
            ))),
            shell.footer,
        );
    } else if screen.wiki_search_open {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!(
                    "busca wiki: {}▌   (Enter confirma · Esc volta · Backspace apaga)",
                    screen.wiki_query
                ),
                Style::default().fg(palette.help),
            ))),
            shell.footer,
        );
    } else {
        footer::render(
            frame,
            shell.footer,
            &footer::keys_for(
                screen.screen.index(),
                !screen.read_only,
                screen.exec.is_some(),
            ),
            palette,
        );
    }

    // Popups modais POR CIMA do chrome; `Clear` impede o texto de baixo de
    // vazar entre as linhas do popup (o statui usa o mesmo recurso).
    if let Some(sheet) = &screen.target {
        render_target(frame, area, sheet, palette, glyphs);
    }
    if let Some(confirm) = &screen.confirm {
        render_confirm(frame, area, confirm, palette, glyphs);
    }
    if screen.exec_open
        && let Some(exec) = &screen.exec
    {
        render_exec(
            frame,
            area,
            exec,
            snapshot.now_secs,
            screen.tick,
            palette,
            glyphs,
        );
    }

    // Leitura da wiki em tela cheia cobre tudo (modal): `Enter` abriu.
    if screen.page_open
        && let Some(page) = ordered_wiki.get(screen.wiki_selected)
    {
        frame.render_widget(
            WikiPageWidget::new(page, screen.preview_scroll, palette).with_glyphs(glyphs),
            area,
        );
    }
    if screen.help_open {
        // `Clear` antes: a ajuda é modal e cobre a tela inteira — sem limpar, o
        // painel de trás vaza nas células que o texto não pinta (defeito que só
        // aparecia de verdade em `KIRA_GLYPHS=ascii`, onde as barras usam `#`).
        frame.render_widget(Clear, area);
        frame.render_widget(help_paragraph(&palette, snapshot, glyphs), area);
    }
}

/// Pinta a fonte ativa (e o inspetor, quando aberto).
#[allow(clippy::too_many_arguments)]
fn render_screen(
    frame: &mut Frame,
    snapshot: &Snapshot,
    screen: &AppState,
    palette: Palette,
    glyphs: Glyphs,
    primary: Rect,
    inspector: Option<Rect>,
    filtered_wiki: &[&WikiPage],
) {
    match screen.screen {
        Screen::Resumo => {
            // Cartões (PR-UX2): o grid decide quantas colunas e quantas linhas
            // cada um leva; o conteúdo sai todo do MESMO snapshot do
            // `--snapshot` de texto, então número de tela e número de pipe não
            // divergem. Modo degradado (<80×24) continua na lista de texto.
            cards::render_grid(
                frame.buffer_mut(),
                primary,
                snapshot,
                Live {
                    exec: screen.exec.as_ref(),
                    tick: screen.tick,
                },
                true,
                palette,
                glyphs,
            );
        }
        Screen::Achados => {
            // A tabela desenha a lista FILTRADA (f/m), por referência.
            let filtered = filtered_findings(snapshot, screen);
            let filter_label = findings_filter_label(screen);
            let empty = if filter_label.is_some() {
                "— nenhum achado com o filtro atual (Esc limpa)".to_string()
            } else {
                state::empty_findings_line(snapshot)
            };
            frame.render_widget(
                FindingsWidget::new(&filtered, screen.findings_selected, true, empty, palette)
                    .with_filter(filter_label)
                    .with_glyphs(glyphs),
                primary,
            );
            if let Some(area) = inspector
                && let Some(row) = filtered.get(screen.findings_selected)
            {
                let detail = state::detail_lines(row);
                frame.render_widget(DetailWidget::new(&detail, palette), area);
            }
        }
        Screen::Sessoes => {
            frame.render_widget(
                SessionsWidget::new(
                    &snapshot.session_rows,
                    snapshot.now_secs,
                    screen.sessions_selected,
                    true,
                    state::empty_sessions_line(snapshot),
                    palette,
                )
                .with_glyphs(glyphs),
                primary,
            );
            if let Some(area) = inspector
                && let Some(row) = snapshot.session_rows.get(screen.sessions_selected)
            {
                let detail = state::session_detail_lines(row, snapshot.now_secs);
                frame.render_widget(DetailWidget::new(&detail, palette), area);
            }
        }
        Screen::Wiki => {
            frame.render_widget(
                WikiWidget::new(
                    filtered_wiki,
                    snapshot.now_secs,
                    screen.wiki_selected,
                    true,
                    state::empty_wiki_line(snapshot),
                    palette,
                )
                .with_glyphs(glyphs),
                primary,
            );
            if let Some(area) = inspector
                && let Some(page) = filtered_wiki.get(screen.wiki_selected)
            {
                frame.render_widget(
                    WikiPageWidget::new(page, screen.preview_scroll, palette).with_glyphs(glyphs),
                    area,
                );
            }
        }
    }
}

/// Overlay de ajuda: matriz de teclas + causas integrais das pendências.
fn help_paragraph(palette: &Palette, snapshot: &Snapshot, glyphs: Glyphs) -> Paragraph<'static> {
    let pendings = state::pending_causes(snapshot);
    // Setas são glifo: em modo ASCII a linha ensina o nome da tecla ("setas")
    // em vez de desenhar uma seta que o console não tem.
    let arrows = if glyphs.mode().is_ascii() {
        "setas"
    } else {
        "↑ / ↓"
    };
    let mut text = vec![
        "ajuda do kira dashboard (navegar não executa — ADR-0033)".to_string(),
        String::new(),
        "q                sair".to_string(),
        "Ctrl+C           sair (em qualquer estado)".to_string(),
        "Esc              voltar (fecha ajuda/leitura/inspetor/busca)".to_string(),
        "1–4 / Tab        trocar de tela (Resumo · Achados · Sessões · Wiki)".to_string(),
        format!("j / k ou {arrows}   mover a seleção da tela"),
        "g / G            topo / fim".to_string(),
        "Enter            abrir inspetor (achado/sessão) ou ler a página".to_string(),
        "i                abrir/fechar o inspetor 50–50".to_string(),
        "s / t / d        pedir scan / triage / rascunho de PR (Enter confirma)".to_string(),
        "o                abrir/fechar a saída da execução".to_string(),
        "/                busca incremental na wiki (só na tela Wiki)".to_string(),
        "PgUp/PgDn        rolar a leitura em tela cheia".to_string(),
        "r                reler o disco (sem rede, sem escrita)".to_string(),
        "?                abrir/fechar esta ajuda".to_string(),
    ];
    if pendings.is_empty() {
        text.push(String::new());
        text.push("pendências: — nenhuma pendência registrada".to_string());
    } else {
        text.push(String::new());
        text.push("pendências (causa integral; o chrome corta em uma linha):".to_string());
        text.extend(pendings.iter().cloned());
    }
    // A série de tendência sai do cartão em 80×24 (as linhas vão para barras e
    // índice), mas NÃO some: aqui ela aparece inteira, uma linha por scan. Dado
    // que some da tela é dado que o dono vai pedir depois.
    text.push(String::new());
    if snapshot.scan_series.len() >= 2 {
        // A ajuda é modal de altura fixa: 50 scans = 50 linhas, e a lista
        // empurraria o rodapé ("dado ausente aparece como —") para fora da
        // tela. Os mais NOVOS ficam; os antigos continuam no `--snapshot`.
        let total = snapshot.scan_series.len();
        let shown = total.min(HELP_TREND_LINES);
        text.push(format!(
            "tendência de achados ({total} scans, mais antigo primeiro):"
        ));
        if total > shown {
            text.push(format!(
                "  … {rest} scans mais antigos no --snapshot",
                rest = total - shown
            ));
        }
        for (offset, count) in snapshot.scan_series.iter().skip(total - shown).enumerate() {
            let number = total - shown + offset + 1;
            text.push(format!("  #{number:<3} {count} achados"));
        }
    } else {
        let note = snapshot
            .scan_series_note
            .clone()
            .unwrap_or_else(|| "nenhum scan na trilha — rode `kira scan`".to_string());
        text.push(format!("tendência: {note}"));
    }
    text.push(String::new());
    text.push("Dado ausente aparece como — com a causa; nada é inventado.".to_string());
    Paragraph::new(
        text.into_iter()
            .map(|line| Line::from(Span::styled(line, Style::default().fg(palette.help))))
            .collect::<Vec<_>>(),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_set(glyphs.border_set())
            .title(" ajuda ")
            .border_style(Style::default().fg(palette.help)),
    )
    .wrap(Wrap { trim: false })
}

/// Popup de confirmação: alvo, efeitos e pré-requisitos com cor por estado.
/// Nada aqui executa — o widget só mostra o que o plano declarou.
fn render_confirm(
    frame: &mut Frame,
    area: Rect,
    confirm: &ConfirmState,
    palette: Palette,
    glyphs: Glyphs,
) {
    use action::confirmacao::Kind;

    let popup = layout::centered_rect(70, 62, area);
    frame.render_widget(Clear, popup);
    let mut text: Vec<Line> = action::confirmacao::lines(&confirm.plan)
        .into_iter()
        .map(|line| {
            let color = match line.kind {
                Kind::Target => palette.value,
                Kind::Effect => palette.missing,
                Kind::Ok => palette.ok,
                Kind::Fail => palette.failed,
            };
            Line::from(Span::styled(line.text, Style::default().fg(color)))
        })
        .collect();
    text.push(Line::from(""));
    text.push(Line::from(vec![
        Span::styled(
            format!("Enter {}", confirm.plan.action.verb()),
            Style::default()
                .fg(palette.title)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ),
        Span::raw("    "),
        Span::styled("Esc cancelar", Style::default().fg(palette.help)),
    ]));
    text.push(Line::from(Span::styled(
        "nada foi executado ainda",
        Style::default().fg(palette.missing),
    )));
    frame.render_widget(
        Paragraph::new(text)
            .block(crate::tui::widgets::panel::block_with(
                &confirm.plan.action.title(),
                true,
                palette,
                glyphs,
            ))
            .wrap(Wrap { trim: true }),
        popup,
    );
}

/// Folha de alvo: lista de ids conhecidos + campo digitado.
fn render_target(
    frame: &mut Frame,
    area: Rect,
    sheet: &TargetSheet,
    palette: Palette,
    glyphs: Glyphs,
) {
    let popup = layout::centered_rect(56, 62, area);
    frame.render_widget(Clear, popup);
    // Marcadores do MESMO conjunto da sessão: a cor acompanha o glifo, então
    // em ASCII a seleção continua destacada (`>` em vez de `➤`).
    let selection = glyphs.selection().trim_end();
    let field = glyphs.field_open();
    let mut text: Vec<Line> = sheet
        .lines_with(glyphs)
        .into_iter()
        .map(|line| {
            let color = if line.starts_with(selection) {
                palette.value
            } else if line.starts_with(field) {
                palette.title
            } else {
                palette.help
            };
            Line::from(Span::styled(line, Style::default().fg(color)))
        })
        .collect();
    text.push(Line::from(""));
    text.push(Line::from(Span::styled(
        "Enter continuar   Tab completar   Esc cancelar",
        Style::default().fg(palette.help),
    )));
    frame.render_widget(
        Paragraph::new(text).block(crate::tui::widgets::panel::block_with(
            sheet.title(),
            true,
            palette,
            glyphs,
        )),
        popup,
    );
}

/// Overlay `o`: a saída REAL do subprocesso, já sanitizada e redigida.
///
/// Gramática igual ao resto da casca: painel com título, cabeçalho de stream
/// (`═| stdout |══`), linhas do processo e a dica de teclas no fim.
fn render_exec(
    frame: &mut Frame,
    area: Rect,
    exec: &ExecState,
    now_secs: Option<i64>,
    tick: u64,
    palette: Palette,
    glyphs: Glyphs,
) {
    frame.render_widget(Clear, area);
    let status = exec_status_label(exec, now_secs, tick, glyphs);
    let block = crate::tui::widgets::panel::block_with(
        &format!("{} · {status}", exec.action.title()),
        true,
        palette,
        glyphs,
    );
    let inner = block.inner(area);

    let mut body: Vec<Line> = Vec::new();
    if exec.dropped > 0 {
        body.push(Line::from(Span::styled(
            format!(" ({} linhas antigas descartadas pelo teto)", exec.dropped),
            Style::default().fg(palette.missing),
        )));
    }
    let mut current: Option<bool> = None; // Some(true) = stderr
    for line in &exec.lines {
        match line {
            action::LogLine::Stdout(text) => {
                if current != Some(false) {
                    body.push(exec_header("stdout", inner.width, palette, glyphs));
                    current = Some(false);
                }
                body.push(Line::from(Span::styled(
                    format!(" {text}"),
                    Style::default().fg(palette.value),
                )));
            }
            action::LogLine::Stderr(text) => {
                if current != Some(true) {
                    body.push(exec_header("stderr", inner.width, palette, glyphs));
                    current = Some(true);
                }
                body.push(Line::from(Span::styled(
                    format!(" {text}"),
                    Style::default().fg(palette.failed),
                )));
            }
            action::LogLine::Note(text) => body.push(Line::from(Span::styled(
                format!(" {text}"),
                Style::default().fg(palette.missing),
            ))),
        }
    }
    if body.is_empty() {
        body.push(Line::from(Span::styled(
            " aguardando a primeira linha…",
            Style::default().fg(palette.missing),
        )));
    }

    let height = usize::from(inner.height);
    let scroll = if exec.follow {
        body.len().saturating_sub(height)
    } else {
        exec.scroll.min(body.len().saturating_sub(1))
    };
    body.push(Line::from(Span::styled(
        " j/k rolar · G seguir · Esc fechar (o processo continua)",
        Style::default().fg(palette.help),
    )));
    let scroll = u16::try_from(scroll).unwrap_or(u16::MAX);
    frame.render_widget(Paragraph::new(body).block(block).scroll((scroll, 0)), area);
}

/// `═| stdout |══…` na largura real do painel (mesma régua do inspetor).
///
/// O traço vem do conjunto de glifos: em `KIRA_GLYPHS=ascii` vira `-| stdout |-`.
fn exec_header(label: &str, width: u16, palette: Palette, glyphs: Glyphs) -> Line<'static> {
    let bar = glyphs.border_set().horizontal_top;
    let left = format!("{bar}| {label} |");
    let fill = usize::from(width).saturating_sub(left.chars().count() + 1);
    Line::from(Span::styled(
        format!("{left}{}", bar.repeat(fill)),
        Style::default().fg(palette.rule),
    ))
}

/// Rótulo do estado: `executando há 4s` / `concluído (exit 0)`.
fn exec_status_label(exec: &ExecState, now_secs: Option<i64>, tick: u64, glyphs: Glyphs) -> String {
    match exec.status {
        action::ExecStatus::Running => {
            // Spinner só aqui: o estado é Running porque o filho está vivo.
            let spinner = crate::tui::spinner::frame(tick, glyphs);
            let label = match now_secs {
                Some(now) => format!(
                    "executando há {}",
                    state::format_duration_ms((now - exec.started_unix).max(0) * 1_000)
                ),
                None => "executando".to_string(),
            };
            format!("{label} {spinner}")
        }
        action::ExecStatus::Exited { code } => match code {
            Some(0) => "concluído (exit 0)".to_string(),
            Some(code) => format!("falhou (exit {code})"),
            None => "encerrado sem código".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::Finding;
    use crate::store::Project;
    use crate::tui::state::rows_from;
    use ratatui::backend::TestBackend;

    fn plain(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn findings_state() -> AppState {
        AppState {
            screen: Screen::Achados,
            findings_len: 3,
            ..AppState::default()
        }
    }

    fn snapshot_with_findings() -> Snapshot {
        let findings = vec![
            Finding {
                tool: "audit".to_string(),
                rule: "vulnerable".to_string(),
                package: "minimist".to_string(),
                version: "1.2.5".to_string(),
                severity: "critical".to_string(),
                path: "node_modules".to_string(),
                message: "Prototype pollution".to_string(),
                fingerprint: "0123456789abcdef".to_string(),
            },
            Finding {
                tool: "dotnet".to_string(),
                rule: "vulnerable".to_string(),
                package: "Newtonsoft.Json".to_string(),
                version: "12.0.3".to_string(),
                severity: "high".to_string(),
                path: "Kira.Backend".to_string(),
                message: "Advisory".to_string(),
                fingerprint: "fedcba9876543210".to_string(),
            },
        ];
        Snapshot {
            project: Some(Project {
                id: "abc".to_string(),
                root_path: "C:\\proj".to_string(),
                name: "proj".to_string(),
                first_seen_at: "2026-11-11T09:00:00Z".to_string(),
            }),
            dev_name: Some("Sérgio".to_string()),
            pat_masked: Some("Aa2x****".to_string()),
            findings: Some(2),
            finding_rows: rows_from(findings),
            ..Snapshot::default()
        }
    }

    /// Plano de teste: sem pré-requisitos resolvidos (o popup é o alvo aqui).
    fn sample_plan(step: StepAction) -> action::ActionPlan {
        action::ActionPlan {
            action: step,
            program: std::path::PathBuf::from("C:/bin/kira.exe"),
            argv: step.argv(std::path::Path::new("C:/proj")),
            root: std::path::PathBuf::from("C:/proj"),
            effects: step.effects(),
            prerequisites: Vec::new(),
        }
    }

    fn render_text(screen: &AppState, snapshot: &Snapshot, width: u16, height: u16) -> String {
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

    #[test]
    fn digits_go_straight_to_their_screen() {
        let state = AppState::default();
        assert_eq!(
            decide(&state, plain(KeyCode::Char('2'))),
            Outcome::GoScreen(Screen::Achados)
        );
        assert_eq!(
            decide(&state, plain(KeyCode::Char('3'))),
            Outcome::GoScreen(Screen::Sessoes)
        );
        assert_eq!(
            decide(&state, plain(KeyCode::Char('4'))),
            Outcome::GoScreen(Screen::Wiki)
        );
        assert_eq!(
            decide(&state, plain(KeyCode::Char('1'))),
            Outcome::GoScreen(Screen::Resumo)
        );
    }

    #[test]
    fn tab_cycles_the_four_screens_with_wrap() {
        assert_eq!(next_screen(Screen::Resumo, 1), Screen::Achados);
        assert_eq!(next_screen(Screen::Wiki, 1), Screen::Resumo);
        assert_eq!(next_screen(Screen::Resumo, -1), Screen::Wiki);
        let mut state = AppState::default();
        apply_outcome(&mut state, Outcome::CycleScreen(1));
        assert_eq!(state.screen, Screen::Achados);
        assert!(!state.inspecting, "trocar de tela fecha o inspetor");
    }

    #[test]
    fn move_is_scoped_per_screen() {
        let mut state = AppState {
            findings_len: 5,
            sessions_len: 5,
            wiki_len: 5,
            ..AppState::default()
        };
        state.screen = Screen::Achados;
        apply_outcome(&mut state, Outcome::Move(2));
        assert_eq!(state.findings_selected, 2);
        assert_eq!(state.sessions_selected, 0);
        assert_eq!(state.wiki_selected, 0);
        state.screen = Screen::Sessoes;
        apply_outcome(&mut state, Outcome::Move(1));
        assert_eq!(state.sessions_selected, 1);
        assert_eq!(state.findings_selected, 2, "não muda a outra tela");
    }

    #[test]
    fn top_and_bottom_saturate() {
        assert_eq!(apply_move(3, isize::MIN), 0);
        assert_eq!(apply_move(3, isize::MAX), usize::MAX);
        let mut state = findings_state();
        apply_outcome(&mut state, Outcome::Move(isize::MIN));
        assert_eq!(state.findings_selected, 0);
    }

    #[test]
    fn enter_opens_the_inspector_only_when_there_is_a_record() {
        let empty = AppState {
            screen: Screen::Achados,
            findings_len: 0,
            ..AppState::default()
        };
        assert_eq!(decide(&empty, plain(KeyCode::Enter)), Outcome::Ignore);
        let state = findings_state();
        assert_eq!(
            decide(&state, plain(KeyCode::Enter)),
            Outcome::ToggleInspector
        );
        let resumo = AppState::default();
        assert_eq!(decide(&resumo, plain(KeyCode::Enter)), Outcome::Ignore);
    }

    #[test]
    fn inspect_key_toggles_and_esc_closes_it_first() {
        let state = findings_state();
        assert_eq!(
            decide(&state, plain(KeyCode::Char('i'))),
            Outcome::ToggleInspector
        );
        let mut open = findings_state();
        open.inspecting = true;
        assert_eq!(decide(&open, plain(KeyCode::Esc)), Outcome::ToggleInspector);
        apply_outcome(&mut open, Outcome::ToggleInspector);
        assert!(!open.inspecting);
    }

    #[test]
    fn navigation_never_returns_an_execution_outcome() {
        // Contrato do ADR-0033: nenhuma tecla de navegação vira ação.
        let state = findings_state();
        let keys = [
            KeyCode::Tab,
            KeyCode::BackTab,
            KeyCode::Char('j'),
            KeyCode::Char('k'),
            KeyCode::Char('g'),
            KeyCode::Char('G'),
            KeyCode::Enter,
            KeyCode::Char('i'),
            KeyCode::Char('r'),
            KeyCode::Char('?'),
            KeyCode::Esc,
        ];
        for code in keys {
            let outcome = decide(&state, plain(code));
            assert!(
                matches!(
                    outcome,
                    Outcome::Reload
                        | Outcome::ToggleInspector
                        | Outcome::GoScreen(_)
                        | Outcome::CycleScreen(_)
                        | Outcome::Move(_)
                        | Outcome::SearchWiki
                        | Outcome::SearchChar(_)
                        | Outcome::SearchBackspace
                        | Outcome::CloseSearch
                        | Outcome::OpenPage
                        | Outcome::ClosePage
                        | Outcome::ScrollPage(_)
                        | Outcome::Help
                        | Outcome::Redraw
                        | Outcome::Ignore
                ),
                "navegação produziu efeito inesperado: {code:?} -> {outcome:?}"
            );
        }
    }

    #[test]
    fn search_is_wiki_only_and_modal() {
        let achados = findings_state();
        assert_eq!(decide(&achados, plain(KeyCode::Char('/'))), Outcome::Ignore);

        let mut wiki = AppState {
            screen: Screen::Wiki,
            wiki_len: 2,
            ..AppState::default()
        };
        assert_eq!(
            decide(&wiki, plain(KeyCode::Char('/'))),
            Outcome::SearchWiki
        );
        apply_outcome(&mut wiki, Outcome::SearchWiki);
        assert!(wiki.wiki_search_open);
        assert_eq!(
            decide(&wiki, plain(KeyCode::Char('b'))),
            Outcome::SearchChar('b')
        );
        apply_outcome(&mut wiki, Outcome::SearchChar('b'));
        assert_eq!(wiki.wiki_query, "b");
        apply_outcome(&mut wiki, Outcome::SearchBackspace);
        assert!(wiki.wiki_query.is_empty());
        // `q` é texto durante a busca, não saída.
        assert_eq!(
            decide(&wiki, plain(KeyCode::Char('q'))),
            Outcome::SearchChar('q')
        );
        assert_eq!(decide(&wiki, plain(KeyCode::Enter)), Outcome::CloseSearch);
        assert_eq!(decide(&wiki, plain(KeyCode::Esc)), Outcome::CloseSearch);
    }

    #[test]
    fn page_reading_is_modal_and_scrolls() {
        let mut state = AppState {
            screen: Screen::Wiki,
            wiki_len: 1,
            page_open: true,
            ..AppState::default()
        };
        assert_eq!(
            decide(&state, plain(KeyCode::Char('j'))),
            Outcome::ScrollPage(1)
        );
        assert_eq!(
            decide(&state, plain(KeyCode::PageDown)),
            Outcome::ScrollPage(SCROLL_PAGE_STEP)
        );
        assert_eq!(decide(&state, plain(KeyCode::Enter)), Outcome::ClosePage);
        apply_outcome(&mut state, Outcome::ScrollPage(3));
        assert_eq!(state.preview_scroll, 3);
        apply_outcome(&mut state, Outcome::ClosePage);
        assert!(!state.page_open);
    }

    #[test]
    fn ctrl_c_quits_everywhere_and_help_is_modal() {
        let state = AppState {
            help_open: true,
            ..AppState::default()
        };
        let ctrl_c = KeyEvent {
            modifiers: KeyModifiers::CONTROL,
            ..KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE)
        };
        assert_eq!(decide(&state, ctrl_c), Outcome::Quit);
        assert_eq!(decide(&state, plain(KeyCode::Char('?'))), Outcome::Help);
        // Por baixo da ajuda não se navega.
        assert_eq!(decide(&state, plain(KeyCode::Char('j'))), Outcome::Ignore);
    }

    #[test]
    fn full_frame_shows_status_tabs_screen_and_footer_chips() {
        let snapshot = snapshot_with_findings();
        let state = AppState {
            findings_len: snapshot.finding_rows.len(),
            ..AppState::default()
        };
        let text = render_text(&state, &snapshot, 80, 24);
        assert!(text.contains("pat Aa2x****"), "{text}");
        assert!(text.contains("[1 Resumo]"), "aba ativa marcada: {text}");
        // PR-UX2: o Resumo é um grid de cartões, não um painel de seções.
        for card in ["| Risco", "| Sessões |", "| Projeto |", "| Pendências |"] {
            assert!(text.contains(card), "cartão {card}: {text}");
        }
        assert!(text.contains("critical"), "barras de severidade: {text}");
        assert!(text.contains("Sair"), "rodapé com chips: {text}");
        // Sem `--read-only`, o rodapé oferece as teclas de ação.
        assert!(text.contains(" Scan"), "{text}");
        assert!(text.contains("Triage"), "{text}");
    }

    /// A série de tendência saiu do cartão em 80×24, então a ajuda é o lugar
    /// dela — e ela tem teto: 30 scans não podem empurrar o rodapé da ajuda
    /// para fora da tela. Os antigos ficam no `--snapshot`.
    #[test]
    fn help_lists_the_trend_series_and_caps_it() {
        let mut snapshot = snapshot_with_findings();
        snapshot.scan_series = (1..=30).collect();
        let state = AppState {
            help_open: true,
            ..AppState::default()
        };
        let text = render_text(&state, &snapshot, 120, 40);
        // A ajuda quebra linha ao desenhar; a asserção é sobre as PALAVRAS.
        let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(flat.contains("30 scans"), "{text}");
        assert!(
            flat.contains("20 scans mais antigos no --snapshot"),
            "{text}"
        );
        assert!(flat.contains("#30"), "o mais novo aparece: {text}");
        assert!(!flat.contains("#1 "), "o mais antigo sai da tela: {text}");
        assert!(
            flat.contains("Dado ausente"),
            "rodapé da ajuda preservado: {text}"
        );
    }

    #[test]
    fn achados_screen_uses_the_whole_workspace_and_inspector_splits() {
        let snapshot = snapshot_with_findings();
        let mut state = AppState {
            screen: Screen::Achados,
            findings_len: snapshot.finding_rows.len(),
            ..AppState::default()
        };
        let full = render_text(&state, &snapshot, 120, 40);
        assert!(full.contains("| Achados — critical 1"), "{full}");
        assert!(full.contains("Prototype pollution"), "{full}");
        state.inspecting = true;
        let split = render_text(&state, &snapshot, 120, 40);
        assert!(
            split.contains("fingerprint"),
            "inspetor com o registro: {split}"
        );
    }

    #[test]
    fn degraded_terminal_renders_a_single_list_with_a_warning() {
        let snapshot = snapshot_with_findings();
        let mut state = AppState {
            findings_len: snapshot.finding_rows.len(),
            degraded: true,
            ..AppState::default()
        };
        state.screen = Screen::Achados;
        let text = render_text(&state, &snapshot, 60, 18);
        assert!(text.contains("lista única"), "{text}");
        assert!(text.contains("aviso: terminal menor que"), "{text}");
        assert!(text.contains("critical"), "{text}");
    }

    #[test]
    fn read_only_mode_is_stated_in_the_chrome() {
        let snapshot = snapshot_with_findings();
        let state = AppState {
            read_only: true,
            ..AppState::default()
        };
        let text = render_text(&state, &snapshot, 100, 25);
        assert!(text.contains("leitura"), "{text}");
        assert!(
            !text.contains(" Scan"),
            "read-only não oferece tecla de ação no rodapé: {text}"
        );
    }

    #[test]
    fn help_overlay_lists_the_new_keys_and_pendencies() {
        let snapshot = snapshot_with_findings();
        let state = AppState {
            help_open: true,
            ..AppState::default()
        };
        let text = render_text(&state, &snapshot, 100, 30);
        assert!(text.contains("1–4 / Tab"), "{text}");
        assert!(text.contains("inspetor"), "{text}");
        assert!(text.contains("nada é inventado"), "{text}");
    }

    #[test]
    fn severity_filter_cycles_present_values_and_wraps_back_to_all() {
        let mut state = AppState {
            screen: Screen::Achados,
            findings_len: 2,
            severity_values: vec!["critical".to_string(), "high".to_string()],
            ..AppState::default()
        };
        apply_outcome(&mut state, Outcome::CycleSeverityFilter);
        assert_eq!(state.severity_filter.as_deref(), Some("critical"));
        assert_eq!(
            findings_filter_label(&state).as_deref(),
            Some("critical"),
            "o título do bloco mostra o filtro"
        );
        apply_outcome(&mut state, Outcome::CycleSeverityFilter);
        assert_eq!(state.severity_filter.as_deref(), Some("high"));
        apply_outcome(&mut state, Outcome::CycleSeverityFilter);
        assert_eq!(state.severity_filter, None, "fim do ciclo = todas");
    }

    #[test]
    fn filters_only_exist_on_achados_and_esc_clears_them() {
        let sessoes = AppState {
            screen: Screen::Sessoes,
            sessions_len: 1,
            ..AppState::default()
        };
        assert_eq!(decide(&sessoes, plain(KeyCode::Char('f'))), Outcome::Ignore);
        assert_eq!(decide(&sessoes, plain(KeyCode::Char('m'))), Outcome::Ignore);

        let filtered = AppState {
            screen: Screen::Achados,
            findings_len: 2,
            severity_filter: Some("high".to_string()),
            tool_filter: Some("audit".to_string()),
            ..AppState::default()
        };
        assert_eq!(
            decide(&filters_state(), plain(KeyCode::Char('f'))),
            Outcome::CycleSeverityFilter
        );
        assert_eq!(
            decide(&filtered, plain(KeyCode::Esc)),
            Outcome::ClearFindingsFilters
        );
        let mut cleared = filtered;
        apply_outcome(&mut cleared, Outcome::ClearFindingsFilters);
        assert!(!filters_active(&cleared));
        assert_eq!(cleared.findings_selected, 0);
    }

    fn filters_state() -> AppState {
        AppState {
            screen: Screen::Achados,
            findings_len: 2,
            ..AppState::default()
        }
    }

    #[test]
    fn filtered_findings_and_label_are_pure_over_the_snapshot() {
        let snapshot = snapshot_with_findings();
        let mut state = AppState {
            screen: Screen::Achados,
            severity_filter: Some("high".to_string()),
            ..AppState::default()
        };
        let visible = filtered_findings(&snapshot, &state);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].severity, "high");
        state.tool_filter = Some("audit".to_string());
        assert!(
            filtered_findings(&snapshot, &state).is_empty(),
            "high + audit não casa com nenhum achado do fixture"
        );
        assert_eq!(
            findings_filter_label(&state).as_deref(),
            Some("high · audit")
        );
    }

    #[test]
    fn resumo_sections_answer_risk_activity_and_gaps() {
        let snapshot = snapshot_with_findings();
        let sections = state::resumo_sections(&snapshot);
        let titles: Vec<&str> = sections.iter().map(|(title, _)| title.as_str()).collect();
        assert_eq!(
            titles,
            vec!["Achados", "Primeiros achados", "Pendências"],
            "sem sessões, a seção de atividade é omitida"
        );
        assert!(
            sections[0].1[0].contains("critical 1 · high 1"),
            "{:?}",
            sections[0].1
        );
        assert!(
            sections
                .last()
                .is_some_and(|(_, lines)| lines.iter().any(|line| line.contains('—'))),
            "pendência honesta, nunca inventada"
        );
    }

    #[test]
    fn read_only_ignores_action_keys_and_keeps_navigation() {
        let state = AppState {
            read_only: true,
            ..AppState::default()
        };
        for key in ['s', 't', 'd'] {
            assert_eq!(
                decide(&state, plain(KeyCode::Char(key))),
                Outcome::Ignore,
                "read-only precisa barrar {key} antes de qualquer modal"
            );
        }
        // Navegação continua exatamente igual.
        assert_eq!(decide(&state, plain(KeyCode::Tab)), Outcome::CycleScreen(1));
        assert_eq!(decide(&state, plain(KeyCode::Char('?'))), Outcome::Help);

        // Sem `--read-only`, a tecla vira PEDIDO — nunca execução.
        let acting = AppState::default();
        assert_eq!(
            decide(&acting, plain(KeyCode::Char('s'))),
            Outcome::RequestAction(Request::Scan)
        );
    }

    #[test]
    fn scan_goes_straight_to_a_plan_while_triage_asks_for_the_item() {
        let mut state = AppState {
            known_work_items: vec![4821, 4790],
            ..AppState::default()
        };
        apply_outcome(&mut state, Outcome::RequestAction(Request::Scan));
        assert!(
            state.pending.is_some(),
            "s já tem alvo: vai direto ao plano"
        );
        assert!(state.target.is_none());

        apply_outcome(&mut state, Outcome::RequestAction(Request::Triage));
        let sheet = state
            .target
            .as_ref()
            .unwrap_or_else(|| panic!("t abre a folha de alvo"));
        assert_eq!(sheet.selected(), Some(4821), "o mais recente vem primeiro");
        apply_outcome(&mut state, Outcome::TargetConfirm);
        assert!(state.target.is_none());
        assert_eq!(
            state.pending,
            Some(StepAction::Triage { work_item: 4821 }),
            "confirmar o id não executa: só prepara o plano"
        );
    }

    #[test]
    fn target_sheet_refuses_zero_and_esc_closes_it_before_everything() {
        let mut state = AppState::default();
        apply_outcome(&mut state, Outcome::RequestAction(Request::PrDraft));
        assert!(state.target.is_some());
        apply_outcome(&mut state, Outcome::TargetPush('0'));
        apply_outcome(&mut state, Outcome::TargetConfirm);
        assert!(state.target.is_some(), "id zero mantém a folha aberta");
        assert!(
            state
                .notice
                .as_deref()
                .unwrap_or_default()
                .contains("inválido"),
            "{:?}",
            state.notice
        );
        assert_eq!(decide(&state, plain(KeyCode::Esc)), Outcome::TargetCancel);
        apply_outcome(&mut state, Outcome::TargetCancel);
        assert!(state.target.is_none());
    }

    #[test]
    fn confirmation_popup_lists_effects_and_navigation_cannot_execute() {
        let snapshot = snapshot_with_findings();
        let state = AppState {
            confirm: Some(ConfirmState {
                plan: sample_plan(StepAction::Scan),
            }),
            ..AppState::default()
        };
        let text = render_text(&state, &snapshot, 100, 30);
        assert!(text.contains("executar scan"), "{text}");
        assert!(text.contains(".kira/findings.json"), "{text}");
        assert!(text.contains("nada foi executado ainda"), "{text}");

        // Enter é o único caminho; Esc cancela; o resto não navega por baixo.
        assert_eq!(
            decide(&state, plain(KeyCode::Enter)),
            Outcome::ConfirmExecute
        );
        assert_eq!(decide(&state, plain(KeyCode::Esc)), Outcome::ConfirmCancel);
        assert_eq!(decide(&state, plain(KeyCode::Char('j'))), Outcome::Ignore);
        assert_eq!(decide(&state, plain(KeyCode::Tab)), Outcome::Ignore);
    }

    #[test]
    fn target_sheet_popup_shows_the_typed_id_and_the_known_list() {
        let snapshot = snapshot_with_findings();
        let mut state = AppState::default();
        apply_outcome(&mut state, Outcome::RequestAction(Request::Triage));
        apply_outcome(&mut state, Outcome::TargetPush('4'));
        let text = render_text(&state, &snapshot, 100, 30);
        assert!(text.contains("alvo do triage"), "{text}");
        assert!(
            text.contains("4821") || text.contains("work item"),
            "{text}"
        );
        assert!(text.contains("▌"), "campo com cursor: {text}");
    }

    #[test]
    fn reload_keeps_the_selected_finding_by_fingerprint() {
        let before = snapshot_with_findings();
        let mut state = AppState {
            screen: Screen::Achados,
            findings_len: before.finding_rows.len(),
            findings_selected: 1, // Newtonsoft (high)
            ..AppState::default()
        };
        let key = selection_key(&before, &state);
        assert_eq!(
            key,
            Some(state::SelectionKey::Finding("fedcba9876543210".to_string()))
        );

        // Depois: um critical NOVO entra no topo — a seleção não pode "andar"
        // para outro achado só porque o índice 1 mudou de conteúdo.
        let after = Snapshot {
            finding_rows: rows_from(vec![
                Finding {
                    tool: "audit".to_string(),
                    rule: "vulnerable".to_string(),
                    package: "novo".to_string(),
                    version: "1.0.0".to_string(),
                    severity: "critical".to_string(),
                    path: "node_modules".to_string(),
                    message: "novo achado".to_string(),
                    fingerprint: "aaaabbbbccccdddd".to_string(),
                },
                Finding {
                    tool: "audit".to_string(),
                    rule: "vulnerable".to_string(),
                    package: "minimist".to_string(),
                    version: "1.2.5".to_string(),
                    severity: "critical".to_string(),
                    path: "node_modules".to_string(),
                    message: "Prototype pollution".to_string(),
                    fingerprint: "0123456789abcdef".to_string(),
                },
                Finding {
                    tool: "dotnet".to_string(),
                    rule: "vulnerable".to_string(),
                    package: "Newtonsoft.Json".to_string(),
                    version: "12.0.3".to_string(),
                    severity: "high".to_string(),
                    path: "Kira.Backend".to_string(),
                    message: "Advisory".to_string(),
                    fingerprint: "fedcba9876543210".to_string(),
                },
            ]),
            ..before.clone()
        };
        reanchor(&mut state, key, &after);
        assert_eq!(
            state.findings_selected, 2,
            "segue o Newtonsoft, não o índice"
        );
        assert_eq!(
            after.finding_rows[state.findings_selected].fingerprint,
            "fedcba9876543210"
        );
    }

    #[test]
    fn exec_overlay_shows_streams_status_and_keys() {
        let snapshot = snapshot_with_findings();
        let mut exec = ExecState::new(StepAction::Scan, 0);
        exec.apply(RunEvent::Line {
            stream: action::Stream::Out,
            text: "linha do scan".to_string(),
        });
        exec.status = action::ExecStatus::Exited { code: Some(0) };
        let state = AppState {
            exec: Some(exec),
            exec_open: true,
            ..AppState::default()
        };
        let text = render_text(&state, &snapshot, 100, 30);
        assert!(text.contains("executar scan"), "{text}");
        assert!(text.contains("stdout"), "{text}");
        assert!(text.contains("linha do scan"), "{text}");
        assert!(text.contains("concluído (exit 0)"), "{text}");
        assert!(text.contains("j/k rolar"), "{text}");
        // Esc fecha o overlay sem matar nada (só o `q` derruba o filho).
        assert_eq!(
            decide(&state, plain(KeyCode::Esc)),
            Outcome::ToggleExecution
        );
        assert_eq!(
            decide(&state, plain(KeyCode::Char('G'))),
            Outcome::FollowExecution
        );
    }

    #[test]
    fn drain_without_a_runner_is_a_no_op() {
        let mut state = AppState::default();
        let mut runner = None;
        let mut snapshot = snapshot_with_findings();
        drain_execution(&mut state, &mut runner, &mut snapshot);
        assert!(runner.is_none());
        assert!(state.exec.is_none());
    }

    /// Critério 4 do PR-UX1 na TELA INTEIRA: com `KIRA_GLYPHS=ascii`, nenhum
    /// painel, popup ou overlay pode emitir box-drawing, bloco ou seta.
    ///
    /// Roda pelo `render_frame_with` porque o modo de glifo é injetado — teste
    /// que muta env é proibido aqui (edition 2024 torna `set_var` unsafe).
    #[test]
    fn ascii_mode_renders_the_whole_frame_without_special_glyph() {
        use crate::tui::glyph::{Glyphs, Mode, has_special_glyphs};

        let snapshot = snapshot_with_findings();
        let glyphs = Glyphs::new(Mode::Ascii);
        let render = |state: &AppState, width: u16, height: u16| -> String {
            let backend = ratatui::backend::TestBackend::new(width, height);
            let mut terminal =
                ratatui::Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
            terminal
                .draw(|frame| render_frame_with(frame, &snapshot, state, theme::mono(), glyphs))
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
        };

        // As quatro telas.
        for screen in [
            Screen::Resumo,
            Screen::Achados,
            Screen::Sessoes,
            Screen::Wiki,
        ] {
            let state = AppState {
                screen,
                findings_len: snapshot.finding_rows.len(),
                ..AppState::default()
            };
            let text = render(&state, 80, 24);
            assert!(!has_special_glyphs(&text), "{screen:?}: {text}");
        }
        // Overlay de ajuda.
        let help = AppState {
            help_open: true,
            findings_len: snapshot.finding_rows.len(),
            ..AppState::default()
        };
        let text = render(&help, 80, 24);
        assert!(!has_special_glyphs(&text), "ajuda: {text}");
        // Folha de alvo (popup com campo e lista de ids).
        let target = AppState {
            target: Some(TargetSheet::new(Request::Triage, vec![4821, 900])),
            ..AppState::default()
        };
        let text = render(&target, 80, 24);
        assert!(!has_special_glyphs(&text), "folha de alvo: {text}");
        // Overlay `o` (saída da execução, com cabeçalho de stream).
        let mut exec = ExecState::new(StepAction::Scan, 0);
        exec.apply(RunEvent::Line {
            stream: action::Stream::Out,
            text: "linha do scan".to_string(),
        });
        let exec_state = AppState {
            exec: Some(exec),
            exec_open: true,
            ..AppState::default()
        };
        let text = render(&exec_state, 80, 24);
        assert!(!has_special_glyphs(&text), "overlay o: {text}");
    }
}
