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

use std::io::Read;
use std::path::{Path, PathBuf};

use crate::credentials;
use crate::doctor;
use crate::scan::{self, Finding};
use crate::store::{self, Store};

/// O estado de tela inteiro do dashboard (W6-PR2: cabeçalho + achados).
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
    /// Linhas do painel de achados (W6-PR2): já normalizadas e ordenadas
    /// critical→low. Vazio = estado vazio honesto (`empty_findings_line`).
    pub finding_rows: Vec<FindingRow>,
    /// Últimas [`SESSIONS_LIMIT`] sessões da trilha (painel de sessões,
    /// W6-PR3), mais recente primeiro. Vazio = estado vazio honesto
    /// (`empty_sessions_line`).
    pub session_rows: Vec<SessionLine>,
    /// Causa quando o painel de sessões está vazio mesmo com store legível
    /// (`store sem sessões`); ausente, a causa cai para `store_note`.
    pub sessions_note: Option<String>,
    /// Páginas da wiki (W6-PR4): o índice `wiki_page` da store (read-only)
    /// e o markdown de cada página lido de `.kira/wiki/`. Vazio = estado
    /// vazio honesto (`empty_wiki_line`).
    pub wiki_rows: Vec<WikiPage>,
    /// Causa quando o índice da wiki está vazio mesmo com store legível
    /// (`wiki vazia — rode kira wiki reindex`); ausente, a causa cai para
    /// `store_note` (store ausente é a causa real, não uma inventada).
    pub wiki_note: Option<String>,
    /// `now` capturado UMA vez por snapshot (unix, segundos): toda idade
    /// relativa da tela é função pura [`humanize_age`] sobre este valor —
    /// NUNCA `SystemTime::now()` dentro do widget (determinismo da spec W6).
    pub now_secs: Option<i64>,
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

/// Projeto + idade do último scan + últimas sessões, direto da trilha
/// (read-only).
///
/// Uma única consulta de histórico (`SESSION_HISTORY_LIMIT`) alimenta os DOIS
/// consumidores: a idade do último `scan` (que precisa enxergar longe) e o
/// painel de sessões, que leva só as [`SESSIONS_LIMIT`] mais recentes desse
/// recorte via [`session_rows_from`] — sem query nova por frame e sem API nova
/// na store (o read-only real é o `Store::open_readonly_at`, inalterado).
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
    // `now` capturado UMA VEZ por snapshot: todas as idades relativas da tela
    // comparam contra o mesmo instante (determinismo dentro do snapshot).
    let now = store::rfc3339_utc_to_unix_seconds(&store::now_rfc3339_utc());
    snapshot.now_secs = now;
    let opened = Store::open_readonly_at(&db).and_then(|store| {
        // 500 linhas cobram qualquer histórico real de scan sem query nova no
        // store: filtrar `command == "scan"` em memória mantém a API intacta.
        store.find_project(&root).and_then(|project| {
            project.map_or(Ok(None), |project| {
                store
                    .last_sessions(&project.id, SESSION_HISTORY_LIMIT)
                    .and_then(|sessions| {
                        // A MESMA conexão read-only alimenta o painel de
                        // wiki (W6-PR4) — nenhuma segunda abertura, nenhum
                        // write no disco.
                        store
                            .wiki_pages()
                            .map(|wiki| Some((project, sessions, wiki)))
                    })
            })
        })
    });
    match opened {
        Ok(Some((project, sessions, wiki))) => {
            if let Some(scan_session) = sessions
                .iter()
                .find(|session| session.command == "scan")
                .cloned()
            {
                snapshot.last_scan_age_secs =
                    age_secs_from_rfc3339_at(now, &scan_session.started_at);
                snapshot.last_scan_local = scan_session.local_started_at;
            }
            snapshot.session_rows = session_rows_from(&sessions);
            if sessions.is_empty() {
                snapshot.sessions_note = Some("store sem sessões".to_string());
            }
            snapshot.project = Some(project);
            apply_wiki(snapshot, Ok(wiki), &root);
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

/// Contagem + linhas de achados do último scan (painel W6-PR2). A decisão
/// ausente/corrompido é função pura em [`apply_findings`] — testável sem disco.
fn load_findings(snapshot: &mut Snapshot) {
    let Some(root) = snapshot.root.clone() else {
        return; // sem root, a causa já está na nota do store
    };
    apply_findings(snapshot, scan::read_findings_file(&root));
}

/// Aplica o resultado da leitura de `findings.json` ao snapshot — função pura
/// para o contrato fail-open ser unit-testável: ausente/corrompido ⇒ lista
/// vazia + causa acionável, NUNCA pânico nem dado inventado.
fn apply_findings(snapshot: &mut Snapshot, loaded: Result<Option<Vec<Finding>>, scan::ScanError>) {
    match loaded {
        Ok(Some(findings)) => {
            snapshot.findings = Some(findings.len());
            snapshot.finding_rows = rows_from(findings);
        }
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
            // Fail-open: JSON corrompido vira lista vazia com a causa real da
            // leitura (o próprio erro já diz "rode `kira scan` de novo").
            snapshot.findings_note = Some(format!("{err}"));
            snapshot.finding_rows = Vec::new();
        }
    }
}

/// Idade em segundos de um registro RFC 3339 UTC da store até um `now` fixo.
///
/// `None` = stamp não-parseável (a store só escreve `...Z`; qualquer outra
/// forma vira idade desconhecida em vez de 1970) — o chamador mostra `—`.
/// Receber o `now` como parâmetro (capturado uma vez em [`load_store`]) é o
/// que mantém a idade determinística e unit-testável com casos fixos.
fn age_secs_from_rfc3339_at(now: Option<i64>, rfc3339: &str) -> Option<i64> {
    let stamp = store::rfc3339_utc_to_unix_seconds(rfc3339)?;
    Some((now? - stamp).max(0))
}

/// Uma linha da tabela de achados: o que o painel precisa de um `Finding`.
///
/// O `scan` não informa linha (o path do npm é a cadeia de dependência, o do
/// dotnet é o csproj); quando o path já chega com sufixo `:N` (analisadores
/// que apontam arquivo:linha), separamos para a coluna LOCAL exibir
/// `path:linha` — nunca inventamos um número de linha que não veio do disco.
/// `fingerprint`/`message` completos viajam junto porque o detalhe do `Enter`
/// mostra o valor inteiro, não o truncado da tabela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingRow {
    /// Severidade em minúsculas (exibição; o dado cru fica no findings.json).
    pub severity: String,
    pub tool: String,
    pub path: String,
    /// Linha quando o path veio como `arquivo:N`; `None` = sem linha conhecida.
    pub line: Option<u32>,
    /// Primeiros 8 chars do fingerprint hex de 64 bits.
    pub fingerprint_short: String,
    /// Fingerprint completo (o detalhe do `Enter` mostra os 16 chars).
    pub fingerprint: String,
    /// Primeira linha da mensagem (o que cabe na coluna RESUMO).
    pub message_first_line: String,
    /// Mensagem inteira (o detalhe do `Enter` exibe sem corte).
    pub message: String,
}

impl FindingRow {
    /// LOCAL da tabela: `path:linha` quando a linha existe, senão só o path.
    pub fn location(&self) -> String {
        match self.line {
            Some(line) => format!("{}:{line}", self.path),
            None => self.path.clone(),
        }
    }
}

/// Ordem canônica de severidade do painel (spec W6): critical→low.
pub const SEVERITY_ORDER: [&str; 4] = ["critical", "high", "medium", "low"];

/// Rank de ordenação: as quatro severidades canônicas primeiro, qualquer
/// severidade fora do padrão (ex.: `moderate` do npm) depois, por ordem
/// alfabética — determinismo em vez de sumiço.
fn severity_rank(severity: &str) -> usize {
    SEVERITY_ORDER
        .iter()
        .position(|canonical| *canonical == severity)
        .unwrap_or(SEVERITY_ORDER.len())
}

/// Separa um sufixo `:N` do path (ex.: `src/lib.rs:42`). Só aceita dígitos
/// depois do último `:` — um `C:\proj` do Windows não vira linha falsa.
fn split_path_line(path: &str) -> (String, Option<u32>) {
    if let Some(colon) = path.rfind(':') {
        let (file, line) = path.split_at(colon);
        if !file.is_empty()
            && let Ok(number) = line[1..].parse::<u32>()
        {
            return (file.to_string(), Some(number));
        }
    }
    (path.to_string(), None)
}

/// Converte os `Finding` do scan em linhas do painel, já ordenadas
/// critical→low (e dentro da mesma severidade por ferramenta/path/fingerprint,
/// que é a ordem determinística do arquivo). Função pura: teste sem disco.
pub fn rows_from(findings: Vec<Finding>) -> Vec<FindingRow> {
    let mut rows: Vec<FindingRow> = findings
        .iter()
        .map(|finding| {
            let severity = finding.severity.trim().to_lowercase();
            let (path, line) = split_path_line(finding.path.trim());
            let fingerprint = finding.fingerprint.trim().to_string();
            let message = finding.message.trim().to_string();
            let message_first_line = message.lines().next().unwrap_or_default().to_string();
            FindingRow {
                severity,
                tool: finding.tool.trim().to_string(),
                path,
                line,
                fingerprint_short: fingerprint.chars().take(8).collect(),
                fingerprint,
                message_first_line,
                message,
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        severity_rank(&a.severity)
            .cmp(&severity_rank(&b.severity))
            .then_with(|| a.tool.cmp(&b.tool))
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a.fingerprint.cmp(&b.fingerprint))
    });
    rows
}

/// Contagem por severidade, na ordem canônica do painel (critical→low com
/// zeros visíveis) + severidades fora do padrão em ordem alfabética.
pub fn severity_counts(rows: &[FindingRow]) -> Vec<(String, usize)> {
    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for row in rows {
        *counts.entry(row.severity.clone()).or_default() += 1;
    }
    let mut ordered: Vec<(String, usize)> = SEVERITY_ORDER
        .iter()
        .map(|severity| {
            (
                (*severity).to_string(),
                *counts.get(*severity).unwrap_or(&0),
            )
        })
        .collect();
    for (severity, count) in counts {
        if !SEVERITY_ORDER.contains(&severity.as_str()) {
            ordered.push((severity, count));
        }
    }
    ordered
}

/// Contagem por ferramenta, alfabética (determinismo; o painel mostra
/// `ferramentas: audit 2, dotnet 1` quando há achados).
pub fn tool_counts(rows: &[FindingRow]) -> Vec<(String, usize)> {
    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for row in rows {
        *counts.entry(row.tool.clone()).or_default() += 1;
    }
    counts.into_iter().collect()
}

/// Cabeçalho de contagem do painel: `critical 1 · high 2 · medium 0 · low 0`
/// (severidades fora do padrão entram no fim, também com ` · `).
pub fn count_header_line(rows: &[FindingRow]) -> String {
    severity_counts(rows)
        .iter()
        .map(|(severity, count)| format!("{severity} {count}"))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Linha de ferramentas do painel: `ferramentas: audit 2, dotnet 1`; vazia
/// quando não há achados (o chamador omite a linha inteira).
pub fn tools_line(rows: &[FindingRow]) -> String {
    let counts = tool_counts(rows);
    if counts.is_empty() {
        return String::new();
    }
    let parts = counts
        .iter()
        .map(|(tool, count)| format!("{tool} {count}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("ferramentas: {parts}")
}

/// Estado vazio honesto do painel (spec W6-PR2): arquivo presente e vazio ⇒
/// `(0 achados no último scan)`; ausente/corrompido ⇒ `—` com a causa que o
/// snapshot já carrega — nunca uma causa genérica inventada aqui. SEM o
/// prefixo `achados:` — o campo vive uma única vez no cabeçalho; repeti-lo na
/// seção era duplicação (o painel só traz o conteúdo, não o rótulo de novo).
pub fn empty_findings_line(snapshot: &Snapshot) -> String {
    if snapshot.findings == Some(0) {
        return "(0 achados no último scan)".to_string();
    }
    let cause = snapshot
        .findings_note
        .clone()
        .unwrap_or_else(|| "findings.json ausente — rode kira scan".to_string());
    dash_line(&cause)
}

/// Linhas de texto do painel de achados: a MESMA lista alimenta o widget TUI
/// (modo degradado) e a saída do `--snapshot`/não-TTY. Fixas em colunas — sem
/// scroll horizontal em nenhum dos dois mundos.
pub fn findings_lines(snapshot: &Snapshot) -> Vec<String> {
    let rows = &snapshot.finding_rows;
    if rows.is_empty() {
        return vec![empty_findings_line(snapshot)];
    }
    let mut lines = vec![
        format!(
            "painel achados — {} · {}",
            count_header_line(rows),
            tools_line(rows)
        ),
        format!(
            "{:<10} {:<12} {:<26}{}",
            "SEVERIDADE", "FERRAMENTA", "LOCAL", "RESUMO"
        ),
    ];
    for row in rows {
        lines.push(format!(
            "{:<10} {:<12} {:<26}{}",
            row.severity,
            row.tool,
            row.location(),
            row.message_first_line
        ));
    }
    lines
}

/// Linhas do detalhe aberto pelo `Enter`: severidade, ferramenta, LOCAL,
/// fingerprint completo e a mensagem inteira — nada truncado aqui, o detalhe
/// existe justamente para mostrar o valor que a tabela corta.
pub fn detail_lines(row: &FindingRow) -> Vec<String> {
    vec![
        format!("severidade: {}", row.severity),
        format!("ferramenta: {}", row.tool),
        format!("local: {}", row.location()),
        format!("fingerprint: {}", row.fingerprint),
        String::from("mensagem:"),
        row.message.clone(),
    ]
}

/// Quantas sessões o painel de sessões mostra (spec W6: últimas N, default 50).
pub const SESSIONS_LIMIT: usize = 50;

/// Recorte de histórico que a store já consulta para achar o último `scan`
/// (500 cobram qualquer uso real); o painel leva só as [`SESSIONS_LIMIT`]
/// mais recentes desse recorte — uma query, dois consumidores.
const SESSION_HISTORY_LIMIT: usize = 500;

/// Uma linha da tabela de sessões (W6-PR3): o que o painel precisa de um
/// `SessionRow` da store.
///
/// `summary` viaja INTEIRO (≤ [`crate::store::SUMMARY_MAX_CHARS`] chars já
/// gravados pela store) porque o detalhe do `Enter` mostra o valor completo —
/// a tabela corta, o detalhe não. O tempo relativo NÃO é calculado aqui: o
/// stamp unix cru entra e o widget aplica [`humanize_age`] sobre o `now` do
/// snapshot (determinístico, sem relógio dentro da pintura).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionLine {
    /// Id AUTOINCREMENT da store (maior = mais recente).
    pub id: i64,
    pub command: String,
    /// Modelo usado pelo comando; `None`/vazio = comando sem LLM.
    pub model: Option<String>,
    /// `started_at` em unix (s) — `None` = stamp não-parseável (mostra `—`).
    pub started_unix: Option<i64>,
    /// `started_at` cru (RFC 3339 UTC), para o detalhe.
    pub started_at: String,
    /// `started_at` já em hora local para exibição (convenção da store).
    pub local_started_at: Option<String>,
    pub duration_ms: i64,
    /// `false` = falha (linha destacada na tabela).
    pub ok: bool,
    /// Primeira linha do summary (o que cabe na coluna RESUMO).
    pub summary_first_line: String,
    /// Summary inteiro (o detalhe do `Enter` exibe sem corte).
    pub summary: String,
}

/// Converte as sessões da store em linhas do painel: MAIS RECENTE PRIMEIRO
/// (o id cresce com o INSERT AUTOINCREMENT; a ordenação aqui é defensiva e
/// determinística — não depende da ordem do SELECT) e limitada às
/// [`SESSIONS_LIMIT`] últimas. Função pura: teste sem disco. A idade relativa
/// não é calculada aqui — o stamp unix cru entra na linha e
/// [`session_age_line`] aplica [`humanize_age`] sobre o `now` do snapshot.
pub fn session_rows_from(sessions: &[store::SessionRow]) -> Vec<SessionLine> {
    let mut rows: Vec<SessionLine> = sessions
        .iter()
        .map(|session| {
            let summary = session.summary.trim().to_string();
            SessionLine {
                id: session.id,
                command: session.command.trim().to_string(),
                model: session
                    .model
                    .as_ref()
                    .map(|model| model.trim().to_string())
                    .filter(|model| !model.is_empty()),
                started_unix: store::rfc3339_utc_to_unix_seconds(&session.started_at),
                started_at: session.started_at.clone(),
                local_started_at: session.local_started_at.clone(),
                duration_ms: session.duration_ms.max(0),
                ok: session.ok,
                summary_first_line: summary.lines().next().unwrap_or_default().to_string(),
                summary,
            }
        })
        .collect();
    rows.sort_by_key(|row| std::cmp::Reverse(row.id));
    rows.truncate(SESSIONS_LIMIT);
    rows
}

/// Tempo relativo determinístico (W6-PR3): `humanize_age(now, then)` é PURA
/// sobre dois stamps unix em segundos — nenhum `SystemTime::now()` dentro do
/// widget; o `now` vem do snapshot, capturado uma vez em [`load_store`].
/// Relógio adiantado (`then` > `now`) satura em "há 0s" em vez de inventar
/// tempo negativo.
pub fn humanize_age(now_secs: i64, then_secs: i64) -> String {
    format_age_secs((now_secs - then_secs).max(0))
}

/// Coluna QUANDO da tabela: idade relativa pura sobre o `now` do snapshot —
/// `—` quando o snapshot não tem relógio ou o stamp não parseou (honestidade,
/// nunca "há 1970 dias" de mentira).
pub fn session_age_line(line: &SessionLine, now_secs: Option<i64>) -> String {
    match (now_secs, line.started_unix) {
        (Some(now), Some(then)) => humanize_age(now, then),
        _ => "—".to_string(),
    }
}

/// Duração compacta e determinística: `412ms`, `5s`, `1m5s`, `1h05m`.
pub fn format_duration_ms(ms: i64) -> String {
    let ms = ms.max(0);
    if ms < 1_000 {
        return format!("{ms}ms");
    }
    let secs = ms / 1_000;
    if secs < 60 {
        return format!("{secs}s");
    }
    let mins = secs / 60;
    if mins < 60 {
        return format!("{mins}m{}s", secs % 60);
    }
    format!("{}h{:02}m", mins / 60, mins % 60)
}

/// Estado vazio honesto do painel de sessões: store legível sem sessões ⇒
/// `— (store sem sessões)`; store ausente/ilegível ⇒ a causa concreta que o
/// snapshot já carrega — nunca uma causa genérica inventada aqui.
pub fn empty_sessions_line(snapshot: &Snapshot) -> String {
    let cause = snapshot
        .sessions_note
        .clone()
        .or_else(|| snapshot.store_note.clone())
        .unwrap_or_else(|| "store sem sessões".to_string());
    format!("sessões: — ({cause})")
}

/// Linhas de texto do painel de sessões: a MESMA lista alimenta o widget TUI
/// (modo degradado) e a saída do `--snapshot`/não-TTY. Fixas em colunas — sem
/// scroll horizontal em nenhum dos dois mundos (mesma regra dos achados).
pub fn sessions_lines(snapshot: &Snapshot) -> Vec<String> {
    let rows = &snapshot.session_rows;
    if rows.is_empty() {
        return vec![empty_sessions_line(snapshot)];
    }
    let mut lines = vec![
        format!("painel sessões — últimas {}", rows.len()),
        format!(
            "{:<8} {:<10} {:<12} {:<12} {:<8}{}",
            "#ID", "QUANDO", "COMANDO", "MODELO", "DURAÇÃO", "RESUMO"
        ),
    ];
    for row in rows {
        lines.push(format!(
            "{:<8} {:<10} {:<12} {:<12} {:<8}{}",
            format!("#{}", row.id),
            session_age_line(row, snapshot.now_secs),
            row.command,
            row.model.clone().unwrap_or_else(|| "—".to_string()),
            format_duration_ms(row.duration_ms),
            row.summary_first_line,
        ));
    }
    lines
}

/// Linhas do detalhe da sessão aberto pelo `Enter`: identidade, comando,
/// modelo, quando, duração, status e o summary INTEIRO (o que a tabela corta
/// é exatamente o que o detalhe existe para mostrar).
pub fn session_detail_lines(line: &SessionLine, now_secs: Option<i64>) -> Vec<String> {
    vec![
        format!("#{}", line.id),
        format!("comando: {}", line.command),
        format!(
            "modelo: {}",
            line.model.clone().unwrap_or_else(|| "—".to_string())
        ),
        format!("quando: {}", session_age_line(line, now_secs)),
        format!("duração: {}", format_duration_ms(line.duration_ms)),
        format!("status: {}", if line.ok { "ok" } else { "falhou" }),
        String::from("resumo:"),
        line.summary.clone(),
    ]
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

// ----------------------------------------------------------------------
// Painel de wiki (W6-PR4)
// ----------------------------------------------------------------------

/// Limite de leitura do markdown de UMA página (256 KiB): uma página gigante
/// (acidente de conteúdo) não pode encher a memória do dashboard nem travar o
/// desenho — o corte é silencioso por design (o índice continua honesto).
pub const WIKI_PAGE_MAX_BYTES: usize = 256 * 1024;

/// Largura de quebra do markdown no modo texto (`--snapshot`/não-TTY): o TUI
/// quebra pela largura real do painel; o texto fixa 100 colunas.
const WIKI_TEXT_WIDTH: usize = 100;

/// Uma página da wiki para o painel: o que o índice (`wiki_page`) sabe sobre
/// a página + o markdown REAL lido de `.kira/wiki/` (a fonte de verdade é o
/// disco — o `body` da store é derivado e pode estar velho depois de uma
/// edição manual; o `updated_at` do índice dá a idade relativa da lista).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiPage {
    /// Slug da página (ex.: `contexto`, `bugs/4821`).
    pub slug: String,
    /// Tipo do índice (`decisao`, `padrao`, `bug`, `glossario`, ...).
    pub kind: String,
    pub title: String,
    /// Caminho real do markdown no disco, como o índice gravou.
    pub path: String,
    /// `updated_at` cru (RFC 3339 UTC) — para o detalhe.
    pub updated_at: String,
    /// `updated_at` em unix (s) — `None` = stamp não-parseável (mostra `—`).
    pub updated_unix: Option<i64>,
    /// Markdown lido de `.kira/wiki/` com limite de
    /// [`WIKI_PAGE_MAX_BYTES`]; vazio = página ilegível/ausente no disco.
    pub content: String,
}

/// Aplica o resultado da consulta de `wiki_page` ao snapshot — função pura
/// para o contrato fail-open ser unit-testável: índice vazio ⇒ lista vazia +
/// causa acionável, NUNCA pânico nem dado inventado.
fn apply_wiki(
    snapshot: &mut Snapshot,
    loaded: Result<Vec<store::WikiPageRow>, store::StoreError>,
    root: &Path,
) {
    match loaded {
        Ok(rows) => {
            snapshot.wiki_rows = wiki_rows_from(&rows, root);
            if rows.is_empty() {
                // A causa honesta é o comando que conserta: o índice é
                // derivado, então `kira wiki reindex` o reconstrói do disco.
                snapshot.wiki_note = Some("wiki vazia — rode kira wiki reindex".to_string());
            }
        }
        Err(err) => {
            snapshot.wiki_note = Some(format!("índice da wiki ilegível ({err})"));
            snapshot.wiki_rows = Vec::new();
        }
    }
}

/// Lê no máximo `max_bytes` de um arquivo; `None` = ausente/ilegível.
///
/// `String::from_utf8_lossy` em vez de `read_to_string`: um markdown com
/// byte quebrado nunca impede o painel de abrir (fail-open, como todo o
/// dashboard).
fn read_capped(path: &Path, max_bytes: usize) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let mut buffer = Vec::new();
    file.take(max_bytes as u64).read_to_end(&mut buffer).ok()?;
    Some(String::from_utf8_lossy(&buffer).into_owned())
}

/// Markdown de UMA página: o caminho que o índice gravou primeiro; se o
/// arquivo sumiu/se mudou (o índice é derivado, pode estar velho), tenta o
/// caminho canônico `.kira/wiki/<slug>.md`. Vazio = nada no disco — o painel
/// mostra a página com a nota de conteúdo ausente, nunca inventa texto.
fn read_page_content(path: &str, root: &Path, slug: &str) -> String {
    let stored = Path::new(path);
    if let Some(content) = read_capped(stored, WIKI_PAGE_MAX_BYTES) {
        return content;
    }
    let fallback = root.join(crate::wiki::WIKI_DIR).join(format!("{slug}.md"));
    read_capped(&fallback, WIKI_PAGE_MAX_BYTES).unwrap_or_default()
}

/// Converte as linhas de `wiki_page` em páginas do painel. Função pura
/// (a leitura de disco é a única parte I/O, e é read-only). A ordem é a do
/// índice (slug), que já é determinística.
pub fn wiki_rows_from(rows: &[store::WikiPageRow], root: &Path) -> Vec<WikiPage> {
    rows.iter()
        .map(|row| WikiPage {
            slug: row.slug.trim().to_string(),
            kind: row.kind.trim().to_string(),
            title: row.title.trim().to_string(),
            path: row.path.trim().to_string(),
            updated_at: row.updated_at.trim().to_string(),
            updated_unix: store::rfc3339_utc_to_unix_seconds(row.updated_at.trim()),
            content: read_page_content(&row.path, root, &row.slug),
        })
        .collect()
}

/// Busca incremental do painel (`/`) — função PURA e testada: filtra por
/// título E conteúdo, case-insensitive E sem acento-sensível (o dono digita
/// "sessao" e acha "Sessão"). Agulha vazia = todas as páginas.
pub fn filter_pages<'a>(pages: &'a [WikiPage], needle: &str) -> Vec<&'a WikiPage> {
    let folded = fold_text(needle);
    if folded.is_empty() {
        return pages.iter().collect();
    }
    pages
        .iter()
        .filter(|page| {
            fold_text(&page.title).contains(&folded) || fold_text(&page.content).contains(&folded)
        })
        .collect()
}

/// Normaliza um texto para comparação: minúsculas + tirar acento comum
/// (pt-BR/latino). Sem dependência nova — a tabela cobre o alfabeto que a
/// wiki realmente usa; qualquer outro char passa como está.
pub fn fold_text(text: &str) -> String {
    text.chars()
        .flat_map(|c| c.to_lowercase())
        .map(deaccent)
        .collect()
}

/// Um acento latino → letra base (só exibição de busca, nunca reescreve dado).
fn deaccent(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        'ý' | 'ÿ' => 'y',
        _ => c,
    }
}

/// Coluna ATUALIZAÇÃO da lista de páginas: idade relativa pura sobre o `now`
/// do snapshot — `—` quando não há relógio ou o stamp não parseou.
pub fn wiki_age_line(page: &WikiPage, now_secs: Option<i64>) -> String {
    match (now_secs, page.updated_unix) {
        (Some(now), Some(then)) => humanize_age(now, then),
        _ => "—".to_string(),
    }
}

/// Estado vazio honesto do painel de wiki: índice vazio com store legível ⇒
/// `— (wiki vazia — rode kira wiki reindex)`; store ausente/ilegível ⇒ a
/// causa concreta que o snapshot já carrega — nunca uma genérica inventada.
pub fn empty_wiki_line(snapshot: &Snapshot) -> String {
    let cause = snapshot
        .wiki_note
        .clone()
        .or_else(|| snapshot.store_note.clone())
        .unwrap_or_else(|| "wiki vazia — rode kira wiki reindex".to_string());
    format!("wiki: — ({cause})")
}

/// Linhas de texto da LISTA de páginas (mesma lista da TUI no modo
/// degradado + `--snapshot`). Fixas em colunas — sem scroll horizontal.
pub fn wiki_lines(snapshot: &Snapshot) -> Vec<String> {
    let pages = &snapshot.wiki_rows;
    if pages.is_empty() {
        return vec![empty_wiki_line(snapshot)];
    }
    let noun = if pages.len() == 1 {
        "página"
    } else {
        "páginas"
    };
    let mut lines = vec![
        format!("painel wiki — {} {noun}", pages.len()),
        format!("{:<10} {:<30}{}", "TIPO", "TÍTULO", "ATUALIZAÇÃO"),
    ];
    for page in pages {
        lines.push(format!(
            "{:<10} {:<30}{}",
            page.kind,
            page.title,
            wiki_age_line(page, snapshot.now_secs)
        ));
    }
    lines
}

/// Preview renderizado de UMA página (o que `Enter` abre em tela cheia na
/// TUI e o `--snapshot` imprime em texto): renderizador próprio, sem `#`
/// nem `**` nem HTML executado — o dono lê um blog dentro do terminal.
pub fn wiki_preview_lines(page: &WikiPage, width: usize) -> Vec<String> {
    let mut lines = vec![format!("página: {} ({})", page.title, page.kind)];
    let rendered = crate::tui::markdown::render(&page.content, width);
    if rendered.is_empty() {
        // Markdown ausente/ilegível no disco: a causa, nunca texto inventado.
        lines.push("(conteúdo ausente em .kira/wiki/ — rode kira wiki reindex)".to_string());
    } else {
        lines.extend(rendered);
    }
    lines
}

/// Linha `— (causa)` de um dado ausente — o contrato de honestidade.
fn dash_line(cause: &str) -> String {
    format!("— ({cause})")
}

/// Máximo de CHARS de um VALOR do cabeçalho (100): o painel é denso e cada
/// campo ocupa exatamente uma linha — a causa integral vai para o `?` e para
/// o `--snapshot` (texto, que aceita várias linhas), nunca para o painel.
pub const HEADER_VALUE_MAX_CHARS: usize = 100;

/// Colapsa um valor em EXATAMENTE UMA linha (regra do dono: quebrar linha no
/// cabeçalho é erro imperdoável — o defeito real de fumaça veio da causa de
/// duas linhas do PAT). CR/LF viram espaço, espaços múltiplos colapsam e o
/// resultado é truncado em `max` CHARS com `…` — conta `char`, nunca byte,
/// então um caractere multibyte nunca é cortado ao meio. Função pura.
pub fn one_line(value: &str, max: usize) -> String {
    let mut line = String::new();
    let mut pending_space = false;
    for ch in value.chars() {
        let is_space = matches!(ch, ' ' | '\t' | '\n' | '\r');
        if is_space {
            // Espaço só entra se já há conteúdo (também apara bordas) e uma
            // única vez por sequência — `\r\n` e tabuladas viram um espaço.
            if !line.is_empty() {
                pending_space = true;
            }
            continue;
        }
        if pending_space {
            line.push(' ');
            pending_space = false;
        }
        line.push(ch);
    }
    if line.chars().count() > max {
        // Total ≤ max: `max - 1` chars de conteúdo + o `…` do corte.
        let kept: String = line.chars().take(max.saturating_sub(1)).collect();
        return format!("{kept}…");
    }
    line
}

/// Causa INTEGRAL de cada dado ausente cujo cabeçalho corta para caber em uma
/// linha (pat, dev, store, achados): alimentam a seção de pendências do `?` e
/// do `--snapshot` — texto, que aceita várias linhas. Pura sobre o snapshot.
pub fn pending_causes(snapshot: &Snapshot) -> Vec<String> {
    let causes = [
        ("pat", snapshot.pat_note.as_ref()),
        ("dev", snapshot.dev_note.as_ref()),
        ("store", snapshot.store_note.as_ref()),
        ("achados", snapshot.findings_note.as_ref()),
    ];
    let mut lines = Vec::new();
    for (label, note) in causes {
        let Some(note) = note else { continue };
        for (index, line) in note.lines().enumerate() {
            if index == 0 {
                // O rótulo usa ` — ` (e não `: `) para nunca colidir com o
                // campo de uma linha do cabeçalho (`pat: —`).
                lines.push(format!("· {label} — {line}"));
            } else {
                // Continuação indentada: é texto, pode ter várias linhas.
                lines.push(format!("  {line}"));
            }
        }
    }
    lines
}

/// Linhas do cabeçalho, puras sobre o snapshot: a MESMA lista alimenta o
/// widget TUI e a saída de texto do `--snapshot`/não-TTY (CI).
///
/// A store ausente não esconde o cabeçalho: projeto/root saem como `—` com a
/// nota acionável na última linha (contrato 4 da spec — exit 0 é diagnóstico).
pub fn header_lines(snapshot: &Snapshot) -> Vec<String> {
    // REGRA (dono): cada campo do cabeçalho ocupa EXATAMENTE uma linha — todo
    // valor passa por [`one_line`] (CR/LF colapsados, corte em 100 chars com
    // `…`); a causa integral vai para o `?`/`--snapshot` via
    // [`pending_causes`].
    let max = HEADER_VALUE_MAX_CHARS;
    let mut lines = Vec::new();
    match &snapshot.project {
        Some(project) => {
            lines.push(format!("projeto: {}", one_line(&project.name, max)));
            lines.push(format!(
                "root: {}",
                one_line(&store::display_path(Path::new(&project.root_path)), max)
            ));
            lines.push(format!("id: {}", one_line(&project.id, max)));
        }
        None => {
            lines.push(format!(
                "projeto: {}",
                one_line(
                    &dash_line(
                        snapshot
                            .store_note
                            .as_deref()
                            .unwrap_or("store sem projeto")
                    ),
                    max
                )
            ));
            lines.push(format!(
                "root: {}",
                one_line(
                    &snapshot
                        .root
                        .as_ref()
                        .map(|root| store::display_path(root))
                        .unwrap_or_else(|| dash_line("sem diretório atual")),
                    max
                )
            ));
            lines.push(format!("id: {}", dash_line("sem projeto no store")));
        }
    }
    match &snapshot.dev_name {
        Some(name) => lines.push(format!("dev: {}", one_line(name, max))),
        None => lines.push(format!(
            "dev: {}",
            one_line(
                &dash_line(
                    snapshot
                        .dev_note
                        .as_deref()
                        .unwrap_or("sem dev_name no config")
                ),
                max
            )
        )),
    }
    lines.push(format!("llm: {}", one_line(LLM_LABEL, max)));
    match &snapshot.pat_masked {
        Some(masked) => {
            let value = format!(
                "{masked} (de {})",
                snapshot
                    .pat_source
                    .as_deref()
                    .unwrap_or("origem desconhecida")
            );
            lines.push(format!("pat: {}", one_line(&value, max)));
        }
        None => {
            // A causa integral (duas linhas, com a dica do `kira doctor`)
            // estourava o campo — defeito real de fumaça. No painel, só o
            // ponteiro para o `?`; o texto inteiro está em [`pending_causes`].
            lines.push(format!(
                "pat: {}",
                dash_line("PAT não encontrado — veja ? para o passo a passo")
            ));
        }
    }
    match snapshot.findings {
        Some(count) => lines.push(format!("achados: {count}")),
        None => lines.push(format!(
            "achados: {}",
            one_line(
                &dash_line(
                    snapshot
                        .findings_note
                        .as_deref()
                        .unwrap_or("nunca rodou kira scan")
                ),
                max
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
    // O campo `store:` saiu do cabeçalho: dizia o mesmo que `projeto: — (…)`
    // e a causa integral já aparece nos painéis que dependem do store (e em
    // [`pending_causes`]).
    lines
}

/// Rodapé de 1 linha com as teclas — mesmo texto no TUI e no snapshot.
pub fn footer_line() -> String {
    "teclas: q sair · Tab/Shift+Tab painel · j/k ou ↑/↓ mover · g/G topo/fim · / busca wiki · r reler disco · ? ajuda".to_string()
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
    // Pendências (causa INTEGRAL, pode ter várias linhas — é texto, não
    // painel): o cabeçalho corta em 100 chars, o snapshot não mente.
    let pendings = pending_causes(snapshot);
    if !pendings.is_empty() {
        text.push_str("\npendências:\n");
        for line in pendings {
            text.push_str(&line);
            text.push('\n');
        }
    }
    // Painel de achados (W6-PR2): mesma informação da TUI, em colunas de texto.
    let findings = findings_lines(snapshot);
    if !findings.is_empty() {
        text.push('\n');
        for line in findings {
            text.push_str(&line);
            text.push('\n');
        }
    }
    // Painel de sessões (W6-PR3): mesmas linhas da tabela da TUI.
    let sessions = sessions_lines(snapshot);
    if !sessions.is_empty() {
        text.push('\n');
        for line in sessions {
            text.push_str(&line);
            text.push('\n');
        }
    }
    // Painel de wiki (W6-PR4): lista + o preview renderizado de cada página
    // (a MESMA saída do renderizador que o `Enter` abre na TUI).
    let wiki = wiki_lines(snapshot);
    if !wiki.is_empty() {
        text.push('\n');
        for line in wiki {
            text.push_str(&line);
            text.push('\n');
        }
    }
    for page in &snapshot.wiki_rows {
        for line in wiki_preview_lines(page, WIKI_TEXT_WIDTH) {
            text.push_str(&line);
            text.push('\n');
        }
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
        let lines = header_lines(&snapshot);
        let text = lines.join("\n");
        assert!(text.contains("projeto: — (nenhuma trilha em"), "{text}");
        // O campo `store:` saiu do cabeçalho: dizia o mesmo que `projeto: —`
        // (duplicação) — a causa integral agora mora nas pendências.
        assert!(
            !lines.iter().any(|line| line.starts_with("store:")),
            "campo store removido do cabeçalho: {text}"
        );
        let pendings = pending_causes(&snapshot).join("\n");
        assert!(
            pendings.contains("· store — nenhuma trilha em"),
            "causa integral do store nas pendências: {pendings}"
        );
    }

    // ------------------------------------------------------------------
    // Disciplina de linha (regra do dono: quebrar linha é erro imperdoável)
    // ------------------------------------------------------------------

    #[test]
    fn one_line_collapses_breaks_and_spaces_and_never_returns_two_lines() {
        // CR/LF (isolados ou na sequência `\r\n`) viram UM espaço.
        assert_eq!(one_line("a\nb", HEADER_VALUE_MAX_CHARS), "a b");
        assert_eq!(one_line("a\r\nb", HEADER_VALUE_MAX_CHARS), "a b");
        assert_eq!(one_line("a\r\rb", HEADER_VALUE_MAX_CHARS), "a b");
        // Espaços múltiplos (e tab) colapsam; bordas aparadas.
        assert_eq!(one_line("a   b\t c", HEADER_VALUE_MAX_CHARS), "a b c");
        assert_eq!(one_line("   ", HEADER_VALUE_MAX_CHARS), "");
        // Vazio e valor curto passam intactos.
        assert_eq!(one_line("", HEADER_VALUE_MAX_CHARS), "");
        assert_eq!(one_line("curto", HEADER_VALUE_MAX_CHARS), "curto");
        // A garantia principal: NUNCA devolve mais de uma linha.
        let multi = "primeira\nsegunda\r\n\r\n\tterceira";
        let line = one_line(multi, HEADER_VALUE_MAX_CHARS);
        assert!(!line.contains('\n'), "{line}");
        assert!(!line.contains('\r'), "{line}");
    }

    #[test]
    fn one_line_truncates_exactly_at_the_max_never_splitting_a_multibyte_char() {
        // Exatamente no limite: intacto (sem `…` de sobra).
        let at_limit = "x".repeat(HEADER_VALUE_MAX_CHARS);
        assert_eq!(one_line(&at_limit, HEADER_VALUE_MAX_CHARS), at_limit);
        // Um char acima do limite: total == max, terminando no `…`.
        let cut = one_line(
            &"x".repeat(HEADER_VALUE_MAX_CHARS + 1),
            HEADER_VALUE_MAX_CHARS,
        );
        assert_eq!(cut.chars().count(), HEADER_VALUE_MAX_CHARS);
        assert!(cut.ends_with('…'), "{cut}");
        // Multibyte no limite: conta CHAR (não byte) — nenhum corte ao meio,
        // nenhum pânico; todo char do resultado é válido.
        let accents = "á".repeat(HEADER_VALUE_MAX_CHARS + 5);
        let cut = one_line(&accents, HEADER_VALUE_MAX_CHARS);
        assert_eq!(cut.chars().count(), HEADER_VALUE_MAX_CHARS);
        assert!(
            cut.chars().all(|c| c == 'á' || c == '…'),
            "chars íntegros: {cut}"
        );
    }

    #[test]
    fn missing_pat_shows_the_short_pointer_and_the_full_cause_goes_to_pendings() {
        // Defeito real de fumaça: a causa de DUAS linhas do PAT estourava o
        // campo. No cabeçalho, ponteiro curto; no `?`/snapshot, o texto todo.
        let snapshot = Snapshot {
            pat_note: Some(
                "PAT não encontrado — defina a variável de ambiente AZURE_DEVOPS_PAT com um \
                 Personal Access Token (ex.: export AZURE_DEVOPS_PAT=...)\n\
                 para diagnosticar a configuração desta máquina, rode `kira doctor`"
                    .to_string(),
            ),
            ..Snapshot::default()
        };
        let header = header_lines(&snapshot).join("\n");
        assert!(
            header.contains("pat: — (PAT não encontrado — veja ? para o passo a passo)"),
            "ponteiro curto no painel: {header}"
        );
        assert!(
            !header.contains("defina a variável"),
            "a causa integral NUNCA vai para o painel: {header}"
        );
        let pendings = pending_causes(&snapshot).join("\n");
        assert!(
            pendings.contains("· pat — PAT não encontrado — defina a variável"),
            "{pendings}"
        );
        assert!(
            pendings.contains("para diagnosticar a configuração desta máquina, rode `kira doctor`"),
            "a dica do doctor inteira nas pendências: {pendings}"
        );
        // O `--snapshot` (texto) carrega a seção de pendências inteira.
        let report = text_report(&snapshot);
        assert!(report.contains("\npendências:\n"), "{report}");
        assert!(report.contains("kira doctor"), "{report}");
    }

    #[test]
    fn every_header_line_is_a_single_line_within_the_value_budget() {
        // Causas longas e multilinha em TODOS os campos de uma vez: a
        // disciplina de linha vale para o cabeçalho inteiro, não só o pat.
        let long = "causa longa ".repeat(30) + "\nsegunda linha que NUNCA vaza";
        let snapshot = Snapshot {
            store_note: Some(long.clone()),
            dev_note: Some(long.clone()),
            pat_note: Some(long.clone()),
            findings_note: Some(long),
            ..Snapshot::default()
        };
        for line in header_lines(&snapshot) {
            assert!(!line.contains('\n') && !line.contains('\r'), "{line}");
            // O VALOR cabe no orçamento (o rótulo `campo: ` vem por fora).
            let value = line
                .split_once(": ")
                .map(|(_, value)| value)
                .unwrap_or(&line);
            assert!(
                value.chars().count() <= HEADER_VALUE_MAX_CHARS,
                "valor acima do orçamento: {line}"
            );
        }
        // E a seção de pendências carrega a segunda linha integral (texto).
        let pendings = pending_causes(&snapshot).join("\n");
        assert!(
            pendings.contains("segunda linha que NUNCA vaza"),
            "{pendings}"
        );
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

    /// Achados de referência (severidades canônicas + uma fora do padrão) que
    /// os testes de agregação e de ordenação compartilham.
    fn fixture_finding(tool: &str, severity: &str, path: &str, message: &str) -> Finding {
        Finding {
            tool: tool.to_string(),
            rule: "outdated".to_string(),
            package: "pkg".to_string(),
            version: "1.0.0".to_string(),
            severity: severity.to_string(),
            path: path.to_string(),
            message: message.to_string(),
            fingerprint: "0123456789abcdef".to_string(),
        }
    }

    #[test]
    fn rows_are_sorted_critical_to_low_and_normalize_display() {
        let rows = rows_from(vec![
            fixture_finding("dotnet", "Low", "App.csproj", "pacote deprecado"),
            fixture_finding("audit", "High", "package.json", "advisory"),
            fixture_finding("audit", "CRITICAL", "src/lib.rs:42", "Prototype Pollution"),
            fixture_finding("npm", "moderate", "package.json", "fora do padrão"),
        ]);
        let severities: Vec<&str> = rows.iter().map(|row| row.severity.as_str()).collect();
        assert_eq!(
            severities,
            // As quatro canônicas em ordem canônica (low antes do fora-de-
            // padrão, que ganha o último rank) — determinismo, nunca sumiço.
            vec!["critical", "high", "low", "moderate"],
            "ordem canônica com o fora-de-padrão depois das canônicas: {rows:?}"
        );
        // Severidade normalizada para exibição (comparação é só de tela).
        assert_eq!(rows[0].severity, "critical");
        assert_eq!(rows[3].severity, "moderate");
        // E a case crua do arquivo não vaza na exibição ("CRITICAL" → "critical").
        assert_eq!(rows[0].path, "src/lib.rs");
        assert_eq!(rows[0].line, Some(42));
    }

    #[test]
    fn location_shows_path_line_when_the_path_carries_one() {
        let rows = rows_from(vec![fixture_finding(
            "audit",
            "high",
            "src/lib.rs:42",
            "Prototype Pollution",
        )]);
        assert_eq!(rows[0].path, "src/lib.rs");
        assert_eq!(rows[0].line, Some(42));
        assert_eq!(rows[0].location(), "src/lib.rs:42");
        // Windows drive letter: o `:` da unidade NÃO vira linha falsa.
        let windows = rows_from(vec![fixture_finding(
            "dotnet",
            "low",
            "C:\\proj\\App.csproj",
            "m",
        )]);
        assert_eq!(windows[0].line, None);
        assert_eq!(windows[0].location(), "C:\\proj\\App.csproj");
    }

    #[test]
    fn counts_aggregate_by_severity_and_tool() {
        let rows = rows_from(vec![
            fixture_finding("audit", "critical", "src/a.rs", "A"),
            fixture_finding("audit", "High", "src/b.rs", "B"),
            fixture_finding("dotnet", "high", "App.csproj", "C"),
        ]);
        assert_eq!(
            severity_counts(&rows),
            vec![
                ("critical".to_string(), 1),
                ("high".to_string(), 2),
                ("medium".to_string(), 0),
                ("low".to_string(), 0),
            ]
        );
        assert_eq!(
            tool_counts(&rows),
            vec![("audit".to_string(), 2), ("dotnet".to_string(), 1)]
        );
        // O cabeçalho do painel mostra as QUATRO severidades, mesmo com zero.
        assert_eq!(
            count_header_line(&rows),
            "critical 1 · high 2 · medium 0 · low 0"
        );
        assert_eq!(tools_line(&rows), "ferramentas: audit 2, dotnet 1");
    }

    #[test]
    fn message_first_line_carries_only_the_first_line_but_detail_keeps_all() {
        let rows = rows_from(vec![fixture_finding(
            "audit",
            "high",
            "src/lib.rs:42",
            "Prototype Pollution\nCVE-2020-8203 — rode `npm audit fix`",
        )]);
        assert_eq!(rows[0].message_first_line, "Prototype Pollution");
        assert_eq!(rows[0].fingerprint_short, "01234567");
        assert_eq!(rows[0].fingerprint, "0123456789abcdef");
        let detail = detail_lines(&rows[0]).join("\n");
        assert!(detail.contains("severidade: high"), "{detail}");
        assert!(detail.contains("local: src/lib.rs:42"), "{detail}");
        assert!(detail.contains("fingerprint: 0123456789abcdef"), "{detail}");
        assert!(
            detail.contains("CVE-2020-8203"),
            "o detalhe mostra a mensagem INTEIRA: {detail}"
        );
    }

    #[test]
    fn corrupted_findings_fail_open_to_empty_list_with_the_real_cause() {
        // JSON corrompido: lista vazia + a causa da leitura — fail-open
        // (nada de pânico, nada de dado inventado), igual ao store ausente.
        let mut snapshot = Snapshot::default();
        apply_findings(
            &mut snapshot,
            Err(scan::ScanError::Runtime {
                message: "findings.json ilegível (EOF) — rode `kira scan` de novo".to_string(),
            }),
        );
        assert!(
            snapshot.finding_rows.is_empty(),
            "{:?}",
            snapshot.finding_rows
        );
        assert!(snapshot.findings.is_none());
        let note = snapshot.findings_note.clone().unwrap_or_default();
        assert!(note.contains("ilegível"), "{note}");
        // O painel traz o conteúdo SEM repetir o rótulo `achados:` (o campo
        // vive uma única vez, no cabeçalho).
        assert!(empty_findings_line(&snapshot).starts_with("— ("), "{note}");
    }

    #[test]
    fn empty_state_is_honest_about_absent_file_vs_zero_findings() {
        // Arquivo ausente: `—` com a causa (cópia da que o load Findings grava).
        let absent = Snapshot {
            findings_note: Some("nunca rodou kira scan".to_string()),
            ..Snapshot::default()
        };
        assert_eq!(empty_findings_line(&absent), "— (nunca rodou kira scan)");
        // Arquivo presente e vazio: contagem zero explícita, sem `—` de mentira.
        let zero = Snapshot {
            findings: Some(0),
            ..Snapshot::default()
        };
        assert_eq!(empty_findings_line(&zero), "(0 achados no último scan)");
        // E `findings_lines` devolve exatamente essa linha no estado vazio.
        assert_eq!(findings_lines(&zero), vec!["(0 achados no último scan)"]);
    }

    #[test]
    fn findings_lines_show_counts_columns_and_locations() {
        let mut snapshot = Snapshot::default();
        apply_findings(
            &mut snapshot,
            Ok(Some(vec![
                fixture_finding("audit", "critical", "src/lib.rs:42", "Prototype Pollution"),
                fixture_finding("dotnet", "High", "App.csproj", "pacote deprecado"),
                fixture_finding("audit", "high", "package.json", "advisory"),
            ])),
        );
        let lines = findings_lines(&snapshot).join("\n");
        assert!(
            lines.contains("painel achados — critical 1 · high 2 · medium 0 · low 0"),
            "{lines}"
        );
        assert!(lines.contains("ferramentas: audit 2, dotnet 1"), "{lines}");
        assert!(lines.contains("SEVERIDADE"), "{lines}");
        assert!(lines.contains("src/lib.rs:42"), "{lines}");
        assert!(lines.contains("Prototype Pollution"), "{lines}");
    }

    // ------------------------------------------------------------------
    // Painel de sessões (W6-PR3)
    // ------------------------------------------------------------------

    /// Sessão de referência da store (o que `last_sessions` devolve).
    fn fixture_session(
        id: i64,
        command: &str,
        model: Option<&str>,
        started_at: &str,
        duration_ms: i64,
        ok: bool,
        summary: &str,
    ) -> store::SessionRow {
        store::SessionRow {
            id,
            command: command.to_string(),
            args: String::new(),
            model: model.map(|model| model.to_string()),
            started_at: started_at.to_string(),
            local_started_at: Some("2026-11-11 09:00:00".to_string()),
            duration_ms,
            ok,
            summary: summary.to_string(),
        }
    }

    #[test]
    fn session_rows_are_most_recent_first_and_limited_to_fifty() {
        // Ordem de entrada deliberadamente embaralhada: a ordenação do painel
        // é da função pura (id maior = mais recente), não do SELECT.
        let sessions = vec![
            fixture_session(
                3,
                "doctor",
                None,
                "2026-11-11T09:00:00Z",
                400,
                true,
                "saudável",
            ),
            fixture_session(1, "scan", None, "2026-11-09T09:00:00Z", 1_200, true, "ok"),
            fixture_session(
                2,
                "triage",
                Some(" qwen2.5:7b "),
                "2026-11-10T09:00:00Z",
                65_000,
                true,
                "nota gravada",
            ),
        ];
        let rows = session_rows_from(&sessions);
        let ids: Vec<i64> = rows.iter().map(|row| row.id).collect();
        assert_eq!(ids, vec![3, 2, 1], "mais recente primeiro: {rows:?}");
        // Modelo normalizado (trim; vazio nunca vira string fantasma).
        assert_eq!(rows[1].model.as_deref(), Some("qwen2.5:7b"));
        assert_eq!(rows[0].model, None);
        // Falha preservada (coluna destacada no widget).
        let failed = fixture_session(4, "scan", None, "2026-11-11T10:00:00Z", 100, false, "boom");
        assert!(!session_rows_from(&[failed])[0].ok);
    }

    #[test]
    fn session_rows_truncate_to_the_fifty_most_recent() {
        let sessions: Vec<store::SessionRow> = (1..=55)
            .rev()
            .map(|id| fixture_session(id, "scan", None, "2026-11-11T09:00:00Z", 100, true, "s"))
            .collect();
        let rows = session_rows_from(&sessions);
        assert_eq!(rows.len(), SESSIONS_LIMIT, "limite da spec: {rows:?}");
        // Ficam as 50 MAIS recentes (ids 55..=6), nunca as antigas.
        assert_eq!(rows[0].id, 55);
        assert_eq!(rows[SESSIONS_LIMIT - 1].id, 6);
    }

    #[test]
    fn humanize_age_is_pure_and_deterministic_minutes_hours_days() {
        let now = 1_000_000_i64;
        assert_eq!(humanize_age(now, now), "há 0s");
        assert_eq!(humanize_age(now, now - 45), "há 45s");
        // Minutos: 180s = "há 3 min" (o caso fixo da spec).
        assert_eq!(humanize_age(now, now - 180), "há 3 min");
        assert_eq!(humanize_age(now, now - 7_200), "há 2 h");
        assert_eq!(humanize_age(now, now - 86_400), "há 1 dia");
        assert_eq!(humanize_age(now, now - 5 * 86_400), "há 5 dias");
        // Relógio adiantado: satura em 0s, nunca tempo negativo.
        assert_eq!(humanize_age(now, now + 60), "há 0s");
    }

    #[test]
    fn duration_formats_ms_seconds_minutes_hours() {
        assert_eq!(format_duration_ms(412), "412ms");
        assert_eq!(format_duration_ms(1_200), "1s");
        assert_eq!(format_duration_ms(65_000), "1m5s");
        assert_eq!(format_duration_ms(3_900_000), "1h05m");
        // Duração negativa (não deveria existir) não vira lixo de exibição.
        assert_eq!(format_duration_ms(-7), "0ms");
    }

    #[test]
    fn session_age_line_falls_back_to_dash_without_clock_or_stamp() {
        // Stamp que a store não escreve (o formato real é RFC 3339 UTC):
        // sem idade inventada — `—` honesto.
        let broken = fixture_session(1, "scan", None, "stamp quebrado", 100, true, "s");
        let parsed = session_rows_from(std::slice::from_ref(&broken));
        assert!(parsed[0].started_unix.is_none());
        assert_eq!(session_age_line(&parsed[0], Some(1_762_864_800)), "—");
        // E sem `now` no snapshot também (o relógio do snapshot é o único now).
        let ok_stamp = fixture_session(2, "scan", None, "2026-11-11T09:00:00Z", 100, true, "s");
        let parsed = session_rows_from(std::slice::from_ref(&ok_stamp));
        assert!(parsed[0].started_unix.is_some());
        assert_eq!(session_age_line(&parsed[0], None), "—");
    }

    #[test]
    fn sessions_lines_show_id_when_command_model_duration_and_first_line() {
        let sessions = vec![
            fixture_session(
                3,
                "scan",
                None,
                "2026-11-11T09:00:00Z",
                1_200,
                true,
                "scan fixture: 3 achados\nsegunda linha que NÃO vai para a tabela",
            ),
            fixture_session(
                2,
                "triage",
                Some("qwen2.5:7b"),
                "2026-11-10T09:00:00Z",
                65_000,
                true,
                "nota gravada no episódio",
            ),
        ];
        let snapshot = Snapshot {
            session_rows: session_rows_from(&sessions),
            now_secs: Some(1_762_864_800),
            ..Snapshot::default()
        };
        let lines = sessions_lines(&snapshot).join("\n");
        assert!(lines.contains("painel sessões — últimas 2"), "{lines}");
        assert!(lines.contains("#ID"), "{lines}");
        assert!(lines.contains("QUANDO"), "{lines}");
        assert!(lines.contains("#3"), "{lines}");
        assert!(lines.contains("scan fixture: 3 achados"), "{lines}");
        // Só a 1ª linha do summary vai para a tabela — o resto é do detalhe.
        assert!(
            !lines.contains("segunda linha"),
            "a tabela NUNCA vaza o summary inteiro: {lines}"
        );
        assert!(lines.contains("triage"), "{lines}");
        assert!(lines.contains("qwen2.5:7b"), "{lines}");
        assert!(lines.contains("1m5s"), "{lines}");
        assert!(lines.contains("há"), "idade relativa presente: {lines}");
    }

    #[test]
    fn session_detail_keeps_the_whole_summary_but_respects_the_2000_cap() {
        // A store já grava no máximo 2000 chars (`cap_summary`); o detalhe
        // mostra o valor que veio do disco, inteiro, sem truncar de novo.
        let long = "linha um\n".repeat(300);
        let sessions = vec![fixture_session(
            7,
            "scan",
            None,
            "2026-11-11T09:00:00Z",
            100,
            true,
            long.trim_end(),
        )];
        let rows = session_rows_from(&sessions);
        assert_eq!(rows[0].summary_first_line, "linha um");
        let detail = session_detail_lines(&rows[0], Some(0)).join("\n");
        assert!(detail.contains("#7"), "{detail}");
        assert!(detail.contains("comando: scan"), "{detail}");
        assert!(detail.contains("modelo: —"), "{detail}");
        assert!(detail.contains("duração: 100ms"), "{detail}");
        assert!(detail.contains("status: ok"), "{detail}");
        assert!(
            detail.matches("linha um").count() >= 300,
            "o summary INTEIRO está no detalhe: {detail}"
        );
        // Sessão falha: status honesto na tela.
        let failed = fixture_session(
            8,
            "doctor",
            None,
            "2026-11-11T09:00:00Z",
            100,
            false,
            "boom",
        );
        let detail = session_detail_lines(&session_rows_from(&[failed])[0], Some(0)).join("\n");
        assert!(detail.contains("status: falhou"), "{detail}");
    }

    #[test]
    fn empty_sessions_line_is_honest_about_the_real_cause() {
        // Store legível, zero sessões: a causa vem de `sessions_note`.
        let snapshot = Snapshot {
            sessions_note: Some("store sem sessões".to_string()),
            ..Snapshot::default()
        };
        assert_eq!(
            empty_sessions_line(&snapshot),
            "sessões: — (store sem sessões)"
        );
        // Store ausente: a causa concreta do store (nunca inventada aqui).
        let absent = Snapshot {
            store_note: Some(
                "nenhuma trilha em store.db — rode `kira scan` ou `kira doctor`".to_string(),
            ),
            ..Snapshot::default()
        };
        assert_eq!(
            empty_sessions_line(&absent),
            "sessões: — (nenhuma trilha em store.db — rode `kira scan` ou `kira doctor`)"
        );
        // E `sessions_lines` devolve exatamente essa linha no estado vazio.
        assert_eq!(
            sessions_lines(&absent),
            vec!["sessões: — (nenhuma trilha em store.db — rode `kira scan` ou `kira doctor`)"]
        );
    }

    #[test]
    fn text_report_carries_the_sessions_table_too() {
        let snapshot = Snapshot {
            session_rows: session_rows_from(&[fixture_session(
                1,
                "scan",
                None,
                "2026-11-11T09:00:00Z",
                1_200,
                true,
                "scan fixture",
            )]),
            ..Snapshot::default()
        };
        let report = text_report(&snapshot);
        assert!(report.contains("painel sessões — últimas 1"), "{report}");
        assert!(report.contains("#1"), "{report}");
        assert!(report.contains("scan fixture"), "{report}");
    }

    // ------------------------------------------------------------------
    // Painel de wiki (W6-PR4)
    // ------------------------------------------------------------------

    /// Página de referência da wiki (o que `wiki_rows_from` devolve).
    fn fixture_page(
        slug: &str,
        kind: &str,
        title: &str,
        updated_at: &str,
        content: &str,
    ) -> WikiPage {
        WikiPage {
            slug: slug.to_string(),
            kind: kind.to_string(),
            title: title.to_string(),
            path: String::new(),
            updated_at: updated_at.to_string(),
            updated_unix: store::rfc3339_utc_to_unix_seconds(updated_at),
            content: content.to_string(),
        }
    }

    #[test]
    fn filter_pages_matches_title_and_content_case_and_accent_insensitive() {
        let pages = vec![
            fixture_page(
                "contexto",
                "contexto",
                "Contexto do projeto",
                "2026-11-11T09:00:00Z",
                "sessão expirada em produção",
            ),
            fixture_page(
                "bugs/4821",
                "bug",
                "Sessão 4821",
                "2026-11-10T09:00:00Z",
                "token expirado",
            ),
        ];
        // Agulha vazia = tudo.
        assert_eq!(filter_pages(&pages, "").len(), 2);
        // Título, case-insensitive.
        assert_eq!(filter_pages(&pages, "CONTEXTO").len(), 1);
        // Conteúdo, sem acento ("sessao" acha "Sessão" na página e no texto).
        let hits = filter_pages(&pages, "sessao");
        assert_eq!(hits.len(), 2, "título e conteúdo casam: {hits:?}");
        // Sem casamento = lista vazia, nunca pânico.
        assert!(filter_pages(&pages, "nada-disso-aqui").is_empty());
    }

    #[test]
    fn empty_wiki_line_names_reindex_and_store_causes_fall_through() {
        // Store legível, índice vazio: a causa é o comando que conserta.
        let empty = Snapshot {
            wiki_note: Some("wiki vazia — rode kira wiki reindex".to_string()),
            ..Snapshot::default()
        };
        assert_eq!(
            empty_wiki_line(&empty),
            "wiki: — (wiki vazia — rode kira wiki reindex)"
        );
        // Store ausente: a causa concreta da store (nunca inventada aqui).
        let absent = Snapshot {
            store_note: Some(
                "nenhuma trilha em store.db — rode `kira scan` ou `kira doctor`".to_string(),
            ),
            ..Snapshot::default()
        };
        assert_eq!(
            empty_wiki_line(&absent),
            "wiki: — (nenhuma trilha em store.db — rode `kira scan` ou `kira doctor`)"
        );
        // E `wiki_lines` devolve exatamente essa linha no estado vazio.
        assert_eq!(
            wiki_lines(&absent),
            vec!["wiki: — (nenhuma trilha em store.db — rode `kira scan` ou `kira doctor`)"]
        );
    }

    #[test]
    fn wiki_lines_show_kind_title_and_relative_age() {
        // now = 2026-11-11T09:00:00Z (1_794_387_600) + 3 dias ⇒ "há 3 dias"
        // determinístico (o stamp é PARSEADO, o relógio é fixo do teste).
        let now = 1_794_387_600 + 3 * 86_400;
        let snapshot = Snapshot {
            wiki_rows: vec![
                fixture_page(
                    "contexto",
                    "contexto",
                    "Contexto do projeto",
                    "2026-11-11T09:00:00Z",
                    "# Contexto\n\n**decisão** tomada",
                ),
                fixture_page(
                    "bugs/4821",
                    "bug",
                    "Sessão 4821",
                    "2026-11-10T09:00:00Z",
                    "token expirado",
                ),
            ],
            now_secs: Some(now),
            ..Snapshot::default()
        };
        let lines = wiki_lines(&snapshot).join("\n");
        assert!(lines.contains("painel wiki — 2 páginas"), "{lines}");
        assert!(lines.contains("TIPO"), "{lines}");
        assert!(lines.contains("TÍTULO"), "{lines}");
        assert!(lines.contains("ATUALIZAÇÃO"), "{lines}");
        assert!(lines.contains("Contexto do projeto"), "{lines}");
        assert!(lines.contains("há 3 dias"), "{lines}");
        // Stamp não-parseável: `—` honesto, nunca idade inventada.
        let broken = fixture_page("x", "bug", "X", "stamp quebrado", "");
        assert_eq!(wiki_age_line(&broken, Some(now)), "—");
    }

    #[test]
    fn wiki_preview_renders_markdown_without_hash_or_bold_markers() {
        let page = fixture_page(
            "contexto",
            "contexto",
            "Contexto do projeto",
            "2026-11-11T09:00:00Z",
            "# Contexto\n\n- item um\n\n**negrito** e `código`",
        );
        let lines = wiki_preview_lines(&page, 80).join("\n");
        assert!(
            lines.contains("página: Contexto do projeto (contexto)"),
            "{lines}"
        );
        assert!(lines.contains("▌ Contexto"), "{lines}");
        assert!(lines.contains("• item um"), "{lines}");
        assert!(lines.contains("negrito"), "{lines}");
        assert!(lines.contains("código"), "{lines}");
        assert!(!lines.contains('#'), "o `#` cru nunca sai: {lines}");
        assert!(!lines.contains("**"), "{lines}");
        assert!(!lines.contains('`'), "{lines}");
        // Página ilegível no disco: causa, nunca texto inventado.
        let ghost = fixture_page("ghost", "bug", "Fantasma", "2026-11-11T09:00:00Z", "");
        let lines = wiki_preview_lines(&ghost, 80).join("\n");
        assert!(lines.contains("conteúdo ausente"), "{lines}");
    }

    #[test]
    fn text_report_carries_the_wiki_list_and_rendered_pages() {
        let snapshot = Snapshot {
            wiki_rows: vec![fixture_page(
                "contexto",
                "contexto",
                "Contexto do projeto",
                "2026-11-11T09:00:00Z",
                "## Decisão\n\n- item",
            )],
            now_secs: Some(1_762_861_200),
            ..Snapshot::default()
        };
        let report = text_report(&snapshot);
        assert!(report.contains("painel wiki — 1 página"), "{report}");
        assert!(report.contains("página: Contexto do projeto"), "{report}");
        assert!(report.contains("■ Decisão"), "{report}");
        assert!(report.contains("• item"), "{report}");
        assert!(!report.contains("##"), "{report}");
    }
}
