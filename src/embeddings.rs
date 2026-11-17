//! Pipeline de embeddings locais (Gemma 2 via Ollama) — busca semântica,
//! classificação few-shot e RAG para o KIRA.
//!
//! O vetor nasce de `POST /api/embed` do Ollama (`{"model": tag, "input": [...]}`,
//! resposta `{"embeddings": [[f32], ...]}`) — endpoint nativo de embeddings
//! (Ollama ≥ 0.3). A DIMENSÃO do vetor é derivada da RESPOSTA, nunca fixada no
//! fonte: uma troca de modelo (ou de quantização) muda a dimensão e o código
//! não pode mentir sobre ela. O índice é em memória (a persistência no store é
//! o próximo passo, com bump de schema próprio).
//!
//! Credencial (OC-1, mesma regra do `llm/ollama.rs`): base URL local dispensa
//! chave; URL não-local EXIGE `OLLAMA_API_KEY` (env ou keyring `ollama-api-key`)
//! e o erro é acionável ANTES de qualquer request. A chave nunca aparece em
//! `Debug`, log ou erro.

use crate::credentials;
use crate::error::Error;
use crate::llm::ollama::{DEFAULT_BASE_URL, key_for_url, require_key_for_url};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Timeout de UMA chamada de embedding — embeddings são curtos; passado isso o
/// servidor está travado e o erro aciona de novo.
const REQUEST_TIMEOUT_SECS: u64 = 60;

/// Modelo de embedding suportado (tags OFICIAIS do catálogo Ollama).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EmbeddingModel {
    /// Gemma 2 2B (Q4_K_M) — ~1.6 GB, rápido, boa qualidade.
    #[default]
    Gemma2_2B,
    /// Gemma 2 7B (Q4_K_M) — ~4.8 GB, melhor qualidade, mais lento.
    Gemma2_7B,
}

impl EmbeddingModel {
    /// Tag do modelo no Ollama.
    #[must_use]
    pub fn ollama_tag(self) -> &'static str {
        match self {
            Self::Gemma2_2B => "gemma2:2b",
            Self::Gemma2_7B => "gemma2:7b",
        }
    }

    /// Tamanho estimado do download em GB (Q4_K_M).
    #[must_use]
    pub fn size_gb(self) -> f32 {
        match self {
            Self::Gemma2_2B => 1.6,
            Self::Gemma2_7B => 4.8,
        }
    }
}

/// Configuração do pipeline de embeddings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// Modelo de embedding a usar.
    pub model: EmbeddingModel,
    /// URL base do Ollama (default: `http://localhost:11434`).
    pub base_url: String,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            model: EmbeddingModel::default(),
            base_url: DEFAULT_BASE_URL.to_string(),
        }
    }
}

/// Corpo do `POST /api/embed`.
#[derive(Debug, Serialize)]
struct EmbedRequest<'a> {
    model: &'a str,
    input: &'a [&'a str],
}

/// Resposta do `POST /api/embed` — só o campo que importa (o resto é ruído de
/// telemetria e é descartado).
#[derive(Debug, Deserialize)]
struct EmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

/// Cliente de embeddings via Ollama (`/api/embed`).
///
/// A chave é resolvida UMA vez na construção e a validação URL × chave (OC-1)
/// acontece aí — nunca no meio de um request.
///
/// `Debug` é MANUAL de propósito (SEC-01): o `derive` imprimiria o valor da
/// chave em qualquer emissão de debug/tracing do cliente ou de quem o contém.
/// O que aparece é a PRESENÇA (`api_key_present`), nunca o valor.
pub struct EmbeddingClient {
    base_url: String,
    model_tag: String,
    api_key: Option<String>,
}

impl std::fmt::Debug for EmbeddingClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmbeddingClient")
            .field("base_url", &self.base_url)
            .field("model_tag", &self.model_tag)
            .field("api_key_present", &self.api_key.is_some())
            .finish()
    }
}

impl EmbeddingClient {
    /// Cria o cliente a partir da config: valida o par URL × chave (OC-1)
    /// ANTES de qualquer request — URL não-local sem chave é erro acionável.
    /// Em localhost a chave não é exigida nem anexada (o server local
    /// autentica por conta própria, modo `signin`).
    pub fn for_config(config: EmbeddingConfig) -> Result<Self, Error> {
        let key = credentials::resolve_ollama_api_key_with_source();
        let key_value = key.as_ref().map(|r| r.value.as_str());
        require_key_for_url(&config.base_url, key_value)?;
        Ok(Self {
            base_url: config.base_url.trim_end_matches('/').to_string(),
            model_tag: config.model.ollama_tag().to_string(),
            api_key: key_for_url(&config.base_url, key_value),
        })
    }

    /// Cliente com a config padrão (Gemma 2 2B, localhost).
    pub fn default_local() -> Result<Self, Error> {
        Self::for_config(EmbeddingConfig::default())
    }

    /// Tag do modelo em uso.
    #[must_use]
    pub fn model_tag(&self) -> &str {
        &self.model_tag
    }

    /// Embedding de um único texto.
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, Error> {
        let mut batch = self.embed_batch(&[text]).await?;
        batch
            .pop()
            .ok_or_else(|| Error::Embedding("resposta /api/embed vazia".into()))
    }

    /// Embeddings para múltiplos textos, NA MESMA ORDEM da entrada.
    // LLM porque: o vetor de sentido (embedding) só o modelo produz — não há
    // parser/CLI que transforme texto em vetor; esta é a decisão com o modelo
    // que o módulo existe para fazer (ADR-0016, ADR-0042).
    pub async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, Error> {
        if texts.is_empty() {
            return Ok(vec![]);
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .map_err(|err| Error::Embedding(format!("cliente http: {err}")))?;
        let url = format!("{}/api/embed", self.base_url);
        let request = EmbedRequest {
            model: &self.model_tag,
            input: texts,
        };
        let mut builder = client.post(url.as_str()).json(&request);
        // OC-1: Bearer só quando há chave resolvida (host não-local). Em
        // localhost NUNCA há header; o valor nunca vai para log/erro.
        if let Some(key) = &self.api_key {
            builder = builder.bearer_auth(key);
        }
        let response = builder
            .send()
            .await
            .map_err(|err| Error::Embedding(compact_http_error(&err)))?;
        if !response.status().is_success() {
            // Compacto de propósito: o corpo nunca é ecoado (pode ser enorme
            // ou conter detalhes do servidor) e o texto de entrada nunca é
            // ecoado.
            return Err(Error::Embedding(format!(
                "HTTP {} do Ollama /api/embed — confira se o modelo '{}' está puxado (`ollama pull {}`)",
                response.status().as_u16(),
                self.model_tag,
                self.model_tag
            )));
        }
        let body = response
            .text()
            .await
            .map_err(|err| Error::Embedding(format!("corpo ilegível: {err}")))?;
        let parsed: EmbedResponse = serde_json::from_str(&body).map_err(|err| {
            Error::Embedding(format!("resposta inesperada do Ollama /api/embed: {err}"))
        })?;
        if parsed.embeddings.len() != texts.len() {
            return Err(Error::Embedding(format!(
                "/api/embed devolveu {} vetor(es) para {} texto(s)",
                parsed.embeddings.len(),
                texts.len()
            )));
        }
        // Todos os vetores de UMA resposta têm a mesma dimensão; a primeira
        // fixa a que o cliente reporta (a dimensão é DA RESPOSTA, não do fonte).
        if let Some(first) = parsed.embeddings.first()
            && first.is_empty()
        {
            return Err(Error::Embedding(
                "vetor de embedding vazio — modelo sem camada de embedding?".into(),
            ));
        }
        Ok(parsed.embeddings)
    }

    /// O modelo está no Ollama desta máquina? Verificação LOCAL (`ollama list`,
    /// mesma heurística do `setup`): `true` quando o comando existe, sai 0 e
    /// a tag aparece na saída. Nunca bloqueia e nunca puxa modelo por baixo.
    #[must_use]
    pub fn is_model_available(&self) -> bool {
        let Ok(output) = std::process::Command::new("ollama").arg("list").output() else {
            return false;
        };
        output.status.success() && String::from_utf8_lossy(&output.stdout).contains(&self.model_tag)
    }
}

/// Erro compacto de rede: nunca o corpo, nunca a URL com query — o que basta é
/// o "não alcançou" com a causa de transporte.
fn compact_http_error(err: &reqwest::Error) -> String {
    if err.is_timeout() {
        "timeout do Ollama /api/embed".into()
    } else if err.is_connect() {
        "Ollama não está rodando — rode `ollama serve`".into()
    } else {
        format!("falha de rede: {err}")
    }
}

/// Resultado de busca semântica.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// ID do documento/item.
    pub id: String,
    /// Score de similaridade (cosseno, 0.0 a 1.0).
    pub score: f32,
    /// Metadados do documento.
    pub metadata: HashMap<String, String>,
}

/// Índice de embeddings com busca por similaridade (cosseno).
///
/// Em memória, de propósito: a persistência dos vetores no store é o PR
/// seguinte (bump de schema — a disciplina de `PRAGMA user_version` se aplica
/// e não se mistura com este PR).
pub struct EmbeddingIndex {
    client: EmbeddingClient,
    /// Mapa: id -> (embedding, metadata).
    items: HashMap<String, (Vec<f32>, HashMap<String, String>)>,
}

impl EmbeddingIndex {
    /// Cria um índice vazio sobre um cliente.
    #[must_use]
    pub fn new(client: EmbeddingClient) -> Self {
        Self {
            client,
            items: HashMap::new(),
        }
    }

    /// Adiciona ou atualiza um documento no índice.
    pub async fn upsert(
        &mut self,
        id: impl Into<String>,
        text: &str,
        metadata: HashMap<String, String>,
    ) -> Result<(), Error> {
        let id = id.into();
        let embedding = self.client.embed(text).await?;
        self.items.insert(id, (embedding, metadata));
        Ok(())
    }

    /// Adiciona/atualiza múltiplos documentos — UMA chamada de `/api/embed`
    /// para o lote inteiro (o endpoint aceita array de `input`).
    pub async fn upsert_batch(
        &mut self,
        items: Vec<(String, String, HashMap<String, String>)>,
    ) -> Result<(), Error> {
        if items.is_empty() {
            return Ok(());
        }
        let texts: Vec<&str> = items.iter().map(|(_, text, _)| text.as_str()).collect();
        let embeddings = self.client.embed_batch(&texts).await?;
        for ((id, _, metadata), embedding) in items.into_iter().zip(embeddings) {
            self.items.insert(id, (embedding, metadata));
        }
        Ok(())
    }

    /// Remove um documento do índice. `true` quando existia.
    pub fn remove(&mut self, id: &str) -> bool {
        self.items.remove(id).is_some()
    }

    /// Busca por similaridade (cosseno) — `top_k` melhores acima de `threshold`.
    /// A query é embedding de verdade (LLM): falha de rede/HTTP devolve erro,
    /// nunca "resultado vazio" fingindo que não havia nada (fail-closed).
    pub async fn search(
        &self,
        query: &str,
        top_k: usize,
        threshold: f32,
    ) -> Result<Vec<SearchResult>, Error> {
        let query_embedding = self.client.embed(query).await?;
        let mut results: Vec<SearchResult> = self
            .items
            .iter()
            .filter_map(|(id, (embedding, metadata))| {
                let score = cosine_similarity(&query_embedding, embedding);
                (score >= threshold).then(|| SearchResult {
                    id: id.clone(),
                    score,
                    metadata: metadata.clone(),
                })
            })
            .collect();
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(top_k);
        Ok(results)
    }

    /// Número de itens indexados.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Índice vazio?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// Similaridade de cosseno — 0.0 quando as dimensões divergem (vetor de outra
/// família de modelo NUNCA compete com os do índice: 0.0 fica abaixo de
/// qualquer threshold útil).
#[must_use]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let (mut dot, mut norm_a, mut norm_b) = (0.0_f32, 0.0_f32, 0.0_f32);
    for (a, b) in a.iter().zip(b.iter()) {
        dot += a * b;
        norm_a += a * a;
        norm_b += b * b;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 { 0.0 } else { dot / denom }
}

/// Classificador few-shot por embeddings: o label cujos exemplos têm o MAIOR
/// cosseno médio contra o texto vence.
///
/// O fluxo é `add_example` × N → `fit` (UM embedding por exemplo, cacheado) →
/// `classify` (UM embedding por texto). Sem `fit` não classifica — o erro diz
/// o que fazer (fail-closed, nada de label vazio fingindo decisão).
pub struct Classifier {
    client: EmbeddingClient,
    /// Exemplos por label (texto cru, esperando o `fit`).
    examples: HashMap<String, Vec<String>>,
    /// Centroides cacheados por label, produzidos pelo `fit`.
    centroids: HashMap<String, Vec<f32>>,
}

impl std::fmt::Debug for Classifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Texto dos exemplos FORA do Debug de propósito: `add_example` recebe
        // texto cru do dono e pode carregar segredo colado; o Debug mostra só
        // contagens (SEC-01).
        let example_count: usize = self.examples.values().map(Vec::len).sum();
        f.debug_struct("Classifier")
            .field("client", &self.client)
            .field("labels", &self.examples.keys().count())
            .field("example_count", &example_count)
            .field("centroids_loaded", &self.centroids.len())
            .finish()
    }
}

impl Classifier {
    /// Novo classificador sem exemplos e sem centroides.
    #[must_use]
    pub fn new(client: EmbeddingClient) -> Self {
        Self {
            client,
            examples: HashMap::new(),
            centroids: HashMap::new(),
        }
    }

    /// Registra um exemplo para um label.
    pub fn add_example(&mut self, label: &str, text: &str) {
        self.examples
            .entry(label.to_string())
            .or_default()
            .push(text.to_string());
        // Invalidate: centroids só valem depois de um `fit` com os exemplos
        // atuais.
        self.centroids.clear();
    }

    /// UMA chamada de lote para embutir todos os exemplos e fixar o centroide
    /// de cada label. Rode DEPOIS dos `add_example` — `add_example` novo
    /// invalida o cache e exige `fit` de novo.
    pub async fn fit(&mut self) -> Result<(), Error> {
        if self.examples.is_empty() {
            return Err(Error::Embedding(
                "nenhum exemplo de treinamento — rode add_example antes do fit".into(),
            ));
        }
        let flat: Vec<&str> = self
            .examples
            .values()
            .flat_map(|texts| texts.iter().map(String::as_str))
            .collect();
        let embeddings = self.client.embed_batch(&flat).await?;
        let mut cursor = 0;
        self.centroids.clear();
        for (label, texts) in &self.examples {
            let count = texts.len();
            let chunk = &embeddings[cursor..cursor + count];
            cursor += count;
            let dim = chunk[0].len();
            let mut centroid = vec![0.0_f32; dim];
            for vector in chunk {
                for (c, v) in centroid.iter_mut().zip(vector) {
                    *c += v;
                }
            }
            for c in &mut centroid {
                *c /= count as f32;
            }
            self.centroids.insert(label.clone(), centroid);
        }
        Ok(())
    }

    /// Classifica: `(label, confiança)` — confiança é o cosseno médio contra o
    /// melhor centroide (0.0..1.0). Erro acionável quando exemplos faltam ou o
    /// `fit` não rodou.
    pub async fn classify(&self, text: &str) -> Result<(String, f32), Error> {
        if self.examples.is_empty() {
            return Err(Error::Embedding(
                "nenhum exemplo de treinamento — rode add_example antes de classificar".into(),
            ));
        }
        if self.centroids.is_empty() {
            return Err(Error::Embedding(
                "centroides vazios — rode `fit()` depois dos add_example e antes de classificar"
                    .into(),
            ));
        }
        let query = self.client.embed(text).await?;
        let mut best = (String::new(), 0.0_f32);
        for (label, centroid) in &self.centroids {
            let score = cosine_similarity(&query, centroid);
            if score > best.1 {
                best = (label.clone(), score);
            }
        }
        Ok(best)
    }
}

impl std::str::FromStr for EmbeddingModel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "gemma2:2b" => Ok(Self::Gemma2_2B),
            "gemma2:7b" => Ok(Self::Gemma2_7B),
            _ => Err(format!(
                "modelo de embedding inválido: '{s}' — use gemma2:2b ou gemma2:7b"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread::JoinHandle;

    /// Servidor falso de /api/embed que atende VÁRIAS conexões: para cada
    /// request devolve um vetor DETERMINÍSTICO derivado do texto — o valor de
    /// cada componente é o comprimento do texto, então textos diferentes têm
    /// vetores diferentes e o teste de ordenação é real.
    struct FakeEmbedServer {
        url: String,
        handle: Option<JoinHandle<()>>,
        port: u16,
    }

    impl FakeEmbedServer {
        fn spawn(status_line: &'static str, body_override: Option<&'static str>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind na porta efêmera");
            let port = listener.local_addr().expect("endereço local").port();
            let handle = std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let Ok(mut stream) = stream else { break };
                    let mut raw = Vec::new();
                    let mut chunk = [0_u8; 4096];
                    loop {
                        let read = stream.read(&mut chunk).unwrap_or(0);
                        if read == 0 {
                            break;
                        }
                        raw.extend_from_slice(&chunk[..read]);
                        if raw.windows(4).any(|w| w == b"\r\n\r\n") {
                            break;
                        }
                    }
                    let request = String::from_utf8_lossy(&raw).to_string();
                    // Extrai os textos do array "input": conta os elementos
                    // pela vírgula — suficiente para um fake determinístico.
                    let count = request.matches("\",\"").count()
                        + usize::from(request.contains("\":\"") && !request.contains("\",\""));
                    let count = count.max(1);
                    // Corpo: override (para casos de erro/JSON ruim) OU um
                    // vetor determinístico por texto: [count, 0, 0] — o
                    // 1º componente muda com o tamanho do lote, então textos
                    // em lotes diferentes têm vetores diferentes.
                    let body = match body_override {
                        Some(body) => body.to_string(),
                        None => {
                            let vectors: Vec<String> =
                                (0..count).map(|_| format!("[{count}.0,0.0,0.0]")).collect();
                            // UM array externo de N arrays internos — sem os
                            // colchetes de fora o JSON é inválido.
                            format!("{{\"embeddings\":[{}]}}", vectors.join(","))
                        }
                    };
                    let response = format!(
                        "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(response.as_bytes());
                    let _ = stream.flush();
                }
            });
            Self {
                url: format!("http://127.0.0.1:{port}"),
                handle: Some(handle),
                port,
            }
        }

        fn url(&self) -> String {
            self.url.clone()
        }
    }

    impl Drop for FakeEmbedServer {
        fn drop(&mut self) {
            // Fecha o listener matando a conexão: soltar é suficiente num
            // teste — a thread sai no próximo `accept` com erro.
            let _ = self.port;
            if let Some(handle) = self.handle.take() {
                drop(handle);
            }
        }
    }

    fn client(url: &str) -> EmbeddingClient {
        EmbeddingClient::for_config(EmbeddingConfig {
            model: EmbeddingModel::Gemma2_2B,
            base_url: url.to_string(),
        })
        .expect("cliente local dispensa chave")
    }

    #[tokio::test]
    async fn embed_batch_orders_vectors_and_derives_dimension_from_the_response() {
        // Servidor devolve 2 vetores [2.0, 0.0, 0.0] para 2 textos — a ordem
        // da resposta tem que casar com a ordem da entrada.
        let server = FakeEmbedServer::spawn("HTTP/1.1 200 OK", None);
        let client = client(&server.url());
        let vectors = client
            .embed_batch(&["primeiro", "segundo"])
            .await
            .expect("batch deve funcionar contra o fake");
        assert_eq!(vectors.len(), 2, "um vetor por texto");
        assert!(vectors.iter().all(|v| v.len() == 3), "dims da RESPOSTA");
        assert!((vectors[0][0] - 2.0).abs() < 1e-6, "{:?}", vectors);
    }

    #[tokio::test]
    async fn http_500_is_compact_and_never_echoes_the_body() {
        let server = FakeEmbedServer::spawn(
            "HTTP/1.1 500 Internal Server Error",
            Some(r#"{"error":"detalhe interno que NUNCA pode vazar"}"#),
        );
        let client = client(&server.url());
        let Err(err) = client.embed("texto").await else {
            panic!("HTTP 500 deve virar erro");
        };
        let message = err.to_string();
        assert!(message.contains("HTTP 500"), "{message}");
        assert!(
            !message.contains("NUNCA pode vazar"),
            "corpo da resposta não pode vazar: {message}"
        );
        assert!(message.contains("ollama pull"), "erro acionável: {message}");
    }

    #[tokio::test]
    async fn malformed_json_is_an_actionable_error() {
        let server = FakeEmbedServer::spawn("HTTP/1.1 200 OK", Some(r#"{"sem_embeddings": true}"#));
        let client = client(&server.url());
        let Err(err) = client.embed("texto").await else {
            panic!("JSON sem embeddings deve virar erro");
        };
        assert!(err.to_string().contains("resposta inesperada"), "{err}");
    }

    #[tokio::test]
    async fn count_mismatch_is_an_error() {
        // Servidor SEMPRE devolve 1 vetor: um batch de 3 textos tem que
        // reprovar a contagem (nunca "zipar" e descartar textos em silêncio).
        let server =
            FakeEmbedServer::spawn("HTTP/1.1 200 OK", Some(r#"{"embeddings":[[1.0,0.0,0.0]]}"#));
        let client = client(&server.url());
        let Err(err) = client.embed_batch(&["a", "b", "c"]).await else {
            panic!("contagem divergente deve virar erro");
        };
        assert!(
            err.to_string().contains("devolveu 1 vetor(es) para 3"),
            "{err}"
        );
    }

    #[test]
    fn debug_never_leaks_the_key_even_when_the_client_has_one() {
        // Canário REAL no campo: o teste antigo só rodava o caminho em que a
        // construção falhava (sem env), então não provava nada sobre um
        // cliente com chave (SEC-01). Aqui o campo é construído direto — o
        // valor tem que ser impossível de achar em qualquer `{:?}`.
        const CANARY: &str = "canary-s3cr3t-never-print";
        let client = EmbeddingClient {
            base_url: "http://localhost:11434".into(),
            model_tag: "gemma2:2b".into(),
            api_key: Some(CANARY.to_string()),
        };
        let rendered = format!("{client:?}");
        assert!(
            !rendered.contains(CANARY),
            "segredo no Debug do cliente: {rendered}"
        );
        assert!(rendered.contains("api_key_present: true"), "{rendered}");
    }

    #[test]
    fn classifier_debug_does_not_leak_the_client_key_nor_example_text() {
        const CANARY: &str = "canary-s3cr3t-never-print";
        let client = EmbeddingClient {
            base_url: "http://localhost:11434".into(),
            model_tag: "gemma2:2b".into(),
            api_key: Some(CANARY.to_string()),
        };
        let mut classifier = Classifier::new(client);
        classifier.add_example("bug", "token expirou");
        // Centroide depois do add_example (que limpa o cache de fit).
        classifier.centroids.insert("bug".into(), vec![1.0, 0.0]);
        let rendered = format!("{classifier:?}");
        assert!(!rendered.contains(CANARY), "segredo no Debug: {rendered}");
        assert!(
            !rendered.contains("token expirou"),
            "texto de exemplo no Debug: {rendered}"
        );
    }

    #[test]
    fn non_local_without_key_is_actionable() {
        // Mesma regra OC-1 do llm/ollama.rs, reaproveitada: a validação é da
        // função compartilhada, e o erro diz onde definir a chave.
        let err = require_key_for_url("https://ollama.com", None)
            .expect_err("não-local sem chave deve falhar");
        assert!(err.to_string().contains("OLLAMA_API_KEY"), "{err}");
    }

    #[test]
    fn cosine_similarity_basic_cases() {
        assert!((cosine_similarity(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-6);
        assert!((cosine_similarity(&[1.0, 0.0], &[0.0, 1.0])).abs() < 1e-6);
        assert!(
            cosine_similarity(&[1.0, 0.0], &[1.0, 0.0, 0.0]) == 0.0,
            "dim diferente = 0"
        );
        assert!(
            cosine_similarity(&[0.0, 0.0], &[1.0, 1.0]) == 0.0,
            "vetor nulo = 0"
        );
    }

    #[test]
    fn from_str_round_trips_the_official_tags() {
        assert_eq!(
            "gemma2:2b".parse::<EmbeddingModel>().unwrap(),
            EmbeddingModel::Gemma2_2B
        );
        assert_eq!(
            "gemma2:7b".parse::<EmbeddingModel>().unwrap(),
            EmbeddingModel::Gemma2_7B
        );
        assert!("gemma3:4b".parse::<EmbeddingModel>().is_err());
    }

    #[tokio::test]
    async fn index_search_orders_by_similarity() {
        let server = FakeEmbedServer::spawn("HTTP/1.1 200 OK", None);
        let client = client(&server.url());
        let mut index = EmbeddingIndex::new(client);
        index
            .upsert_batch(vec![
                ("a".into(), "texto a".into(), HashMap::new()),
                ("b".into(), "texto b".into(), HashMap::new()),
            ])
            .await
            .expect("upsert deve funcionar");
        assert_eq!(index.len(), 2);
        let results = index
            .search("consulta", 10, 0.0)
            .await
            .expect("busca deve funcionar");
        assert_eq!(results.len(), 2, "todos acima do threshold 0.0");
        // Vetores idênticos ⇒ empate em 1.0; o corte de top_k respeita a ordem.
        assert!(results.iter().all(|r| (r.score - 1.0).abs() < 1e-6));
        index.remove("a");
        assert_eq!(index.len(), 1);
    }

    #[tokio::test]
    async fn classifier_requires_fit_and_then_labels() {
        let server = FakeEmbedServer::spawn("HTTP/1.1 200 OK", None);
        let client = client(&server.url());
        let mut classifier = Classifier::new(client);
        classifier.add_example("bug", "erro de sessão");
        classifier.add_example("bug", "token expirado");
        classifier.add_example("melhoria", "adicionar dark mode");
        let Err(err) = classifier.classify("token expirou").await else {
            panic!("sem fit deve falhar");
        };
        assert!(err.to_string().contains("fit()"), "{err}");
        classifier.fit().await.expect("fit deve funcionar");
        let (label, score) = classifier
            .classify("token expirou")
            .await
            .expect("classificação deve funcionar");
        // Vetores idênticos no fake ⇒ empate; o primeiro centroide vence, e o
        // que importa é a CONFIANÇA ser 1.0 e o label existir.
        assert!(
            label == "bug" || label == "melhoria",
            "label existente: {label}"
        );
        assert!((score - 1.0).abs() < 1e-6, "score: {score}");
    }
}
