//! Camada de AÇÃO do dashboard (ADR-0033) — a ÚNICA porta pela qual a TUI
//! pode executar o próprio binário.
//!
//! Regras que este módulo materializa:
//! - a tecla pede uma ação (`Request`), não um argv: `t`/`d` ainda não sabem o
//!   work item, então a folha de alvo (`target`) resolve o id antes;
//! - o argv é allowlist por CONSTRUÇÃO (`Action::argv`), nunca montado por
//!   texto livre e nunca passando por shell;
//! - o programa é sempre `current_exe()` (ou o injetado em teste), jamais
//!   `cmd`/`sh`;
//! - todo efeito (disco, trilha, rede, LLM) é declarado antes de rodar e
//!   aparece no popup de confirmação.
//!
//! `kira approve` NÃO está aqui: o subcomando ainda não existe (W5/PR18) —
//! incluí-lo na allowlist seria prometer um argv que falha.

use std::path::{Path, PathBuf};

pub mod check;
pub mod confirmacao;
pub mod log;
pub mod runner;
pub mod target;

pub use check::{Prerequisite, Status};
pub use log::{LogLine, MAX_BYTES, MAX_LINES, Stream};
pub use runner::{RunEvent, Runner, RunnerError};
pub use target::{TargetSheet, known_work_items};

/// Ação que o plano final executa (já tem id quando precisa).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Scan,
    Triage { work_item: u32 },
    PrDraft { work_item: u32 },
}

/// Pedido da tecla, ainda SEM id — o que `decide` devolve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    Scan,
    Triage,
    PrDraft,
}

impl Request {
    /// `t`/`d` precisam do id antes de virar plano; `s` não.
    pub fn needs_work_item(self) -> bool {
        matches!(self, Request::Triage | Request::PrDraft)
    }

    /// Tecla exibida no rodapé.
    pub fn key(self) -> char {
        match self {
            Request::Scan => 's',
            Request::Triage => 't',
            Request::PrDraft => 'd',
        }
    }

    /// Rótulo curto do chip (`s Scan`, `t Triage`, `d PR`).
    pub fn label(self) -> &'static str {
        match self {
            Request::Scan => "Scan",
            Request::Triage => "Triage",
            Request::PrDraft => "PR",
        }
    }

    /// Título da folha de alvo.
    pub fn target_title(self) -> &'static str {
        match self {
            Request::Scan => "alvo do scan",
            Request::Triage => "alvo do triage",
            Request::PrDraft => "alvo do rascunho de PR",
        }
    }

    /// Ação com o id resolvido (`None` quando o pedido exige id e ele falta).
    pub fn into_action(self, work_item: Option<u32>) -> Option<Action> {
        match self {
            Request::Scan => Some(Action::Scan),
            Request::Triage => work_item.map(|work_item| Action::Triage { work_item }),
            Request::PrDraft => work_item.map(|work_item| Action::PrDraft { work_item }),
        }
    }
}

impl Action {
    pub fn request(self) -> Request {
        match self {
            Action::Scan => Request::Scan,
            Action::Triage { .. } => Request::Triage,
            Action::PrDraft { .. } => Request::PrDraft,
        }
    }

    pub fn work_item(self) -> Option<u32> {
        match self {
            Action::Scan => None,
            Action::Triage { work_item } | Action::PrDraft { work_item } => Some(work_item),
        }
    }

    /// Título do popup — verbo constante com o alvo explícito.
    pub fn title(self) -> String {
        match self {
            Action::Scan => "executar scan".to_string(),
            Action::Triage { work_item } => format!("executar triage de #{work_item}"),
            Action::PrDraft { work_item } => format!("rascunho de PR de #{work_item}"),
        }
    }

    /// Verbo do rodapé do popup (`Enter executar scan`).
    pub fn verb(self) -> &'static str {
        match self {
            Action::Scan => "Executar scan",
            Action::Triage { .. } => "Executar triage",
            Action::PrDraft { .. } => "Gerar rascunho",
        }
    }

    /// argv ALLOWLIST. Testado literalmente: nenhuma flag extra entra sem
    /// revisão, e nada aqui é shell.
    pub fn argv(self, root: &Path) -> Vec<String> {
        match self {
            Action::Scan => vec![
                "scan".to_string(),
                "--path".to_string(),
                root.display().to_string(),
            ],
            Action::Triage { work_item } => vec!["triage".to_string(), work_item.to_string()],
            Action::PrDraft { work_item } => {
                vec!["pr".to_string(), "draft".to_string(), work_item.to_string()]
            }
        }
    }

    /// Efeitos declarados ANTES de rodar (o popup mostra exatamente isto).
    pub fn effects(self) -> Vec<Effect> {
        match self {
            Action::Scan => vec![
                Effect::WritesFindings,
                Effect::WritesSession,
                Effect::Network("registro npm/pnpm (audit)"),
            ],
            Action::Triage { .. } => vec![
                Effect::WritesWikiEpisode,
                Effect::WritesSession,
                Effect::Network("Azure DevOps Work Items"),
                Effect::Llm,
            ],
            // `pr draft` é LOCAL por contrato (AGENTS.md): nada de rede, LLM
            // ou escrita — o efeito é só o texto no overlay.
            Action::PrDraft { .. } => Vec::new(),
        }
    }
}

/// Efeito de uma execução, em vocabulário do dono.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// `<root>/.kira/findings.json` (substitui o arquivo anterior).
    WritesFindings,
    /// Episódio em `.kira/wiki/bugs/<id>.md`.
    WritesWikiEpisode,
    /// Sessão na trilha local — gravada pelo processo filho.
    WritesSession,
    /// Acesso de rede, com o destino declarado.
    Network(&'static str),
    /// Backend de IA resolvido pelo filho (nunca implícito pela TUI).
    Llm,
}

impl Effect {
    /// Linha do popup, já com o prefixo alinhado.
    pub fn describe(self) -> String {
        match self {
            Effect::WritesFindings => {
                "grava  .kira/findings.json (substitui o anterior)".to_string()
            }
            Effect::WritesWikiEpisode => "grava  episódio em .kira/wiki/bugs/<id>.md".to_string(),
            Effect::WritesSession => "grava  sessão na trilha local".to_string(),
            Effect::Network(what) => format!("rede   {what}"),
            Effect::Llm => "ia     backend do ambiente (nunca implícito)".to_string(),
        }
    }
}

/// Plano imutável: tudo que o runner precisa, nada que ele possa inventar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionPlan {
    pub action: Action,
    /// SEMPRE `current_exe()`; testes injetam um shim.
    pub program: PathBuf,
    pub argv: Vec<String>,
    /// Alvo declarado (root do snapshot), não o cwd do processo filho.
    pub root: PathBuf,
    pub effects: Vec<Effect>,
    pub prerequisites: Vec<Prerequisite>,
}

impl ActionPlan {
    /// Linha copiável para a ajuda/notice (`kira scan --path …`).
    pub fn command_line(&self) -> String {
        self.argv.join(" ")
    }
}

/// Popup de confirmação aberto: o plano já está montado e NADA rodou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmState {
    pub plan: ActionPlan,
}

/// Situação da execução em andamento (ou da última que terminou).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecStatus {
    Running,
    Exited { code: Option<i32> },
}

/// O que o overlay `o` desenha: linhas já sanitizadas, relógio de inicio e
/// estado do processo. Não guarda `Child` nem canal — isso é do `run`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecState {
    pub action: Action,
    pub lines: Vec<LogLine>,
    pub status: ExecStatus,
    pub started_unix: i64,
    /// Follow ligado = a tela acompanha o fim da saída.
    pub follow: bool,
    pub scroll: usize,
    /// Linhas que saíram pelo teto de buffer (o overlay avisa).
    pub dropped: usize,
}

impl ExecState {
    pub fn new(action: Action, started_unix: i64) -> Self {
        Self {
            action,
            lines: Vec::new(),
            status: ExecStatus::Running,
            started_unix,
            follow: true,
            scroll: 0,
            dropped: 0,
        }
    }

    pub fn is_running(&self) -> bool {
        matches!(self.status, ExecStatus::Running)
    }

    /// Rolar para cima desliga o follow (padrão de log: o dono assumiu o scroll).
    /// A saturação é local para a camada de ação não depender do `app`.
    pub fn scroll_by(&mut self, offset: isize) {
        self.scroll = if offset < 0 {
            self.scroll.saturating_sub(offset.unsigned_abs())
        } else if offset == isize::MAX {
            usize::MAX
        } else {
            self.scroll.saturating_add(offset as usize)
        };
        self.follow = false;
    }
}

/// Contexto injetado: nenhum I/O escondido dentro de [`plan`].
pub struct ActionCtx<'a> {
    pub root: Option<&'a Path>,
    pub program: &'a Path,
    pub org_project: Option<(&'a str, &'a str)>,
    pub pat_present: bool,
    /// Resolve uma ferramenta no PATH (o `binresolve` real em produção).
    pub tools: &'a dyn Fn(&str) -> bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    NoProjectRoot,
    WorkItemZero,
    ProgramMissing,
}

impl PlanError {
    /// Mensagem acionável (vai para o rodapé, nunca popup de erro).
    pub fn message(&self) -> &'static str {
        match self {
            PlanError::NoProjectRoot => {
                "sem projeto detectado — rode `kira dashboard` dentro do repositório"
            }
            PlanError::WorkItemZero => "work item inválido: informe um id numérico maior que zero",
            PlanError::ProgramMissing => {
                "não foi possível localizar o binário do kira para executar a ação"
            }
        }
    }
}

/// Monta o plano da ação: allowlist de argv + efeitos + pré-requisitos.
///
/// Puro sobre o contexto: a única coisa que ele "sabe" do SO é o que veio em
/// [`ActionCtx`], então o teste fixa argv sem tocar terminal, PATH ou rede.
pub fn plan(action: Action, ctx: &ActionCtx<'_>) -> Result<ActionPlan, PlanError> {
    let root = ctx.root.ok_or(PlanError::NoProjectRoot)?;
    if ctx.program.as_os_str().is_empty() {
        return Err(PlanError::ProgramMissing);
    }
    if action.work_item().is_some_and(|work_item| work_item == 0) {
        return Err(PlanError::WorkItemZero);
    }
    Ok(ActionPlan {
        action,
        program: ctx.program.to_path_buf(),
        argv: action.argv(root),
        root: root.to_path_buf(),
        effects: action.effects(),
        prerequisites: check::resolve_all(action, ctx),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shim de ferramenta: `dotnet` presente, `pnpm` ausente.
    fn tools(name: &str) -> bool {
        name == "dotnet"
    }

    fn ctx<'a>(
        root: &'a Path,
        program: &'a Path,
        tools: &'a dyn Fn(&str) -> bool,
    ) -> ActionCtx<'a> {
        ActionCtx {
            root: Some(root),
            program,
            org_project: Some(("minha-org", "Kira")),
            pat_present: true,
            tools,
        }
    }

    fn root() -> PathBuf {
        PathBuf::from("C:/proj")
    }

    fn program() -> PathBuf {
        PathBuf::from("C:/bin/kira.exe")
    }

    #[test]
    fn plan_uses_allowlisted_argv_and_never_a_shell() {
        let (root, program) = (root(), program());
        let tools = tools as fn(&str) -> bool;
        let ctx = ctx(&root, &program, &tools);

        let scan = plan(Action::Scan, &ctx).unwrap_or_else(|err| panic!("{err:?}"));
        assert_eq!(
            scan.argv,
            vec!["scan", "--path", "C:/proj"],
            "o alvo vai explícito: nunca depende do cwd do filho"
        );

        let triage =
            plan(Action::Triage { work_item: 4821 }, &ctx).unwrap_or_else(|err| panic!("{err:?}"));
        assert_eq!(triage.argv, vec!["triage", "4821"]);

        let draft =
            plan(Action::PrDraft { work_item: 4821 }, &ctx).unwrap_or_else(|err| panic!("{err:?}"));
        assert_eq!(draft.argv, vec!["pr", "draft", "4821"]);

        for plan in [&scan, &triage, &draft] {
            assert_eq!(plan.program, program, "sempre o próprio binário");
            for token in &plan.argv {
                // Comparação por TOKEN: `--path` não pode ser confundido com
                // um `--pat` (é o mesmo prefixo, e o alvo do scan é `--path`).
                assert!(
                    !token.starts_with("--apply"),
                    "nenhuma ação carrega --apply: {token}"
                );
                for forbidden in ["--pat", "--llm", "--file", "cmd", "sh", "bash", "-c"] {
                    assert_ne!(token, forbidden, "token proibido no argv");
                }
                for shell in ['|', '&', ';', '>', '<'] {
                    assert!(
                        !token.contains(shell),
                        "sem metacaractere de shell: {token}"
                    );
                }
            }
        }
        assert_eq!(scan.command_line(), "scan --path C:/proj");
    }

    #[test]
    fn triage_and_prdraft_reject_zero_id() {
        let (root, program) = (root(), program());
        let tools = tools as fn(&str) -> bool;
        let ctx = ctx(&root, &program, &tools);
        assert_eq!(
            plan(Action::Triage { work_item: 0 }, &ctx),
            Err(PlanError::WorkItemZero)
        );
        assert_eq!(
            plan(Action::PrDraft { work_item: 0 }, &ctx),
            Err(PlanError::WorkItemZero)
        );
    }

    #[test]
    fn plan_requires_a_project_root_and_a_program() {
        let (root, program) = (root(), program());
        let tools = tools as fn(&str) -> bool;
        let no_root = ActionCtx {
            root: None,
            ..ctx(&root, &program, &tools)
        };
        assert_eq!(plan(Action::Scan, &no_root), Err(PlanError::NoProjectRoot));

        let empty_program = PathBuf::new();
        let no_program = ActionCtx {
            program: &empty_program,
            ..ctx(&root, &program, &tools)
        };
        assert_eq!(
            plan(Action::Scan, &no_program),
            Err(PlanError::ProgramMissing)
        );
        assert!(
            PlanError::NoProjectRoot
                .message()
                .contains("dentro do repositório")
        );
    }

    #[test]
    fn effects_are_declared_for_every_action() {
        // Scan e triage escrevem e/ou tocam rede: o popup precisa dizer.
        let scan: Vec<String> = Action::Scan
            .effects()
            .iter()
            .map(|e| e.describe())
            .collect();
        assert!(scan.iter().any(|line| line.contains(".kira/findings.json")));
        assert!(scan.iter().any(|line| line.starts_with("rede")));
        let triage: Vec<String> = Action::Triage { work_item: 7 }
            .effects()
            .iter()
            .map(|e| e.describe())
            .collect();
        assert!(triage.iter().any(|line| line.contains(".kira/wiki/bugs")));
        assert!(triage.iter().any(|line| line.contains("Azure DevOps")));
        assert!(triage.iter().any(|line| line.starts_with("ia")));
        // `pr draft` é local: nenhum efeito a declarar (nada é gravado).
        assert!(Action::PrDraft { work_item: 7 }.effects().is_empty());
    }

    #[test]
    fn prerequisites_report_missing_tool_as_fail_without_spawning() {
        let (root, program) = (root(), program());
        let tools = tools as fn(&str) -> bool;
        let scan_plan = plan(Action::Scan, &ctx(&root, &program, &tools))
            .unwrap_or_else(|err| panic!("{err:?}"));
        let lines: Vec<String> = scan_plan
            .prerequisites
            .iter()
            .map(Prerequisite::describe)
            .collect();
        assert!(lines.iter().any(|line| line.starts_with("ok")), "{lines:?}");
        assert!(
            lines.iter().any(|line| line.starts_with("falta")),
            "pnpm ausente aparece como falta: {lines:?}"
        );
        // Triage sem PAT: a falta é declarada, não escondida.
        let no_pat = ActionCtx {
            pat_present: false,
            ..ctx(&root, &program, &tools)
        };
        let triage =
            plan(Action::Triage { work_item: 9 }, &no_pat).unwrap_or_else(|err| panic!("{err:?}"));
        assert!(
            triage
                .prerequisites
                .iter()
                .any(|prerequisite| prerequisite.status == Status::Fail
                    && prerequisite.describe().contains("PAT"))
        );
    }

    #[test]
    fn requests_map_to_actions_only_with_a_valid_id() {
        assert_eq!(Request::Scan.into_action(None), Some(Action::Scan));
        assert_eq!(Request::Triage.into_action(None), None);
        assert_eq!(
            Request::Triage.into_action(Some(12)),
            Some(Action::Triage { work_item: 12 })
        );
        assert_eq!(
            Request::PrDraft.into_action(Some(12)),
            Some(Action::PrDraft { work_item: 12 })
        );
        assert_eq!(Action::Scan.request(), Request::Scan);
        assert!(Request::Triage.needs_work_item());
        assert!(!Request::Scan.needs_work_item());
        assert_eq!(Request::Triage.key(), 't');
    }
}
