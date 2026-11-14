//! `kira triage --eval <dir>` (W2-bis, ADR-0018): mede o gate M2.
//!
//! Cada item do diretório é uma pasta com `item.json` (work item no formato da
//! API do Azure DevOps, o mesmo do `kira triage --file`) e `gold.json` com o
//! rótulo verdadeiro do dono (`causa_raiz`, `arquivos`, `confianca`). O triage
//! roda offline (`--file`) contra o backend escolhido e a resposta é comparada
//! com o gold por regras DETERMINÍSTICAS — nada de LLM julgando a si mesmo
//! (ADR-0018, "LLM-judge ... rejeitado por ora"):
//!
//! - **causa raiz**: similaridade de tokens de conteúdo
//!   (normalizar: minúsculas, sem acento, sem stopwords PT, palavras ≥ 4
//!   caracteres; `score = |interseção| / |gold|`). Acerto = score ≥
//!   [`CAUSA_LIMIAR`] (0.50).
//! - **arquivos**: pelo menos 1 basename do gold presente entre os basenames
//!   previstos (comparação sem caixa).
//!
//! Veredito do item = causa E arquivos acertados. O gate M2 passa com
//! ≥ [`GATE_LIMIAR_PCT`] (70%) dos itens — o exit code do comando traduz isso
//! (0 = PASSOU, 1 = NÃO PASSOU). Qualquer falha num item aborta a medição:
//! número parcial não é número.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::llm::LlmKind;
use crate::triage;

/// Limiar de acerto da causa raiz: `score >= 0.50` — documentado no README e
/// no ADR-0018; metade dos tokens de conteúdo do gold bastam.
pub const CAUSA_LIMIAR: f64 = 0.50;

/// Limiar do gate M2 em percentual: ≥ 70% dos itens com veredito ACERTOU.
pub const GATE_LIMIAR_PCT: f64 = 70.0;

/// Palavras de função do português que não contam como conteúdo. Aplicada dos
/// DOIS lados (prevista e gold), então qualquer escolha determinística é justa;
/// a lista é deliberadamente pequena e comentável — não é ciência, é filtro.
const STOPWORDS: [&str; 20] = [
    "para", "com", "que", "uma", "como", "mais", "isso", "este", "esta", "esse", "essa", "quando",
    "onde", "porque", "sobre", "entre", "antes", "depois", "tambem", "muito",
];

/// Mapeia os acentos comuns do português (e o cedilha) para a letra base.
/// Sem dependência nova: a normalização é uma tabela pequena, testada.
fn letra_sem_acento(ch: char) -> char {
    match ch {
        'á' | 'à' | 'â' | 'ã' | 'ä' | 'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' | 'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'u',
        'ç' | 'Ç' => 'c',
        'ñ' | 'Ñ' => 'n',
        outro => outro,
    }
}

/// Normaliza o texto: sem acento e em minúsculas (a ordem importa — o mapa já
/// cobre as maiúsculas acentuadas, então tanto faz, mas faixa única é mais
/// simples de testar).
fn normalizar(texto: &str) -> String {
    texto
        .chars()
        .map(letra_sem_acento)
        .collect::<String>()
        .to_lowercase()
}

/// Tokens de conteúdo do texto: palavras com ≥ 4 caracteres (letras/números),
/// sem acento, sem caixa e sem as [`STOPWORDS`].
#[must_use]
pub fn content_tokens(texto: &str) -> HashSet<String> {
    let normalizado = normalizar(texto);
    normalizado
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .filter(|palavra| palavra.chars().count() >= 4)
        .filter(|palavra| !STOPWORDS.contains(palavra))
        .map(str::to_string)
        .collect()
}

/// Similaridade da causa raiz prevista com o gold: `|interseção| / |gold|`.
///
/// Gold sem token de conteúdo devolve 0.0 (rotulagem vazia não pode "acertar
/// de graça"). O cálculo é simétrico e auditável: nenhuma IA no meio.
#[must_use]
pub fn causa_score(prevista: &str, gold: &str) -> f64 {
    let gold_tokens = content_tokens(gold);
    if gold_tokens.is_empty() {
        return 0.0;
    }
    let previstos = content_tokens(prevista);
    let intersecao = gold_tokens.intersection(&previstos).count();
    f64::from(u32::try_from(intersecao).unwrap_or(u32::MAX))
        / f64::from(u32::try_from(gold_tokens.len()).unwrap_or(u32::MAX))
}

/// Extrai os basenames dos arquivos citados na seção "Arquivos prováveis":
/// um caminho (ou nome) por token, separados por espaço/vírgula/ponto-e-
/// vírgula/quebra de linha. O placeholder "—" (seção vazia) e o
/// `não identificado` exigido do prompt não viram basename. Comparação
/// fica em minúsculas (Windows é insensível a caixa).
#[must_use]
pub fn file_basenames(texto: &str) -> Vec<String> {
    texto
        .split(|ch: char| ch == ',' || ch == ';' || ch.is_whitespace())
        .map(str::trim)
        .map(|token| {
            token
                .trim_matches(|ch: char| {
                    matches!(ch, '"' | '\'' | '`' | '*' | '(' | ')' | '[' | ']')
                })
                .replace('\\', "/")
        })
        .filter(|caminho| !caminho.is_empty())
        .filter(|caminho| caminho != "—" && caminho != crate::llm::prompts::TRIAGE_FILES_UNKNOWN)
        .map(|caminho| {
            let nome = caminho
                .rsplit('/')
                .next()
                .map_or_else(|| caminho.clone(), str::to_string);
            nome.to_lowercase()
        })
        .filter(|nome| !nome.is_empty())
        .collect()
}

/// Basename de um caminho do gold (mesma normalização do lado previsto).
fn gold_basename(caminho: &str) -> String {
    caminho
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .map_or_else(|| caminho.to_string(), str::to_lowercase)
}

/// Acerto de arquivos: pelo menos 1 basename do gold aparece entre os
/// previstos. Gold sem arquivo devolve `false` (nada rotulado = nada acerta).
#[must_use]
pub fn files_ok(previstos: &str, gold: &[String]) -> bool {
    let gold_nomes: HashSet<String> = gold
        .iter()
        .map(String::as_str)
        .map(gold_basename)
        .filter(|nome| !nome.is_empty())
        .collect();
    if gold_nomes.is_empty() {
        return false;
    }
    let previstos_nomes: HashSet<String> = file_basenames(previstos).into_iter().collect();
    previstos_nomes.intersection(&gold_nomes).next().is_some()
}

/// Confiança normalizada aceita no gold (o prompt exige alta|média|baixa).
fn confianca_normalizada(valor: &str) -> Option<String> {
    match normalizar(valor).as_str() {
        "alta" => Some("alta".to_string()),
        "media" => Some("media".to_string()),
        "baixa" => Some("baixa".to_string()),
        _ => None,
    }
}

/// Rótulo verdadeiro do dono (`gold.json` de cada pasta do diretório).
#[derive(Debug, Deserialize)]
pub struct GoldLabel {
    causa_raiz: String,
    #[serde(default)]
    arquivos: Vec<String>,
    #[serde(default)]
    confianca: Option<String>,
}

/// Resultado da medição de um item.
#[derive(Debug, Clone, Serialize)]
pub struct ItemResult {
    /// Id do work item (do `item.json`).
    pub id: i32,
    /// Nome da pasta do item (ordenação e rastreio).
    pub pasta: String,
    /// Similaridade de tokens da causa raiz (0.0..=1.0).
    pub causa_score: f64,
    /// Causa raiz acertou (score ≥ [`CAUSA_LIMIAR`]).
    pub causa_ok: bool,
    /// Pelo menos 1 basename do gold apareceu na previsão.
    pub arquivos_ok: bool,
    /// "ACERTOU" quando causa E arquivos acertaram; "ERROU" caso contrário.
    pub veredito: String,
    /// Causa raiz prevista pelo triage (texto literal da seção).
    pub causa_prevista: String,
    /// Causa raiz do gold (texto literal).
    pub causa_gold: String,
    /// Basenames previstos.
    pub arquivos_previstos: Vec<String>,
    /// Caminhos do gold.
    pub arquivos_gold: Vec<String>,
    /// Confiança prevista (seção do triage).
    pub confianca_prevista: String,
    /// Confiança do gold, normalizada (alta|media|baixa).
    pub confianca_gold: Option<String>,
}

/// Veredito agregado do gate M2.
#[derive(Debug, Serialize)]
pub struct GateResult {
    /// Itens com veredito ACERTOU.
    pub acertos: usize,
    /// Total de itens medidos.
    pub total: usize,
    /// Percentual exato (o texto impresso arredonda; o JSON carrega o valor).
    pub pct: f64,
    /// `pct >= GATE_LIMIAR_PCT` — o critério do ADR-0018 (≥ 70%).
    pub passou: bool,
}

/// Relatório completo do `--json`.
#[derive(Debug, Serialize)]
pub struct EvalReport {
    /// Limiares usados (auditáveis, mesmos valores das constantes).
    pub limiares: EvalThresholds,
    /// Um resultado por item, na ordem de pasta.
    pub itens: Vec<ItemResult>,
    /// Veredito agregado do gate.
    pub gate: GateResult,
}

/// Limiares expostos no JSON (documentados no README/ADR-0018).
#[derive(Debug, Serialize)]
pub struct EvalThresholds {
    /// Causa raiz: score mínimo para acerto.
    pub causa: f64,
    /// Gate M2: percentual mínimo para PASSOU.
    pub gate_pct: f64,
}

/// Pastas seed sintéticas embutidas no binário (`--seed`): nome e conteúdo.
/// As fixtures vivem em `tests/fixtures/triage_gold_seed/` e são CLARAMENTE
/// sintéticas (nome `seed-sintetico-*` + campo `nota` no gold) — nunca um
/// rótulo inventado do dono.
const SEEDS: [(&str, &str, &str); 3] = [
    (
        "seed-sintetico-01",
        include_str!("../tests/fixtures/triage_gold_seed/seed-sintetico-01/item.json"),
        include_str!("../tests/fixtures/triage_gold_seed/seed-sintetico-01/gold.json"),
    ),
    (
        "seed-sintetico-02",
        include_str!("../tests/fixtures/triage_gold_seed/seed-sintetico-02/item.json"),
        include_str!("../tests/fixtures/triage_gold_seed/seed-sintetico-02/gold.json"),
    ),
    (
        "seed-sintetico-03",
        include_str!("../tests/fixtures/triage_gold_seed/seed-sintetico-03/item.json"),
        include_str!("../tests/fixtures/triage_gold_seed/seed-sintetico-03/gold.json"),
    ),
];

/// Mensagem acionável para diretório de eval vazio/ausente: o schema esperado
/// com um exemplo de 5 linhas e a saída `--seed`.
fn erro_schema(dir: &Path) -> Error {
    Error::Eval(format!(
        "o diretório de eval '{}' não existe ou está vazio — cada item é uma pasta com 2 arquivos:\n\
         \x20 <pasta-do-bug>/\n\
         \x20   item.json  → JSON do work item do Azure DevOps (o mesmo do `kira triage --file`)\n\
         \x20   gold.json  → rótulo do dono:\n\
         \x20     {{\n\
         \x20       \"causa_raiz\": \"TTL do token de sessão expira antes do refresh\",\n\
         \x20       \"arquivos\": [\"src/Api/Auth/TokenService.cs\"],\n\
         \x20       \"confianca\": \"alta\"\n\
         \x20     }}\n\
         para testar o formato sem rotular os 20, rode `kira triage --eval '{}' --seed` (copia 3 fixtures sintéticas seed-sintetico-*)",
        dir.display(),
        dir.display()
    ))
}

/// Lista as pastas de item do diretório (ordem estável): toda subpasta que
/// tem `item.json` e `gold.json`.
fn item_dirs(dir: &Path) -> Result<Vec<PathBuf>, Error> {
    if !dir.is_dir() {
        return Err(erro_schema(dir));
    }
    let mut pastas: Vec<PathBuf> = Vec::new();
    let mut entradas = std::fs::read_dir(dir).map_err(|source| Error::Io {
        path: dir.display().to_string(),
        source,
    })?;
    loop {
        let entrada = match entradas.next() {
            Some(Ok(entrada)) => entrada,
            Some(Err(source)) => {
                return Err(Error::Io {
                    path: dir.display().to_string(),
                    source,
                });
            }
            None => break,
        };
        let path = entrada.path();
        if path.join("item.json").is_file() && path.join("gold.json").is_file() {
            pastas.push(path);
        }
    }
    // A ordem não muda o veredito, mas a tabela tem que ser a mesma em duas
    // execuções (determinismo do AGENTS.md).
    pastas.sort();
    Ok(pastas)
}

/// Lê e valida o gold de uma pasta: `causa_raiz` obrigatória, `confianca`
/// (quando presente) tem que ser alta|media|baixa.
fn ler_gold(pasta: &Path) -> Result<GoldLabel, Error> {
    let caminho = pasta.join("gold.json");
    let corpo = std::fs::read_to_string(&caminho).map_err(|source| Error::Io {
        path: caminho.display().to_string(),
        source,
    })?;
    let gold: GoldLabel = serde_json::from_str(&corpo).map_err(|err| {
        Error::Eval(format!(
            "gold.json inválido em '{}' — o schema é {{\"causa_raiz\": string, \"arquivos\": [string], \"confianca\": \"alta|media|baixa\"}}: {err}",
            pasta.display()
        ))
    })?;
    // let-chain (edition 2024): a validação vive numa única guarda.
    if let Some(confianca) = gold.confianca.as_deref()
        && confianca_normalizada(confianca).is_none()
    {
        return Err(Error::Eval(format!(
            "confianca '{confianca}' inválida no gold de '{}' — use alta|media|baixa",
            pasta.display()
        )));
    }
    Ok(gold)
}

/// Copia as 3 fixtures seed sintéticas para `dir` — SÓ se o diretório estiver
/// vazio (ou não existir ainda; é criado). Nunca sobrescreve nada: rótulo do
/// dono não se apaga por acidente.
pub fn seed_dir(dir: &Path) -> Result<(), Error> {
    if dir.exists() {
        let mut entradas = std::fs::read_dir(dir).map_err(|source| Error::Io {
            path: dir.display().to_string(),
            source,
        })?;
        let cheio = entradas
            .next()
            .transpose()
            .map_err(|source| Error::Io {
                path: dir.display().to_string(),
                source,
            })?
            .is_some();
        if cheio {
            return Err(Error::Eval(format!(
                "o diretório '{}' não está vazio — `--seed` só escreve num diretório vazio (as seeds são sintéticas e nomeadas seed-sintetico-*); aponte para um diretório novo",
                dir.display()
            )));
        }
    }
    std::fs::create_dir_all(dir).map_err(|source| Error::Io {
        path: dir.display().to_string(),
        source,
    })?;
    for (nome, item, gold) in SEEDS {
        let pasta = dir.join(nome);
        std::fs::create_dir_all(&pasta).map_err(|source| Error::Io {
            path: pasta.display().to_string(),
            source,
        })?;
        std::fs::write(pasta.join("item.json"), item).map_err(|source| Error::Io {
            path: pasta.join("item.json").display().to_string(),
            source,
        })?;
        std::fs::write(pasta.join("gold.json"), gold).map_err(|source| Error::Io {
            path: pasta.join("gold.json").display().to_string(),
            source,
        })?;
    }
    Ok(())
}

/// Roda a medição completa do gate M2: um triage por item (offline, `--file`,
/// backend explícito) + comparação determinística com o gold. Qualquer item
/// que falhe aborta tudo — a medição nunca sai parcial.
pub async fn run(
    dir: &Path,
    kind: LlmKind,
    base_url: &str,
    model: &str,
    ledger: Option<&crate::store::Store>,
) -> Result<(Vec<ItemResult>, GateResult), Error> {
    let pastas = item_dirs(dir)?;
    if pastas.is_empty() {
        return Err(erro_schema(dir));
    }
    let max_calls = crate::credentials::max_llm_calls_per_day();
    let mut itens: Vec<ItemResult> = Vec::with_capacity(pastas.len());
    for pasta in &pastas {
        // P0 (auditoria F2): o `--eval` faz UMA chamada de LLM por item do
        // gold; o teto diário é checado ANTES de cada uma — esgotou, a medição
        // aborta inteira (nunca parcial, mesmo contrato do erro de item).
        if let Some(store) = ledger
            && let Err(err) = store.ensure_llm_budget(max_calls, "triage --eval")
        {
            return Err(Error::Eval(err.to_string()));
        }
        itens.push(medir_item(pasta, kind, base_url, model, ledger).await?);
    }
    let resultado_gate = gate(&itens);
    Ok((itens, resultado_gate))
}

/// Mede um item: triage offline + comparação determinística.
async fn medir_item(
    pasta: &Path,
    kind: LlmKind,
    base_url: &str,
    model: &str,
    ledger: Option<&crate::store::Store>,
) -> Result<ItemResult, Error> {
    let gold = ler_gold(pasta)?;
    let item_path = pasta.join("item.json");
    // LLM porque: a causa raiz prevista é hipótese sobre texto (mesma resposta
    // do triage, ADR-0016); a COMPARAÇÃO com o gold aqui embaixo é 100%
    // determinística — o `--eval` jamais usa um LLM como juiz.
    // A medição é abortada no primeiro item que falha: número parcial não é
    // número, e o erro do backend/triage já é acionável por si só.
    let report = triage::triage_file_llm(kind, base_url, model, &item_path, ledger)
        .await
        .map_err(|err| {
            Error::Eval(format!(
                "item '{}' falhou no triage — medição abortada (nunca parcial): {err}",
                pasta.display()
            ))
        })?;
    let secoes = triage::sections_of(&report.output);
    let causa_prevista = secoes[0].clone();
    let arquivos_previstos = file_basenames(&secoes[1]);
    let score = causa_score(&causa_prevista, &gold.causa_raiz);
    let causa_ok = score >= CAUSA_LIMIAR;
    let arquivos_ok = files_ok(&secoes[1], &gold.arquivos);
    Ok(ItemResult {
        id: report.item_id,
        pasta: pasta
            .file_name()
            .map_or_else(String::new, |nome| nome.to_string_lossy().into_owned()),
        causa_score: score,
        causa_ok,
        arquivos_ok,
        veredito: if causa_ok && arquivos_ok {
            "ACERTOU".to_string()
        } else {
            "ERROU".to_string()
        },
        causa_prevista,
        causa_gold: gold.causa_raiz,
        arquivos_previstos,
        arquivos_gold: gold.arquivos,
        confianca_prevista: secoes[2].clone(),
        confianca_gold: gold.confianca.as_deref().and_then(confianca_normalizada),
    })
}

/// Agrega os itens no veredito do gate M2.
#[must_use]
pub fn gate(itens: &[ItemResult]) -> GateResult {
    let total = itens.len();
    let acertos = itens
        .iter()
        .filter(|item| item.veredito == "ACERTOU")
        .count();
    let pct = if total == 0 {
        0.0
    } else {
        f64::from(u32::try_from(acertos).unwrap_or(u32::MAX)) * 100.0
            / f64::from(u32::try_from(total).unwrap_or(u32::MAX))
    };
    GateResult {
        acertos,
        total,
        pct,
        passou: pct >= GATE_LIMIAR_PCT,
    }
}

/// Monta o relatório `--json` (mesmos campos da tabela + textos previstos/gold).
#[must_use]
pub fn report(itens: &[ItemResult], gate_result: &GateResult) -> EvalReport {
    EvalReport {
        limiares: EvalThresholds {
            causa: CAUSA_LIMIAR,
            gate_pct: GATE_LIMIAR_PCT,
        },
        itens: itens.to_vec(),
        gate: GateResult {
            acertos: gate_result.acertos,
            total: gate_result.total,
            pct: gate_result.pct,
            passou: gate_result.passou,
        },
    }
}

/// Tabela por item + a linha do gate. Determinística: mesma medição, mesmo
/// texto byte a byte.
#[must_use]
pub fn render_table(itens: &[ItemResult], gate_result: &GateResult) -> String {
    let largura_id = itens
        .iter()
        .map(|item| item.id.to_string().len())
        .max()
        .unwrap_or(2)
        .max(2);
    let mut linhas: Vec<String> = Vec::with_capacity(itens.len() + 2);
    linhas.push(format!(
        "{:>wid$} | {} | {} | {}",
        "id",
        "causa %",
        "arquivos ok?",
        "veredito",
        wid = largura_id
    ));
    for item in itens {
        linhas.push(format!(
            "{:>wid$} | {:>7} | {:>12} | {}",
            item.id,
            format!("{:.0}%", item.causa_score * 100.0),
            if item.arquivos_ok { "sim" } else { "não" },
            item.veredito,
            wid = largura_id
        ));
    }
    linhas.push(gate_line(gate_result));
    let mut texto = linhas.join("\n");
    texto.push('\n');
    texto
}

/// A linha final do gate: `GATE M2: <acertos>/<total> = <pct>% (limiar 70%) — PASSOU|NÃO PASSOU`.
#[must_use]
pub fn gate_line(gate_result: &GateResult) -> String {
    format!(
        "GATE M2: {}/{} = {:.0}% (limiar {:.0}%) — {}",
        gate_result.acertos,
        gate_result.total,
        gate_result.pct,
        GATE_LIMIAR_PCT,
        if gate_result.passou {
            "PASSOU"
        } else {
            "NÃO PASSOU"
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Monta um texto com `total` tokens no gold e `comuns` tokens em comum
    /// com a previsão — para testar o limiar nos dois lados sem depender de
    /// acaso da tokenização.
    fn causa_com_intersecao(total: usize, comuns: usize) -> (String, String) {
        let gold: Vec<String> = (0..total).map(|i| format!("palavra{i:04}")).collect();
        let prevista: Vec<String> = std::iter::once("texto".to_string())
            .chain((0..comuns).map(|i| format!("palavra{i:04}")))
            .collect();
        (gold.join(" "), prevista.join(" "))
    }

    #[test]
    fn causa_score_exact_match_is_one() {
        let gold = "TTL do token de sessão expira antes do refresh";
        assert_eq!(causa_score(gold, gold), 1.0);
    }

    #[test]
    fn causa_score_sinonimos_e_vazio_sao_zero() {
        // Nenhum token em comum = 0.0 mesmo com texto cheio dos dois lados.
        let (gold, prevista) = causa_com_intersecao(5, 0);
        assert_eq!(causa_score(&prevista, &gold), 0.0);
        // Previsão vazia (seção "—") = 0.0.
        assert_eq!(causa_score("—", &gold), 0.0);
        // Gold sem token de conteúdo (rotulagem vazia) = 0.0, nunca acerto.
        assert_eq!(causa_score("TTL do token curto", ""), 0.0);
        assert_eq!(causa_score("TTL do token curto", "de da ao"), 0.0);
    }

    #[test]
    fn causa_score_normaliza_acento_e_caixa() {
        let gold = "Sessão Expirada com TOKEN válido";
        // Caixa e acento não podem derrubar o score: mesmas palavras.
        assert_eq!(causa_score("sessao expirada com token valido", gold), 1.0);
        assert_eq!(causa_score("SESSÃO EXPIRADA COM TOKEN VÁLIDO", gold), 1.0);
    }

    #[test]
    fn causa_score_respeita_o_limiar_nos_dois_lados() {
        // 3/7 ≈ 0.43 < 0.50 → ERROU; 4/7 ≈ 0.57 ≥ 0.50 → acerto.
        let (gold, abaixo) = causa_com_intersecao(7, 3);
        assert!(causa_score(&abaixo, &gold) < CAUSA_LIMIAR, "abaixo");
        let (_, acima) = causa_com_intersecao(7, 4);
        assert!(causa_score(&acima, &gold) >= CAUSA_LIMIAR, "acima");
        // Lado exato do limiar: 1/2 = 0.50 acerta (score ≥ 0.50).
        let gold2 = "token sessao";
        assert_eq!(causa_score("token expirado", gold2), 0.5);
        // 0.49 vs 0.51 com a mesma direção da desigualdade:
        let (gold100, quase) = causa_com_intersecao(100, 49);
        assert!(causa_score(&quase, &gold100) < CAUSA_LIMIAR, "0.49");
        let (_, passado) = causa_com_intersecao(100, 51);
        assert!(causa_score(&passado, &gold100) > CAUSA_LIMIAR, "0.51");
    }

    #[test]
    fn content_tokens_corta_stopwords_e_palavras_curtas() {
        let tokens = content_tokens("O token de sessão para o usuário expirou");
        // "para" é stopword; "o", "de", "com" caem no corte de 4 chars.
        assert!(tokens.contains("token"));
        assert!(tokens.contains("sessao"));
        assert!(tokens.contains("usuario"));
        assert!(tokens.contains("expirou"));
        assert!(!tokens.contains("para"));
        assert!(!tokens.contains("de"));
        // "com" tem 3 chars e já cai no corte de tamanho.
        assert!(!tokens.contains("com"));
    }

    #[test]
    fn file_basenames_aceita_caminho_completo_e_relativo() {
        // Caminho absoluto Windows e relativo Unix batem no mesmo basename.
        let previstos = file_basenames("C:\\proj\\src\\Api\\Auth\\TokenService.cs");
        assert_eq!(previstos, vec!["tokenservice.cs"]);
        let previstos = file_basenames("src/Api/Auth/TokenService.cs");
        assert_eq!(previstos, vec!["tokenservice.cs"]);
        // Vários arquivos numa linha, separados por vírgula.
        let previstos = file_basenames("src/a/session.rs, src/b/token.rs");
        assert_eq!(previstos, vec!["session.rs", "token.rs"]);
    }

    #[test]
    fn files_ok_bate_basename_do_gold_contra_previsao() {
        // Gold com caminho completo, previsão com basename → acerto.
        assert!(files_ok(
            "src/Api/Auth/TokenService.cs",
            &["src\\Auth\\TokenService.cs".to_string()]
        ));
        // Previsão com caminho completo, gold com basename → acerto.
        assert!(files_ok(
            "C:\\x\\src\\auth\\session.rs",
            &["session.rs".to_string()]
        ));
        // Nenhum basename em comum → erro.
        assert!(!files_ok(
            "src/auth/session.rs",
            &["src/db/repositories.cs".to_string()]
        ));
        // Placeholders do prompt nunca acertam.
        assert!(!files_ok("—", &["session.rs".to_string()]));
        assert!(!files_ok("não identificado", &["session.rs".to_string()]));
        // Gold sem arquivo → nada acerta (não há como acertar sem rótulo).
        assert!(!files_ok("src/auth/session.rs", &[]));
    }

    #[test]
    fn confianca_normalizada_aceita_o_vocabulario_do_prompt() {
        assert_eq!(confianca_normalizada("alta").as_deref(), Some("alta"));
        assert_eq!(confianca_normalizada("média").as_deref(), Some("media"));
        assert_eq!(confianca_normalizada("MÉDIA").as_deref(), Some("media"));
        assert_eq!(confianca_normalizada("Baixa").as_deref(), Some("baixa"));
        assert_eq!(confianca_normalizada("certeza"), None);
        assert_eq!(confianca_normalizada(""), None);
    }

    #[test]
    fn gate_line_e_render_table_sao_deterministicos() {
        let itens = vec![
            ItemResult {
                id: 529511,
                pasta: "01-hit".to_string(),
                causa_score: 1.0,
                causa_ok: true,
                arquivos_ok: true,
                veredito: "ACERTOU".to_string(),
                causa_prevista: "TTL do token muito curto".to_string(),
                causa_gold: "TTL do token muito curto".to_string(),
                arquivos_previstos: vec!["session.rs".to_string()],
                arquivos_gold: vec!["src/auth/session.rs".to_string()],
                confianca_prevista: "alta".to_string(),
                confianca_gold: Some("alta".to_string()),
            },
            ItemResult {
                id: 529512,
                pasta: "02-miss".to_string(),
                causa_score: 0.0,
                causa_ok: false,
                arquivos_ok: false,
                veredito: "ERROU".to_string(),
                causa_prevista: "—".to_string(),
                causa_gold: "Corrupção de memória no cache".to_string(),
                arquivos_previstos: Vec::new(),
                arquivos_gold: vec!["src/cache/service.cs".to_string()],
                confianca_prevista: "—".to_string(),
                confianca_gold: Some("baixa".to_string()),
            },
        ];
        let gate_result = gate(&itens);
        assert_eq!(gate_result.acertos, 1);
        assert_eq!(gate_result.total, 2);
        assert!((gate_result.pct - 50.0).abs() < f64::EPSILON);
        assert!(!gate_result.passou);
        assert_eq!(
            gate_line(&gate_result),
            "GATE M2: 1/2 = 50% (limiar 70%) — NÃO PASSOU"
        );
        let tabela = render_table(&itens, &gate_result);
        // O espaçamento das colunas é alinhado à direita; a comparação aqui
        // normaliza espaços para não depender da largura dos ids de teste.
        let aperta = |texto: &str| texto.split_whitespace().collect::<Vec<&str>>().join(" ");
        let linhas_apertadas: Vec<String> = tabela.lines().map(&aperta).collect();
        assert!(
            linhas_apertadas.contains(&"529511 | 100% | sim | ACERTOU".to_string()),
            "{tabela}"
        );
        assert!(
            linhas_apertadas.contains(&"529512 | 0% | não | ERROU".to_string()),
            "{tabela}"
        );
        assert!(
            tabela.ends_with("GATE M2: 1/2 = 50% (limiar 70%) — NÃO PASSOU\n"),
            "{tabela}"
        );
    }
}
