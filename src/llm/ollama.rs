//! Ollama backend: a local, offline LLM server (`http://localhost:11434`).
//!
//! Zero cloud, localhost only. The base URL comes from `KIRA_OLLAMA_URL`
//! (default `http://localhost:11434`) and the model from `KIRA_MODEL` or the
//! `--model` flag (default `qwen3.5:4b`).

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::llm::LlmBackend;

/// Default model: official Ollama tag `qwen3.5:4b` (Q4_K_M, ~3.3 GB — fits a
/// 6 GB RTX 2060 entirely in GPU memory).
pub const DEFAULT_MODEL: &str = "qwen3.5:4b";

/// Default base URL of the local Ollama server.
pub const DEFAULT_BASE_URL: &str = "http://localhost:11434";

/// Conservative context window for a 6 GB GPU.
pub const DEFAULT_NUM_CTX: u32 = 4096;

/// Low temperature: triage must be factual, not creative.
pub const DEFAULT_TEMPERATURE: f32 = 0.2;

/// Hard ceiling for one completion; past this the model is stuck.
pub const REQUEST_TIMEOUT_SECS: u64 = 120;

/// Reads the Ollama base URL from `KIRA_OLLAMA_URL`, falling back to
/// [`DEFAULT_BASE_URL`].
pub fn base_url_from_env() -> String {
    env_or_default("KIRA_OLLAMA_URL", DEFAULT_BASE_URL)
}

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

/// Backend that talks to a local Ollama server over `POST /api/chat`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OllamaBackend {
    base_url: String,
    model: String,
}

impl OllamaBackend {
    /// Creates a backend for the given base URL (trailing `/` is trimmed) and
    /// model tag.
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            model: model.into(),
        }
    }
}

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: [Message<'a>; 2],
    stream: bool,
    options: Options,
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
        };
        let response = client.post(url.as_str()).json(&request).send().await;
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
}
