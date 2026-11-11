//! Foundry backend: Microsoft Foundry via HTTP OpenAI-compatible (BYOK).
//!
//! PR11b / ADR-0020. The Copilot CLI path (ADR-0019) is blocked by SAML
//! policy, so the explicit HTTP path is the way to the expensive model. This
//! backend speaks **two surfaces**, chosen by the shape of the endpoint:
//!
//! - **v1** (endpoint ends in `/v1` — the shape of the owner's resource
//!   `https://<recurso>.openai.azure.com/openai/v1`): `POST
//!   {endpoint}/chat/completions` — no `/openai/deployments/...`, no
//!   `?api-version`; the deployment goes in the `model` JSON field.
//! - **legacy**: any other endpoint becomes `POST
//!   {endpoint}/openai/deployments/{deployment}/chat/completions?api-version=
//!   {KIRA_FOUNDRY_API_VERSION}` (default `2024-10-21`, overridable).
//!
//! A project endpoint (`*/api/projects/*`) is rejected up front with an
//! actionable error: it does not serve chat completions.
//!
//! Auth is `api-key: <chave>` by default (Azure) or `Authorization: Bearer`
//! with `KIRA_FOUNDRY_AUTH=bearer` (OpenAI-compatible gateways). The key
//! never appears in logs, errors, Debug or the trail; server-side error text
//! passes through [`crate::redact::redact`] before it reaches an error.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::llm::LlmBackend;
use crate::llm::ollama::DEFAULT_TEMPERATURE;
use crate::redact::redact;

/// Default `api-version` for the legacy surface (overridable via
/// `KIRA_FOUNDRY_API_VERSION`; a rejection mentioning `api-version` becomes
/// [`Error::FoundryApiVersion`], which says to set it).
pub const DEFAULT_API_VERSION: &str = "2024-10-21";

/// Default `max_tokens` for one completion.
pub const DEFAULT_MAX_TOKENS: u32 = 1024;

/// Default request timeout in seconds (same ceiling as Ollama).
pub const DEFAULT_TIMEOUT_SECS: u64 = 120;

/// Env: resource endpoint (v1 surface ends in `/openai/v1`).
pub const ENV_ENDPOINT: &str = "KIRA_FOUNDRY_ENDPOINT";

/// Env: verbatim request URL (escape hatch for OpenAI-compatible gateways
/// and offline tests; when set, the endpoint is not required).
pub const ENV_URL: &str = "KIRA_FOUNDRY_URL";

/// Env: deployment name — URL segment on the legacy surface, `model` field
/// on the v1 surface.
pub const ENV_DEPLOYMENT: &str = "KIRA_FOUNDRY_DEPLOYMENT";

/// Env: legacy-surface `api-version` override.
pub const ENV_API_VERSION: &str = "KIRA_FOUNDRY_API_VERSION";

/// Env: the resource key (never logged, never echoed).
pub const ENV_API_KEY: &str = "KIRA_FOUNDRY_API_KEY";

/// Env: auth mode — `api-key` (default, Azure) or `bearer` (gateways).
pub const ENV_AUTH: &str = "KIRA_FOUNDRY_AUTH";

/// Env: `max_tokens` override (integer >= 1).
pub const ENV_MAX_TOKENS: &str = "KIRA_FOUNDRY_MAX_TOKENS";

/// Env: request timeout in seconds (integer >= 1).
pub const ENV_TIMEOUT: &str = "KIRA_FOUNDRY_TIMEOUT";

/// Env: body shape override — `auto` (default), `classic` (temperature +
/// `max_tokens`) or `reasoning` (GPT-5 / o-series: no temperature,
/// `max_completion_tokens`).
pub const ENV_BODY_MODE: &str = "KIRA_FOUNDRY_BODY_MODE";

/// Body parameter family. GPT-5 and o-series models reject `temperature`
/// (they only accept the server default) and reject `max_tokens` (they
/// require `max_completion_tokens`); the classic models take the old fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BodyMode {
    /// Detect by deployment/model name prefix (case-insensitive): `gpt-5`,
    /// `o1`, `o3`, `o4` → reasoning; everything else → classic.
    #[default]
    Auto,
    /// Always send `temperature: 0.2` + `max_tokens`.
    Classic,
    /// Never send `temperature`; send `max_completion_tokens`.
    Reasoning,
}

/// Detects the body family from the deployment/model name (case-insensitive
/// prefix match) — the `auto` behaviour of [`BodyMode`].
#[must_use]
pub fn detect_body_mode(deployment: &str) -> BodyMode {
    let name = deployment.trim().to_lowercase();
    if ["gpt-5", "o1", "o3", "o4"]
        .iter()
        .any(|prefix| name.starts_with(prefix))
    {
        BodyMode::Reasoning
    } else {
        BodyMode::Classic
    }
}

/// How the key travels on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FoundryAuthMode {
    /// `api-key: <key>` header — the Azure OpenAI default.
    #[default]
    ApiKey,
    /// `Authorization: Bearer <key>` — OpenAI-compatible gateways.
    Bearer,
}

/// Endpoint config for the Foundry backend (no secret lives here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundryConfig {
    /// `KIRA_FOUNDRY_URL` — when set, the request goes verbatim to this URL
    /// and `model` travels in the body.
    pub url_override: Option<String>,
    /// `KIRA_FOUNDRY_ENDPOINT` — required unless [`FoundryConfig::url_override`]
    /// is set.
    pub endpoint: Option<String>,
    /// Deployment name (both surfaces).
    pub deployment: String,
    /// Legacy-surface `api-version` (ignored on the v1 surface).
    pub api_version: String,
    /// Auth mode.
    pub auth: FoundryAuthMode,
    /// `max_tokens` of the completion.
    pub max_tokens: u32,
    /// Request timeout in seconds.
    pub timeout_secs: u64,
    /// Body parameter family override.
    pub body_mode: BodyMode,
}

impl FoundryConfig {
    /// Reads the configuration from the `KIRA_FOUNDRY_*` environment.
    pub fn from_env() -> Result<Self, Error> {
        let url_override = env_trim(ENV_URL);
        let endpoint = env_trim(ENV_ENDPOINT);
        let endpoint = match (endpoint, url_override.is_some()) {
            (Some(endpoint), _) => Some(endpoint),
            (None, true) => None, // gateway verbatim: endpoint not needed
            (None, false) => return Err(Error::FoundryMissingEndpoint),
        };
        let deployment = env_trim(ENV_DEPLOYMENT).ok_or(Error::FoundryMissingDeployment)?;
        let api_version =
            env_trim(ENV_API_VERSION).unwrap_or_else(|| DEFAULT_API_VERSION.to_string());
        let auth = match env_trim(ENV_AUTH) {
            Some(value) => parse_auth(&value)?,
            None => FoundryAuthMode::default(),
        };
        let max_tokens = match env_trim(ENV_MAX_TOKENS) {
            Some(value) => parse_max_tokens(&value)?,
            None => DEFAULT_MAX_TOKENS,
        };
        let timeout_secs = match env_trim(ENV_TIMEOUT) {
            Some(value) => parse_timeout_secs(&value)?,
            None => DEFAULT_TIMEOUT_SECS,
        };
        let body_mode = match env_trim(ENV_BODY_MODE) {
            Some(value) => parse_body_mode(&value)?,
            None => BodyMode::default(),
        };
        Ok(Self {
            url_override,
            endpoint,
            deployment,
            api_version,
            auth,
            max_tokens,
            timeout_secs,
            body_mode,
        })
    }

    /// Resolves the request target: `(url, model_in_body)`.
    ///
    /// - verbatim `KIRA_FOUNDRY_URL` wins and `model` goes in the body;
    /// - endpoint ending in `/v1` → `{endpoint}/chat/completions` (v1
    ///   surface, `model` in the body, no `api-version`);
    /// - anything else → the legacy deployments URL with the `api-version`
    ///   query and no `model` in the body.
    pub fn resolve_request(&self) -> Result<(String, bool), Error> {
        if let Some(url) = &self.url_override {
            return Ok((url.clone(), true));
        }
        let endpoint = self
            .endpoint
            .as_deref()
            .ok_or(Error::FoundryMissingEndpoint)?;
        // The project endpoint answers the SDK/project APIs, not chat
        // completions — failing here beats a confusing 404 downstream.
        if endpoint.contains("/api/projects/") {
            return Err(Error::FoundryEndpoint);
        }
        let base = endpoint.trim_end_matches('/');
        if base.ends_with("/v1") {
            return Ok((format!("{base}/chat/completions"), true));
        }
        Ok((
            format!(
                "{base}/openai/deployments/{}/chat/completions?api-version={}",
                self.deployment, self.api_version
            ),
            false,
        ))
    }
}

/// Backend that talks to Microsoft Foundry over the OpenAI-compatible HTTP
/// API. Constructing it validates the key and the endpoint — a missing key
/// fails before any network call.
pub struct FoundryBackend {
    config: FoundryConfig,
    url: String,
    model_in_body: bool,
    api_key: String,
}

// Manual Debug on purpose: the key must never land in a log via `{:?}`.
impl std::fmt::Debug for FoundryBackend {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FoundryBackend")
            .finish_non_exhaustive()
    }
}

impl FoundryBackend {
    /// Builds the backend from the `KIRA_FOUNDRY_*` environment, validating
    /// the key before any network call.
    pub fn from_env() -> Result<Self, Error> {
        let config = FoundryConfig::from_env()?;
        let api_key = std::env::var(ENV_API_KEY)
            .ok()
            .filter(|key| !key.trim().is_empty())
            .ok_or(Error::FoundryMissingKey)?;
        Self::new(config, api_key)
    }

    /// Builds the backend from an explicit config; the key is validated here
    /// so an absent key never reaches the network.
    pub fn new(config: FoundryConfig, api_key: impl Into<String>) -> Result<Self, Error> {
        let api_key = api_key.into();
        if api_key.trim().is_empty() {
            return Err(Error::FoundryMissingKey);
        }
        let (url, model_in_body) = config.resolve_request()?;
        Ok(Self {
            config,
            url,
            model_in_body,
            api_key,
        })
    }
}

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    messages: [Message<'a>; 2],
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<&'a str>,
}

#[derive(Debug, Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    content: String,
}

impl LlmBackend for FoundryBackend {
    async fn complete(&self, system: &str, prompt: &str) -> Result<String, Error> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(self.config.timeout_secs))
            .build()
            .map_err(|err| network_error(&err))?;
        let reasoning = match self.config.body_mode {
            BodyMode::Auto => detect_body_mode(&self.config.deployment),
            explicit => explicit,
        } == BodyMode::Reasoning;
        let body = ChatRequest {
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
            // GPT-5 / o-series reject `temperature` and `max_tokens`; the
            // family decides the shape (auto-detect or explicit override).
            temperature: (!reasoning).then_some(DEFAULT_TEMPERATURE),
            max_tokens: (!reasoning).then_some(self.config.max_tokens),
            max_completion_tokens: reasoning.then_some(self.config.max_tokens),
            model: self
                .model_in_body
                .then_some(self.config.deployment.as_str()),
        };
        let mut builder = client.post(&self.url).json(&body);
        match self.config.auth {
            FoundryAuthMode::ApiKey => {
                builder = builder.header("api-key", &self.api_key);
            }
            FoundryAuthMode::Bearer => {
                // The header is only ever attached, never echoed.
                builder = builder.header("authorization", format!("Bearer {}", self.api_key));
            }
        }
        let response = builder.send().await;
        let response = match response {
            Ok(response) => response,
            Err(err) if err.is_timeout() => {
                return Err(Error::FoundryTimeout {
                    timeout_secs: self.config.timeout_secs,
                });
            }
            Err(err) => return Err(network_error(&err)),
        };
        let status = response.status().as_u16();
        // Read before the body: `text()` consumes the response.
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
            .filter(|value| !value.trim().is_empty());
        let body = response.text().await.map_err(|err| network_error(&err))?;
        if status >= 400 {
            return Err(status_error(status, retry_after, &body));
        }
        parse_success(&body)
    }
}

/// Network failure without a response: the message is redacted (a verbatim
/// URL may carry a query with a secret) and no body is echoed.
fn network_error(err: &reqwest::Error) -> Error {
    Error::FoundryFailed {
        detail: redact(&err.to_string()),
    }
}

/// Maps a non-2xx answer to the actionable variant. The server message is
/// reduced to its first line (<= 200 chars) and redacted — never the raw body.
fn status_error(status: u16, retry_after: Option<String>, body: &str) -> Error {
    let line = extract_error_message(body);
    match status {
        401 | 403 => Error::FoundryAuth { status },
        404 if line.contains("api-version") => Error::FoundryApiVersion { status },
        404 => Error::FoundryDeployment,
        429 => {
            Error::FoundryRateLimited(retry_after.unwrap_or_else(|| "alguns segundos".to_string()))
        }
        _ if status == 400 && line.contains("api-version") => Error::FoundryApiVersion { status },
        // Reasoning models (GPT-5, o-series) reject the classic fields — the
        // fix is a body-mode change, not a retry.
        _ if status == 400 && rejects_body_parameter(&line) => Error::FoundryBodyMode {
            status,
            detail: redact(&line),
        },
        _ => Error::FoundryFailed {
            detail: redact(&line),
        },
    }
}

/// True when the server message says the request body carried a parameter
/// this model family does not take.
fn rejects_body_parameter(line: &str) -> bool {
    line.contains("unsupported_parameter")
        || line.contains("temperature")
        || line.contains("max_tokens")
        || line.contains("max_completion_tokens")
}

/// First line of the server `error.message`, bounded to 200 chars; empty when
/// the shape is unexpected (the body is never echoed raw).
fn extract_error_message(body: &str) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return String::new();
    };
    let message = value
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(|message| message.as_str())
        .unwrap_or_default();
    message
        .lines()
        .next()
        .unwrap_or_default()
        .chars()
        .take(200)
        .collect()
}

/// Extracts `choices[0].message.content`; an unexpected shape cites the
/// missing field (serde's wording) instead of dumping the body.
fn parse_success(body: &str) -> Result<String, Error> {
    let parsed: ChatResponse = serde_json::from_str(body).map_err(|err| Error::FoundryFailed {
        detail: format!("resposta inesperada do Foundry: {err}"),
    })?;
    let choice = parsed
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| Error::FoundryFailed {
            detail: "resposta inesperada do Foundry: choices vazio".to_string(),
        })?;
    Ok(choice.message.content)
}

/// `KIRA_FOUNDRY_BODY_MODE` — `auto` (default), `classic` or `reasoning`.
pub fn parse_body_mode(value: &str) -> Result<BodyMode, Error> {
    match value.trim().to_lowercase().as_str() {
        "auto" => Ok(BodyMode::Auto),
        "classic" => Ok(BodyMode::Classic),
        "reasoning" => Ok(BodyMode::Reasoning),
        other => Err(Error::FoundryEnv {
            env: ENV_BODY_MODE,
            value: other.to_string(),
            hint: "use 'auto' (detecta pela família do modelo), 'classic' (temperature + max_tokens) ou 'reasoning' (GPT-5/o-series: max_completion_tokens)",
        }),
    }
}

/// `KIRA_FOUNDRY_AUTH` — `api-key` (default) or `bearer`.
pub fn parse_auth(value: &str) -> Result<FoundryAuthMode, Error> {
    match value.trim().to_lowercase().as_str() {
        "api-key" | "apikey" => Ok(FoundryAuthMode::ApiKey),
        "bearer" => Ok(FoundryAuthMode::Bearer),
        other => Err(Error::FoundryEnv {
            env: ENV_AUTH,
            value: other.to_string(),
            hint: "use 'api-key' (padrão Azure) ou 'bearer' (gateways OpenAI-compatible)",
        }),
    }
}

/// `KIRA_FOUNDRY_MAX_TOKENS` — integer >= 1.
pub fn parse_max_tokens(value: &str) -> Result<u32, Error> {
    let tokens: u32 = value
        .trim()
        .parse()
        .map_err(|_| int_env_error(ENV_MAX_TOKENS, value))?;
    if tokens == 0 {
        return Err(int_env_error(ENV_MAX_TOKENS, value));
    }
    Ok(tokens)
}

/// `KIRA_FOUNDRY_TIMEOUT` — integer >= 1 seconds.
pub fn parse_timeout_secs(value: &str) -> Result<u64, Error> {
    let secs: u64 = value
        .trim()
        .parse()
        .map_err(|_| int_env_error(ENV_TIMEOUT, value))?;
    if secs == 0 {
        return Err(int_env_error(ENV_TIMEOUT, value));
    }
    Ok(secs)
}

fn int_env_error(env: &'static str, value: &str) -> Error {
    let hint = match env {
        ENV_MAX_TOKENS => "defina um inteiro >= 1 (ex.: export KIRA_FOUNDRY_MAX_TOKENS=1024)",
        _ => "defina um inteiro >= 1 (ex.: export KIRA_FOUNDRY_TIMEOUT=120)",
    };
    Error::FoundryEnv {
        env,
        value: value.to_string(),
        hint,
    }
}

fn env_trim(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread::JoinHandle;

    const FAKE_CONTENT: &str = "Causa raiz provável:\nTTL do token muito curto.\nArquivos prováveis:\nsrc/auth/session.rs\nConfiança:\nmédia";

    /// Canonical OpenAI-compatible answer served by the fake server.
    const FAKE_OPENAI_BODY: &str = r#"{"id":"chatcmpl-1","object":"chat.completion","model":"gpt-5-mini","choices":[{"index":0,"message":{"role":"assistant","content":"Causa raiz provável:\nTTL do token muito curto.\nArquivos prováveis:\nsrc/auth/session.rs\nConfiança:\nmédia"},"finish_reason":"stop"}],"usage":{"total_tokens":42}}"#;

    /// Key whose shape `redact` masks (`sk-` + 16+ chars): proves the key
    /// never leaks even when the server echoes it back in an error message.
    const TEST_KEY: &str = "sk-kiratestkey1234567890";

    const DEPLOYMENT: &str = "gpt-4.1-mini";

    /// Reasoning-family deployment (GPT-5): rejects `temperature`/`max_tokens`.
    const GPT5_DEPLOYMENT: &str = "gpt-5";

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

    /// One-shot fake Foundry on an ephemeral port: answers `status_line` with
    /// `body` (+ `extra_headers`) and captures the raw request.
    fn spawn_fake_foundry(
        status_line: &'static str,
        extra_headers: &str,
        body: &'static str,
    ) -> (String, JoinHandle<Option<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .unwrap_or_else(|err| panic!("bind na porta efêmera: {err}"));
        let addr = listener
            .local_addr()
            .unwrap_or_else(|err| panic!("endereço local: {err}"));
        let extra_headers = extra_headers.to_string();
        let body = body.to_string();
        let handle = std::thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return None;
            };
            let request = read_http_request(&mut stream);
            let response = format!(
                "{status_line}\r\nContent-Type: application/json\r\n{extra_headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
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

    /// Fake Foundry that accepts, sleeps and only then answers — for the
    /// timeout test.
    fn spawn_slow_foundry(
        body: &'static str,
        delay: Duration,
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
            std::thread::sleep(delay);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
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

    fn config_for(endpoint: String) -> FoundryConfig {
        FoundryConfig {
            url_override: None,
            endpoint: Some(endpoint),
            deployment: DEPLOYMENT.to_string(),
            api_version: DEFAULT_API_VERSION.to_string(),
            auth: FoundryAuthMode::default(),
            max_tokens: DEFAULT_MAX_TOKENS,
            timeout_secs: DEFAULT_TIMEOUT_SECS,
            body_mode: BodyMode::default(),
        }
    }

    fn backend_for(endpoint: String) -> FoundryBackend {
        FoundryBackend::new(config_for(endpoint), TEST_KEY)
            .unwrap_or_else(|err| panic!("backend de teste deve ser válido: {err}"))
    }

    fn request_from(handle: JoinHandle<Option<String>>) -> String {
        handle
            .join()
            .unwrap_or_else(|_| panic!("thread do servidor falso entrou em pânico"))
            .unwrap_or_else(|| panic!("servidor falso deve capturar a requisição"))
    }

    #[tokio::test]
    async fn v1_surface_posts_to_base_chat_completions_with_model_in_body() {
        let (base, server) = spawn_fake_foundry("HTTP/1.1 200 OK", "", FAKE_OPENAI_BODY);
        let backend = backend_for(format!("{base}/v1"));
        let content = backend
            .complete("sistema fixo", "prompt de teste")
            .await
            .unwrap_or_else(|err| panic!("complete deve funcionar na superfície v1: {err}"));
        assert_eq!(content, FAKE_CONTENT);
        let request = request_from(server);
        assert!(
            request.starts_with("POST /v1/chat/completions HTTP/1.1"),
            "a URL v1 deve ser exatamente {{endpoint}}/chat/completions: {request}"
        );
        assert!(
            !request.contains("api-version"),
            "a superfície v1 não pode anexar api-version: {request}"
        );
        assert!(
            request.contains(&format!("api-key: {TEST_KEY}")),
            "header api-key deve viajar: {request}"
        );
        assert!(
            request.contains(&format!("\"model\":\"{DEPLOYMENT}\"")),
            "o deployment deve ir no campo model do corpo (v1): {request}"
        );
        assert!(request.contains("\"temperature\":0.2"), "{request}");
        assert!(
            request.contains("\"max_tokens\":1024"),
            "max_tokens default no corpo: {request}"
        );
        assert!(request.contains("\"role\":\"system\""), "{request}");
        assert!(request.contains("\"role\":\"user\""), "{request}");
    }

    #[tokio::test]
    async fn legacy_surface_posts_to_the_deployments_url_with_api_version() {
        let (base, server) = spawn_fake_foundry("HTTP/1.1 200 OK", "", FAKE_OPENAI_BODY);
        let backend = backend_for(base.clone());
        let content = backend
            .complete("sistema", "prompt")
            .await
            .unwrap_or_else(|err| panic!("complete deve funcionar na superfície legada: {err}"));
        assert_eq!(content, FAKE_CONTENT);
        let request = request_from(server);
        assert!(
            request.starts_with(&format!(
                "POST /openai/deployments/{DEPLOYMENT}/chat/completions?api-version={DEFAULT_API_VERSION} HTTP/1.1"
            )),
            "URL legada com deployments + api-version: {request}"
        );
        assert!(
            !request.contains("\"model\":"),
            "na superfície legada o deployment vai na URL, não no corpo: {request}"
        );
        let _ = base;
    }

    #[tokio::test]
    async fn bearer_mode_sends_the_authorization_header() {
        let (base, server) = spawn_fake_foundry("HTTP/1.1 200 OK", "", FAKE_OPENAI_BODY);
        let mut config = config_for(format!("{base}/v1"));
        config.auth = FoundryAuthMode::Bearer;
        let backend = FoundryBackend::new(config, TEST_KEY)
            .unwrap_or_else(|err| panic!("backend de teste deve ser válido: {err}"));
        let content = backend
            .complete("sistema", "prompt")
            .await
            .unwrap_or_else(|err| panic!("complete deve funcionar em modo bearer: {err}"));
        assert_eq!(content, FAKE_CONTENT);
        let request = request_from(server).to_lowercase();
        assert!(
            request.contains(&format!(
                "authorization: bearer {}",
                TEST_KEY.to_lowercase()
            )),
            "modo bearer deve enviar Authorization: Bearer: {request}"
        );
        assert!(
            !request.contains("\napi-key:"),
            "modo bearer não pode enviar o header api-key: {request}"
        );
    }

    #[tokio::test]
    async fn http_401_maps_to_foundry_auth() {
        let body = r#"{"error":{"message":"Access denied due to invalid subscription key\nsegunda linha"}}"#;
        let (base, server) = spawn_fake_foundry("HTTP/1.1 401 Unauthorized", "", body);
        let backend = backend_for(format!("{base}/v1"));
        let Err(err) = backend.complete("sistema", "prompt").await else {
            panic!("401 deve virar erro");
        };
        drop(server);
        assert!(matches!(err, Error::FoundryAuth { status: 401 }), "{err:?}");
        let message = err.to_string();
        assert!(
            message.contains("gere uma nova chave"),
            "erro 401 deve dizer como trocar a chave: {message}"
        );
        assert!(
            message.contains("KIRA_FOUNDRY_AUTH"),
            "erro 401 deve mencionar o modo de auth: {message}"
        );
    }

    #[tokio::test]
    async fn http_404_maps_to_foundry_deployment() {
        let body = r#"{"error":{"message":"Deployment not found"}}"#;
        let (base, server) = spawn_fake_foundry("HTTP/1.1 404 Not Found", "", body);
        drop(server);
        let backend = backend_for(base);
        let Err(err) = backend.complete("sistema", "prompt").await else {
            panic!("404 deve virar erro");
        };
        assert!(
            matches!(err, Error::FoundryDeployment),
            "404 deve virar FoundryDeployment: {err:?}"
        );
        let message = err.to_string();
        assert!(
            message.contains("KIRA_FOUNDRY_DEPLOYMENT") && message.contains("confira"),
            "erro 404 deve dizer onde conferir: {message}"
        );
    }

    #[tokio::test]
    async fn api_version_rejection_tells_how_to_override_it() {
        let body = r#"{"error":{"message":"api-version 2024-10-21 is not supported by this endpoint\nuse a versão vigente"}}"#;
        let (base, server) = spawn_fake_foundry("HTTP/1.1 400 Bad Request", "", body);
        drop(server);
        let backend = backend_for(base);
        let Err(err) = backend.complete("sistema", "prompt").await else {
            panic!("400 citando api-version deve virar erro");
        };
        assert!(
            matches!(err, Error::FoundryApiVersion { status: 400 }),
            "400 citando api-version deve virar FoundryApiVersion: {err:?}"
        );
        let message = err.to_string();
        assert!(
            message.contains("KIRA_FOUNDRY_API_VERSION"),
            "o erro deve dizer para definir KIRA_FOUNDRY_API_VERSION: {message}"
        );
    }

    #[tokio::test]
    async fn http_429_includes_the_retry_after_value() {
        let body = r#"{"error":{"message":"Rate limit exceeded"}}"#;
        let (base, server) = spawn_fake_foundry(
            "HTTP/1.1 429 Too Many Requests",
            "Retry-After: 42\r\n",
            body,
        );
        drop(server);
        let backend = backend_for(base);
        let Err(err) = backend.complete("sistema", "prompt").await else {
            panic!("429 deve virar erro");
        };
        assert!(
            matches!(err, Error::FoundryRateLimited(ref value) if value == "42"),
            "429 deve carregar o retry-after: {err:?}"
        );
        let message = err.to_string();
        assert!(
            message.contains("42") && message.contains("aguarde"),
            "a mensagem deve incluir o retry-after: {message}"
        );
    }

    #[tokio::test]
    async fn success_without_choices_cites_the_missing_field() {
        let (base, server) = spawn_fake_foundry(
            "HTTP/1.1 200 OK",
            "",
            r#"{"object":"chat.completion","id":"x"}"#,
        );
        let backend = backend_for(base);
        let Err(err) = backend.complete("sistema", "prompt").await else {
            panic!("shape inesperado deve virar erro");
        };
        drop(server);
        assert!(matches!(err, Error::FoundryFailed { .. }), "{err:?}");
        let message = err.to_string();
        assert!(
            message.contains("choices"),
            "shape inesperado deve citar o campo faltante: {message}"
        );
        assert!(
            !message.contains("chat.completion"),
            "o corpo da resposta não pode vazar no erro: {message}"
        );
    }

    #[tokio::test]
    async fn server_error_message_is_redacted_and_truncated() {
        let body = r#"{"error":{"message":"a chave sk-kiratestkey1234567890 está inválida — contate o dono do recurso\nsegunda linha que não pode aparecer"}}"#;
        let (base, server) = spawn_fake_foundry("HTTP/1.1 500 Internal Server Error", "", body);
        let backend = backend_for(base);
        let Err(err) = backend.complete("sistema", "prompt").await else {
            panic!("500 deve virar erro");
        };
        drop(server);
        assert!(
            matches!(err, Error::FoundryFailed { .. }),
            "500 deve virar FoundryFailed: {err:?}"
        );
        let message = err.to_string();
        assert!(
            !message.contains(TEST_KEY),
            "a chave não pode aparecer na mensagem de erro: {message}"
        );
        assert!(
            !message.contains("segunda linha"),
            "só a 1ª linha da error.message pode aparecer: {message}"
        );
        assert!(
            message.contains("inválida"),
            "a mensagem redigida do servidor deve aparecer: {message}"
        );
    }

    #[tokio::test]
    async fn missing_key_fails_without_touching_the_network() {
        // Port bound and immediately dropped: if any dial happened, the error
        // would be a network one — instead the key check fires first.
        let listener = TcpListener::bind("127.0.0.1:0")
            .unwrap_or_else(|err| panic!("bind na porta efêmera: {err}"));
        let addr = listener
            .local_addr()
            .unwrap_or_else(|err| panic!("endereço local: {err}"));
        drop(listener);
        let config = config_for(format!("{addr}/v1"));
        let Err(err) = FoundryBackend::new(config, "   ") else {
            panic!("chave vazia deve recusar o backend");
        };
        assert!(
            matches!(err, Error::FoundryMissingKey),
            "chave ausente/vazia deve virar FoundryMissingKey: {err:?}"
        );
        let message = err.to_string();
        assert!(
            message.contains("KIRA_FOUNDRY_API_KEY"),
            "o erro deve nomear a env da chave: {message}"
        );
    }

    #[tokio::test]
    async fn project_endpoint_is_rejected_with_an_actionable_error() {
        let config = config_for(
            "https://sdossantosjunior-5474-resource.services.ai.azure.com/api/projects/sdossantosjunior-5474"
                .to_string(),
        );
        let Err(err) = config.resolve_request() else {
            panic!("endpoint de projeto deve ser recusado");
        };
        assert!(matches!(err, Error::FoundryEndpoint), "{err:?}");
        let message = err.to_string();
        assert!(
            message.contains("/openai/v1"),
            "o erro deve apontar o formato correto do endpoint: {message}"
        );
    }

    #[tokio::test]
    async fn verbatim_url_is_used_without_api_version() {
        let (base, server) = spawn_fake_foundry("HTTP/1.1 200 OK", "", FAKE_OPENAI_BODY);
        let mut config = config_for("https://nao-usado.openai.azure.com".to_string());
        config.url_override = Some(format!("{base}/qualquer/gateway/chat/completions"));
        let backend = FoundryBackend::new(config, TEST_KEY)
            .unwrap_or_else(|err| panic!("backend de teste deve ser válido: {err}"));
        let content = backend
            .complete("sistema", "prompt")
            .await
            .unwrap_or_else(|err| panic!("KIRA_FOUNDRY_URL verbatim deve funcionar: {err}"));
        assert_eq!(content, FAKE_CONTENT);
        let request = request_from(server);
        assert!(
            request.starts_with("POST /qualquer/gateway/chat/completions HTTP/1.1"),
            "KIRA_FOUNDRY_URL deve ser usada verbatim: {request}"
        );
        assert!(
            !request.contains("api-version"),
            "URL verbatim não ganha query de api-version: {request}"
        );
        assert!(
            request.contains(&format!("\"model\":\"{DEPLOYMENT}\"")),
            "gateway OpenAI-compatible recebe model no corpo: {request}"
        );
    }

    #[tokio::test]
    async fn custom_max_tokens_reaches_the_body() {
        let (base, server) = spawn_fake_foundry("HTTP/1.1 200 OK", "", FAKE_OPENAI_BODY);
        let mut config = config_for(format!("{base}/v1"));
        config.max_tokens = 256;
        let backend = FoundryBackend::new(config, TEST_KEY)
            .unwrap_or_else(|err| panic!("backend de teste deve ser válido: {err}"));
        let content = backend
            .complete("sistema", "prompt")
            .await
            .unwrap_or_else(|err| panic!("complete deve funcionar: {err}"));
        assert_eq!(content, FAKE_CONTENT);
        let request = request_from(server);
        assert!(
            request.contains("\"max_tokens\":256"),
            "max_tokens customizado deve viajar: {request}"
        );
    }

    #[tokio::test]
    async fn gpt5_body_omits_temperature_and_uses_max_completion_tokens() {
        let (base, server) = spawn_fake_foundry("HTTP/1.1 200 OK", "", FAKE_OPENAI_BODY);
        let mut config = config_for(format!("{base}/v1"));
        config.deployment = GPT5_DEPLOYMENT.to_string();
        let backend = FoundryBackend::new(config, TEST_KEY)
            .unwrap_or_else(|err| panic!("backend de teste deve ser válido: {err}"));
        let content = backend
            .complete("sistema", "prompt")
            .await
            .unwrap_or_else(|err| panic!("complete deve funcionar com gpt-5: {err}"));
        assert_eq!(content, FAKE_CONTENT);
        let request = request_from(server);
        assert!(
            !request.contains("\"temperature\":"),
            "GPT-5 rejeita temperature (400 unsupported_parameter): {request}"
        );
        assert!(
            !request.contains("\"max_tokens\":"),
            "GPT-5 rejeita max_tokens: {request}"
        );
        assert!(
            request.contains("\"max_completion_tokens\":1024"),
            "GPT-5 exige max_completion_tokens: {request}"
        );
    }

    #[tokio::test]
    async fn body_mode_classic_forces_the_old_fields_for_gpt5() {
        let (base, server) = spawn_fake_foundry("HTTP/1.1 200 OK", "", FAKE_OPENAI_BODY);
        let mut config = config_for(format!("{base}/v1"));
        config.deployment = GPT5_DEPLOYMENT.to_string();
        config.body_mode = BodyMode::Classic;
        let backend = FoundryBackend::new(config, TEST_KEY)
            .unwrap_or_else(|err| panic!("backend de teste deve ser válido: {err}"));
        let _ = backend
            .complete("sistema", "prompt")
            .await
            .unwrap_or_else(|err| panic!("complete deve funcionar: {err}"));
        let request = request_from(server);
        assert!(
            request.contains("\"temperature\":0.2") && request.contains("\"max_tokens\":1024"),
            "KIRA_FOUNDRY_BODY_MODE=classic volta ao formato antigo: {request}"
        );
        assert!(
            !request.contains("\"max_completion_tokens\":"),
            "classic não pode enviar max_completion_tokens: {request}"
        );
    }

    #[tokio::test]
    async fn body_mode_reasoning_forces_max_completion_tokens() {
        let (base, server) = spawn_fake_foundry("HTTP/1.1 200 OK", "", FAKE_OPENAI_BODY);
        let mut config = config_for(format!("{base}/v1"));
        config.body_mode = BodyMode::Reasoning;
        let backend = FoundryBackend::new(config, TEST_KEY)
            .unwrap_or_else(|err| panic!("backend de teste deve ser válido: {err}"));
        let _ = backend
            .complete("sistema", "prompt")
            .await
            .unwrap_or_else(|err| panic!("complete deve funcionar: {err}"));
        let request = request_from(server);
        assert!(
            !request.contains("\"temperature\":"),
            "reasoning não pode enviar temperature: {request}"
        );
        assert!(
            !request.contains("\"max_tokens\":"),
            "reasoning não pode enviar max_tokens: {request}"
        );
        assert!(
            request.contains("\"max_completion_tokens\":1024"),
            "reasoning deve enviar max_completion_tokens: {request}"
        );
    }

    #[tokio::test]
    async fn http_400_unsupported_parameter_suggests_body_mode() {
        let body = r#"{"error":{"message":"unsupported_parameter: temperature is not supported with this model\nuse max_completion_tokens instead"}}"#;
        let (base, server) = spawn_fake_foundry("HTTP/1.1 400 Bad Request", "", body);
        let backend = backend_for(base);
        let Err(err) = backend.complete("sistema", "prompt").await else {
            panic!("unsupported_parameter deve virar erro");
        };
        drop(server);
        assert!(
            matches!(err, Error::FoundryBodyMode { status: 400, .. }),
            "400 com unsupported_parameter deve virar FoundryBodyMode: {err:?}"
        );
        let message = err.to_string();
        assert!(
            message.contains("KIRA_FOUNDRY_BODY_MODE") && message.contains("reasoning"),
            "o erro deve sugerir KIRA_FOUNDRY_BODY_MODE: {message}"
        );
    }

    #[test]
    fn body_mode_detection_is_case_insensitive_and_prefix_based() {
        for name in [
            "gpt-5",
            "GPT-5",
            "gpt-5-mini",
            "o1",
            "o1-preview",
            "o3-mini",
            "O3-MINI",
            "o4-mini",
        ] {
            assert_eq!(
                detect_body_mode(name),
                BodyMode::Reasoning,
                "{name} é família reasoning"
            );
        }
        for name in ["gpt-4.1-mini", "gpt-4o", "gpt-35-turbo", "qwen3.5:4b"] {
            assert_eq!(
                detect_body_mode(name),
                BodyMode::Classic,
                "{name} é família clássica"
            );
        }
    }

    #[tokio::test]
    async fn timeout_maps_to_foundry_timeout() {
        let (base, server) = spawn_slow_foundry(FAKE_OPENAI_BODY, Duration::from_millis(2500));
        let mut config = config_for(format!("{base}/v1"));
        config.timeout_secs = 1;
        let backend = FoundryBackend::new(config, TEST_KEY)
            .unwrap_or_else(|err| panic!("backend de teste deve ser válido: {err}"));
        let Err(err) = backend.complete("sistema", "prompt").await else {
            panic!("estouro de tempo deve virar erro");
        };
        drop(server);
        assert!(
            matches!(err, Error::FoundryTimeout { timeout_secs: 1 }),
            "estouro de tempo deve virar FoundryTimeout: {err:?}"
        );
        let message = err.to_string();
        assert!(
            message.contains("KIRA_FOUNDRY_TIMEOUT"),
            "o erro deve nomear a env do timeout: {message}"
        );
    }

    #[test]
    fn invalid_env_values_are_rejected_with_the_env_name() {
        for value in ["abc", "0", "1.5", ""] {
            let Err(err) = parse_max_tokens(value) else {
                panic!("valor {value:?} de max_tokens deve ser recusado");
            };
            let message = err.to_string();
            assert!(
                matches!(err, Error::FoundryEnv { .. }),
                "valor inválido deve virar FoundryEnv: {err:?}"
            );
            assert!(
                message.contains("KIRA_FOUNDRY_MAX_TOKENS"),
                "o erro deve nomear a env ofensora: {message}"
            );
        }
        for value in ["abc", "0", ""] {
            let Err(err) = parse_timeout_secs(value) else {
                panic!("valor {value:?} de timeout deve ser recusado");
            };
            let message = err.to_string();
            assert!(
                message.contains("KIRA_FOUNDRY_TIMEOUT"),
                "o erro deve nomear a env ofensora: {message}"
            );
        }
        let Err(err) = parse_auth("kerberos") else {
            panic!("auth inválido deve ser recusado");
        };
        let message = err.to_string();
        assert!(
            message.contains("KIRA_FOUNDRY_AUTH") && message.contains("bearer"),
            "auth inválido deve listar os valores válidos: {message}"
        );
        let Err(err) = parse_body_mode("turbo") else {
            panic!("body_mode inválido deve ser recusado");
        };
        let message = err.to_string();
        assert!(
            message.contains("KIRA_FOUNDRY_BODY_MODE"),
            "body_mode inválido deve nomear a env: {message}"
        );
        assert!(parse_body_mode("auto").is_ok());
        assert!(parse_body_mode("classic").is_ok());
        assert!(parse_body_mode("reasoning").is_ok());
        assert!(parse_auth("bearer").is_ok());
        assert!(parse_auth("api-key").is_ok());
        assert!(parse_auth("API-KEY").is_ok());
        assert_eq!(parse_max_tokens("2048").ok(), Some(2048));
        assert_eq!(parse_timeout_secs("60").ok(), Some(60));
    }
}
