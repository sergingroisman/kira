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
use kira::error::Error;
use kira::format;
use kira::ingest;
use kira::llm::ollama;
use kira::publish;
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
        /// Skip the wiki reasoning cache (PR35): always run the LLM route. A
        /// cache hit is deterministic (similarity >= 0.50 against the known
        /// cause in `.kira/wiki/bugs/`) and spends no LLM call — this flag
        /// trades the economy for a fresh model answer
        #[arg(long)]
        no_cache: bool,
        /// Token profile of the pack (PR40): `budget` | `balanced` | `quality`.
        /// Overrides `token_profile` in config.toml; default `balanced` (= the
        /// pre-PR40 caps). An invalid value fails with the actionable list —
        /// never a silent fallback
        #[arg(long)]
        profile: Option<String>,
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
        after_help = "Exemplos:\n  kira setup                # nome → projeto (raiz detectada) → backend de IA → resumo\n  kira setup --yes          # não-interativo: nunca bloqueia; sem Ollama escolhe \"nenhuma\" (e imprime o pulado)\n  kira setup --org minha-org --project MeuProjeto --pat <token>   # atalho: valida e salva sem perguntas\n  kira setup --skip ia      # pula um passo (repetível: ia; 'pat' segue aceito como no-op)\n  kira setup --reset        # remove o PAT e a chave do Foundry do keyring; org/projeto/dev ficam\n\n  # A credencial do board nasce em `kira connect azure` (MVP-3), não aqui."
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
        /// Pula um passo (repetível): `ia` (`pat` segue aceito como no-op —
        /// o passo deixou de existir no fluxo local-first)
        #[arg(long)]
        skip: Vec<String>,
        /// How the dev wants to be called (saved as `dev_name` in config.toml;
        /// shown by `kira doctor` — never sent to any LLM prompt)
        #[arg(long)]
        name: Option<String>,
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
    /// .kira/facts.md — no AI, no network required (ADR-0038)
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
    /// .kira/drafts/. Requires `kira trust` and a previous `kira ingest`
    #[command(
        after_help = "Exemplos:\n  kira digest --lang pt-BR\n  kira digest --pause 1\n  kira digest --resume --budget 200000"
    )]
    Digest {
        /// Project folder (defaults to the project root detected like in the store)
        #[arg(long)]
        path: Option<PathBuf>,
        /// Ollama model tag (overrides KIRA_MODEL); goes into the cache key.
        /// Explicit choice wins over the specialist routing (HF-5)
        #[arg(long)]
        model: Option<String>,
        /// Generate ONLY this post kind (intro, c4-container); the kind's
        /// mapped tag from model_role is used when no explicit model is given
        #[arg(long)]
        kind: Option<String>,
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
    /// Publish the digested drafts into the project wiki (.kira/wiki), gated by the
    /// source hash: a post edited by hand is never overwritten without --force
    #[command(after_help = "Exemplos:\n  kira publish\n  kira publish --force")]
    Publish {
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
        /// Token profile of the pack (PR40): `budget` | `balanced` | `quality`.
        /// Overrides `token_profile` in config.toml; default `balanced` (= the
        /// pre-PR40 caps). An invalid value fails with the actionable list —
        /// never a silent fallback
        #[arg(long)]
        profile: Option<String>,
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
    /// Catálogo de cartas de instrução (onda SK, SK-1): bundled no binário +
    /// `<projeto>/.kira/skills/*.md`; o índice FTS5 é derivado
    #[command(
        after_help = "Exemplos:\n  kira skills list                       # bundled + skills do projeto\n  kira skills show react                 # corpo do skill\n  kira skills add meu-skill.md           # valida, redige e copia para .kira/skills/\n  kira skills search testing             # top-k no índice (rode reindex antes)\n  kira skills reindex                    # reconstrói o índice derivado"
    )]
    Skills {
        #[command(subcommand)]
        action: SkillsAction,
        /// Raiz do projeto (default: detecta do diretório atual)
        #[arg(long)]
        path: Option<PathBuf>,
    },
    /// Catálogo local de modelos via Ollama (HF-2, ADR-0040: HF é catálogo
    /// de GGUF, nunca runtime — quem baixa e executa é o Ollama)
    #[command(
        after_help = "Exemplos:\n  kira model list                            # catálogo local (leitura, sempre livre)\n  kira model pull hf.co/<repo>:Q4_K_M        # baixa via ollama pull (pergunta em TTY)\n  kira model pull hf.co/<repo>:Q4_K_M --yes  # sem pergunta (não-interativo)\n  kira model admit hf.co/<repo>:Q4_K_M --sha256 <64-hex> --probes-ok\n                                             # admite no routing de especialistas\n\nPiso da casa: Q4_K_M — Q2_K é proibido por política (gate de admissão, ADR-0040)"
    )]
    Model {
        #[command(subcommand)]
        action: ModelAction,
    },
    /// Textos do dono em `<store_dir>/knowledge/*.md` (onda SK, SK-2): a
    /// fonte é o markdown no disco; o índice FTS5 no store é derivado
    #[command(
        after_help = "Exemplos:\n  kira knowledge list                # o que está indexado (fonte: *.md no disco)\n  kira knowledge search \"pbm\"        # top-k no índice (rode reindex antes)\n  kira knowledge reindex             # reconstrói o índice derivado do zero\n\nO diretório é seu: o KIRA nunca escreve nele — só lê os *.md"
    )]
    Knowledge {
        #[command(subcommand)]
        action: KnowledgeAction,
    },
    /// Agendamentos do ciclo (F2, PR16): declara a cadência de `scan`,
    /// `digest` ou `publish`; `kira loop --scheduled` executa só as fases
    /// vencidas. Sem daemon/serviço: o agendamento só roda com o KIRA vivo.
    #[command(
        after_help = "Exemplos:\n  kira schedule add --action digest --every 2h\n  kira schedule add --action scan --every 30m\n  kira schedule list\n  kira schedule rm 1\n\nO agendamento NÃO vira serviço: ele só roda quando `kira loop --scheduled` roda."
    )]
    Schedule {
        #[command(subcommand)]
        action: ScheduleAction,
    },
    /// Aprova (ou recusa) um pedido de escrita no mundo (F2, PR18). É a única
    /// porta do círculo de aprovação: sem `approved`, nada escreve no Azure,
    /// no repo do projeto ou no blog.
    #[command(
        after_help = "Exemplos:\n  kira approvals                 # o que espera decisão\n  kira approve 3                 # aprova o pedido 3\n  kira approve 3 --reject        # recusa (registra o motivo no estado)\n\nA aprovação vale para UM alvo (ação + alvo do pedido)."
    )]
    Approve {
        /// Id do pedido mostrado por `kira approvals`
        id: i64,
        /// Recusa o pedido em vez de aprovar
        #[arg(long)]
        reject: bool,
        /// Quem decidiu (default: `dono`; entra na trilha de auditoria)
        #[arg(long)]
        by: Option<String>,
    },
    /// Exame de rotina do ciclo (F2, PR19): sessões, fila de posts, aprovações
    /// e orçamento na janela — leitura pura do store, sem LLM e sem rede.
    #[command(
        after_help = "Exemplos:\n  kira audit                     # últimos 7 dias (padrão)\n  kira audit --days 30           # janela maior\n\nSó lê o store: nada é publicado (o blog saiu do escopo)."
    )]
    Audit {
        /// Janela em dias (default 7, o "weekly")
        #[arg(long)]
        days: Option<i64>,
    },
    /// Lista os pedidos de escrita no mundo (default: só os pendentes)
    Approvals {
        /// Mostra também os já decididos
        #[arg(long)]
        all: bool,
    },
    /// Wrapper do revisor externo rea (TE PR41): resolve o binário no PATH
    /// (ou KIRA_REA_BIN) ANTES do spawn e imprime a saída sempre REDIGIDA e
    /// capada em 8 KiB — output cru da ferramenta nunca chega ao terminal.
    /// Read-only: sem daemon, sem LLM, sem escrita no board/repo.
    #[command(
        after_help = "Exemplos:\n  kira rea --help               # o próprio rea ensina os argumentos\n  kira rea --diff HEAD~1        # revisa o diff do último commit (args passthrough)\n\nSem o binário instalado, o KIRA diz o que fazer (\"instale o rea\") sem tentar executar."
    )]
    Rea {
        /// Argumentos passthrough para o binário rea (flags com `-`/`--`
        /// incluídas — nada é interpretado pelo KIRA)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Ciclo local do KIRA (PR15): um tick = ingest → digest (se trusted) →
    /// publish. Tudo LOCAL — nunca escreve no mundo (Azure/git remoto); sem
    /// daemon: roda em primeiro plano e sai. `--once` = um tick (CI)
    #[command(
        after_help = "Exemplos:\n  kira loop --once                 # um tick e sai (CI/teste)\n  kira loop                        # ticks até estabilizar (nada a fazer)\n  kira loop --skip-digest --once   # ciclo determinístico, sem LLM\n  kira loop --ticks 4              # teto de ticks por execução"
    )]
    Loop {
        /// Roda UM tick e sai (nunca entra no modo "até estabilizar")
        #[arg(long)]
        once: bool,
        /// Teto de ticks por execução (default 16; só sem --once)
        #[arg(long)]
        ticks: Option<usize>,
        /// Raiz do projeto (default: detecta do diretório atual)
        #[arg(long)]
        path: Option<PathBuf>,
        /// Tag do modelo do digest (default: KIRA_MODEL/qwen3.5:4b)
        #[arg(long)]
        model: Option<String>,
        /// Idioma dos posts (default: KIRA_LANG ou en)
        #[arg(long)]
        lang: Option<String>,
        /// Pula o digest: ciclo 100% determinístico (ingest + publish)
        #[arg(long)]
        skip_digest: bool,
        /// O ingest não roda o scan dentro do ciclo
        #[arg(long)]
        skip_scan: bool,
        /// Roda só as fases com agendamento vencido (`kira schedule add`)
        #[arg(long)]
        scheduled: bool,
    },
}

/// `kira skills` actions (onda SK, SK-1).
#[derive(Debug, Subcommand)]
enum SkillsAction {
    /// Lista bundled + skills do usuário com origem e descrição
    List,
    /// Mostra o corpo de um skill (bundled ou do usuário)
    Show {
        /// Nome do skill (o `name` do frontmatter)
        name: String,
    },
    /// Valida o frontmatter, redige o corpo e copia para `.kira/skills/`
    Add {
        /// Arquivo `.md` do skill
        file: PathBuf,
    },
    /// Busca top-k no índice derivado (bundled + projeto atual)
    Search {
        /// Termos da busca (frase única citada no FTS5)
        query: String,
        /// Quantos resultados no máximo (default 5)
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Reconstrói o índice derivado `skill`/`skill_fts`
    Reindex,
}

/// `kira schedule` actions (F2, PR16).
#[derive(Debug, Subcommand)]
enum ScheduleAction {
    /// Declara a cadência de uma fase do ciclo
    Add {
        /// Fase agendada: `scan`, `digest` ou `publish`
        #[arg(long)]
        action: String,
        /// Cadência legível: `30m`, `2h`, `1d` (mínimo 60s)
        #[arg(long)]
        every: String,
        /// Cria desligado (para ligar depois com `kira schedule enable`)
        #[arg(long)]
        disabled: bool,
    },
    /// Lista os agendamentos com o próximo tick DERIVADO (nunca gravado)
    List,
    /// Remove um agendamento pelo id
    Rm {
        /// Id mostrado por `kira schedule list`
        id: i64,
    },
    /// Liga um agendamento existente (preserva o histórico de execução)
    Enable {
        /// Id mostrado por `kira schedule list`
        id: i64,
    },
    /// Desliga um agendamento sem apagar a linha
    Disable {
        /// Id mostrado por `kira schedule list`
        id: i64,
    },
}

/// Ações do `kira knowledge` (onda SK, SK-2).
#[derive(Debug, Subcommand)]
enum KnowledgeAction {
    /// Lista os documentos indexados (fonte: `*.md` no disco do dono)
    List,
    /// Busca top-k no índice derivado
    Search {
        /// Termos da busca (frase única citada no FTS5)
        query: String,
        /// Quantos resultados no máximo (default 5)
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Reconstrói o índice derivado `knowledge`/`knowledge_fts` do zero
    Reindex,
}

/// Ações do `kira model` (HF-2).
#[derive(Debug, clap::Subcommand)]
enum ModelAction {
    /// Lista os modelos locais do Ollama (parse de `ollama list`; sem rede
    /// fora do localhost do servidor ollama)
    List,
    /// Baixa um modelo GGUF (ex.: `hf.co/<repo>:<quant>`) via `ollama pull`
    /// — só com consentimento: `--yes` OU prompt interativo em TTY com
    /// destino e espaço livre; sem um dos dois, NADA é baixado
    Pull {
        /// Modelo no formato do Ollama (`hf.co/<repo>:Q4_K_M` ou `qwen3.5:4b`)
        repo: String,
        /// Confirma o download sem perguntar (em TTY a flag vence o prompt)
        #[arg(long)]
        yes: bool,
    },
    /// Registra que a tag passou NO GATE DE ADMISSÃO (HF-5, ADR-0040) e entra
    /// no routing de especialistas: política de quant (piso Q4_K_M, Q2_K
    /// proibido), proveniência conhecida, SHA-256 conferido contra o model
    /// card e probes rodados no GGUF quantizado
    Admit {
        /// Modelo no formato do Ollama (`hf.co/<repo>:Q4_K_M` ou `qwen3.5:4b`)
        tag: String,
        /// SHA-256 conferido PELO DONO contra o model card do repositório
        #[arg(long)]
        sha256: Option<String>,
        /// Atesta que `security_probe` + `--eval` do kind rodaram NO GGUF
        /// quantizado que vai pro ar (medir no FP16 não prova o quant)
        #[arg(long)]
        probes_ok: bool,
        /// Re-admite conferindo um template que mudou (reconfira no model
        /// card antes — o drift pode ser substituição de modelo)
        #[arg(long)]
        force: bool,
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
    /// Ledger de economia (PR34) quando o comando construiu pack de contexto
    /// — hoje só o `triage`. `None` é a regra: os demais comandos passam
    /// `..none()` e o history exibe `—` com a causa.
    economy: Option<store::economy::EconomyRow>,
}

impl Outcome {
    fn success(summary: impl Into<String>) -> Self {
        Self {
            code: ExitCode::SUCCESS,
            ok: true,
            records: true,
            summary: summary.into(),
            economy: None,
        }
    }

    fn failure(summary: impl Into<String>) -> Self {
        Self {
            code: ExitCode::FAILURE,
            ok: false,
            records: true,
            summary: summary.into(),
            economy: None,
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
            // O ledger de economia viaja com o outcome (PR34): quem construiu
            // pack é que o tem — o trail só grava.
            economy: outcome.economy.clone(),
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
                economy: None,
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
            no_cache,
            profile,
        } => triage_command(id, file, model, llm, eval, json, seed, no_cache, profile).await,
        Command::Setup {
            org,
            project,
            reset,
            pat,
            yes,
            backend,
            skip,
            name,
            plain,
        } => Outcome {
            code: setup_command(org, project, reset, pat, yes, backend, skip, name, plain).await,
            ok: true,
            records: false,
            summary: String::new(),
            economy: None,
        },
        Command::History { last } => store_command(history_output(last)),
        Command::Note { text, remove } => store_command(note_output(text, remove)),
        Command::Profile => store_command(profile_output()),
        Command::Scan { path } => scan_command(path.as_deref()),
        Command::Rea { args } => rea_command(&args),
        Command::Schedule { action } => schedule_command(action),
        Command::Approve { id, reject, by } => approve_command(id, reject, by.as_deref()),
        Command::Approvals { all } => approvals_command(all),
        Command::Audit { days } => audit_command(days),
        Command::Ingest {
            path,
            json,
            skip_scan,
        } => ingest_command(path.as_deref(), json, skip_scan),
        Command::Trust { path, revoke } => trust_command(path.as_deref(), revoke),
        Command::Publish { path, force, json } => publish_command(path.as_deref(), force, json),
        Command::Digest {
            path,
            model,
            kind,
            lang,
            budget,
            pause,
            resume,
            json,
        } => {
            digest_command(
                path.as_deref(),
                model,
                kind,
                lang,
                budget,
                pause,
                resume,
                json,
            )
            .await
        }
        Command::Wiki { action, path } => wiki_command(action, path.as_deref()),
        Command::Context {
            id,
            file,
            explain,
            json,
            profile,
        } => context_command(id, file, explain, json, profile),
        Command::Dashboard {
            snapshot,
            read_only,
        } => dashboard_command(snapshot, read_only),
        Command::Pr { action } => pr_command(action),
        Command::Connect { provider, yes } => connect_command(&provider, yes).await,
        Command::Skills { action, path } => skills_command(action, path.as_deref()),
        Command::Knowledge { action } => knowledge_command(action),
        Command::Model { action } => match action {
            ModelAction::List => model_list_command(),
            ModelAction::Pull { repo, yes } => model_pull_command(repo.as_str(), yes),
            ModelAction::Admit {
                tag,
                sha256,
                probes_ok,
                force,
            } => model_admit_command(&tag, sha256.as_deref(), probes_ok, force),
        },
        Command::Loop {
            once,
            ticks,
            path,
            model,
            lang,
            skip_digest,
            skip_scan,
            scheduled,
        } => {
            loop_command(
                path.as_deref(),
                once,
                ticks,
                model,
                lang,
                skip_digest,
                skip_scan,
                scheduled,
            )
            .await
        }
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
            economy: None,
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
        economy: None,
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
                economy: None,
            }
        }
        Err(err) => {
            print_error(&err);
            Outcome {
                code: exit_code(false),
                ok: false,
                records: false,
                summary: err.to_string(),
                economy: None,
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
            no_cache,
            profile,
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
                if *no_cache {
                    // PR35: a trilha registra como a rota foi forçada — o
                    // `pr draft` e a auditoria sabem que o cache foi pulado.
                    args.push_str(" --no-cache");
                }
            }
            if let Some(profile) = profile {
                // PR40: o perfil entra na trilha — é identidade do cache e do
                // pack, e a auditoria precisa saber com qual se executou.
                args.push_str(&format!(" --profile {profile}"));
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
        Command::Rea { args } => {
            // Os args passthrough entram na trilha como foram passados: a
            // auditoria precisa saber o que o rea recebeu (e nada de segredo
            // passa por aqui — o KIRA não pede segredo ao rea).
            ("rea".to_string(), shellwords_join(args), None)
        }
        Command::Schedule { action } => {
            // A trilha registra a invocação (ação + id), nunca o conteúdo do store.
            ("schedule".to_string(), schedule_args_text(action), None)
        }
        Command::Approve { id, reject, by } => (
            "approve".to_string(),
            format!(
                "{id}{}{}",
                if *reject { " reject" } else { "" },
                by.as_deref()
                    .map(|who| format!(" by={who}"))
                    .unwrap_or_default()
            ),
            None,
        ),
        Command::Audit { days } => (
            "audit".to_string(),
            days.as_ref()
                .map(|d| format!("--days {d}"))
                .unwrap_or_default(),
            None,
        ),
        Command::Approvals { all } => (
            "approvals".to_string(),
            if *all {
                "all".to_string()
            } else {
                String::new()
            },
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
            kind,
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
            if let Some(kind) = kind {
                args.push_str(&format!(" --kind {kind}"));
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
        Command::Publish { path, force, json } => {
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
            ("publish".to_string(), args.trim().to_string(), None)
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
            profile,
        } => {
            let mut args = String::new();
            if let Some(profile) = profile {
                args.push_str(&format!(" --profile {profile}"));
            }
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
        Command::Skills { action, path } => {
            let args = match action {
                SkillsAction::List => "list".to_string(),
                SkillsAction::Show { name } => format!("show {name}"),
                SkillsAction::Add { file } => format!("add {}", file.display()),
                SkillsAction::Search { query, limit } => match limit {
                    Some(limit) => format!("search {query} --limit {limit}"),
                    None => format!("search {query}"),
                },
                SkillsAction::Reindex => "reindex".to_string(),
            };
            let args = match path {
                Some(path) => format!("{args} --path {}", path.display()),
                None => args,
            };
            ("skills".to_string(), args, None)
        }
        Command::Knowledge { action } => match action {
            KnowledgeAction::List => ("knowledge".to_string(), "list".to_string(), None),
            KnowledgeAction::Search { query, limit } => (
                "knowledge".to_string(),
                match limit {
                    Some(limit) => format!("search {query} --limit {limit}"),
                    None => format!("search {query}"),
                },
                None,
            ),
            KnowledgeAction::Reindex => ("knowledge".to_string(), "reindex".to_string(), None),
        },
        Command::Model { action } => match action {
            ModelAction::List => ("model".to_string(), "list".to_string(), None),
            ModelAction::Pull { repo, yes } => (
                "model".to_string(),
                format!("pull {repo}{}", if *yes { " --yes" } else { "" }),
                None,
            ),
            ModelAction::Admit {
                tag,
                sha256,
                probes_ok,
                force,
            } => {
                let mut args = format!("admit {tag}");
                if let Some(sha) = sha256 {
                    // O SHA vai à trilha com o valor MASCARADO (8 primeiros
                    // chars): hash conferido é dado de auditoria, não segredo —
                    // mas a regra dura da casa não ecoa valor longo de nada.
                    let prefix: String = sha.chars().take(8).collect();
                    args.push_str(&format!(" --sha256 {prefix}…"));
                }
                if *probes_ok {
                    args.push_str(" --probes-ok");
                }
                if *force {
                    args.push_str(" --force");
                }
                ("model".to_string(), args, None)
            }
        },
        Command::Loop {
            once,
            ticks,
            path,
            model,
            lang,
            skip_digest,
            skip_scan,
            scheduled,
        } => {
            let mut args = String::new();
            if *once {
                args.push_str("--once");
            }
            if let Some(ticks) = ticks {
                args.push_str(&format!(" --ticks {ticks}"));
            }
            if let Some(path) = path {
                args.push_str(&format!(" --path {}", path.display()));
            }
            if let Some(model) = model {
                args.push_str(&format!(" --model {model}"));
            }
            if let Some(lang) = lang {
                args.push_str(&format!(" --lang {lang}"));
            }
            if *skip_digest {
                args.push_str(" --skip-digest");
            }
            if *skip_scan {
                args.push_str(" --skip-scan");
            }
            if *scheduled {
                args.push_str(" --scheduled");
            }
            ("loop".to_string(), args.trim().to_string(), model.clone())
        }
    }
}

/// Junta os args passthrough do `rea` para a trilha (um espaço entre eles:
/// a trilha descreve a invocação, não re-parseia nada).
fn shellwords_join(args: &[String]) -> String {
    args.join(" ")
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
    /// Ledger de economia (PR34): pack vs teto, chamadas, rota. `None` para
    /// comandos que não constroem pack (inspect/doctor) — o history mostra
    /// `—` com a causa, nunca um zero inventado.
    economy: Option<store::economy::EconomyRow>,
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
    let session_id = store
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
        .map_err(|error| error.to_string())?;
    // O ledger entra NA MESMA trilha, mesmo store e mesmo fail-open: falha
    // aqui vira o `aviso:` de trilha do `record_session`, nunca erro do
    // comando (PR34 — a sessão existe, a economia é dado irmão).
    if let Some(economy) = &run.economy {
        store
            .record_economy(session_id, economy)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
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
    // Ledger de economia (PR34): busca em lote as linhas das sessões listadas —
    // sessão sem linha não é erro, é `—` com a causa (dado ausente nunca é
    // inventado, regra da casa).
    let ids: Vec<i64> = sessions.iter().map(|session| session.id).collect();
    let economies = store.economy_for_sessions(&ids)?;
    for session in &sessions {
        text.push_str(&format_session_line(session));
        match economies.get(&session.id) {
            Some(economy) => text.push_str(&format_economy_line(economy)),
            None => text.push_str(
                "   economia: — (sem registro nesta sessão — comando sem pack de contexto \
                 ou sessão anterior à v13)\n",
            ),
        }
    }
    Ok(text)
}

/// One `kira history` economy line (PR34): pack vs teto em chars, a estimativa
/// de tokens (chars/3), as chamadas de LLM e a rota. PR38 acrescenta a
/// economia do dedup contíguo quando ela existiu. Números do ledger, nunca
/// estimados na exibição.
fn format_economy_line(economy: &store::economy::EconomyRow) -> String {
    let idle = if economy.pack_idle {
        " · pack ocioso"
    } else {
        ""
    };
    // A categoria do PR38 só aparece quando mediu algo: linha antiga (v13) e
    // sessão sem repetição nenhuma não ganham rufo de zero.
    let dedup = if economy.dedup_chars_saved > 0 {
        format!(" · dedup −{} chars", economy.dedup_chars_saved)
    } else {
        String::new()
    };
    format!(
        "   economia: ~{} tok (pack {}/{} chars) · chamadas de LLM: {} · rota: {}{}{}\n",
        economy.tokens_est,
        economy.pack_chars,
        economy.budget_chars,
        economy.llm_calls,
        economy.route,
        dedup,
        idle
    )
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

/// `kira rea <args>` (TE PR41): wrapper read-only do revisor externo. A
/// sessão entra na trilha como o scan faz — rodar uma revisão é trabalho
/// medível; a trilha é local e fail-open (o erro nunca quebra o comando).
fn rea_command(args: &[String]) -> Outcome {
    match kira::rea::run(args) {
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

/// `kira schedule` (F2, PR16). Todos os caminhos imprimem texto legível — o
/// agendamento é configuração do dono, não saída de máquina.
fn schedule_command(action: ScheduleAction) -> Outcome {
    let result = match action {
        ScheduleAction::Add {
            action,
            every,
            disabled,
        } => schedule_add(&action, &every, disabled),
        ScheduleAction::List => schedule_list(),
        ScheduleAction::Rm { id } => schedule_remove(id),
        ScheduleAction::Enable { id } => schedule_set_enabled(id, true),
        ScheduleAction::Disable { id } => schedule_set_enabled(id, false),
    };
    match result {
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

/// Texto da invocação para a trilha de auditoria (nunca o conteúdo do store).
fn schedule_args_text(action: &ScheduleAction) -> String {
    match action {
        ScheduleAction::Add {
            action,
            every,
            disabled,
        } => format!(
            "add {action} {every}{}",
            if *disabled { " disabled" } else { "" }
        ),
        ScheduleAction::List => "list".to_string(),
        ScheduleAction::Rm { id } => format!("rm {id}"),
        ScheduleAction::Enable { id } => format!("enable {id}"),
        ScheduleAction::Disable { id } => format!("disable {id}"),
    }
}

fn schedule_store() -> Result<kira::store::Store, kira::error::Error> {
    Ok(kira::store::Store::open_default()?)
}

fn schedule_add(action: &str, every: &str, disabled: bool) -> Result<String, kira::error::Error> {
    use kira::scheduler::{self, ScheduleAction as Phase};
    let phase = Phase::parse(action)?;
    let seconds = scheduler::parse_every(every)?;
    let store = schedule_store()?;
    let id = store.add_schedule(phase.as_str(), seconds, !disabled)?;
    let mut out = format!(
        "agendamento {id} criado: {} a cada {}\n",
        phase.as_str(),
        scheduler::format_every(seconds)
    );
    out.push_str(&format!(
        "  estado: {}\n",
        if disabled { "desligado" } else { "ativo" }
    ));
    out.push_str(
        "  aviso: agendamento NÃO é serviço — só roda com o KIRA vivo (`kira loop --scheduled`)\n",
    );
    Ok(out)
}

fn schedule_list() -> Result<String, kira::error::Error> {
    use kira::scheduler;
    let store = schedule_store()?;
    let rows = store.list_schedules()?;
    if rows.is_empty() {
        return Ok(
            "nenhum agendamento — crie um com `kira schedule add --action digest --every 2h`\n"
                .to_string(),
        );
    }
    let now = scheduler::parse_unix_seconds(&kira::store::now_rfc3339_utc()).unwrap_or(0);
    let mut out = format!("agendamentos ({})\n", rows.len());
    for row in &rows {
        out.push_str(&format!(
            "  id {} · {} · a cada {} · {} · {}\n",
            row.id,
            row.action,
            scheduler::format_every(row.every_seconds),
            scheduler::next_run_text(row, now),
            if row.enabled { "ativo" } else { "desligado" }
        ));
    }
    out.push_str("  roda com: `kira loop --scheduled` (sem daemon/serviço)\n");
    Ok(out)
}

fn schedule_remove(id: i64) -> Result<String, kira::error::Error> {
    let store = schedule_store()?;
    if !store.remove_schedule(id)? {
        return Err(kira::error::Error::ScheduleNotFound { id });
    }
    Ok(format!("agendamento {id} removido\n"))
}

/// Decisão humana sobre um pedido de escrita (`kira approve`).
fn approve_command(id: i64, reject: bool, by: Option<&str>) -> Outcome {
    let result = (|| -> Result<String, kira::error::Error> {
        let store = kira::store::Store::open_default()?;
        let who = by.unwrap_or("dono");
        if !store.decide_approval(id, !reject, who)? {
            // Ou não existe, ou já foi decidido: a mensagem certa importa.
            return match store.get_approval(id)? {
                None => Err(kira::error::Error::ApprovalNotFound { id }),
                Some(row) => Ok(format!(
                    "pedido {id} já estava `{}` ({}) — decisão anterior mantida\n",
                    row.state, row.action
                )),
            };
        }
        Ok(format!(
            "pedido {id} {} por {}\n",
            if reject { "recusado" } else { "aprovado" },
            who
        ))
    })();
    match result {
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

/// Relatório do período (`kira audit`): leitura pura do store.
fn audit_command(days: Option<i64>) -> Outcome {
    let result = (|| -> Result<String, kira::error::Error> {
        let store = kira::store::Store::open_default()?;
        let window = days.unwrap_or(kira::audit::DEFAULT_WINDOW_DAYS);
        let audit = kira::audit::collect_now(&store, window)?;
        Ok(kira::audit::render(&audit))
    })();
    match result {
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

/// Lista os pedidos de escrita no mundo (`kira approvals`).
fn approvals_command(all: bool) -> Outcome {
    let result = (|| -> Result<String, kira::error::Error> {
        let store = kira::store::Store::open_default()?;
        let rows = store.list_approvals(!all)?;
        if rows.is_empty() {
            return Ok(if all {
                "nenhum pedido de aprovação registrado\n".to_string()
            } else {
                "nenhuma escrita no mundo aguardando aprovação\n".to_string()
            });
        }
        let mut out = format!("pedidos ({})\n", rows.len());
        for row in &rows {
            out.push_str(&format!(
                "  id {} · {} · {} · [{}] · {}\n",
                row.id, row.action, row.target, row.state, row.summary
            ));
        }
        if !all {
            out.push_str("  decidir: `kira approve <id>` (ou `kira approve <id> --reject`)\n");
        }
        Ok(out)
    })();
    match result {
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

fn schedule_set_enabled(id: i64, enabled: bool) -> Result<String, kira::error::Error> {
    let store = schedule_store()?;
    if !store.set_schedule_enabled(id, enabled)? {
        return Err(kira::error::Error::ScheduleNotFound { id });
    }
    Ok(format!(
        "agendamento {id} {}\n",
        if enabled { "ligado" } else { "desligado" }
    ))
}

/// `kira skills` (onda SK, SK-1): catálogo de cartas de instrução.
///
/// `list`/`show` leem o disco (bundled embutido + `.kira/skills/`) — leitura
/// livre. `add` grava o skill redigido em `.kira/skills/` (local, nunca
/// commitado) e tenta atualizar o índice; falha de índice vira nota, nunca
/// desfaz o add. `search`/`reindex` usam o `skill_fts` derivado (fail-open:
/// sem FTS5 a busca degrada para LIKE dentro do store).
fn skills_command(action: SkillsAction, path: Option<&Path>) -> Outcome {
    let root = path.map_or_else(
        || store::detect_project_root().unwrap_or_else(|| PathBuf::from(".")),
        Path::to_path_buf,
    );
    match action {
        SkillsAction::List => {
            let text = kira::skills::list(&root);
            print!("{text}");
            Outcome::success(text)
        }
        SkillsAction::Show { name } => match kira::skills::show(&root, &name) {
            Ok(text) => {
                print!("{text}");
                Outcome::success(text)
            }
            Err(err) => {
                print_error(&err);
                Outcome::failure(err.to_string())
            }
        },
        SkillsAction::Add { file } => match kira::skills::add(&root, &file) {
            Ok(skill) => {
                let mut text = format!("skill adicionado: {}@{}\n", skill.name, skill.version);
                match reindex_skills(&root) {
                    Ok(count) => text.push_str(&format!("índice: {count} skill(s)\n")),
                    Err(cause) => {
                        text.push_str(&format!("aviso: índice não atualizado ({cause})\n"));
                    }
                }
                print!("{text}");
                Outcome::success(text)
            }
            Err(err) => {
                print_error(&err);
                Outcome::failure(err.to_string())
            }
        },
        SkillsAction::Search { query, limit } => {
            let store = match open_skills_store() {
                Ok(pair) => pair,
                Err(err) => {
                    print_error(&err);
                    return Outcome::failure(err.to_string());
                }
            };
            let (store, project_id) = store;
            match kira::skills::search(&store, &project_id, &query, limit.unwrap_or(5)) {
                Ok(hits) => {
                    let text = kira::skills::render_hits(&hits);
                    if text.is_empty() {
                        println!(
                            "nenhum skill casou `{query}` — rode `kira skills reindex` se acabou de adicionar"
                        );
                    } else {
                        print!("{text}");
                    }
                    Outcome::success(text)
                }
                Err(err) => {
                    print_error(&err);
                    Outcome::failure(err.to_string())
                }
            }
        }
        SkillsAction::Reindex => match reindex_skills(&root) {
            Ok(count) => {
                let text = format!("índice de skills: {count} skill(s)\n");
                print!("{text}");
                Outcome::success(text)
            }
            Err(err) => {
                print_error(&err);
                Outcome::failure(err.to_string())
            }
        },
    }
}

/// Store central + id do projeto atual (Search/Reindex de skills).
fn open_skills_store() -> Result<(Store, String), kira::skills::SkillError> {
    let root = store::detect_project_root().ok_or_else(|| {
        kira::skills::SkillError::Store(
            "sem projeto detectado — rode dentro da raiz do repositório".to_string(),
        )
    })?;
    let store =
        Store::open_default().map_err(|err| kira::skills::SkillError::Store(err.to_string()))?;
    let project_id = store::project_id_for_root(&root)
        .map_err(|err| kira::skills::SkillError::Store(err.to_string()))?;
    Ok((store, project_id))
}

/// Reconstrói o índice derivado de skills para o projeto da raiz.
fn reindex_skills(root: &Path) -> Result<usize, kira::skills::SkillError> {
    let store =
        Store::open_default().map_err(|err| kira::skills::SkillError::Store(err.to_string()))?;
    let project_id = store::project_id_for_root(root)
        .map_err(|err| kira::skills::SkillError::Store(err.to_string()))?;
    kira::skills::reindex(root, &store, &project_id)
}

/// `kira knowledge` (onda SK, SK-2): textos do dono.
///
/// `list` lê o disco — leitura livre, e diretório vazio/ausente devolve a
/// dica acionável, não erro. `search`/`reindex` usam o índice derivado no
/// store (fail-open: sem FTS5 a busca degrada para LIKE dentro do store).
/// NUNCA escreve no diretório do dono: é só leitura.
fn knowledge_command(action: KnowledgeAction) -> Outcome {
    match action {
        KnowledgeAction::List => {
            let text = kira::knowledge::list();
            print!("{text}");
            Outcome::success(text)
        }
        KnowledgeAction::Search { query, limit } => {
            let store = match Store::open_default() {
                Ok(store) => store,
                Err(err) => {
                    print_error(&err);
                    return Outcome::failure(err.to_string());
                }
            };
            match kira::knowledge::search(&store, &query, limit.unwrap_or(5)) {
                Ok(hits) => {
                    let text = kira::knowledge::render_hits(&hits);
                    if text.is_empty() {
                        println!(
                            "nenhum conhecimento casou `{query}` — rode `kira knowledge reindex` se acabou de adicionar texto em knowledge/"
                        );
                    } else {
                        print!("{text}");
                    }
                    Outcome::success(text)
                }
                Err(err) => {
                    print_error(&err);
                    Outcome::failure(err.to_string())
                }
            }
        }
        KnowledgeAction::Reindex => {
            let store = match Store::open_default() {
                Ok(store) => store,
                Err(err) => {
                    print_error(&err);
                    return Outcome::failure(err.to_string());
                }
            };
            match kira::knowledge::reindex(&store) {
                Ok(count) => {
                    let text = format!("índice de conhecimento: {count} documento(s)\n");
                    print!("{text}");
                    Outcome::success(text)
                }
                Err(err) => {
                    print_error(&err);
                    Outcome::failure(err.to_string())
                }
            }
        }
    }
}

/// `kira model list` (HF-2, ADR-0040): catálogo local via ollama — leitura
/// pura, nunca LLM, nunca rede fora do localhost do ollama.
fn model_list_command() -> Outcome {
    match kira::model::list() {
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

/// `kira loop` (PR15): o turno do binário — ticks locais (ingest → digest se
/// trusted → publish) até estabilizar; `--once` roda um tick e sai (CI).
/// Read-only no mundo: escrita externa só via `awaiting_approval` +
/// `kira approve` humano (PR18). O digest dentro do ciclo passa pelo mesmo
/// `post::run` do comando (cache, trust, orçamento — nada reimplementado).
#[allow(clippy::too_many_arguments)]
async fn loop_command(
    path: Option<&Path>,
    once: bool,
    ticks: Option<usize>,
    model: Option<String>,
    lang: Option<String>,
    skip_digest: bool,
    skip_scan: bool,
    scheduled: bool,
) -> Outcome {
    let root = kira::r#loop::resolve_root(path);
    let mut options = kira::r#loop::LoopOptions::at(root);
    if let Some(model) = model {
        options.model = model;
    }
    if let Some(lang) = lang {
        options.lang = lang;
    } else if let Ok(lang) = std::env::var("KIRA_LANG") {
        options.lang = lang;
    }
    options.skip_digest = skip_digest;
    options.run_scan = !skip_scan;
    options.scheduled = scheduled;
    match kira::r#loop::run(&options, once, ticks.unwrap_or(16)).await {
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

/// `kira model pull <repo>` (HF-2, ADR-0040): baixa GGUF via `ollama pull`
/// com consentimento (`--yes` ou prompt em TTY) e disco checado antes.
/// O download NÃO é chamada de LLM — não toca o contador (`llm_calls`
/// mede inferência, não trabalho); o log de auditoria do pull é a trilha
/// automática do `Outcome`.
fn model_pull_command(repo: &str, yes: bool) -> Outcome {
    match kira::model::pull(repo, yes) {
        Ok(text) => {
            println!("{text}");
            Outcome::success(text)
        }
        Err(err) => {
            print_error(&err);
            Outcome::failure(err.to_string())
        }
    }
}

/// `kira model admit <tag>` (HF-5, ADR-0040): registra o gate de admissão.
/// Escrita no STORE local (trilha), nunca no mundo — sem `--apply`; não é
/// chamada de LLM (não toca o contador). Recusa acionável por critério.
fn model_admit_command(tag: &str, sha256: Option<&str>, probes_ok: bool, force: bool) -> Outcome {
    match kira::model::admit(tag, sha256, probes_ok, force) {
        Ok(text) => {
            println!("{text}");
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
/// Determinístico: mesma árvore ⇒ mesmo `.kira/facts.md`. Rede só se `--skip-scan`
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

/// `kira digest` — o turno local (ADR-0038, M3).
///
/// O backend é construído **aqui** (composition root) e o módulo de decisão recebe
/// a referência: `post` não escolhe provider, só decide com o modelo.
#[allow(clippy::too_many_arguments)]
async fn digest_command(
    path: Option<&Path>,
    model: Option<String>,
    kind: Option<String>,
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
    // HF-5: `--kind` gera SÓ um kind — recusa acionável ANTES de qualquer
    // trabalho quando o kind não existe no plano (a tabela `model_role` fala
    // de kind de post real, não de desejo futuro).
    let kind_filter = match kind.as_deref() {
        Some(kind) => {
            if !kira::post::plan::KINDS.contains(&kind) {
                let valid = kira::post::plan::KINDS.join(", ");
                let message = format!("kind desconhecido: {kind} — kinds válidos: {valid}");
                eprintln!("erro: {message}");
                return Outcome::failure(message);
            }
            Some(kind.to_string())
        }
        None => None,
    };
    // HF-5 (routing): tag EXPLÍCITA (--model ou KIRA_MODEL presente) vence o
    // routing; sem escolha explícita, o sentinela `ollama` liga a resolução
    // por kind dentro do `post::run` (tabela `model_role` + gate de admissão).
    let explicit_env = std::env::var("KIRA_MODEL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let explicit_tag = model
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or(explicit_env);
    let lang = lang
        .or_else(|| std::env::var("KIRA_LANG").ok())
        .unwrap_or_else(|| "en".to_string());
    let mut options = kira::post::DigestOptions::at(root);
    options.model = match &explicit_tag {
        Some(tag) => format!("ollama:{tag}"),
        None => "ollama".to_string(), // sentinela: routing por kind (HF-5)
    };
    options.kind_filter = kind_filter;
    options.lang = lang;
    if let Some(budget) = budget {
        options.budget_tokens = budget;
    }
    options.pause_after = pause;
    options.resume = resume;

    // FÁBRICA de backend por (kind, tag): a DECISÃO de tag acontece dentro do
    // `post::run` (routing de especialistas, HF-5) — aqui só o wiring da URL.
    let base_url = ollama::base_url_from_env();
    // OC-1: validação URL × chave ANTES de qualquer request — URL não-local
    // sem chave é erro acionável, nunca request anônimo que falha confuso.
    let template = match ollama::OllamaBackend::for_base_url(&base_url, ollama::DEFAULT_MODEL) {
        Ok(template) => template,
        Err(err) => {
            print_error(&err);
            return Outcome::failure(err.to_string());
        }
    };
    // OC-1: aviso de egresso (aviso ≠ erro) — URL não-local significa que o
    // prompt SAI da máquina; o dev precisa saber antes do run.
    if let Some(reason) = kira::endpoint::egress_reason_for_url(&base_url) {
        eprintln!("aviso: egresso de dados neste run ({reason})");
    }
    match kira::post::run(&options, &|_kind: &str, tag: &str| template.with_model(tag)).await {
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

/// `kira publish` — publica os rascunhos no wiki, gate por hash (ADR-0038, M5).
///
/// A decisão de sobrescrever é do dono: `--force` explícito. Sem ele, post editado
/// à mão é recusado com o nome do arquivo na frente.
fn publish_command(path: Option<&Path>, force: bool, json: bool) -> Outcome {
    let root = path.map_or_else(
        || store::detect_project_root().unwrap_or_else(|| PathBuf::from(".")),
        Path::to_path_buf,
    );
    let mut options = publish::PublishOptions::at(root);
    options.force = force;
    match publish::run(&options) {
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
                economy: None,
            }
        }
        Err(err) => {
            print_error(&err);
            Outcome {
                code: exit_code(false),
                ok: false,
                records: false,
                summary: err.to_string(),
                economy: None,
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
fn context_command(
    id: Option<i32>,
    file: Option<PathBuf>,
    explain: bool,
    json: bool,
    profile_flag: Option<String>,
) -> Outcome {
    let source = match context_source(id, file) {
        Ok(source) => source,
        Err(message) => {
            eprintln!("erro: {message}");
            eprintln!("uso: kira context --file <work-item.json> [--explain] [--json]");
            return Outcome::failure(message);
        }
    };
    // PR40: o perfil é da própria sessão do pack — inválido falha antes de
    // qualquer coleta, com a lista dos válidos (nunca fallback silencioso).
    let profile = match kira::context::resolve_profile(profile_flag.as_deref()) {
        Ok(profile) => profile,
        Err(err) => {
            print_error(&err);
            return Outcome::failure(err.to_string());
        }
    };
    match context_output(&source, explain, json, profile) {
        Ok(text) => {
            print!("{text}");
            Outcome {
                code: ExitCode::SUCCESS,
                ok: true,
                records: false,
                summary: first_line(&text, HISTORY_SUMMARY_LINE_CHARS),
                economy: None,
            }
        }
        Err(err) => {
            print_error(&err);
            Outcome {
                code: exit_code(false),
                ok: false,
                records: false,
                summary: err.to_string(),
                economy: None,
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
fn context_output(
    file: &Path,
    explain: bool,
    json: bool,
    profile: kira::context::TokenProfile,
) -> Result<String, Error> {
    let root = store::detect_project_root().unwrap_or_else(|| PathBuf::from("."));
    let body = std::fs::read_to_string(file).map_err(|source| Error::Io {
        path: file.display().to_string(),
        source,
    })?;
    let item = kira::work_item::WorkItem::from_api_json(&body)?;
    let pack = kira::context::pack_excluding(&root, &item, Some(file), profile.caps());
    if json {
        return kira::context::report_json(&pack, item.id, profile)
            .map_err(Error::ContextReportJson);
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
            economy: None,
        }
    } else {
        let options = kira::tui::app::RunOptions { read_only };
        match kira::tui::app::run(model, options) {
            Ok(()) => Outcome {
                code: ExitCode::SUCCESS,
                ok: true,
                records: false,
                summary: "dashboard fechado pelo usuário".to_string(),
                economy: None,
            },
            Err(message) => {
                eprintln!("erro: {message}");
                Outcome {
                    code: ExitCode::FAILURE,
                    ok: false,
                    records: false,
                    summary: message,
                    economy: None,
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
                economy: None,
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

/// HF-5 (routing de especialistas, ADR-0040): tag de uma tarefa/kind.
/// Escolha explícita (`--model`/`KIRA_MODEL`) vence; sem ela, override
/// `model_role.<task>` do config com o GATE DE ADMISSÃO no store — tag não
/// admitida não roteia: cai no default da casa com nota no stderr (nunca
/// silêncio, nunca roteamento cego).
fn resolve_task_model(
    task: &str,
    flag: Option<&str>,
    budget_store: Option<&kira::store::Store>,
) -> String {
    if let Some(tag) = flag.map(str::trim).filter(|value| !value.is_empty()) {
        return tag.to_string();
    }
    if let Some(tag) = std::env::var("KIRA_MODEL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        return tag;
    }
    let config = match kira::credentials::read_config() {
        Ok(pairs) => pairs,
        Err(err) => {
            eprintln!("aviso: config ilegível ({err}) — routing segue no default");
            Vec::new()
        }
    };
    let routing =
        kira::llm::roles::resolve_tag(task, &config, None, &|candidate| match budget_store
            .map(|store| store.model_admission_get(candidate))
        {
            Some(Ok(Some(_))) => Some(true),
            Some(Ok(None)) => Some(false),
            _ => None,
        });
    for note in &routing.notes {
        eprintln!("nota: routing — {note}");
    }
    routing.tag
}

// A flag do cache (PR35) soma o 8º argumento: o comando reflete o CLI — o
// conserto estrutural (struct de opções) é refactor maior do que o escopo.
#[allow(clippy::too_many_arguments)]
async fn triage_command(
    id: Option<i32>,
    file: Option<PathBuf>,
    model_flag: Option<String>,
    llm_flag: Option<String>,
    eval: Option<PathBuf>,
    json: bool,
    seed: bool,
    no_cache: bool,
    profile_flag: Option<String>,
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
        return eval_command(eval, json, seed, model_flag, llm_flag, profile_flag).await;
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
    // PR40: o perfil é resolvido ANTES de tudo (flag > config.toml > balanced)
    // e é ÚNICO para a execução: os caps do pack, o gate do cache e o episódio
    // gravado usam o mesmo valor. Inválido = erro acionável, exit ≠ 0.
    let profile = match kira::context::resolve_profile(profile_flag.as_deref()) {
        Ok(profile) => profile,
        Err(err) => {
            print_error(&err);
            return Outcome::failure(err.to_string());
        }
    };
    let base_url = ollama::base_url_from_env();
    // Budget diário (mandato/auditoria P0): checado ANTES da chamada. Store
    // indisponível = fail-open com aviso VISÍVEL (regra dura da trilha), nunca
    // bypass silencioso; teto atingido = erro acionável, exit ≠ 0. O mesmo
    // store serve de LEDGER — `record_llm_call` acontece dentro do
    // `triage_item`, que recebe a referência abaixo.
    let budget_max = kira::credentials::max_llm_calls_per_day();
    let budget_store = match kira::store::Store::open_default() {
        Ok(store) => {
            if let Err(err) = store.ensure_llm_budget(budget_max, "triage") {
                print_error(&err);
                return Outcome::failure(err.to_string());
            }
            Some(store)
        }
        Err(err) => {
            eprintln!("aviso: budget de chamadas não verificado ({err}) — sigo sem enforcement");
            None
        }
    };
    // HF-5 (routing de especialistas, ADR-0040): tag da tarefa de triage —
    // flag/env explícitos vencem; sem eles, tabela `model_role` com o gate
    // de admissão (nota no stderr quando cai no default; nunca silêncio).
    let model = resolve_task_model("triage", model_flag.as_deref(), budget_store.as_ref());
    let result = match source {
        TriageSource::Id(id) => match resolve_board(None) {
            Ok(board) => {
                triage::triage_live_llm(
                    kind,
                    board.as_ref(),
                    id,
                    &base_url,
                    &model,
                    // PR35: `--no-cache` desliga o cache de raciocínio —
                    // a rota LLM roda sempre, byte a byte como antes.
                    !no_cache,
                    budget_store.as_ref(),
                    profile,
                )
                .await
            }
            Err(err) => Err(err),
        },
        TriageSource::File(path) => {
            triage::triage_file_llm(
                kind,
                &base_url,
                &model,
                &path,
                !no_cache,
                budget_store.as_ref(),
                profile,
            )
            .await
        }
    };
    match result {
        Ok(report) => {
            print!("{}", report.output);
            // PR35: o hit aponta a página que serviu a resposta — o dev sabe
            // que NÃO houve modelo nesta resposta (e o ledger registra isso).
            if let Some(hit) = &report.cache {
                eprintln!(
                    "fonte: cache-wiki — {} (similaridade {:.2})",
                    hit.page, hit.similarity
                );
            }
            // PR7: identifica a geração do prompt no stderr — duas saídas do
            // modelo só são comparáveis se o texto do prompt foi o mesmo. No
            // hit não houve prompt: a linha só existe na rota LLM.
            if report.cache.is_none() {
                eprintln!("prompt: {}", kira::llm::prompts::triage_prompt_id());
            }
            // ADR-0015: the user sees exactly which evidence the model got. Goes
            // to stderr so the three-section answer stays pipeable.
            if !report.context.is_empty() {
                eprintln!("{}", report.context.summary());
            }
            for note in &report.context.notes {
                eprintln!("aviso: contexto — {note}");
            }
            // Ledger de economia (PR34/PR35/PR38, onda TE): o pack que a sessão
            // REALMENTE enviou, o teto e a chamada de julgamento que ela
            // custou. No hit do cache NADA foi enviado: pack 0, ZERO chamadas
            // (determinístico não consome budget, ADR-0016) e rota
            // `cache-wiki` — a economia que o ledger existe para provar.
            // O sinal de pack ocioso (D9) fica visível no stderr — é nota,
            // nunca erro, e nunca conta como chamada. A economia do dedup
            // contíguo (PR38) é categoria própria (schema v14) e, no hit,
            // é zero junto com o pack (nada enviado = nada deduplicado).
            // As constantes espelham o schema v14 (`store::economy`).
            let cache_hit = report.cache.is_some();
            let economy = store::economy::EconomyRow {
                pack_chars: if cache_hit {
                    0
                } else {
                    i64::try_from(report.context.used_chars).unwrap_or(0)
                },
                budget_chars: if cache_hit {
                    i64::try_from(kira::context::budget_chars()).unwrap_or(0)
                } else {
                    i64::try_from(report.context.budget_chars).unwrap_or(0)
                },
                tokens_est: if cache_hit {
                    0
                } else {
                    i64::try_from(report.context.used_chars / 3).unwrap_or(0)
                },
                llm_calls: i64::from(!cache_hit),
                route: if cache_hit { "cache-wiki" } else { "llm" }.to_string(),
                // No hit o pack ocioso é o OBJETIVO (não houve coleta a furar):
                // a flag D9 só sinaliza furo de coleta na rota LLM.
                pack_idle: !cache_hit && report.context.idle_warning().is_some(),
                // PR38: a economia medida pelo dedup contíguo na montagem do
                // pack (o packer já calcula; aqui só vai para o ledger).
                dedup_chars_saved: if cache_hit {
                    0
                } else {
                    i64::try_from(report.context.dedup_chars_saved).unwrap_or(0)
                },
            };
            if !cache_hit && let Some(aviso) = report.context.idle_warning() {
                eprintln!("{aviso}");
            }
            // Episodic capture (ADR-0013, PR30): fail-open, never changes the
            // output or the exit code. A capture failure also prefixes the
            // session-trail summary (W2-fix PR-F3) so `kira history` shows it.
            // D1 (PR35): o hit NÃO grava episódio — a página original já
            // guarda a conclusão; regravar viraria log de re-execução.
            let (episode_note, episode_failed) = if cache_hit {
                (None, false)
            } else {
                // PR40: o episódio carrega o perfil que produziu a conclusão —
                // é a identidade do cache de raciocínio (cache_serves).
                record_triage_episode(&report, profile.as_str())
            };
            if let Some(note) = episode_note {
                eprint!("{note}");
            }
            if episode_failed {
                Outcome {
                    code: ExitCode::SUCCESS,
                    ok: true,
                    records: true,
                    summary: format!(
                        "wikipage: episódio failed (triage sem conteúdo nas 3 seções)\n{}",
                        report.output
                    ),
                    economy: Some(economy),
                }
            } else {
                Outcome {
                    code: ExitCode::SUCCESS,
                    ok: true,
                    records: true,
                    summary: report.output,
                    economy: Some(economy),
                }
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
    profile_flag: Option<String>,
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
    // PR40: mesmo contrato do triage — o perfil único da medição vale para o
    // pack dos itens medidos. Inválido = erro acionável, medição nem começa.
    let profile = match kira::context::resolve_profile(profile_flag.as_deref()) {
        Ok(profile) => profile,
        Err(err) => {
            print_error(&err);
            return Outcome::failure(err.to_string());
        }
    };
    let base_url = ollama::base_url_from_env();
    // `--seed` copia as 3 fixtures sintéticas num diretório vazio e segue para
    // a medição: o dono vê a mecânica do gate antes de rotular os 20 reais.
    if seed {
        if let Err(err) = kira::eval::seed_dir(&dir) {
            print_error(&err);
            return Outcome::failure(err.to_string());
        }
        eprintln!("seeds sintéticas copiadas para {}", dir.display());
    }
    // P0 (auditoria F2): o `--eval` faz UMA chamada de LLM por item do gold —
    // o mesmo teto do triage vale aqui, checado antes de cada item (o run
    // aborta inteiro, nunca parcial). Store indisponível = aviso visível e
    // execução sem teto, mesma conduta do triage vivo.
    let budget_max = kira::credentials::max_llm_calls_per_day();
    let budget_store = match kira::store::Store::open_default() {
        Ok(store) => {
            if let Err(err) = store.ensure_llm_budget(budget_max, "triage --eval") {
                print_error(&err);
                return Outcome::failure(err.to_string());
            }
            Some(store)
        }
        Err(err) => {
            eprintln!("aviso: budget de chamadas não verificado ({err}) — sigo sem enforcement");
            None
        }
    };
    // HF-5: o `--eval` roda POR MODELO — `--model <tag>` mede aquela tag; sem
    // escolha explícita, o routing decide a tag do julgamento (mesma tabela).
    let model = resolve_task_model("triage", model_flag.as_deref(), budget_store.as_ref());
    match kira::eval::run(
        &dir,
        kind,
        &base_url,
        &model,
        budget_store.as_ref(),
        profile,
    )
    .await
    {
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
                economy: None,
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
fn record_triage_episode(report: &triage::TriageReport, profile: &str) -> (Option<String>, bool) {
    match record_triage_episode_inner(report, profile) {
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
fn record_triage_episode_inner(report: &triage::TriageReport, profile: &str) -> EpisodeCapture {
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
        // PR40: perfil gravado na página = identidade do cache. Episódio
        // repetido sob o MESMO perfil não duplica (linhas iguais); trocar de
        // perfil gera bloco novo — o conhecimento fica honesto por perfil.
        profile: (!profile.trim().is_empty()).then(|| profile.trim().to_string()),
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
    let options = setup::SetupOptions {
        org,
        project,
        reset,
        pat,
        interactive,
        name,
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
