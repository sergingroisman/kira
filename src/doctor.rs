//! Configuration diagnosis for `kira doctor`.
//!
//! All output formatting lives in pure functions so the ✓/✗ messages are
//! unit-tested with local data. The only network call goes through the
//! [`BoardProvider`] trait and happens exclusively after every required
//! setting is present, so an incomplete setup never touches the network.

use crate::board;
use crate::credentials::{self, Source};
use crate::error::Error;

/// Maximum number of existing project names listed when the configured
/// project is not found.
pub const MAX_LISTED_PROJECTS: usize = 10;

/// Masks a secret for display: at most the first 4 characters plus the
/// literal `****`. The full value is never shown.
pub fn mask_pat(pat: &str) -> String {
    let prefix: String = pat.chars().take(4).collect();
    format!("{prefix}****")
}

/// ✓ line for `AZURE_DEVOPS_PAT`, e.g.
/// `✓ AZURE_DEVOPS_PAT presente (34 chars: Aa2x****)`.
pub fn format_pat_env_check(pat: &str) -> String {
    format!(
        "✓ AZURE_DEVOPS_PAT presente ({} chars: {})",
        pat.chars().count(),
        mask_pat(pat)
    )
}

/// ✓ line for a PAT found in the OS keyring.
pub fn format_pat_keyring_check(pat: &str) -> String {
    format!(
        "✓ AZURE_DEVOPS_PAT presente no keyring do OS ({} chars: {})",
        pat.chars().count(),
        mask_pat(pat)
    )
}

/// ✓ line for a plain setting resolved from an explicit source.
pub fn format_setting_source_check(name: &str, value: &str, source: Source) -> String {
    match source {
        Source::Env => format!("✓ {name} ({value})"),
        other => format!("✓ {name} ({value} — de {})", other.label()),
    }
}

/// ✓ line for a plain (non-secret) environment variable.
pub fn format_env_ok(name: &str, value: &str) -> String {
    format!("✓ {name} ({value})")
}

/// ✗ line for a missing setting, including the concrete fix.
pub fn format_missing_env_check(name: &str, fix: &str) -> String {
    format!("✗ {name} não definida — {fix}")
}

/// Como definir uma variável de ambiente no shell DESTA máquina.
///
/// A sintaxe POSIX não serve no PowerShell/cmd, e a mensagem que manda rodar
/// algo impossível é pior que não dizer nada: no Windows o texto traz as duas
/// formas (PowerShell é o default moderno; `setx` cobre o cmd), no Unix o
/// `export` de sempre.
pub fn set_env_command(name: &str, value: &str) -> String {
    #[cfg(windows)]
    {
        format!("$env:{name}='{value}' (PowerShell) ou setx {name} \"{value}\" (cmd)")
    }
    #[cfg(not(windows))]
    {
        format!("export {name}={value}")
    }
}

/// Concrete fix for a missing setting.
pub fn missing_setting_fix(name: &str) -> String {
    let value = match name {
        "AZURE_DEVOPS_PAT" => "<pat>",
        "AZURE_DEVOPS_ORG" => "nome-da-org",
        "AZURE_DEVOPS_PROJECT" => "nome-do-projeto",
        other => return format!("defina {other}"),
    };
    format!("rode `kira setup` (ou {})", set_env_command(name, value))
}

/// Final summary for an incomplete setup.
pub fn format_missing_env_summary(missing: &[&str]) -> String {
    format!(
        "Variáveis ausentes: {} — defina-as e rode `kira doctor` de novo.",
        missing.join(", ")
    )
}

/// ✓ line for the organization connection check.
pub fn format_connection_check(organization: &str, project_count: usize) -> String {
    format!("✓ org {organization} acessível ({project_count} projetos)")
}

/// ✗ line for HTTP 401: the PAT is invalid or lacks the required scope.
pub fn format_unauthorized_check(organization: &str) -> String {
    format!(
        "✗ PAT inválido ou sem escopo Work Items (Read) — gere em https://dev.azure.com/{organization}/_usersSettings/tokens"
    )
}

/// Explicação CURTA do PAT (a aula completa vive no `kira setup`; o doctor
/// repete o resumo quando o PAT está ausente ou inválido — ADR-0025). Quando
/// a org não é conhecida, usa `<sua-org>` no link.
pub fn format_pat_lesson_short(organization: Option<&str>) -> String {
    let url = match organization {
        Some(org) => format!("https://dev.azure.com/{org}/_usersSettings/tokens"),
        None => "https://dev.azure.com/<sua-org>/_usersSettings/tokens".to_string(),
    };
    format!(
        "PAT = senha descartável de leitura de Work Items: expira na data escolhida \
         (30/90 dias) e aparece uma única vez.\n   \
         Crie em {url} com o escopo Work Items → Read; depois rode `kira setup` para colar\n   \
         (ou pule: o KIRA funciona em modo local com `kira triage --file`)."
    )
}

/// Linha do nome do dev no `kira doctor`: `dev: Sergin` quando há nome salvo
/// em `config.toml`, `dev: —` quando ausente (configs antigas seguem válidas).
pub fn format_dev_check(name: Option<&str>) -> String {
    match name {
        Some(name) => format!("dev: {name}"),
        None => "dev: —".to_string(),
    }
}

/// Finds a project by name, case-insensitively, returning the exact name.
pub fn find_project_case_insensitive<'a>(wanted: &str, available: &'a [String]) -> Option<&'a str> {
    let wanted = wanted.trim().to_lowercase();
    available
        .iter()
        .map(String::as_str)
        .find(|name| name.to_lowercase() == wanted)
}

/// Lists up to [`MAX_LISTED_PROJECTS`] existing project names for copying.
pub fn format_project_names(available: &[String]) -> String {
    let shown: Vec<&str> = available
        .iter()
        .take(MAX_LISTED_PROJECTS)
        .map(String::as_str)
        .collect();
    if available.len() > MAX_LISTED_PROJECTS {
        format!(
            "projetos existentes ({} de {}): {}",
            MAX_LISTED_PROJECTS,
            available.len(),
            shown.join(", ")
        )
    } else {
        format!("projetos existentes: {}", shown.join(", "))
    }
}

/// ✓/✗ line for the project existence check.
pub fn format_project_check(wanted: &str, available: &[String]) -> String {
    match find_project_case_insensitive(wanted, available) {
        Some(actual) if actual == wanted => format!("✓ projeto {wanted} existe"),
        Some(actual) => format!("✓ projeto {wanted} existe (nome exato: {actual})"),
        None if available.is_empty() => {
            format!("✗ projeto {wanted} não encontrado (nenhum projeto visível para este PAT)")
        }
        None => format!(
            "✗ projeto {wanted} não encontrado — {}",
            format_project_names(available)
        ),
    }
}

/// Resolves one setting for display: ✓ with source, or ✗ with the fix.
fn format_resolved_check(
    name: &str,
    resolved: &Result<credentials::Resolved, Error>,
) -> (String, bool) {
    match resolved {
        Ok(found) if name == "AZURE_DEVOPS_PAT" => match found.source {
            Source::Keyring => (format_pat_keyring_check(&found.value), true),
            _ => (format_pat_env_check(&found.value), true),
        },
        Ok(found) => (
            format_setting_source_check(name, &found.value, found.source),
            true,
        ),
        Err(_) => (
            format_missing_env_check(name, &missing_setting_fix(name)),
            false,
        ),
    }
}

/// Runs the `kira doctor` diagnosis and returns `(success, output)`.
///
/// The full printed output is returned as the second item so the caller (e.g.
/// the automatic session trail in `main.rs`) can store a summary of what was
/// shown — without retyping it.
///
/// Step 1 never touches the network: when any required setting is missing
/// the remaining steps are reported as skipped and the command exits offline.
pub async fn run() -> (bool, String) {
    // Emits the line to stdout AND collects it for the session summary.
    let mut out = String::new();
    let emit = |out: &mut String, line: &str| {
        println!("{line}");
        out.push_str(line);
        out.push('\n');
    };

    let pat = credentials::resolve_pat_with_source();
    let organization = credentials::resolve_org_with_source();
    let project = credentials::resolve_project_with_source(None);

    emit(
        &mut out,
        "1. Configuração (env, keyring do OS ou config.toml)",
    );
    let mut missing: Vec<&str> = Vec::new();
    let checks = [
        ("AZURE_DEVOPS_PAT", &pat),
        ("AZURE_DEVOPS_ORG", &organization),
        ("AZURE_DEVOPS_PROJECT", &project),
    ];
    for (name, resolved) in checks {
        let (line, ok) = format_resolved_check(name, resolved);
        emit(&mut out, &format!("   {line}"));
        if !ok {
            // PAT ausente: repete a explicação curta (ADR-0025) — o usuário
            // que desistiu do token entende o que está faltando sem rodar o
            // wizard de novo.
            if *name == *"AZURE_DEVOPS_PAT" {
                let org = organization.as_ref().ok().map(|found| found.value.as_str());
                emit(&mut out, &format!("   {}", format_pat_lesson_short(org)));
            }
            missing.push(name);
        }
    }

    // Nome do dev (opcional): linha informativa, nunca entra no "missing" —
    // ausente só significa config antiga, não configuração quebrada.
    let dev_name = credentials::dev_name_from_config();
    emit(
        &mut out,
        &format!("   {}", format_dev_check(dev_name.as_deref())),
    );

    if !missing.is_empty() {
        emit(&mut out, "2. Conexão com a organização");
        emit(&mut out, "   - ignorada: corrija os itens ✗ acima");
        emit(&mut out, "3. Projeto");
        emit(&mut out, "   - ignorada: corrija os itens ✗ acima");
        emit(&mut out, "");
        emit(&mut out, &format_missing_env_summary(&missing));
        return (false, out);
    }

    // All three are Ok at this point (checked above); no unwrap needed.
    let (pat, organization, project) = match (pat, organization, project) {
        (Ok(pat), Ok(organization), Ok(project)) => (pat.value, organization.value, project.value),
        _ => return (false, out),
    };
    let board = match board::for_board(&board::name_from_env(), &pat, &organization, &project) {
        Ok(board) => board,
        Err(err) => {
            emit(&mut out, &format!("   erro: {err}"));
            return (false, out);
        }
    };

    emit(&mut out, "2. Conexão com a organização");
    let project_names = match board.project_names().await {
        Ok(names) => {
            emit(
                &mut out,
                &format!("   {}", format_connection_check(&organization, names.len())),
            );
            names
        }
        Err(Error::AdoUnauthorized { organization: org }) => {
            emit(&mut out, &format!("   {}", format_unauthorized_check(&org)));
            // PAT inválido: repete a explicação curta (ADR-0025).
            emit(
                &mut out,
                &format!("   {}", format_pat_lesson_short(Some(&org))),
            );
            emit(&mut out, "");
            emit(
                &mut out,
                "PAT inválido — gere um novo token e rode `kira doctor` de novo.",
            );
            return (false, out);
        }
        Err(err) => {
            emit(
                &mut out,
                &format!("   ✗ falha ao acessar a org {organization}: {err}"),
            );
            emit(&mut out, "");
            emit(
                &mut out,
                "Falha de conexão — resolva o item ✗ acima e rode `kira doctor` de novo.",
            );
            return (false, out);
        }
    };

    emit(&mut out, "3. Projeto");
    emit(
        &mut out,
        &format!("   {}", format_project_check(&project, &project_names)),
    );
    if find_project_case_insensitive(&project, &project_names).is_none() {
        emit(&mut out, "");
        emit(
            &mut out,
            "Projeto incorreto — use um dos nomes listados e rode `kira inspect <id>`.",
        );
        return (false, out);
    }
    emit(&mut out, "");
    emit(&mut out, "Tudo ok — rode: kira inspect <id>");
    (true, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pat_line_shows_at_most_four_chars_and_length() {
        let pat = "Aa2xXXXXXXXXXXXXXXXXXXXXXXXXXXXX";
        assert_eq!(pat.chars().count(), 32);
        let line = format_pat_env_check(pat);
        assert_eq!(line, "✓ AZURE_DEVOPS_PAT presente (32 chars: Aa2x****)");
        assert!(
            !line.contains("XXXXXXXXXXXXXXXXXXXXXXXXXXXX"),
            "PAT completo não pode aparecer: {line}"
        );
    }

    #[test]
    fn short_pat_is_masked_still_showing_at_most_four_chars() {
        assert_eq!(mask_pat("abc"), "abc****");
        assert_eq!(mask_pat(""), "****");
        assert_eq!(
            format_pat_env_check("abc"),
            "✓ AZURE_DEVOPS_PAT presente (3 chars: abc****)"
        );
    }

    #[test]
    fn plain_env_ok_line_uses_check_mark() {
        assert_eq!(
            format_env_ok("AZURE_DEVOPS_ORG", "minha-org"),
            "✓ AZURE_DEVOPS_ORG (minha-org)"
        );
    }

    #[test]
    fn missing_env_line_uses_cross_and_concrete_fix() {
        // A dica acompanha o SO: `export` no Unix, `$env:`/`setx` no Windows.
        let fix = set_env_command("AZURE_DEVOPS_ORG", "nome-da-org");
        let line = format_missing_env_check("AZURE_DEVOPS_ORG", &fix);
        assert_eq!(line, format!("✗ AZURE_DEVOPS_ORG não definida — {fix}"));
        if cfg!(windows) {
            assert!(fix.contains("$env:AZURE_DEVOPS_ORG"), "{fix}");
            assert!(fix.contains("setx"), "{fix}");
        } else {
            assert_eq!(fix, "export AZURE_DEVOPS_ORG=nome-da-org");
        }
        // A linha de "defina a variável" reusa a mesma dica.
        let hint = missing_setting_fix("AZURE_DEVOPS_PAT");
        assert!(hint.contains("kira setup"), "{hint}");
        assert!(hint.contains("AZURE_DEVOPS_PAT"), "{hint}");
    }

    #[test]
    fn missing_env_summary_names_each_variable() {
        assert_eq!(
            format_missing_env_summary(&["AZURE_DEVOPS_PAT", "AZURE_DEVOPS_ORG"]),
            "Variáveis ausentes: AZURE_DEVOPS_PAT, AZURE_DEVOPS_ORG — \
             defina-as e rode `kira doctor` de novo."
        );
    }

    #[test]
    fn connection_line_mentions_org_and_project_count() {
        assert_eq!(
            format_connection_check("minha-org", 7),
            "✓ org minha-org acessível (7 projetos)"
        );
    }

    #[test]
    fn unauthorized_line_points_to_the_org_token_page() {
        assert_eq!(
            format_unauthorized_check("minha-org"),
            "✗ PAT inválido ou sem escopo Work Items (Read) — \
             gere em https://dev.azure.com/minha-org/_usersSettings/tokens"
        );
    }

    #[test]
    fn finds_project_case_insensitively() {
        let available = vec!["Backend".to_string(), "Meu-Projeto".to_string()];
        assert_eq!(
            find_project_case_insensitive("meu-projeto", &available),
            Some("Meu-Projeto")
        );
        assert_eq!(
            find_project_case_insensitive("MEU-PROJETO", &available),
            Some("Meu-Projeto")
        );
        assert_eq!(find_project_case_insensitive("outro", &available), None);
    }

    #[test]
    fn project_check_reports_exact_name_when_case_differs() {
        let available = vec!["Meu-Projeto".to_string()];
        let line = format_project_check("meu-projeto", &available);
        assert_eq!(
            line,
            "✓ projeto meu-projeto existe (nome exato: Meu-Projeto)"
        );
    }

    #[test]
    fn project_check_accepts_exact_name() {
        let available = vec!["Kira".to_string(), "Outro".to_string()];
        assert_eq!(
            format_project_check("Kira", &available),
            "✓ projeto Kira existe"
        );
    }

    #[test]
    fn project_not_found_lists_at_most_ten_names() {
        let available: Vec<String> = (0..15).map(|i| format!("p{i:02}")).collect();
        let line = format_project_check("nao-existe", &available);
        assert_eq!(
            line,
            "✗ projeto nao-existe não encontrado — \
             projetos existentes (10 de 15): p00, p01, p02, p03, p04, p05, p06, p07, p08, p09"
        );
    }

    #[test]
    fn project_not_found_with_few_projects_lists_all_without_count() {
        let available = vec!["alpha".to_string(), "beta".to_string()];
        let line = format_project_check("x", &available);
        assert_eq!(
            line,
            "✗ projeto x não encontrado — projetos existentes: alpha, beta"
        );
    }

    #[test]
    fn project_not_found_with_no_visible_projects() {
        assert_eq!(
            format_project_check("x", &[]),
            "✗ projeto x não encontrado (nenhum projeto visível para este PAT)"
        );
    }

    #[test]
    fn dev_check_shows_the_saved_name_or_a_dash() {
        assert_eq!(format_dev_check(Some("Sergin")), "dev: Sergin");
        assert_eq!(format_dev_check(None), "dev: —");
    }

    #[test]
    fn pat_lesson_short_teaches_and_points_to_setup_or_local_mode() {
        let lesson = format_pat_lesson_short(Some("minha-org"));
        assert!(lesson.contains("senha descartável"), "{lesson}");
        assert!(
            lesson.contains("https://dev.azure.com/minha-org/_usersSettings/tokens"),
            "{lesson}"
        );
        assert!(lesson.contains("Work Items → Read"), "{lesson}");
        assert!(lesson.contains("kira setup"), "{lesson}");
        assert!(lesson.contains("modo local"), "{lesson}");

        // Sem org conhecida: o link usa o placeholder (nunca link quebrado).
        let lesson = format_pat_lesson_short(None);
        assert!(
            lesson.contains("https://dev.azure.com/<sua-org>/_usersSettings/tokens"),
            "{lesson}"
        );
    }
}
