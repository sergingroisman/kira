//! `kira setup` — interactive first-time configuration.
//!
//! Flow: org/project (flags > env > config.toml > prompt) → in interactive
//! mode (TTY) a "Você já tem um PAT pronto?" question BEFORE anything browser:
//! yes ⇒ straight to the paste prompt; no ⇒ the instruction steps (PAT URL +
//! scopes) and a CONFIRMED browser open ("Abrir no navegador agora?"). The
//! open is fire-and-forget (spawn) and never blocks the terminal. Piped stdin
//! keeps the old fast path (steps + open, then paste); `--no-browser` skips
//! the steps/open entirely; `--pat` skips every token prompt. → PAT from
//! stdin (visible, one line) → validate against the board BEFORE saving →
//! save to the credential store + org/project to `config.toml` (0600). A PAT
//! passed via `--pat` is never echoed in the output: only the masked form
//! (first 4 chars + `****`) is ever shown.
//!
//! Depois do PAT validado e ANTES de salvar, o setup resolve o nome do dev:
//! `--name` > nome já salvo em `config.toml` (`dev_name`, com "Olá de novo,
//! <nome>.") > pergunta "Como devo te chamar? (Enter = <padrão>)" — só em
//! TTY — > padrão do SO (`USER`/`USERNAME` com inicial maiúscula; vazio ⇒
//! `dev`). Sem TTY (stdin pipado, `--pat`, CI) NUNCA bloqueia: usa o padrão
//! (ou `--name`) e registra a linha `dev: <nome>`. O nome nunca entra em
//! prompt de LLM — só na saída humana (setup/doctor).
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
    self, CONFIG_KEY_DEV_NAME, CONFIG_KEY_ORG, CONFIG_KEY_PROJECT, CRED_KEY_PAT, CredStore,
    ORG_ENV, PROJECT_ENV,
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
    /// `--pat <token>`: PAT passed straight on the command line (skips the
    /// stdin prompt; the value is never echoed in output, error or log).
    pub pat: Option<String>,
    /// `--no-browser`: skip the instruction steps and the browser open; go
    /// straight to the "paste the PAT" stdin prompt.
    pub no_browser: bool,
    /// stdin é interativo (TTY), injetado pelo binário real via
    /// `stdin().is_terminal()`; só no modo interativo o fluxo pergunta
    /// "Você já tem um PAT pronto?" antes de qualquer abertura de navegador —
    /// stdin pipado mantém o fast path antigo (passos + abrir, colar no fim).
    pub interactive: bool,
    /// `--name <nome>`: como o dev quer ser chamado (salvo como `dev_name` em
    /// `config.toml`); vence o nome salvo e a pergunta em TTY. Nunca entra em
    /// prompt de LLM.
    pub name: Option<String>,
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

    // Com `--pat` o token veio na linha de comando: nada de navegador, nada de
    // prompt. Com `--no-browser` os passos e o navegador são pulados. No modo
    // interativo (TTY) vem a pergunta "Você já tem um PAT pronto?": o navegador
    // só abre após confirmação. Stdin pipado mantém o fast path antigo.
    let pat = if let Some(pat) = options.pat.as_deref() {
        pat.to_string()
    } else if options.interactive && !options.no_browser {
        if ask_yes_no("Você já tem um PAT pronto? [S/n]: ", input, output) {
            // Já tem token: direto ao prompt de colar — sem passos, sem navegador.
            match prompt_for_pat(input, output) {
                Some(pat) => pat,
                None => return ExitCode::FAILURE,
            }
        } else {
            let url = pat_url(&organization);
            print_pat_steps(&url, output);
            if ask_yes_no("Abrir no navegador agora? [S/n]: ", input, output) {
                open_pat_page(&url, output, open_url);
            }
            match prompt_for_pat(input, output) {
                Some(pat) => pat,
                None => return ExitCode::FAILURE,
            }
        }
    } else {
        if !options.no_browser {
            let url = pat_url(&organization);
            print_pat_steps(&url, output);
            match open_url(&url) {
                Ok(()) => {
                    let _ = writeln!(output, "     (se não abrir, a URL está acima)");
                }
                Err(_) => {
                    let _ = writeln!(
                        output,
                        "     (não foi possível abrir o navegador — a URL está acima)"
                    );
                }
            }
        }
        match prompt_for_pat(input, output) {
            Some(pat) => pat,
            None => return ExitCode::FAILURE,
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

    // Nome do dev: `--name` > nome já salvo em `config.toml` > pergunta (só
    // em TTY) > padrão do SO. Roda DEPOIS de org/projeto e do PAT validado,
    // ANTES de salvar — assim uma falha de PAT/org/projeto nunca pergunta o
    // nome. Sem TTY nunca bloqueia esperando input: cai no padrão (ou flag)
    // e registra a linha `dev:`. O nome fica só na saída humana — nunca vai
    // para prompt de LLM (privacidade).
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
    } else if options.interactive {
        let default = default_dev_name_from_env(env);
        ask_dev_name(&default, input, output)
    } else {
        default_dev_name_from_env(env)
    };
    let _ = writeln!(output, "dev: {dev_name}");

    if let Err(err) = store.set(CRED_KEY_PAT, &pat) {
        let _ = writeln!(output, "erro: {err}");
        return ExitCode::FAILURE;
    }
    let _ = writeln!(
        output,
        "✓ PAT salvo no keyring do OS ({} chars: {}).",
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

/// URL da página de criação de PAT da organização.
fn pat_url(organization: &str) -> String {
    format!("https://dev.azure.com/{organization}/_usersSettings/tokens")
}

/// Passos de criação do token: URL da página e os escopos mínimos.
fn print_pat_steps(url: &str, output: &mut dyn Write) {
    let _ = writeln!(
        output,
        "Para o KIRA ler work items, crie um Personal Access Token (PAT):"
    );
    let _ = writeln!(output, "  1. Abra {url}");
    let _ = writeln!(
        output,
        "  2. Clique em \"+ New Token\" e marque os escopos:"
    );
    let _ = writeln!(output, "       Work Items (Read) + Project and Team (Read)");
    let _ = writeln!(output, "  3. Copie o token gerado.");
}

/// Pergunta 's'/'n' com default 's' (Enter); respostas inválidas repetem no
/// máximo 2 vezes e então assumem o padrão, com aviso.
fn ask_yes_no(question: &str, input: &mut dyn BufRead, output: &mut dyn Write) -> bool {
    let mut invalid = 0;
    loop {
        let _ = write!(output, "{question}");
        let _ = output.flush();
        match read_line(input) {
            Some(line) => match line.to_lowercase().as_str() {
                "" | "s" | "sim" | "y" | "yes" => return true,
                "n" | "não" | "nao" | "no" => return false,
                _ => {
                    invalid += 1;
                    let _ = writeln!(
                        output,
                        "resposta inválida ('{line}') — responda 's' ou 'n'."
                    );
                    if invalid >= 2 {
                        let _ = writeln!(
                            output,
                            "resposta inválida de novo — assumindo 's' (padrão)."
                        );
                        return true;
                    }
                }
            },
            None => return true,
        }
    }
}

/// Prompt "Cole o PAT": uma linha visível de stdin; vazia ⇒ erro acionável.
fn prompt_for_pat(input: &mut dyn BufRead, output: &mut dyn Write) -> Option<String> {
    let _ = writeln!(
        output,
        "Cole o PAT e pressione Enter (a entrada fica visível):"
    );
    match read_line(input) {
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

/// Abertura de navegador CONFIRMADA e nunca bloqueante: o opener injetado usa
/// `Command::spawn` (fire-and-forget, handle descartado); em qualquer falha do
/// spawn a URL é impressa de novo e o fluxo simplesmente continua.
fn open_pat_page(
    url: &str,
    output: &mut dyn Write,
    open_url: &dyn Fn(&str) -> Result<(), std::io::Error>,
) {
    let _ = writeln!(
        output,
        "Abrindo {url} no navegador (se não abrir, a URL está acima)..."
    );
    if open_url(url).is_err() {
        let _ = writeln!(
            output,
            "não foi possível abrir o navegador — cole esta URL: {url}"
        );
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

    fn env_from<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name: &str| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| value.to_string())
        }
    }

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
