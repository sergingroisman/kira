//! `kira setup` — o wizard LOCAL-FIRST (ADR-0030).
//!
//! O KIRA funciona completo sem nenhuma integração externa: scan, wiki,
//! context, pr draft e dashboard rodam com zero credencial. Por isso o setup
//! pergunta primeiro o que é LOCAL e nunca pede credencial:
//!
//! nome do dev → projeto (raiz detectada + confirmar) → ingestão (índice +
//! digest, SEM scan — ADR-0038 D1: só o `ingest` entra no setup) → backend de
//! IA (4 opções, com **4) nenhuma por agora** como default quando o Ollama
//! não está instalado) → resumo no formato `pronto: / funciona agora: /
//! desativado: / ativar depois: / próximo passo:  kira connect azure` (ou
//! `kira triage <id>` com o board conectado) — nunca "configuração concluída".
//!
//! PAT não é assunto do setup (ADR-0030 #3/#4): credencial nasce em
//! `kira connect <provider>` (MVP-3), onde a aula (`pat_lesson_lines`) será
//! reutilizada. O setup ainda aceita `--pat` como atalho EXPLÍCITO para quem
//! já tem o token: nesse caso ele é validado na hora (as 4 mensagens exatas
//! da spec — 200 / 401 / 403 / sem rede) e salvo no keyring. Nada é conectado
//! implicitamente: env/keyring com PAT NÃO são usados por aqui.
//!
//! Entrada de token SEM ECO (melhor esforço via `stty -echo` no Unix; o valor
//! digitado nunca é impresso — nada vai para stdout/stderr/log e nunca para
//! prompt de LLM). Regra do backend de IA: o caro NUNCA é default nem
//! fallback silencioso, e `--yes` nunca bloqueia (sem Ollama ⇒ "nenhuma",
//! com o pulado impresso).
//!
//! Todo input/output é injetado (stdin, stdout, store, config path, board
//! factory, hooks de IA), então os testes dirigem o fluxo completo com stdin
//! falso, um store [`crate::credentials::InMemory`] e um servidor ADO falso —
//! o keyring do OS nunca é tocado em testes.

use std::future::Future;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::ExitCode;

/// Wizard de tela cheia (ADR-0025 + layout do opencode): coleta as respostas e
/// devolve `SetupOptions` para o motor de texto abaixo executar.
pub mod wizard;

use crate::board::{self, BoardProvider};
use crate::capabilities::{self, Capability, CapabilityState};
use crate::credentials::{
    self, CONFIG_KEY_DEV_NAME, CONFIG_KEY_ORG, CONFIG_KEY_OUTPUT_LANG, CONFIG_KEY_PROJECT,
    CRED_KEY_PAT, CredStore, ORG_ENV, PAT_ENV, PROJECT_ENV,
};
use crate::doctor;
use crate::error::Error;
use crate::llm::LlmBackend;
use crate::llm::foundry;
use crate::llm::foundry::{FoundryBackend, FoundryConfig};
use crate::llm::ollama::DEFAULT_MODEL;
use crate::store;

/// Máximo de tokens da chamada de teste do Foundry: o teste de configuração é
/// barato de propósito (uma chamada, resposta mínima).
const FOUNDRY_TEST_MAX_TOKENS: u32 = 16;

/// Flags accepted by `kira setup`.
#[derive(Debug, Default, Clone)]
pub struct SetupOptions {
    /// `--org <nome>`: atalho para quem já tem a organização — NUNCA é
    /// perguntado (ADR-0030 #3); vai para o config.toml quando resolvível.
    pub org: Option<String>,
    /// `--project <nome>`: atalho para quem já tem o projeto — NUNCA é
    /// perguntado (ADR-0030 #3); vai para o config.toml quando resolvível.
    pub project: Option<String>,
    /// `--reset`: remove o que o setup guardou como credencial (PAT e chave do
    /// Foundry no keyring, endpoint/deployment no `config.toml`) e sai.
    pub reset: bool,
    /// `--pat <token>`: atalho EXPLÍCITO (ADR-0030 #4 — credencial só nasce
    /// por consentimento). Sem a flag o setup NÃO usa PAT de env/keyring: o
    /// token informado é validado e salvo; o valor nunca é ecoado — só a
    /// máscara.
    pub pat: Option<String>,
    /// stdin é interativo (TTY), injetado pelo binário real via
    /// `stdin().is_terminal()`; só no modo interativo o nome é perguntado e o
    /// menu de IA aparece.
    pub interactive: bool,
    /// `--name <nome>`: como o dev quer ser chamado (salvo como `dev_name` em
    /// `config.toml`); vence o nome salvo e a pergunta em TTY. Nunca entra em
    /// prompt de LLM.
    pub name: Option<String>,
    /// `--yes`: modo não-interativo mesmo em TTY — nunca bloqueia; sem
    /// Ollama escolhe "nenhuma por agora" e imprime o pulado.
    pub yes: bool,
    /// `--backend <ollama|copilot|foundry|nenhuma>`: escolha EXPLÍCITA do
    /// backend de IA (o caro nunca é default nem fallback silencioso).
    pub backend: Option<String>,
    /// `--skip <passo>` (repetível): pula um passo — `pat` (no-op, o passo
    /// deixou de existir), `ia` ou `ingest` (M6).
    pub skip: Vec<String>,
    /// Etapa Ingestão (M6, ADR-0038 D1): `Some(true)` ingere (índice +
    /// digest, sem scan — rede fica fora do setup), `Some(false)` pula com
    /// nota. `None` = o chamador não decidiu: só o binário real e o wizard
    /// decidem; os testes de processo antigos seguem sem ingestão nenhuma.
    pub ingest: Option<bool>,
    /// `--lang <código>` (ADR-0038 D4): idioma do que o humano lê (posts,
    /// cards, PR). Vem da flag ou do wizard; `None` usa o `output_lang` salvo
    /// em `config.toml`, senão `en`. O valor validado vai ao config.
    pub lang: Option<String>,
    /// Raiz do projeto detectada, injetada pelo binário real
    /// (`store::detect_project_root`); testes fixam o valor. `None` faz o
    /// motor detectar pelo cwd.
    pub project_root: Option<String>,
    /// Dados do Foundry já coletados (pelo wizard de tela cheia, ADR-0035).
    /// Quando presentes, o motor NÃO pergunta nada: testa estes valores e, se
    /// o teste passar, grava endpoint/deployment no config e a chave no
    /// keyring. `None` mantém o caminho de sempre (env ou prompt em TTY).
    pub foundry: Option<FoundryChoice>,
    /// Pede eco escondido na entrada de segredo. O binário real marca `true`;
    /// testes `false` (stdin é buffer e `stty` não pode rodar). Onde o SO não
    /// tem mecanismo (Windows, sem dependência nova), o texto do prompt AVISA
    /// que o valor pode aparecer — o pedido nunca vira promessa falsa.
    pub hidden_input: bool,
}

/// Builds the board used to validate the PAT: `(pat, org, project) -> board`.
pub type BoardFactory<'a> = dyn Fn(&str, &str, &str) -> Result<Box<dyn BoardProvider>, Error> + 'a;

/// Escolha do backend de IA no setup (`4` = nenhuma por agora).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IaChoice {
    Ollama,
    Copilot,
    Foundry,
    Nenhuma,
}

/// Dados que o usuário passou para o teste do Foundry (a chave vive só aqui,
/// em memória, e nunca é impressa nem logada).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundryChoice {
    /// Endpoint do recurso (ex.: `https://...openai.azure.com/openai/v1`).
    pub endpoint: String,
    /// Nome do deployment/modelo (ex.: `gpt-5`).
    pub deployment: String,
    /// Chave da API (entrada oculta; impressa apenas mascarada).
    pub api_key: String,
}

/// Hooks de verificação do passo de IA, injetados para os testes nunca
/// tocarem em processo real, PATH ou rede.
/// Tipo do hook do Foundry (verboso por causa do `Future` em caixa — o alias
/// mantém `clippy::type_complexity` longe).
pub type FoundryCallFn =
    dyn Fn(&FoundryChoice) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send>>;

/// Hooks de verificação do passo de IA (ver alias acima para o hook do
/// Foundry).
pub struct IaHooks<'a> {
    /// `ollama list`: `Some(saída)` quando o comando existe e responde;
    /// `None` quando o ollama está ausente ou falhou.
    pub ollama_models: &'a dyn Fn() -> Option<String>,
    /// Localiza `copilot` no PATH: `Some(caminho)` ou `None`.
    pub copilot_bin: &'a dyn Fn() -> Option<String>,
    /// UMA chamada de teste barata do Foundry (401/403/404 mapeados em erros
    /// acionáveis; a chave nunca aparece em mensagem).
    pub foundry_call: &'a FoundryCallFn,
}

/// Hooks reais (usados só pelo binário; testes constroem os deles).
pub fn real_ia_hooks() -> IaHooks<'static> {
    IaHooks {
        ollama_models: &ollama_models_real,
        copilot_bin: &copilot_bin_real,
        foundry_call: &foundry_call_real,
    }
}

/// `ollama list` real: `None` quando o comando não existe, falha ou sai com
/// erro — nunca pânico, nunca bloqueio longo.
fn ollama_models_real() -> Option<String> {
    let output = std::process::Command::new("ollama")
        .arg("list")
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        None
    }
}

/// `copilot` no PATH real (which-style via `binresolve`).
fn copilot_bin_real() -> Option<String> {
    crate::binresolve::find_on_path(std::env::var_os("PATH").as_deref(), "copilot")
        .map(|path| path.display().to_string())
}

/// Chamada de teste barata real: UMA completion mínima (max_tokens 16) com
/// endpoint/deployment/chave explícitos — os mesmos valores que o usuário
/// exportaria para o `kira triage` usar.
fn foundry_call_real(
    choice: &FoundryChoice,
) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send>> {
    let choice = choice.clone();
    Box::pin(async move {
        let config = FoundryConfig {
            url_override: None,
            endpoint: Some(choice.endpoint),
            deployment: choice.deployment,
            api_version: foundry::DEFAULT_API_VERSION.to_string(),
            auth: foundry::FoundryAuthMode::default(),
            max_tokens: FOUNDRY_TEST_MAX_TOKENS,
            timeout_secs: foundry::DEFAULT_TIMEOUT_SECS,
            body_mode: foundry::BodyMode::default(),
        };
        let backend = FoundryBackend::new(config, choice.api_key)?;
        backend
            .complete(
                "Você é um teste de configuração do KIRA. Responda com uma palavra.",
                "ping",
            )
            .await
            .map(|_| ())
    })
}

/// Runs `kira setup` and returns the process exit code.
///
/// Fluxo local-first (ADR-0030 #3): nome do dev → projeto (raiz detectada) →
/// backend de IA (4 opções) → resumo. PAT só entra pelo atalho `--pat`.
#[allow(clippy::too_many_arguments)]
pub async fn run(
    options: &SetupOptions,
    store: &dyn CredStore,
    config_path: Option<&Path>,
    env: &dyn Fn(&str) -> Option<String>,
    input: &mut dyn BufRead,
    output: &mut dyn Write,
    make_board: &BoardFactory<'_>,
    ia_hooks: &IaHooks<'_>,
) -> ExitCode {
    if options.reset {
        return reset(store, config_path, output);
    }

    let config_pairs = read_config(config_path, output);

    // Valores explícitos inválidos falham de cara, sem adivinhar: --skip só
    // aceita 'pat'/'ia'/'ingest' ('pat' segue aceito por compatibilidade —
    // virou no-op porque o passo deixou de existir) e --backend só os nomes
    // conhecidos (nada de fallback silencioso).
    for skipped_step in &options.skip {
        let step = skipped_step.trim().to_ascii_lowercase();
        if step != "pat" && step != "ia" && step != "ingest" {
            let _ = writeln!(
                output,
                "erro: --skip aceita 'pat', 'ia' ou 'ingest' (recebi '{skipped_step}')."
            );
            return ExitCode::FAILURE;
        }
    }
    if let Some(name) = &options.backend {
        let parsed = name.trim().to_ascii_lowercase();
        if parsed != "ollama" && parsed != "copilot" && parsed != "foundry" && parsed != "nenhuma" {
            let _ = writeln!(
                output,
                "erro: backend desconhecido '{name}' — use ollama, copilot, foundry ou nenhuma."
            );
            return ExitCode::FAILURE;
        }
    }

    // ------------------------------------------------------------------
    // Idioma de saída (ADR-0038 D4): flag/wizard > `output_lang` salvo >
    // `en`. Explícito inválido falha de cara — a mesma regra de --skip/
    // --backend: nada de guardar lixo para o `kira digest` descobrir depois.
    // ------------------------------------------------------------------
    let saved_lang = config_pairs
        .as_ref()
        .and_then(|pairs| value_of(pairs, CONFIG_KEY_OUTPUT_LANG))
        .and_then(|value| validate_lang(&value));
    let lang = match options.lang.as_deref() {
        Some(raw) => match validate_lang(raw) {
            Some(value) => value,
            None => {
                let _ = writeln!(
                    output,
                    "erro: --lang inválido '{raw}' — use um código como en ou pt-BR (2 a 20 letras, dígitos ou hífens)."
                );
                return ExitCode::FAILURE;
            }
        },
        None => saved_lang.unwrap_or_else(|| "en".to_string()),
    };

    // ------------------------------------------------------------------
    // Passo 1 — nome do dev: `--name` > nome já salvo em `config.toml` >
    // pergunta (só em TTY) > padrão do SO. Sem TTY nunca bloqueia. Nunca
    // entra em prompt de LLM — só na saída humana (setup/doctor).
    // ------------------------------------------------------------------
    let saved_name = config_pairs
        .as_ref()
        .and_then(|pairs| value_of(pairs, CONFIG_KEY_DEV_NAME))
        .and_then(|name| validate_dev_name(&name));
    let dev_name = if let Some(name) = options.name.as_deref().and_then(validate_dev_name) {
        name
    } else if let Some(name) = saved_name {
        // Nome já existe: pergunta não aparece; troca só via `--name`.
        let _ = writeln!(output, "Olá de novo, {name}.");
        name
    } else if options.interactive && !options.yes {
        let default = default_dev_name_from_env(env);
        ask_dev_name(&default, input, output)
    } else {
        default_dev_name_from_env(env)
    };
    let _ = writeln!(output, "dev: {dev_name}");

    // ------------------------------------------------------------------
    // org/projeto NÃO são perguntados (ADR-0030 #3): entram só quando o dono
    // já tem (flag > env > config) e vão ao config.toml para o `kira connect`
    // achar depois. resolve_saved nunca pergunta.
    // ------------------------------------------------------------------
    let organization = resolve_saved(
        options.org.as_deref(),
        env(ORG_ENV),
        config_pairs
            .as_ref()
            .and_then(|pairs| value_of(pairs, CONFIG_KEY_ORG)),
    );
    let project = resolve_saved(
        options.project.as_deref(),
        env(PROJECT_ENV),
        config_pairs
            .as_ref()
            .and_then(|pairs| value_of(pairs, CONFIG_KEY_PROJECT)),
    );

    // ------------------------------------------------------------------
    // Passo PAT — existe SÓ quando o dono o traz por `--pat` (atalho
    // explícito; ADR-0030 #4: nada é conectado implicitamente, então
    // env/keyring com PAT NÃO são usados aqui). Validação na hora, com as 4
    // mensagens exatas da spec. Um 401/403 NÃO grava; falha de rede grava e
    // o fluxo segue (o `kira doctor` completa o diagnóstico depois).
    // ------------------------------------------------------------------
    if let Some(token) = options.pat.as_deref() {
        let Some(organization) = organization.as_deref() else {
            let _ = writeln!(
                output,
                "erro: --pat precisa da organização — rode de novo com --org <nome> (ou defina AZURE_DEVOPS_ORG)."
            );
            return ExitCode::FAILURE;
        };
        let Some(project) = project.as_deref() else {
            let _ = writeln!(
                output,
                "erro: --pat precisa do projeto — rode de novo com --project <nome> (ou defina AZURE_DEVOPS_PROJECT)."
            );
            return ExitCode::FAILURE;
        };
        let _ = writeln!(output, "Validando o PAT na organização {organization}...");
        let board = match make_board(token, organization, project) {
            Ok(board) => board,
            Err(err) => {
                let _ = writeln!(output, "erro: {err}");
                return ExitCode::FAILURE;
            }
        };
        match board.project_names().await {
            Ok(project_names) => {
                if doctor::find_project_case_insensitive(project, &project_names).is_none() {
                    let _ = writeln!(output);
                    let _ = writeln!(
                        output,
                        "{}",
                        doctor::format_project_check(project, &project_names)
                    );
                    let _ = writeln!(
                        output,
                        "projeto incorreto — o token NÃO foi salvo; rode `kira setup` de novo com o nome exato."
                    );
                    return ExitCode::FAILURE;
                }
                let _ = writeln!(output, "{}", msg_pat_ok(organization, project));
                if let Err(err) = store.set(CRED_KEY_PAT, token) {
                    let _ = writeln!(output, "erro: {err}");
                    return ExitCode::FAILURE;
                }
                let _ = writeln!(
                    output,
                    "✓ PAT salvo no keyring do OS ({} chars: {}).",
                    token.chars().count(),
                    doctor::mask_pat(token)
                );
            }
            Err(Error::AdoUnauthorized { .. }) => {
                let _ = writeln!(output, "{}", msg_pat_401(organization));
                let _ = writeln!(output, "o token NÃO foi salvo; rode `kira setup` de novo.");
                return ExitCode::FAILURE;
            }
            Err(Error::AdoStatus { status: 403, .. }) => {
                let _ = writeln!(output, "{}", msg_pat_403());
                let _ = writeln!(output, "o token NÃO foi salvo; rode `kira setup` de novo.");
                return ExitCode::FAILURE;
            }
            Err(_) => {
                // rede/timeout: o token É salvo (spec) e o fluxo segue —
                // o `kira doctor` completa o diagnóstico depois.
                if let Err(err) = store.set(CRED_KEY_PAT, token) {
                    let _ = writeln!(output, "erro: {err}");
                    return ExitCode::FAILURE;
                }
                let _ = writeln!(
                    output,
                    "✓ {} — salvo no keyring do OS; validação pendente (rode kira doctor)",
                    doctor::mask_pat(token)
                );
                let _ = writeln!(output, "{}", msg_pat_rede(true, false));
            }
        }
    }

    // org/projeto/nome vão para o config.toml mesmo sem credencial — assim o
    // `kira doctor` e o `kira connect` (MVP-3) já têm o contexto.
    let mut config_updates: Vec<(&str, &str)> = Vec::new();
    config_updates.push((CONFIG_KEY_DEV_NAME, dev_name.as_str()));
    config_updates.push((CONFIG_KEY_OUTPUT_LANG, lang.as_str()));
    if let Some(organization) = organization.as_deref() {
        config_updates.push((CONFIG_KEY_ORG, organization));
    }
    if let Some(project) = project.as_deref() {
        config_updates.push((CONFIG_KEY_PROJECT, project));
    }
    match config_path {
        Some(path) => {
            if let Err(err) = credentials::write_config_file(path, &config_updates) {
                let _ = writeln!(output, "aviso: não salvei org/projeto no config: {err}");
            }
        }
        None => {
            let _ = writeln!(
                output,
                "aviso: sem diretório home, as respostas não foram salvas em config.toml"
            );
        }
    }

    // ------------------------------------------------------------------
    // Passo 2 — projeto: a raiz é DETECTADA (mesma regra do store) e o dono
    // só confirma em TTY; fora de TTY nunca bloqueia.
    // ------------------------------------------------------------------
    let detected_root = options
        .project_root
        .clone()
        .or_else(detect_project_root_display)
        .unwrap_or_else(|| ".".to_string());
    let project_root = if options.interactive && !options.yes {
        ask_project_root(&detected_root, input, output)
    } else {
        let _ = writeln!(output, "projeto: {detected_root} (raiz detectada)");
        detected_root
    };
    let _ = writeln!(output, "projeto: {project_root}");

    // ------------------------------------------------------------------
    // Passo 2b — ingestão (M6, ADR-0038 D1): só o `ingest` entra no setup,
    // e SEM scan — o contrato do M1 é explícito (`run_scan: false` no
    // setup; rede fica fora do caminho local-first). O wizard COLETA a
    // escolha; este é o único lugar que executa (ADR-0034). Falha de
    // ingestão é nota, nunca erro do setup.
    // ------------------------------------------------------------------
    let mut ingest_line: Option<IngestLine> = None;
    match options.ingest {
        None => {}
        Some(false) => {
            let _ = writeln!(output, "ingestão pulada — ativar depois: kira ingest");
            ingest_line = Some(IngestLine::Skipped);
        }
        Some(true) => {
            let root = PathBuf::from(project_root.as_str());
            let mut ingest_options = crate::ingest::IngestOptions::at(root.clone());
            ingest_options.run_scan = false;
            match crate::ingest::run(&ingest_options) {
                Ok(report) => {
                    crate::ingest::ensure_local_marker(&root);
                    let _ = writeln!(
                        output,
                        "ingerido: {} arquivo(s), índice {} linha(s), digest {}",
                        report.files_ingested, report.lines_indexed, report.digest_path
                    );
                    for note in &report.notes {
                        let _ = writeln!(output, "nota: {note}");
                    }
                    ingest_line = Some(IngestLine::Done(IngestSummary {
                        files: report.files_ingested,
                        findings: report.findings_total,
                        digest_path: report.digest_path,
                    }));
                }
                Err(cause) => {
                    let cause = cause.to_string();
                    let _ = writeln!(
                        output,
                        "aviso: ingestão não rodou ({cause}) — nada foi perdido; rode kira ingest depois."
                    );
                    ingest_line = Some(IngestLine::Failed(cause));
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Passo 3 — backend de IA: 4 opções, com "nenhuma por agora" como default
    // quando o Ollama não está instalado. Custo e dados explicados em cada
    // opção e pré-requisito verificado. Backend caro NUNCA é default nem
    // fallback; `--yes` nunca bloqueia.
    // ------------------------------------------------------------------
    let mut ia_summary: Option<String> = None;
    if options
        .skip
        .iter()
        .any(|item| item.trim().eq_ignore_ascii_case("ia"))
    {
        let _ = writeln!(output, "IA pulada (--skip) — dê para escolher depois.");
    } else {
        // Pré-requisito verificado UMA vez: o default do menu e a decisão
        // não-interativa dependem dele (nunca outro processo além deste).
        let ollama_models = (ia_hooks.ollama_models)();
        let choice = if let Some(name) = options.backend.as_deref() {
            // Escolha explícita vence (mesmo em TTY): o caro só por flag.
            match name.trim().to_ascii_lowercase().as_str() {
                "ollama" => IaChoice::Ollama,
                "copilot" => IaChoice::Copilot,
                "foundry" => IaChoice::Foundry,
                _ => IaChoice::Nenhuma,
            }
        } else if options.interactive && !options.yes {
            // Default HONESTO: com Ollama ausente, "nenhuma" é o default —
            // nunca um pré-requisito que a máquina não tem.
            let default = if ollama_models.is_some() { "1" } else { "4" };
            match ask_menu_choice(IA_MENU, &["1", "2", "3", "4"], default, input, output).as_str() {
                "2" => IaChoice::Copilot,
                "3" => IaChoice::Foundry,
                "4" => IaChoice::Nenhuma,
                _ => IaChoice::Ollama,
            }
        } else if ollama_models.is_some() {
            // Não-interativo: o barato local é tentado (e a ausência do
            // modelo vira dica, não erro). Nunca um backend caro por omissão.
            IaChoice::Ollama
        } else {
            // Sem Ollama nesta máquina: "nenhuma" — e o pulado é impresso
            // abaixo, com a causa. Nunca bloqueia.
            IaChoice::Nenhuma
        };
        match choice {
            IaChoice::Ollama => match &ollama_models {
                Some(models) if models.contains(DEFAULT_MODEL) => {
                    let _ = writeln!(
                        output,
                        "✓ ia: ollama ({DEFAULT_MODEL}) — 0 custo, o código não sai da sua máquina"
                    );
                    ia_summary = Some(format!("ollama ({DEFAULT_MODEL})"));
                }
                Some(_) => {
                    let _ = writeln!(
                        output,
                        "{DEFAULT_MODEL} não está no ollama — rode: ollama pull {DEFAULT_MODEL} (3,3 GB). Por agora, ia: nenhuma."
                    );
                }
                None => {
                    let _ = writeln!(
                        output,
                        "ollama não encontrado nesta máquina — instale (https://ollama.com) e rode: ollama pull {DEFAULT_MODEL} (3,3 GB). Por agora, ia: nenhuma."
                    );
                }
            },
            IaChoice::Copilot => match (ia_hooks.copilot_bin)() {
                Some(path) => {
                    let _ = writeln!(
                        output,
                        "✓ ia: copilot ({path}) — usa sua assinatura; mesma política da sua empresa"
                    );
                    ia_summary = Some("copilot".to_string());
                }
                None => {
                    let _ = writeln!(
                        output,
                        "copilot não está no PATH — instale/autentique o Copilot CLI (pré-requisito: a política da sua empresa permitir). Por agora, ia: nenhuma."
                    );
                }
            },
            IaChoice::Foundry => {
                let saved_pairs = config_pairs.clone().unwrap_or_default();
                match collect_foundry_choice(options, env, store, &saved_pairs, input, output) {
                    Some(foundry_choice) => {
                        let _ = writeln!(
                            output,
                            "chave: {} — testando com UMA chamada barata...",
                            doctor::mask_pat(&foundry_choice.api_key)
                        );
                        match ((ia_hooks.foundry_call)(&foundry_choice)).await {
                            Ok(()) => {
                                let _ = writeln!(
                                    output,
                                    "✓ ia: foundry ({}) — BYOK: custo por token",
                                    foundry_choice.deployment
                                );
                                if options.foundry.is_some() {
                                    // Coletado no wizard (ADR-0035): gravar é o
                                    // que torna o formulário útil — o dono não
                                    // deve exportar nada depois.
                                    persist_foundry(config_path, store, &foundry_choice, output);
                                } else {
                                    // A dica de env segue o shell do SO — `export`
                                    // no PowerShell é texto que não roda.
                                    let hint = [
                                        foundry::ENV_ENDPOINT,
                                        foundry::ENV_DEPLOYMENT,
                                        foundry::ENV_API_KEY,
                                    ]
                                    .iter()
                                    .map(|name| doctor::set_env_command(name, "..."))
                                    .collect::<Vec<_>>()
                                    .join(" · ");
                                    let _ = writeln!(
                                        output,
                                        "lembrete: para o kira triage usar este backend, defina as três variáveis nesta máquina — {hint}"
                                    );
                                }
                                ia_summary =
                                    Some(format!("foundry ({})", foundry_choice.deployment));
                            }
                            Err(Error::FoundryAuth { status }) => {
                                let _ = writeln!(
                                    output,
                                    "erro: chave recusada pelo Foundry (HTTP {status}) — a IA fica pulada; rode kira setup de novo para tentar outra chave. Por agora, ia: nenhuma."
                                );
                            }
                            Err(Error::FoundryDeployment) => {
                                let _ = writeln!(
                                    output,
                                    "erro: HTTP 404 do Foundry — confira KIRA_FOUNDRY_ENDPOINT e KIRA_FOUNDRY_DEPLOYMENT; a IA fica pulada. Por agora, ia: nenhuma."
                                );
                            }
                            Err(err) => {
                                let _ = writeln!(
                                    output,
                                    "erro: {err} — a IA fica pulada; rode kira setup de novo. Por agora, ia: nenhuma."
                                );
                            }
                        }
                    }
                    // Sem endpoint/deployment/chave: erro acionável SEM
                    // nenhuma chamada de rede.
                    None => return ExitCode::FAILURE,
                }
            }
            IaChoice::Nenhuma => {
                // De onde veio "nenhuma": com Ollama ausente e sem escolha
                // explícita, a causa vem junto (o pulado é impresso, não
                // escondido); escolha explícita só confirma.
                if options.backend.is_none() && ollama_models.is_none() {
                    let _ = writeln!(
                        output,
                        "ollama não encontrado nesta máquina — instale (https://ollama.com) e rode: ollama pull {DEFAULT_MODEL} (3,3 GB)"
                    );
                }
                let _ = writeln!(
                    output,
                    "ia: nenhuma por agora — scan, wiki, context, pr draft e dashboard funcionam sem IA."
                );
            }
        }
    }

    // ------------------------------------------------------------------
    // Resumo final: o que ficou pronto, o que funciona AGORA (sem credencial
    // nenhuma), o que fica desativado até uma capacidade conectar e o
    // PRÓXIMO comando — nunca "configuração concluída".
    // ------------------------------------------------------------------
    let store_line = store::db_path()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "indisponível (sem HOME)".to_string());
    let board_connected = board_azure_connected(Path::new(project_root.as_str()));
    let _ = writeln!(output);
    let _ = writeln!(
        output,
        "{}",
        format_summary(&SummaryInput {
            dev: &dev_name,
            ia: ia_summary.as_deref(),
            store_path: &store_line,
            lang: &lang,
            ingest: ingest_line,
            board_connected,
        })
    );
    ExitCode::SUCCESS
}

/// `board.azure` conectada para ESTA raiz? A mesma fonte do gating (store por
/// projeto, `kira connect` que marca); qualquer falha de leitura = não
/// conectada — o resumo é diagnóstico, nunca promessa.
fn board_azure_connected(root: &Path) -> bool {
    let Ok(store) = store::Store::open_default() else {
        return false;
    };
    let Ok(project_id) = store::project_id_for_root(root) else {
        return false;
    };
    store
        .capability_state_raw(&project_id, Capability::BoardAzure.name())
        .ok()
        .flatten()
        .and_then(|value| CapabilityState::from_stored(&value))
        == Some(CapabilityState::Conectada)
}

/// Primeiro valor não vazio entre as fontes (flag > env > config), SEM
/// pergunta: org/projeto não são mais pedidos no setup (ADR-0030 #3).
fn resolve_saved(
    flag: Option<&str>,
    env_value: Option<String>,
    config_value: Option<String>,
) -> Option<String> {
    [flag.map(str::to_string), env_value, config_value]
        .into_iter()
        .flatten()
        .find(|value| !value.trim().is_empty())
}

/// Raiz do projeto (mesma detecção do store) como texto, para o passo
/// "projeto" do setup. `None` = sem cwd (o chamador usa um fallback).
fn detect_project_root_display() -> Option<String> {
    store::detect_project_root().map(|root| root.display().to_string())
}

/// Passo "projeto": mostra a raiz detectada e confirma (Enter = sim). Um
/// caminho digitado substitui o detectado SÓ se existir (nunca inventa raiz);
/// EOF ⇒ confirma o detectado. Só é chamada em modo interativo (TTY).
fn ask_project_root(detected: &str, input: &mut dyn BufRead, output: &mut dyn Write) -> String {
    let _ = write!(output, "Projeto: {detected}\nÉ este? (Enter = sim): ");
    let _ = output.flush();
    match read_line(input) {
        None => detected.to_string(),
        Some(line) if line.is_empty() => detected.to_string(),
        Some(line) => {
            if Path::new(line.as_str()).is_dir() {
                line
            } else {
                let _ = writeln!(
                    output,
                    "caminho '{line}' não existe — mantendo a raiz detectada."
                );
                detected.to_string()
            }
        }
    }
}

/// `--reset`: remove TUDO o que o `kira setup` gravou como credencial ou
/// configuração de credencial — PAT e chave do Foundry no keyring, e
/// endpoint/deployment (públicos) do `config.toml` (ADR-0035).
///
/// Simetria de propósito: o wizard grava os três; o reset apaga os três. Org,
/// projeto e `dev_name` ficam — são identidade do projeto, não credencial, e
/// apagá-los obrigaria o dono a redigitar tudo. Idempotente: ausência não é
/// erro, e ausência de arquivo também não.
fn reset(store: &dyn CredStore, config_path: Option<&Path>, output: &mut dyn Write) -> ExitCode {
    let had_foundry_key = store
        .get(credentials::CRED_KEY_FOUNDRY_API_KEY)
        .ok()
        .flatten()
        .is_some_and(|key| !key.trim().is_empty());
    if let Err(err) = store
        .remove(CRED_KEY_PAT)
        .and_then(|()| store.remove(credentials::CRED_KEY_FOUNDRY_API_KEY))
    {
        let _ = writeln!(output, "erro: {err}");
        return ExitCode::FAILURE;
    }
    let _ = writeln!(output, "PAT removido do keyring do OS.");
    if had_foundry_key {
        let _ = writeln!(output, "chave do Foundry removida do keyring do OS.");
    } else {
        let _ = writeln!(
            output,
            "não havia chave do Foundry no keyring — nada a remover."
        );
    }
    match config_path {
        Some(path) => match credentials::remove_config_keys(
            path,
            &[
                credentials::CONFIG_KEY_FOUNDRY_ENDPOINT,
                credentials::CONFIG_KEY_FOUNDRY_DEPLOYMENT,
            ],
        ) {
            Ok(true) => {
                let _ = writeln!(
                    output,
                    "endpoint/deployment do Foundry removidos do config.toml."
                );
            }
            Ok(false) => {
                let _ = writeln!(
                    output,
                    "config.toml já não tinha endpoint/deployment do Foundry."
                );
            }
            Err(err) => {
                let _ = writeln!(
                    output,
                    "aviso: config.toml não pôde ser limpo ({err}) — remova `foundry_endpoint`/`foundry_deployment` à mão."
                );
            }
        },
        None => {
            let _ = writeln!(output, "sem diretório home: nada a limpar em config.toml.");
        }
    }
    let _ = writeln!(
        output,
        "org, projeto e dev foram mantidos em config.toml — o comando de retomada é `kira setup`."
    );
    ExitCode::SUCCESS
}

/// Reads the config pairs, printing a warning (never crashing) on failure.
fn read_config(path: Option<&Path>, output: &mut dyn Write) -> Option<Vec<(String, String)>> {
    match path {
        Some(path) => match credentials::read_config_file(path) {
            Ok(pairs) => Some(pairs),
            Err(err) => {
                let _ = writeln!(output, "aviso: ignorando config.toml: {err}");
                None
            }
        },
        None => None,
    }
}

fn value_of(pairs: &[(String, String)], key: &str) -> Option<String> {
    pairs
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.clone())
}

fn read_line(input: &mut dyn BufRead) -> Option<String> {
    let mut line = String::new();
    match input.read_line(&mut line) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(line.trim().to_string()),
    }
}

/// URL da página de criação de PAT da organização.
fn pat_url(organization: &str) -> String {
    format!("https://dev.azure.com/{organization}/_usersSettings/tokens")
}

/// A aula do PAT (texto exato da spec, com `<sua-org>` substituído pela org
/// lida do config/env quando existir): o que é (analogia da senha
/// descartável), por que pedimos, como fazer e o que acontece.
pub fn pat_lesson(url: &str) -> String {
    pat_lesson_lines(url).join("\n")
}

/// A aula do PAT em LINHAS — a fonte ÚNICA do texto (ADR-0030 #4): o setup
/// deixou de imprimi-la e o `kira connect <provider>` (MVP-3) a reaproveita
/// sem duplicar string. Cada elemento é uma linha pronta para imprimir.
pub fn pat_lesson_lines(url: &str) -> Vec<String> {
    let url = url.to_string();
    vec![
        "O que é um PAT".to_string(),
        "PAT é uma senha descartável, feita para uma coisa só. Em vez de entregar a".to_string(),
        "senha da sua conta para o KIRA, você cria um Personal Access Token: um código".to_string(),
        "que permite apenas ler Work Items do projeto que você escolher, e que expira".to_string(),
        "na data que você definir. Se ele vazar, você apaga aquele código — sua conta e"
            .to_string(),
        "o resto do Azure DevOps continuam intactos. É por isso que ele é mais seguro".to_string(),
        "que a senha.".to_string(),
        String::new(),
        "Como criar (2 minutos)".to_string(),
        format!("1. Abra {url}"),
        "2. New Token → nome: kira-local → validade: 30 dias (ou 90, se preferir)".to_string(),
        "3. Em Scopes, marque Work Items → Read. Só isso. Nada mais é necessário.".to_string(),
        "4. Clique Create e copie o valor agora — ele aparece uma única vez.".to_string(),
    ]
}

/// Menu do backend de IA (texto exato da spec; o default depende da máquina:
/// com Ollama ausente, `4` — "nenhuma" — é o default; o caro nunca é
/// escolhido por omissão).
pub const IA_MENU: &str = "Como o KIRA deve pensar?\n\
     1) Ollama local            (recomendado: 0 custo, o código não sai da sua máquina)\n\
     2) Copilot CLI             (usa sua assinatura; mesma política da sua empresa)\n\
     3) Microsoft Foundry       (BYOK: custo por token; precisa endpoint + deployment + chave)\n\
     4) nenhuma por agora       (scan, wiki, context, pr draft e dashboard funcionam sem IA)\n";

/// Pergunta de menu genérica: imprime o menu + `Escolha [<default>]: ` e
/// valida contra `valid`; Enter/EOF ⇒ default; resposta inválida repete no
/// máximo 2x e então assume o default, com aviso (mesmo contrato da antiga
/// pergunta s/n).
fn ask_menu_choice(
    menu: &str,
    valid: &[&str],
    default: &str,
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> String {
    let options = valid.join(", ");
    let mut invalid = 0;
    loop {
        let _ = write!(output, "{menu}Escolha [{default}]: ");
        let _ = output.flush();
        match read_line(input) {
            None => return default.to_string(),
            Some(line) => {
                let answer = if line.is_empty() {
                    default.to_string()
                } else {
                    line.clone()
                };
                if valid.contains(&answer.as_str()) {
                    return answer;
                }
                invalid += 1;
                let _ = writeln!(output, "resposta inválida ('{line}') — use {options}.");
                if invalid >= 2 {
                    let _ = writeln!(
                        output,
                        "resposta inválida de novo — assumindo '{default}' (padrão)."
                    );
                    return default.to_string();
                }
            }
        }
    }
}

/// Melhor esforço para esconder o que o usuário digita: `stty -echo`/`echo`
/// no Unix (o eco é do terminal, não do KIRA; o valor lido nunca é impresso
/// de qualquer forma). Chamado SÓ quando `hidden_input` é `true` e o SO tem
/// mecanismo: antes disso o `stty` rodava até nos testes. Windows não tem
/// mecanismo sem dependência nova — o prompt avisa em vez de prometer.
fn terminal_echo(enable: bool) {
    #[cfg(unix)]
    {
        use std::process::{Command, Stdio};
        let _ = Command::new("stty")
            .arg(if enable { "echo" } else { "-echo" })
            .stdin(Stdio::inherit())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(not(unix))]
    {
        let _ = enable;
    }
}

/// Linha do prompt de segredo, HONESTA sobre o eco: prometer "não aparece"
/// onde o SO não esconde é mentira que o dono descobre colando o token.
fn secret_prompt_line(prompt: &str, hidden: bool, can_hide: bool) -> String {
    if can_hide {
        format!("{prompt} (a entrada não aparece na tela):")
    } else if hidden {
        format!("{prompt} (o terminal pode ecoar o valor — cole com cuidado):")
    } else {
        format!("{prompt}:")
    }
}

/// Prompt de segredo. O eco só é escondido onde existe mecanismo sem
/// dependência nova (`cfg!(unix)` + `stty`); o texto acompanha a realidade.
/// Nada do que foi digitado volta para stdout/stderr/log — e nunca para
/// prompt de LLM (a camada `redact` é a segunda barreira).
fn prompt_secret(
    prompt: &str,
    hidden: bool,
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> Option<String> {
    let can_hide = hidden && cfg!(unix);
    let _ = writeln!(output, "{}", secret_prompt_line(prompt, hidden, can_hide));
    let _ = output.flush();
    if can_hide {
        terminal_echo(false);
    }
    let value = read_line(input);
    if can_hide {
        terminal_echo(true);
        // Com o eco desligado o Enter não desenha linha nova: fecha a linha.
        let _ = writeln!(output);
    }
    match value {
        Some(pat) if !pat.is_empty() => Some(pat),
        _ => {
            let _ = writeln!(
                output,
                "erro: nenhum PAT informado — rode `kira setup` de novo."
            );
            None
        }
    }
}

/// As 4 mensagens de validação (texto exato da spec):
/// 200 numa leitura barata.
pub fn msg_pat_ok(organization: &str, project: &str) -> String {
    format!("PAT ok — lê Work Items de {organization}/{project}")
}

/// 401: expirado ou copiado incompleto; a mensagem traz o link.
pub fn msg_pat_401(organization: &str) -> String {
    format!(
        "PAT não autenticou (401). O token pode ter expirado ou foi copiado \
         incompleto — crie outro em {}",
        pat_url(organization)
    )
}

/// 403: autenticou, mas sem o escopo Work Items.
pub fn msg_pat_403() -> &'static str {
    "PAT autenticou, mas sem escopo Work Items (403). Edite o token e marque Work Items → Read"
}

/// rede/timeout: para token colado o salvamento acontece (spec). A origem do
/// token muda a frase — dizer "está na env" para quem usa keyring é mentira.
pub fn msg_pat_rede(pasted: bool, from_store: bool) -> &'static str {
    if pasted {
        "não foi possível validar agora (sem rede?). O token foi salvo; rode kira doctor depois"
    } else if from_store {
        "não foi possível validar agora (sem rede?). O PAT do keyring do OS está intacto; rode kira doctor depois"
    } else {
        "não foi possível validar agora (sem rede?). O token está na env AZURE_DEVOPS_PAT; rode kira doctor depois"
    }
}

/// Coleta endpoint/deployment/chave do Foundry. Fora de TTY lê as envs
/// (`KIRA_FOUNDRY_*`); em TTY pergunta (chave com entrada oculta). Faltando
/// qualquer um ⇒ `None` após mensagem acionável — SEM chamada de rede.
fn collect_foundry_choice(
    options: &SetupOptions,
    env: &dyn Fn(&str) -> Option<String>,
    store: &dyn CredStore,
    saved: &[(String, String)],
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> Option<FoundryChoice> {
    // O wizard já coletou no formulário inline: usar exatamente o que o dono
    // digitou evita perguntar de novo (e evita perder os valores).
    if let Some(choice) = &options.foundry {
        return Some(choice.clone());
    }
    if options.interactive && !options.yes {
        let _ = write!(
            output,
            "Endpoint do Foundry (ex.: https://seu-recurso.openai.azure.com/openai/v1): "
        );
        let _ = output.flush();
        let endpoint = match read_line(input) {
            Some(value) if !value.is_empty() => value,
            _ => {
                let _ = writeln!(
                    output,
                    "erro: endpoint do Foundry não informado — defina KIRA_FOUNDRY_ENDPOINT ou rode kira setup e escolha 3 de novo."
                );
                return None;
            }
        };
        let _ = write!(output, "Deployment (ex.: gpt-5): ");
        let _ = output.flush();
        let deployment = match read_line(input) {
            Some(value) if !value.is_empty() => value,
            _ => {
                let _ = writeln!(
                    output,
                    "erro: deployment do Foundry não informado — rode kira setup e escolha 3 de novo."
                );
                return None;
            }
        };
        let api_key = match prompt_secret(
            "Cole a chave do Foundry",
            options.hidden_input,
            input,
            output,
        ) {
            Some(key) => key,
            None => {
                let _ = writeln!(output, "erro: {}", Error::FoundryMissingKey);
                return None;
            }
        };
        return Some(FoundryChoice {
            endpoint,
            deployment,
            api_key,
        });
    }

    // Não-interativo: env > config.toml (endpoint/deployment públicos) >
    // keyring (chave) — a MESMA ordem do runtime (ADR-0035), para o que o
    // wizard gravou valer aqui sem exportar nada. Nada é perguntado.
    //
    // `saved` vem do caminho de config INJETADO em `run` (não do
    // `credentials::read_config()`, que ignoraria `KIRA_CONFIG` dos testes).
    let saved_value = |key: &str| {
        saved
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.trim().to_string())
            .filter(|value| !value.is_empty())
    };
    let endpoint = env(foundry::ENV_ENDPOINT)
        .or_else(|| env(foundry::ENV_URL))
        .or_else(|| saved_value(credentials::CONFIG_KEY_FOUNDRY_ENDPOINT));
    let Some(endpoint) = endpoint.filter(|value| !value.trim().is_empty()) else {
        let _ = writeln!(
            output,
            "erro: endpoint do Foundry não informado — defina KIRA_FOUNDRY_ENDPOINT (ou KIRA_FOUNDRY_URL) ou rode kira setup e preencha o formulário do Foundry."
        );
        return None;
    };
    let deployment = env(foundry::ENV_DEPLOYMENT)
        .filter(|value| !value.trim().is_empty())
        .or_else(|| saved_value(credentials::CONFIG_KEY_FOUNDRY_DEPLOYMENT));
    let Some(deployment) = deployment else {
        let _ = writeln!(
            output,
            "erro: deployment do Foundry não informado — defina KIRA_FOUNDRY_DEPLOYMENT ou rode kira setup e preencha o formulário do Foundry."
        );
        return None;
    };
    let api_key = env(foundry::ENV_API_KEY)
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            store
                .get(credentials::CRED_KEY_FOUNDRY_API_KEY)
                .ok()
                .flatten()
                .filter(|key| !key.trim().is_empty())
        });
    let Some(api_key) = api_key else {
        let _ = writeln!(output, "erro: {}", Error::FoundryMissingKey);
        return None;
    };
    Some(FoundryChoice {
        endpoint,
        deployment,
        api_key,
    })
}

/// Grava o Foundry coletado pelo wizard: endpoint/deployment (públicos) em
/// `config.toml` e a chave no keyring do OS (ADR-0035).
///
/// Só é chamada DEPOIS do teste barato passar — chave recusada (401/403/404)
/// nunca é gravada, a mesma regra do PAT. Falha de gravação é `aviso:` e o
/// comando segue: o dono ainda pode exportar as envs.
fn persist_foundry(
    config_path: Option<&Path>,
    store: &dyn CredStore,
    choice: &FoundryChoice,
    output: &mut dyn Write,
) {
    let config_saved = match config_path {
        Some(path) => match credentials::write_config_file(
            path,
            &[
                (
                    credentials::CONFIG_KEY_FOUNDRY_ENDPOINT,
                    choice.endpoint.as_str(),
                ),
                (
                    credentials::CONFIG_KEY_FOUNDRY_DEPLOYMENT,
                    choice.deployment.as_str(),
                ),
            ],
        ) {
            Ok(()) => true,
            Err(err) => {
                let _ = writeln!(
                    output,
                    "aviso: endpoint/deployment do Foundry não foram salvos em config.toml ({err})"
                );
                false
            }
        },
        None => false,
    };
    let key_saved = match store.set(credentials::CRED_KEY_FOUNDRY_API_KEY, &choice.api_key) {
        Ok(()) => true,
        Err(err) => {
            let _ = writeln!(
                output,
                "aviso: chave do Foundry não foi para o keyring ({err}) — defina {} nesta sessão",
                foundry::ENV_API_KEY
            );
            false
        }
    };
    match (config_saved, key_saved) {
        (true, true) => {
            let _ = writeln!(
                output,
                "✓ foundry gravado: endpoint/deployment em config.toml, chave no keyring do OS ({} no ambiente continua vencendo)",
                foundry::ENV_API_KEY
            );
        }
        (false, true) => {
            let _ = writeln!(
                output,
                "foundry parcial: só a chave foi para o keyring — informe endpoint/deployment por {} / {}",
                foundry::ENV_ENDPOINT,
                foundry::ENV_DEPLOYMENT
            );
        }
        (true, false) => {
            let _ = writeln!(
                output,
                "foundry parcial: endpoint/deployment salvos; a chave ficou só nesta sessão — defina {}",
                foundry::ENV_API_KEY
            );
        }
        (false, false) => {
            let _ = writeln!(
                output,
                "foundry NÃO foi gravado — configure {} / {} / {} para usar este backend",
                foundry::ENV_ENDPOINT,
                foundry::ENV_DEPLOYMENT,
                foundry::ENV_API_KEY
            );
        }
    }
}

/// Comando de ativação de cada passo que ficou desativado (o resumo imprime
/// na seção "ativar depois").
pub fn resume_command(step: &str) -> &'static str {
    match step {
        // A credencial agora nasce em `connect` (ADR-0030 #4), não em setup.
        "pat" => "kira connect azure",
        "ia" => "kira setup --backend ollama",
        // M6: a ingestão pulada se reativa com um comando, não com o setup.
        "ingest" => "kira ingest",
        _ => "kira setup",
    }
}

/// Números da ingestão que o resumo ecoa (M6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestSummary {
    pub files: usize,
    /// `None` = sem `findings.json` — o setup NÃO roda scan (contrato do M1:
    /// rede fora do caminho local-first). A linha diz isso com a causa.
    pub findings: Option<usize>,
    pub digest_path: String,
}

/// O que a linha de ingestão do resumo diz — as três saídas possíveis do
/// passo, todas com o comando que resolve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestLine {
    Skipped,
    Failed(String),
    Done(IngestSummary),
}

/// Entrada do resumo final (M6): as respostas do setup + o resultado da
/// ingestão + a capacidade que decide o próximo passo.
pub struct SummaryInput<'a> {
    pub dev: &'a str,
    pub ia: Option<&'a str>,
    pub store_path: &'a str,
    pub lang: &'a str,
    pub ingest: Option<IngestLine>,
    pub board_connected: bool,
}

/// Linha de ingestão pura (testável): dado ausente com causa e comando,
/// nunca número inventado.
#[must_use]
pub fn ingest_line_text(line: &IngestLine) -> String {
    match line {
        IngestLine::Skipped => "ingestão: pulada — ativar depois: kira ingest".to_string(),
        IngestLine::Failed(cause) => {
            format!("ingestão: não rodou ({cause}) — ativar depois: kira ingest")
        }
        IngestLine::Done(summary) => {
            let findings = match summary.findings {
                Some(count) => format!("{count} achado(s)"),
                None => "sem scan (rode kira scan)".to_string(),
            };
            format!(
                "ingestão: {} arquivo(s), {findings}, digest {}",
                summary.files, summary.digest_path
            )
        }
    }
}

/// Valida um código de idioma no formato prático do bcp47 (`en`, `pt-BR`,
/// `zh-Hans`): 2 a 20 caracteres de letras/dígitos ASCII e hífens, sem hífen
/// nas pontas. É o que entra em `config.toml` e no `cache_key` do digest —
/// por isso a recusa é dura em flag explícita e suave em default.
#[must_use]
pub fn validate_lang(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.len() < 2 || value.len() > 20 {
        return None;
    }
    if value.starts_with('-') || value.ends_with('-') {
        return None;
    }
    if value.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        Some(value.to_string())
    } else {
        None
    }
}

/// Resumo final (formato do MVP-2 + M6): `pronto:` com dev/ia/idioma/store/
/// ingestão, os blocos "funciona agora" (toda a jornada local, zero
/// credencial), "desativado" (o que espera uma capacidade) e "ativar depois"
/// (com a linha do backend de IA só quando ficou sem IA). O próximo passo
/// saiu do `kira scan` hardcoded: com ingestão no setup, o scan já não é o
/// caminho — é `kira connect azure`, ou `kira triage <id>` quando o board
/// já está conectado. Nunca termina com "configuração concluída".
#[must_use]
pub fn format_summary(input: &SummaryInput<'_>) -> String {
    let mut text = String::new();
    text.push_str("pronto:\n");
    text.push_str(&format!("  dev: {}\n", input.dev));
    let ia_line = input.ia.unwrap_or("nenhuma");
    text.push_str(&format!("  ia:  {ia_line}\n"));
    text.push_str(&format!("  idioma: {} (posts, cards e PR)\n", input.lang));
    text.push_str(&format!("  store: {}\n", input.store_path));
    if let Some(line) = &input.ingest {
        text.push_str(&format!("  {}\n", ingest_line_text(line)));
    }
    text.push('\n');
    text.push_str("funciona agora:  scan · wiki · context · pr draft · dashboard\n");
    text.push_str("desativado:      judge de item do board · read (board) · publicar PR\n");
    text.push_str("ativar depois:   kira connect azure   (leitura de work items)\n");
    if input.ia.is_none() {
        text.push_str(&format!("                 {}\n", resume_command("ia")));
    }
    if input.board_connected {
        text.push_str("próximo passo:   kira triage <id>");
    } else {
        text.push_str("próximo passo:   kira connect azure");
    }
    text
}

/// Valida o nome do dev: 1..=40 caracteres, sem control chars (o que cobre
/// quebras de linha) e sem espaços nas pontas (trim). Inválido ⇒ `None` —
/// a pergunta se repete (TTY) ou cai no padrão (não-interativo); nunca erro
/// fatal.
pub fn validate_dev_name(raw: &str) -> Option<String> {
    let name = raw.trim();
    if name.is_empty() || name.chars().count() > 40 {
        return None;
    }
    if name.chars().any(char::is_control) {
        return None;
    }
    Some(name.to_string())
}

/// Padrão do nome do dev: usuário do SO (`USER`/`USERNAME`) com a inicial em
/// maiúscula; vazio/ausente ⇒ `dev`. Lê via o lookup de env injetado para os
/// testes nunca tocarem nas variáveis reais do processo.
pub fn default_dev_name_from_env(env: &dyn Fn(&str) -> Option<String>) -> String {
    let raw = env("USER").or_else(|| env("USERNAME")).unwrap_or_default();
    let raw = raw.trim();
    if raw.is_empty() {
        return "dev".to_string();
    }
    let mut chars = raw.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
        None => "dev".to_string(),
    }
}

/// Pergunta "Como devo te chamar?": Enter ⇒ padrão; entrada inválida repete a
/// pergunta; stdin fechou (EOF) ⇒ padrão. Nunca é erro fatal — só é chamada
/// no modo interativo (TTY), nunca em stdin pipado/CI.
fn ask_dev_name(default: &str, input: &mut dyn BufRead, output: &mut dyn Write) -> String {
    loop {
        let _ = write!(output, "Como devo te chamar? (Enter = {default}): ");
        let _ = output.flush();
        match read_line(input) {
            None => return default.to_string(),
            Some(line) if line.is_empty() => return default.to_string(),
            Some(line) => match validate_dev_name(&line) {
                Some(name) => return name,
                None => {
                    let _ = writeln!(
                        output,
                        "nome inválido ('{line}') — use de 1 a 40 caracteres, sem quebras de linha."
                    );
                }
            },
        }
    }
}

/// Env que aponta o Azure DevOps para OUTRO endpoint (`KIRA_ADO_URL`).
///
/// Existe para o e2e do binário (`tests/capabilities_e2e.rs`) provar os 4
/// estados de validação contra um servidor falso LOCAL — nunca dev.azure.com.
/// Nenhum fluxo de produto a define; sem a env o comportamento é o de sempre.
pub const ADO_URL_ENV: &str = "KIRA_ADO_URL";

/// Board factory used by the real binary: the Azure DevOps provider selected
/// by `KIRA_BOARD` (default `azure`), com o override de endpoint de teste.
pub fn default_board_factory(
    pat: &str,
    organization: &str,
    project: &str,
) -> Result<Box<dyn BoardProvider>, Error> {
    let endpoint = std::env::var(ADO_URL_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty());
    let name = board::name_from_env();
    match (name.trim().to_lowercase().as_str(), endpoint) {
        (board::azure::BOARD_NAME, Some(endpoint)) => Ok(Box::new(
            board::azure::AdoBoard::with_endpoint(&endpoint, pat, organization, project),
        )),
        _ => board::for_board(&name, pat, organization, project),
    }
}

/// Env lookup used by the real binary.
pub fn env_lookup(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

// ---------------------------------------------------------------------------
// `kira connect azure` (MVP-3, ADR-0030 #4): a credencial nasce AQUI.
// ---------------------------------------------------------------------------

/// Menu do `kira connect azure` (texto do escopo do MVP-3): colar, usar a env
/// ou cancelar. O default é CANCELAR (3) — a opção que não grava nada; Enter
/// ou EOF nunca viram ação que guarda segredo.
pub const CONNECT_AZURE_MENU: &str = "Como o KIRA conecta o board (Azure DevOps)?\n     1) colar PAT\n     2) usar AZURE_DEVOPS_PAT do ambiente\n     3) cancelar\n";

/// Opções do `kira connect azure` — tudo injetado, como em [`run`]: os testes
/// dirigem o fluxo com stdin falso, [`credentials::InMemory`], servidor ADO
/// falso e store isolado; o keyring do OS nunca é tocado em teste.
pub struct ConnectAzureOptions<'a> {
    /// stdin é TTY E o dev não passou `--yes`: só aqui a aula e o menu
    /// aparecem. Fora disso o fluxo é não-interativo — nunca bloqueia.
    pub interactive: bool,
    /// Pede eco escondido na entrada do PAT colado (binário real: `true`;
    /// testes `false`, porque stdin é buffer e `stty` não pode rodar).
    pub hidden_input: bool,
    /// Onde o PAT validado é gravado (keyring do OS no binário real).
    pub secret_store: &'a dyn CredStore,
    /// config.toml: org/projeto lidos daqui quando as envs faltam e gravados
    /// no SUCESSO (valores públicos; segredo nunca mora nesse arquivo).
    pub config_path: Option<&'a Path>,
    /// Lookup de env injetado (os testes nunca tocam nas variáveis do processo).
    pub env: &'a dyn Fn(&str) -> Option<String>,
    /// Fábrica do board para a validação imediata (endpoint falso em teste).
    pub make_board: &'a BoardFactory<'a>,
    /// Raiz do projeto atual: o estado da capacidade é POR PROJETO (ADR-0030).
    pub project_root: &'a Path,
    /// Store do sqlite onde o estado da capacidade é gravado.
    pub state_store: &'a store::Store,
}

/// `kira connect azure` (MVP-3).
///
/// TTY: imprime a aula (fonte ÚNICA [`pat_lesson_lines`], com a org vinda do
/// config/env quando existir e `<sua-org>` como marcador), oferece o menu
/// [`CONNECT_AZURE_MENU`], lê o segredo SEM ECO, valida na hora com as 4
/// mensagens exatas da spec (200/401/403/sem rede) e, no 200, grava o PAT no
/// keyring e marca `board.azure` como conectada PARA O PROJETO ATUAL.
///
/// Regras de falha do escopo: 401/403/projeto-incorreto/cancelar/sem PAT/sem
/// org ⇒ NADA é gravado, a mensagem traz a causa e o link de correção, exit ≠
/// 0. Sem rede o token É salvo (mesma conduta do `setup --pat` — é isso que
/// torna [`msg_pat_rede`] verdadeira), mas a capacidade NÃO é marcada.
///
/// Fora de TTY (ou com `--yes`): a única origem do segredo é
/// `AZURE_DEVOPS_PAT` — sem ela, erro acionável; nunca fica pendurado lendo
/// stdin. O `github`/`gitlab` seguem "ainda não implementado" (no `main`).
///
/// # Errors
/// Erros do board/store são impressos com causa; o exit code é o resultado.
#[allow(clippy::too_many_arguments)]
pub async fn connect_azure(
    options: &ConnectAzureOptions<'_>,
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> ExitCode {
    let config_pairs = read_config(options.config_path, output);
    // org/projeto: env > config.toml — a MESMA resolução do `kira doctor` e
    // do `kira triage`, para o que o connect validar ser o que eles usam.
    let organization = resolve_saved(
        None,
        (options.env)(ORG_ENV),
        config_pairs
            .as_ref()
            .and_then(|pairs| value_of(pairs, CONFIG_KEY_ORG)),
    );
    let project = resolve_saved(
        None,
        (options.env)(PROJECT_ENV),
        config_pairs
            .as_ref()
            .and_then(|pairs| value_of(pairs, CONFIG_KEY_PROJECT)),
    );

    // Fora de TTY: nada de aula, nada de menu, nada de leitura do stdin —
    // a env é a única origem e a ausência dela é erro acionável.
    if !options.interactive {
        return connect_azure_from_env(options, organization, project, output).await;
    }

    // Aula do PAT (texto do ADR-0025, uma única fonte): com org conhecida o
    // link aponta a página certa; sem ela, `<sua-org>` é o marcador honesto.
    let lesson_url = pat_url(organization.as_deref().unwrap_or("<sua-org>"));
    for line in pat_lesson_lines(&lesson_url) {
        let _ = writeln!(output, "{line}");
    }
    let _ = writeln!(output);

    let choice = ask_menu_choice(CONNECT_AZURE_MENU, &["1", "2", "3"], "3", input, output);
    let (token, pasted) = match choice.as_str() {
        "3" => {
            let _ = writeln!(
                output,
                "connect cancelado — nada foi gravado; board.azure continua desconectada."
            );
            return ExitCode::FAILURE;
        }
        "1" => {
            let Some(token) = connect_secret_input(options.hidden_input, input, output) else {
                let _ = writeln!(
                    output,
                    "erro: nenhum PAT informado — rode `kira connect azure` de novo."
                );
                return ExitCode::FAILURE;
            };
            (token, true)
        }
        // Qualquer resposta fora de 1/2/3 já virou cancelamento no menu; 2 é
        // a env — e a env VAZIA nunca é segredo.
        _ => match (options.env)(PAT_ENV).filter(|value| !value.trim().is_empty()) {
            Some(token) => (token, false),
            None => {
                let _ = writeln!(
                    output,
                    "erro: {PAT_ENV} não está definida — exporte a env e escolha 2 de novo, ou escolha 1 para colar o PAT."
                );
                return ExitCode::FAILURE;
            }
        },
    };

    // org/projeto: sem eles a validação não tem contra quem conferir — o
    // erro sai ANTES de qualquer rede, com o comando que resolve.
    let Some(organization) = organization else {
        let _ = writeln!(
            output,
            "erro: organização do Azure DevOps não configurada (a aula acima usa <sua-org> como marcador) — rode `kira setup --org <nome> --project <nome>` ou defina {ORG_ENV}."
        );
        return ExitCode::FAILURE;
    };
    let Some(project) = project else {
        let _ = writeln!(
            output,
            "erro: projeto do Azure DevOps não configurado — rode `kira setup --org <nome> --project <nome>` ou defina {PROJECT_ENV}."
        );
        return ExitCode::FAILURE;
    };

    connect_azure_validate(options, &token, pasted, &organization, &project, output).await
}

/// Caminho NÃO-interativo (sem TTY ou com `--yes`): a env é a única origem do
/// segredo; sem ela o erro é acionável e o stdin NUNCA é lido — `--yes` em CI
/// não pode ficar pendurado esperando o menu.
async fn connect_azure_from_env(
    options: &ConnectAzureOptions<'_>,
    organization: Option<String>,
    project: Option<String>,
    output: &mut dyn Write,
) -> ExitCode {
    let Some(token) = (options.env)(PAT_ENV).filter(|value| !value.trim().is_empty()) else {
        let _ = writeln!(
            output,
            "erro: kira connect azure não-interativo precisa de {PAT_ENV} — exporte a env e rode de novo (ou rode `kira connect azure` em um terminal para colar o PAT)."
        );
        return ExitCode::FAILURE;
    };
    let Some(organization) = organization else {
        let _ = writeln!(
            output,
            "erro: organização do Azure DevOps não configurada — rode `kira setup --org <nome> --project <nome>` ou defina {ORG_ENV}."
        );
        return ExitCode::FAILURE;
    };
    let Some(project) = project else {
        let _ = writeln!(
            output,
            "erro: projeto do Azure DevOps não configurado — rode `kira setup --org <nome> --project <nome>` ou defina {PROJECT_ENV}."
        );
        return ExitCode::FAILURE;
    };
    connect_azure_validate(options, &token, false, &organization, &project, output).await
}

/// Validação imediata do `connect azure`: as 4 mensagens EXATAS da spec
/// (reaproveitadas de cima, nunca reescritas) e a gravação só onde elas são
/// verdadeiras. 200 valida também o projeto (mesmo check do `setup --pat`).
async fn connect_azure_validate(
    options: &ConnectAzureOptions<'_>,
    token: &str,
    pasted: bool,
    organization: &str,
    project: &str,
    output: &mut dyn Write,
) -> ExitCode {
    let _ = writeln!(output, "Validando o PAT na organização {organization}...");
    let board = match (options.make_board)(token, organization, project) {
        Ok(board) => board,
        Err(err) => {
            let _ = writeln!(output, "erro: {err}");
            return ExitCode::FAILURE;
        }
    };
    match board.project_names().await {
        Ok(project_names) => {
            if doctor::find_project_case_insensitive(project, &project_names).is_none() {
                let _ = writeln!(output);
                let _ = writeln!(
                    output,
                    "{}",
                    doctor::format_project_check(project, &project_names)
                );
                let _ = writeln!(
                    output,
                    "projeto incorreto — o token NÃO foi salvo; rode `kira connect azure` de novo com o nome exato."
                );
                return ExitCode::FAILURE;
            }
            let _ = writeln!(output, "{}", msg_pat_ok(organization, project));
            if let Err(err) = options.secret_store.set(CRED_KEY_PAT, token) {
                let _ = writeln!(output, "erro: {err}");
                return ExitCode::FAILURE;
            }
            let _ = writeln!(
                output,
                "✓ PAT salvo no keyring do OS ({} chars: {}).",
                token.chars().count(),
                doctor::mask_pat(token)
            );
            // Sucesso = consentimento explícito (decisão 6 do ADR-0030): só
            // AGORA a capacidade fica conectada, e só para ESTE projeto.
            if let Err(err) = capabilities::set_state(
                options.state_store,
                options.project_root,
                Capability::BoardAzure,
                CapabilityState::Conectada,
            ) {
                let _ = writeln!(
                    output,
                    "erro: PAT salvo, mas board.azure não foi marcada como conectada ({err}) — rode `kira doctor`."
                );
                return ExitCode::FAILURE;
            }
            // org/projeto (públicos) vão ao config.toml para o `kira triage`
            // achar depois — mesmo caminho do setup; segredo nunca mora aqui.
            if let Some(path) = options.config_path
                && let Err(err) = credentials::write_config_file(
                    path,
                    &[
                        (CONFIG_KEY_ORG, organization),
                        (CONFIG_KEY_PROJECT, project),
                    ],
                )
            {
                let _ = writeln!(output, "aviso: não salvei org/projeto no config: {err}");
            }
            let _ = writeln!(
                output,
                "✓ board.azure conectada — próximo passo: kira triage <work-item-id>"
            );
            ExitCode::SUCCESS
        }
        Err(Error::AdoUnauthorized { .. }) => {
            let _ = writeln!(output, "{}", msg_pat_401(organization));
            let _ = writeln!(
                output,
                "o token NÃO foi salvo; rode `kira connect azure` de novo."
            );
            ExitCode::FAILURE
        }
        Err(Error::AdoStatus { status: 403, .. }) => {
            let _ = writeln!(output, "{}", msg_pat_403());
            let _ = writeln!(
                output,
                "o token NÃO foi salvo; rode `kira connect azure` de novo."
            );
            ExitCode::FAILURE
        }
        Err(_) => {
            // Sem rede: o PAT é salvo (mesma conduta do `setup --pat`) — é o
            // que torna a mensagem de rede verdadeira — mas a capacidade NÃO
            // é marcada: nada foi validado. Refazer o connect com rede marca.
            if let Err(err) = options.secret_store.set(CRED_KEY_PAT, token) {
                let _ = writeln!(output, "erro: {err}");
                return ExitCode::FAILURE;
            }
            let _ = writeln!(
                output,
                "✓ {} — salvo no keyring do OS; validação pendente (rode kira doctor)",
                doctor::mask_pat(token)
            );
            let _ = writeln!(output, "{}", msg_pat_rede(pasted, false));
            let _ = writeln!(
                output,
                "board.azure continua desconectada — com rede, rode `kira connect azure` de novo."
            );
            ExitCode::FAILURE
        }
    }
}

/// Entrada do PAT colado no `connect azure` (sem eco onde o SO permite):
/// mesma mecânica do prompt de segredo do setup. O valor lido NUNCA é
/// impresso; vazio ⇒ `None` — o erro é do chamador, com o comando de retomada
/// do connect (não do setup).
fn connect_secret_input(
    hidden: bool,
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> Option<String> {
    let can_hide = hidden && cfg!(unix);
    let _ = writeln!(
        output,
        "{}",
        secret_prompt_line("Cole o PAT", hidden, can_hide)
    );
    let _ = output.flush();
    if can_hide {
        terminal_echo(false);
    }
    let value = read_line(input);
    if can_hide {
        terminal_echo(true);
        // Com o eco desligado o Enter não desenha linha nova: fecha a linha.
        let _ = writeln!(output);
    }
    value.filter(|pat| !pat.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufReader;

    fn env_from<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name: &str| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| value.to_string())
        }
    }

    // ------------------------------------------------------------------
    // Mensagens de validação: as 4 exatas da spec.
    // ------------------------------------------------------------------
    #[test]
    fn validation_messages_are_exact() {
        assert_eq!(
            msg_pat_ok("minha-org", "Kira"),
            "PAT ok — lê Work Items de minha-org/Kira"
        );
        assert_eq!(
            msg_pat_401("minha-org"),
            "PAT não autenticou (401). O token pode ter expirado ou foi copiado \
             incompleto — crie outro em https://dev.azure.com/minha-org/_usersSettings/tokens"
        );
        assert_eq!(
            msg_pat_403(),
            "PAT autenticou, mas sem escopo Work Items (403). Edite o token e \
             marque Work Items → Read"
        );
        assert_eq!(
            msg_pat_rede(true, false),
            "não foi possível validar agora (sem rede?). O token foi salvo; rode kira doctor depois"
        );
        assert_eq!(
            msg_pat_rede(false, false),
            "não foi possível validar agora (sem rede?). O token está na env \
             AZURE_DEVOPS_PAT; rode kira doctor depois"
        );
        assert!(
            msg_pat_rede(false, true).contains("keyring do OS"),
            "origem keyring não pode ser chamada de env"
        );
    }

    // ------------------------------------------------------------------
    // A aula do PAT (em linhas, fonte única para o `connect` do MVP-3): o
    // que é, por que, como fazer (URL + escopo + validade + valor único).
    // ------------------------------------------------------------------
    #[test]
    fn pat_lesson_lines_teach_what_how_and_single_use() {
        let lines = pat_lesson_lines("https://dev.azure.com/minha-org/_usersSettings/tokens");
        let lesson = lines.join("\n");
        assert!(lesson.contains("senha descartável"), "aula: {lesson}");
        assert!(lesson.contains("Personal Access Token"), "aula: {lesson}");
        assert!(lesson.contains("https://dev.azure.com/minha-org/_usersSettings/tokens"));
        assert!(lesson.contains("Work Items → Read"), "aula: {lesson}");
        assert!(lesson.contains("30 dias"), "aula: {lesson}");
        assert!(lesson.contains("90"), "aula: {lesson}");
        assert!(lesson.contains("uma única vez"), "aula: {lesson}");
        // Cada elemento é uma linha pronta (sem quebra interna): o `connect`
        // pode numerar/indentar sem re-parsear.
        for line in &lines {
            assert!(!line.contains('\n'), "linha com quebra interna: {line:?}");
        }
        // pat_lesson é a mesma fonte, só junta — nunca uma segunda cópia.
        assert_eq!(
            pat_lesson("https://x/y"),
            pat_lesson_lines("https://x/y").join("\n")
        );
    }

    // ------------------------------------------------------------------
    // Menu de IA: 4 opções, com a 4ª = "nenhuma por agora" (o default quando
    // o Ollama não está instalado; o caro nunca é default).
    // ------------------------------------------------------------------
    #[test]
    fn ia_menu_has_four_options_with_the_none_choice() {
        assert!(IA_MENU.contains("1) Ollama local"));
        assert!(IA_MENU.contains("2) Copilot CLI"));
        assert!(IA_MENU.contains("3) Microsoft Foundry"));
        assert!(IA_MENU.contains("4) nenhuma por agora"));
        assert!(IA_MENU.contains("custo por token"));
        assert!(IA_MENU.contains("funcionam sem IA"));
    }

    #[test]
    fn secret_prompt_only_claims_hiding_when_the_os_can_do_it() {
        assert!(
            secret_prompt_line("Cole o PAT", true, true).contains("não aparece na tela"),
            "onde o eco some, o texto pode prometer"
        );
        let honest = secret_prompt_line("Cole o PAT", true, false);
        assert!(honest.contains("pode ecoar"), "{honest}");
        assert!(
            !honest.contains("não aparece"),
            "sem mecanismo, nada de promessa falsa: {honest}"
        );
        assert_eq!(
            secret_prompt_line("Cole o PAT", false, false),
            "Cole o PAT:"
        );
    }

    #[test]
    fn menu_choice_defaults_and_repeats_on_invalid_answers() {
        // Enter vazio ⇒ default; EOF ⇒ default; inválida 2x ⇒ default com aviso.
        let mut input = BufReader::new(std::io::Cursor::new("\n"));
        let mut output: Vec<u8> = Vec::new();
        assert_eq!(
            ask_menu_choice(
                "menu\n",
                &["1", "2", "3", "4"],
                "4",
                &mut input,
                &mut output
            ),
            "4"
        );

        let mut input = BufReader::new(std::io::Cursor::new("x\nx\n"));
        let mut output: Vec<u8> = Vec::new();
        assert_eq!(
            ask_menu_choice(
                "menu\n",
                &["1", "2", "3", "4"],
                "2",
                &mut input,
                &mut output
            ),
            "2"
        );
        let text = String::from_utf8_lossy(&output).to_string();
        assert!(
            text.contains("resposta inválida ('x') — use 1, 2, 3, 4"),
            "deve listar as opções válidas: {text}"
        );
        assert!(
            text.contains("assumindo '2' (padrão)"),
            "deve avisar o default assumido: {text}"
        );

        let mut input = BufReader::new(std::io::Cursor::new(""));
        let mut output: Vec<u8> = Vec::new();
        assert_eq!(
            ask_menu_choice(
                "menu\n",
                &["1", "2", "3", "4"],
                "3",
                &mut input,
                &mut output
            ),
            "3"
        );
    }

    // ------------------------------------------------------------------
    // Resumo final: formato exato do MVP-2 (pronto / funciona agora /
    // desativado / ativar depois / próximo passo) — e nada de PAT: o setup
    // não fala de credencial.
    // ------------------------------------------------------------------
    #[test]
    fn summary_has_ready_works_disabled_and_next_command_sections() {
        let text = format_summary(&SummaryInput {
            dev: "Sergin",
            ia: Some("ollama (qwen3.5:4b)"),
            store_path: "/home/x/.config/kira/store.db",
            lang: "en",
            ingest: Some(IngestLine::Done(IngestSummary {
                files: 181,
                findings: Some(39),
                digest_path: ".kira/digest.md".to_string(),
            })),
            board_connected: false,
        });
        assert!(text.starts_with("pronto:\n  dev: Sergin\n"), "{text}");
        assert!(text.contains("  ia:  ollama (qwen3.5:4b)"));
        assert!(text.contains("  idioma: en (posts, cards e PR)"), "{text}");
        assert!(text.contains("  store: /home/x/.config/kira/store.db"));
        assert!(
            text.contains("  ingestão: 181 arquivo(s), 39 achado(s), digest .kira/digest.md"),
            "{text}"
        );
        assert!(text.contains("funciona agora:  scan · wiki · context · pr draft · dashboard"));
        assert!(
            text.contains("desativado:      judge de item do board · read (board) · publicar PR"),
            "{text}"
        );
        assert!(text.contains("ativar depois:   kira connect azure   (leitura de work items)"));
        // Com IA escolhida, a linha do backend não aparece em "ativar depois".
        assert!(!text.contains("kira setup --backend ollama"), "{text}");
        // M6: o próximo passo saiu do `kira scan` hardcoded.
        assert!(
            text.contains("próximo passo:   kira connect azure"),
            "{text}"
        );
        assert!(!text.contains("kira scan\n"), "{text}");
        assert!(
            !text.contains("configuração concluída"),
            "nunca termina com 'configuração concluída': {text}"
        );
        // A palavra PAT nunca entra no resumo do setup.
        assert!(!text.contains("PAT"), "{text}");
    }

    #[test]
    fn summary_without_ia_marks_none_and_the_activation_command() {
        let text = format_summary(&SummaryInput {
            dev: "dev",
            ia: None,
            store_path: "store.db",
            lang: "pt-BR",
            ingest: None,
            board_connected: true,
        });
        assert!(text.contains("  ia:  nenhuma"), "{text}");
        assert!(
            text.contains("                 kira setup --backend ollama"),
            "sem IA, o ativar depois traz o comando: {text}"
        );
        // Board conectado: o próximo passo é o triage, não o connect.
        assert!(text.contains("próximo passo:   kira triage <id>"), "{text}");
        // Sem decisão de ingestão (motor de teste), a linha não aparece.
        assert!(!text.contains("ingestão:"), "{text}");
        assert!(!text.contains("PAT"), "{text}");
    }

    #[test]
    fn ingest_line_texts_always_name_the_command_that_fixes_it() {
        assert_eq!(
            ingest_line_text(&IngestLine::Skipped),
            "ingestão: pulada — ativar depois: kira ingest"
        );
        let failed = ingest_line_text(&IngestLine::Failed("raiz inexistente".to_string()));
        assert!(failed.contains("não rodou (raiz inexistente)"), "{failed}");
        assert!(failed.contains("kira ingest"), "{failed}");
        // Sem findings, a linha diz a causa em vez de inventar zero.
        let done = ingest_line_text(&IngestLine::Done(IngestSummary {
            files: 2,
            findings: None,
            digest_path: ".kira/digest.md".to_string(),
        }));
        assert_eq!(
            done,
            "ingestão: 2 arquivo(s), sem scan (rode kira scan), digest .kira/digest.md"
        );
    }

    #[test]
    fn validate_lang_accepts_codes_and_rejects_everything_loose() {
        assert_eq!(validate_lang("en"), Some("en".to_string()));
        assert_eq!(validate_lang(" pt-BR "), Some("pt-BR".to_string()));
        assert_eq!(validate_lang("zh-Hans"), Some("zh-Hans".to_string()));
        assert_eq!(validate_lang("e"), None, "um caractere não é código");
        assert_eq!(validate_lang(""), None);
        assert_eq!(validate_lang("pt BR"), None, "espaço não entra no config");
        assert_eq!(validate_lang("-pt"), None);
        assert_eq!(validate_lang("pt-"), None);
        assert_eq!(
            validate_lang("pt-BR-brasil-extra-longo-demais"),
            None,
            "> 20"
        );
        assert_eq!(validate_lang("pt_BR"), None, "só hífen, não underscore");
    }

    #[test]
    fn resume_commands_point_to_the_right_flags() {
        // A credencial agora nasce em `connect` (MVP-3), não no setup.
        assert_eq!(resume_command("pat"), "kira connect azure");
        assert_eq!(resume_command("ia"), "kira setup --backend ollama");
        // M6: a ingestão pulada se reativa com o comando próprio.
        assert_eq!(resume_command("ingest"), "kira ingest");
        // Passo desconhecido cai no comando neutro (nada de fantasma).
        assert_eq!(resume_command("fonte"), "kira setup");
    }

    // ------------------------------------------------------------------
    // Nome do dev (comportamento existente, preservado).
    // ------------------------------------------------------------------
    #[test]
    fn default_dev_name_capitalizes_the_os_user() {
        let env = env_from(&[("USER", "sergin")]);
        assert_eq!(default_dev_name_from_env(&env), "Sergin");

        let env = env_from(&[("USER", "MARIA")]);
        assert_eq!(default_dev_name_from_env(&env), "MARIA");

        // USERNAME entra quando USER está ausente.
        let env = env_from(&[("USERNAME", "joao.silva")]);
        assert_eq!(default_dev_name_from_env(&env), "Joao.silva");

        // USER vence USERNAME.
        let env = env_from(&[("USER", "sergin"), ("USERNAME", "joao.silva")]);
        assert_eq!(default_dev_name_from_env(&env), "Sergin");
    }

    #[test]
    fn default_dev_name_falls_back_to_dev() {
        let env = env_from(&[]);
        assert_eq!(default_dev_name_from_env(&env), "dev");

        let env = env_from(&[("USER", "   ")]);
        assert_eq!(default_dev_name_from_env(&env), "dev");

        let env = env_from(&[("USERNAME", "")]);
        assert_eq!(default_dev_name_from_env(&env), "dev");
    }

    #[test]
    fn validate_dev_name_accepts_and_trims() {
        assert_eq!(validate_dev_name("Sergin"), Some("Sergin".into()));
        assert_eq!(validate_dev_name("  Sergin  "), Some("Sergin".into()));
        assert_eq!(validate_dev_name("Ana"), Some("Ana".into()));

        // Exatamente 40 caracteres ainda é válido.
        let forty = "a".repeat(40);
        assert_eq!(validate_dev_name(&forty), Some(forty.clone()));
        // 41 já não é.
        let forty_one = format!("{forty}a");
        assert_eq!(validate_dev_name(&forty_one), None);
    }

    #[test]
    fn validate_dev_name_rejects_empty_control_and_newlines() {
        assert_eq!(validate_dev_name(""), None);
        assert_eq!(validate_dev_name("   "), None);
        // Quebra de linha nas pontas é whitespace: trim, não rejeição.
        assert_eq!(validate_dev_name("sergin\n"), Some("sergin".into()));
        // Control chars e quebras NO MEIO são rejeitados.
        assert_eq!(validate_dev_name("ser\ngin"), None);
        assert_eq!(validate_dev_name("ser\r\ngin"), None);
        assert_eq!(validate_dev_name("ser\tgin"), None);
    }
}
