//! Snapshot imutável do que o dashboard mostra.
//!
//! Regra de performance e honestidade (docs/kira-w6-dashboard.md): o snapshot é
//! lido do disco **uma vez** — no início e a cada `r` — nunca por frame. Dado
//! ausente vira `—` com a causa (nunca inventado): `achados: — (nunca rodou
//! `kira scan`)`. Toda a montagem das linhas do cabeçalho é função pura sobre
//! o snapshot, o que torna a honestidade unit-testável sem disco.
//!
//! Read-only por construção: a store abre em `SQLITE_OPEN_READ_ONLY`
//! (`Store::open_readonly_at`), findings/config/PAT são leituras, e nada aqui
//! toca rede ou LLM (`src/tui` está fora da allowlist do ADR-0016).

use std::path::{Path, PathBuf};

use crate::credentials;
use crate::doctor;
use crate::scan;
use crate::store::{self, Store};

/// O estado de tela inteiro do dashboard nesta PR (só o cabeçalho existe).
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    /// Raiz canônica do projeto detectada do cwd; `None` = sem diretório.
    pub root: Option<PathBuf>,
    /// Projeto do store; `None` = a causa acionável vive em `store_note`.
    pub project: Option<store::Project>,
    /// Nota acionável quando o store não pôde ser lido (store ausente etc.).
    pub store_note: Option<String>,
    /// Nome do dev no config (`dev_name`), quando existir.
    pub dev_name: Option<String>,
    /// Causa quando não há `dev_name` legível no config.
    pub dev_note: Option<String>,
    /// PAT já mascarado (primeiros 4 chars + `****`) e de onde veio.
    pub pat_masked: Option<String>,
    pub pat_source: Option<String>,
    /// Causa quando não há PAT resolvível.
    pub pat_note: Option<String>,
    /// Quantidade de achados do último `kira scan` (`findings.json`).
    pub findings: Option<usize>,
    /// Causa quando não há contagem de achados.
    pub findings_note: Option<String>,
    /// Idade do último `kira scan` gravado na trilha (segundos desde então).
    pub last_scan_age_secs: Option<i64>,
    /// Data/hora local do último scan, já pronta para exibição.
    pub last_scan_local: Option<String>,
}

/// Constante exibida como backend LLM default.
///
/// Texto estático de propósito: a resolução real de backend (`KIRA_LLM`,
/// `--llm`) vive em `src/llm/` e `src/tui` não pode importá-la (allowlist do
/// ADR-0016). O dashboard não chama modelo — o `—`/causa do candidato seria
/// inventar hierarquia que este PR não tem.
pub const LLM_LABEL: &str = "ollama (default, localhost — o dashboard não chama modelo)";

/// Lê o estado do disco: store (read-only), findings.json, config e PAT.
///
/// Nada aqui falha o comando: todo erro vira nota/`—` com causa, porque o
/// dashboard é diagnóstico (exit 0 mesmo com store ausente — contrato 4 da
/// spec).
pub fn load() -> Snapshot {
    let root = store::detect_project_root();
    let mut snapshot = Snapshot {
        root,
        ..Snapshot::default()
    };
    load_store(&mut snapshot);
    load_dev_name(&mut snapshot);
    load_pat(&mut snapshot);
    load_findings(&mut snapshot);
    snapshot
}

/// Projeto + idade do último scan, direto da trilha (read-only).
fn load_store(snapshot: &mut Snapshot) {
    let Some(root) = snapshot.root.clone() else {
        snapshot.store_note = Some("sem diretório atual para detectar o projeto".to_string());
        return;
    };
    let Some(db) = store::db_path() else {
        snapshot.store_note = Some(
            "sem diretório para o store — defina HOME (ou USERPROFILE) ou KIRA_STORE_DIR"
                .to_string(),
        );
        return;
    };
    if !db.exists() {
        snapshot.store_note = Some(format!(
            "nenhuma trilha em {} — rode `kira scan` ou `kira doctor`",
            store::display_path(&db)
        ));
        return;
    }
    let opened = Store::open_readonly_at(&db).and_then(|store| {
        // 500 linhas cobram qualquer histórico real de scan sem query nova no
        // store: filtrar `command == "scan"` em memória mantém a API intacta.
        store.find_project(&root).and_then(|project| {
            project.map_or(Ok(None), |project| {
                store
                    .last_sessions(&project.id, 500)
                    .map(|sessions| Some((project, sessions)))
            })
        })
    });
    match opened {
        Ok(Some((project, sessions))) => {
            if let Some(scan_session) = sessions
                .iter()
                .find(|session| session.command == "scan")
                .cloned()
            {
                snapshot.last_scan_age_secs = age_secs_from_rfc3339(&scan_session.started_at);
                snapshot.last_scan_local = scan_session.local_started_at;
            }
            snapshot.project = Some(project);
        }
        Ok(None) => {
            snapshot.store_note = Some(format!(
                "nenhuma trilha para {} — rode `kira scan` ou `kira doctor`",
                store::display_path(&root)
            ));
        }
        Err(err) => {
            snapshot.store_note = Some(format!(
                "trilha ilegível ({err}) — rode `kira scan` ou `kira doctor`"
            ));
        }
    }
}

/// `dev_name` no config (opcional, escrita futura do `kira setup`).
fn load_dev_name(snapshot: &mut Snapshot) {
    match credentials::read_config() {
        Ok(pairs) => {
            snapshot.dev_name = pairs
                .iter()
                .find(|(key, _)| key == "dev_name")
                .map(|(_, value)| value.clone());
            if snapshot.dev_name.is_none() {
                snapshot.dev_note =
                    Some("sem dev_name no config — adicione `dev_name = \"seu nome\"`".to_string());
            }
        }
        Err(err) => {
            snapshot.dev_note = Some(format!("config ilegível ({err})"));
        }
    }
}

/// PAT mascarado para exibição (mesma máscara do `kira doctor`).
fn load_pat(snapshot: &mut Snapshot) {
    match credentials::resolve_pat_with_source() {
        Ok(resolved) => {
            snapshot.pat_masked = Some(doctor::mask_pat(&resolved.value));
            snapshot.pat_source = Some(resolved.source.label().to_string());
        }
        Err(err) => {
            snapshot.pat_note = Some(format!("{err}"));
        }
    }
}

/// Contagem de achados do último scan (o painel completo é o W6-PR2).
fn load_findings(snapshot: &mut Snapshot) {
    let Some(root) = snapshot.root.clone() else {
        return; // sem root, a causa já está na nota do store
    };
    match scan::read_findings_file(&root) {
        Ok(Some(findings)) => snapshot.findings = Some(findings.len()),
        Ok(None) => {
            // Honestidade depende do histórico: sem scan na trilha E sem
            // arquivo, a causa é "nunca rodou"; com scan gravado, o arquivo
            // sumiu (ou o scan rodou em outro diretório).
            snapshot.findings_note = Some(if snapshot.last_scan_age_secs.is_some() {
                "findings.json ausente — rode `kira scan` de novo".to_string()
            } else {
                "nunca rodou kira scan".to_string()
            });
        }
        Err(err) => {
            snapshot.findings_note = Some(format!("{err}"));
        }
    }
}

/// Idade em segundos de um registro RFC 3339 UTC da store até agora.
///
/// `None` = stamp não-parseável (a store só escreve `...Z`; qualquer outra
/// forma vira idade desconhecida em vez de 1970) — o chamador mostra `—`.
fn age_secs_from_rfc3339(rfc3339: &str) -> Option<i64> {
    let stamp = store::rfc3339_utc_to_unix_seconds(rfc3339)?;
    let now = store::rfc3339_utc_to_unix_seconds(&store::now_rfc3339_utc())?;
    Some((now - stamp).max(0))
}

/// Idade humanizada: `há 45s`, `há 3 min`, `há 2 h`, `há 5 dias`.
pub fn format_age_secs(secs: i64) -> String {
    match secs {
        0..=59 => format!("há {secs}s"),
        60..=3_599 => format!("há {} min", secs / 60),
        3_600..=86_399 => format!("há {} h", secs / 3_600),
        _ => {
            let days = secs / 86_400;
            if days == 1 {
                "há 1 dia".to_string()
            } else {
                format!("há {days} dias")
            }
        }
    }
}

/// Linha `— (causa)` de um dado ausente — o contrato de honestidade.
fn dash_line(cause: &str) -> String {
    format!("— ({cause})")
}

/// Linhas do cabeçalho, puras sobre o snapshot: a MESMA lista alimenta o
/// widget TUI e a saída de texto do `--snapshot`/não-TTY (CI).
///
/// A store ausente não esconde o cabeçalho: projeto/root saem como `—` com a
/// nota acionável na última linha (contrato 4 da spec — exit 0 é diagnóstico).
pub fn header_lines(snapshot: &Snapshot) -> Vec<String> {
    let mut lines = Vec::new();
    match &snapshot.project {
        Some(project) => {
            lines.push(format!("projeto: {}", project.name));
            lines.push(format!(
                "root: {}",
                store::display_path(Path::new(&project.root_path))
            ));
            lines.push(format!("id: {}", project.id));
        }
        None => {
            lines.push(format!(
                "projeto: {}",
                dash_line(
                    snapshot
                        .store_note
                        .as_deref()
                        .unwrap_or("store sem projeto")
                )
            ));
            lines.push(format!(
                "root: {}",
                snapshot
                    .root
                    .as_ref()
                    .map(|root| store::display_path(root))
                    .unwrap_or_else(|| dash_line("sem diretório atual"))
            ));
            lines.push(format!("id: {}", dash_line("sem projeto no store")));
        }
    }
    match &snapshot.dev_name {
        Some(name) => lines.push(format!("dev: {name}")),
        None => lines.push(format!(
            "dev: {}",
            dash_line(
                snapshot
                    .dev_note
                    .as_deref()
                    .unwrap_or("sem dev_name no config")
            )
        )),
    }
    lines.push(format!("llm: {LLM_LABEL}"));
    match &snapshot.pat_masked {
        Some(masked) => lines.push(format!(
            "pat: {masked} (de {})",
            snapshot
                .pat_source
                .as_deref()
                .unwrap_or("origem desconhecida")
        )),
        None => lines.push(format!(
            "pat: {}",
            dash_line(
                snapshot
                    .pat_note
                    .as_deref()
                    .unwrap_or("defina AZURE_DEVOPS_PAT ou rode kira setup")
            )
        )),
    }
    match snapshot.findings {
        Some(count) => lines.push(format!("achados: {count}")),
        None => lines.push(format!(
            "achados: {}",
            dash_line(
                snapshot
                    .findings_note
                    .as_deref()
                    .unwrap_or("nunca rodou kira scan")
            )
        )),
    }
    match snapshot.last_scan_age_secs {
        Some(age) => {
            let local = snapshot
                .last_scan_local
                .as_deref()
                .map(|local| format!(" ({local} local)"))
                .unwrap_or_default();
            lines.push(format!("último scan: {}{local}", format_age_secs(age)));
        }
        None => lines.push(format!(
            "último scan: {}",
            dash_line("nunca rodou kira scan")
        )),
    }
    if let Some(note) = &snapshot.store_note {
        lines.push(format!("store: {note}"));
    }
    lines
}

/// Rodapé de 1 linha com as teclas — mesmo texto no TUI e no snapshot.
pub fn footer_line() -> String {
    "teclas: q sair · Tab/Shift+Tab painel · j/k ou ↑/↓ mover · g/G topo/fim · r reler disco · ? ajuda".to_string()
}

/// Relatório de texto do `kira dashboard --snapshot` (e do modo não-TTY).
///
/// Nunca desenha na tela (sem alternate screen, sem raw mode): é impressão
/// comum que o CI captura. Exit 0 mesmo com dados ausentes — diagnóstico.
pub fn text_report(snapshot: &Snapshot) -> String {
    let mut text = String::from("kira dashboard (read-only — snapshot de texto)\n");
    for line in header_lines(snapshot) {
        text.push_str(&line);
        text.push('\n');
    }
    text.push_str(&footer_line());
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Project;

    /// Snapshot de referência com tudo presente (o caso feliz).
    fn full_snapshot() -> Snapshot {
        Snapshot {
            root: Some(PathBuf::from("/tmp/proj")),
            project: Some(Project {
                id: "a1b2c3d4e5f60718".to_string(),
                root_path: "/tmp/proj".to_string(),
                name: "proj".to_string(),
                first_seen_at: "2026-11-01T10:00:00Z".to_string(),
            }),
            dev_name: Some("Sérgio".to_string()),
            pat_masked: Some("Aa2x****".to_string()),
            pat_source: Some("env".to_string()),
            findings: Some(7),
            last_scan_age_secs: Some(86_400 * 2),
            last_scan_local: Some("2026-11-09 14:03:00".to_string()),
            ..Snapshot::default()
        }
    }

    #[test]
    fn full_snapshot_shows_every_real_value() {
        let lines = header_lines(&full_snapshot());
        let text = lines.join("\n");
        assert!(text.contains("projeto: proj"), "{text}");
        assert!(text.contains("root: /tmp/proj"), "{text}");
        assert!(text.contains("id: a1b2c3d4e5f60718"), "{text}");
        assert!(text.contains("dev: Sérgio"), "{text}");
        assert!(text.contains("llm: ollama"), "{text}");
        assert!(text.contains("pat: Aa2x**** (de env)"), "{text}");
        assert!(text.contains("achados: 7"), "{text}");
        assert!(text.contains("último scan: há 2 dias"), "{text}");
    }

    #[test]
    fn missing_data_is_a_dash_with_the_cause_never_invented() {
        // Nada presente: cada linha ausente mostra `—` + a causa concreta.
        let lines = header_lines(&Snapshot::default());
        let text = lines.join("\n");
        assert!(
            text.contains("achados: — (nunca rodou kira scan)"),
            "{text}"
        );
        assert!(
            text.contains("último scan: — (nunca rodou kira scan)"),
            "{text}"
        );
        assert!(text.contains("pat: — ("), "{text}");
        assert!(text.contains("dev: — ("), "{text}");
    }

    #[test]
    fn scan_without_findings_file_names_the_missing_file() {
        // Scan existe na trilha, mas findings.json sumiu: a causa NÃO é
        // "nunca rodou" — seria mentir sobre o histórico.
        let snapshot = Snapshot {
            last_scan_age_secs: Some(60),
            findings_note: Some("findings.json ausente — rode `kira scan` de novo".to_string()),
            ..Snapshot::default()
        };
        let text = header_lines(&snapshot).join("\n");
        assert!(
            text.contains("achados: — (findings.json ausente — rode `kira scan` de novo)"),
            "{text}"
        );
        assert!(text.contains("último scan: há 1 min"), "{text}");
    }

    #[test]
    fn absent_store_keeps_the_actionable_note_and_exits_zero_semantics() {
        let snapshot = Snapshot {
            store_note: Some(
                "nenhuma trilha em store.db — rode `kira scan` ou `kira doctor`".to_string(),
            ),
            ..Snapshot::default()
        };
        let text = header_lines(&snapshot).join("\n");
        assert!(text.contains("projeto: — (nenhuma trilha em"), "{text}");
        assert!(text.contains("store: nenhuma trilha em"), "{text}");
    }

    #[test]
    fn ages_display_humanized() {
        assert_eq!(format_age_secs(0), "há 0s");
        assert_eq!(format_age_secs(45), "há 45s");
        assert_eq!(format_age_secs(120), "há 2 min");
        assert_eq!(format_age_secs(7_200), "há 2 h");
        assert_eq!(format_age_secs(86_400), "há 1 dia");
        assert_eq!(format_age_secs(172_800), "há 2 dias");
    }

    #[test]
    fn text_report_carries_header_and_footer() {
        let report = text_report(&full_snapshot());
        assert!(report.contains("kira dashboard (read-only"), "{report}");
        assert!(report.contains("dev: Sérgio"), "{report}");
        assert!(report.contains("teclas: q sair"), "{report}");
    }
}
