use std::error::Error as StdError;
use std::io::{BufWriter, IsTerminal};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use clap::{Parser, Subcommand};

use kira::board::BoardProvider;
use kira::capabilities::{self, Capability};
use kira::credentials;
use kira::doctor;
use kira::emit;
use kira::error::Error;
use kira::format;
use kira::ingest;
use kira::llm::ollama;
use kira::scan;
use kira::setup;
use kira::store::{self, Store, StoreError};
use kira::triage;
use kira::trust;
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
        /// LLM backend: `ollama` (default, local) | `copilot` (costly,
        /// spawns the Copilot CLI and requires `copilot auth login`;
        /// ADR-0019) | `foundry` (BYOK Microsoft Foundry over HTTP, ADR-0020)
        /// — overrides KIRA_LLM. The costly backends are NEVER implicit and
        /// never fall back to Ollama
        #[arg(long, value_parser = ["ollama", "copilot", "foundry"])]
        llm: Option<String>,
        /// Measure the M2 gate (ADR-0018) against a directory of gold labels:
        /// each item is a folder with `item.json` + `gold.json`. Runs triage
        /// offline (`--file` mode) on the chosen backend and compares the
        /// answer to the gold deterministically (token similarity, threshold
        /// 0.50, for the root cause; basename match, for the files) — no LLM
        /// judging itself. Exit code: 0 = >=70% of the items (PASSOU),
        /// 1 = <70% (NÃO PASSOU) or an error
        #[arg(long)]
        eval: Option<PathBuf>,
        /// Print the eval report as JSON (with --eval): per-item scores plus
        /// the predicted/gold texts, for automation
        #[arg(long)]
        json: bool,
        /// Copy the 3 synthetic fixtures (seed-sintetico-*) into an EMPTY
        /// --eval directory, then run the measurement on them — for testing
        /// the format before labelling the 20 real bugs
        #[arg(long)]
        seed: bool,
    },
    /// Configure the local-first setup: dev name → project root → IA backend
    #[command(
        after_help = "Exemplos:\n  kira setup                # nome → projeto → ingestão → backend de IA → resumo\n  kira setup --yes          # não-interativo: nunca bloqueia; ingere, e sem Ollama escolhe \"nenhuma\" (imprime o pulado)\n  kira setup --org minha-org --project MeuProjeto --pat <token>   # atalho: valida e salva sem perguntas\n  kira setup --skip ia      # pula um passo (repetível: ia, ingest; 'pat' segue aceito como no-op)\n  kira setup --lang pt-BR   # idioma dos posts/cards/PR (ADR-0038 D4); salvo como output_lang\n  kira setup --reset        # remove o PAT e a chave do Foundry do keyring; org/projeto/dev ficam\n\n  # A credencial do board nasce em `kira connect azure` (MVP-3), não aqui."
    )]
    Setup {
        /// Azure DevOps organization (atalho para quem já tem — nunca é
        /// perguntada; vai ao config.toml para o `kira connect` achar)
        #[arg(long)]
        org: Option<String>,
        /// Azure DevOps project (atalho para quem já tem — nunca é
        /// perguntado; vai ao config.toml para o `kira connect` achar)
        #[arg(long)]
        project: Option<String>,
        /// Remove o PAT e a chave do Foundry do keyring (e endpoint/deployment
        /// do config.toml) e sai; idempotente
        #[arg(long)]
        reset: bool,
        /// Atalho EXPLÍCITO para quem já tem o token: valida na hora e salva
        /// no keyring (precisa de --org/--project). Sem a flag o setup não
        /// usa credencial nenhuma — a env `AZURE_DEVOPS_PAT` continua sendo
        /// lida pelo `kira connect`; nunca é ecoado — só a máscara aparece
        #[arg(long)]
        pat: Option<String>,
        /// Modo não-interativo: nunca bloqueia; sem Ollama escolhe "nenhuma
        /// por agora" e imprime o pulado
        #[arg(long)]
        yes: bool,
        /// Backend de IA para verificar: ollama (barato) | copilot | foundry
        /// | nenhuma. O caro NUNCA é default nem fallback silencioso — só
        /// entra por escolha explícita
        #[arg(long, value_parser = ["ollama", "copilot", "foundry", "nenhuma"])]
        backend: Option<String>,
        /// Pula um passo (repetível): `ia`, `ingest` (`pat` segue aceito como
        /// no-op — o passo deixou de existir no fluxo local-first)
        #[arg(long)]
        skip: Vec<String>,
        /// How the dev wants to be called (saved as `dev_name` in config.toml;
        /// shown by `kira doctor` — never sent to any LLM prompt)
        #[arg(long)]
        name: Option<String>,
        /// Idioma do que o humano lê (posts, cards e PR): ex. `en`, `pt-BR`.
        /// Salvo como `output_lang` em config.toml; `kira digest` lê na ordem
        /// flag > KIRA_LANG > config > en (ADR-0038 D4)
        #[arg(long)]
        lang: Option<String>,
        /// Força o wizard de texto (sem tela cheia) — para CI, terminal
        /// restrito ou quem prefere linha a linha
        #[arg(long)]
        plain: bool,
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
    /// Ingest the project: walks everything the denylist does not exclude, refreshes
    /// the FTS5 index, runs `scan` (optional) and writes a deterministic
    /// .kira/digest.md — no AI, no network required (ADR-0038)
    #[command(
        after_help = "Exemplos:\n  kira ingest\n  kira ingest --path ./repo --json\n  kira ingest --skip-scan"
    )]
    Ingest {
        /// Project folder (defaults to the project root detected like in the store)
        #[arg(long)]
        path: Option<PathBuf>,
        /// Print the report as JSON instead of the human summary
        #[arg(long)]
        json: bool,
        /// Do not run the stack scanners (offline mode: digest uses the last findings.json)
        #[arg(long)]
        skip_scan: bool,
    },
    /// Trust this repository (or revoke it): the gate before `kira digest` sends any
    /// code to a model. Without trust, nothing leaves the machine (ADR-0038)
    #[command(after_help = "Exemplos:\n  kira trust\n  kira trust --revoke")]
    Trust {
        /// Project folder (defaults to the project root detected like in the store)
        #[arg(long)]
        path: Option<PathBuf>,
        /// Revoke the trust instead of granting it
        #[arg(long)]
        revoke: bool,
    },
    /// Digest the ingested facts into posts with the LOCAL model (ADR-0038): enqueues
    /// what is missing in `posts`, claims with a lease and writes drafts under
    /// .kira/digest/. Requires `kira trust` and a previous `kira ingest`
    #[command(
        after_help = "Exemplos:\n  kira digest --lang pt-BR\n  kira digest --pause 1\n  kira digest --resume --budget 200000"
    )]
    Digest {
        /// Project folder (defaults to the project root detected like in the store)
        #[arg(long)]
        path: Option<PathBuf>,
        /// Ollama model tag (overrides KIRA_MODEL); goes into the cache key
        #[arg(long)]
        model: Option<String>,
        /// Language of the generated posts (pt-BR, en, ...); overrides KIRA_LANG
        #[arg(long)]
        lang: Option<String>,
        /// Token ceiling for this run (default 1_000_000)
        #[arg(long)]
        budget: Option<u64>,
        /// Stop after N delivered posts — the pause switch
        #[arg(long)]
        pause: Option<usize>,
        /// Release posts left claimed by a previous run before starting
        #[arg(long)]
        resume: bool,
        /// Print the report as JSON
        #[arg(long)]
        json: bool,
    },
    /// Emit the digested drafts into the project wiki (.kira/wiki), gated by the
    /// source hash: a post edited by hand is never overwritten without --force
    #[command(after_help = "Exemplos:\n  kira emit\n  kira emit --force")]
    Emit {
        /// Project folder (defaults to the project root detected like in the store)
        #[arg(long)]
        path: Option<PathBuf>,
        /// Overwrite posts edited by hand in .kira/wiki
        #[arg(long)]
        force: bool,
        /// Print the report as JSON
        #[arg(long)]
        json: bool,
    },
    /// Read and grow the local project wiki at .kira/wiki (knowledge, not chat logs)
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
    /// Print the context pack that would reach the LLM prompt — no model, no
    /// network (ADR-0021)
    Context {
        /// Work item id (the live path needs the network; unsupported here —
        /// use --file)
        id: Option<i32>,
        /// Read the work item from a local JSON file (offline path; no PAT)
        #[arg(long)]
        file: Option<PathBuf>,
        /// Also show why each item entered the pack (source, bm25 rank,
        /// finding priority)
        #[arg(long)]
        explain: bool,
        /// Emit the pack as JSON (recall evaluation)
        #[arg(long)]
        json: bool,
    },
    /// Open the terminal dashboard (ADR-0033: leitura por padrão, ação confirmada)
    ///
    /// Interactive ONLY with a TTY stdout; without a TTY (or with --snapshot)
    /// the SAME content is printed as plain text and the command exits 0 —
    /// that is how the CI tests it. Actions run only after an explicit key and
    /// confirmation; `--read-only` disables them (store hash e2e).
    #[command(
        after_help = "Exemplos:\n  kira dashboard                 # TUI interativa (q sair, r reler, ? ajuda)\n  kira dashboard --snapshot      # mesmo conteúdo em texto, para CI/pipes\n  kira dashboard --read-only     # TUI sem ações: hash do store.db byte-idêntico"
    )]
    Dashboard {
        /// Print the text form and exit (never draws, never enters raw mode)
        #[arg(long)]
        snapshot: bool,
        /// Disable every action key (scan/triage/pr draft); navigation only
        #[arg(long)]
        read_only: bool,
    },
    /// Generate the markdown PR draft from what KIRA already knows — no
    /// network and no LLM (W4: triage + findings + wiki, evidence cited)
    #[command(
        after_help = "Exemplos:\n  kira pr draft 4821                          # stdout\n  kira pr draft 4821 --file item.json         # usa o item do board do JSON local\n  kira pr draft 4821 --out draft.md           # escreve o arquivo (caminho explícito)\n  kira pr draft 4821 --json                   # estrutura para consumo por máquina"
    )]
    Pr {
        #[command(subcommand)]
        action: PrAction,
    },
    /// Conecta uma integração opcional (capacidade, ADR-0030): é AQUI que a
    /// credencial nasce (aula do PAT, menu 1/2/3, entrada sem eco, validação
    /// 200/401/403/sem rede, gravação no keyring e marcação da capacidade
    /// conectada POR PROJETO). `github`/`gitlab` continuam "ainda não
    /// implementado" (MVP posterior)
    #[command(
        after_help = "Exemplos:\n  kira connect azure         # board Azure DevOps (aula + menu no TTY)\n  kira connect azure --yes   # não-interativo: valida e conecta com AZURE_DEVOPS_PAT\n  kira connect github        # scm.github — ainda não implementado\n  kira connect gitlab        # scm.gitlab — ainda não implementado"
    )]
    Connect {
        /// Provider da integração: `azure` | `github` | `gitlab`
        provider: String,
        /// Modo não-interativo: valida e conecta com AZURE_DEVOPS_PAT do
        /// ambiente — nunca bloqueia esperando o menu; sem a env, erro
        /// acionável
        #[arg(long)]
        yes: bool,
    },
}

/// `kira pr` actions: for now only the local draft.
#[derive(Debug, Subcommand)]
enum PrAction {
    /// Build the deterministic PR draft (triage + findings + wiki); item data
    /// comes only from `--file` (this command never touches the network)
    Draft {
        /// Work item id whose triage recorded in the store anchors the draft
        id: i32,
        /// Work item JSON saved locally (adds title/context; no PAT, no net)
        #[arg(long)]
        file: Option<PathBuf>,
        /// Write the markdown at this explicit path (never chosen on its own)
        #[arg(long)]
        out: Option<PathBuf>,
        /// Emit the structure as JSON (markdown stays the same content)
        #[arg(long)]
        json: bool,
    },
}

/// `kira wiki` actions (ADR-0013: markdown is the source of truth).
#[derive(Debug, Subcommand)]
enum WikiAction {
    /// List the pages that exist in .kira/wiki
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
    /// Delete a page from .kira/wiki and from the search index (LGPD)
    Forget {
        /// Page id (e.g. bugs/4821-sessao-expirada)
        page: String,
    },
    /// Promote pages staged by an unattended cycle into .kira/wiki
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
    // O item é resolvido AQUI, antes de `run` consumir o comando: o `triage
    // --file` não tem o id nos argumentos, e é justamente esse caminho que
    // deixava o `pr draft` oco (issue #78).
    let item_id = command_item_id(&cli.command);
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
            item_id,
        });
    }
    outcome.code
}

async fn run(command: Command) -> Outcome {
    match command {
        Command::Inspect { id, project } => {
            // ADR-0030 #5/#6: a jornada de board só começa com a capacidade
            // conectada — a prova vem ANTES de resolver credencial ou rede.
            if let Err(blocked) = gate_board() {
                return blocked;
            }
            match inspect(id, project).await {
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
        Command::Doctor => {
            let (ok, summary) = doctor::run().await;
            Outcome {
                code: exit_code(ok),
                ok,
                records: true,
                summary,
            }
        }
        Command::Triage {
            id,
            file,
            model,
            llm,
            eval,
            json,
            seed,
        } => triage_command(id, file, model, llm, eval, json, seed).await,
        Command::Setup {
            org,
            project,
            reset,
            pat,
            yes,
            backend,
            skip,
            name,
            lang,
            plain,
        } => Outcome {
            code: setup_command(
                org, project, reset, pat, yes, backend, skip, name, lang, plain,
            )
            .await,
            ok: true,
            records: false,
            summary: String::new(),
        },
        Command::History { last } => store_command(history_output(last)),
        Command::Note { text, remove } => store_command(note_output(text, remove)),
        Command::Profile => store_command(profile_output()),
        Command::Scan { path } => scan_command(path.as_deref()),
        Command::Ingest {
            path,
            json,
            skip_scan,
        } => ingest_command(path.as_deref(), json, skip_scan),
        Command::Trust { path, revoke } => trust_command(path.as_deref(), revoke),
        Command::Emit { path, force, json } => emit_command(path.as_deref(), force, json),
        Command::Digest {
            path,
            model,
            lang,
            budget,
            pause,
            resume,
            json,
        } => digest_command(path.as_deref(), model, lang, budget, pause, resume, json).await,
        Command::Wiki { action, path } => wiki_command(action, path.as_deref()),
        Command::Context {
            id,
            file,
            explain,
            json,
        } => context_command(id, file, explain, json),
        Command::Dashboard {
            snapshot,
            read_only,
        } => dashboard_command(snapshot, read_only),
        Command::Pr { action } => pr_command(action),
        Command::Connect { provider, yes } => connect_command(&provider, yes).await,
    }
}

/// ADR-0030 #5/#6: a jornada de board (`triage <id>`, `inspect`) só começa com
/// `board.azure` conectada PARA ESTE PROJETO. A conferência roda antes de
/// qualquer credencial ou rede; qualquer falha do store também bloqueia
/// (fail-closed) — sem prova do estado, o comando não executa.
fn gate_board() -> Result<(), Outcome> {
    let Some(root) = store::detect_project_root() else {
        let message =
            "não foi possível determinar o projeto atual — rode dentro da raiz do repositório"
                .to_string();
        eprintln!("erro: {message}");
        return Err(Outcome::failure(message));
    };
    let opened = Store::open_default();
    let store = match opened {
        Ok(store) => store,
        Err(err) => {
            eprintln!("erro: {err}");
            return Err(Outcome::failure(err.to_string()));
        }
    };
    match capabilities::require_connected(&store, &root, Capability::BoardAzure) {
        Ok(()) => Ok(()),
        Err(capabilities::GateError::Missing(err)) => {
            // Mensagem de capacidade, nunca 401/404 cru (ADR-0030 #5).
            print_error(&err);
            Err(Outcome::failure(err.to_string()))
        }
        Err(capabilities::GateError::Store(err)) => {
            eprintln!("erro: {err}");
            Err(Outcome::failure(err.to_string()))
        }
    }
}

/// `kira connect <provider>` (ADR-0030 #4): a credencial nasce aqui. `azure`
/// é a implementação do MVP-3 (aula + menu + validação + keyring + capacidade
/// por projeto); `github`/`gitlab` seguem aceitos mas "ainda não
/// implementado" (contrato de teste do MVP-1 preservado), e provider
/// desconhecido lista os válidos. Exit non-zero nos casos sem conexão real.
async fn connect_command(provider: &str, yes: bool) -> Outcome {
    let message = match provider {
        "github" | "gitlab" => format!(
            "conecte com: kira connect {provider} (ainda não implementado — fica para um MVP posterior)"
        ),
        other => format!("provider '{other}' desconhecido — use: kira connect azure|github|gitlab"),
    };
    if provider != "azure" {
        println!("{message}");
        return Outcome {
            code: ExitCode::FAILURE,
            ok: false,
            records: false,
            summary: message,
        };
    }
    connect_azure_command(yes).await
}

/// `kira connect azure` (MVP-3): no TTY imprime a aula e oferece o menu
/// (1 colar / 2 env / 3 cancelar); fora de TTY (ou com `--yes`) usa SÓ a env
/// `AZURE_DEVOPS_PAT` — nunca fica pendurado esperando input. Sucesso marca
/// `board.azure` como conectada PARA O PROJETO ATUAL no store.
async fn connect_azure_command(yes: bool) -> Outcome {
    // A capacidade é POR PROJETO: sem raiz detectada não há onde marcar.
    let Some(root) = store::detect_project_root() else {
        let message =
            "não foi possível determinar o projeto atual — rode dentro da raiz do repositório"
                .to_string();
        eprintln!("erro: {message}");
        return Outcome::failure(message);
    };
    // Fail fast: sem onde guardar o segredo (ou sem store de capacidades) o
    // connect para ANTES do dev criar um token novo.
    let secret_store = match credentials::open_default_store() {
        Ok(store) => store,
        Err(err) => {
            print_error(&err);
            return Outcome::failure(err.to_string());
        }
    };
    let state_store = match Store::open_default() {
        Ok(store) => store,
        Err(err) => {
            print_error(&err);
            return Outcome::failure(err.to_string());
        }
    };
    // Interativo = stdin é TTY e o dev NÃO pediu --yes: `--yes` vence sempre,
    // como no setup — nunca bloqueia quem está em pipe/CI.
    let interactive = std::io::stdin().is_terminal() && !yes;
    let env_lookup: &dyn Fn(&str) -> Option<String> = &setup::env_lookup;
    let make_board: &setup::BoardFactory<'_> = &setup::default_board_factory;
    let config_path = credentials::config_path();
    let options = setup::ConnectAzureOptions {
        interactive,
        hidden_input: true,
        secret_store: secret_store.as_ref(),
        config_path: config_path.as_deref(),
        env: env_lookup,
        make_board,
        project_root: &root,
        state_store: &state_store,
    };
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut input = stdin.lock();
    let mut output = stdout.lock();
    let code = setup::connect_azure(&options, &mut input, &mut output).await;
    Outcome {
        code,
        ok: code == ExitCode::SUCCESS,
        records: false,
        summary: if code == ExitCode::SUCCESS {
            "connect azure: board.azure conectada (PAT no keyring)"
        } else {
            "connect azure: conexão não concluída — veja a saída"
        }
        .to_string(),
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

/// Item do board desta execução, quando a jornada é de item.
///
/// Hoje só o `triage` precisa: o `pr draft <id>` recupera o triage pela trilha
/// e, sem o id gravado, casava a string dos argumentos — o que falhava para
/// `triage --file item.json` (issue #78). `--eval` não é jornada de item.
///
/// Fail-open de propósito: arquivo ilegível devolve `None` e a trilha segue; a
/// falta do id degrada o rascunho (que mostra `—`), nunca a triagem em si.
fn command_item_id(command: &Command) -> Option<i32> {
    match command {
        Command::Triage { id, file, eval, .. } => {
            if eval.is_some() {
                return None;
            }
            match (id, file) {
                (Some(id), _) => Some(*id),
                (None, Some(path)) => item_id_from_file(path),
                (None, None) => None,
            }
        }
        _ => None,
    }
}

/// Lê só o `id` do JSON do item (sem desserializar o resto e sem tocar rede).
fn item_id_from_file(path: &std::path::Path) -> Option<i32> {
    let texto = std::fs::read_to_string(path).ok()?;
    let documento: serde_json::Value = serde_json::from_str(&texto).ok()?;
    let id = documento.get("id")?;
    // A API do Azure DevOps manda número; `as_i64` cobre também "4821" quando
    // o JSON vier com string, e o `try_from` recusa qualquer coisa fora de i32.
    let valor = id.as_i64().or_else(|| id.as_str()?.parse().ok())?;
    i32::try_from(valor).ok()
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
        Command::Triage {
            id,
            file,
            model,
            llm,
            eval,
            json,
            seed,
        } => {
            // O `--eval` é outra origem (uma pasta de gold), exclusiva do id
            // e do `--file`; a trilha registra exatamente como foi chamado.
            let mut args = String::new();
            if let Some(eval) = eval {
                args.push_str(&format!("--eval {}", eval.display()));
                if *seed {
                    args.push_str(" --seed");
                }
                if *json {
                    args.push_str(" --json");
                }
            } else {
                match (id, file) {
                    (Some(id), _) => args = id.to_string(),
                    (None, Some(file)) => args = format!("--file {}", file.display()),
                    (None, None) => {}
                }
                if let Some(llm) = llm {
                    // O valor já é validado pelo value_parser do clap
                    // (`ollama|copilot|foundry`) — a trilha registra exatamente o
                    // backend usado nesta execução.
                    args.push_str(&format!(" --llm {llm}"));
                }
            }
            (
                "triage".to_string(),
                args,
                Some(model.clone().unwrap_or_else(ollama::model_from_env)),
            )
        }
        Command::Setup { .. } => ("setup".to_string(), String::new(), None),
        Command::Connect { provider, .. } => ("connect".to_string(), provider.clone(), None),
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
        Command::Ingest {
            path,
            json,
            skip_scan,
        } => {
            let mut args = String::new();
            if let Some(path) = path {
                args.push_str(&format!("--path {}", path.display()));
            }
            if *json {
                args.push_str(" --json");
            }
            if *skip_scan {
                args.push_str(" --skip-scan");
            }
            ("ingest".to_string(), args.trim().to_string(), None)
        }
        Command::Trust { path, revoke } => {
            let mut args = String::new();
            if let Some(path) = path {
                args.push_str(&format!("--path {}", path.display()));
            }
            if *revoke {
                args.push_str(" --revoke");
            }
            ("trust".to_string(), args.trim().to_string(), None)
        }
        Command::Digest {
            path,
            model,
            lang,
            budget,
            pause,
            resume,
            json,
        } => {
            let mut args = String::new();
            if let Some(path) = path {
                args.push_str(&format!("--path {}", path.display()));
            }
            if let Some(model) = model {
                args.push_str(&format!(" --model {model}"));
            }
            if let Some(lang) = lang {
                args.push_str(&format!(" --lang {lang}"));
            }
            if let Some(budget) = budget {
                args.push_str(&format!(" --budget {budget}"));
            }
            if let Some(pause) = pause {
                args.push_str(&format!(" --pause {pause}"));
            }
            if *resume {
                args.push_str(" --resume");
            }
            if *json {
                args.push_str(" --json");
            }
            ("digest".to_string(), args.trim().to_string(), model.clone())
        }
        Command::Emit { path, force, json } => {
            let mut args = String::new();
            if let Some(path) = path {
                args.push_str(&format!("--path {}", path.display()));
            }
            if *force {
                args.push_str(" --force");
            }
            if *json {
                args.push_str(" --json");
            }
            ("emit".to_string(), args.trim().to_string(), None)
        }
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
        Command::Context {
            id,
            file,
            explain,
            json,
        } => {
            let mut args = String::new();
            if let Some(id) = id {
                args.push_str(&id.to_string());
            }
            if let Some(file) = file {
                if !args.is_empty() {
                    args.push(' ');
                }
                args.push_str(&format!("--file {}", file.display()));
            }
            if *explain {
                args.push_str(" --explain");
            }
            if *json {
                args.push_str(" --json");
            }
            ("context".to_string(), args, None)
        }
        Command::Dashboard {
            snapshot,
            read_only,
        } => {
            let mut args = String::new();
            if *snapshot {
                args.push_str("--snapshot");
            }
            if *read_only {
                if !args.is_empty() {
                    args.push(' ');
                }
                args.push_str("--read-only");
            }
            ("dashboard".to_string(), args, None)
        }
        Command::Pr { action } => match action {
            PrAction::Draft {
                id,
                file,
                out,
                json,
            } => {
                let mut args = format!("draft {id}");
                if let Some(file) = file {
                    args.push_str(&format!(" --file {}", file.display()));
                }
                if let Some(out) = out {
                    args.push_str(&format!(" --out {}", out.display()));
                }
                if *json {
                    args.push_str(" --json");
                }
                ("pr".to_string(), args, None)
            }
        },
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
    /// Item da jornada, quando houver (issue #78) — gravado na trilha.
    item_id: Option<i32>,
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
            run.item_id,
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

/// `kira ingest` — fatos do projeto, sem LLM (ADR-0038, M1).
///
/// Determinístico: mesma árvore ⇒ mesmo `.kira/digest.md`. Rede só se `--skip-scan`
/// não for passado, e mesmo assim a falha do scan vira nota, nunca erro.
fn ingest_command(path: Option<&Path>, json: bool, skip_scan: bool) -> Outcome {
    let root = path.map_or_else(
        || store::detect_project_root().unwrap_or_else(|| PathBuf::from(".")),
        Path::to_path_buf,
    );
    let mut options = ingest::IngestOptions::at(root);
    options.run_scan = !skip_scan;
    match ingest::run(&options) {
        Ok(report) => {
            ingest::ensure_local_marker(&options.root);
            if json {
                match serde_json::to_string_pretty(&report) {
                    Ok(text) => {
                        println!("{text}");
                        Outcome::success(text)
                    }
                    Err(err) => {
                        print_error(&err);
                        Outcome::failure(err.to_string())
                    }
                }
            } else {
                let text = report.summary();
                print!("{text}");
                Outcome::success(text)
            }
        }
        Err(err) => {
            print_error(&err);
            Outcome::failure(err.to_string())
        }
    }
}

/// `kira trust` — aceite do repositório (ADR-0038, M2).
///
/// O `ingest` é read-only e não pede aceite; o `digest`, que envia código ao modelo,
/// recusa rodar sem ele. Revogar é tão fácil quanto aceitar, e nenhum dos dois apaga
/// artefato já gravado.
fn trust_command(path: Option<&Path>, revoke: bool) -> Outcome {
    let root = path.map_or_else(
        || store::detect_project_root().unwrap_or_else(|| PathBuf::from(".")),
        Path::to_path_buf,
    );
    if revoke {
        return match trust::revoke(&root) {
            Ok(true) => {
                let text = format!("confiança revogada: {}\n", root.display());
                print!("{text}");
                Outcome::success(text)
            }
            Ok(false) => {
                let text = format!("nada a revogar em {}\n", root.display());
                print!("{text}");
                Outcome::success(text)
            }
            Err(err) => {
                print_error(&err);
                Outcome::failure(err.to_string())
            }
        };
    }
    match trust::accept(&root) {
        Ok(record) => {
            let text = format!(
                "repositório aceito: {}\n  hash: {}\n",
                record.root, record.hash
            );
            print!("{text}");
            Outcome::success(text)
        }
        Err(err) => {
            print_error(&err);
            Outcome::failure(err.to_string())
        }
    }
}

/// Idioma salvo pelo `kira setup` (ADR-0038 D4). Qualquer falha de leitura =
/// `None` — o digest cai no default `en`, nunca trava por config.
fn output_lang_from_config() -> Option<String> {
    credentials::read_config()
        .ok()?
        .into_iter()
        .find(|(name, _)| name == credentials::CONFIG_KEY_OUTPUT_LANG)
        .map(|(_, value)| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// `kira digest` — o `Courier` local (ADR-0038, M3).
///
/// O backend é construído **aqui** (composition root) e o módulo de decisão recebe
/// a referência: `post` não escolhe provider, só decide com o modelo.
#[allow(clippy::too_many_arguments)]
async fn digest_command(
    path: Option<&Path>,
    model: Option<String>,
    lang: Option<String>,
    budget: Option<u64>,
    pause: Option<usize>,
    resume: bool,
    json: bool,
) -> Outcome {
    let root = path.map_or_else(
        || store::detect_project_root().unwrap_or_else(|| PathBuf::from(".")),
        Path::to_path_buf,
    );
    let model_tag = model.unwrap_or_else(ollama::model_from_env);
    // ADR-0038 D4: flag > env > `output_lang` salvo pelo setup > en.
    let lang = lang
        .or_else(|| std::env::var("KIRA_LANG").ok())
        .or_else(output_lang_from_config)
        .unwrap_or_else(|| "en".to_string());
    let mut options = kira::post::DigestOptions::at(root);
    options.model = format!("ollama:{model_tag}");
    options.lang = lang;
    if let Some(budget) = budget {
        options.budget_tokens = budget;
    }
    options.pause_after = pause;
    options.resume = resume;

    let backend = ollama::OllamaBackend::new(ollama::base_url_from_env(), model_tag);
    match kira::post::run(&options, &backend).await {
        Ok(report) => {
            if json {
                match serde_json::to_string_pretty(&report_json(&report)) {
                    Ok(text) => {
                        println!("{text}");
                        Outcome::success(text)
                    }
                    Err(err) => {
                        print_error(&err);
                        Outcome::failure(err.to_string())
                    }
                }
            } else {
                let text = report.summary();
                print!("{text}");
                Outcome::success(text)
            }
        }
        Err(err) => {
            print_error(&err);
            Outcome::failure(err.to_string())
        }
    }
}

fn report_json(report: &kira::post::DigestReport) -> serde_json::Value {
    serde_json::json!({
        "planned": report.planned,
        "generated": report.generated,
        "cached": report.cached,
        "failed": report.failed,
        "tokens_used": report.tokens_used,
        "budget_tokens": report.budget_tokens,
        "budget_hit": report.budget_hit,
        "briefing_path": report.briefing_path,
        "notes": report.notes,
    })
}

/// `kira emit` — publica os rascunhos no wiki, gate por hash (ADR-0038, M5).
///
/// A decisão de sobrescrever é do dono: `--force` explícito. Sem ele, post editado
/// à mão é recusado com o nome do arquivo na frente.
fn emit_command(path: Option<&Path>, force: bool, json: bool) -> Outcome {
    let root = path.map_or_else(
        || store::detect_project_root().unwrap_or_else(|| PathBuf::from(".")),
        Path::to_path_buf,
    );
    let mut options = emit::EmitOptions::at(root);
    options.force = force;
    match emit::run(&options) {
        Ok(report) => {
            if json {
                let value = serde_json::json!({
                    "published": report.published,
                    "unchanged": report.unchanged,
                    "edited": report.edited,
                    "missing_drafts": report.missing_drafts,
                    "index_path": report.index_path,
                    "notes": report.notes,
                });
                match serde_json::to_string_pretty(&value) {
                    Ok(text) => {
                        println!("{text}");
                        Outcome::success(text)
                    }
                    Err(err) => {
                        print_error(&err);
                        Outcome::failure(err.to_string())
                    }
                }
            } else {
                let text = report.summary();
                print!("{text}");
                Outcome::success(text)
            }
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
                "wiki de '{}' ({} páginas, em .kira/wiki):\n",
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
                "página '{page}' apagada de .kira/wiki{}\n",
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
                "{} página(s) promovida(s) para .kira/wiki:\n",
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

// ---------------------------------------------------------------------------
// `kira context` — the context pack without the LLM (ADR-0021)
// ---------------------------------------------------------------------------

/// `kira context <id|--file> [--explain] [--json]` — prints the pack that
/// would reach the triage prompt: source (index/grep), sections, every excerpt
/// as `path:linha — trecho truncado`, and the `contexto: ...` summary. 100%
/// local: no model, no network (ADR-0021). Does not write a session row.
fn context_command(id: Option<i32>, file: Option<PathBuf>, explain: bool, json: bool) -> Outcome {
    let source = match context_source(id, file) {
        Ok(source) => source,
        Err(message) => {
            eprintln!("erro: {message}");
            eprintln!("uso: kira context --file <work-item.json> [--explain] [--json]");
            return Outcome::failure(message);
        }
    };
    match context_output(&source, explain, json) {
        Ok(text) => {
            print!("{text}");
            Outcome {
                code: ExitCode::SUCCESS,
                ok: true,
                records: false,
                summary: first_line(&text, HISTORY_SUMMARY_LINE_CHARS),
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

/// Origin of the work item for `kira context`: only the offline file path
/// exists — the live id would need the network and this command never touches
/// it (ADR-0021).
fn context_source(id: Option<i32>, file: Option<PathBuf>) -> Result<PathBuf, String> {
    match (id, file) {
        (None, Some(path)) => Ok(path),
        (Some(id), None) => Err(format!(
            "o work item #{id} ao vivo exigiria rede e `kira context` é 100% local (sem LLM, sem rede) — salve o JSON do item e rode `kira context --file <caminho.json>`"
        )),
        (Some(_), Some(_)) => Err(
            "informe apenas uma origem: o id do work item OU --file <caminho.json>".to_string(),
        ),
        (None, None) => Err(
            "informe a origem do work item: `kira context --file <caminho.json>` (offline; o id ao vivo exige rede e este comando não a toca)"
                .to_string(),
        ),
    }
}

/// Body of `kira context`: detect the project root, parse the work item file,
/// refresh the index (inside `pack_excluding`, fail-open) and render the report.
fn context_output(file: &Path, explain: bool, json: bool) -> Result<String, Error> {
    let root = store::detect_project_root().unwrap_or_else(|| PathBuf::from("."));
    let body = std::fs::read_to_string(file).map_err(|source| Error::Io {
        path: file.display().to_string(),
        source,
    })?;
    let item = kira::work_item::WorkItem::from_api_json(&body)?;
    let pack = kira::context::pack_excluding(&root, &item, Some(file));
    if json {
        return kira::context::report_json(&pack, item.id).map_err(Error::ContextReportJson);
    }
    Ok(kira::context::explain_report(&pack, item.id, explain))
}

// ---------------------------------------------------------------------------
// `kira dashboard` — casca TUI (ADR-0014, ADR-0033)
// ---------------------------------------------------------------------------

/// `kira dashboard [--snapshot] [--read-only]` — o dashboard é o MESMO binário
/// em modo interativo (nunca daemon): com stdout TTY abre a TUI
/// (ratatui/crossterm); sem TTY ou com `--snapshot` imprime o mesmo conteúdo em
/// texto e sai 0 — é assim que o CI testa. Navegar nunca executa; `--read-only`
/// desliga toda ação (ADR-0033). NUNCA grava sessão na trilha por conta própria
/// (o e2e prova isso com o hash do `store.db`) e nunca toca rede/LLM.
fn dashboard_command(snapshot: bool, read_only: bool) -> Outcome {
    let model = kira::tui::state::load();
    if snapshot || !std::io::stdout().is_terminal() {
        // Fora de TTY o desenho é impossível (raw mode/alternate screen não
        // existem em pipe): a forma de texto é a mesma informação, exit 0.
        let text = kira::tui::state::text_report(&model);
        print!("{text}");
        Outcome {
            code: ExitCode::SUCCESS,
            ok: true,
            // Read-only TOTAL: nem a trilha automática pode tocar o store
            // aqui — é o que o teste de hash do `store.db` verifica.
            records: false,
            summary: text,
        }
    } else {
        let options = kira::tui::app::RunOptions { read_only };
        match kira::tui::app::run(model, options) {
            Ok(()) => Outcome {
                code: ExitCode::SUCCESS,
                ok: true,
                records: false,
                summary: "dashboard fechado pelo usuário".to_string(),
            },
            Err(message) => {
                eprintln!("erro: {message}");
                Outcome {
                    code: ExitCode::FAILURE,
                    ok: false,
                    records: false,
                    summary: message,
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// `kira pr draft` — rascunho local, sem rede e sem LLM (W4)
// ---------------------------------------------------------------------------

/// `kira pr draft <id> [--file item.json] [--out caminho.md] [--json]`.
///
/// O rascunho é montado só com o que o KIRA já sabe (triage no store,
/// findings.json, wiki, item em `--file`): NÃO faz push, NÃO abre PR e NÃO
/// chama LLM. Read-only total: nem grava sessão na trilha (como
/// `context`/`dashboard`). Falha de leitura do `--file`/`--out` é erro;
/// store/findings/wiki ausentes viram `—` com causa e o comando sai 0.
fn pr_command(action: PrAction) -> Outcome {
    let PrAction::Draft {
        id,
        file,
        out,
        json,
    } = action;
    match kira::prdraft::run(&kira::prdraft::DraftOptions {
        item_id: id,
        file,
        out,
        json,
    }) {
        Ok(text) => {
            print!("{text}");
            Outcome {
                code: ExitCode::SUCCESS,
                ok: true,
                records: false,
                summary: first_line(&text, HISTORY_SUMMARY_LINE_CHARS),
            }
        }
        Err(err) => {
            print_error(&err);
            Outcome::failure(err.to_string())
        }
    }
}

/// Resolves org/project/PAT (flags > env > keyring/config.toml) and builds the
/// board provider selected by `KIRA_BOARD` (default `azure`) — pela MESMA
/// fábrica do setup, que também respeita o override de endpoint de teste
/// (`KIRA_ADO_URL`): o que o `connect` validar é o que o `triage` usa.
fn resolve_board(project_override: Option<&str>) -> Result<Box<dyn BoardProvider>, Error> {
    let config = credentials::resolve_azure_config(project_override)?;
    setup::default_board_factory(&config.pat, &config.organization, &config.project)
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
    llm_flag: Option<String>,
    eval: Option<PathBuf>,
    json: bool,
    seed: bool,
) -> Outcome {
    // W2-bis (ADR-0018): o `--eval` mede o gate M2 e é exclusivo das demais
    // origens — um erro de composição aqui é acionável, nunca implícito.
    if eval.is_some() || seed {
        if id.is_some() || file.is_some() {
            let message =
                "informe apenas uma origem: o id OU --file OU --eval <pasta-de-gold>".to_string();
            eprintln!("erro: {message}");
            return Outcome::failure(message);
        }
        return eval_command(eval, json, seed, model_flag, llm_flag).await;
    }
    if json {
        let message = "--json só faz sentido com --eval (a medição do gate M2)".to_string();
        eprintln!("erro: {message}");
        return Outcome::failure(message);
    }
    let source = match triage_source(id, file) {
        Ok(source) => source,
        Err(message) => {
            eprintln!("erro: {message}");
            eprintln!("uso: kira triage <work-item-id>  |  kira triage --file <caminho.json>");
            return Outcome::failure(message);
        }
    };
    // ADR-0030 #5/#6: só o caminho VIVO (id) exige `board.azure` conectada. A
    // prova vem ANTES de resolver credencial ou de qualquer rede — falhar aqui
    // significa zero tentativa de rede (nada de reqwest, nada de PAT). O
    // caminho offline (`--file`) fica intacto: não exige capacidade nenhuma.
    if matches!(source, TriageSource::Id(_))
        && let Err(blocked) = gate_board()
    {
        return blocked;
    }
    // ADR-0019/ADR-0020: the backend choice is explicit and validated — a
    // costly backend missing (CLI absent, key unset) FAILS LOUDLY, never a
    // silent Ollama fallback.
    let kind = match kira::llm::LlmKind::from_flag_and_env(llm_flag.as_deref()) {
        Ok(kind) => kind,
        Err(err) => {
            print_error(&err);
            return Outcome::failure(err.to_string());
        }
    };
    let base_url = ollama::base_url_from_env();
    let model = model_flag.unwrap_or_else(ollama::model_from_env);
    let result = match source {
        TriageSource::Id(id) => match resolve_board(None) {
            Ok(board) => triage::triage_live_llm(kind, board.as_ref(), id, &base_url, &model).await,
            Err(err) => Err(err),
        },
        TriageSource::File(path) => triage::triage_file_llm(kind, &base_url, &model, &path).await,
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
            // output or the exit code. A capture failure also prefixes the
            // session-trail summary (W2-fix PR-F3) so `kira history` shows it.
            let (episode_note, episode_failed) = record_triage_episode(&report);
            if let Some(note) = episode_note {
                eprint!("{note}");
            }
            if episode_failed {
                Outcome::success(format!(
                    "wikipage: episódio failed (triage sem conteúdo nas 3 seções)\n{}",
                    report.output
                ))
            } else {
                Outcome::success(report.output)
            }
        }
        Err(err) => {
            print_error(&err);
            Outcome::failure(err.to_string())
        }
    }
}

/// W2-bis (ADR-0018): mede o gate M2 com `kira triage --eval <dir>`.
///
/// O backend é o mesmo contrato do `triage` (explicit choice, fail loud —
/// não se mede com dado falso). O exit code é o próprio gate: 0 quando
/// ≥ 70% dos itens acertam, 1 quando < 70% ou quando algo falha.
async fn eval_command(
    eval: Option<PathBuf>,
    json: bool,
    seed: bool,
    model_flag: Option<String>,
    llm_flag: Option<String>,
) -> Outcome {
    let Some(dir) = eval else {
        let message = "--seed precisa de --eval <pasta-de-gold>".to_string();
        eprintln!("erro: {message}");
        return Outcome::failure(message);
    };
    let kind = match kira::llm::LlmKind::from_flag_and_env(llm_flag.as_deref()) {
        Ok(kind) => kind,
        Err(err) => {
            print_error(&err);
            return Outcome::failure(err.to_string());
        }
    };
    let base_url = ollama::base_url_from_env();
    let model = model_flag.unwrap_or_else(ollama::model_from_env);
    // `--seed` copia as 3 fixtures sintéticas num diretório vazio e segue para
    // a medição: o dono vê a mecânica do gate antes de rotular os 20 reais.
    if seed {
        if let Err(err) = kira::eval::seed_dir(&dir) {
            print_error(&err);
            return Outcome::failure(err.to_string());
        }
        eprintln!("seeds sintéticas copiadas para {}", dir.display());
    }
    match kira::eval::run(&dir, kind, &base_url, &model).await {
        Ok((itens, gate)) => {
            if json {
                let report = kira::eval::report(&itens, &gate);
                match serde_json::to_string_pretty(&report) {
                    Ok(corpo) => print!("{corpo}"),
                    Err(err) => {
                        let failure = Error::Eval(format!(
                            "não foi possível serializar o relatório --json: {err}"
                        ));
                        eprintln!("erro: {failure}");
                        return Outcome::failure(failure.to_string());
                    }
                }
            } else {
                print!("{}", kira::eval::render_table(&itens, &gate));
            }
            // No modo JSON a linha do gate vai ao stderr (o veredito humano
            // não pode se perder no JSON); na tabela ela já está no stdout.
            let linha = kira::eval::gate_line(&gate);
            if json {
                eprintln!("{linha}");
            }
            Outcome {
                code: exit_code(gate.passou),
                ok: gate.passou,
                records: true,
                summary: format!("triage --eval: {linha}"),
            }
        }
        Err(err) => {
            print_error(&err);
            Outcome::failure(err.to_string())
        }
    }
}

/// Episodic capture after a triage: a wiki problem is one `aviso:` line on
/// stderr, never a failed command (same fail-open contract as the session
/// trail). Returns the stderr note (if any) plus whether the episode failed to
/// capture for *content* reasons — an empty triage writes nothing to the wiki
/// and only shows up on the trail (W2-fix PR-F3).
fn record_triage_episode(report: &triage::TriageReport) -> (Option<String>, bool) {
    match record_triage_episode_inner(report) {
        EpisodeCapture::Recorded(path) => (
            Some(format!(
                "wiki: episódio do item {} registrado em {}\n",
                report.item_id,
                store::display_path(&path)
            )),
            false,
        ),
        EpisodeCapture::Skipped => (None, false),
        EpisodeCapture::Empty => (
            Some(format!(
                "aviso: wiki não registrou o episódio do item {} (triage sem conteúdo — falha registrada na trilha)\n",
                report.item_id
            )),
            true,
        ),
        EpisodeCapture::Failed(cause) => (
            Some(format!("aviso: wiki não registrou o episódio ({cause})\n")),
            false,
        ),
    }
}

/// What the episodic capture did (W2-fix PR-F3).
enum EpisodeCapture {
    /// No project root detected — nothing to do, silent.
    Skipped,
    /// Episode written to the wiki page (path feeds the success note).
    Recorded(PathBuf),
    /// All three triage sections empty (or `—`): the wiki is NOT written; the
    /// failure lives only on the session trail.
    Empty,
    /// Wiki failure: fail-open, one `aviso:` line on stderr.
    Failed(String),
}

/// Builds the episode from what the command already produced and writes it to
/// the main wiki — a human ran `kira triage` (ADR-0013). An unattended
/// cycle must use [`episode::EpisodeTarget::Staging`] instead (PR15).
fn record_triage_episode_inner(report: &triage::TriageReport) -> EpisodeCapture {
    let Some(root) = store::detect_project_root() else {
        return EpisodeCapture::Skipped;
    };
    let wiki = match Wiki::open(&root) {
        Ok(wiki) => wiki,
        Err(err) => return EpisodeCapture::Failed(err.to_string()),
    };
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
    // W2-fix PR-F3 (bug 529511): an episode whose three sections carry nothing
    // is not knowledge — writing it would plant an empty page inside the
    // client's project. The failure goes to the trail only.
    if episode.is_empty() {
        return EpisodeCapture::Empty;
    }
    match episode::record(
        &wiki,
        &episode,
        episode::EpisodeTarget::Wiki,
        &store::now_rfc3339_utc(),
    ) {
        Ok(path) => EpisodeCapture::Recorded(path),
        Err(err) => EpisodeCapture::Failed(err.to_string()),
    }
}

#[allow(clippy::too_many_arguments)]
async fn setup_command(
    org: Option<String>,
    project: Option<String>,
    reset: bool,
    pat: Option<String>,
    yes: bool,
    backend: Option<String>,
    skip: Vec<String>,
    name: Option<String>,
    lang: Option<String>,
    plain: bool,
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
    // Modo interativo = stdin é um TTY: só nele o nome é perguntado e o menu
    // de IA aparece; `--yes` força o modo não-interativo (nunca bloqueia;
    // sem Ollama escolhe "nenhuma" e imprime o pulado).
    let interactive = std::io::stdin().is_terminal();
    // M6: o binário SEMPRE decide a ingestão (o wizard refaz a coleta na
    // etapa própria); `--skip ingest` é o "não" explícito. O motor executa.
    let skip_ingest = skip
        .iter()
        .any(|item| item.trim().eq_ignore_ascii_case("ingest"));
    let options = setup::SetupOptions {
        org,
        project,
        reset,
        pat,
        interactive,
        name,
        lang,
        ingest: Some(!skip_ingest),
        yes,
        backend,
        skip,
        // Raiz do projeto DETECTADA (mesma regra do store): entra no passo
        // "projeto" do fluxo local-first.
        project_root: store::detect_project_root().map(|root| store::display_path(&root)),
        // O wizard de tela cheia entrega aqui o Foundry digitado no
        // formulário inline (ADR-0035); em modo texto vem `None`.
        foundry: None,
        // Entrada de segredo com eco escondido onde existe mecanismo; no
        // wizard o raw mode já desliga o eco nos três SOs.
        hidden_input: true,
    };

    // Wizard só em TTY grande e SEM flag explícita: flags vencem sempre (quem
    // passou `--pat` não quer um wizard) e `--reset` é ação única.
    let explicit = options.reset
        || options.org.is_some()
        || options.project.is_some()
        || options.pat.is_some()
        || options.backend.is_some()
        || options.lang.is_some()
        || !options.skip.is_empty()
        || options.yes;
    let stdout_tty = std::io::stdout().is_terminal();
    let (width, height) = crossterm::terminal::size().unwrap_or((0, 0));
    let options = if setup::wizard::mode(interactive && stdout_tty, width, height, plain, explicit)
        == setup::wizard::Mode::Wizard
    {
        match setup::wizard::run(&options, &setup::env_lookup) {
            Some(collected) => collected,
            None => {
                println!("setup cancelado — nada foi alterado");
                return ExitCode::SUCCESS;
            }
        }
    } else {
        options
    };

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut input = stdin.lock();
    let mut output = BufWriter::new(stdout.lock());
    setup::run(
        &options,
        store.as_ref(),
        credentials::config_path().as_deref(),
        &setup::env_lookup,
        &mut input,
        &mut output,
        &setup::default_board_factory,
        &setup::real_ia_hooks(),
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
