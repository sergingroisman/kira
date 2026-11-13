//! `kira digest` — o `Courier` que gera os posts (ADR-0038, M3).
//!
//! Lê os fatos do `ingest`, enfileira o que falta em `posts`, retira com lease e
//! chama o modelo local para cada carta. Tudo o que é estado fica no SQLite: parar
//! no meio (`--pause` ou Ctrl-C) não perde trabalho, e retomar (`--resume`) não
//! refaz o que já foi entregue — o `cache_key` da linha resolve os dois casos.
//!
//! Regras que este módulo respeita:
//! - **trust antes de tudo** (ADR-0038, D2): sem `.kira/trust.json`, nada sai da máquina;
//! - **Ollama primeiro** (D1): o backend vem do chamador, e o caro só entra explícito;
//! - **orçamento com parada dura e nota** (D4): teto por post e por run;
//! - **idioma é dado** (D6): `lang` entra no `cache_key`, então trocar de idioma
//!   regenera em vez de servir cache do outro idioma;
//! - **briefing sempre escrita**: é o que o dev lê ao voltar e o que o próximo run lê.

use std::path::{Path, PathBuf};

use crate::llm::LlmBackend;
use crate::post::plan::PlannedPost;
use crate::store::Store;
use crate::store::posts::{EnqueueOutcome, NewPost, PostState};
use crate::trust;

pub mod plan;
pub mod prompts;

/// Teto do run inteiro (D4).
pub const DEFAULT_BUDGET_TOKENS: u64 = 1_000_000;

/// Teto por post (D4): sem ele, um artigo guloso come o run.
pub const PER_POST_TOKENS: u64 = 30_000;

/// Chars por token — mesma constante que o resto do KIRA usa para estimar.
const CHARS_PER_TOKEN: u64 = 3;

/// Lease do claim, em segundos. Curto: se o processo morrer, a carta volta sozinha.
const LEASE_SECONDS: i64 = 120;

/// Erros que impedem o `digest` (o resto vira nota).
#[derive(Debug, thiserror::Error)]
pub enum DigestError {
    #[error(
        "este repositório não foi aceito — rode: kira trust\n\
         (o `digest` envia código ao modelo; o `ingest` não precisa de aceite)"
    )]
    NotTrusted,
    #[error("nada ingerido ainda — rode: kira ingest")]
    NotIngested,
    #[error("store indisponível: {0}")]
    Store(String),
    #[error("não consegui escrever {path}: {source}")]
    Write {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// Opções de uma rodada do `Courier`.
#[derive(Debug, Clone)]
pub struct DigestOptions {
    pub root: PathBuf,
    /// Etiqueta do modelo que entra no `cache_key` (`ollama:qwen3.5:4b`).
    pub model: String,
    /// Idioma do **artefato** (o post). O prompt é sempre inglês.
    pub lang: String,
    pub budget_tokens: u64,
    pub per_post_tokens: u64,
    /// Para depois de N posts entregues — o `--pause` do dev.
    pub pause_after: Option<usize>,
    /// Se true, devolve à fila o que está `picked` antes de começar.
    pub resume: bool,
}

impl DigestOptions {
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            model: "ollama".to_string(),
            lang: "en".to_string(),
            budget_tokens: DEFAULT_BUDGET_TOKENS,
            per_post_tokens: PER_POST_TOKENS,
            pause_after: None,
            resume: false,
        }
    }
}

/// Resultado de uma rodada.
#[derive(Debug, Clone, Default)]
pub struct DigestReport {
    pub planned: usize,
    pub generated: usize,
    pub cached: usize,
    pub failed: usize,
    pub tokens_used: u64,
    /// Teto usado nesta rodada (vai para o resumo, para não mentir o número).
    pub budget_tokens: u64,
    pub budget_hit: bool,
    pub briefing_path: String,
    pub notes: Vec<String>,
}

impl DigestReport {
    pub fn summary(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "digest: {} post(s) novo(s), {} no cache, {} falha(s)\n",
            self.generated, self.cached, self.failed
        ));
        out.push_str(&format!(
            "tokens: {} de {} usados{}\n",
            self.tokens_used,
            self.budget_tokens,
            if self.budget_hit {
                " — orçamento esgotado"
            } else {
                ""
            }
        ));
        for note in &self.notes {
            out.push_str(&format!("nota: {note}\n"));
        }
        out.push_str(&format!("briefing: {}\n", self.briefing_path));
        out
    }
}

/// Executa uma rodada do `Courier`.
pub async fn run<B: LlmBackend>(
    options: &DigestOptions,
    backend: &B,
) -> Result<DigestReport, DigestError> {
    let root = &options.root;
    if !trust::is_trusted(root) {
        return Err(DigestError::NotTrusted);
    }
    let facts_path = root.join(".kira").join("facts.md");
    let digest = std::fs::read_to_string(&facts_path).map_err(|_| DigestError::NotIngested)?;

    let store = Store::open_default().map_err(|err| DigestError::Store(err.to_string()))?;
    let project_id = crate::store::project_id_for_root(root)
        .map_err(|err| DigestError::Store(err.to_string()))?;

    let mut report = DigestReport {
        budget_tokens: options.budget_tokens,
        ..DigestReport::default()
    };
    if options.resume {
        let released = store
            .post_release_all()
            .map_err(|err| DigestError::Store(err.to_string()))?;
        if released > 0 {
            report.notes.push(format!(
                "{released} carta(s) retirada(s) devolvida(s) à fila"
            ));
        }
    }

    // 1. Plano puro → fila idempotente.
    let planned = plan::plan(&digest);
    report.planned = planned.len();
    for post in &planned {
        let outcome = store
            .post_enqueue(&project_id, &new_post(post, options))
            .map_err(|err| DigestError::Store(err.to_string()))?;
        if outcome == EnqueueOutcome::Unchanged {
            report.cached += 1;
        }
    }

    // 2. Evidências: o digest inteiro é o perfil; os arquivos-chave já estão nele.
    let evidence = read_key_files(root, &digest);

    // 3. Courier: retira, gera, entrega.
    let now = || {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| i64::try_from(d.as_secs()).unwrap_or(0))
            .unwrap_or(0)
    };
    loop {
        if let Some(limit) = options.pause_after
            && report.generated >= limit
        {
            report
                .notes
                .push(format!("pausado depois de {limit} post(s) (--pause)"));
            break;
        }
        if report.tokens_used >= options.budget_tokens {
            report.budget_hit = true;
            report.notes.push(format!(
                "orçamento de {} token(s) esgotado — rode de novo para continuar",
                options.budget_tokens
            ));
            break;
        }
        let claimed = match store.post_claim(&project_id, now(), LEASE_SECONDS) {
            Ok(Some(row)) => row,
            Ok(None) => break,
            Err(err) => return Err(DigestError::Store(err.to_string())),
        };
        let (system, prompt) = build_prompt(
            &claimed.kind,
            &options.lang,
            &digest,
            &evidence,
            options.per_post_tokens,
        );

        // LLM porque: traduzir fatos determinísticos (digest) em prosa e diagrama C4
        // é julgamento — nenhum parser ou comando de linha produz esse texto, e o
        // ADR-0038 confina a chamada a este módulo, atrás de trust e de orçamento.
        let answer = backend.complete(&system, &prompt).await;

        match answer {
            Ok(text) if !text.trim().is_empty() => {
                report.tokens_used = report
                    .tokens_used
                    .saturating_add(estimate_tokens(&prompt, &text));
                let draft = root
                    .join(".kira")
                    .join("drafts")
                    .join(format!("{}.md", claimed.slug));
                if let Err(source) = write_draft(&draft, &claimed.slug, &options.lang, &text) {
                    let state = store.post_fail(&claimed.id).unwrap_or(PostState::Sealed);
                    report.failed += 1;
                    report.notes.push(format!(
                        "{}: falha ao gravar ({source}); estado {state:?}",
                        claimed.slug
                    ));
                    continue;
                }
                let rel = draft
                    .strip_prefix(root)
                    .map(|rest| rest.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_else(|_| draft.display().to_string());
                if store.post_deliver(&claimed.id, &rel).is_err() {
                    report.notes.push(format!(
                        "{}: entregue mas o estado não foi gravado",
                        claimed.slug
                    ));
                }
                report.generated += 1;
            }
            Ok(_) => {
                let state = store.post_fail(&claimed.id).unwrap_or(PostState::Sealed);
                report.failed += 1;
                report.notes.push(format!(
                    "{}: resposta vazia do modelo; estado {state:?}",
                    claimed.slug
                ));
            }
            Err(err) => {
                let state = store.post_fail(&claimed.id).unwrap_or(PostState::Sealed);
                report.failed += 1;
                report.notes.push(format!(
                    "{}: modelo falhou ({err}); estado {state:?}",
                    claimed.slug
                ));
                // Falha de backend não é falha de uma carta: parar evita 3 tentativas
                // por post com o mesmo erro (o orçamento é do dev, não do loop).
                if matches!(state, PostState::Sealed) {
                    break;
                }
            }
        }
    }

    // 4. Briefing: estado da fila + o que fazer a seguir.
    let posts = store
        .posts_of(&project_id)
        .map_err(|err| DigestError::Store(err.to_string()))?;
    let sealed = posts
        .iter()
        .filter(|p| p.state == PostState::Sealed)
        .count();
    let delivered = posts
        .iter()
        .filter(|p| p.state == PostState::Delivered)
        .count();
    let failed = posts
        .iter()
        .filter(|p| p.state == PostState::Failed)
        .count();
    let briefing = render_briefing(options, &report, sealed, delivered, failed);
    let briefing_path = root.join(".kira").join("briefing.md");
    write_text(&briefing_path, &briefing).map_err(|source| DigestError::Write {
        path: briefing_path.display().to_string(),
        source,
    })?;
    report.briefing_path = relative(root, &briefing_path);
    Ok(report)
}

fn new_post(planned: &PlannedPost, options: &DigestOptions) -> NewPost {
    NewPost {
        id: format!("{}-{}", planned.slug, &planned.source_hash[..12]),
        slug: planned.slug.clone(),
        kind: planned.kind.clone(),
        lang: options.lang.clone(),
        source_hash: planned.source_hash.clone(),
        prompt_version: prompts::PROMPT_VERSION.to_string(),
        model: options.model.clone(),
    }
}

/// Perfil e evidência vêm do próprio digest (que já é redigido e determinístico).
fn read_key_files(root: &Path, digest: &str) -> String {
    let _ = root;
    // O digest já carrega os arquivos-chave; a seção é o melhor material de evidência.
    let start = digest.find("## Arquivos-chave").unwrap_or(0);
    let evidence = &digest[start..];
    evidence.chars().take(60_000).collect()
}

fn build_prompt(
    kind: &str,
    lang: &str,
    digest: &str,
    evidence: &str,
    per_post_tokens: u64,
) -> (String, String) {
    let project = digest
        .lines()
        .next()
        .map(|line| line.trim_start_matches("# Digest — ").to_string())
        .unwrap_or_else(|| "project".to_string());
    let tree = digest
        .split("## Árvore")
        .nth(1)
        .and_then(|rest| rest.split("## Perfil").next())
        .unwrap_or("")
        .chars()
        .take(chars_for(per_post_tokens))
        .collect::<String>();
    let profile = digest
        .split("## Perfil")
        .nth(1)
        .and_then(|rest| rest.split("## Arquivos-chave").next())
        .unwrap_or("")
        .to_string();
    let prompt = match kind {
        "c4-container" => prompts::c4_container(lang, &project, &profile, evidence),
        _ => prompts::intro(lang, &project, &profile, &tree),
    };
    (prompts::SYSTEM.to_string(), prompt)
}

fn chars_for(tokens: u64) -> usize {
    usize::try_from(tokens.saturating_mul(CHARS_PER_TOKEN)).unwrap_or(usize::MAX)
}

/// Estimativa de tokens por chars/3 — a mesma régua do resto do KIRA.
pub fn estimate_tokens(prompt: &str, answer: &str) -> u64 {
    let chars = (prompt.chars().count() + answer.chars().count()) as u64;
    chars / CHARS_PER_TOKEN
}

fn write_draft(path: &Path, slug: &str, lang: &str, body: &str) -> Result<(), std::io::Error> {
    let header =
        format!("<!-- gerado por `kira digest` (ADR-0038). slug: {slug} · lang: {lang} -->\n\n");
    write_text(path, &format!("{header}{}", crate::redact::redact(body)))
}

fn write_text(path: &Path, body: &str) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, body)
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|rest| rest.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.display().to_string())
}

fn render_briefing(
    options: &DigestOptions,
    report: &DigestReport,
    sealed: usize,
    delivered: usize,
    failed: usize,
) -> String {
    let mut out = String::new();
    out.push_str("# Briefing do digest\n\n");
    out.push_str(
        "<!-- estado da fila em `.kira/briefing.md`. É o que um run novo lê primeiro. -->\n\n",
    );
    out.push_str(&format!("- idioma dos posts: {}\n", options.lang));
    out.push_str(&format!("- modelo: {}\n", options.model));
    out.push_str(&format!("- plano: {} post(s)\n", report.planned));
    out.push_str(&format!("- entregues nesta rodada: {}\n", report.generated));
    out.push_str(&format!("- no cache (sem trabalho): {}\n", report.cached));
    out.push_str(&format!("- falhas: {}\n", report.failed));
    out.push_str(&format!(
        "- tokens: {} de {}\n",
        report.tokens_used, options.budget_tokens
    ));
    out.push_str(&format!(
        "- fila: {sealed} pendente(s) · {delivered} entregue(s) · {failed} falha(s)\n"
    ));
    if !report.notes.is_empty() {
        out.push_str("\n## Notas\n\n");
        for note in &report.notes {
            out.push_str(&format!("- {note}\n"));
        }
    }
    out.push_str("\n## Próximo passo\n\n");
    if sealed > 0 {
        out.push_str("```\nkira digest --resume\n```\n");
    } else {
        out.push_str("```\nkira publish\n```\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_tokens_uses_three_chars_per_token() {
        assert_eq!(estimate_tokens("123", "456"), 2);
        assert_eq!(estimate_tokens("", ""), 0);
    }

    #[test]
    fn prompt_budget_caps_the_tree_that_goes_to_the_model() {
        let digest = "# Digest — demo\n\n## Árvore\n\n```text\nsrc/main.rs\n```\n\n## Perfil\n\n- arquivos: 1\n";
        let (_, prompt) = build_prompt("intro", "pt-BR", digest, "", 10);
        assert!(prompt.contains("src/main.rs"), "árvore precisa entrar");
        assert!(
            prompt.chars().count() < chars_for(10) + 2_000,
            "teto do post não respeitado"
        );
    }

    #[test]
    fn briefing_points_to_the_next_command() {
        let options = DigestOptions::at("/tmp");
        let report = DigestReport {
            planned: 2,
            generated: 2,
            ..DigestReport::default()
        };
        let text = render_briefing(&options, &report, 0, 2, 0);
        assert!(
            text.contains("kira publish"),
            "fila vazia deve apontar o publish:\n{text}"
        );
        let pending = render_briefing(&options, &report, 1, 1, 0);
        assert!(
            pending.contains("kira digest --resume"),
            "fila com pendente deve pedir resume"
        );
    }
}
