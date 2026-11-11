use std::error::Error as StdError;
use std::io::{BufWriter, IsTerminal};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use clap::{Parser, Subcommand};

use kira::board::{self, BoardProvider};
use kira::credentials;
use kira::doctor;
use kira::error::Error;
use kira::format;
use kira::llm::ollama;
use kira::scan;
use kira::setup;
use kira::store::{self, Store, StoreError};
use kira::triage;
use kira::wiki::{WIKI_DIR, Wiki, WikiError, episode};

/// Hard cap for the summary line shown by `kira history`.
const HISTORY_SUMMARY_LINE_CHARS: usize = 160;

/// KIRA — Knowledge-Informed Remediation Agent (*Keep It Running, Always*).
#[derive(Debug, Parser)]
#[command(
    name = "kira",
    version,
    about = "KIRA — Knowledge-Informed Remediation Agent (*Keep It Running, Always*)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print a readable summary of an Azure DevOps work item
    Inspect {
        /// Work item id
        id: i32,
        /// Azure DevOps project name (defaults to $AZURE_DEVOPS_PROJECT or config.toml)
        #[arg(long)]
        project: Option<String>,
    },
    /// Diagnose the Azure DevOps configuration of this machine
    Doctor,
    /// Suggest a triage for a work item using the local LLM (read-only)
    Triage {
        /// Work item id (live path; requires the Azure DevOps env vars)
        id: Option<i32>,
        /// Read the work item from a local JSON file (offline path; no PAT)
        #[arg(long)]
        file: Option<PathBuf>,
        /// Ollama model tag (overrides KIRA_MODEL)
        #[arg(long)]
        model: Option<String>,
    },
    /// Save the Azure DevOps configuration and the PAT in the OS keyring
    #[command(
        after_help = "Exemplos:\n  kira setup                    # organização, projeto e a pergunta 'Já tem um PAT pronto? [S/n]'\n                                #   S: cole o PAT | n: passos + confirmar abrir no navegador\n  kira setup --no-browser       # pede org/projeto/PAT sem abrir o navegador\n  kira setup --org minha-org --project MeuProjeto --pat <token>   # sem perguntas"
    )]
    Setup {
        /// Azure DevOps organization name (skips the prompt)
        #[arg(long)]
        org: Option<String>,
        /// Azure DevOps project name (skips the prompt)
        #[arg(long)]
        project: Option<String>,
        /// Remove the saved PAT from the keyring and exit
        #[arg(long)]
        reset: bool,
        /// PAT passed straight on the command line (skips the prompt; never echoed)
        #[arg(long)]
        pat: Option<String>,
        /// Skip the PAT steps and the browser open; paste the PAT when asked
        #[arg(long)]
        no_browser: bool,
    },
    /// List the recent sessions recorded for the current project
    History {
        /// Number of recent sessions to show (default 10)
        #[arg(long)]
        last: Option<usize>,
    },
    /// Add a profile note (fact) to the current project, or remove one
    Note {
        /// Text of the note to add
        text: Option<String>,
        /// Remove the note with the given number (kira note --remove 2)
        #[arg(long)]
        remove: Option<i64>,
    },
    /// Print the project profile: root, first seen date and all facts
    Profile,
    /// Scan the project with the stack-standard tools (dotnet, npm/pnpm) and
    /// write a deterministic findings.json — no AI in this phase
    Scan {
        /// Project folder (defaults to the project root detected like in the store)
        #[arg(long)]
        path: Option<PathBuf>,
    },
    /// Read and grow the project wiki at docs/wiki (knowledge, not chat logs)
    #[command(
        after_help = "Exemplos:\n  kira wiki list\n  kira wiki show contexto\n  kira wiki decision \"fix roda em jail mínimo — ADR-0011\""
    )]
    Wiki {
        #[command(subcommand)]
        action: WikiAction,
        /// Project root (defaults to the detected project root)
        #[arg(long)]
        path: Option<PathBuf>,
    },
}

/// `kira wiki` actions (ADR-0013: markdown is the source of truth).
#[derive(Debug, Subcommand)]
enum WikiAction {
    /// List the pages that exist in docs/wiki
    List,
    /// Print one page (e.g. contexto, decisoes, bugs/4821-sessao-expirada)
    Show {
        /// Page id
        page: String,
    },
    /// Append a decision to decisoes.md (masked before writing)
    Decision {
        /// Decision text
        text: String,
    },
    /// Rebuild the derived search index from the markdown on disk
    Reindex,
    /// Search the wiki (needs `kira wiki reindex` at least once)
    Search {
        /// What to look for
        query: String,
        /// Maximum hits (default 5)
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Delete a page from docs/wiki and from the search index (LGPD)
    Forget {
        /// Page id (e.g. bugs/4821-sessao-expirada)
        page: String,
    },
    /// Promote pages staged by an unattended cycle into docs/wiki
    Promote,
    /// Print the whole wiki as one markdown bundle (or save it with --out)
    Export {
        /// Save the bundle at this path, inside the project root
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

/// Result of one command run: exit code, plain success flag (kept separate
/// from the exit code so the automatic session trail does not need to test
/// `ExitCode` for equality) and what was printed — the summary for the trail.
struct Outcome {
    code: ExitCode,
    ok: bool,
    /// Whether this command writes a session row (inspect/doctor/triage do;
    /// the store commands and `setup` do not).
    records: bool,
    summary: String,
}

impl Outcome {
    fn success(summary: impl Into<String>) -> Self {
        Self {
            code: ExitCode::SUCCESS,
            ok: true,
            records: true,
            summary: summary.into(),
        }
    }

    fn failure(summary: impl Into<String>) -> Self {
        Self {
            code: ExitCode::FAILURE,
            ok: false,
            records: true,
            summary: summary.into(),
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    let started_at = store::now_rfc3339_utc();
    let started = Instant::now();
    let (command, args, model) = session_context(&cli.command);
    let outcome = run(cli.command).await;
    // Automatic session trail: never breaks the main command. The store is
    // opened twice per run (here and only here) to keep the trail decoupled
    // from the command code.
    if outcome.records {
        record_session(SessionRun {
            command: &command,
            args: &args,
            model,
            started_at: &started_at,
            duration_ms: i64::try_from(started.elapsed().as_millis()).unwrap_or(0),
            ok: outcome.ok,
            summary: &outcome.summary,
        });
    }
    outcome.code
}

async fn run(command: Command) -> Outcome {
    match command {
        Command::Inspect { id, project } => match inspect(id, project).await {
            Ok(text) => {
                print!("{text}");
                Outcome::success(text)
            }
            Err(err) => {
                print_error(&err);
                Outcome::failure(err.to_string())
            }
        },
        Command::Doctor => {
            let (ok, summary) = doctor::run().await;
            Outcome {
                code: exit_code(ok),
                ok,
                records: true,
                summary,
            }
        }
        Command::Triage { id, file, model } => triage_command(id, file, model).await,
        Command::Setup {
            org,
            project,
            reset,
            pat,
            no_browser,
        } => Outcome {
            code: setup_command(org, project, reset, pat, no_browser).await,
            ok: true,
            records: false,
            summary: String::new(),
        },
        Command::History { last } => store_command(history_output(last)),
        Command::Note { text, remove } => store_command(note_output(text, remove)),
        Command::Profile => store_command(profile_output()),
        Command::Scan { path } => scan_command(path.as_deref()),
        Command::Wiki { action, path } => wiki_command(action, path.as_deref()),
    }
}

/// Boolean → process exit code.
fn exit_code(ok: bool) -> ExitCode {
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Runs a store-backed command (`history`/`note`/`profile`): here the store
/// IS the command, so failure prints a normal error and exits non-zero. These
/// commands do NOT write a session row.
fn store_command(outcome: Result<String, StoreError>) -> Outcome {
    match outcome {
        Ok(text) => {
            print!("{text}");
            Outcome {
                code: ExitCode::SUCCESS,
                ok: true,
                records: false,
                summary: text,
            }
        }
        Err(err) => {
            print_error(&err);
            Outcome {
                code: exit_code(false),
                ok: false,
                records: false,
                summary: err.to_string(),
            }
        }
    }
}

fn print_error(err: &dyn StdError) {
    eprintln!("erro: {err}");
    let mut source = StdError::source(err);
    while let Some(cause) = source {
        eprintln!("  causado por: {cause}");
        source = cause.source();
    }
}

/// Description of the current run for the automatic session trail: named
/// command, how it was invoked and (only for triage) the resolved model.
fn session_context(command: &Command) -> (String, String, Option<String>) {
    match command {
        Command::Inspect { id, project } => {
            let mut args = id.to_string();
            if let Some(project) = project {
                args.push_str(&format!(" --project {project}"));
            }
            ("inspect".to_string(), args, None)
        }
        Command::Doctor => ("doctor".to_string(), String::new(), None),
        Command::Triage { id, file, model } => {
            let mut args = String::new();
            match (id, file) {
                (Some(id), _) => args = id.to_string(),
                (None, Some(file)) => args = format!("--file {}", file.display()),
                (None, None) => {}
            }
            (
                "triage".to_string(),
                args,
                Some(model.clone().unwrap_or_else(ollama::model_from_env)),
            )
        }
        Command::Setup { .. } => ("setup".to_string(), String::new(), None),
        Command::History { last } => (
            "history".to_string(),
            last.map_or_else(String::new, |n| format!("--last {n}")),
            None,
        ),
        Command::Note { text, remove } => ("note".to_string(), note_args_text(text, remove), None),
        Command::Profile => ("profile".to_string(), String::new(), None),
        Command::Scan { path } => (
            "scan".to_string(),
            path.as_ref()
                .map(|path| format!("--path {}", path.display()))
                .unwrap_or_default(),
            None,
        ),
        Command::Wiki { action, path } => {
            let args = match action {
                WikiAction::List => "list".to_string(),
                WikiAction::Show { page } => format!("show {page}"),
                WikiAction::Decision { text } => format!("decision {text}"),
                WikiAction::Reindex => "reindex".to_string(),
                WikiAction::Search { query, limit } => match limit {
                    Some(limit) => format!("search {query} --limit {limit}"),
                    None => format!("search {query}"),
                },
                WikiAction::Forget { page } => format!("forget {page}"),
                WikiAction::Promote => "promote".to_string(),
                WikiAction::Export { out } => match out {
                    Some(out) => format!("export --out {}", out.display()),
                    None => "export".to_string(),
                },
            };
            let args = match path {
                Some(path) => format!("{args} --path {}", path.display()),
                None => args,
            };
            ("wiki".to_string(), args, None)
        }
    }
}

/// Args description for the `note` trail (no note text ends up doubled).
fn note_args_text(text: &Option<String>, remove: &Option<i64>) -> String {
    match (remove, text) {
        (Some(number), _) => format!("--remove {number}"),
        (None, Some(text)) => text.clone(),
        (None, None) => String::new(),
    }
}

/// Everything the trail needs to record one finished session.
struct SessionRun<'a> {
    command: &'a str,
    args: &'a str,
    model: Option<String>,
    started_at: &'a str,
    duration_ms: i64,
    ok: bool,
    summary: &'a str,
}

/// Records the session trail, FIRST-CLASS fail-open: every store problem is
/// reported as a single warning on stderr and the main command's code/output
/// is untouched. No store call anywhere else in the dispatch path.
fn record_session(run: SessionRun<'_>) {
    if let Err(cause) = record_session_inner(&run) {
        eprintln!("aviso: trilha não foi gravada ({cause})");
    }
}

fn record_session_inner(run: &SessionRun<'_>) -> Result<(), String> {
    let root = store::detect_project_root()
        .ok_or_else(|| "não foi possível obter o diretório atual".to_string())?;
    let store = Store::open_default().map_err(|error| error.to_string())?;
    let project = store
        .ensure_project(&root)
        .map_err(|error| error.to_string())?;
    store
        .record(
            &project,
            run.command,
            run.args,
            run.model.as_deref(),
            run.started_at,
            run.duration_ms,
            run.ok,
            run.summary,
        )
        .map_err(|error| error.to_string())
}

// ---------------------------------------------------------------------------
// Store-backed commands (these touch no network and no keyring)
// ---------------------------------------------------------------------------

/// Opens the default store and finds the current project WITHOUT creating a
/// row (queries only); `None` when the project was never recorded.
fn current_project(store: &Store) -> Result<Option<store::Project>, StoreError> {
    match store::detect_project_root() {
        Some(root) => store.find_project(&root),
        None => Ok(None),
    }
}

/// `kira history [--last N]` — recent sessions of the current project.
fn history_output(last: Option<usize>) -> Result<String, StoreError> {
    let store = Store::open_default()?;
    let Some(project) = current_project(&store)? else {
        return Ok(
            "nenhuma sessão registrada ainda — rode kira inspect/triage/doctor para começar\n"
                .to_string(),
        );
    };
    let sessions = store.last_sessions(&project.id, last.unwrap_or(10))?;
    if sessions.is_empty() {
        return Ok(format!(
            "nenhuma sessão registrada ainda para o projeto '{}'\n",
            project.name
        ));
    }
    let mut text = format!(
        "história do projeto '{}' — mais recentes primeiro:\n",
        project.name
    );
    for session in &sessions {
        text.push_str(&format_session_line(session));
    }
    Ok(text)
}

/// One `kira history` line: `#id  ✓/✗  data local  comando  duração  modelo?`.
/// The "one-liner" local-time convention lives in the store: SQLite's
/// `datetime(started_at, 'localtime')`.
fn format_session_line(session: &store::SessionRow) -> String {
    let status = if session.ok { "✓" } else { "✗" };
    let local = session
        .local_started_at
        .as_deref()
        .unwrap_or(&session.started_at);
    let mut invocation = session.command.clone();
    if !session.args.is_empty() {
        invocation.push(' ');
        invocation.push_str(&session.args);
    }
    let model = match &session.model {
        Some(model) => format!(" [{model}]"),
        None => String::new(),
    };
    format!(
        "#{}  {}  {}  {}{}  {}  {}\n",
        session.id,
        status,
        local,
        invocation,
        model,
        format_duration_ms(session.duration_ms),
        first_line(&session.summary, HISTORY_SUMMARY_LINE_CHARS),
    )
}

/// `123ms` / `1.4s` / `2m05s` display for a session duration.
fn format_duration_ms(duration_ms: i64) -> String {
    match duration_ms {
        0..=999 => format!("{duration_ms}ms"),
        _ if duration_ms < 60_000 => {
            format!("{:.1}s", duration_ms as f64 / 1_000.0)
        }
        _ => format!(
            "{}m{:02}s",
            duration_ms / 60_000,
            (duration_ms % 60_000) / 1_000
        ),
    }
}

/// First non-empty-ish line of a summary, clipped for a history table.
fn first_line(summary: &str, max_chars: usize) -> String {
    let line = summary.trim().lines().next().unwrap_or_default();
    if line.chars().count() <= max_chars {
        line.to_string()
    } else {
        let clipped: String = line.chars().take(max_chars).collect();
        format!("{clipped}…")
    }
}

/// `kira profile` — project identity plus every stored fact, in order.
fn profile_output() -> Result<String, StoreError> {
    let store = Store::open_default()?;
    let Some(project) = current_project(&store)? else {
        return Ok(
            "nenhum perfil para este diretório ainda — rode kira inspect/triage/doctor ou kira note\n"
                .to_string(),
        );
    };
    let facts = store.facts(&project.id)?;
    let mut text = format!(
        "projeto: {}\nroot: {}\nid: {}\nprimeira sessão: {}",
        project.name,
        store::display_path(Path::new(&project.root_path)),
        project.id,
        project.first_seen_at
    );
    if let Some(local) = store.local_display(&project.first_seen_at)? {
        text.push_str(&format!(" ({local} local)"));
    }
    text.push('\n');
    if facts.is_empty() {
        text.push_str("fatos: nenhum — adicione com 'kira note \"texto do fato\"'\n");
    } else {
        text.push_str(&format!("fatos ({}):\n", facts.len()));
        for fact in &facts {
            text.push_str(&format!(
                "  {}  ({})  {}\n",
                fact.key,
                fact.local_updated_at.as_deref().unwrap_or(&fact.updated_at),
                indent_continuation_lines(&fact.value)
            ));
        }
    }
    Ok(text)
}

/// Keeps multi-line fact values readable inside the indented list.
fn indent_continuation_lines(value: &str) -> String {
    value.split('\n').collect::<Vec<_>>().join("\n    ")
}

enum NoteAction {
    Add(String),
    Remove(i64),
}
impl std::fmt::Debug for NoteAction {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NoteAction::Add(_) => formatter.write_str("Add(<texto da nota>)"),
            NoteAction::Remove(number) => formatter.write_fmt(format_args!("Remove({number})")),
        }
    }
}

/// `kira note <texto>` / `kira note --remove <N>` — validates the form first.
fn note_action(text: Option<String>, remove: Option<i64>) -> Result<NoteAction, String> {
    match (text, remove) {
        (Some(text), None) if text.trim().is_empty() => {
            Err("o texto da nota não pode ficar vazio — ex.: kira note \"build roda com make\"".to_string())
        }
        (Some(text), None) => Ok(NoteAction::Add(text)),
        (None, Some(number)) if number >= 1 => Ok(NoteAction::Remove(number)),
        (None, Some(number)) => Err(format!(
            "número da nota inválido ({number}) — use kira note --remove <N> com N ≥ 1 (veja os números com kira profile)"
        )),
        (Some(_), Some(_)) => Err(
            "informe apenas uma ação: o texto da nota OU --remove <N>".to_string(),
        ),
        (None, None) => Err(
            "informe o texto da nota — ex.: kira note \"build roda com make\"  (remover: kira note --remove <N>)"
                .to_string(),
        ),
    }
}

fn note_output(text: Option<String>, remove: Option<i64>) -> Result<String, StoreError> {
    let action =
        note_action(text, remove).map_err(|message| StoreError::InvalidNote { message })?;
    let store = Store::open_default()?;
    let Some(root) = store::detect_project_root() else {
        return Err(StoreError::InvalidNote {
            message: "não foi possível obter o diretório atual".to_string(),
        });
    };
    // `note` materializes the project on first contact: the fact belongs to
    // the profile.
    let project = store.ensure_project(&root)?;
    match action {
        NoteAction::Add(value) => {
            let key = store.add_fact(&project.id, &value)?;
            Ok(format!("fato gravado: {key} — {value}\n"))
        }
        NoteAction::Remove(number) => {
            let key = format!("note-{number}");
            if !store.remove_fact(&project.id, &key)? {
                return Err(StoreError::InvalidNote {
                    message: format!("não existe o fato '{key}' — veja a lista com kira profile"),
                });
            }
            Ok(format!("fato removido: {key}\n"))
        }
    }
}

// ---------------------------------------------------------------------------
// Network/keyring commands
// ---------------------------------------------------------------------------

/// `kira scan [--path <dir>]`: the scan reports its session like
/// inspect/doctor/triage (success or failure). The stdout first line doubles
/// as the trail summary (stacks + counts).
fn scan_command(path: Option<&Path>) -> Outcome {
    match scan::run(path) {
        Ok(text) => {
            print!("{text}");
            Outcome::success(text)
        }
        Err(err) => {
            print_error(&err);
            Outcome::failure(err.to_string())
        }
    }
}

/// `kira wiki <action>` — reads and appends to the project wiki (ADR-0013).
///
/// Deterministic: no model, no network (ADR-0016). Every write goes through
/// `redact` (privacy) and `guard::path` (confinement), inside the Wiki type.
fn wiki_command(action: WikiAction, path: Option<&Path>) -> Outcome {
    let root = match path {
        Some(root) => root.to_path_buf(),
        None => store::detect_project_root().unwrap_or_else(|| PathBuf::from(".")),
    };
    match wiki_output(&action, &root) {
        Ok(text) => {
            print!("{text}");
            Outcome {
                code: ExitCode::SUCCESS,
                ok: true,
                records: false,
                summary: text,
            }
        }
        Err(err) => {
            print_error(&err);
            Outcome {
                code: exit_code(false),
                ok: false,
                records: false,
                summary: err.to_string(),
            }
        }
    }
}

/// Pure-ish body of `kira wiki`: the only I/O is the wiki itself.
fn wiki_output(action: &WikiAction, root: &Path) -> Result<String, WikiError> {
    let wiki = Wiki::open(root)?;
    match action {
        WikiAction::List => {
            let pages = wiki.list()?;
            if pages.is_empty() {
                return Ok(format!(
                    "a wiki de '{}' está vazia — a primeira página nasce com `kira wiki decision \"...\"`\n",
                    root.display()
                ));
            }
            let mut text = format!(
                "wiki de '{}' ({} páginas, em docs/wiki):\n",
                root.display(),
                pages.len()
            );
            for page in pages {
                text.push_str(&format!("  {page}\n"));
            }
            Ok(text)
        }
        WikiAction::Show { page } => match wiki.read(page)? {
            Some(content) => Ok(content),
            None => Ok(format!(
                "página '{page}' ainda não existe — registre conhecimento com `kira wiki decision \"...\"`\n"
            )),
        },
        WikiAction::Decision { text } => {
            let written = wiki.append("decisoes", text)?;
            Ok(format!("decisão registrada em {}\n", written.display()))
        }
        WikiAction::Reindex => {
            let store = Store::open_default()?;
            let indexed = wiki.reindex(&store)?;
            let engine = if store.wiki_fts5_enabled() {
                "FTS5"
            } else {
                "LIKE (FTS5 indisponível)"
            };
            Ok(format!(
                "índice da wiki reconstruído: {indexed} páginas, busca por {engine}\n"
            ))
        }
        WikiAction::Search { query, limit } => {
            let store = Store::open_default()?;
            let hits = wiki.search(&store, query, limit.unwrap_or(5))?;
            if hits.is_empty() {
                return Ok(format!(
                    "nenhuma página da wiki contém '{query}' — rode `kira wiki reindex` se a wiki mudou desde a última indexação\n"
                ));
            }
            let mut text = format!("{} página(s) com '{query}':\n", hits.len());
            for hit in hits {
                text.push_str(&format!("  {} [{}] {}\n", hit.slug, hit.kind, hit.title));
                let snippet = hit.snippet.replace('\n', " ");
                text.push_str(&format!("      {snippet}\n"));
            }
            Ok(text)
        }
        WikiAction::Forget { page } => {
            if !wiki.forget(page)? {
                return Ok(format!(
                    "página '{page}' não existe em {} — nada a apagar (veja `kira wiki list`)\n",
                    root.join(WIKI_DIR).display()
                ));
            }
            Ok(format!(
                "página '{page}' apagada de docs/wiki{}\n",
                forget_index(page)
            ))
        }
        WikiAction::Promote => {
            let store = Store::open_default()?;
            let promoted = wiki.promote(&store)?;
            if promoted.is_empty() {
                return Ok(
                    "nada em .kira/wiki-staging/ — nenhum episódio aguardando promoção\n"
                        .to_string(),
                );
            }
            let mut text = format!(
                "{} página(s) promovida(s) para docs/wiki:\n",
                promoted.len()
            );
            for page in promoted {
                text.push_str(&format!("  {page}\n"));
            }
            Ok(text)
        }
        WikiAction::Export { out } => match out {
            Some(dest) => {
                let (path, pages) = wiki.export_to(dest)?;
                Ok(format!(
                    "bundle de {pages} página(s) gravado em {}\n",
                    path.display()
                ))
            }
            None => Ok(wiki.export()?.0),
        },
    }
}

/// Cleans the derived index after a `wiki forget`.
///
/// Never an error: markdown is the truth and `kira wiki reindex` rebuilds the
/// index from the files, so a store problem only becomes a warning in the same
/// line the command already prints.
fn forget_index(page: &str) -> String {
    match Store::open_default().and_then(|store| store.wiki_forget(page)) {
        Ok(true) => " e do índice".to_string(),
        Ok(false) => " (o índice não tinha essa página)".to_string(),
        Err(err) => format!("; aviso: índice não foi limpo ({err}) — rode `kira wiki reindex`"),
    }
}

/// Resolves org/project/PAT (flags > env > keyring/config.toml) and builds the
/// board provider selected by `KIRA_BOARD` (default `azure`).
fn resolve_board(project_override: Option<&str>) -> Result<Box<dyn BoardProvider>, Error> {
    let config = credentials::resolve_azure_config(project_override)?;
    board::for_board(
        &board::name_from_env(),
        &config.pat,
        &config.organization,
        &config.project,
    )
}

async fn inspect(id: i32, project_flag: Option<String>) -> Result<String, Error> {
    let board = resolve_board(project_flag.as_deref())?;
    let item = board.work_item(id).await?;
    Ok(format::format_work_item(&item))
}

/// Source of the work item to triage: a live Azure DevOps id or a local file.
enum TriageSource {
    Id(i32),
    File(PathBuf),
}

fn triage_source(id: Option<i32>, file: Option<PathBuf>) -> Result<TriageSource, String> {
    match (id, file) {
        (Some(id), None) => Ok(TriageSource::Id(id)),
        (None, Some(path)) => Ok(TriageSource::File(path)),
        (Some(_), Some(_)) => {
            Err("informe apenas uma origem: o id do work item OU --file <caminho.json>".to_string())
        }
        (None, None) => Err(
            "informe a origem do work item: o id (caminho vivo, precisa do Azure configurado) \
             OU --file <caminho.json> (offline, sem PAT)"
                .to_string(),
        ),
    }
}

async fn triage_command(
    id: Option<i32>,
    file: Option<PathBuf>,
    model_flag: Option<String>,
) -> Outcome {
    let source = match triage_source(id, file) {
        Ok(source) => source,
        Err(message) => {
            eprintln!("erro: {message}");
            eprintln!("uso: kira triage <work-item-id>  |  kira triage --file <caminho.json>");
            return Outcome::failure(message);
        }
    };
    let base_url = ollama::base_url_from_env();
    let model = model_flag.unwrap_or_else(ollama::model_from_env);
    let result = match source {
        TriageSource::Id(id) => triage_live(&base_url, &model, id).await,
        TriageSource::File(path) => triage::triage_file(&base_url, &model, &path).await,
    };
    match result {
        Ok(report) => {
            print!("{}", report.output);
            // PR7: identifica a geração do prompt no stderr — duas saídas do
            // modelo só são comparáveis se o texto do prompt foi o mesmo.
            eprintln!("prompt: {}", kira::llm::prompts::triage_prompt_id());
            // ADR-0015: the user sees exactly which evidence the model got. Goes
            // to stderr so the three-section answer stays pipeable.
            if !report.context.is_empty() {
                eprintln!("{}", report.context.summary());
            }
            for note in &report.context.notes {
                eprintln!("aviso: contexto — {note}");
            }
            // Episodic capture (ADR-0013, PR30): fail-open, never changes the
            // output or the exit code.
            if let Some(note) = record_triage_episode(&report) {
                eprint!("{note}");
            }
            Outcome::success(report.output)
        }
        Err(err) => {
            print_error(&err);
            Outcome::failure(err.to_string())
        }
    }
}

/// Episodic capture after a triage: a wiki problem is one `aviso:` line on
/// stderr, never a failed command (same fail-open contract as the session
/// trail).
fn record_triage_episode(report: &triage::TriageReport) -> Option<String> {
    match record_triage_episode_inner(report) {
        Ok(Some(path)) => Some(format!(
            "wiki: episódio do item {} registrado em {}\n",
            report.item_id,
            path.display()
        )),
        Ok(None) => None,
        Err(cause) => Some(format!("aviso: wiki não registrou o episódio ({cause})\n")),
    }
}

/// Builds the episode from what the command already produced and writes it to
/// the committed wiki — a human ran `kira triage` (ADR-0013). An unattended
/// cycle must use [`episode::EpisodeTarget::Staging`] instead (PR15).
fn record_triage_episode_inner(report: &triage::TriageReport) -> Result<Option<PathBuf>, String> {
    let Some(root) = store::detect_project_root() else {
        return Ok(None);
    };
    let wiki = Wiki::open(&root).map_err(|err| err.to_string())?;
    let sections = triage::sections_of(&report.output);
    let findings_brief = scan::read_findings_file(&root)
        .ok()
        .flatten()
        .filter(|findings| !findings.is_empty())
        .map(|findings| scan::severity_brief(&findings));
    let episode = episode::Episode {
        item_id: report.item_id,
        root_cause: sections[0].clone(),
        files: sections[1].clone(),
        confidence: sections[2].clone(),
        findings_brief,
    };
    let path = episode::record(
        &wiki,
        &episode,
        episode::EpisodeTarget::Committed,
        &store::now_rfc3339_utc(),
    )
    .map_err(|err| err.to_string())?;
    Ok(Some(path))
}

async fn triage_live(base_url: &str, model: &str, id: i32) -> Result<triage::TriageReport, Error> {
    let board = resolve_board(None)?;
    triage::triage_live(board.as_ref(), id, base_url, model).await
}

async fn setup_command(
    org: Option<String>,
    project: Option<String>,
    reset: bool,
    pat: Option<String>,
    no_browser: bool,
) -> ExitCode {
    // Fail fast (before the user creates a token) when there is nowhere to
    // save it; on machines without a keyring the env var remains the path.
    let store = match credentials::open_default_store() {
        Ok(store) => store,
        Err(err) => {
            print_error(&err);
            return ExitCode::FAILURE;
        }
    };
    // Modo interativo = stdin é um TTY: só nele o fluxo pergunta
    // "Você já tem um PAT pronto?"; stdin pipado mantém o fast path antigo.
    let interactive = std::io::stdin().is_terminal();
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut input = stdin.lock();
    let mut output = BufWriter::new(stdout.lock());
    setup::run(
        &setup::SetupOptions {
            org,
            project,
            reset,
            pat,
            no_browser,
            interactive,
        },
        store.as_ref(),
        credentials::config_path().as_deref(),
        &setup::env_lookup,
        &mut input,
        &mut output,
        &setup::default_board_factory,
        &setup::open_url_in_browser,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_display_compactly() {
        assert_eq!(format_duration_ms(0), "0ms");
        assert_eq!(format_duration_ms(450), "450ms");
        assert_eq!(format_duration_ms(1_400), "1.4s");
        assert_eq!(format_duration_ms(61_250), "1m01s");
    }

    #[test]
    fn history_shows_the_first_summary_line_clipped() {
        assert_eq!(first_line("Causa raiz:\nTTL curto", 160), "Causa raiz:");
        let long = "x".repeat(180);
        let clipped = first_line(&long, 160);
        assert_eq!(clipped.chars().count(), 161, "160 chars + ellipsis");
        assert!(clipped.ends_with('…'));
    }

    #[test]
    fn note_form_is_single_action() {
        let error = note_action(None, None).unwrap_err();
        assert!(error.contains("informe o texto"), "{error}");
        let error = note_action(Some("a".into()), Some(1)).unwrap_err();
        assert!(error.contains("apenas uma ação"), "{error}");
        let error = note_action(None, Some(0)).unwrap_err();
        assert!(error.contains("inválido"), "{error}");
        assert!(matches!(
            note_action(Some("ok".into()), None),
            Ok(NoteAction::Add(_))
        ));
        assert!(matches!(
            note_action(None, Some(2)),
            Ok(NoteAction::Remove(2))
        ));
    }
}
