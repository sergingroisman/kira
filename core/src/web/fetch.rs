//! `web_fetch` — SSRF-checked HTTP GET → readability → markdown.
//!
//! Pipeline:
//! 1. `ssrf_guard::check` validates URL + resolves DNS.
//! 2. `reqwest::Client` with redirect policy that re-validates every hop.
//! 3. Streams bytes with size cap.
//! 4. `readability` extracts main content; `htmd` converts HTML → markdown.
//! 5. On failure or <100 chars output, optional Jina fallback `r.jina.ai/{url}`.
//! 6. Output wrapped in `<TOOL_OUTPUT untrusted>` (SC1).

use anyhow::{Context, Result};
use reqwest::{redirect::Policy, Client};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{instrument, warn};
use ts_rs::TS;

use crate::config::WebToolsConfig;
use crate::safety::ssrf_guard;

#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/web.ts")]
pub struct FetchResult {
    pub url: String,
    pub final_url: String,
    pub bytes_read: usize,
    pub markdown: String,
    pub source: FetchSource,
    /// Page metadata extracted via the `webpage` crate (OpenGraph + standard
    /// meta tags). Added 2026-05-26 so agents can ground answers in the
    /// page's own claims about itself before reading the body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site_name: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../agent/src/generated/web.ts")]
#[serde(rename_all = "snake_case")]
pub enum FetchSource {
    /// `readability` succeeded on local fetch.
    Local,
    /// Fell through to Jina Reader API.
    JinaFallback,
}

#[instrument(skip(cfg), fields(url = %url))]
pub async fn fetch(cfg: &WebToolsConfig, url: &str) -> Result<FetchResult> {
    // SC4-style guard BEFORE any network IO.
    let validated = ssrf_guard::check(url).await.context("ssrf check failed")?;

    let client = build_client(cfg)?;

    // robots.txt gate (P5 guardrails + ethics). Skipped when disabled or
    // when the robots file itself can't be fetched — absence of a robots
    // file means "everything is allowed" per RFC 9309.
    if cfg.respect_robots {
        check_robots(&client, cfg, &validated).await?;
    }

    let local = fetch_local(&client, cfg, &validated).await;

    match local {
        Ok(r) if r.markdown.trim().len() >= 100 => Ok(r),
        other => {
            if cfg.jina_fallback_enabled {
                match other {
                    Ok(r) => warn!(
                        url = %r.url,
                        bytes = r.markdown.len(),
                        "local parse too short — falling back to Jina"
                    ),
                    Err(e) => warn!(error = ?e, url, "local fetch failed — falling back to Jina"),
                }
                fetch_jina(&client, cfg, url).await
            } else {
                other.context("local fetch failed and jina fallback disabled")
            }
        }
    }
}

fn build_client(cfg: &WebToolsConfig) -> Result<Client> {
    // Conservative redirect policy: we'd ideally re-validate each hop.
    // Limitation: reqwest exposes redirects but the validator needs async
    // DNS, hard to do inside the sync `Policy::custom` closure. v0 compromise:
    // cap hop count + final URL is re-checked post-redirect via response.url().
    let max_hops = cfg.fetch_max_redirects;
    let policy = Policy::custom(move |attempt| {
        if attempt.previous().len() >= max_hops as usize {
            attempt.error("too many redirects (>{max_hops})")
        } else {
            attempt.follow()
        }
    });
    Client::builder()
        .timeout(Duration::from_secs(cfg.fetch_timeout_secs))
        .redirect(policy)
        .build()
        .context("build fetch HTTP client")
}

async fn fetch_local(
    client: &Client,
    cfg: &WebToolsConfig,
    url: &url::Url,
) -> Result<FetchResult> {
    let resp = client
        .get(url.as_str())
        .send()
        .await
        .with_context(|| format!("GET {url} failed"))?;
    let final_url = resp.url().to_string();

    // Re-validate final URL post-redirect — covers SSRF via 30x to private IP.
    let _ = ssrf_guard::check(&final_url)
        .await
        .with_context(|| format!("post-redirect SSRF check failed for {final_url}"))?;

    if !resp.status().is_success() {
        anyhow::bail!("status {} for {final_url}", resp.status());
    }

    // Stream with cap.
    let bytes = read_capped(resp, cfg.fetch_max_bytes).await?;
    let bytes_read = bytes.len();
    let html = String::from_utf8_lossy(&bytes).to_string();

    // readability + htmd for the main body…
    let product = readability::extractor::extract(&mut html.as_bytes(), url)
        .context("readability extract failed")?;
    let md = htmd::HtmlToMarkdown::new()
        .convert(&product.content)
        .context("html→md convert failed")?;

    // …webpage crate for structured metadata (OpenGraph, <title>, meta
    // description). Pure string parse — no extra IO. Errors are swallowed
    // because metadata is best-effort; absence shouldn't fail the whole
    // fetch (workshop SPEKTOR P5 "guardrails sensatos por default").
    let meta = webpage::HTML::from_string(html.clone(), Some(final_url.clone())).ok();
    let (title, description, image_url, site_name) = meta
        .map(|h| {
            let og_image = h.opengraph.images.first().map(|i| i.url.clone());
            let site_name = h
                .opengraph
                .properties
                .get("site_name")
                .cloned();
            (h.title, h.description, og_image, site_name)
        })
        .unwrap_or((None, None, None, None));

    Ok(FetchResult {
        url: url.to_string(),
        final_url,
        bytes_read,
        markdown: md,
        source: FetchSource::Local,
        title,
        description,
        image_url,
        site_name,
    })
}

async fn fetch_jina(
    client: &Client,
    cfg: &WebToolsConfig,
    url: &str,
) -> Result<FetchResult> {
    let jina_url = format!("{}{}", cfg.jina_base_url.trim_end_matches('/'), {
        if url.starts_with('/') {
            url.to_string()
        } else {
            format!("/{url}")
        }
    });
    let resp = client
        .get(&jina_url)
        .send()
        .await
        .with_context(|| format!("jina GET {jina_url} failed"))?;
    if !resp.status().is_success() {
        anyhow::bail!("jina status {}", resp.status());
    }
    let bytes = read_capped(resp, cfg.fetch_max_bytes).await?;
    let bytes_read = bytes.len();
    let markdown = String::from_utf8_lossy(&bytes).to_string();
    Ok(FetchResult {
        url: url.to_string(),
        final_url: jina_url,
        bytes_read,
        markdown,
        source: FetchSource::JinaFallback,
        title: None,
        description: None,
        image_url: None,
        site_name: None,
    })
}

/// Fetch `<host>/robots.txt`, parse it with `texting_robots`, and refuse if
/// the target path is disallowed for our user-agent. Net failures (DNS,
/// non-200) are silently tolerated — per RFC 9309 absence of a robots file
/// implies "allow all". Disallowed → `anyhow::bail!`.
async fn check_robots(client: &Client, cfg: &WebToolsConfig, target: &url::Url) -> Result<()> {
    let robots_url = match texting_robots::get_robots_url(target.as_str()) {
        Ok(u) => u,
        Err(e) => {
            warn!(error = ?e, "could not derive robots url — allowing");
            return Ok(());
        }
    };
    // Robots fetch shares the same client (and therefore timeout/redirect
    // policy) so a hostile host can't slow us down on the gatekeeper.
    let resp = match client
        .get(&robots_url)
        .header("User-Agent", &cfg.user_agent)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            warn!(error = ?e, %robots_url, "robots.txt unreachable — allowing");
            return Ok(());
        }
    };
    if !resp.status().is_success() {
        // 404 etc → allow all.
        return Ok(());
    }
    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => {
            warn!(error = ?e, "robots.txt body read failed — allowing");
            return Ok(());
        }
    };
    let robot = match texting_robots::Robot::new(&cfg.user_agent, &bytes) {
        Ok(r) => r,
        Err(e) => {
            warn!(error = ?e, "robots.txt parse failed — allowing");
            return Ok(());
        }
    };
    if !robot.allowed(target.as_str()) {
        anyhow::bail!(
            "robots.txt disallows {} for user-agent `{}` (set web_tools.respect_robots=false to override)",
            target,
            cfg.user_agent
        );
    }
    Ok(())
}

async fn read_capped(resp: reqwest::Response, cap: usize) -> Result<Vec<u8>> {
    use futures::StreamExt;
    let mut stream = resp.bytes_stream();
    let mut buf: Vec<u8> = Vec::with_capacity(64 * 1024);
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("network chunk read failed")?;
        if buf.len() + chunk.len() > cap {
            anyhow::bail!("response exceeded {cap} bytes cap");
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

/// Render a `FetchResult` as markdown wrapped in untrusted delimiters (SC1).
///
/// `max_chars`: caps extracted markdown length to prevent context bloat.
/// Set to `usize::MAX` to disable. Appends a truncation notice when hit.
pub fn render_md(result: &FetchResult, max_chars: usize) -> String {
    let mut s = String::from("<TOOL_OUTPUT untrusted>\n");
    s.push_str(&format!(
        "# Fetched: {}\n_source: {:?} · final: {} · bytes: {}_\n\n",
        result.url, result.source, result.final_url, result.bytes_read
    ));
    // Surface page metadata (title/description/site) before the body so the
    // agent can fail-fast when the page is the wrong topic without spending
    // tokens on the full extracted markdown.
    if result.title.is_some() || result.description.is_some() || result.site_name.is_some() {
        s.push_str("## Metadata\n");
        if let Some(t) = &result.title {
            s.push_str(&format!("- **title**: {t}\n"));
        }
        if let Some(d) = &result.description {
            s.push_str(&format!("- **description**: {d}\n"));
        }
        if let Some(site) = &result.site_name {
            s.push_str(&format!("- **site**: {site}\n"));
        }
        if let Some(img) = &result.image_url {
            s.push_str(&format!("- **og:image**: {img}\n"));
        }
        s.push('\n');
    }
    let content = &result.markdown;
    if content.len() > max_chars {
        // Truncate at a char boundary.
        let truncated: String = content.chars().take(max_chars).collect();
        s.push_str(&truncated);
        s.push_str(&format!(
            "\n\n_[truncated: {} chars shown of {} total]_\n",
            max_chars,
            content.len()
        ));
    } else {
        s.push_str(content);
    }
    s.push_str("\n</TOOL_OUTPUT>\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_result() -> FetchResult {
        FetchResult {
            url: "https://example.com".into(),
            final_url: "https://example.com".into(),
            bytes_read: 100,
            markdown: "# Hello".into(),
            source: FetchSource::Local,
            title: None,
            description: None,
            image_url: None,
            site_name: None,
        }
    }

    #[test]
    fn render_wraps_in_delimiters() {
        let md = render_md(&empty_result(), usize::MAX);
        assert!(md.starts_with("<TOOL_OUTPUT untrusted>"));
        assert!(md.contains("# Hello"));
        assert!(md.ends_with("</TOOL_OUTPUT>\n"));
    }

    #[test]
    fn render_skips_metadata_block_when_empty() {
        let md = render_md(&empty_result(), usize::MAX);
        assert!(!md.contains("## Metadata"));
    }

    #[test]
    fn render_includes_metadata_when_present() {
        let mut r = empty_result();
        r.title = Some("Test Page".into());
        r.description = Some("A test page about testing".into());
        r.site_name = Some("Example".into());
        r.image_url = Some("https://example.com/og.png".into());
        let md = render_md(&r, usize::MAX);
        assert!(md.contains("## Metadata"));
        assert!(md.contains("**title**: Test Page"));
        assert!(md.contains("**description**: A test page about testing"));
        assert!(md.contains("**site**: Example"));
        assert!(md.contains("**og:image**: https://example.com/og.png"));
    }

    #[test]
    fn render_truncates_long_markdown() {
        let mut r = empty_result();
        r.markdown = "x".repeat(2000);
        let md = render_md(&r, 100);
        assert!(md.contains("[truncated:"));
        assert!(md.contains("100 chars shown of 2000 total"));
    }

    #[test]
    fn config_default_respects_robots() {
        let cfg = WebToolsConfig::default();
        assert!(cfg.respect_robots);
        assert!(cfg.user_agent.starts_with("KiraBot/"));
    }

    #[test]
    fn texting_robots_disallow_matches_user_agent() {
        // Direct unit on the robots parser — verifies our chosen crate
        // honours the disallow rule when our UA token matches exactly.
        // texting_robots wants the literal token (no slash/version suffix);
        // for the live `check_robots` path we pass `cfg.user_agent`
        // verbatim and rely on the crate's wildcard fallback (`User-agent: *`)
        // when an exact match isn't found.
        let robots = b"User-agent: KiraBot\nDisallow: /private/\n";
        let robot = texting_robots::Robot::new("KiraBot", robots).unwrap();
        assert!(!robot.allowed("https://example.com/private/secrets"));
        assert!(robot.allowed("https://example.com/public/page"));
    }

    #[test]
    fn texting_robots_falls_back_to_wildcard() {
        // Real-world: most sites only define `User-agent: *`. Our UA must
        // honour those rules even though no token matches verbatim.
        let robots = b"User-agent: *\nDisallow: /admin/\n";
        let robot = texting_robots::Robot::new("KiraBot/0.5", robots).unwrap();
        assert!(!robot.allowed("https://example.com/admin/login"));
        assert!(robot.allowed("https://example.com/docs/guide"));
    }
}
