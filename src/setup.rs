//! `kira setup` — interactive first-time configuration.
//!
//! Flow: org/project (flags > env > config.toml > prompt) → short instructions
//! with the PAT URL (opened in the browser) → PAT read from stdin (visible, one
//! line; works piped) → validate against the board BEFORE saving → save to the
//! credential store + org/project to `config.toml` (0600).
//!
//! Every input/output is injected (stdin, stdout, store, config path, board
//! factory, browser opener), so tests drive the full flow with fake stdin, an
//! [`crate::credentials::InMemory`] store and a fake Azure DevOps server — the
//! OS keyring is never touched in tests.

use std::io::{BufRead, Write};
use std::path::Path;
use std::process::ExitCode;

use crate::board::{self, BoardProvider};
use crate::credentials::{
    self, CONFIG_KEY_ORG, CONFIG_KEY_PROJECT, CRED_KEY_PAT, CredStore, ORG_ENV, PROJECT_ENV,
};
use crate::doctor;
use crate::error::Error;

/// Flags accepted by `kira setup`.
#[derive(Debug, Default, Clone)]
pub struct SetupOptions {
    /// `--org <nome>`
    pub org: Option<String>,
    /// `--project <nome>`
    pub project: Option<String>,
    /// `--reset`: remove the PAT from the store and exit.
    pub reset: bool,
}

/// Builds the board used to validate the PAT: `(pat, org, project) -> board`.
pub type BoardFactory<'a> = dyn Fn(&str, &str, &str) -> Result<Box<dyn BoardProvider>, Error> + 'a;

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
    open_url: &dyn Fn(&str) -> Result<(), std::io::Error>,
) -> ExitCode {
    if options.reset {
        return reset(store, output);
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

    // Step-by-step instructions with the PAT URL, opened in the browser.
    let url = format!("https://dev.azure.com/{organization}/_usersSettings/tokens");
    let _ = writeln!(
        output,
        "Para o KIRA ler work items, crie um Personal Access Token (PAT):"
    );
    let _ = writeln!(output, "  1. Abra {url}");
    match open_url(&url) {
        Ok(()) => {
            let _ = writeln!(output, "     (a URL já foi aberta no navegador)");
        }
        Err(_) => {
            let _ = writeln!(
                output,
                "     (não foi possível abrir o navegador — copie a URL acima)"
            );
        }
    }
    let _ = writeln!(
        output,
        "  2. Clique em \"+ New Token\" e marque os escopos:"
    );
    let _ = writeln!(output, "       Work Items (Read) + Project and Team (Read)");
    let _ = writeln!(output, "  3. Copie o token gerado.");

    let _ = writeln!(
        output,
        "Cole o PAT e pressione Enter (a entrada fica visível):"
    );
    let pat = match read_line(input) {
        Some(pat) if !pat.is_empty() => pat,
        _ => {
            let _ = writeln!(
                output,
                "erro: nenhum PAT informado — rode `kira setup` de novo."
            );
            return ExitCode::FAILURE;
        }
    };

    // Validate BEFORE saving: a bad PAT is never stored.
    let _ = writeln!(output, "Validando o PAT na organização {organization}...");
    let board = match make_board(&pat, &organization, &project) {
        Ok(board) => board,
        Err(err) => {
            let _ = writeln!(output, "erro: {err}");
            return ExitCode::FAILURE;
        }
    };
    let project_names = match board.project_names().await {
        Ok(names) => names,
        Err(Error::AdoUnauthorized { organization }) => {
            let _ = writeln!(output);
            let _ = writeln!(
                output,
                "PAT não aceito (401) — o token NÃO foi salvo. Revise:"
            );
            let _ = writeln!(output, "  1. o token foi colado por completo, sem espaços;");
            let _ = writeln!(
                output,
                "  2. os escopos Work Items (Read) e Project and Team (Read);"
            );
            let _ = writeln!(
                output,
                "  3. a organização {organization} está correta (token gerado nela)."
            );
            let _ = writeln!(output, "Depois rode `kira setup` de novo.");
            return ExitCode::FAILURE;
        }
        Err(err) => {
            let _ = writeln!(
                output,
                "erro: falha ao validar o PAT: {err} — o token NÃO foi salvo. Tente de novo."
            );
            return ExitCode::FAILURE;
        }
    };
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

    if let Err(err) = store.set(CRED_KEY_PAT, &pat) {
        let _ = writeln!(output, "erro: {err}");
        return ExitCode::FAILURE;
    }
    let _ = writeln!(
        output,
        "PAT salvo no keyring do OS ({} chars: {}).",
        pat.chars().count(),
        doctor::mask_pat(&pat)
    );

    match config_path {
        Some(path) => {
            if let Err(err) = credentials::write_config_file(
                path,
                &[
                    (CONFIG_KEY_ORG, &organization),
                    (CONFIG_KEY_PROJECT, &project),
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

    let _ = writeln!(output, "setup concluído — rode kira triage <id>");
    ExitCode::SUCCESS
}

/// `--reset`: removes the PAT from the store (idempotent).
fn reset(store: &dyn CredStore, output: &mut dyn Write) -> ExitCode {
    match store.remove(CRED_KEY_PAT) {
        Ok(()) => {
            let _ = writeln!(output, "PAT removido do keyring do OS.");
            ExitCode::SUCCESS
        }
        Err(err) => {
            let _ = writeln!(output, "erro: {err}");
            ExitCode::FAILURE
        }
    }
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

/// Opens `url` in the default browser (fire-and-forget; failures are
/// non-fatal: the URL is printed anyway).
pub fn open_url_in_browser(url: &str) -> Result<(), std::io::Error> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .map(|_| ())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map(|_| ())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map(|_| ())
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
