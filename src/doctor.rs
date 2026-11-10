//! Configuration diagnosis for `kira doctor`.
//!
//! All output formatting lives in pure functions so the ✓/✗ messages are
//! unit-tested with local data. The only network call goes through the
//! [`BoardProvider`] trait and happens exclusively after every required
//! setting is present, so an incomplete setup never touches the network.

use std::process::ExitCode;

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

/// Concrete fix for a missing setting.
pub fn missing_setting_fix(name: &str) -> String {
    let export = match name {
        "AZURE_DEVOPS_PAT" => "export AZURE_DEVOPS_PAT=<pat>",
        "AZURE_DEVOPS_ORG" => "export AZURE_DEVOPS_ORG=nome-da-org",
        "AZURE_DEVOPS_PROJECT" => "export AZURE_DEVOPS_PROJECT=nome-do-projeto",
        other => return format!("defina {other}"),
    };
    format!("rode `kira setup` (ou {export})")
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

/// Runs the `kira doctor` diagnosis and returns the process exit code.
///
/// Step 1 never touches the network: when any required setting is missing
/// the remaining steps are reported as skipped and the command exits offline.
pub async fn run() -> ExitCode {
    let pat = credentials::resolve_pat_with_source();
    let organization = credentials::resolve_org_with_source();
    let project = credentials::resolve_project_with_source(None);

    println!("1. Configuração (env, keyring do OS ou config.toml)");
    let mut missing: Vec<&str> = Vec::new();
    let checks = [
        ("AZURE_DEVOPS_PAT", &pat),
        ("AZURE_DEVOPS_ORG", &organization),
        ("AZURE_DEVOPS_PROJECT", &project),
    ];
    for (name, resolved) in checks {
        let (line, ok) = format_resolved_check(name, resolved);
        println!("   {line}");
        if !ok {
            missing.push(name);
        }
    }

    if !missing.is_empty() {
        println!("2. Conexão com a organização");
        println!("   - ignorada: corrija os itens ✗ acima");
        println!("3. Projeto");
        println!("   - ignorada: corrija os itens ✗ acima");
        println!();
        println!("{}", format_missing_env_summary(&missing));
        return ExitCode::FAILURE;
    }

    // All three are Ok at this point (checked above); no unwrap needed.
    let (pat, organization, project) = match (pat, organization, project) {
        (Ok(pat), Ok(organization), Ok(project)) => (pat.value, organization.value, project.value),
        _ => return ExitCode::FAILURE,
    };
    let board = match board::for_board(&board::name_from_env(), &pat, &organization, &project) {
        Ok(board) => board,
        Err(err) => {
            println!("   erro: {err}");
            return ExitCode::FAILURE;
        }
    };

    println!("2. Conexão com a organização");
    let project_names = match board.project_names().await {
        Ok(names) => {
            println!("   {}", format_connection_check(&organization, names.len()));
            names
        }
        Err(Error::AdoUnauthorized { organization: org }) => {
            println!("   {}", format_unauthorized_check(&org));
            println!();
            println!("PAT inválido — gere um novo token e rode `kira doctor` de novo.");
            return ExitCode::FAILURE;
        }
        Err(err) => {
            println!("   ✗ falha ao acessar a org {organization}: {err}");
            println!();
            println!("Falha de conexão — resolva o item ✗ acima e rode `kira doctor` de novo.");
            return ExitCode::FAILURE;
        }
    };

    println!("3. Projeto");
    println!("   {}", format_project_check(&project, &project_names));
    if find_project_case_insensitive(&project, &project_names).is_none() {
        println!();
        println!("Projeto incorreto — use um dos nomes listados e rode `kira inspect <id>`.");
        return ExitCode::FAILURE;
    }
    println!();
    println!("Tudo ok — rode: kira inspect <id>");
    ExitCode::SUCCESS
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
        let line =
            format_missing_env_check("AZURE_DEVOPS_ORG", "export AZURE_DEVOPS_ORG=nome-da-org");
        assert_eq!(
            line,
            "✗ AZURE_DEVOPS_ORG não definida — export AZURE_DEVOPS_ORG=nome-da-org"
        );
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
}
