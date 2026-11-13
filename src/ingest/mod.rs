//! `kira ingest` — os fatos do projeto, sem LLM (ADR-0038, M1).
//!
//! Orquestra, nesta ordem: inventário (uma caminhada) → índice FTS5 → `scan`
//! opcional → digest determinístico. Tudo é **fail-open por etapa**: ferramenta de
//! scan ausente, FTS5 indisponível ou store inacessível viram **nota no digest**,
//! nunca erro do comando — porque a ingestão não é um gate, é percepção.
//!
//! Este módulo **não** importa `llm::*`: ingestão determinística não chama modelo
//! (ADR-0016/ADR-0038), e o teste de allowlist do repositório reprova se importar.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub mod digest;
pub mod inventory;

use crate::context;
use crate::context::ignore::MAX_FILE_BYTES;
use crate::ingest::digest::{FindingsBrief, render, select_key_files};
use crate::ingest::inventory::{Budget, Inventory};
use crate::scan;
use crate::stack::{StackProfile, detect};

/// Erros que realmente impedem a ingestão (o resto degrada para nota).
#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error("não consegui escrever {path}: {source}")]
    Write {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("raiz inválida: {0}")]
    Root(String),
}

/// O que o `ingest` pode fazer.
#[derive(Debug, Clone)]
pub struct IngestOptions {
    pub root: PathBuf,
    /// Roda `scan` (rede, lento) no fim. Default `true` no comando, `false` no setup.
    pub run_scan: bool,
    pub budget: Budget,
}

impl IngestOptions {
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            run_scan: true,
            budget: Budget::default(),
        }
    }
}

/// Resultado da ingestão — é o que o CLI imprime e o `--json` serializa.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestReport {
    pub project_name: String,
    pub root: String,
    pub files_ingested: usize,
    pub total_bytes: u64,
    pub stacks: Vec<String>,
    pub flavors: Vec<String>,
    /// `None` = `scan` não produziu nem leu `findings.json`.
    pub findings_total: Option<usize>,
    pub files_indexed: usize,
    pub lines_indexed: usize,
    pub index_truncated: bool,
    pub facts_path: String,
    pub inventory_path: String,
    pub notes: Vec<String>,
}

impl IngestReport {
    /// Resumo de uma linha por assunto, no formato dos outros comandos.
    pub fn summary(&self) -> String {
        let stacks = if self.stacks.is_empty() {
            "stack não detectada".to_string()
        } else {
            let mut s = self.stacks.join(", ");
            if !self.flavors.is_empty() {
                s.push_str(&format!(" ({})", self.flavors.join(", ")));
            }
            s
        };
        let findings = match self.findings_total {
            Some(0) => "0 achados".to_string(),
            Some(n) => format!("{n} achado(s)"),
            None => "sem scan".to_string(),
        };
        let mut out = String::new();
        out.push_str(&format!(
            "ingerido: {} arquivo(s), {} KB, {stacks}, {findings}\n",
            self.files_ingested,
            self.total_bytes / 1024
        ));
        out.push_str(&format!(
            "índice: {} arquivo(s) novo(s), {} linha(s){}\n",
            self.files_indexed,
            self.lines_indexed,
            if self.index_truncated {
                " — ATENÇÃO: teto de arquivos atingido"
            } else {
                ""
            }
        ));
        for note in &self.notes {
            out.push_str(&format!("nota: {note}\n"));
        }
        out.push_str(&format!("facts: {}\n", self.facts_path));
        out
    }
}

/// Executa a ingestão completa.
pub fn run(options: &IngestOptions) -> Result<IngestReport, IngestError> {
    let root = &options.root;
    if !root.is_dir() {
        return Err(IngestError::Root(root.display().to_string()));
    }
    let mut notes: Vec<String> = Vec::new();

    // 1. Inventário: uma caminhada, sem I/O de conteúdo além do sniff.
    let inv = inventory::walk(root, options.budget);
    if inv.is_empty() {
        notes.push("nenhum arquivo ingerível encontrado — confira a raiz do projeto".to_string());
    }

    // 2. Stack: marcador na raiz, nunca extensão solta.
    let profile = detect(root);

    // 3. Índice FTS5 (incremental). Falha aqui degrada o retrieval, não o comando.
    let (files_indexed, lines_indexed, index_truncated) = reindex(root, &mut notes);

    // 4. Scan opcional: rede, portanto fora do caminho do `setup`.
    let mut findings_brief: Option<FindingsBrief> = None;
    if options.run_scan {
        match scan::run(Some(root)) {
            Ok(_) => {}
            Err(cause) => notes.push(format!("scan não rodou ({cause})")),
        }
    }
    match scan::read_findings_file(root) {
        Ok(Some(findings)) if !findings.is_empty() => {
            findings_brief = Some(FindingsBrief {
                total: findings.len(),
                by_severity: scan::severity_brief(&findings),
            });
        }
        Ok(_) => {}
        Err(cause) => notes.push(format!("findings.json ilegível ({cause})")),
    }

    // 5. Digest: seleção pura + leitura redigida dos arquivos-chave.
    let paths: Vec<String> = inv.files.iter().map(|f| f.path.clone()).collect();
    let chosen = select_key_files(&paths);
    let mut key_files: Vec<(String, String)> = Vec::new();
    for rel in &chosen {
        match read_for_digest(root, rel) {
            Some(body) => key_files.push((rel.clone(), body)),
            None => notes.push(format!("arquivo-chave ilegível: {rel}")),
        }
    }
    let text = render(
        &inv.project_name,
        &inv,
        &profile,
        &key_files,
        findings_brief.as_ref(),
        &notes,
    );

    let kira_dir = root.join(".kira");
    let ingest_dir = kira_dir.join("ingest");
    write_file(&ingest_dir.join("inventory.json"), &inventory_json(&inv)?)?;
    let facts_path = kira_dir.join("facts.md");
    write_file(&facts_path, &text)?;

    Ok(IngestReport {
        project_name: inv.project_name.clone(),
        root: root.display().to_string().replace('\\', "/"),
        files_ingested: inv.files.len(),
        total_bytes: inv.total_bytes,
        stacks: profile.all.iter().map(|s| s.as_str().to_string()).collect(),
        flavors: profile.flavors.clone(),
        findings_total: findings_brief.as_ref().map(|b| b.total),
        files_indexed,
        lines_indexed,
        index_truncated,
        facts_path: display_rel(root, &facts_path),
        inventory_path: display_rel(root, &ingest_dir.join("inventory.json")),
        notes: {
            let mut all = notes;
            all.extend(inv.notes.iter().cloned());
            all.extend(profile.notes.iter().cloned());
            all
        },
    })
}

/// Índice FTS5 incremental. Sem store ou sem FTS5 ⇒ `(0, 0, false)` + nota.
fn reindex(root: &Path, notes: &mut Vec<String>) -> (usize, usize, bool) {
    let Ok(store) = crate::store::Store::open_default() else {
        notes.push("índice do repositório não atualizado (store indisponível)".to_string());
        return (0, 0, false);
    };
    let Ok(project_id) = crate::store::project_id_for_root(root) else {
        notes.push("índice do repositório não atualizado (projeto não registrado)".to_string());
        return (0, 0, false);
    };
    match context::reindex(root, &store, &project_id) {
        Ok(stats) => {
            if stats.truncated {
                notes.push(format!(
                    "índice truncado no teto de {} arquivo(s) — o retrieval está parcial",
                    stats.max_files
                ));
            }
            (stats.files_indexed, stats.lines_indexed, stats.truncated)
        }
        Err(cause) => {
            notes.push(format!("índice do repositório não atualizado ({cause})"));
            (0, 0, false)
        }
    }
}

/// Lê até o teto e redige linha a linha: arquivo de repositório é onde mora segredo.
fn read_for_digest(root: &Path, rel: &str) -> Option<String> {
    use std::io::Read;
    let path = root.join(rel);
    let file = std::fs::File::open(&path).ok()?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES).read_to_end(&mut bytes).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let mut out = String::new();
    for line in text.lines() {
        out.push_str(&crate::redact::redact(line));
        out.push('\n');
    }
    Some(out)
}

fn inventory_json(inv: &Inventory) -> Result<String, IngestError> {
    serde_json::to_string_pretty(inv).map_err(|source| IngestError::Write {
        path: "inventory.json".to_string(),
        source: std::io::Error::other(source),
    })
}

fn display_rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|rest| rest.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.to_string_lossy().replace('\\', "/"))
}

fn write_file(path: &Path, body: &str) -> Result<(), IngestError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| IngestError::Write {
            path: parent.display().to_string(),
            source,
        })?;
    }
    std::fs::write(path, body).map_err(|source| IngestError::Write {
        path: path.display().to_string(),
        source,
    })
}

/// `.kira/` é local: escreve o marcador que impede commitar o diretório.
pub fn ensure_local_marker(root: &Path) {
    let kira = root.join(".kira");
    let ignore = kira.join(".gitignore");
    if !ignore.exists() {
        let _ = write_file(
            &ignore,
            "# .kira/ é LOCAL: artefatos do KIRA nunca vão para o repositório.\n*\n",
        );
    }
}

/// Perfil de stack da raiz, para quem já tem inventário (usado pelo doctor/setup).
pub fn profile_of(root: &Path) -> StackProfile {
    detect(root)
}
