//! `kira pr draft` — rascunho de PR em markdown a partir do que o KIRA já sabe.
//!
//! 100% local: SEM rede e SEM LLM (ADR-0016: parser/cálculo antes de modelo —
//! montar um rascunho é colar dados, não julgamento). Fontes, nesta ordem:
//! (a) último `kira triage ITEM_ID` registrado na trilha do store (causa raiz,
//! arquivos, confiança); (b) achados do `.kira/findings.json` cujo path
//! aparece entre os arquivos citados pelo triage; (c) até 3 páginas de wiki
//! relacionadas por termo comum; (d) item do board quando `--file` é passado.
//!
//! Regra anti-invenção (inegociável): evidência só entra se veio de dado real
//! (sessão/finding/wiki). Quando falta, a seção mostra `—` com a CAUSA —
//! nunca arquivo, linha, causa ou métrica inventados. O texto inteiro passa
//! por [`crate::redact`] antes de sair (o rascunho pode ir para PR público).
//!
//! Determinismo: template fixo, nenhuma chamada a `now()` — data vem do dado
//! gravado. Mesma entrada = mesma saída byte a byte.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::format::DESCRIPTION_MAX_CHARS;
use crate::html::html_to_text;
use crate::store::{Store, WikiPageRow};
use crate::work_item::WorkItem;

/// Placeholder de seção sem dado (mesma convenção do `kira triage`).
pub const MISSING: &str = "—";

/// Limite de páginas de wiki relacionadas no rascunho (escopo fechado: 3).
const WIKI_MAX_PAGES: usize = 3;

/// Quantos termos comuns são usados para relacionar páginas de wiki.
const TERMS_MAX: usize = 3;

/// Quantas sessões recentes são vasculhadas para achar o triage do item.
const TRIAGE_LOOKBACK: usize = 200;

/// Teto do trecho de wiki exibido em Evidências.
const WIKI_EXCERPT_CHARS: usize = 160;

/// Palavras de ligação que não caracterizam relação entre textos (PT/EN).
const STOPWORDS: [&str; 22] = [
    "para", "isso", "esse", "essa", "este", "esta", "como", "sobre", "quando", "onde", "mais",
    "muito", "entre", "porque", "também", "there", "that", "this", "with", "from", "then", "than",
];

/// Opções de uma execução de `kira pr draft`.
pub struct DraftOptions {
    /// Work item id pedido na linha de comando.
    pub item_id: i32,
    /// JSON do item do board (opcional; sem ele usa só o que está no store).
    pub file: Option<PathBuf>,
    /// Caminho explícito para escrever o markdown (nunca escolhido aqui).
    pub out: Option<PathBuf>,
    /// Devolve a estrutura em JSON no stdout (o `--out` continua gravando o markdown).
    pub json: bool,
}

/// Tudo que o rascunho conhece sobre o item — só dado real, nada inventado.
pub struct DraftData {
    /// Id pedido (é dado real da linha de comando).
    pub item_id: i32,
    /// Item do board quando `--file` foi passado.
    pub item: Option<WorkItem>,
    /// Último triage do item registrado no store.
    pub triage: Option<TriageSource>,
    /// Achados do scan cujo path casa com os arquivos citados pelo triage.
    pub findings: Vec<PrFinding>,
    /// Páginas de wiki relacionadas por termo comum (até 3).
    pub wiki: Vec<WikiEvidence>,
}

/// O triage de uma sessão da trilha, já recortado nas 3 seções fixas.
pub struct TriageSource {
    /// Id da sessão na trilha (`kira history` mostra o mesmo número).
    pub session_id: i64,
    /// Data do triage em RFC 3339 UTC — data vem do dado, nunca de `now()`.
    pub started_at: String,
    /// Texto da seção `Causa raiz provável` (pode ser `—`: é o que o triage disse).
    pub causa_raiz: String,
    /// Arquivos da seção `Arquivos prováveis`, normalizados para `/`.
    pub arquivos: Vec<String>,
    /// Texto da seção `Confiança`.
    pub confianca: String,
}

/// Um achado do `.kira/findings.json`, tolerante a campo ausente.
#[derive(Debug, Clone, Deserialize)]
pub struct PrFinding {
    #[serde(default)]
    pub tool: String,
    #[serde(default)]
    pub package: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub severity: String,
    /// Caminho (relação da ferramenta) — a chave do casamento com o triage.
    pub path: String,
    #[serde(default)]
    pub message: String,
    /// Linha, quando o arquivo de findings a traz; sem linha NUNCA se inventa uma.
    #[serde(default)]
    pub line: Option<u64>,
}

/// Evidência de wiki: caminho real no disco + trecho literal do corpo.
#[derive(Debug, Clone)]
pub struct WikiEvidence {
    /// Path da página como gravado no índice (fonte de verdade do caminho).
    pub path: String,
    /// Primeiros caracteres do corpo, whitespace normalizado.
    pub excerpt: String,
}

/// Roda o comando inteiro: coleta, rende, redige e (só com `--out`) escreve.
///
/// Devolve o texto do stdout: markdown por padrão, JSON com `--json`.
/// A leitura do `--file` falha alto (o usuário pediu aquele caminho);
/// store/findings/wiki falham ABERTO (o rascunho sai com `—` e a causa).
pub fn run(options: &DraftOptions) -> Result<String, Error> {
    let data = collect(options.item_id, options.file.as_deref())?;
    let markdown = draft_text(&data);
    if let Some(out) = &options.out {
        // Caminho explícito: o comando nunca escolhe onde escrever sozinho.
        std::fs::write(out, &markdown).map_err(|source| Error::Io {
            path: out.display().to_string(),
            source,
        })?;
    }
    if options.json {
        return Ok(render_json(&data));
    }
    Ok(markdown)
}

/// Texto final do rascunho (markdown rendido + redação obrigatória).
///
/// A redação vem DEPOIS do template: o rascunho pode ir para PR público,
/// então `token=...` no triage gravado não pode vazar.
#[must_use]
pub fn draft_text(data: &DraftData) -> String {
    crate::redact::redact(&render_markdown(data))
}

/// Coleta os dados de todas as fontes (item `--file` é erro alto; o resto é
/// fail-open com causa, seguindo a regra do AGENTS.md para leitura).
fn collect(item_id: i32, item_file: Option<&Path>) -> Result<DraftData, Error> {
    let item = match item_file {
        Some(path) => {
            let body = std::fs::read_to_string(path).map_err(|source| Error::Io {
                path: path.display().to_string(),
                source,
            })?;
            Some(WorkItem::from_api_json(&body)?)
        }
        None => None,
    };
    let store = match Store::open_default() {
        Ok(store) => store,
        Err(cause) => {
            // Fail-open: um rascunho sem store ainda é útil (e honesto, com —).
            eprintln!("aviso: store indisponível ({cause}) — o rascunho sai sem triage/wiki");
            return Ok(DraftData {
                item_id,
                item,
                triage: None,
                findings: Vec::new(),
                wiki: Vec::new(),
            });
        }
    };
    let project = crate::store::detect_project_root()
        .and_then(|root| store.find_project(&root).ok().flatten());
    let triage = project
        .as_ref()
        .and_then(|project| triage_source(&store, &project.id, item_id));
    let cited = triage
        .as_ref()
        .map_or_else(Vec::new, |source| source.arquivos.clone());
    let findings = match_findings(&read_findings(), &cited);
    // Termos comuns vêm do triage (texto) e do título do item quando existe.
    // As strings são donas da vida do escopo: referências temporárias dentro
    // do vetor fariam o borrow checker recusar (temp morto na mesma linha).
    let title = item
        .as_ref()
        .map_or(String::new(), |item| item.title.clone());
    let causa_text = triage
        .as_ref()
        .map_or(String::new(), |source| source.causa_raiz.clone());
    let files_text = triage
        .as_ref()
        .map_or(String::new(), |source| source.arquivos.join(" "));
    let mut term_texts: Vec<&str> = Vec::new();
    for text in [&causa_text, &files_text, &title] {
        if !text.is_empty() {
            term_texts.push(text);
        }
    }
    let terms = common_terms(&term_texts);
    let wiki = related_wiki(&read_wiki_pages(&store), &terms);
    Ok(DraftData {
        item_id,
        item,
        triage,
        findings,
        wiki,
    })
}

/// Acha a última sessão de triage do item na trilha e recorta as seções.
///
/// A chave é `sessions.item_id` — o item é dado da sessão (issue #78), então
/// funciona igual para `triage 4821` e para `triage --file item.json`. O
/// casamento pela string dos argumentos fica como fallback para linhas
/// gravadas antes da migração v5, que não têm a coluna preenchida.
fn triage_source(store: &Store, project_id: &str, item_id: i32) -> Option<TriageSource> {
    let sessions = store.last_sessions(project_id, TRIAGE_LOOKBACK).ok()?;
    let id_token = item_id.to_string();
    // `last_sessions` vem mais recente primeiro: o primeiro casamento é o último triage.
    let por_item = sessions
        .iter()
        .find(|session| session.command == "triage" && session.item_id == Some(item_id));
    let legado = || {
        sessions.iter().find(|session| {
            session.command == "triage"
                && session.args.split_whitespace().next() == Some(id_token.as_str())
        })
    };
    por_item.or_else(legado).and_then(triage_from_session)
}

/// Recorta o resumo gravado (o output formatado do triage, primeiros 2000
/// chars) nas 3 seções fixas via a MESMA função do `kira triage` — nada de
/// parser paralelo divergente.
///
/// Sem os cabeçalhos no resumo não há triage confiável: `None`, e a seção do
/// rascunho mostra a causa (o resumo truncado não vira "causa raiz").
#[must_use]
pub fn triage_from_session(session: &crate::store::SessionRow) -> Option<TriageSource> {
    if !session.summary.contains("Causa raiz provável") {
        return None;
    }
    let sections = crate::triage::sections_of(&session.summary);
    Some(TriageSource {
        session_id: session.id,
        started_at: session.started_at.clone(),
        causa_raiz: sections[0].clone(),
        arquivos: cited_files(&sections[1]),
        confianca: sections[2].clone(),
    })
}

/// Extrai os caminhos citados numa seção `Arquivos prováveis`.
///
/// Aceita separação por vírgula, ponto-e-vírgula, quebra de linha e espaço,
/// limpa cercas de markdown e normaliza `\` para `/` (convenção do scan).
/// Só entra token que PARECE caminho (contém `.` ou `/`) — a seção pode trazer
/// prosa do modelo, e prosa não é arquivo.
#[must_use]
pub fn cited_files(section: &str) -> Vec<String> {
    let mut files: Vec<String> = Vec::new();
    for token in section.split(|c: char| c == ',' || c == ';' || c.is_whitespace()) {
        let cleaned = token.trim_matches(|c| c == '`' || c == '"' || c == '\'');
        let looks_like_path =
            cleaned.len() >= 2 && (cleaned.contains('.') || cleaned.contains('/'));
        if !looks_like_path {
            continue;
        }
        let normalized = cleaned.replace('\\', "/");
        if !files.contains(&normalized) {
            files.push(normalized);
        }
    }
    files
}

/// Lê o `.kira/findings.json` do projeto detectado — FAIL-OPEN: arquivo
/// ausente, ilegível ou malformado devolve lista vazia (sem achado não há
/// evidência; nunca inventar). O campo `line` é opcional: o scan de hoje não
/// grava linha, e o draft só imprime `path:linha` quando ELA EXISTE.
#[must_use]
pub fn read_findings() -> Vec<PrFinding> {
    #[derive(Deserialize)]
    struct Document {
        #[serde(default)]
        findings: Vec<PrFinding>,
    }
    let Some(root) = crate::store::detect_project_root() else {
        return Vec::new();
    };
    let path = root.join(".kira").join("findings.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    serde_json::from_str::<Document>(&text)
        .map_or_else(|_| Vec::new(), |document| document.findings)
}

/// Filtra os achados cujo path casa com algum arquivo citado pelo triage.
///
/// Casamento por contenção nos dois sentidos (a ferramenta pode trazer
/// `src/lib/auth/session.rs` quando o triage citou `src/auth/session.rs`),
/// com piso de tamanho para não casar string vazia.
#[must_use]
pub fn match_findings(findings: &[PrFinding], cited: &[String]) -> Vec<PrFinding> {
    findings
        .iter()
        .filter(|finding| {
            let path = finding.path.trim();
            path.len() >= 2
                && cited
                    .iter()
                    .any(|file| path.contains(file.as_str()) || file.contains(path))
        })
        .cloned()
        .collect()
}

/// Lê as páginas do índice de wiki (derivado e descartável); falha é nota
/// internalizada como lista vazia.
fn read_wiki_pages(store: &Store) -> Vec<WikiPageRow> {
    store.wiki_pages().unwrap_or_default()
}

/// Até 3 páginas de wiki relacionadas por termo comum.
///
/// Score = quantos termos comuns aparecem em título+corpo; empate cai para o
/// slug (ordem alfabética) — determinístico e sem FTS5 (a relação é por
/// termo, não por ranking de busca). Score 0 = sem relação = fora.
#[must_use]
pub fn related_wiki(pages: &[WikiPageRow], terms: &[String]) -> Vec<WikiEvidence> {
    if terms.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<(usize, &WikiPageRow)> = pages
        .iter()
        .map(|page| (score_page(page, terms), page))
        .filter(|(score, _)| *score > 0)
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.slug.cmp(&b.1.slug)));
    scored
        .into_iter()
        .take(WIKI_MAX_PAGES)
        .map(|(_, page)| WikiEvidence {
            path: page.path.clone(),
            excerpt: excerpt_of(&page.body),
        })
        .collect()
}

fn score_page(page: &WikiPageRow, terms: &[String]) -> usize {
    let haystack = format!(
        "{}\n{}",
        page.title.to_lowercase(),
        page.body.to_lowercase()
    );
    terms
        .iter()
        .filter(|term| haystack.contains(term.as_str()))
        .count()
}

/// Primeiros [`WIKI_EXCERPT_CHARS`] do corpo com whitespace normalizado
/// (trecho literal, só mais legível em uma linha).
fn excerpt_of(body: &str) -> String {
    let flat = body.split_whitespace().collect::<Vec<_>>().join(" ");
    flat.chars().take(WIKI_EXCERPT_CHARS).collect()
}

/// Termos comuns (≥4 chars, fora da lista de ligação) ordenados por
/// frequência e, no empate, alfabeticamente — corte determinístico.
#[must_use]
pub fn common_terms(texts: &[&str]) -> Vec<String> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for text in texts {
        for token in text.split(|c: char| !c.is_alphanumeric()) {
            if token.chars().count() < 4 {
                continue;
            }
            let lowered = token.to_lowercase();
            if STOPWORDS.contains(&lowered.as_str()) {
                continue;
            }
            *counts.entry(lowered).or_default() += 1;
        }
    }
    let mut pairs: Vec<(String, usize)> = counts.into_iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    pairs
        .into_iter()
        .take(TERMS_MAX)
        .map(|(term, _)| term)
        .collect()
}

/// Bullet de evidência de um achado: `path:linha — trecho` quando o achado
/// TEM linha; `path — trecho` quando não tem. Linha nunca é deduzida.
#[must_use]
pub fn finding_bullet(finding: &PrFinding) -> String {
    let tool = non_empty_or(&finding.tool, "ferramenta desconhecida");
    let severity = non_empty_or(&finding.severity, "sem severidade");
    let message = non_empty_or(&finding.message, "(sem mensagem)");
    let anchor = match finding.line {
        Some(line) => format!("{}:{line}", finding.path),
        None => finding.path.clone(),
    };
    format!("{anchor} — achado {tool}/{severity}: {message}")
}

fn non_empty_or(text: &str, fallback: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

/// Título do cabeçalho `#`: o do item quando `--file` trouxe o item; sem item
/// o id é o único dado real disponível — inventar título é proibido.
fn title_of(data: &DraftData) -> String {
    data.item.as_ref().map_or_else(
        || format!("item {}", data.item_id),
        |item| item.title.trim().to_string(),
    )
}

/// Bloco `## Contexto`: dados do item; sem item, a referência ao triage do
/// store; sem nenhum dos dois, `—` com a causa certa.
fn contexto_block(data: &DraftData) -> String {
    if let Some(item) = &data.item {
        let mut block = format!(
            "Item #{} do tipo {} no estado {}.\n",
            item.id, item.work_item_type, item.state
        );
        if let Some(area) = item
            .area_path
            .as_deref()
            .filter(|area| !area.trim().is_empty())
        {
            block.push_str(&format!("Área: {area}\n"));
        }
        if let Some(owner) = item
            .assigned_to
            .as_deref()
            .filter(|owner| !owner.trim().is_empty())
        {
            block.push_str(&format!("Responsável: {owner}\n"));
        }
        block.push_str("Descrição:\n");
        block.push_str(&description_text(item.description.as_deref()));
        block.push('\n');
        return block;
    }
    if let Some(triage) = &data.triage {
        // Sem o JSON do item o contexto honesto é a origem no store — a data
        // vem do dado gravado (RFC 3339 UTC), nunca de `now()`.
        return format!(
            "{MISSING} (não há dado do item; passe --file caminho.json para o contexto completo)\n\
             Último triage registrado no store: sessão #{} ({})\n",
            triage.session_id, triage.started_at
        );
    }
    format!(
        "{MISSING} (não há dado; rode kira triage {} antes ou passe --file caminho.json)\n",
        data.item_id
    )
}

/// Descrição do item limpa de HTML e truncada como no prompt do triage.
fn description_text(description: Option<&str>) -> String {
    let Some(description) = description else {
        return "(sem descrição)".to_string();
    };
    let plain = html_to_text(description);
    if plain.is_empty() {
        return "(sem descrição)".to_string();
    }
    let mut text: String = plain.chars().take(DESCRIPTION_MAX_CHARS).collect();
    if plain.chars().count() > DESCRIPTION_MAX_CHARS {
        text.push('…');
    }
    text
}

/// Bloco `## Causa raiz provável`: texto do triage + confiança; sem triage,
/// `—` com a causa (o rascunho nunca chuta causa).
fn causa_block(data: &DraftData) -> String {
    match &data.triage {
        Some(triage) => {
            let mut block = triage.causa_raiz.clone();
            block.push_str("\n\nConfiança: ");
            block.push_str(&triage.confianca);
            block.push('\n');
            block
        }
        None => format!(
            "{MISSING} (não há dado; rode kira triage {} antes)\n",
            data.item_id
        ),
    }
}

/// Bloco `## Mudança proposta`: derivada SÓ dos arquivos citados pelo triage —
/// nenhum arquivo novo é sugerido (anti-invenção).
fn mudanca_block(data: &DraftData) -> String {
    let Some(triage) = &data.triage else {
        return format!(
            "{MISSING} (não há dado; rode kira triage {} antes)\n",
            data.item_id
        );
    };
    if triage.arquivos.is_empty() {
        return format!(
            "{MISSING} (o triage registrado não citou arquivos; rode kira triage {} de novo)\n",
            data.item_id
        );
    }
    let mut block = String::from(
        "A partir dos arquivos citados pelo último triage (nenhum arquivo novo é sugerido):\n",
    );
    for file in &triage.arquivos {
        block.push_str(&format!("- revisar `{file}` (citado pelo triage)\n"));
    }
    block
}

/// Bloco `## Evidências`: bullets de findings casados + páginas de wiki.
fn evidencias_block(data: &DraftData) -> String {
    let mut bullets: Vec<String> = data
        .findings
        .iter()
        .map(finding_bullet)
        .chain(
            data.wiki
                .iter()
                .map(|page| format!("{} — trecho: \"{}\"", page.path, page.excerpt)),
        )
        .collect();
    if bullets.is_empty() {
        return format!(
            "{MISSING} (não há dado; rode kira triage {} antes ou rode kira scan)\n",
            data.item_id
        );
    }
    bullets.push(String::new());
    bullets.join("\n")
}

/// Bloco `## Riscos`: só o que os achados relacionados dizem (severidade é
/// a gravada pela ferramenta) — sem risco, `—` com a causa.
fn riscos_block(data: &DraftData) -> String {
    if data.findings.is_empty() {
        return format!(
            "{MISSING} (não há achado de scan relacionado aos arquivos citados; rode kira scan antes)\n"
        );
    }
    let mut bullets: Vec<String> = data
        .findings
        .iter()
        .map(|finding| {
            let severity = non_empty_or(&finding.severity, "sem severidade");
            let message = non_empty_or(&finding.message, "(sem mensagem)");
            format!(
                "{severity}: {} {} — {message}",
                non_empty_or(&finding.package, "pacote desconhecido"),
                non_empty_or(&finding.version, "versão desconhecida")
            )
        })
        .collect();
    bullets.push(String::new());
    bullets.join("\n")
}

/// Checklist fixo e determinístico: gates locais, merge humano, sem segredo.
const CHECKLIST_BLOCK: &str = "- [ ] `cargo fmt --check`, `cargo clippy --all-targets -D warnings`, `cargo test` e `cargo build` verdes\n\
     - [ ] revisão humana antes do merge — o merge é sempre humano\n\
     - [ ] diff sem segredos (nenhum PAT/token/e-mail no diff)\n";

/// Rende o markdown do rascunho no template fixo (ordem das seções é contrato).
#[must_use]
pub fn render_markdown(data: &DraftData) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", title_of(data)));
    out.push_str("## Contexto\n");
    out.push_str(&contexto_block(data));
    out.push('\n');
    out.push_str("## Causa raiz provável\n");
    out.push_str(&causa_block(data));
    out.push('\n');
    out.push_str("## Mudança proposta\n");
    out.push_str(&mudanca_block(data));
    out.push('\n');
    out.push_str("## Evidências\n");
    out.push_str(&evidencias_block(data));
    out.push('\n');
    out.push_str("## Riscos\n");
    out.push_str(&riscos_block(data));
    out.push('\n');
    out.push_str("## Checklist\n");
    out.push_str(CHECKLIST_BLOCK);
    out
}

/// Estrutura devolvida por `--json`: os mesmos blocos do markdown, por seção,
/// mais o markdown inteiro (já redigido) para consumo por máquina.
#[derive(Serialize)]
struct DraftJson {
    item_id: i32,
    title: String,
    contexto: String,
    causa_raiz: String,
    mudanca_proposta: Vec<String>,
    evidencias: Vec<String>,
    riscos: Vec<String>,
    markdown: String,
}

/// JSON (pretty + `\n` final) de `--json`. Todo campo de texto passa por
/// [`crate::redact`] — a estrutura sai tão pública quanto o markdown.
fn render_json(data: &DraftData) -> String {
    let mudanca = match &data.triage {
        Some(triage) if !triage.arquivos.is_empty() => triage
            .arquivos
            .iter()
            .map(|file| format!("revisar `{file}` (citado pelo triage)"))
            .collect(),
        _ => Vec::new(),
    };
    let evidencias: Vec<String> = data
        .findings
        .iter()
        .map(finding_bullet)
        .chain(
            data.wiki
                .iter()
                .map(|page| format!("{} — trecho: \"{}\"", page.path, page.excerpt)),
        )
        .collect();
    let riscos: Vec<String> = data
        .findings
        .iter()
        .map(|finding| {
            let severity = non_empty_or(&finding.severity, "sem severidade");
            let message = non_empty_or(&finding.message, "(sem mensagem)");
            format!(
                "{severity}: {} {} — {message}",
                non_empty_or(&finding.package, "pacote desconhecido"),
                non_empty_or(&finding.version, "versão desconhecida")
            )
        })
        .collect();
    let payload = DraftJson {
        item_id: data.item_id,
        title: crate::redact::redact(&title_of(data)),
        contexto: crate::redact::redact(&contexto_block(data)),
        causa_raiz: crate::redact::redact(&causa_block(data)),
        mudanca_proposta: mudanca
            .iter()
            .map(|line| crate::redact::redact(line))
            .collect(),
        evidencias: evidencias
            .iter()
            .map(|line| crate::redact::redact(line))
            .collect(),
        riscos: riscos
            .iter()
            .map(|line| crate::redact::redact(line))
            .collect(),
        markdown: draft_text(data),
    };
    let mut text = serde_json::to_string_pretty(&payload).unwrap_or_else(|_| {
        // allow-expect-equivalente: os campos são todos String/i32/Vec<String>,
        // a serialização não pode falhar; cair no placeholder mantém o comando
        // sem pânico (regra AGENTS.md) e honesto sobre o resultado.
        "{}".to_string()
    });
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::SessionRow;

    const ID: i32 = 4821;

    fn item() -> WorkItem {
        WorkItem {
            id: ID,
            title: "Sessão expirada com token válido".to_string(),
            state: "Active".to_string(),
            work_item_type: "Bug".to_string(),
            assigned_to: Some("Maria Souza".to_string()),
            description: Some(
                "<div>Usuário relata sessão expirada após 5 minutos.</div>".to_string(),
            ),
            area_path: Some("Kira\\Backend".to_string()),
        }
    }

    fn triage() -> TriageSource {
        TriageSource {
            session_id: 7,
            started_at: "2026-01-05T10:00:00Z".to_string(),
            causa_raiz: "TTL do token muito curto.".to_string(),
            arquivos: vec!["src/auth/session.rs".to_string()],
            confianca: "média".to_string(),
        }
    }

    fn finding(line: Option<u64>) -> PrFinding {
        PrFinding {
            tool: "audit".to_string(),
            package: "session-mgmt".to_string(),
            version: "1.2.0".to_string(),
            severity: "high".to_string(),
            path: "src/auth/session.rs".to_string(),
            message: "TTL fixo sem configurar".to_string(),
            line,
        }
    }

    fn wiki_page(path: &str, body: &str) -> WikiPageRow {
        WikiPageRow {
            slug: "bugs/4821".to_string(),
            kind: "bugs".to_string(),
            path: path.to_string(),
            title: "Sessão expirada".to_string(),
            updated_at: "2026-01-04T00:00:00Z".to_string(),
            content_hash: "0f0f0f0f0f0f0f0f".to_string(),
            body: body.to_string(),
        }
    }

    fn full_data() -> DraftData {
        DraftData {
            item_id: ID,
            item: Some(item()),
            triage: Some(triage()),
            findings: vec![finding(None)],
            wiki: vec![WikiEvidence {
                path: ".kira/wiki/bugs/4821.md".to_string(),
                excerpt: "TTL do token muito curto causa relogin frequente.".to_string(),
            }],
        }
    }

    #[test]
    fn template_completo_tem_todas_as_secoes_com_dado_real() {
        let text = render_markdown(&full_data());
        assert!(
            text.contains("# Sessão expirada com token válido\n"),
            "{text}"
        );
        assert!(
            text.contains("## Contexto\nItem #4821 do tipo Bug no estado Active."),
            "{text}"
        );
        assert!(
            text.contains("Usuário relata sessão expirada após 5 minutos."),
            "{text}"
        );
        assert!(!text.contains("<div>"), "HTML cru não pode vazar: {text}");
        assert!(
            text.contains("TTL do token muito curto.\n\nConfiança: média"),
            "{text}"
        );
        assert!(
            text.contains("- revisar `src/auth/session.rs` (citado pelo triage)"),
            "{text}"
        );
        assert!(
            text.contains("src/auth/session.rs — achado audit/high: TTL fixo sem configurar"),
            "{text}"
        );
        assert!(
            text.contains(".kira/wiki/bugs/4821.md — trecho: \"TTL do token muito curto"),
            "{text}"
        );
        assert!(
            text.contains("## Riscos\nhigh: session-mgmt 1.2.0 — TTL fixo sem configurar"),
            "{text}"
        );
        assert!(text.contains("o merge é sempre humano"), "{text}");
        assert!(text.contains("diff sem segredos"), "{text}");
    }

    /// Sem triage, cada seção que depende dele mostra `—` com a CAUSA certa —
    /// nunca um placeholder genérico sem causa.
    #[test]
    fn dados_faltando_devolvem_menos_com_a_causa_certa() {
        let vazio = DraftData {
            item_id: ID,
            item: None,
            triage: None,
            findings: Vec::new(),
            wiki: Vec::new(),
        };
        let text = render_markdown(&vazio);
        assert!(
            text.contains("## Contexto\n— (não há dado; rode kira triage 4821 antes ou passe --file caminho.json)"),
            "{text}"
        );
        assert!(
            text.contains("## Causa raiz provável\n— (não há dado; rode kira triage 4821 antes)"),
            "{text}"
        );
        assert!(
            text.contains("## Mudança proposta\n— (não há dado; rode kira triage 4821 antes)"),
            "{text}"
        );
        assert!(
            text.contains(
                "## Evidências\n— (não há dado; rode kira triage 4821 antes ou rode kira scan)"
            ),
            "{text}"
        );
        assert!(
            text.contains("## Riscos\n— (não há achado de scan relacionado aos arquivos citados; rode kira scan antes)"),
            "{text}"
        );
        // Sem item o título não pode ser inventado: cai para o id real.
        assert!(text.contains("# item 4821\n"), "{text}");
        // Checklist nunca fica vazio: é template, não dado.
        assert!(text.contains("- [ ] `cargo fmt --check`"), "{text}");
    }

    /// Contexto com triage mas sem item: a referência ao store é dado real.
    #[test]
    fn contexto_sem_item_cita_o_triage_do_store() {
        let data = DraftData {
            item_id: ID,
            item: None,
            triage: Some(triage()),
            findings: Vec::new(),
            wiki: Vec::new(),
        };
        let text = render_markdown(&data);
        assert!(
            text.contains("passe --file caminho.json para o contexto completo"),
            "{text}"
        );
        assert!(text.contains("sessão #7 (2026-01-05T10:00:00Z)"), "{text}");
    }

    /// Mudança proposta sem arquivos citados (triage vazio na seção) tem
    /// causa própria — diferente de "sem triage".
    #[test]
    fn mudanca_sem_arquivos_citados_tem_causa_propria() {
        let mut source = triage();
        source.arquivos = Vec::new();
        let data = DraftData {
            item_id: ID,
            item: None,
            triage: Some(source),
            findings: Vec::new(),
            wiki: Vec::new(),
        };
        let text = render_markdown(&data);
        assert!(
            text.contains("## Mudança proposta\n— (o triage registrado não citou arquivos; rode kira triage 4821 de novo)"),
            "{text}"
        );
    }

    /// Mesma entrada = mesma saída byte a byte (nenhum timestamp de runtime).
    #[test]
    fn mesma_entrada_produz_saida_byte_a_byte() {
        let first = render_markdown(&full_data());
        let second = render_markdown(&full_data());
        assert_eq!(first, second);
        assert_eq!(first.as_bytes(), second.as_bytes());
        // E o texto final redigido também é determinístico.
        assert_eq!(draft_text(&full_data()), draft_text(&full_data()));
    }

    /// Privacidade: token no triage gravado não pode vazar no rascunho.
    #[test]
    fn redaction_token_no_summary_nao_vaza() {
        let mut source = triage();
        source.causa_raiz = "TTL curto; PAT vazou: token=AbC123XyZ789QqQ".to_string();
        let data = DraftData {
            item_id: ID,
            item: Some(item()),
            triage: Some(source),
            findings: Vec::new(),
            wiki: Vec::new(),
        };
        let text = draft_text(&data);
        assert!(!text.contains("AbC123XyZ789QqQ"), "PAT vazou: {text}");
        assert!(text.contains("token="), "a chave fica legível: {text}");
        assert!(text.contains("[REDACTED]"), "{text}");
    }

    /// path:linha SÓ quando o finding tem linha — sem linha não se deduz.
    #[test]
    fn path_linha_so_quando_o_finding_tem_linha() {
        let com_linha = finding_bullet(&finding(Some(42)));
        assert!(
            com_linha.starts_with("src/auth/session.rs:42 — "),
            "{com_linha}"
        );
        let sem_linha = finding_bullet(&finding(None));
        assert!(
            sem_linha.starts_with("src/auth/session.rs — "),
            "{sem_linha}"
        );
        assert!(!sem_linha.contains("session.rs:"), "{sem_linha}");
    }

    #[test]
    fn cited_files_ignora_prosa_e_normaliza_separadores() {
        let files = cited_files(
            "src/auth/session.rs, src/lib/token.rs; `web/package.json`\nsrc\\win\\path.rs e o modelo falou de config",
        );
        assert_eq!(
            files,
            vec![
                "src/auth/session.rs".to_string(),
                "src/lib/token.rs".to_string(),
                "web/package.json".to_string(),
                "src/win/path.rs".to_string(),
            ]
        );
        assert!(cited_files("—").is_empty());
        assert!(cited_files("").is_empty());
    }

    #[test]
    fn cited_files_sem_duplicata() {
        assert_eq!(
            cited_files("src/a.rs src/a.rs"),
            vec!["src/a.rs".to_string()]
        );
    }

    #[test]
    fn match_findings_só_puxa_path_relacionado() {
        let relacionado = finding(None);
        let mut distante = finding(None);
        distante.path = "web/package.json".to_string();
        let matched = match_findings(
            &[relacionado, distante],
            &["src/auth/session.rs".to_string()],
        );
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].path, "src/auth/session.rs");
    }

    #[test]
    fn related_wiki_ordena_por_score_e_empata_por_slug() {
        let fora = wiki_page(".kira/wiki/outro.md", "fala de outra coisa completamente");
        let fraca = wiki_page(".kira/wiki/a-fraca.md", "token apenas");
        let forte = wiki_page(".kira/wiki/b-forte.md", "token ttl curto session token");
        let pages = vec![forte, fraca, fora];
        let terms = common_terms(&["TTL do token muito curto na session".to_string().as_str()]);
        let related = related_wiki(&pages, &terms);
        assert!(related.len() <= 3, "{related:?}");
        assert_eq!(related[0].path, ".kira/wiki/b-forte.md");
        // Página sem nenhum termo comum não entra.
        assert!(
            !related
                .iter()
                .any(|page| page.path == ".kira/wiki/outro.md")
        );
    }

    #[test]
    fn common_terms_ignora_conectivos_e_ordena_por_frequencia() {
        let terms = common_terms(&["token token ttl curto para com isso".to_string().as_str()]);
        assert_eq!(terms[0], "token");
        assert!(!terms.contains(&"para".to_string()));
        assert!(!terms.contains(&"isso".to_string()));
    }

    /// O resumo gravado pode ter sido truncado no cap de 2000 chars sem os
    /// cabeçalhos: sem formato confiável, o draft trata como sem triage.
    #[test]
    fn summary_sem_cabecalho_nao_vira_causa_raiz() {
        let session = SessionRow {
            id: 3,
            command: "triage".to_string(),
            args: "4821".to_string(),
            model: None,
            started_at: "2026-01-05T10:00:00Z".to_string(),
            local_started_at: None,
            duration_ms: 10,
            ok: true,
            summary: "resumo truncado sem formato algum".to_string(),
            item_id: None,
        };
        assert!(triage_from_session(&session).is_none());
    }

    /// Regressão da issue #78: `triage --file item.json` grava `--file` nos
    /// argumentos, então o casamento pela string NUNCA acha. O id na coluna
    /// `sessions.item_id` faz o rascunho recuperar o julgamento do mesmo jeito.
    #[test]
    fn triage_por_item_id_acha_o_que_foi_julgado_via_file() {
        let dir = std::env::temp_dir().join(format!("kira-prdraft-78-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("criar tempdir");
        let store = Store::open_in_dir(&dir).expect("abrir store");
        // O projeto tem que existir no disco: `ensure_project` canonicaliza.
        let projeto = store.ensure_project(&dir).expect("projeto do teste");

        // Sessão exatamente como o `triage --file` grava: args sem o id.
        store
            .record(
                &projeto,
                "triage",
                "--file item.json --llm copilot",
                Some("shim"),
                "2026-11-12T10:00:00Z",
                10,
                true,
                "Causa raiz provável:\nTTL do token muito curto.\nArquivos prováveis:\nsrc/auth/session.rs\nConfiança:\nmédia",
                Some(4821),
            )
            .expect("gravar sessão de triage");

        let achado = triage_source(&store, &projeto.id, 4821)
            .expect("o triage do item 4821 deve ser encontrado pelo item_id");
        assert!(
            achado.causa_raiz.contains("TTL do token muito curto."),
            "causa recuperada: {:?}",
            achado.causa_raiz
        );

        // Outro item não herda o julgamento deste.
        assert!(
            triage_source(&store, &projeto.id, 9999).is_none(),
            "item sem triage não recebe a causa de outro"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Caminho feliz do parser de sessão: as 3 seções saem na ordem fixa.
    #[test]
    fn summary_formatado_rende_as_tres_secoes() {
        let session = SessionRow {
            id: 3,
            command: "triage".to_string(),
            args: "4821 --llm ollama".to_string(),
            model: Some("qwen:4b".to_string()),
            started_at: "2026-01-05T10:00:00Z".to_string(),
            local_started_at: None,
            duration_ms: 10,
            ok: true,
            summary: "Causa raiz provável:\nTTL do token muito curto.\n\nArquivos prováveis:\nsrc/auth/session.rs\n\nConfiança:\nmédia\n".to_string(),
            item_id: None,
        };
        let source = triage_from_session(&session).unwrap_or_else(|| panic!("deve parsear"));
        assert_eq!(source.causa_raiz, "TTL do token muito curto.");
        assert_eq!(source.arquivos, vec!["src/auth/session.rs".to_string()]);
        assert_eq!(source.confianca, "média");
        assert_eq!(source.session_id, 3);
    }
}
