//! SC5 — Secret redaction for logs, audit trails, and SSE payloads.
//!
//! Conservative pattern set focused on **high-confidence** tokens so the
//! false-positive rate stays low. Each pattern is anchored on a distinctive
//! prefix (e.g. `sk-`, `ghp_`, `AKIA`) — generic high-entropy strings are
//! intentionally NOT matched here, since redacting random base64 in a code
//! diff would destroy useful audit signal.
//!
//! Workshop SPEKTOR P5 calls out "redaction/sanitização de secrets/logs" as
//! a guardrail. Plan note AP10 (2026-05-25): module previously only existed
//! in comments — `core/src/safety/mod.rs:3` mentioned `secrets_filter` but
//! there was no actual implementation.
//!
//! Usage: call `redact(&s)` before logging / writing to audit / streaming.
//! Replacement is a placeholder of the same shape so downstream readers can
//! tell *what kind* of secret was scrubbed without seeing its value.

use regex::Regex;

/// Returns a copy of `input` with known secret patterns replaced by an
/// opaque placeholder. Safe to call on already-redacted text (idempotent
/// when patterns no longer match).
pub fn redact(input: &str) -> String {
    let mut out = input.to_string();
    for (re, replacement) in PATTERNS.iter() {
        out = re.replace_all(&out, *replacement).into_owned();
    }
    out
}

/// True when `input` contains any recognised secret pattern. Cheap check —
/// useful for log-level decisions ("don't include this payload at INFO").
pub fn looks_secret(input: &str) -> bool {
    PATTERNS.iter().any(|(re, _)| re.is_match(input))
}

type Pattern = (Regex, &'static str);

static PATTERNS: std::sync::LazyLock<Vec<Pattern>> = std::sync::LazyLock::new(|| {
    vec![
        // Anthropic keys (sk-ant-*) — must match before generic OpenAI sk-.
        (
            Regex::new(r"sk-ant-[A-Za-z0-9_\-]{20,}").unwrap(),
            "[REDACTED:anthropic]",
        ),
        // OpenAI keys (sk-, sk-proj-, sk-svcacct-, etc).
        (
            Regex::new(r"sk-(?:proj-|svcacct-|admin-)?[A-Za-z0-9_-]{20,}").unwrap(),
            "[REDACTED:openai]",
        ),
        // GitHub tokens: ghp_/gho_/ghs_/ghr_/ghu_ (PATs, OAuth, server tokens).
        (
            Regex::new(r"gh[posru]_[A-Za-z0-9]{36,}").unwrap(),
            "[REDACTED:github]",
        ),
        // GitHub fine-grained PATs.
        (
            Regex::new(r"github_pat_[A-Za-z0-9_]{60,}").unwrap(),
            "[REDACTED:github-pat]",
        ),
        // AWS access key IDs.
        (
            Regex::new(r"AKIA[A-Z0-9]{16}").unwrap(),
            "[REDACTED:aws-akid]",
        ),
        // Slack tokens (xoxb, xoxp, xoxa, xoxo).
        (
            Regex::new(r"xox[bpoars]-[A-Za-z0-9\-]{10,}").unwrap(),
            "[REDACTED:slack]",
        ),
        // Google API key.
        (
            Regex::new(r"AIza[A-Za-z0-9_\-]{35}").unwrap(),
            "[REDACTED:google-api]",
        ),
        // DeepSeek / DashScope / generic provider keys exposed via
        // `Authorization: Bearer <token>`. We redact the token, keeping the
        // word "Bearer" so the log still reveals the auth scheme.
        (
            Regex::new(r"(?i)bearer\s+[A-Za-z0-9_\-\.=]{16,}").unwrap(),
            "Bearer [REDACTED:token]",
        ),
        // `password=` / `passwd=` / `pwd=` assignments. Conservative: only
        // matches when followed by non-whitespace value of >= 4 chars.
        (
            Regex::new(r#"(?i)(password|passwd|pwd)\s*[:=]\s*['"]?[^\s'"\n]{4,}"#).unwrap(),
            "$1=[REDACTED:password]",
        ),
    ]
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_openai_key() {
        let s = "config: sk-proj-abc123def456ghi789jkl012mno345pqr678";
        let r = redact(s);
        assert!(r.contains("[REDACTED:openai]"), "got: {r}");
        assert!(!r.contains("sk-proj-abc"));
    }

    #[test]
    fn redacts_anthropic_key() {
        let s = "ANTHROPIC_API_KEY=sk-ant-api03-abcdef1234567890abcdef";
        let r = redact(s);
        assert!(r.contains("[REDACTED:anthropic]"));
    }

    #[test]
    fn redacts_github_pat() {
        let s = "Bearer ghp_aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789";
        let r = redact(s);
        // Bearer pattern fires first; either redaction is acceptable.
        assert!(r.contains("[REDACTED"));
        assert!(!r.contains("ghp_aBcDe"));
    }

    #[test]
    fn redacts_aws_akid() {
        let s = "aws creds: AKIAIOSFODNN7EXAMPLE found in logs";
        let r = redact(s);
        assert!(r.contains("[REDACTED:aws-akid]"));
    }

    #[test]
    fn redacts_bearer_token() {
        let s = "curl -H 'Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9'";
        let r = redact(s);
        assert!(r.contains("Bearer [REDACTED:token]"));
        assert!(!r.contains("eyJhbGci"));
    }

    #[test]
    fn redacts_password_assignment() {
        let s = "DB_URL: postgres://user:hunter2pass@host/db; password=hunter2pass";
        let r = redact(s);
        assert!(r.contains("[REDACTED:password]"));
    }

    #[test]
    fn idempotent_when_no_secret() {
        let s = "regular log line with no secrets, just a path /tmp/foo and grep 'a|b'";
        assert_eq!(redact(s), s);
    }

    #[test]
    fn looks_secret_signals_match() {
        assert!(looks_secret("sk-ant-api03-aaaaaaaaaaaaaaaaaaaaaaaa"));
        assert!(!looks_secret("hello world"));
    }
}
