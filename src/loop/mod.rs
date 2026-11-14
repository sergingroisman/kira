//! `kira loop` (PR15) — o turno do binário: ciclos locais com estado no SQLite.
//!
//! Um tick = `ingest` → `digest` (só se o repositório está aceito) → `publish`.
//! Tudo LOCAL: o loop nunca escreve no mundo (Azure DevOps, git remoto) —
//! escrita no mundo só depois de `awaiting_approval` + `kira approve` humano
//! (PR18). Sem daemon: é um modo do binário que roda ticks em primeiro plano
//! e sai; `--once` roda UM tick (CI/teste).
//!
//! Anti-PR-inócuo (D2): 2 ticks sem mudança ⇒ estado idêntico. O segundo tick
//! não reescreve facts (ingest determinístico), não gera post novo (cache do
//! digest por `cache_key`) e não republica (`published_hash` do publish). A
//! detecção de estabilidade é [`TickReport::changed`] — pura, testável.
//!
//! Este módulo é WIRING (como o `main.rs`): constrói o backend do digest e o
//! entrega ao `post::run`, que é quem decide com o modelo (e carrega o
//! `// LLM porque:` no call site dele — ADR-0016/0038). O loop em si não faz
//! nenhuma pergunta de julgamento ao modelo.

use std::path::{Path, PathBuf};

use crate::ingest::{self, IngestOptions};
use crate::llm::ollama;
use crate::post::{self, DigestError, DigestOptions};
use crate::publish::{self, PublishOptions};
use crate::trust;

/// Erros do loop — todos acionáveis (a casa nunca ecoa body cru).
#[derive(Debug, thiserror::Error)]
pub enum LoopError {
    #[error("o ingest do ciclo falhou: {0}")]
    Ingest(String),
    #[error("o digest do ciclo falhou: {0}")]
    Digest(String),
    #[error("o publish do ciclo falhou: {0}")]
    Publish(String),
}

/// Opções de um ciclo.
#[derive(Debug, Clone)]
pub struct LoopOptions {
    pub root: PathBuf,
    /// Tag do modelo do digest (a etiqueta do `cache_key` é `ollama:<tag>`).
    pub model: String,
    /// Idioma dos posts (entra no `cache_key` do digest).
    pub lang: String,
    /// Pula o digest (ciclo 100% determinístico: ingest + publish).
    pub skip_digest: bool,
    /// O ingest roda o `scan` dentro do ciclo (default: sim — findings novos
    /// mudam o brief; em CI/fixture sem stacks o scan não spawna nada).
    pub run_scan: bool,
}

impl LoopOptions {
    #[must_use]
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            model: ollama::model_from_env(),
            lang: "en".to_string(),
            skip_digest: false,
            run_scan: true,
        }
    }
}

/// O que UM tick fez — a base da detecção de estabilidade.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TickReport {
    /// O `.kira/facts.md` mudou em disco neste tick.
    pub facts_changed: bool,
    pub digest_generated: usize,
    pub digest_cached: usize,
    pub digest_failed: usize,
    pub published: usize,
    pub publish_unchanged: usize,
    pub notes: Vec<String>,
}

impl TickReport {
    /// O tick mudou o estado persistido? Facts novos, post gerado ou
    /// publicação nova contam; cache/unchanged/failed NÃO (falha já é estado
    /// gravado pelo digest e não se repete no tick seguinte).
    #[must_use]
    pub fn changed(&self) -> bool {
        self.facts_changed || self.digest_generated > 0 || self.published > 0
    }

    /// Linha única do tick para stdout/trilha.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "ingest: {} · digest: {} novo(s), {} cache, {} falha(s) · publish: {} publicado(s), {} igual(is)",
            if self.facts_changed {
                "facts mudaram"
            } else {
                "facts iguais"
            },
            self.digest_generated,
            self.digest_cached,
            self.digest_failed,
            self.published,
            self.publish_unchanged,
        )
    }
}

/// Roda UM tick: ingest → digest (se trusted) → publish.
///
/// Cada fase é o comando real (mesmo código do CLI — o loop não reimplementa
/// nada); o que o loop acrescenta é a ORDEM, a detecção de mudança e a
/// política de pular o digest sem trust.
pub async fn tick(options: &LoopOptions) -> Result<TickReport, LoopError> {
    let root = &options.root;
    let mut report = TickReport::default();

    // 1. INGEST — sempre. Determinístico, sem LLM, sem rede (o scan dentro
    //    dele só spawna ferramenta se houver stack). A mudança é medida pelo
    //    conteúdo do facts.md antes/depois — nunca por timestamp.
    let facts_path = root.join(".kira").join("facts.md");
    let facts_before = std::fs::read(&facts_path).ok();
    let mut ingest_options = IngestOptions::at(root.clone());
    ingest_options.run_scan = options.run_scan;
    let ingest = ingest::run(&ingest_options).map_err(|err| LoopError::Ingest(err.to_string()))?;
    let facts_after = std::fs::read(&facts_path).ok();
    report.facts_changed = facts_before != facts_after;
    report.notes.extend(ingest.notes.iter().cloned());

    // 2. DIGEST — só com trust (ADR-0038 D2: sem aceite, nada sai da máquina).
    //    Sem trust não é erro do ciclo: é nota acionável (o dono decide).
    if options.skip_digest {
        report
            .notes
            .push("digest pulado (--skip-digest)".to_string());
    } else if !trust::is_trusted(root) {
        report
            .notes
            .push("digest pulado: repositório não aceito — rode `kira trust`".to_string());
    } else {
        let mut digest_options = DigestOptions::at(root.clone());
        digest_options.model = format!("ollama:{}", options.model);
        digest_options.lang = options.lang.clone();
        // Wiring: o backend é construído aqui e a DECISÃO com o modelo
        // acontece dentro de `post::run` (que carrega o `// LLM porque:`).
        let backend = ollama::OllamaBackend::new(ollama::base_url_from_env(), &options.model);
        match post::run(&digest_options, &backend).await {
            Ok(digest) => {
                report.digest_generated = digest.generated;
                report.digest_cached = digest.cached;
                report.digest_failed = digest.failed;
                report.notes.extend(digest.notes.iter().cloned());
            }
            // Trust revogado entre a checagem e o run (corrida rara): mesma
            // conduta de "sem trust" — nota, nunca erro do ciclo.
            Err(DigestError::NotTrusted) => report
                .notes
                .push("digest pulado: repositório não aceito — rode `kira trust`".to_string()),
            Err(err) => return Err(LoopError::Digest(err.to_string())),
        }
    }

    // 3. PUBLISH — sempre. Local: publica os rascunhos entregues na wiki e
    //    regenera o índice derivado. Sem rascunho novo, não muda nada.
    let publish_options = PublishOptions::at(root.clone());
    let publish =
        publish::run(&publish_options).map_err(|err| LoopError::Publish(err.to_string()))?;
    report.published = publish.published;
    report.publish_unchanged = publish.unchanged;
    report.notes.extend(publish.notes.iter().cloned());

    Ok(report)
}

/// Driver do ciclo: `once` = um tick e sai (CI); senão roda até um tick não
/// mudar nada (ponto fixo) ou até `max_ticks` (teto de segurança — ciclo que
/// nunca estabiliza é nota, nunca loop infinito).
pub async fn run(options: &LoopOptions, once: bool, max_ticks: usize) -> Result<String, LoopError> {
    let limit = if once { 1 } else { max_ticks.max(1) };
    let mut out = String::new();
    let mut number = 0usize;
    loop {
        number += 1;
        let report = tick(options).await?;
        out.push_str(&format!("tick {number}: {}\n", report.line()));
        for note in &report.notes {
            out.push_str(&format!("  nota: {note}\n"));
        }
        if once {
            break;
        }
        if !report.changed() {
            out.push_str("estável: nada a fazer — o estado não mudou neste tick\n");
            break;
        }
        if number >= limit {
            out.push_str(&format!(
                "limite de {limit} ticks atingido ainda com mudança — rode `kira loop` de novo\n"
            ));
            break;
        }
    }
    Ok(out)
}

/// Caminho do projeto para o ciclo: flag > detecção da raiz (mesma regra dos
/// outros comandos).
#[must_use]
pub fn resolve_root(path: Option<&Path>) -> PathBuf {
    path.map_or_else(
        || crate::store::detect_project_root().unwrap_or_else(|| PathBuf::from(".")),
        Path::to_path_buf,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_report_changed_tracks_real_state_moves() {
        // Tick que não moveu nada: estável (o anti-PR-inócuo do D2).
        let idle = TickReport {
            digest_cached: 2,
            publish_unchanged: 2,
            ..TickReport::default()
        };
        assert!(!idle.changed());

        // Facts novos contam como mudança (mesmo sem post).
        assert!(
            TickReport {
                facts_changed: true,
                ..TickReport::default()
            }
            .changed()
        );

        // Post gerado conta.
        assert!(
            TickReport {
                digest_generated: 1,
                ..TickReport::default()
            }
            .changed()
        );

        // Publicação nova conta.
        assert!(
            TickReport {
                published: 1,
                ..TickReport::default()
            }
            .changed()
        );

        // Falha NÃO conta como mudança: o estado failed já foi gravado pelo
        // digest e não se repete — senão o ciclo nunca estabilizaria.
        assert!(
            !TickReport {
                digest_failed: 3,
                ..TickReport::default()
            }
            .changed()
        );
    }

    #[test]
    fn tick_line_reports_every_phase() {
        let report = TickReport {
            facts_changed: true,
            digest_generated: 2,
            digest_cached: 1,
            digest_failed: 0,
            published: 2,
            publish_unchanged: 0,
            ..TickReport::default()
        };
        let line = report.line();
        assert!(line.contains("facts mudaram"), "{line}");
        assert!(line.contains("2 novo(s)"), "{line}");
        assert!(line.contains("2 publicado(s)"), "{line}");
    }

    #[test]
    fn options_defaults_are_the_safe_ones() {
        let options = LoopOptions::at("/tmp/proj");
        assert!(!options.skip_digest, "digest faz parte do ciclo");
        assert!(options.run_scan, "ingest completo por padrão");
        assert_eq!(options.lang, "en");
    }
}
