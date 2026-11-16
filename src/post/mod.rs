//! `kira digest` — um turno (`turn`) com fases `claim → generate → deliver` (ADR-0038, M3).
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

/// Opções de uma rodada do turno.
#[derive(Debug, Clone)]
pub struct DigestOptions {
    pub root: PathBuf,
    /// Etiqueta do modelo. Valor explícito (`--model`/`KIRA_MODEL`) vem como
    /// `ollama:<tag>` e vence TUDO; o sentinela `ollama` liga o ROUTING de
    /// especialistas (HF-5, ADR-0040): cada kind resolve a própria tag pela
    /// tabela `model_role` com o gate de admissão.
    pub model: String,
    /// Idioma do **artefato** (o post). O prompt é sempre inglês.
    pub lang: String,
    /// Gera SÓ este kind (`kira digest --kind <kind>`); `None` = todos.
    pub kind_filter: Option<String>,
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
            kind_filter: None,
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

/// Executa uma rodada do turno.
///
/// `build_backend` é a FÁBRICA de backend por `(kind, tag)` — o routing de
/// especialistas (HF-5) decide a tag por kind (com o gate de admissão no
/// store) e a fábrica constrói quem executa. A DECISÃO com o modelo continua
/// aqui dentro, atrás de trust e orçamento (`// LLM porque:` no claim).
pub async fn run<B: LlmBackend>(
    options: &DigestOptions,
    build_backend: &dyn Fn(&str, &str) -> B,
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

    // HF-5 (routing): config lido uma vez (fail-open com nota — config
    // ilegível degrada para os defaults, nunca quebra o turno).
    let config_pairs = match crate::credentials::read_config() {
        Ok(pairs) => pairs,
        Err(err) => {
            report.notes.push(format!(
                "config ilegível ({err}) — routing segue nos defaults"
            ));
            Vec::new()
        }
    };
    // Tag explícita (`--model`/`KIRA_MODEL`, no formato `ollama:<tag>`) vence
    // o routing; o sentinela `ollama` liga a resolução por kind.
    let explicit_tag = if options.model == "ollama" {
        None
    } else {
        Some(
            options
                .model
                .strip_prefix("ollama:")
                .map_or_else(|| options.model.clone(), std::borrow::ToOwned::to_owned),
        )
    };
    // A consulta de admissão no store: Some(true/false) quando a trilha
    // respondeu, None quando falhou (o resolver falha aberto com nota).
    let admitted = |candidate: &str| match store.model_admission_get(candidate) {
        Ok(Some(_)) => Some(true),
        Ok(None) => Some(false),
        Err(_) => None,
    };
    // Tag resolvida UMA VEZ por kind — o `cache_key` da linha herda a tag
    // (`model` entra no cache: trocar de especialista regenera, nunca serve
    // cache errado).
    let mut model_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    // 1. Plano puro → fila idempotente.
    let mut planned = plan::plan(&digest);
    if let Some(kind) = &options.kind_filter {
        planned.retain(|post| &post.kind == kind);
    }
    report.planned = planned.len();
    // OC-1: o aviso de egresso por tag aparece UMA vez por run (nota, não erro).
    let mut egress_warned = false;
    for post in &planned {
        let tag = model_map
            .entry(post.kind.clone())
            .or_insert_with(|| {
                let routing = crate::llm::roles::resolve_tag(
                    &post.kind,
                    &config_pairs,
                    explicit_tag.as_deref(),
                    &admitted,
                );
                report.notes.extend(routing.notes);
                routing.tag
            })
            .clone();
        if !egress_warned && let Some(note) = cloud_tag_note(&tag) {
            egress_warned = true;
            report.notes.push(note);
        }
        let outcome = store
            .post_enqueue(
                &project_id,
                &new_post(post, options, &format!("ollama:{tag}")),
            )
            .map_err(|err| DigestError::Store(err.to_string()))?;
        if outcome == EnqueueOutcome::Unchanged {
            report.cached += 1;
        }
    }

    // 2. Evidências: o digest inteiro é o perfil; os arquivos-chave já estão nele.
    let evidence = read_key_files(root, &digest);

    // 3. Fases do turno: retira (claim), gera (generate), entrega (deliver).
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
        // P0 (auditoria F2): o teto diário de CHAMADAS é checado antes do
        // claim — nunca retirar uma carta que não dá para gerar (o lease
        // venceria e a devolveria de qualquer jeito). O budget de tokens
        // acima limita tamanho; este limita quantas vezes o turno fala com
        // o modelo (mandato: 50/dia, env/config).
        match store.ensure_llm_budget(crate::credentials::max_llm_calls_per_day(), "digest") {
            Ok(()) => {}
            Err(crate::store::StoreError::BudgetExhausted {
                today,
                max,
                command,
            }) => {
                report.budget_hit = true;
                report.notes.push(format!(
                    "limite diário de chamadas de LLM atingido ({today}/{max} hoje em `{command}`) — retome amanhã ou aumente max_llm_calls_per_day"
                ));
                break;
            }
            Err(err) => return Err(DigestError::Store(err.to_string())),
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

        // HF-5: tag do kind (resolvida no planejamento; kind fora do plano
        // resolve na hora, mesmo mecanismo). O `cache_key` da linha gravada
        // já carrega esta tag — especialista trocado regenera.
        let tag = model_map
            .entry(claimed.kind.clone())
            .or_insert_with(|| {
                crate::llm::roles::resolve_tag(
                    &claimed.kind,
                    &config_pairs,
                    explicit_tag.as_deref(),
                    &admitted,
                )
                .tag
            })
            .clone();
        // OC-1: tag `*:cloud` roteada na hora (kind fora do plano) também
        // avisa — o egresso é do run inteiro, não do planejamento.
        if !egress_warned && let Some(note) = cloud_tag_note(&tag) {
            egress_warned = true;
            report.notes.push(note);
        }
        // LLM porque: traduzir fatos determinísticos (digest) em prosa e diagrama C4
        // é julgamento — nenhum parser ou comando de linha produz esse texto, e o
        // ADR-0038 confina a chamada a este módulo, atrás de trust e de orçamento.
        // A tag vem do routing de especialistas (HF-5); a fábrica constrói o
        // backend com ela — o modelo continua sendo decisão deste módulo.
        let backend = build_backend(&claimed.kind, &tag);
        let answer = backend.complete(&system, &prompt).await;

        // P0 (auditoria F2): a chamada entra no ledger do mandato, venha boa,
        // vazia ou falha — o custo aconteceu. O budget é o MESMO para qualquer
        // especialista (HF-5: chamada de modelo conta em `llm_calls` igual).
        // Best-effort: falha de store vira nota, nunca quebra o turno.
        if let Err(err) = store.record_llm_call(
            "ollama",
            &format!("ollama:{tag}"),
            "digest",
            estimate_tokens(&prompt, answer.as_deref().unwrap_or("")),
        ) {
            report
                .notes
                .push(format!("chamada LLM não registrada no ledger ({err})"));
        }

        match answer {
            Ok(text) if !text.trim().is_empty() => {
                report.tokens_used = report
                    .tokens_used
                    .saturating_add(estimate_tokens(&prompt, &text));
                // HF-3: guardrail determinístico de saída (puro Rust, zero
                // dependência). O rascunho é montado AQUI (front-matter +
                // corpo já redigido) e CONFERIDO antes de tocar o disco:
                // front-matter ausente/malformado ou teto estourado vira
                // nota acionável + estado failed — nunca truncamento, nunca
                // gravação pela metade.
                let draft_text = build_draft(&claimed.slug, &options.lang, &text);
                if let Err(violation) = guard_draft(&draft_text) {
                    let state = store.post_fail(&claimed.id).unwrap_or(PostState::Sealed);
                    report.failed += 1;
                    report.notes.push(format!(
                        "{}: guardrail de saída recusou ({violation}); estado {state:?} — nada gravado",
                        claimed.slug
                    ));
                    continue;
                }
                let draft = root
                    .join(".kira")
                    .join("drafts")
                    .join(format!("{}.md", claimed.slug));
                if let Err(source) = write_text(&draft, &draft_text) {
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
    let briefing = render_briefing(
        options,
        &report,
        &briefing_model_lines(options, &model_map),
        sealed,
        delivered,
        failed,
    );
    let briefing_path = root.join(".kira").join("briefing.md");
    write_text(&briefing_path, &briefing).map_err(|source| DigestError::Write {
        path: briefing_path.display().to_string(),
        source,
    })?;
    report.briefing_path = relative(root, &briefing_path);
    Ok(report)
}

fn new_post(planned: &PlannedPost, options: &DigestOptions, model_display: &str) -> NewPost {
    NewPost {
        id: format!("{}-{}", planned.slug, &planned.source_hash[..12]),
        slug: planned.slug.clone(),
        kind: planned.kind.clone(),
        lang: options.lang.clone(),
        source_hash: planned.source_hash.clone(),
        prompt_version: prompts::PROMPT_VERSION.to_string(),
        model: model_display.to_string(),
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

/// Nota de egresso para tag `*:cloud` (OC-1, PURA): o routing pode mandar o
/// prompt para o cloud MESMO com a URL local (matriz de escalonamento do
/// HF-5). `None` para tag local — localhost nunca é egresso.
fn cloud_tag_note(tag: &str) -> Option<String> {
    if crate::endpoint::is_cloud_tag(tag) {
        Some(format!(
            "aviso: egresso de dados neste run (tag `{tag}` é cloud — o prompt sai da máquina; o cloud processa, não treina com ele, porém ele sai)"
        ))
    } else {
        None
    }
}

/// Teto de chars do rascunho pronto (front-matter + corpo): o MESMO teto do
/// prompt por post (`PER_POST_TOKENS` × `CHARS_PER_TOKEN`). Saída acima do
/// teto é RECUSADA, nunca truncada — cortar texto do modelo no meio é
/// publicar meia frase com cara de inteira.
pub const DRAFT_CHAR_CAP: usize = (PER_POST_TOKENS * CHARS_PER_TOKEN) as usize;

/// Monta o texto final do rascunho: front-matter YAML (slug/lang/generator)
/// seguido do corpo JÁ redigido (`redact` — segredo nunca chega ao disco, nem
/// em dado derivado). O `publish` descasca este front-matter antes de embutir
/// o corpo no post publicado (o front-matter de proveniência é dele).
pub fn build_draft(slug: &str, lang: &str, body: &str) -> String {
    format!(
        "---\nslug: {slug}\nlang: {lang}\ngenerator: kira digest (ADR-0038)\n---\n\n{}",
        crate::redact::redact(body)
    )
}

/// Violação do guardrail de saída (HF-3). Toda variante é acionável: diz o
/// que faltou e o que fazer — nunca só "inválido".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DraftViolation {
    /// O texto não começa com o bloco `---`.
    MissingFrontMatter,
    /// O bloco `---` não fecha ou não tem `slug:`/`lang:`.
    MalformedFrontMatter,
    /// Acima do teto — nada é truncado.
    TooLong { chars: usize, cap: usize },
}

impl std::fmt::Display for DraftViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingFrontMatter => write!(
                f,
                "front-matter ausente — o rascunho precisa começar com o bloco --- (slug/lang)"
            ),
            Self::MalformedFrontMatter => write!(
                f,
                "front-matter malformado — o bloco --- precisa fechar e conter slug e lang"
            ),
            Self::TooLong { chars, cap } => write!(
                f,
                "rascunho com {chars} chars acima do teto de {cap} — o modelo passou do orçamento do post; nada foi truncado"
            ),
        }
    }
}

/// Guardrail determinístico de saída (HF-3, função pura): front-matter
/// obrigatório e bem-formado + teto de tamanho. Zero LLM, zero rede — é
/// conferência de forma sobre texto, cálculo em Rust (ADR-0016).
pub fn guard_draft(text: &str) -> Result<(), DraftViolation> {
    let Some(rest) = text.strip_prefix("---\n") else {
        return Err(DraftViolation::MissingFrontMatter);
    };
    let Some(close) = rest.find("\n---") else {
        return Err(DraftViolation::MalformedFrontMatter);
    };
    let block = &rest[..close];
    let has = |key: &str| block.lines().any(|line| line.starts_with(key));
    if !has("slug: ") || !has("lang: ") {
        return Err(DraftViolation::MalformedFrontMatter);
    }
    let chars = text.chars().count();
    if chars > DRAFT_CHAR_CAP {
        return Err(DraftViolation::TooLong {
            chars,
            cap: DRAFT_CHAR_CAP,
        });
    }
    Ok(())
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

/// Linhas de modelo do briefing (HF-5): com escolha explícita, a linha única
/// de sempre; com routing, uma linha por kind (ordem determinística — o
/// briefing é byte-reprodutível). `models` é o mapa kind → tag cru.
fn briefing_model_lines(
    options: &DigestOptions,
    models: &std::collections::HashMap<String, String>,
) -> Vec<String> {
    if options.model != "ollama" {
        return vec![format!("- modelo: {}", options.model)];
    }
    let mut kinds: Vec<(&String, &String)> = models.iter().collect();
    kinds.sort();
    kinds
        .iter()
        .map(|(kind, tag)| format!("- modelo ({kind}): ollama:{tag}"))
        .collect()
}

fn render_briefing(
    options: &DigestOptions,
    report: &DigestReport,
    models: &[String],
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
    // HF-5: com routing, cada kind mostra a própria tag (a decisão fica
    // visível); com escolha explícita, a linha única de sempre.
    for line in models {
        out.push_str(line);
        out.push('\n');
    }
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
    fn cloud_tag_note_warns_only_for_cloud_tags() {
        // Tag cloud (local OU não-local): o aviso vale dos dois jeitos — o
        // prompt sai da máquina mesmo que a URL aponte para localhost.
        let note = cloud_tag_note("fake:cloud").expect("tag cloud tem aviso");
        assert!(
            note.contains("egresso de dados neste run") && note.contains("sai da máquina"),
            "nota acionável: {note}"
        );
        let note = cloud_tag_note("qwen3-coder:480b-cloud").expect("-cloud também");
        assert!(
            note.contains("qwen3-coder:480b-cloud"),
            "nota cita a tag: {note}"
        );
        // Tag local: sem aviso (comportamento de hoje).
        assert!(cloud_tag_note("qwen3.5:4b").is_none());
        assert!(cloud_tag_note("hf.co/bartowski/X:Q4_K_M").is_none());
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

    // -------------------------------------------------------------------
    // HF-3 — guardrail determinístico de saída (build_draft + guard_draft)
    // -------------------------------------------------------------------

    #[test]
    fn build_draft_emits_front_matter_and_passes_the_guard() {
        let text = build_draft("intro", "pt-BR", "KIRA é um CLI.\n");
        assert!(
            text.starts_with("---\n"),
            "front-matter obrigatório: {text}"
        );
        assert!(text.contains("slug: intro"), "{text}");
        assert!(text.contains("lang: pt-BR"), "{text}");
        assert!(text.contains("KIRA é um CLI."), "corpo preservado: {text}");
        guard_draft(&text).unwrap_or_else(|err| panic!("rascunho válido: {err}"));
    }

    #[test]
    fn build_draft_masks_a_fake_token_before_it_reaches_the_disk() {
        // Segredo nunca chega ao disco, nem em dado derivado (regra dura):
        // o corpo passa por `redact` DENTRO do build_draft.
        let text = build_draft(
            "intro",
            "pt-BR",
            "configuração: token=AbC123XyZ789QqQ e chave ghp_0123456789abcdefghijABCDEFGHIJ012345\n",
        );
        assert!(
            !text.contains("AbC123XyZ789QqQ"),
            "token de atribuição vazou: {text}"
        );
        assert!(
            !text.contains("ghp_0123456789abcdefghijABCDEFGHIJ012345"),
            "token em formato conhecido vazou: {text}"
        );
        guard_draft(&text).unwrap_or_else(|err| panic!("rascunho válido: {err}"));
    }

    #[test]
    fn guard_refuses_text_without_front_matter() {
        // O caso do aceite ("post sem front-matter vira failed"): a recusa é
        // da função pura — o binário só grava o que passa por ela.
        assert_eq!(
            guard_draft("só o corpo, sem cabeçalho\n"),
            Err(DraftViolation::MissingFrontMatter)
        );
        assert_eq!(
            guard_draft(""),
            Err(DraftViolation::MissingFrontMatter),
            "vazio também não tem front-matter"
        );
    }

    #[test]
    fn guard_refuses_malformed_front_matter() {
        assert_eq!(
            guard_draft("---\nslug: intro\nsem fechamento\n"),
            Err(DraftViolation::MalformedFrontMatter),
            "bloco que não fecha"
        );
        assert_eq!(
            guard_draft("---\nfoo: bar\n---\n\ncorpo\n"),
            Err(DraftViolation::MalformedFrontMatter),
            "bloco sem slug/lang"
        );
    }

    #[test]
    fn guard_refuses_a_draft_over_the_cap_without_truncating() {
        let huge = "a".repeat(DRAFT_CHAR_CAP + 1);
        let text = build_draft("intro", "pt-BR", &huge);
        let violation = guard_draft(&text).expect_err("acima do teto precisa recusar");
        assert!(
            matches!(violation, DraftViolation::TooLong { .. }),
            "violação de teto: {violation:?}"
        );
        assert!(
            violation.to_string().contains("acima do teto"),
            "mensagem acionável: {violation}"
        );
        // Fronteira exata (sem número mágico): o cabeçalho é medido e o corpo
        // preenche até o teto — no limite passa (inclusivo), um char a mais
        // estoura.
        let head = build_draft("intro", "pt-BR", "");
        let filler = DRAFT_CHAR_CAP - head.chars().count();
        let exact = build_draft("intro", "pt-BR", &"a".repeat(filler));
        assert_eq!(exact.chars().count(), DRAFT_CHAR_CAP);
        guard_draft(&exact).unwrap_or_else(|err| panic!("no teto exato: {err}"));
        let over = build_draft("intro", "pt-BR", &"a".repeat(filler + 1));
        assert!(
            matches!(guard_draft(&over), Err(DraftViolation::TooLong { .. })),
            "um char acima do teto precisa recusar"
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
        let text = render_briefing(&options, &report, &[], 0, 2, 0);
        assert!(
            text.contains("kira publish"),
            "fila vazia deve apontar o publish:\n{text}"
        );
        let pending = render_briefing(&options, &report, &[], 1, 1, 0);
        assert!(
            pending.contains("kira digest --resume"),
            "fila com pendente deve pedir resume: {pending}"
        );
    }

    // -----------------------------------------------------------------------
    // HF-5 — routing de especialistas: a decisão de tag fica visível
    // -----------------------------------------------------------------------

    #[test]
    fn briefing_shows_one_model_line_per_kind_when_routing() {
        let mut options = DigestOptions::at("/tmp");
        options.model = "ollama".to_string(); // sentinela: routing por kind
        let mut models = std::collections::HashMap::new();
        models.insert("intro".to_string(), "qwen3.5:4b".to_string());
        models.insert(
            "c4-container".to_string(),
            "hf.co/bartowski/X-GGUF:Q4_K_M".to_string(),
        );
        let lines = briefing_model_lines(&options, &models);
        assert_eq!(
            lines,
            vec![
                "- modelo (c4-container): ollama:hf.co/bartowski/X-GGUF:Q4_K_M".to_string(),
                "- modelo (intro): ollama:qwen3.5:4b".to_string(),
            ],
            "uma linha por kind, em ordem determinística: {lines:?}"
        );
        // Escolha explícita volta à linha única (o dono pediu, o dono vê a tag).
        options.model = "ollama:fake".to_string();
        assert_eq!(
            briefing_model_lines(&options, &models),
            vec!["- modelo: ollama:fake".to_string()]
        );
    }
}
