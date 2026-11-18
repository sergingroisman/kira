//! Ollama backend: local by default (`http://localhost:11434`), with explicit
//! authentication for non-local hosts (OC-1 — Ollama Cloud direto).
//!
//! The base URL comes from `KIRA_OLLAMA_URL` (default `http://localhost:11434`)
//! and the model from `KIRA_MODEL` or the `--model` flag (default
//! `qwen3.5:4b`). A non-local URL is ALLOWED (Ollama Cloud direct API) but
//! REQUIRES a key (`OLLAMA_API_KEY` env or keyring entry `ollama-api-key`):
//! an anonymous request would fail in a confusing way, so the pair URL × key
//! is validated at construction — never mid-request. The key never appears in
//! `Debug`, logs or errors.

use std::fmt;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::credentials;
use crate::endpoint::is_local_url;
use crate::error::Error;
use crate::llm::LlmBackend;

/// Default model: official Ollama tag `qwen3.5:4b` (Q4_K_M, ~3.3 GB — fits a
/// 6 GB RTX 2060 entirely in GPU memory).
pub const DEFAULT_MODEL: &str = "qwen3.5:4b";

/// Default base URL of the local Ollama server — fonte única em `endpoint`
/// (o `doctor` lê sem importar a camada LLM).
pub use crate::endpoint::DEFAULT_BASE_URL;

/// Conservative context window for a 6 GB GPU.
pub const DEFAULT_NUM_CTX: u32 = 4096;

/// Low temperature: triage must be factual, not creative.
pub const DEFAULT_TEMPERATURE: f32 = 0.2;

/// Hard ceiling for one completion; past this the model is stuck.
pub const REQUEST_TIMEOUT_SECS: u64 = 120;

/// Reads the Ollama base URL from `KIRA_OLLAMA_URL`, falling back to
/// [`crate::endpoint::DEFAULT_BASE_URL`] — a única fonte do nome de env e do
/// default é o módulo de endpoint (o `doctor` usa o mesmo sem importar `llm`).
pub use crate::endpoint::base_url_from_env;

/// Reads the model tag from `KIRA_MODEL`, falling back to [`DEFAULT_MODEL`].
pub fn model_from_env() -> String {
    env_or_default("KIRA_MODEL", DEFAULT_MODEL)
}

/// Reads the context window from `KIRA_NUM_CTX` — re-exported from
/// [`crate::config`] so the knob has one owner (and the context packer can size
/// the prompt budget without importing this backend's module).
pub use crate::config::num_ctx_from_env;

fn env_or_default(name: &str, default: &str) -> String {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

/// Backend that talks to an Ollama server over `POST /api/chat` — local by
/// default, Bearer-authenticated when the URL is non-local (OC-1).
pub struct OllamaBackend {
    base_url: String,
    model: String,
    /// Chave da API, resolvida por [`Self::for_base_url`] — `None` em
    /// localhost (comportamento idêntico ao de hoje: sem header).
    api_key: Option<String>,
}

impl fmt::Debug for OllamaBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // A chave NUNCA aparece no Debug (OC-1): presente/ausente basta —
        // qualquer `println!("{backend:?}")` de debug futuro não pode vazar.
        let key = self.api_key.as_ref().map(|_| "<redacted>");
        f.debug_struct("OllamaBackend")
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("api_key", &key)
            .finish()
    }
}

impl Clone for OllamaBackend {
    fn clone(&self) -> Self {
        // Clone manual: o derive copiaria o valor da chave para um novo
        // campo sem o controle do Debug (o clone herda a máscara acima).
        Self {
            base_url: self.base_url.clone(),
            model: self.model.clone(),
            api_key: self.api_key.clone(),
        }
    }
}

impl PartialEq for OllamaBackend {
    fn eq(&self, other: &Self) -> bool {
        self.base_url == other.base_url && self.model == other.model
    }
}

impl Eq for OllamaBackend {}

impl OllamaBackend {
    /// Creates a backend for the given base URL (trailing `/` is trimmed) and
    /// model tag — sem chave: use [`Self::for_base_url`] nos comandos, que
    /// valida o par URL × chave (OC-1) antes de qualquer request.
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            model: model.into(),
            api_key: None,
        }
    }

    /// Constrói o backend validando o par URL × chave (OC-1): URL NÃO-local
    /// sem chave é erro acionável ANTES de qualquer request — nunca request
    /// anônimo que falha confuso. Em localhost a chave não é exigida nem
    /// anexada (o server local autentica por conta própria, modo `signin`).
    ///
    /// A chave vem da env `OLLAMA_API_KEY` ou do keyring do OS (entrada
    /// `ollama-api-key`); falha de leitura do keyring conta como ausente —
    /// o erro acionável da falta de chave já diz onde definir a env.
    pub fn for_base_url(base_url: &str, model: impl Into<String>) -> Result<Self, Error> {
        let key = credentials::resolve_ollama_api_key_with_source();
        require_key_for_url(base_url, key.as_ref().map(|r| r.value.as_str()))?;
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.into(),
            api_key: key_for_url(base_url, key.as_ref().map(|r| r.value.as_str())),
        })
    }

    /// Clona o backend trocando a tag do modelo — mesma URL, mesma chave. A
    /// fábrica do routing por kind (HF-5) chama por post sem re-resolver a
    /// credencial nem revalidar a URL.
    pub fn with_model(&self, model: impl Into<String>) -> Self {
        let mut clone = self.clone();
        clone.model = model.into();
        clone
    }

    /// Cria o backend com chave EXPLÍCITA SEMPRE anexada (uso de teste): o
    /// fake server dos testes vive em `127.0.0.1` — local por definição —,
    /// então para exercitar o header `Authorization: Bearer` contra ele o
    /// helper ignora a regra de URL (que é coberta por [`key_for_url`]).
    #[cfg(test)]
    pub(crate) fn with_key_for_tests(
        base_url: impl Into<String>,
        model: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Self {
        let mut backend = Self::new(base_url, model);
        backend.api_key = Some(api_key.into());
        backend
    }
}

/// Valida o par URL × chave (OC-1, PURA): URL não-local sem chave é o erro
/// acionável — nunca request anônimo que falha confuso. Local dispensa chave.
/// `pub(crate)` para o cliente de embeddings (`embeddings.rs`) aplicar a
/// MESMA regra sem copiar a lógica (uma regra, um dono).
pub(crate) fn require_key_for_url(base_url: &str, key: Option<&str>) -> Result<(), Error> {
    if is_local_url(base_url) || key.map(|k| !k.trim().is_empty()).unwrap_or(false) {
        return Ok(());
    }
    Err(Error::OllamaKeyMissing {
        url: base_url.to_string(),
    })
}

/// Decide se a chave é anexada ao request (PURA): nunca em localhost — o
/// server local autentica por conta própria (modo `signin`, OC-1 modo (a)),
/// e o comportamento local tem que ficar idêntico ao de hoje (sem header).
/// `pub(crate)`: compartilhada com `embeddings.rs` (mesma regra, um dono).
pub(crate) fn key_for_url(base_url: &str, key: Option<&str>) -> Option<String> {
    if is_local_url(base_url) {
        return None;
    }
    key.map(str::to_string)
}

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: [Message<'a>; 2],
    stream: bool,
    options: Options,
    /// `think` explícito (API do Ollama) — modelos de raciocínio (qwen3.5)
    /// gastam a JANELA INTEIRA com o raciocínio quando o campo fica ausente:
    /// provado no hardware do dono (18/Nov/2026) — 13.891 chars de think
    /// espremeram a resposta final a 73 chars truncados ("# eishoku").
    /// Padrão DESLIGADO: geração grounded em fatos não precisa de cadeia de
    /// pensamento; `KIRA_THINK=on` religa para quem quiser (e sabe que a
    /// janela precisa subir junto).
    think: bool,
}

/// Lê o pensamento da env (PURA): `KIRA_THINK=on|1|true` religa. Default
/// desligado — a janela do modelo é finita e o pensamento não respeita
/// reserva nenhuma (prova acima).
fn think_from_env() -> bool {
    matches!(
        std::env::var("KIRA_THINK").as_deref(),
        Ok("on") | Ok("1") | Ok("true")
    )
}

#[derive(Debug, Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Serialize)]
struct Options {
    num_ctx: u32,
    temperature: f32,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    content: String,
}

impl LlmBackend for OllamaBackend {
    async fn complete(&self, system: &str, prompt: &str) -> Result<String, Error> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .map_err(|err| Error::LlmRequest(err.to_string()))?;
        let url = format!("{}/api/chat", self.base_url);
        let request = ChatRequest {
            model: &self.model,
            messages: [
                Message {
                    role: "system",
                    content: system,
                },
                Message {
                    role: "user",
                    content: prompt,
                },
            ],
            stream: false,
            options: Options {
                num_ctx: num_ctx_from_env(),
                temperature: DEFAULT_TEMPERATURE,
            },
            think: think_from_env(),
        };
        let mut builder = client.post(url.as_str()).json(&request);
        // OC-1: Bearer só quando há chave resolvida (host não-local). Em
        // localhost NUNCA há header — comportamento idêntico ao de hoje, e o
        // valor nunca vai para log/erro: o header sai e o resto é silêncio.
        if let Some(key) = &self.api_key {
            builder = builder.bearer_auth(key);
        }
        let response = builder.send().await;
        let response = match response {
            Ok(response) => response,
            Err(err) if err.is_timeout() => return Err(Error::LlmTimeout),
            Err(err) if is_connection_refused(&err) => return Err(Error::LlmNotRunning),
            Err(err) => return Err(Error::LlmRequest(err.to_string())),
        };
        if !response.status().is_success() {
            // Compact on purpose: the body is never echoed (it may be huge or
            // contain model internals) and the prompt is never echoed.
            return Err(Error::LlmStatus {
                status: response.status().as_u16(),
            });
        }
        let body = response
            .text()
            .await
            .map_err(|err| Error::LlmRequest(err.to_string()))?;
        let parsed: ChatResponse =
            serde_json::from_str(&body).map_err(|err| Error::LlmBadResponse(err.to_string()))?;
        Ok(parsed.message.content)
    }
}

/// True when the request never left the machine: connection refused (nothing
/// listening on the port) or the host could not be reached at all.
fn is_connection_refused(err: &reqwest::Error) -> bool {
    if err.is_connect() {
        return true;
    }
    let text = err.to_string().to_lowercase();
    text.contains("refused") || text.contains("os error 10061")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread::JoinHandle;

    /// Content the fake server returns inside `message.content`.
    const FAKE_CONTENT: &str = "Causa raiz provável:\nTTL do token muito curto.\nArquivos prováveis:\nsrc/auth/session.rs\nConfiança:\nmédia";

    /// JSON body served by the fake server (`\n` inside is a JSON escape, so
    /// the parsed content comes back with real newlines).
    const FAKE_BODY: &str = r#"{"model":"qwen3.5:4b","created_at":"2026-01-01T00:00:00Z","message":{"role":"assistant","content":"Causa raiz provável:\nTTL do token muito curto.\nArquivos prováveis:\nsrc/auth/session.rs\nConfiança:\nmédia"},"done":true}"#;

    fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }

    /// Reads until the full HTTP request (headers + content-length body) is in.
    fn read_http_request(stream: &mut std::net::TcpStream) -> String {
        let mut raw = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let read = stream.read(&mut chunk).unwrap_or(0);
            if read == 0 {
                break;
            }
            raw.extend_from_slice(&chunk[..read]);
            if let Some(headers_end) = find_subsequence(&raw, b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&raw[..headers_end]).to_lowercase();
                let content_length = headers
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length:"))
                    .and_then(|value| value.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                if raw.len() >= headers_end + 4 + content_length {
                    break;
                }
            }
        }
        String::from_utf8_lossy(&raw).to_string()
    }

    /// Spawns a one-shot fake Ollama on an ephemeral port. Returns the base
    /// URL and a handle resolving to the captured raw HTTP request.
    fn spawn_fake_ollama(
        status_line: &'static str,
        body: &'static str,
    ) -> (String, JoinHandle<Option<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .unwrap_or_else(|err| panic!("bind na porta efêmera: {err}"));
        let addr = listener
            .local_addr()
            .unwrap_or_else(|err| panic!("endereço local: {err}"));
        let handle = std::thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return None;
            };
            let request = read_http_request(&mut stream);
            let response = format!(
                "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            if stream.write_all(response.as_bytes()).is_err() {
                return None;
            }
            let _ = stream.flush();
            Some(request)
        });
        (format!("http://{addr}"), handle)
    }

    #[tokio::test]
    async fn complete_posts_chat_request_and_extracts_content() {
        let (base_url, server) = spawn_fake_ollama("HTTP/1.1 200 OK", FAKE_BODY);
        let backend = OllamaBackend::new(base_url, "qwen3.5:4b");
        let content = backend
            .complete("sistema fixo", "prompt de teste")
            .await
            .unwrap_or_else(|err| panic!("complete deve funcionar contra o servidor falso: {err}"));
        assert_eq!(content, FAKE_CONTENT);
        let request = server
            .join()
            .unwrap_or_else(|_| panic!("thread do servidor falso entrou em pânico"));
        let Some(request) = request else {
            panic!("servidor falso deve capturar a requisição");
        };
        assert!(
            request.starts_with("POST /api/chat"),
            "requisição: {request}"
        );
        assert!(request.contains("\"model\":\"qwen3.5:4b\""), "{request}");
        assert!(request.contains("\"role\":\"system\""), "{request}");
        assert!(request.contains("\"role\":\"user\""), "{request}");
        assert!(request.contains("\"stream\":false"), "{request}");
        assert!(request.contains("\"num_ctx\":4096"), "{request}");
        assert!(request.contains("\"temperature\":0.2"), "{request}");
        // O `think` é EXPLÍCITO e desligado por padrão: ausente = o template
        // do modelo decide = qwen3.5 pensa até estourar a janela (prova no
        // hardware do dono, 18/Nov/2026).
        assert!(request.contains("\"think\":false"), "{request}");
    }

    #[tokio::test]
    async fn connection_refused_gives_actionable_message() {
        // Reserve a port just to find a free one, then free it: nothing is
        // listening there anymore.
        let listener = TcpListener::bind("127.0.0.1:0")
            .unwrap_or_else(|err| panic!("bind na porta efêmera: {err}"));
        let addr = listener
            .local_addr()
            .unwrap_or_else(|err| panic!("endereço local: {err}"));
        drop(listener);
        let backend = OllamaBackend::new(format!("http://{addr}"), DEFAULT_MODEL);
        let Err(err) = backend.complete("sistema", "prompt").await else {
            panic!("sem servidor na porta, complete deve falhar");
        };
        let message = err.to_string();
        assert!(
            message.contains("Ollama não está rodando"),
            "mensagem deve ser acionável: {message}"
        );
        assert!(
            message.contains("ollama serve"),
            "mensagem deve dizer como iniciar: {message}"
        );
    }

    #[tokio::test]
    async fn http_error_is_compact_and_never_echoes_the_body() {
        let (base_url, server) = spawn_fake_ollama("HTTP/1.1 500 Internal Server Error", FAKE_BODY);
        let backend = OllamaBackend::new(base_url, DEFAULT_MODEL);
        let Err(err) = backend.complete("sistema", "prompt").await else {
            panic!("HTTP 500 deve virar erro");
        };
        let message = err.to_string();
        assert!(message.contains("HTTP 500"), "mensagem: {message}");
        assert!(
            !message.contains("TTL do token"),
            "corpo da resposta não pode vazar no erro: {message}"
        );
        let _ = server.join();
    }

    #[tokio::test]
    async fn malformed_json_response_is_a_bad_response_error() {
        let (base_url, server) = spawn_fake_ollama("HTTP/1.1 200 OK", r#"{"sem_message": true}"#);
        let backend = OllamaBackend::new(base_url, DEFAULT_MODEL);
        let Err(err) = backend.complete("sistema", "prompt").await else {
            panic!("JSON sem message.content deve virar erro");
        };
        let message = err.to_string();
        assert!(
            message.contains("resposta inesperada do Ollama"),
            "mensagem: {message}"
        );
        let _ = server.join();
    }

    // -------------------------------------------------------------------
    // OC-1 — Ollama Cloud direto: header Bearer, máscara e validação
    // -------------------------------------------------------------------

    /// Chave FALSA de teste (nunca um segredo real: valor literal aqui é
    /// seguro — o contrato é o header chegar e a MÁSCARA valer para chave
    /// verdadeira, provado pelo Debug abaixo).
    const TEST_KEY: &str = "fake-test-key-not-a-secret";

    #[tokio::test]
    async fn non_local_with_key_sends_the_bearer_header() {
        let (base_url, server) = spawn_fake_ollama("HTTP/1.1 200 OK", FAKE_BODY);
        let backend = OllamaBackend::with_key_for_tests(base_url, DEFAULT_MODEL, TEST_KEY);
        let content = backend
            .complete("sistema fixo", "prompt de teste")
            .await
            .unwrap_or_else(|err| panic!("complete com chave deve funcionar: {err}"));
        assert_eq!(content, FAKE_CONTENT);
        let request = server
            .join()
            .unwrap_or_else(|_| panic!("thread do servidor falso entrou em pânico"))
            .expect("requisição capturada");
        assert!(
            request
                .to_lowercase()
                .contains(&format!("authorization: bearer {TEST_KEY}")),
            "o header Bearer tem que chegar ao servidor: {request}"
        );
    }

    #[tokio::test]
    async fn localhost_never_sends_an_authorization_header() {
        // Comportamento local idêntico ao de hoje (OC-1 (c)): o server local
        // autentica por conta própria (modo `signin`) — sem header, sem chave.
        let (base_url, server) = spawn_fake_ollama("HTTP/1.1 200 OK", FAKE_BODY);
        let backend = OllamaBackend::new(base_url, DEFAULT_MODEL);
        backend
            .complete("sistema", "prompt")
            .await
            .unwrap_or_else(|err| panic!("complete local deve funcionar: {err}"));
        let request = server
            .join()
            .unwrap_or_else(|_| panic!("thread do servidor falso entrou em pânico"))
            .expect("requisição capturada");
        assert!(
            !request.to_lowercase().contains("authorization:"),
            "localhost não manda Authorization: {request}"
        );
    }

    #[test]
    fn debug_never_contains_the_key_value() {
        let backend =
            OllamaBackend::with_key_for_tests("https://ollama.com", DEFAULT_MODEL, TEST_KEY);
        let rendered = format!("{backend:?}");
        assert!(
            !rendered.contains(TEST_KEY),
            "Debug NUNCA mostra a chave: {rendered}"
        );
        assert!(
            rendered.contains("<redacted>"),
            "Debug mostra que a chave existe (presente), mascarada: {rendered}"
        );
    }

    #[test]
    fn non_local_without_key_is_an_actionable_error() {
        let err = require_key_for_url("https://ollama.com", None)
            .expect_err("não-local sem chave deve falhar");
        let message = err.to_string();
        assert!(
            message.contains("OLLAMA_API_KEY"),
            "erro deve dizer onde definir a chave: {message}"
        );
        assert!(
            message.contains("localhost"),
            "erro deve oferecer o caminho local: {message}"
        );
        // Com chave (mesma URL): passa — é o caso (a) do OC-1.
        assert!(require_key_for_url("https://ollama.com", Some("qualquer")).is_ok());
        // Chave em branco não conta como presente.
        assert!(
            require_key_for_url("https://ollama.com", Some("   ")).is_err(),
            "chave em branco não autentica"
        );
        // Local dispensa chave: erro nunca acontece (comportamento de hoje).
        assert!(require_key_for_url("http://localhost:11434", None).is_ok());
        assert!(require_key_for_url("http://127.0.0.1:11434", None).is_ok());
    }

    #[test]
    fn key_is_never_attached_for_local_urls() {
        // A regra de anexação é pura e separada da validação: localhost SEMPRE
        // sem header, mesmo com chave resolvida (modo (a) — signin).
        assert!(key_for_url("http://localhost:11434", Some("chave")).is_none());
        assert!(key_for_url("http://[::1]:11434", Some("chave")).is_none());
        assert_eq!(
            key_for_url("https://ollama.com", Some("chave")).as_deref(),
            Some("chave")
        );
        assert!(key_for_url("https://ollama.com", None).is_none());
    }
}
