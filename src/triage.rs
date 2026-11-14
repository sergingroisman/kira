//! `kira triage` — read-only analysis of a work item by a local LLM.
//!
//! The system prompt and the prompt builder are pure functions so they are
//! unit-tested without any model. Triage never writes: the system prompt
//! forbids suggesting code changes and the command never touches Azure DevOps.

use std::path::Path;

use crate::board::BoardProvider;
use crate::error::Error;
use crate::format::DESCRIPTION_MAX_CHARS;
use crate::html::html_to_text;
use crate::llm::LlmBackend;
use crate::llm::LlmKind;
use crate::llm::copilot::CopilotBackend;
use crate::llm::foundry::FoundryBackend;
use crate::llm::ollama::OllamaBackend;
// Single source for the prompt contract: the system text, the section headers
// and the user preamble all live in `llm::prompts` (versioned, golden-tested).
use crate::llm::prompts::{TRIAGE_SECTIONS as SECTION_NAMES, TRIAGE_SYSTEM_PROMPT};
use crate::store::Store;
use crate::wiki::Wiki;
use crate::wiki::episode::{self, Episode};
use crate::work_item::WorkItem;

/// Placeholder used when the model left a section empty.
const SECTION_EMPTY: &str = "—";

/// Limiar do cache de raciocínio (PR35, onda TE): similaridade item × causa
/// conhecida ≥ 0.50 significa "o episódio da wiki já responde este bug".
///
/// A const é PRÓPRIA (e não `eval::CAUSA_LIMIAR`) para não acoplar a semântica
/// do gold de medição à decisão de cache — os dois coincidem hoje (0.50) por
/// escolha documentada e podem divergir. A régua, porém, é a mesma do eval:
/// overlap de tokens de conteúdo `|interseção| / |causa|`, com a página
/// conhecida no papel do gold — mesma conta, mesmo limiar, zero IA no meio.
pub const CACHE_LIMIAR: f64 = 0.50;

/// Builds the user prompt for a work item: title, type, state, area and the
/// description with the raw HTML cleaned by [`html_to_text`].
///
/// Without a [`ContextPack`] this is only the work item — the shape the model
/// answered before PR31.
pub fn build_prompt(item: &WorkItem) -> String {
    build_prompt_with_context(item, &crate::context::ContextPack::default())
}

/// Builds the user prompt and appends the retrieved project context (wiki
/// excerpts with path and age, plus the last `kira scan` brief).
///
/// The pack is already bounded by [`crate::context::budget_chars`]; this function
/// only decides where it sits in the prompt.
pub fn build_prompt_with_context(item: &WorkItem, pack: &crate::context::ContextPack) -> String {
    let mut prompt = String::new();
    // O preâmbulo e o prompt do sistema vivem juntos em `llm::prompts`
    // (versionados); aqui só a ordem das evidências.
    prompt.push_str(crate::llm::prompts::TRIAGE_USER_PREAMBLE);
    prompt.push_str(&format!("ID: {}\n", item.id));
    prompt.push_str(&format!("Tipo: {}\n", item.work_item_type));
    // Privacy boundary: the work item is author-controlled text and can carry a
    // pasted token, e-mail or CPF. Masked before it can reach the model.
    prompt.push_str(&format!("Título: {}\n", crate::redact::redact(&item.title)));
    prompt.push_str(&format!("Estado: {}\n", item.state));
    prompt.push_str(&format!(
        "Área: {}\n",
        item.area_path.as_deref().unwrap_or(SECTION_EMPTY)
    ));
    prompt.push_str(&format!(
        "Responsável: {}\n",
        item.assigned_to.as_deref().unwrap_or(SECTION_EMPTY)
    ));
    prompt.push_str("Descrição:\n");
    prompt.push_str(&description_for_prompt(item.description.as_deref()));
    // Retrieved context goes after the item: the model reads the bug first, then
    // the evidence. Excerpts are already redacted (they come from the wiki, which
    // masks on write) and already bounded by the packer.
    prompt.push_str(&pack.render());
    prompt.push_str(crate::llm::prompts::TRIAGE_UNTRUSTED_END);
    prompt
}

/// Cleans the raw HTML description and truncates it to
/// [`DESCRIPTION_MAX_CHARS`] characters; missing or empty yields
/// "(sem descrição)".
fn description_for_prompt(description: Option<&str>) -> String {
    let Some(description) = description else {
        return "(sem descrição)".to_string();
    };
    let plain = html_to_text(description);
    if plain.is_empty() {
        return "(sem descrição)".to_string();
    }
    let total_chars = plain.chars().count();
    let mut text: String = plain.chars().take(DESCRIPTION_MAX_CHARS).collect();
    if total_chars > DESCRIPTION_MAX_CHARS {
        text.push('…');
    }
    // Same privacy boundary as the title: descriptions routinely carry pasted
    // PATs, connection strings and customer data.
    crate::redact::redact(&text)
}

/// Formats the model answer into the fixed three-section template:
///
/// ```text
/// Causa raiz provável:
/// <model text for this section>
///
/// Arquivos prováveis:
/// <model text for this section>
///
/// Confiança:
/// <model text for this section>
/// ```
///
/// Sections the model did not produce show "—" — but only for an answer that
/// **did** carry at least one of the three headers. An answer with none of them
/// is not a filled template with holes: it is a model that ignored the format,
/// and rendering it as three `—` was the silent failure of W2-fix PR-F2. That
/// case returns [`Error::LlmFormat`] so the command fails loudly instead of
/// recording an empty triage (and an empty wiki episode).
pub fn format_triage(model_text: &str) -> Result<String, Error> {
    if section_headers_found(model_text) == 0 {
        return Err(Error::LlmFormat);
    }
    Ok(render_sections(model_text))
}

/// Number of distinct fixed headers the model answer actually carries.
///
/// Zero means the answer is out of format; one or two mean the template was
/// followed partially (a section that is missing is `—`, a section that is
/// empty is also `—` — both are data, not a format error).
fn section_headers_found(model_text: &str) -> usize {
    let mut found = [false; 3];
    for line in model_text.lines() {
        if let Some((index, _)) = section_header_index(line) {
            found[index] = true;
        }
    }
    found.iter().filter(|seen| **seen).count()
}

/// Renders the three fixed blocks from a model answer that already passed the
/// format check.
fn render_sections(model_text: &str) -> String {
    let sections = split_sections(model_text);
    let blocks: Vec<String> = SECTION_NAMES
        .iter()
        .zip(sections.iter())
        .map(|(name, lines)| {
            let content = lines.trim();
            let content = if content.is_empty() {
                SECTION_EMPTY
            } else {
                content
            };
            format!("{name}:\n{content}")
        })
        .collect();
    let mut out = blocks.join("\n\n");
    out.push('\n');
    out
}

/// Splits the model text into the three section bodies, in the fixed order.
fn split_sections(model_text: &str) -> [String; 3] {
    let mut sections: [String; 3] = [String::new(), String::new(), String::new()];
    let mut current: Option<usize> = None;
    for line in model_text.lines() {
        match section_header_index(line) {
            Some((index, remainder)) => {
                current = Some(index);
                if !remainder.is_empty() {
                    sections[index].push_str(&remainder);
                    sections[index].push('\n');
                }
            }
            None => {
                if let Some(index) = current {
                    sections[index].push_str(line.trim_end());
                    sections[index].push('\n');
                } else {
                    // No header seen yet: everything belongs to the first one.
                    sections[0].push_str(line.trim_end());
                    sections[0].push('\n');
                }
            }
        }
    }
    sections
}

/// Recognizes a section header line (tolerating markdown decorations like
/// `#`, `-`, `**` and a trailing `:`), returning its index plus any content
/// written on the same line.
fn section_header_index(line: &str) -> Option<(usize, String)> {
    let cleaned = line
        .trim()
        .trim_start_matches('#')
        .trim_start_matches('-')
        .trim_start()
        .replace('*', "");
    for (index, name) in SECTION_NAMES.iter().enumerate() {
        if let Some(remainder) = cleaned.strip_prefix(name) {
            let remainder = remainder.trim_start_matches(':').trim();
            return Some((index, remainder.to_string()));
        }
    }
    None
}

/// Hit do cache de raciocínio (PR35): a resposta veio da wiki, sem LLM.
#[derive(Debug, Clone, PartialEq)]
pub struct CacheHit {
    /// Slug da página que serviu a resposta (`bugs/4821`).
    pub page: String,
    /// Similaridade que autorizou o hit (0.0–1.0) — vai ao stderr e ao teste.
    pub similarity: f64,
}

/// Result of one triage run: the work item that was analyzed and the formatted
/// three-section answer.
///
/// The item id travels with the text so the caller can anchor the episodic
/// capture (PR30) without reading the work item twice. The [`ContextPack`]
/// travels too, so `kira triage` can print which evidence was sent (ADR-0015).
/// `cache` is `Some` when the answer came from the reasoning cache (PR35): no
/// LLM call happened, the episode is NOT re-recorded (D1) and the economy
/// ledger records route `cache-wiki` with zero calls.
///
/// [`ContextPack`]: crate::context::ContextPack
#[derive(Debug, Clone, PartialEq)]
pub struct TriageReport {
    /// Id of the analyzed work item.
    pub item_id: i32,
    /// Formatted answer (`Causa raiz provável` / `Arquivos prováveis` /
    /// `Confiança`).
    pub output: String,
    /// Context that was retrieved and sent with the prompt.
    pub context: crate::context::ContextPack,
    /// Cache hit (PR35): `None` = the LLM produced the answer (rota de hoje).
    pub cache: Option<CacheHit>,
}

/// The three sections of an already formatted triage, trimmed — in the fixed
/// order (causa raiz, arquivos, confiança). `—` when a section was empty.
#[must_use]
pub fn sections_of(formatted: &str) -> [String; 3] {
    split_sections(formatted).map(|section| section.trim().to_string())
}

/// Runs triage for a work item read from a local JSON file — the offline path
/// that needs no PAT and no Azure DevOps access.
///
/// Kept with the Ollama default for existing callers; the backend is chosen
/// with [`triage_file_llm`] (ADR-0019/ADR-0020: the costly backend is never
/// implicit). The reasoning cache (PR35) is ON — it is the product default.
pub async fn triage_file(base_url: &str, model: &str, path: &Path) -> Result<TriageReport, Error> {
    triage_file_llm(LlmKind::Ollama, base_url, model, path, true, None).await
}

/// Same as [`triage_file`] with an explicit backend choice.
///
/// `cache` enables the reasoning cache (PR35): a wiki hit (similarity ≥
/// [`CACHE_LIMIAR`]) answers WITHOUT any LLM call. `--no-cache` (command
/// layer) passes `false` and keeps today's route byte-identical.
///
/// `ledger` is the store that counts the call for the daily budget
/// (auditoria P0). In-process callers without a ledger (tests) pass `None`
/// and touch no store — the counter belongs to the command layer.
pub async fn triage_file_llm(
    kind: LlmKind,
    base_url: &str,
    model: &str,
    path: &Path,
    cache: bool,
    ledger: Option<&Store>,
) -> Result<TriageReport, Error> {
    let body = std::fs::read_to_string(path).map_err(|source| Error::Io {
        path: path.display().to_string(),
        source,
    })?;
    let item = WorkItem::from_api_json(&body)?;
    triage_item(kind, base_url, model, &item, Some(path), cache, ledger).await
}

/// Runs triage for a work item fetched live through the board provider
/// (read-only), on the default Ollama backend.
pub async fn triage_live(
    board: &dyn BoardProvider,
    id: i32,
    base_url: &str,
    model: &str,
) -> Result<TriageReport, Error> {
    triage_live_llm(LlmKind::Ollama, board, id, base_url, model, true, None).await
}

/// Same as [`triage_live`] with an explicit backend choice.
pub async fn triage_live_llm(
    kind: LlmKind,
    board: &dyn BoardProvider,
    id: i32,
    base_url: &str,
    model: &str,
    cache: bool,
    ledger: Option<&Store>,
) -> Result<TriageReport, Error> {
    let item = board.work_item(id).await?;
    triage_item(kind, base_url, model, &item, None, cache, ledger).await
}

/// A backend built for one triage call: wiring only — which backend runs is
/// the caller's explicit choice ([`LlmKind`]), never decided here.
enum TriageBackend {
    Ollama(OllamaBackend),
    Copilot(CopilotBackend),
    Foundry(FoundryBackend),
}

async fn triage_item(
    kind: LlmKind,
    base_url: &str,
    model: &str,
    item: &WorkItem,
    exclude_from_grep: Option<&Path>,
    cache: bool,
    ledger: Option<&Store>,
) -> Result<TriageReport, Error> {
    // PR35 (cache de raciocínio): ANTES de qualquer backend — o hit é
    // determinístico (similaridade em Rust, nível 3 da escada, ADR-0016),
    // não fala com modelo e não consome budget. Por isso o lookup vem antes
    // da construção do backend: com hit, até o `--llm copilot`/foundry mal
    // configurado não é tocado — a resposta não precisa de modelo. Falha da
    // wiki é fail-open: nota no pack e rota normal de hoje, byte a byte.
    let mut cache_note: Option<String> = None;
    if cache {
        match cache_lookup(item) {
            Ok(Some((hit, episode))) => return Ok(cache_report(item.id, &hit, &episode)),
            Ok(None) => {}
            Err(cause) => cache_note = Some(cause),
        }
    }
    // LLM porque: hipótese de causa raiz é julgamento sobre texto — nenhuma CLI
    // nem parser responde "por que este bug acontece". Todo o resto do comando
    // (campos, HTML, fingerprint, store, grep do repositório) é determinístico
    // (ADR-0016).
    let backend = match kind {
        LlmKind::Ollama => {
            // OC-1: a validação URL × chave acontece AQUI (igual ao digest) —
            // host não-local sem chave é erro acionável antes do request.
            TriageBackend::Ollama(OllamaBackend::for_base_url(base_url, model)?)
        }
        // The costly paths resolve their own binary/auth/key HERE and FAIL
        // LOUDLY (CopilotMissing/CopilotAuth; FoundryMissingKey/...) — never a
        // silent fallback to Ollama.
        LlmKind::Copilot => TriageBackend::Copilot(CopilotBackend::from_env()?),
        // `from_sources`: env > `config.toml`/keyring gravados por
        // `kira setup` (ADR-0035) — configurar pelo wizard basta.
        LlmKind::Foundry => TriageBackend::Foundry(FoundryBackend::from_sources()?),
    };
    let mut pack = context_pack(item, exclude_from_grep);
    if let Some(cause) = cache_note {
        pack.notes
            .push(format!("cache-wiki indisponível ({cause})"));
    }
    let prompt = build_prompt_with_context(item, &pack);
    let answer = match &backend {
        TriageBackend::Ollama(backend) => backend.complete(TRIAGE_SYSTEM_PROMPT, &prompt).await,
        TriageBackend::Copilot(backend) => backend.complete(TRIAGE_SYSTEM_PROMPT, &prompt).await,
        TriageBackend::Foundry(backend) => backend.complete(TRIAGE_SYSTEM_PROMPT, &prompt).await,
    };
    // P0 (auditoria F2): a chamada entra no ledger do mandato, venha boa ou
    // falha — o custo para o dev aconteceu do mesmo jeito. O ENFORCE do teto
    // vive na camada de comando (checar ANTES de chamar); aqui é só registro.
    record_triage_call(ledger, kind, model, &prompt, &answer);
    let text = answer?;
    Ok(TriageReport {
        item_id: item.id,
        output: format_triage(&text)?,
        context: pack,
        cache: None,
    })
}

/// Registra uma chamada de julgamento no contador diário (tabela `llm_calls`).
///
/// Best-effort por design: sem ledger (chamadas in-process de teste) nada
/// acontece e nenhum store é tocado; com store que falha, o triage continua —
/// a indisponibilidade do store já foi avisada por quem o abriu na camada de
/// comando (regra da casa: o `aviso:` de store sai só no `main.rs`).
fn record_triage_call(
    ledger: Option<&Store>,
    kind: LlmKind,
    model: &str,
    prompt: &str,
    answer: &Result<String, Error>,
) {
    let Some(store) = ledger else { return };
    // Mesma estimativa de `post::estimate_tokens`: 3 chars por token. É
    // número de ledger (observabilidade), não de billing.
    let answer_len = answer.as_ref().map_or(0, |text| text.len());
    let tokens = u64::try_from((prompt.len() + answer_len) / 3).unwrap_or(0);
    let _ = store.record_llm_call(kind.name(), model, "triage", tokens);
}

/// Similaridade item × causa conhecida (PR35): overlap de tokens de conteúdo,
/// `|interseção| / |causa|` — a MESMA conta de `eval::causa_score`, com a
/// página da wiki no papel do gold. Causa sem token de conteúdo devolve 0.0
/// (página vazia não "acerta" de graça). Cálculo simétrico e auditável: nenhuma
/// IA no meio, zero rede, zero chamada (CLI-first, LLM-last — ADR-0016).
#[must_use]
pub fn cache_score(item_text: &str, causa_texto: &str) -> f64 {
    let causa_tokens = crate::eval::content_tokens(causa_texto);
    if causa_tokens.is_empty() {
        return 0.0;
    }
    let item_tokens = crate::eval::content_tokens(item_text);
    let intersecao = causa_tokens.intersection(&item_tokens).count();
    f64::from(u32::try_from(intersecao).unwrap_or(u32::MAX))
        / f64::from(u32::try_from(causa_tokens.len()).unwrap_or(u32::MAX))
}

/// Texto do item usado na similaridade do cache: título + descrição em texto
/// plano — a MESMA limpeza do prompt ([`description_for_prompt`]), então o
/// HTML não vira token e um segredo colado no item não é tratado como termo.
fn item_text_for_cache(item: &WorkItem) -> String {
    format!(
        "{}\n{}",
        item.title,
        description_for_prompt(item.description.as_deref())
    )
}

/// Cache de raciocínio (PR35): procura um episódio da wiki cuja causa conhecida
/// responda o item — página direta do MESMO item primeiro, depois as demais
/// páginas de `wiki/bugs/` (cross-item).
///
/// A fonte de verdade é o MARKDOWN (ADR-0013): `Wiki::list()` varre o disco, o
/// índice FTS5 é derivado e pode estar ausente/velho — o cache não depende dele.
/// Fail-open fica na camada de cima: `Err` vira nota, nunca erro do comando.
fn cache_lookup(item: &WorkItem) -> Result<Option<(CacheHit, Episode)>, String> {
    let Some(root) = crate::store::detect_project_root() else {
        // Sem raiz não há wiki — mesma conduta do pack (silêncio, rota normal).
        return Ok(None);
    };
    cache_lookup_at(&root, item)
}

/// [`cache_lookup`] com a raiz explícita — o mesmo desenho de `context_pack`
/// vs `pack_excluding`: a variante com raiz é a testável sem `chdir`.
fn cache_lookup_at(root: &Path, item: &WorkItem) -> Result<Option<(CacheHit, Episode)>, String> {
    let wiki = Wiki::open(root).map_err(|err| err.to_string())?;
    let texto = item_text_for_cache(item);
    // 1) Página direta do item: o caso comum (re-triagem do mesmo bug).
    let page = episode::page_for(item.id);
    if let Some(hit) = cached_episode(&wiki, &page, &texto)? {
        return Ok(Some(hit));
    }
    // 2) Candidatas: as outras páginas de bugs, em ordem de `list()` (sorted);
    // empate de similaridade fica com a primeira (determinismo do gate M1).
    let mut melhor: Option<(CacheHit, Episode)> = None;
    for candidata in wiki.list().map_err(|err| err.to_string())? {
        if !candidata.starts_with("bugs/") || candidata == page {
            continue;
        }
        if let Some(hit) = cached_episode(&wiki, &candidata, &texto)?
            && melhor
                .as_ref()
                .is_none_or(|(atual, _)| hit.0.similarity > atual.similarity)
        {
            melhor = Some(hit);
        }
    }
    Ok(melhor)
}

/// Lê a página, extrai o episódio mais recente e decide se ele responde o item:
/// causa conhecida (não vazia/`—`) e similaridade ≥ [`CACHE_LIMIAR`].
fn cached_episode(
    wiki: &Wiki,
    page: &str,
    item_text: &str,
) -> Result<Option<(CacheHit, Episode)>, String> {
    let Some(content) = wiki.read(page).map_err(|err| err.to_string())? else {
        return Ok(None);
    };
    let Some(episode) = episode::latest(&content) else {
        return Ok(None);
    };
    let causa = episode.root_cause.trim();
    if causa.is_empty() || causa == SECTION_EMPTY {
        return Ok(None);
    }
    let similarity = cache_score(item_text, causa);
    if similarity < CACHE_LIMIAR {
        return Ok(None);
    }
    Ok(Some((
        CacheHit {
            page: page.to_string(),
            similarity,
        },
        episode,
    )))
}

/// D2 (PR35): a confiança do hit é a da página original, CAPADA em `média` —
/// cache nunca diz `alta`: a resposta não foi produzida nesta execução.
fn cap_confidence(confidence: &str) -> String {
    let trimmed = confidence.trim().trim_end_matches('.');
    if trimmed.is_empty() {
        return SECTION_EMPTY.to_string();
    }
    if trimmed.eq_ignore_ascii_case("alta") {
        return "média".to_string();
    }
    trimmed.to_string()
}

/// Monta o [`TriageReport`] do hit (PR35): as 3 seções vêm da página — nada é
/// inventado — no formato byte-idêntico ao da rota LLM (`render_sections`).
/// O pack fica default (nada foi enviado ao prompt); o ledger registra pack 0,
/// zero chamadas e rota `cache-wiki` na camada de comando.
fn cache_report(item_id: i32, hit: &CacheHit, episodio: &Episode) -> TriageReport {
    let placeholder = |text: &str| {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            SECTION_EMPTY.to_string()
        } else {
            trimmed.to_string()
        }
    };
    let contents = [
        placeholder(&episodio.root_cause),
        placeholder(&episodio.files),
        cap_confidence(&episodio.confidence),
    ];
    let mut output = SECTION_NAMES
        .iter()
        .zip(contents)
        .map(|(name, content)| format!("{name}:\n{content}"))
        .collect::<Vec<_>>()
        .join("\n\n");
    output.push('\n');
    TriageReport {
        item_id,
        output,
        context: crate::context::ContextPack::default(),
        cache: Some(hit.clone()),
    }
}

/// Retrieval for the prompt (PR31 + PR5).
///
/// Fail-open by design: without a project root, store or wiki the pack is empty
/// and `triage` behaves exactly as before — context is an improvement, never a
/// new way to fail.
fn context_pack(item: &WorkItem, exclude: Option<&Path>) -> crate::context::ContextPack {
    let Some(root) = crate::store::detect_project_root() else {
        return crate::context::ContextPack::default();
    };
    crate::context::pack_excluding(&root, item, exclude)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_FULL: &str = include_str!("../tests/fixtures/work_item_4821.json");

    fn parse(body: &str) -> WorkItem {
        WorkItem::from_api_json(body).unwrap_or_else(|err| panic!("fixture inválida: {err}"))
    }

    /// Formats a model answer that is known to be in format (test-only sugar).
    fn format(text: &str) -> String {
        format_triage(text).unwrap_or_else(|err| panic!("formato válido não pode falhar: {err}"))
    }

    #[test]
    fn build_prompt_contains_fields_and_cleaned_html() {
        let prompt = build_prompt(&parse(FIXTURE_FULL));
        assert!(prompt.contains("ID: 4821\n"), "{prompt}");
        assert!(prompt.contains("Tipo: Bug\n"), "{prompt}");
        assert!(
            prompt.contains("Título: Sessão expirada com token válido\n"),
            "{prompt}"
        );
        assert!(prompt.contains("Estado: Active\n"), "{prompt}");
        assert!(prompt.contains("Área: Kira\\Backend\n"), "{prompt}");
        assert!(prompt.contains("Responsável: Maria Souza\n"), "{prompt}");
        assert!(
            prompt.contains("Usuário relata sessão expirada após 5 minutos."),
            "{prompt}"
        );
        assert!(prompt.contains("Login & logout não resolvem."), "{prompt}");
        // Os marcadores do bloco delimitado (ADR-0024 R2) carregam < e > por
        // design; fora deles o prompt não pode conter tag HTML.
        let without_markers = prompt
            .replace(
                crate::llm::prompts::TRIAGE_USER_PREAMBLE,
                crate::llm::prompts::TRIAGE_PROMPT_VERSION,
            )
            .replace(
                crate::llm::prompts::TRIAGE_UNTRUSTED_END,
                crate::llm::prompts::TRIAGE_PROMPT_VERSION,
            );
        assert!(
            !without_markers.contains('<'),
            "prompt não pode conter tag HTML: {prompt}"
        );
        assert!(
            !prompt.contains("&amp;"),
            "prompt não pode conter entidade crua: {prompt}"
        );
    }

    /// Privacy boundary: a description with a pasted PAT, e-mail and CPF must not
    /// reach the model verbatim (W1-PR2, AGENTS.md).
    #[test]
    fn build_prompt_masks_secrets_from_title_and_description() {
        let item = WorkItem {
            id: 4821,
            title: "Erro com token=AbC123XyZ789QqQ no header".to_string(),
            state: "Active".to_string(),
            work_item_type: "Bug".to_string(),
            assigned_to: Some("Maria Souza".to_string()),
            description: Some(
                "<div>Contato serjota@proton.me CPF 123.456.789-01 \
                 Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6</div>"
                    .to_string(),
            ),
            area_path: Some("Kira\\Backend".to_string()),
        };
        let prompt = build_prompt(&item);
        assert!(!prompt.contains("AbC123XyZ789QqQ"), "PAT vazou: {prompt}");
        assert!(
            !prompt.contains("serjota@proton.me"),
            "e-mail vazou: {prompt}"
        );
        assert!(!prompt.contains("123.456.789-01"), "CPF vazou: {prompt}");
        assert!(
            !prompt.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6"),
            "token vazou: {prompt}"
        );
        assert!(
            prompt.contains("Maria Souza"),
            "campo não secreto preservado: {prompt}"
        );
    }

    /// PR31: the retrieved context must reach the prompt, and the no-context path
    /// must stay byte-identical to what shipped before (no silent behaviour change).
    #[test]
    fn context_pack_is_appended_to_the_prompt() {
        let item = parse(FIXTURE_FULL);
        let without = build_prompt(&item);
        let pack = crate::context::ContextPack {
            sections: vec![crate::context::ContextSection {
                page: "bugs/4821".to_string(),
                path: ".kira/wiki/bugs/4821.md".to_string(),
                age_days: Some(3),
                excerpt: "token renovado sem rotacionar".to_string(),
            }],
            code: vec![crate::context::CodeExcerpt {
                path: "src/auth/session.rs".to_string(),
                line: 42,
                text: "let ttl = Duration::minutes(5);".to_string(),
                rank: None,
                priority: false,
                evidence: crate::context::EvidenceState::Confirmed,
                omitted: 0,
            }],
            findings_brief: Some("high 1".to_string()),
            findings: vec!["high · Newtonsoft.Json 12.0.1 (src/App.csproj)".to_string()],
            skills: Vec::new(),
            knowledge: Vec::new(),
            used_chars: 120,
            budget_chars: 12_288,
            notes: Vec::new(),
            repo_source: crate::context::RepoSource::Grep,
            dedup_chars_saved: 0,
        };
        let with = build_prompt_with_context(&item, &pack);
        assert!(
            !without.contains(".kira/wiki/bugs/4821.md"),
            "sem pack não há wiki"
        );
        assert!(with.contains(".kira/wiki/bugs/4821.md"), "{with}");
        assert!(with.contains("3 dia(s) atrás"), "{with}");
        assert!(with.contains("token renovado sem rotacionar"), "{with}");
        assert!(
            with.contains("findings do último `kira scan`: high 1"),
            "{with}"
        );
        // PR5: o trecho de repositório chega com caminho:linha auditável.
        assert!(with.contains("src/auth/session.rs:42 —"), "{with}");
        assert!(
            with.contains("achado: high · Newtonsoft.Json 12.0.1"),
            "{with}"
        );
        // O work item continua antes do contexto.
        let id_at = with.find("ID: 4821").unwrap_or(0);
        let ctx_at = with.find("Contexto do projeto").unwrap_or(usize::MAX);
        assert!(id_at < ctx_at, "work item primeiro, contexto depois");
    }

    #[test]
    fn build_prompt_handles_missing_optional_fields() {
        let item = WorkItem {
            id: 7,
            title: "t".to_string(),
            state: "New".to_string(),
            work_item_type: "Task".to_string(),
            assigned_to: None,
            description: None,
            area_path: None,
        };
        let prompt = build_prompt(&item);
        assert!(prompt.contains("Área: —\n"), "{prompt}");
        assert!(prompt.contains("Responsável: —\n"), "{prompt}");
        assert!(prompt.contains("Descrição:\n(sem descrição)"), "{prompt}");
    }

    #[test]
    fn format_triage_renders_the_three_fixed_sections() {
        let model_text = "Causa raiz provável:\nTTL do token muito curto.\nArquivos prováveis:\nsrc/auth/session.rs\nConfiança:\nmédia";
        assert_eq!(
            format(model_text),
            "Causa raiz provável:\nTTL do token muito curto.\n\n\
             Arquivos prováveis:\nsrc/auth/session.rs\n\n\
             Confiança:\nmédia\n"
        );
    }

    #[test]
    fn format_triage_tolerates_markdown_decorated_headers() {
        let model_text = "## Causa raiz provável\n_ttl_ curto\n**Arquivos prováveis:**\nsrc/x.rs\n- **Confiança:** alta";
        assert_eq!(
            format(model_text),
            "Causa raiz provável:\n_ttl_ curto\n\n\
             Arquivos prováveis:\nsrc/x.rs\n\n\
             Confiança:\nalta\n"
        );
    }

    /// W2-fix PR-F2: the real failure was a model answer with none of the three
    /// headers being rendered as three `—` with exit 0. Out of format is an
    /// actionable error, never an empty result.
    #[test]
    fn format_triage_without_any_header_is_an_actionable_error() {
        for model_text in [
            "texto solto do modelo",
            "",
            "   \n\n",
            "A causa raiz provável é o TTL curto do token.",
        ] {
            match format_triage(model_text) {
                Err(Error::LlmFormat) => {
                    let message = Error::LlmFormat.to_string();
                    assert!(
                        message.contains("fora do formato das 3 seções"),
                        "{message}"
                    );
                    assert!(message.contains("--model"), "{message}");
                    assert!(message.contains("ollama ps"), "{message}");
                }
                other => panic!("{model_text:?} deveria falhar com LlmFormat, veio {other:?}"),
            }
        }
    }

    /// The complement of the rule above: a partially followed template is data.
    /// A section the model omits (or leaves empty) shows `—`; only the absence
    /// of *all* headers is an error.
    #[test]
    fn format_triage_with_two_of_three_sections_placeholders_only_the_missing_one() {
        let out = format("Causa raiz provável:\nTTL curto\nArquivos prováveis:\nsrc/a.rs");
        assert!(out.contains("Causa raiz provável:\nTTL curto\n"), "{out}");
        assert!(out.contains("Arquivos prováveis:\nsrc/a.rs\n"), "{out}");
        assert!(out.contains("Confiança:\n—\n"), "{out}");

        let empty_third =
            format("Causa raiz provável:\nTTL curto\nArquivos prováveis:\nsrc/a.rs\nConfiança:\n");
        assert!(empty_third.contains("Confiança:\n—\n"), "{empty_third}");
        assert!(
            !empty_third.contains("Arquivos prováveis:\n—"),
            "{empty_third}"
        );
    }

    /// PR36 (v3): a saída com tag de evidência — que o prompt agora exige do
    /// modelo — continua parseável por `sections_of` (episódio, `--eval`, pr
    /// draft, painel), sem tratamento especial.
    #[test]
    fn format_triage_tolerates_the_evidence_tags() {
        let out = format(
            "Causa raiz provável:\nTTL do token muito curto.\nArquivos prováveis:\n\
             src/auth/session.rs (CONFIRMADA)\nConfiança:\nmédia",
        );
        assert!(
            out.contains("Arquivos prováveis:\nsrc/auth/session.rs (CONFIRMADA)"),
            "a tag passa pelo formatador intacta: {out}"
        );
        let sections = sections_of(&out);
        assert_eq!(sections[1], "src/auth/session.rs (CONFIRMADA)");
        // A tag vinda do cache/wiki (episódio gravado com tag) também parseia.
        let cached = sections_of(&format(
            "Causa raiz provável:\nx\nArquivos prováveis:\nsrc/a.rs (INFERIDA)\nConfiança:\nbaixa",
        ));
        assert_eq!(cached[1], "src/a.rs (INFERIDA)");
    }

    #[test]
    fn format_triage_shows_placeholder_for_empty_sections() {
        let out = format("Causa raiz provável:\n\nArquivos prováveis:\n\nConfiança:\n");
        assert!(out.contains("Causa raiz provável:\n—\n"), "{out}");
        assert!(out.contains("Arquivos prováveis:\n—\n"), "{out}");
        assert!(out.contains("Confiança:\n—\n"), "{out}");
    }

    /// The episodic capture (PR30) reads back exactly what was printed: the
    /// three sections, in the fixed order, trimmed.
    #[test]
    fn sections_of_returns_the_three_sections_in_order() {
        let formatted = format(
            "Causa raiz provável:\nTTL curto\nArquivos prováveis:\nsrc/a.rs, src/b.rs\nConfiança:\nalta",
        );
        let sections = sections_of(&formatted);
        assert_eq!(sections[0], "TTL curto");
        assert_eq!(sections[1], "src/a.rs, src/b.rs");
        assert_eq!(sections[2], "alta");

        let empty = sections_of(&format(
            "Causa raiz provável:\n\nArquivos prováveis:\n\nConfiança:\n",
        ));
        assert_eq!(empty, ["—", "—", "—"], "seções vazias voltam como —");
    }

    // -----------------------------------------------------------------------
    // PR35 — cache de raciocínio: scorer, limiar, lookup e confiança capada.
    // -----------------------------------------------------------------------

    /// Wiki temporária de teste (mesma mecânica dos testes de `episode`).
    fn temp_wiki(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "kira-triage-cache-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.subsec_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("criar {dir:?}: {err}"));
        dir
    }

    /// Escreve uma página de episódio no formato que `episode::record` produz.
    fn write_bug_page(root: &Path, item_id: i32, cause: &str, confidence: &str) {
        let path = root
            .join(".kira")
            .join("wiki")
            .join("bugs")
            .join(format!("{item_id}.md"));
        std::fs::create_dir_all(path.parent().unwrap_or_else(|| Path::new(".")))
            .unwrap_or_else(|err| panic!("criar bugs/: {err}"));
        let content = format!(
            "<!-- kira-wiki updated_at=2026-11-10T22:18:00Z -->\n\n# Bug {item_id}\n\n## Episódio 2026-11-10T22:18:00Z · item {item_id}\n- Causa raiz prevista: {cause}\n- Arquivos prováveis: src/auth/session.rs\n- Confiança: {confidence}\n"
        );
        std::fs::write(&path, content).unwrap_or_else(|err| panic!("escrever {path:?}: {err}"));
    }

    #[test]
    fn cache_score_is_token_overlap_with_a_documented_threshold() {
        // Cobertura total da causa ⇒ 1.0 (hit confortável).
        let item = "Sessão expirada com token válido após minutos";
        let causa = "sessão expirada com token válido";
        let score = cache_score(item, causa);
        assert!(
            (score - 1.0).abs() < f64::EPSILON,
            "causa coberta ⇒ 1.0: {score}"
        );
        // Metade da causa coberta ⇒ exatamente o limiar (mesma régua do eval).
        let metade = cache_score("sessão expirada", "sessão expirada com token válido");
        assert!(
            (metade - 0.5).abs() < f64::EPSILON && metade >= CACHE_LIMIAR,
            "metade dos tokens = limiar: {metade}"
        );
        // Nada em comum ⇒ bem abaixo do limiar.
        assert!(cache_score("deploy do front quebrou", "sessão expirada") < CACHE_LIMIAR);
        // Causa sem token de conteúdo (só stopwords/curtas) ⇒ 0.0, nunca hit.
        assert_eq!(cache_score("sessão expirada", "com que para"), 0.0);
    }

    #[test]
    fn cache_lookup_hits_the_direct_page_of_the_same_item() {
        let root = temp_wiki("direct-hit");
        write_bug_page(
            &root,
            4821,
            "sessão expirada com token válido após minutos",
            "alta",
        );
        let item = parse(FIXTURE_FULL);
        let (hit, episodio) = cache_lookup_at(&root, &item)
            .unwrap_or_else(|cause| panic!("wiki válida não pode falhar: {cause}"))
            .unwrap_or_else(|| panic!("item com página parecida deve bater no cache"));
        assert_eq!(hit.page, "bugs/4821", "{hit:?}");
        assert!(hit.similarity >= CACHE_LIMIAR, "{hit:?}");
        assert_eq!(
            episodio.root_cause,
            "sessão expirada com token válido após minutos"
        );
        assert_eq!(episodio.files, "src/auth/session.rs");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn cache_lookup_misses_below_the_threshold_and_reports_none() {
        let root = temp_wiki("miss");
        write_bug_page(
            &root,
            4821,
            "cache sem invalidação no deploy do front",
            "baixa",
        );
        let item = parse(FIXTURE_FULL);
        let lookup = cache_lookup_at(&root, &item)
            .unwrap_or_else(|cause| panic!("wiki válida não pode falhar: {cause}"));
        assert!(
            lookup.is_none(),
            "similaridade baixa ⇒ rota LLM: {lookup:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn cache_lookup_falls_back_to_other_bug_pages() {
        let root = temp_wiki("cross-item");
        // O item 4821 não tem página; a causa conhecida está no bug 9999.
        write_bug_page(
            &root,
            9999,
            "sessão expirada com token válido após minutos",
            "média",
        );
        let item = parse(FIXTURE_FULL);
        let (hit, episodio) = cache_lookup_at(&root, &item)
            .unwrap_or_else(|cause| panic!("wiki válida não pode falhar: {cause}"))
            .unwrap_or_else(|| panic!("bug parecido em outra página deve bater (cross-item)"));
        assert_eq!(hit.page, "bugs/9999", "{hit:?}");
        assert_eq!(episodio.item_id, 9999, "{episodio:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    // Fail-open do wiki ilegível é POR SISTEMA: no Unix, `.kira` como arquivo
    // dá ENOTDIR (Err com causa); no Windows o mesmo cenário degrada para
    // NotFound (Ok(None) = miss). Os dois caminhos são o contrato de fail-open
    // — "rota LLM de sempre, nunca pânico" — testados separadamente, cada um
    // no SO que o produz (cenários de chmod/ENOTDIR não são portáveis).
    #[cfg(unix)]
    #[test]
    fn cache_lookup_fails_with_a_cause_when_the_wiki_is_unreadable() {
        // `.kira` como ARQUIVO: ler `bugs/4821.md` é impossível (ENOTDIR).
        let root = temp_wiki("broken");
        std::fs::write(root.join(".kira"), "não sou um diretório")
            .unwrap_or_else(|err| panic!("criar .kira: {err}"));
        let item = parse(FIXTURE_FULL);
        match cache_lookup_at(&root, &item) {
            Err(cause) => assert!(
                !cause.is_empty(),
                "a causa alimenta a nota acionável no stderr"
            ),
            other => panic!("wiki ilegível deve devolver Err, veio {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(windows)]
    #[test]
    fn cache_lookup_misses_when_the_wiki_root_is_not_a_directory() {
        // Windows: abrir `arquivo/filho` volta ERROR_PATH_NOT_FOUND (NotFound),
        // então a página "não existe" — miss limpo, rota LLM, sem pânico.
        let root = temp_wiki("broken-windows");
        std::fs::write(root.join(".kira"), "não sou um diretório")
            .unwrap_or_else(|err| panic!("criar .kira: {err}"));
        let item = parse(FIXTURE_FULL);
        match cache_lookup_at(&root, &item) {
            Ok(None) => {}
            other => panic!("no Windows o cenário é miss, veio {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn cache_confidence_is_capped_at_media_never_alta() {
        // D2: cache nunca diga "alta" — a resposta não foi produzida aqui.
        assert_eq!(cap_confidence("alta"), "média");
        assert_eq!(cap_confidence("ALTA"), "média");
        assert_eq!(cap_confidence("alta."), "média");
        assert_eq!(cap_confidence("média"), "média");
        assert_eq!(cap_confidence("baixa"), "baixa");
        assert_eq!(cap_confidence(""), "—");
    }

    #[test]
    fn cache_report_keeps_the_three_sections_in_the_llm_format() {
        let episodio = Episode {
            item_id: 4821,
            root_cause: "sessão expirada com token válido".to_string(),
            files: "src/auth/session.rs".to_string(),
            confidence: "alta".to_string(),
            findings_brief: None,
        };
        let hit = CacheHit {
            page: "bugs/4821".to_string(),
            similarity: 1.0,
        };
        let report = cache_report(4821, &hit, &episodio);
        assert_eq!(report.item_id, 4821);
        // D2: `alta` vinda de cache sai capada em `média`.
        assert!(
            report.output.contains("Confiança:\nmédia\n"),
            "{}",
            report.output
        );
        assert!(!report.output.contains("alta"), "{}", report.output);
        assert!(
            report
                .output
                .contains("Causa raiz provável:\nsessão expirada com token válido\n"),
            "causa conhecida, nada inventado: {}",
            report.output
        );
        assert!(
            report
                .output
                .contains("Arquivos prováveis:\nsrc/auth/session.rs\n"),
            "{}",
            report.output
        );
        assert!(report.cache.is_some(), "o hit viaja para o ledger/stderr");
        assert!(
            report.context.is_empty(),
            "no hit nada foi enviado ao prompt: pack default"
        );
        // O formato é o mesmo que `format_triage` produz — parseável por
        // `sections_of` (pr draft, painel) sem tratamento especial.
        let sections = sections_of(&report.output);
        assert_eq!(sections[0], "sessão expirada com token válido");
        assert_eq!(sections[2], "média");
    }
}
