//! Configuration diagnosis for `kira doctor`.
//!
//! All output formatting lives in pure functions so the ✓/✗ messages are
//! unit-tested with local data. The only network call goes through the
//! [`BoardProvider`] trait and happens exclusively after every required
//! setting is present, so an incomplete setup never touches the network.

use crate::board;
use crate::capabilities::{Capability, CapabilityState};
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
    // O PAT agora nasce em `connect` (ADR-0030 #4): a dica aponta para o
    // comando que valida e grava, não para o setup (que não pede mais PAT).
    if name == "AZURE_DEVOPS_PAT" {
        return format!(
            "rode `kira connect azure` (ou {})",
            set_env_command(name, "<pat>")
        );
    }
    let value = match name {
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

/// Linhas da onda M (ADR-0038): o que foi ingerido e o estado da fila de posts.
///
/// Fail-open: qualquer dado ausente vira uma frase com a causa e o comando que
/// resolve — nunca número inventado. "Desatualizado" tem definição precisa: o
/// `source_hash` da linha em `posts` não bate mais com o digest atual, ou seja, o
/// repositório mudou depois que o post foi gerado.
#[must_use]
pub fn format_ingest_lines() -> Vec<String> {
    let Some(root) = crate::store::detect_project_root() else {
        return vec![
            "ingestão: sem projeto detectado — rode dentro da raiz do repositório".to_string(),
        ];
    };
    let kira = root.join(".kira");
    let digest_path = kira.join("digest.md");
    let digest = std::fs::read_to_string(&digest_path).ok();
    let files = std::fs::read_to_string(kira.join("ingest").join("inventory.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<crate::ingest::inventory::Inventory>(&text).ok())
        .map(|inv| inv.files.len());
    let trust = if crate::trust::is_trusted(&root) {
        "aceito"
    } else {
        "não aceito (rode `kira trust`)"
    };
    let mut lines = Vec::new();
    match (&digest, files) {
        (Some(_), Some(count)) => lines.push(format!(
            "ingestão: {count} arquivo(s) ingerido(s), digest ok, repositório {trust}"
        )),
        (Some(_), None) => lines.push(format!(
            "ingestão: digest ok (inventário ausente — rode `kira ingest`), repositório {trust}"
        )),
        _ => lines.push(format!(
            "ingestão: nenhuma (rode `kira ingest`), repositório {trust}"
        )),
    }

    let Some(digest) = digest else {
        return lines;
    };
    let Ok(store) = crate::store::Store::open_default() else {
        lines.push("posts: store indisponível".to_string());
        return lines;
    };
    let Ok(project_id) = crate::store::project_id_for_root(&root) else {
        lines.push("posts: projeto não registrado".to_string());
        return lines;
    };
    let Ok(posts) = store.posts_of(&project_id) else {
        lines.push("posts: ilegível".to_string());
        return lines;
    };
    if posts.is_empty() {
        lines.push("posts: nenhum (rode `kira digest`)".to_string());
        return lines;
    }
    let planned = crate::post::plan::plan(&digest);
    let pending = posts
        .iter()
        .filter(|p| p.state == crate::store::posts::PostState::Sealed)
        .count();
    let delivered = posts
        .iter()
        .filter(|p| p.state == crate::store::posts::PostState::Delivered)
        .count();
    let dead = posts
        .iter()
        .filter(|p| p.state == crate::store::posts::PostState::Dead)
        .count();
    let stale = posts
        .iter()
        .filter(|p| p.state == crate::store::posts::PostState::Delivered)
        .filter(|p| {
            planned
                .iter()
                .find(|planned| planned.slug == p.slug)
                .is_some_and(|planned| planned.source_hash != p.source_hash)
        })
        .count();
    let unread = store.posts_unread(&project_id).unwrap_or(0);
    let mut line = format!(
        "posts: {pending} pendente(s) · {delivered} entregue(s) · {unread} não lido(s) · {dead} extraviada(s)"
    );
    if stale > 0 {
        line.push_str(&format!(
            " · {stale} desatualizado(s) — o repositório mudou: rode `kira digest` e `kira emit`"
        ));
    }
    lines.push(line);
    lines
}

/// Linha de capacidades do `kira doctor` (ADR-0030): o que está conectado
/// para o projeto atual, ou o aviso de modo local quando nada está.
pub fn format_capabilities_line(connected: &[&str]) -> String {
    if connected.is_empty() {
        "capacidades: nenhuma conectada (modo local)".to_string()
    } else {
        format!("capacidades: {}", connected.join(", "))
    }
}

/// Lê o estado REAL das capacidades do projeto atual no store (mesma fonte
/// do gating). Qualquer falha de leitura (sem HOME, store indisponível,
/// fora de um projeto) degrada para "nenhuma conectada" — o doctor é
/// diagnóstico, não gate: nunca falha nem bloqueia por isso.
pub fn connected_capabilities() -> Vec<&'static str> {
    const ALL: [Capability; 3] = [
        Capability::BoardAzure,
        Capability::ScmGithub,
        Capability::ScmGitlab,
    ];
    let Ok(store) = crate::store::Store::open_default() else {
        return Vec::new();
    };
    let Some(root) = crate::store::detect_project_root() else {
        return Vec::new();
    };
    let Ok(project) = store.ensure_project(&root) else {
        return Vec::new();
    };
    ALL.iter()
        .filter(|capability| {
            store
                .capability_state_raw(&project.id, capability.name())
                .ok()
                .flatten()
                .and_then(|value| CapabilityState::from_stored(&value))
                .is_some_and(|state| state == CapabilityState::Conectada)
        })
        .map(|capability| capability.name())
        .collect()
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
            // PAT ausente NÃO ensina mais como criar token (ADR-0030 #4): o
            // PAT agora só é assunto de `kira connect` — a linha ✗ já aponta
            // para o comando certo, e o modo local segue válido.
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

    // Capacidades (ADR-0030): o que está conectado PARA ESTE PROJETO, lido
    // na mesma fonte do gating — ou o aviso de modo local, que é o estado
    // normal de uma instalação local-first.
    emit(
        &mut out,
        &format_capabilities_line(&connected_capabilities()).to_string(),
    );

    // Onda M (ADR-0038): ingestão e fila de posts. Fail-open — sem projeto ou sem
    // store, a linha diz que não há dado em vez de inventar número.
    for line in format_ingest_lines() {
        emit(&mut out, &line);
    }

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
            // PAT inválido não ensina mais a aula (ADR-0030 #4): o comando
            // que valida e grava o token é o `kira connect`.
            emit(&mut out, "");
            emit(
                &mut out,
                "PAT inválido — rode `kira connect azure` para reconectar e rode `kira doctor` de novo.",
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
        // A linha de "defina a variável" reusa a mesma dica — e o PAT NÃO
        // aponta mais para o setup (a credencial nasce em `kira connect`).
        let hint = missing_setting_fix("AZURE_DEVOPS_ORG");
        assert!(hint.contains("kira setup"), "{hint}");
        assert!(hint.contains("AZURE_DEVOPS_ORG"), "{hint}");
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
    fn missing_pat_fix_points_to_connect_not_to_setup() {
        // ADR-0030 #4: o PAT só é assunto de `kira connect` — o setup deixou
        // de pedir, então a dica não pode mandar de volta para o wizard.
        let hint = missing_setting_fix("AZURE_DEVOPS_PAT");
        assert!(hint.contains("kira connect azure"), "{hint}");
        assert!(hint.contains("AZURE_DEVOPS_PAT"), "{hint}");
        assert!(!hint.contains("kira setup"), "{hint}");
    }

    #[test]
    fn capabilities_line_shows_local_mode_or_the_connected_list() {
        assert_eq!(
            format_capabilities_line(&[]),
            "capacidades: nenhuma conectada (modo local)"
        );
        assert_eq!(
            format_capabilities_line(&["board.azure"]),
            "capacidades: board.azure"
        );
        assert_eq!(
            format_capabilities_line(&["board.azure", "scm.github"]),
            "capacidades: board.azure, scm.github"
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
    fn connected_capabilities_reads_the_store_or_degrades_to_empty() {
        // Fora de um projeto de verdade / com store indisponível: vazio —
        // nunca pânico e nunca lista inventada.
        let connected = connected_capabilities();
        assert!(
            connected.iter().all(|name| !name.is_empty()),
            "só nomes de contrato: {connected:?}"
        );
    }
}
