//! `kira setup` — o wizard que ENSINA (ADR-0025).
//!
//! Princípio da spec: cada pergunta vem com **o que é**, **por que pedimos**,
//! **como fazer** e **o que acontece se pular** — e nunca bloqueia: tudo tem
//! caminho "depois".
//!
//! Fluxo: org/project (flags > env > config.toml > prompt) → passo PAT: a aula
//! ("O que é um PAT", analogia da senha descartável + passo a passo com a URL
//! `https://dev.azure.com/<org>/_usersSettings/tokens`, escopo Work Items →
//! Read, validade 30/90 dias, aviso de valor único) impressa ANTES de qualquer
//! pedido → menu obrigatório de 3 opções (1 colar PAT [default em TTY],
//! 2 usar `AZURE_DEVOPS_PAT` do ambiente, 3 pular ⇒ modo local) → validação
//! IMEDIATA com leitura barata (lista de projetos) e as 4 mensagens exatas da
//! spec (200 / 401 / 403 / sem rede). Entrada de token SEM ECO (melhor esforço
//! via `stty -echo` no Unix; o valor digitado nunca é impresso — nada vai para
//! stdout/stderr/log e nunca para prompt de LLM). Um 401/403 NÃO grava; falha
//! de rede grava e segue (o `kira doctor` completa o diagnóstico depois).
//! → passo de backend de IA com custo/dados em cada opção e pré-requisito
//! verificado (ollama list contém `qwen3.5:4b`; `copilot` no PATH; Foundry
//! pede endpoint + deployment + chave mascarada e faz UMA chamada de teste
//! barata reportando 401/403/404). Regra: backend caro NUNCA é default nem
//! fallback silencioso. → resumo final no formato `pronto: / pulado (dá para
//! fazer depois): / próximo passo:  kira scan` — nunca "configuração
//! concluída".
//!
//! Todo input/output é injetado (stdin, stdout, store, config path, board
//! factory, hooks de IA), então os testes dirigem o fluxo completo com stdin
//! falso, um store [`crate::credentials::InMemory`] e um servidor ADO falso —
//! o keyring do OS nunca é tocado em testes.

use std::future::Future;
use std::io::{BufRead, Write};
use std::path::Path;
use std::pin::Pin;
use std::process::ExitCode;

/// Wizard de tela cheia (ADR-0025 + layout do opencode): coleta as respostas e
/// devolve `SetupOptions` para o motor de texto abaixo executar.
pub mod wizard;

use crate::board::{self, BoardProvider};
use crate::credentials::{
    self, CONFIG_KEY_DEV_NAME, CONFIG_KEY_ORG, CONFIG_KEY_PROJECT, CRED_KEY_PAT, CredStore,
    ORG_ENV, PAT_ENV, PROJECT_ENV,
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
    /// `--org <nome>`
    pub org: Option<String>,
    /// `--project <nome>`
    pub project: Option<String>,
    /// `--reset`: remove o que o setup guardou como credencial (PAT e chave do
    /// Foundry no keyring, endpoint/deployment no `config.toml`) e sai.
    pub reset: bool,
    /// `--pat <token>`: PAT passed straight on the command line (skips the
    /// aula, o menu e o prompt; o valor nunca é ecoado — só a máscara).
    pub pat: Option<String>,
    /// stdin é interativo (TTY), injetado pelo binário real via
    /// `stdin().is_terminal()`; só no modo interativo o menu do PAT aparece.
    pub interactive: bool,
    /// `--name <nome>`: como o dev quer ser chamado (salvo como `dev_name` em
    /// `config.toml`); vence o nome salvo e a pergunta em TTY. Nunca entra em
    /// prompt de LLM.
    pub name: Option<String>,
    /// `--yes`: modo não-interativo mesmo em TTY — usa a env do PAT se
    /// existir, senão pula, e imprime o pulado. Nunca bloqueia.
    pub yes: bool,
    /// `--backend <ollama|copilot|foundry>`: escolha EXPLÍCITA do backend de
    /// IA (o caro nunca é default nem fallback silencioso).
    pub backend: Option<String>,
    /// `--skip <passo>` (repetível): pula um passo — `pat` ou `ia`.
    pub skip: Vec<String>,
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

/// Resultado do passo PAT antes da validação.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PatResult {
    /// Token colado (opção 1 ou `--pat`).
    Pasted(String),
    /// Token já presente em `AZURE_DEVOPS_PAT` (opção 2).
    FromEnv(String),
    /// Token do keyring do OS (setup anterior, ou o wizard mantendo o que já
    /// existia): é VALIDADO, mas não é regravado.
    FromStore(String),
    /// Pular por agora ⇒ modo local (opção 3).
    Skipped,
}

/// Escolha do backend de IA no setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IaChoice {
    Ollama,
    Copilot,
    Foundry,
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
    let organization = match resolve_input(
        options.org.as_deref(),
        env(ORG_ENV),
        config_pairs
            .as_ref()
            .and_then(|pairs| value_of(pairs, CONFIG_KEY_ORG)),
        "Organização do Azure DevOps (ex.: minha-org): ",
        input,
        output,
    ) {
        Some(value) => value,
        None => return ExitCode::FAILURE,
    };
    let project = match resolve_input(
        options.project.as_deref(),
        env(PROJECT_ENV),
        config_pairs
            .as_ref()
            .and_then(|pairs| value_of(pairs, CONFIG_KEY_PROJECT)),
        "Projeto do Azure DevOps (ex.: MeuProjeto): ",
        input,
        output,
    ) {
        Some(value) => value,
        None => return ExitCode::FAILURE,
    };

    // Valores explícitos inválidos falham de cara, sem adivinhar: --skip só
    // aceita 'pat'/'ia' e --backend só ollama/copilot/foundry (nada de
    // fallback silencioso).
    for skipped_step in &options.skip {
        let step = skipped_step.trim().to_ascii_lowercase();
        if step != "pat" && step != "ia" {
            let _ = writeln!(
                output,
                "erro: --skip aceita 'pat' ou 'ia' (recebi '{skipped_step}')."
            );
            return ExitCode::FAILURE;
        }
    }
    if let Some(name) = &options.backend {
        let parsed = name.trim().to_ascii_lowercase();
        if parsed != "ollama" && parsed != "copilot" && parsed != "foundry" {
            let _ = writeln!(
                output,
                "erro: backend desconhecido '{name}' — use ollama, copilot ou foundry."
            );
            return ExitCode::FAILURE;
        }
    }

    // ------------------------------------------------------------------
    // Passo PAT: a aula vem sempre antes de qualquer pedido (ADR-0025).
    // Com `--pat` o token já existe — zero aula, zero menu, zero prompt.
    // ------------------------------------------------------------------
    let pat_result = if let Some(pat) = options.pat.as_deref() {
        PatResult::Pasted(pat.to_string())
    } else {
        let url = pat_url(&organization);
        print_pat_lesson(&url, output);
        if options
            .skip
            .iter()
            .any(|item| item.trim().eq_ignore_ascii_case("pat"))
        {
            let _ = writeln!(
                output,
                "PAT pulado (--skip) — o KIRA funciona em modo local (kira triage --file)."
            );
            PatResult::Skipped
        } else if options.interactive && !options.yes {
            match pat_menu_interactive(env, options.hidden_input, input, output) {
                Some(result) => result,
                // Entrada vazia no prompt de colar: erro acionável e fim.
                None => return ExitCode::FAILURE,
            }
        } else {
            // Não-interativo (--yes/sem TTY): env > keyring > pular. O PAT do
            // keyring entra aqui porque o wizard pode ter mantido o que já
            // existia — reusar é o que o dono espera, e nada é bloqueante.
            let non_interactive = pat_from_env_store_or_skip(env(PAT_ENV), stored_pat(store));
            match &non_interactive {
                PatResult::FromEnv(_) => {
                    let _ = writeln!(
                        output,
                        "usando AZURE_DEVOPS_PAT do ambiente (o PAT já está nesta máquina)"
                    );
                }
                PatResult::FromStore(_) => {
                    let _ = writeln!(
                        output,
                        "usando o PAT do keyring do OS (salvo por um `kira setup` anterior)"
                    );
                }
                PatResult::Skipped => {
                    let _ = writeln!(
                        output,
                        "PAT pulado — o KIRA funciona em modo local (kira triage --file)."
                    );
                }
                // Colar por stdin não existe fora de TTY: a resolução acima
                // nunca devolve esse caso.
                PatResult::Pasted(_) => {}
            }
            non_interactive
        }
    };

    // ------------------------------------------------------------------
    // Validação na hora, com rótulo explícito do que aconteceu (as 4
    // mensagens exatas da spec). Um 401/403 NÃO grava; falha de rede grava
    // e o fluxo segue (o doctor completa depois).
    // ------------------------------------------------------------------
    let mut pat_summary: Option<String> = None;
    let mut skipped_steps: Vec<&'static str> = Vec::new();
    match &pat_result {
        PatResult::Skipped => {
            skipped_steps.push("pat");
        }
        PatResult::Pasted(token) | PatResult::FromEnv(token) | PatResult::FromStore(token) => {
            let is_pasted = matches!(pat_result, PatResult::Pasted(_));
            // Origem do token, para o resumo não dizer "da env" quando veio
            // do keyring (nem o contrário).
            let origin = match &pat_result {
                PatResult::FromStore(_) => "do keyring do OS",
                _ => "da env AZURE_DEVOPS_PAT",
            };
            let _ = writeln!(output, "Validando o PAT na organização {organization}...");
            let board = match make_board(token, &organization, &project) {
                Ok(board) => board,
                Err(err) => {
                    let _ = writeln!(output, "erro: {err}");
                    return ExitCode::FAILURE;
                }
            };
            match board.project_names().await {
                Ok(project_names) => {
                    if doctor::find_project_case_insensitive(&project, &project_names).is_none() {
                        let _ = writeln!(output);
                        let _ = writeln!(
                            output,
                            "{}",
                            doctor::format_project_check(&project, &project_names)
                        );
                        let _ = writeln!(
                            output,
                            "projeto incorreto — o token NÃO foi salvo; rode `kira setup` de novo com o nome exato."
                        );
                        return ExitCode::FAILURE;
                    }
                    let _ = writeln!(output, "{}", msg_pat_ok(&organization, &project));
                    if is_pasted {
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
                        pat_summary = Some(format!(
                            "{} (lê Work Items de {organization}/{project})",
                            doctor::mask_pat(token)
                        ));
                    } else {
                        // Token da env/keyring fica onde está (env vence o
                        // keyring na resolução); gravar seria cópia inútil.
                        pat_summary = Some(format!(
                            "{} ({origin} — lê Work Items de {organization}/{project})",
                            doctor::mask_pat(token)
                        ));
                    }
                }
                Err(Error::AdoUnauthorized { .. }) => {
                    let _ = writeln!(output, "{}", msg_pat_401(&organization));
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
                    if is_pasted {
                        if let Err(err) = store.set(CRED_KEY_PAT, token) {
                            let _ = writeln!(output, "erro: {err}");
                            return ExitCode::FAILURE;
                        }
                        pat_summary = Some(format!(
                            "{} (salvo; validação pendente — rode kira doctor)",
                            doctor::mask_pat(token)
                        ));
                    } else {
                        pat_summary = Some(format!(
                            "{} ({origin}; validação pendente — rode kira doctor)",
                            doctor::mask_pat(token)
                        ));
                    }
                    let from_store = matches!(pat_result, PatResult::FromStore(_));
                    let _ = writeln!(output, "{}", msg_pat_rede(is_pasted, from_store));
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Nome do dev: `--name` > nome já salvo em `config.toml` > pergunta (só
    // em TTY) > padrão do SO. Sem TTY nunca bloqueia. Nunca entra em prompt
    // de LLM — só na saída humana (setup/doctor).
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

    // org/projeto/nome vão para o config.toml mesmo com PAT pulado — assim o
    // `kira doctor` e a retomada (`kira setup --pat`) já têm o contexto.
    match config_path {
        Some(path) => {
            if let Err(err) = credentials::write_config_file(
                path,
                &[
                    (CONFIG_KEY_ORG, &organization),
                    (CONFIG_KEY_PROJECT, &project),
                    (CONFIG_KEY_DEV_NAME, &dev_name),
                ],
            ) {
                let _ = writeln!(output, "aviso: não salvei org/projeto no config: {err}");
            }
        }
        None => {
            let _ = writeln!(
                output,
                "aviso: sem diretório home, org/projeto não foram salvos em config.toml"
            );
        }
    }

    // ------------------------------------------------------------------
    // Passo backend de IA: custo e dados explicados em cada opção e
    // pré-requisito verificado. Backend caro NUNCA é default nem fallback.
    // ------------------------------------------------------------------
    let mut ia_summary: Option<String> = None;
    if options
        .skip
        .iter()
        .any(|item| item.trim().eq_ignore_ascii_case("ia"))
    {
        let _ = writeln!(output, "IA pulada (--skip) — dê para escolher depois.");
        skipped_steps.push("ia");
    } else {
        let choice = if let Some(name) = options.backend.as_deref() {
            // Escolha explícita vence (mesmo em TTY): o caro só por flag.
            match name.trim().to_ascii_lowercase().as_str() {
                "ollama" => IaChoice::Ollama,
                "copilot" => IaChoice::Copilot,
                _ => IaChoice::Foundry,
            }
        } else if options.interactive && !options.yes {
            match ask_menu_choice(IA_MENU, &["1", "2", "3"], "1", input, output).as_str() {
                "2" => IaChoice::Copilot,
                "3" => IaChoice::Foundry,
                _ => IaChoice::Ollama,
            }
        } else {
            // Sem TTY e sem flag: o default é o barato (Ollama local) —
            // nunca um backend caro por omissão.
            IaChoice::Ollama
        };
        match choice {
            IaChoice::Ollama => match (ia_hooks.ollama_models)() {
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
                        "{DEFAULT_MODEL} não está no ollama — rode: ollama pull {DEFAULT_MODEL} (3,3 GB)"
                    );
                    skipped_steps.push("ia");
                }
                None => {
                    let _ = writeln!(
                        output,
                        "ollama não encontrado nesta máquina — instale (https://ollama.com) e rode: ollama pull {DEFAULT_MODEL} (3,3 GB)"
                    );
                    skipped_steps.push("ia");
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
                        "copilot não está no PATH — instale/autentique o Copilot CLI (pré-requisito: a política da sua empresa permitir)"
                    );
                    skipped_steps.push("ia");
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
                                    "erro: chave recusada pelo Foundry (HTTP {status}) — a IA fica pulada; rode kira setup de novo para tentar outra chave."
                                );
                                skipped_steps.push("ia");
                            }
                            Err(Error::FoundryDeployment) => {
                                let _ = writeln!(
                                    output,
                                    "erro: HTTP 404 do Foundry — confira KIRA_FOUNDRY_ENDPOINT e KIRA_FOUNDRY_DEPLOYMENT; a IA fica pulada."
                                );
                                skipped_steps.push("ia");
                            }
                            Err(err) => {
                                let _ = writeln!(
                                    output,
                                    "erro: {err} — a IA fica pulada; rode kira setup de novo."
                                );
                                skipped_steps.push("ia");
                            }
                        }
                    }
                    // Sem endpoint/deployment/chave: erro acionável SEM
                    // nenhuma chamada de rede.
                    None => return ExitCode::FAILURE,
                }
            }
        }
    }

    // O passo "fonte Nerd" saiu: o resumo mandava rodar `kira setup --deps
    // --only font`, comando que o binário nunca teve. Prometer retomada
    // inexistente é pior que não oferecer a etapa.

    // ------------------------------------------------------------------
    // Resumo final: o que ficou pronto, o que ficou pulado (com o comando
    // de retomada) e o PRÓXIMO comando — nunca "configuração concluída".
    // ------------------------------------------------------------------
    let store_line = store::db_path()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "indisponível (sem HOME)".to_string());
    let _ = writeln!(output);
    let _ = writeln!(
        output,
        "{}",
        format_summary(
            &dev_name,
            pat_summary.as_deref(),
            ia_summary.as_deref(),
            &store_line,
            &skipped_steps,
        )
    );
    ExitCode::SUCCESS
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

/// First non-empty source: flag > env > config; otherwise prompts stdin.
/// Returns `None` (after an actionable message) when nothing was provided.
fn resolve_input(
    flag: Option<&str>,
    env_value: Option<String>,
    config_value: Option<String>,
    prompt: &str,
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> Option<String> {
    for value in [flag.map(str::to_string), env_value, config_value] {
        if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
            return Some(value);
        }
    }
    let _ = write!(output, "{prompt}");
    let _ = output.flush();
    match read_line(input) {
        Some(value) if !value.is_empty() => Some(value),
        _ => {
            let _ = writeln!(
                output,
                "erro: valor não informado — rode `kira setup` e digite o valor pedido."
            );
            None
        }
    }
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
    format!(
        "O que é um PAT\n\
         PAT é uma senha descartável, feita para uma coisa só. Em vez de entregar a\n\
         senha da sua conta para o KIRA, você cria um Personal Access Token: um código\n\
         que permite apenas ler Work Items do projeto que você escolher, e que expira\n\
         na data que você definir. Se ele vazar, você apaga aquele código — sua conta e\n\
         o resto do Azure DevOps continuam intactos. É por isso que ele é mais seguro\n\
         que a senha.\n\
         \n\
         Como criar (2 minutos)\n\
         1. Abra {url}\n\
         2. New Token → nome: kira-local → validade: 30 dias (ou 90, se preferir)\n\
         3. Em Scopes, marque Work Items → Read. Só isso. Nada mais é necessário.\n\
         4. Clique Create e copie o valor agora — ele aparece uma única vez.\n"
    )
}

/// Imprime a aula do PAT antes de qualquer pedido de token.
fn print_pat_lesson(url: &str, output: &mut dyn Write) {
    let _ = writeln!(output, "{}", pat_lesson(url));
}

/// Menu obrigatório do PAT (texto exato da spec; `1` é o default em TTY).
pub const PAT_MENU: &str = "Como o KIRA vai falar com o Azure DevOps?\n\
     1) Colar um PAT agora            (recomendado — leitura de Work Items)\n\
     2) Usar AZURE_DEVOPS_PAT do ambiente (o token já está configurado nesta máquina)\n\
     3) Pular por agora              (o KIRA funciona em modo local: kira triage --file)\n";

/// Menu do backend de IA (texto exato da spec; `1` é o default — o caro
/// nunca é escolhido por omissão).
pub const IA_MENU: &str = "Como o KIRA deve pensar?\n\
     1) Ollama local            (recomendado: 0 custo, o código não sai da sua máquina)\n\
     2) Copilot CLI             (usa sua assinatura; mesma política da sua empresa)\n\
     3) Microsoft Foundry       (BYOK: custo por token; precisa endpoint + deployment + chave)\n";

/// Menu do PAT em TTY: opção 1 colar (entrada oculta), 2 usar a env, 3 pular.
/// Enter/EOF ⇒ 1 (default). Resposta inválida repete no máximo 2x e assume o
/// default com aviso. A opção 2 sem a env avisa e volta ao menu (nunca sai
/// silenciosamente para outra opção).
fn pat_menu_interactive(
    env: &dyn Fn(&str) -> Option<String>,
    hidden: bool,
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> Option<PatResult> {
    loop {
        match ask_menu_choice(PAT_MENU, &["1", "2", "3"], "1", input, output).as_str() {
            "2" => match env(PAT_ENV).filter(|pat| !pat.trim().is_empty()) {
                Some(pat) => {
                    let _ = writeln!(
                        output,
                        "usando AZURE_DEVOPS_PAT do ambiente (o token já está nesta máquina)"
                    );
                    return Some(PatResult::FromEnv(pat));
                }
                None => {
                    let _ = writeln!(
                        output,
                        "AZURE_DEVOPS_PAT não está definida nesta máquina — defina-a ({}) ou escolha 1.",
                        doctor::set_env_command(PAT_ENV, "...")
                    );
                }
            },
            "3" => {
                let _ = writeln!(
                    output,
                    "PAT pulado — o KIRA funciona em modo local (kira triage --file)."
                );
                return Some(PatResult::Skipped);
            }
            _ => {
                return prompt_for_pat(input, output, hidden).map(PatResult::Pasted);
            }
        }
    }
}

/// Seleção do PAT fora de TTY (pura, testável): env > keyring > pular.
///
/// O keyring entra na conta porque o wizard pode ter MANTIDO o PAT que já
/// existia — pedir para colar de novo o token que está salvo seria trabalho
/// inventado. Nunca bloqueia e nunca inventa default caro.
fn pat_from_env_store_or_skip(env_pat: Option<String>, stored: Option<String>) -> PatResult {
    match env_pat.filter(|pat| !pat.trim().is_empty()) {
        Some(pat) => PatResult::FromEnv(pat),
        None => match stored.filter(|pat| !pat.trim().is_empty()) {
            Some(pat) => PatResult::FromStore(pat),
            None => PatResult::Skipped,
        },
    }
}

/// PAT guardado no keyring, quando existir (reuso; o valor nunca é impresso).
fn stored_pat(store: &dyn CredStore) -> Option<String> {
    store.get(CRED_KEY_PAT).ok().flatten()
}

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
                let _ = writeln!(output, "resposta inválida ('{line}') — use 1, 2 ou 3.");
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

/// Prompt "Cole o PAT": uma linha de stdin; vazia ⇒ erro acionável.
fn prompt_for_pat(input: &mut dyn BufRead, output: &mut dyn Write, hidden: bool) -> Option<String> {
    prompt_secret("Cole o PAT e pressione Enter", hidden, input, output)
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

/// Comando de retomada de cada passo pulado (o resumo imprime os dois juntos).
pub fn resume_command(step: &str) -> &'static str {
    match step {
        "pat" => "kira setup --pat",
        "ia" => "kira setup --backend ollama",
        _ => "kira setup",
    }
}

/// Resumo final (formato exato da spec): `pronto:` com dev/pat mascarado/ia/
/// store, `pulado (dá para fazer depois):` com os comandos de retomada e o
/// próximo comando — nunca termina com "configuração concluída".
pub fn format_summary(
    dev: &str,
    pat_line: Option<&str>,
    ia_line: Option<&str>,
    store_path: &str,
    skipped: &[&str],
) -> String {
    let mut text = String::new();
    text.push_str("pronto:\n");
    text.push_str(&format!("  dev: {dev}\n"));
    let pat = pat_line.unwrap_or("não configurado (modo local)");
    text.push_str(&format!("  pat: {pat}\n"));
    let ia = ia_line.unwrap_or("não configurada (rode kira setup --backend)");
    text.push_str(&format!("  ia:  {ia}\n"));
    text.push_str(&format!("  store: {store_path}\n"));
    if !skipped.is_empty() {
        text.push('\n');
        text.push_str("pulado (dá para fazer depois):\n");
        for step in skipped {
            text.push_str(&format!("  {step} — {}\n", resume_command(step)));
        }
    }
    text.push('\n');
    text.push_str("próximo passo:  kira scan");
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

/// Board factory used by the real binary: the Azure DevOps provider selected
/// by `KIRA_BOARD` (default `azure`).
pub fn default_board_factory(
    pat: &str,
    organization: &str,
    project: &str,
) -> Result<Box<dyn BoardProvider>, Error> {
    board::for_board(&board::name_from_env(), pat, organization, project)
}

/// Env lookup used by the real binary.
pub fn env_lookup(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
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
    // A aula do PAT: o que é, por que, como fazer (URL + escopo + validade
    // + valor único) — antes de qualquer pedido.
    // ------------------------------------------------------------------
    #[test]
    fn pat_lesson_teaches_what_how_and_single_use() {
        let lesson = pat_lesson("https://dev.azure.com/minha-org/_usersSettings/tokens");
        assert!(lesson.contains("senha descartável"), "aula: {lesson}");
        assert!(lesson.contains("Personal Access Token"), "aula: {lesson}");
        assert!(lesson.contains("https://dev.azure.com/minha-org/_usersSettings/tokens"));
        assert!(lesson.contains("Work Items → Read"), "aula: {lesson}");
        assert!(lesson.contains("30 dias"), "aula: {lesson}");
        assert!(lesson.contains("90"), "aula: {lesson}");
        assert!(lesson.contains("uma única vez"), "aula: {lesson}");
    }

    // ------------------------------------------------------------------
    // Menus: 3 opções com o default certo (1 = barato/recomendado; o caro
    // nunca é default).
    // ------------------------------------------------------------------
    #[test]
    fn menus_have_three_options_and_the_cheap_default() {
        assert!(PAT_MENU.contains("1) Colar um PAT agora"));
        assert!(PAT_MENU.contains("2) Usar AZURE_DEVOPS_PAT do ambiente"));
        assert!(PAT_MENU.contains("3) Pular por agora"));
        assert!(PAT_MENU.contains("kira triage --file"));
        assert!(IA_MENU.contains("1) Ollama local"));
        assert!(IA_MENU.contains("2) Copilot CLI"));
        assert!(IA_MENU.contains("3) Microsoft Foundry"));
        assert!(IA_MENU.contains("custo por token"));
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
            ask_menu_choice("menu\n", &["1", "2", "3"], "1", &mut input, &mut output),
            "1"
        );

        let mut input = BufReader::new(std::io::Cursor::new("x\nx\n"));
        let mut output: Vec<u8> = Vec::new();
        assert_eq!(
            ask_menu_choice("menu\n", &["1", "2", "3"], "2", &mut input, &mut output),
            "2"
        );
        let text = String::from_utf8_lossy(&output).to_string();
        assert!(
            text.contains("assumindo '2' (padrão)"),
            "deve avisar o default assumido: {text}"
        );

        let mut input = BufReader::new(std::io::Cursor::new(""));
        let mut output: Vec<u8> = Vec::new();
        assert_eq!(
            ask_menu_choice("menu\n", &["1", "2", "3"], "3", &mut input, &mut output),
            "3"
        );
    }

    // ------------------------------------------------------------------
    // Não-interativo: usa a env quando existe; senão pula (nunca bloqueia,
    // nunca inventa token).
    // ------------------------------------------------------------------
    #[test]
    fn non_interactive_pat_prefers_env_then_the_keyring_then_skips() {
        assert_eq!(
            pat_from_env_store_or_skip(Some("tok".into()), Some("keyring".into())),
            PatResult::FromEnv("tok".into()),
            "env vence o keyring"
        );
        assert_eq!(
            pat_from_env_store_or_skip(None, Some("keyring".into())),
            PatResult::FromStore("keyring".into()),
            "o PAT salvo é reusado e validado"
        );
        assert_eq!(pat_from_env_store_or_skip(None, None), PatResult::Skipped);
        assert_eq!(
            pat_from_env_store_or_skip(Some("   ".into()), Some("  ".into())),
            PatResult::Skipped
        );
    }

    // ------------------------------------------------------------------
    // Resumo final: formato exato da spec (pronto / pulado / próximo passo)
    // e pat mascarado — nunca o token completo.
    // ------------------------------------------------------------------
    #[test]
    fn summary_has_ready_skipped_and_next_command_sections() {
        let text = format_summary(
            "Sergin",
            Some("Aa2x**** (lê Work Items de avanade/pague-menos)"),
            Some("ollama (qwen3.5:4b)"),
            "/home/x/.config/kira/store.db",
            &["pat"],
        );
        assert!(text.starts_with("pronto:\n  dev: Sergin\n"), "{text}");
        assert!(text.contains("  pat: Aa2x**** (lê Work Items de avanade/pague-menos)"));
        assert!(text.contains("  ia:  ollama (qwen3.5:4b)"));
        assert!(text.contains("  store: /home/x/.config/kira/store.db"));
        assert!(text.contains("pulado (dá para fazer depois):"));
        assert!(text.contains("  pat — kira setup --pat"));
        // Nenhuma etapa inventada no "pulado": sem fonte, sem comando fantasma.
        assert!(!text.contains("--deps"), "{text}");
        assert!(text.contains("próximo passo:  kira scan"));
        assert!(
            !text.contains("configuração concluída"),
            "nunca termina com 'configuração concluída': {text}"
        );
    }

    #[test]
    fn summary_without_pat_and_ia_marks_local_mode() {
        let text = format_summary("dev", None, None, "store.db", &["pat", "ia"]);
        assert!(
            text.contains("  pat: não configurado (modo local)"),
            "{text}"
        );
        assert!(
            text.contains("  ia:  não configurada (rode kira setup --backend)"),
            "{text}"
        );
        assert!(
            text.contains("  ia — kira setup --backend ollama"),
            "{text}"
        );
    }

    #[test]
    fn resume_commands_point_to_the_right_flags() {
        assert_eq!(resume_command("pat"), "kira setup --pat");
        assert_eq!(resume_command("ia"), "kira setup --backend ollama");
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
