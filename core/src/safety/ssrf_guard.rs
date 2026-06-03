//! SSRF guard — blocks requests to private / loopback / cloud-metadata IPs.
//!
//! Defense-in-depth: even after a URL parses cleanly, the hostname may
//! resolve (or CNAME) to a private IP. Strategy:
//!
//! 1. Parse URL via `url::Url` (reject non-http/https).
//! 2. Extract host. If literal IP → check directly.
//! 3. If hostname → resolve via `hickory-resolver` and check EVERY answer.
//! 4. Reject if any answer falls in a blocked range.
//!
//! Blocked ranges (IPv4):
//!   - 127.0.0.0/8      loopback
//!   - 10.0.0.0/8       RFC1918 private
//!   - 172.16.0.0/12    RFC1918 private
//!   - 192.168.0.0/16   RFC1918 private
//!   - 169.254.0.0/16   link-local (cloud metadata)
//!   - 100.64.0.0/10    CGNAT
//!   - 0.0.0.0/8        wildcard
//!   - 224.0.0.0/4      multicast
//!   - 240.0.0.0/4      reserved
//!
//! IPv6: `::1` loopback, `fc00::/7` ULA, `fe80::/10` link-local rejected.

use anyhow::{Context, Result};
use hickory_resolver::TokioAsyncResolver;
use hickory_resolver::config::{ResolverConfig, ResolverOpts};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use url::Url;

/// Validate URL string. Returns the parsed URL with resolved final IP(s)
/// confirmed safe. Caller proceeds to fetch using the original URL.
pub async fn check(url_str: &str) -> Result<Url> {
    let url = Url::parse(url_str).map_err(|e| anyhow::anyhow!("invalid url: {e}"))?;
    let scheme = url.scheme();
    if scheme != "http" && scheme != "https" {
        anyhow::bail!("scheme '{scheme}' not allowed (need http/https)");
    }
    let host = url.host_str().context("url missing host")?.to_string();

    // Literal IP fast-path.
    if let Ok(ip) = host.parse::<IpAddr>() {
        if let Some(reason) = blocked_reason(&ip) {
            anyhow::bail!("blocked: literal {host} ({reason})");
        }
        return Ok(url);
    }

    // DNS resolve (all A + AAAA answers).
    let resolver = TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default());
    let lookup = resolver
        .lookup_ip(&host)
        .await
        .map_err(|e| anyhow::anyhow!("dns resolution failed for {host}: {e}"))?;
    let mut ips = lookup.iter().peekable();
    if ips.peek().is_none() {
        anyhow::bail!("dns resolution returned no addresses for {host}");
    }
    for ip in ips {
        if let Some(reason) = blocked_reason(&ip) {
            anyhow::bail!("blocked: {host} resolves to {ip} ({reason})");
        }
    }
    Ok(url)
}

/// Returns blocking reason if IP falls in a forbidden range.
pub fn blocked_reason(ip: &IpAddr) -> Option<&'static str> {
    match ip {
        IpAddr::V4(v4) => blocked_v4(v4),
        IpAddr::V6(v6) => blocked_v6(v6),
    }
}

fn blocked_v4(ip: &Ipv4Addr) -> Option<&'static str> {
    let o = ip.octets();
    if o[0] == 127 { return Some("loopback (127.0.0.0/8)"); }
    if o[0] == 10 { return Some("RFC1918 private (10.0.0.0/8)"); }
    if o[0] == 172 && (16..=31).contains(&o[1]) { return Some("RFC1918 private (172.16.0.0/12)"); }
    if o[0] == 192 && o[1] == 168 { return Some("RFC1918 private (192.168.0.0/16)"); }
    if o[0] == 169 && o[1] == 254 { return Some("link-local / cloud metadata (169.254.0.0/16)"); }
    if o[0] == 100 && (64..=127).contains(&o[1]) { return Some("CGNAT (100.64.0.0/10)"); }
    if o[0] == 0 { return Some("wildcard (0.0.0.0/8)"); }
    if o[0] >= 224 && o[0] <= 239 { return Some("multicast (224.0.0.0/4)"); }
    if o[0] >= 240 { return Some("reserved (240.0.0.0/4)"); }
    None
}

fn blocked_v6(ip: &Ipv6Addr) -> Option<&'static str> {
    if ip.is_loopback() { return Some("loopback (::1)"); }
    if ip.is_unspecified() { return Some("unspecified (::)"); }
    let seg = ip.segments();
    // fc00::/7 unique local
    if (seg[0] & 0xfe00) == 0xfc00 { return Some("ULA (fc00::/7)"); }
    // fe80::/10 link-local
    if (seg[0] & 0xffc0) == 0xfe80 { return Some("link-local (fe80::/10)"); }
    // IPv4-mapped — re-check as IPv4
    if let Some(v4) = ip.to_ipv4_mapped() {
        return blocked_v4(&v4);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_blocked() {
        assert!(blocked_reason(&"127.0.0.1".parse().unwrap()).is_some());
        assert!(blocked_reason(&"127.255.255.254".parse().unwrap()).is_some());
    }

    #[test]
    fn rfc1918_blocked() {
        assert!(blocked_reason(&"10.0.0.5".parse().unwrap()).is_some());
        assert!(blocked_reason(&"172.16.0.1".parse().unwrap()).is_some());
        assert!(blocked_reason(&"172.31.255.255".parse().unwrap()).is_some());
        assert!(blocked_reason(&"192.168.1.1".parse().unwrap()).is_some());
    }

    #[test]
    fn cloud_metadata_blocked() {
        assert!(blocked_reason(&"169.254.169.254".parse().unwrap()).is_some());
    }

    #[test]
    fn public_ip_allowed() {
        assert!(blocked_reason(&"8.8.8.8".parse().unwrap()).is_none());
        assert!(blocked_reason(&"1.1.1.1".parse().unwrap()).is_none());
    }

    #[test]
    fn ipv6_loopback_blocked() {
        assert!(blocked_reason(&"::1".parse().unwrap()).is_some());
    }

    #[test]
    fn ipv6_ula_blocked() {
        assert!(blocked_reason(&"fc00::1".parse().unwrap()).is_some());
        assert!(blocked_reason(&"fd00::dead".parse().unwrap()).is_some());
    }

    #[test]
    fn ipv6_link_local_blocked() {
        assert!(blocked_reason(&"fe80::1".parse().unwrap()).is_some());
    }

    #[test]
    fn ipv6_public_allowed() {
        // Google DNS IPv6
        assert!(blocked_reason(&"2001:4860:4860::8888".parse().unwrap()).is_none());
    }

    #[tokio::test]
    async fn check_rejects_bad_scheme() {
        let err = check("ftp://example.com").await.unwrap_err();
        assert!(format!("{err}").contains("scheme"));
    }

    #[tokio::test]
    async fn check_rejects_literal_loopback() {
        let err = check("http://127.0.0.1:8080/").await.unwrap_err();
        assert!(format!("{err}").contains("blocked"));
    }

    #[tokio::test]
    async fn check_rejects_metadata_endpoint() {
        let err = check("http://169.254.169.254/latest/meta-data/").await.unwrap_err();
        assert!(format!("{err}").contains("blocked"));
    }
}
