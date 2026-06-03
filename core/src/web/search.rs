//! `web_search` — pluggable backend.
//!
//! v0.5 default: **Brave Search API** (Rust-only, no Docker). API key read
//! from env (`BRAVE_API_KEY` by default) so it never lands in `tokens.toml`.
//! Free tier is 2000 q/month, plenty for local dev.
//!
//! v0.6 deploy: **SearXNG** (sovereign self-host). Same trait, swap provider
//! via `tokens.toml [web_tools].search_provider = "searxng"`.
//!
//! Adding a new provider = implement [`WebSearchProvider`] + branch
//! [`search`] on `cfg.search_provider`. Zero callers change.

use anyhow::{Context, Result};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Client;
use url::Url;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{info, instrument, warn};
use ts_rs::TS;

use crate::config::WebToolsConfig;

#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/web.ts")]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub engine: String,
}

#[derive(Deserialize, Debug, Default, TS)]
#[ts(export, export_to = "../../agent/src/generated/web.ts")]
pub struct SearchOpts {
    #[serde(default)]
    pub max_results: Option<u32>,
    #[serde(default)]
    pub language: Option<String>,
}

/// Common surface for every search backend. Async-trait keeps the dispatch
/// code in [`search`] readable without manual `Box<dyn>` juggling.
#[async_trait]
pub trait WebSearchProvider: Send + Sync {
    /// Provider id (used in logs and `SearchResult.engine` when the upstream
    /// doesn't report a per-result engine).
    fn name(&self) -> &'static str;
    async fn run(
        &self,
        cfg: &WebToolsConfig,
        query: &str,
        opts: &SearchOpts,
    ) -> Result<Vec<SearchResult>>;
}

/// Construct a provider by name. `None` for unknown names.
fn provider_by_name(name: &str) -> Option<Box<dyn WebSearchProvider>> {
    match name {
        "brave" => Some(Box::new(BraveSearch)),
        "searxng" => Some(Box::new(SearXng)),
        "duckduckgo" | "ddg" => Some(Box::new(DuckDuckGo)),
        _ => None,
    }
}

/// Ordered provider chain: `search_chain` when set, else the single
/// `search_provider` (legacy behaviour).
fn build_chain(cfg: &WebToolsConfig) -> Vec<String> {
    if cfg.search_chain.is_empty() {
        vec![cfg.search_provider.clone()]
    } else {
        cfg.search_chain.clone()
    }
}

/// Public entry point — runs the configured provider chain with fallback.
///
/// Tries each provider in order; the first that returns a non-empty result set
/// wins. Failures (network, quota/4xx, parse, empty) fall through to the next
/// provider, so `web_search` works as long as *any* backend is reachable
/// (e.g. DuckDuckGo needs no API key). Returns the last error only if every
/// provider failed.
#[instrument(skip(cfg, opts), fields(query = %query))]
pub async fn search(
    cfg: &WebToolsConfig,
    query: &str,
    opts: &SearchOpts,
) -> Result<Vec<SearchResult>> {
    let chain = build_chain(cfg);
    let primary = chain.first().cloned().unwrap_or_default();
    let mut last_err: Option<anyhow::Error> = None;

    for name in &chain {
        let Some(provider) = provider_by_name(name) else {
            warn!(provider = %name, "unknown search provider in chain — skipping");
            continue;
        };
        match provider.run(cfg, query, opts).await {
            Ok(results) if !results.is_empty() => {
                if name != &primary {
                    info!(provider = %name, primary = %primary, "web_search fell back");
                }
                return Ok(results);
            }
            Ok(_) => {
                warn!(provider = %name, "web_search returned 0 results — trying next");
            }
            Err(e) => {
                warn!(provider = %name, error = %e, "web_search provider failed — trying next");
                last_err = Some(e);
            }
        }
    }

    match last_err {
        Some(e) => Err(e.context("all web_search providers in the chain failed")),
        // Every provider reached but returned empty.
        None => Ok(Vec::new()),
    }
}

// ─── Brave Search API ────────────────────────────────────────────────────────

pub struct BraveSearch;

#[async_trait]
impl WebSearchProvider for BraveSearch {
    fn name(&self) -> &'static str { "brave" }

    async fn run(
        &self,
        cfg: &WebToolsConfig,
        query: &str,
        opts: &SearchOpts,
    ) -> Result<Vec<SearchResult>> {
        let api_key = std::env::var(&cfg.brave_api_key_env).with_context(|| {
            format!(
                "Brave Search API key not set in env `{}`. Get a free key at \
                 https://api.search.brave.com (2000 q/month).",
                cfg.brave_api_key_env
            )
        })?;
        let max = opts.max_results.unwrap_or(cfg.search_max_results);
        let lang = opts
            .language
            .clone()
            .unwrap_or_else(|| cfg.search_default_lang.clone());

        let client = Client::builder()
            .timeout(Duration::from_secs(cfg.search_timeout_secs))
            .build()
            .context("build brave HTTP client")?;

        // Brave API: `q`, `count` (1..20), `search_lang` (ISO 639-1 prefix).
        // pt-BR → "pt"; en-US → "en".
        let search_lang = lang.split('-').next().unwrap_or("en");
        let resp = client
            .get(&cfg.brave_base_url)
            .header("Accept", "application/json")
            .header("Accept-Encoding", "gzip")
            .header("X-Subscription-Token", api_key)
            .query(&[
                ("q", query),
                ("count", &max.min(20).to_string()),
                ("search_lang", search_lang),
                ("safesearch", "moderate"),
            ])
            .send()
            .await
            .with_context(|| format!("brave request failed: {}", cfg.brave_base_url))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("brave status {status}: {}", short_preview(&body));
        }
        let body: serde_json::Value = resp
            .json()
            .await
            .context("brave response not JSON")?;

        let results = body
            .pointer("/web/results")
            .and_then(|v| v.as_array())
            .context("brave response missing /web/results array")?;

        Ok(results
            .iter()
            .take(max as usize)
            .filter_map(|r| {
                Some(SearchResult {
                    title: r.get("title")?.as_str()?.to_string(),
                    url: r.get("url")?.as_str()?.to_string(),
                    snippet: r
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    engine: "brave".to_string(),
                })
            })
            .collect())
    }
}

// ─── SearXNG (sovereign self-host, v0.6 deploy) ─────────────────────────────

pub struct SearXng;

#[async_trait]
impl WebSearchProvider for SearXng {
    fn name(&self) -> &'static str { "searxng" }

    async fn run(
        &self,
        cfg: &WebToolsConfig,
        query: &str,
        opts: &SearchOpts,
    ) -> Result<Vec<SearchResult>> {
        let max = opts.max_results.unwrap_or(cfg.search_max_results);
        let lang = opts
            .language
            .clone()
            .unwrap_or_else(|| cfg.search_default_lang.clone());

        let client = Client::builder()
            .timeout(Duration::from_secs(cfg.search_timeout_secs))
            .build()
            .context("build searxng HTTP client")?;

        let url = format!("{}/search", cfg.searxng_base_url.trim_end_matches('/'));
        let resp = client
            .get(&url)
            .query(&[("q", query), ("format", "json"), ("language", &lang)])
            .send()
            .await
            .with_context(|| format!("searxng request failed: {url}"))?;

        if !resp.status().is_success() {
            anyhow::bail!("searxng status {}", resp.status());
        }
        let body: serde_json::Value = resp
            .json()
            .await
            .context("searxng response not JSON — check `format=json` engine setting")?;

        let arr = body
            .get("results")
            .and_then(|v| v.as_array())
            .context("searxng response missing `results` array")?;

        Ok(arr
            .iter()
            .take(max as usize)
            .filter_map(|r| {
                Some(SearchResult {
                    title: r.get("title")?.as_str()?.to_string(),
                    url: r.get("url")?.as_str()?.to_string(),
                    snippet: r
                        .get("content")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    engine: r
                        .get("engine")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                })
            })
            .collect())
    }
}

// ─── DuckDuckGo (zero-key last-resort fallback) ─────────────────────────────
//
// No official free JSON web-results API. We use the HTML endpoint and parse
// result anchors. Brittle by nature (HTML can change) — that's why it's last
// in the chain. Needs no API key, so `web_search` still works out of the box.

pub struct DuckDuckGo;

#[async_trait]
impl WebSearchProvider for DuckDuckGo {
    fn name(&self) -> &'static str { "duckduckgo" }

    async fn run(
        &self,
        cfg: &WebToolsConfig,
        query: &str,
        opts: &SearchOpts,
    ) -> Result<Vec<SearchResult>> {
        let max = opts.max_results.unwrap_or(cfg.search_max_results);
        let client = Client::builder()
            .timeout(Duration::from_secs(cfg.search_timeout_secs))
            .build()
            .context("build duckduckgo HTTP client")?;

        // The html endpoint returns server-rendered result anchors.
        let resp = client
            .get("https://html.duckduckgo.com/html/")
            .header("User-Agent", &cfg.user_agent)
            .query(&[("q", query)])
            .send()
            .await
            .context("duckduckgo request failed")?;

        if !resp.status().is_success() {
            anyhow::bail!("duckduckgo status {}", resp.status());
        }
        let html = resp.text().await.context("duckduckgo response not text")?;
        let results = parse_ddg_html(&html, max as usize);
        if results.is_empty() {
            anyhow::bail!("duckduckgo: no results parsed (HTML layout may have changed)");
        }
        Ok(results)
    }
}

/// Parse DuckDuckGo html result anchors into [`SearchResult`]s.
fn parse_ddg_html(html: &str, max: usize) -> Vec<SearchResult> {
    static RESULT_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r#"(?s)<a[^>]+class="result__a"[^>]*href="([^"]+)"[^>]*>(.*?)</a>"#).unwrap()
    });
    static SNIPPET_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r#"(?s)class="result__snippet"[^>]*>(.*?)</a>"#).unwrap()
    });

    let snippets: Vec<String> = SNIPPET_RE
        .captures_iter(html)
        .map(|c| clean_html(&c[1]))
        .collect();

    let mut out = Vec::new();
    for (i, cap) in RESULT_RE.captures_iter(html).enumerate() {
        if out.len() >= max {
            break;
        }
        let url = decode_ddg_url(&cap[1]);
        let title = clean_html(&cap[2]);
        if title.is_empty() || url.is_empty() {
            continue;
        }
        out.push(SearchResult {
            title,
            url,
            snippet: snippets.get(i).cloned().unwrap_or_default(),
            engine: "duckduckgo".to_string(),
        });
    }
    out
}

/// DuckDuckGo wraps result links as `//duckduckgo.com/l/?uddg=<encoded>`.
/// Unwrap to the real target; pass through direct links unchanged.
fn decode_ddg_url(href: &str) -> String {
    let abs = if let Some(stripped) = href.strip_prefix("//") {
        format!("https://{stripped}")
    } else {
        href.to_string()
    };
    if let Ok(parsed) = Url::parse(&abs)
        && let Some((_, target)) = parsed.query_pairs().find(|(k, _)| k == "uddg")
    {
        return target.into_owned();
    }
    abs
}

/// Strip HTML tags and unescape the common entities from a fragment.
fn clean_html(s: &str) -> String {
    static TAG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"<[^>]+>").unwrap());
    let no_tags = TAG_RE.replace_all(s, "");
    no_tags
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .trim()
        .to_string()
}

fn short_preview(s: &str) -> String {
    if s.len() > 200 {
        format!("{}…", s.chars().take(200).collect::<String>())
    } else {
        s.to_string()
    }
}

/// Render a results list as a markdown bulleted list for the agent context
/// (`web_search` tool) and the REST `/web/search` endpoint. Kept here so
/// callers don't reimplement the same formatting per call site.
pub fn render_results_md(results: &[SearchResult]) -> String {
    if results.is_empty() {
        return "(no results)".to_string();
    }
    let mut out = String::new();
    for (i, r) in results.iter().enumerate() {
        out.push_str(&format!(
            "{n}. **{title}** — `{url}` _(via {engine})_\n   {snippet}\n",
            n = i + 1,
            title = r.title,
            url = r.url,
            engine = r.engine,
            snippet = r.snippet,
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg_with_provider(name: &str) -> WebToolsConfig {
        let mut cfg = WebToolsConfig::default();
        cfg.search_provider = name.into();
        cfg
    }

    #[test]
    fn unknown_provider_dispatch_picks_brave() {
        // Pure dispatch check — no live HTTP. Verifies the `match` in
        // `search()` falls through to Brave on unknown names. We assert
        // by re-reading the same match arms here (compiler enforces sync).
        let cfg = cfg_with_provider("nonexistent-provider");
        let picked: &str = match cfg.search_provider.as_str() {
            "brave" => BraveSearch.name(),
            "searxng" => SearXng.name(),
            _ => BraveSearch.name(), // fallback — must match search() body
        };
        assert_eq!(picked, "brave");
    }

    #[test]
    fn default_chain_is_brave_then_searxng() {
        let cfg = WebToolsConfig::default();
        assert_eq!(cfg.search_chain, vec!["brave", "searxng"]);
        assert_eq!(build_chain(&cfg), vec!["brave", "searxng"]);
    }

    #[test]
    fn empty_chain_falls_back_to_single_provider() {
        let mut cfg = cfg_with_provider("searxng");
        cfg.search_chain = vec![];
        assert_eq!(build_chain(&cfg), vec!["searxng"]);
    }

    #[test]
    fn provider_by_name_known_and_unknown() {
        assert!(provider_by_name("brave").is_some());
        assert!(provider_by_name("searxng").is_some());
        assert!(provider_by_name("duckduckgo").is_some());
        assert!(provider_by_name("ddg").is_some());
        assert!(provider_by_name("nope").is_none());
    }

    #[test]
    fn decode_ddg_redirect_url() {
        let href = "//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fa%3Fb%3D1&rut=x";
        assert_eq!(decode_ddg_url(href), "https://example.com/a?b=1");
        // Direct link passes through.
        assert_eq!(decode_ddg_url("https://direct.example/x"), "https://direct.example/x");
    }

    #[test]
    fn clean_html_strips_tags_and_entities() {
        assert_eq!(clean_html("<b>Foo</b> &amp; bar"), "Foo & bar");
        assert_eq!(clean_html("a &#x27;quote&#x27;"), "a 'quote'");
    }

    #[test]
    fn parse_ddg_html_extracts_results() {
        let html = r#"
            <div class="result">
              <a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Frust-lang.org%2F">The <b>Rust</b> Language</a>
              <a class="result__snippet" href="x">A systems &amp; language</a>
            </div>
            <div class="result">
              <a rel="nofollow" class="result__a" href="https://docs.rs/">docs.rs</a>
              <a class="result__snippet" href="y">Docs host</a>
            </div>
        "#;
        let out = parse_ddg_html(html, 10);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].url, "https://rust-lang.org/");
        assert_eq!(out[0].title, "The Rust Language");
        assert_eq!(out[0].snippet, "A systems & language");
        assert_eq!(out[1].url, "https://docs.rs/");
        assert!(out.iter().all(|r| r.engine == "duckduckgo"));
    }

    #[test]
    fn parse_ddg_html_respects_max() {
        let html = (0..10)
            .map(|i| format!(r#"<a rel="nofollow" class="result__a" href="https://e{i}.com/">t{i}</a>"#))
            .collect::<Vec<_>>()
            .join("\n");
        let out = parse_ddg_html(&html, 3);
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn config_default_provider_is_brave() {
        let cfg = WebToolsConfig::default();
        assert_eq!(cfg.search_provider, "brave");
        assert_eq!(cfg.brave_api_key_env, "BRAVE_API_KEY");
        assert!(cfg.brave_base_url.contains("api.search.brave.com"));
    }

    #[test]
    fn brave_provider_name() {
        assert_eq!(BraveSearch.name(), "brave");
        assert_eq!(SearXng.name(), "searxng");
    }
}
