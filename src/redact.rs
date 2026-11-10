//! Privacy boundary — masks secrets before they reach a log, an error message
//! or an LLM prompt (`AGENTS.md`: nunca enviar segredos ao LLM; W1-PR2).
//!
//! The function is pure and idempotent: `redact(redact(x)) == redact(x)`, so it
//! is safe to apply at more than one layer without piling up markers.
//!
//! Design: placeholders (`[REDACTED]`, `[EMAIL]`, `[CPF]`) instead of partial
//! masks. A partial mask ("a***@b.com") still leaks length and shape, and a
//! leaked token shape is what makes guessing worthwhile.

use regex::Regex;
use std::sync::OnceLock;

/// Marker for a masked credential.
pub const MASKED: &str = "[REDACTED]";

/// Regexes are compiled once, lazily; `regex` was already a transitive
/// dependency (same version) and is the right layer for this per ADR-0016
/// (cli/parser before LLM — masking is parser work, never a model task).
struct Patterns {
    bearer: Regex,
    keyed: Regex,
    url_query: Regex,
    known_prefixes: Regex,
    email: Regex,
    cpf: Regex,
}

/// Compiles a pattern that is a literal in this file.
fn compile(pattern: &str) -> Regex {
    // allow-expect: o padrão é constante de código, nunca dado de usuário; um erro aqui é
    // bug de build (coberto pelos testes de redact). Devolver Result obrigaria todo chamador
    // a tratar uma falha impossível, e falhar em silêncio na privacidade seria pior.
    Regex::new(pattern).expect("regex constante")
}

fn patterns() -> &'static Patterns {
    static PATTERNS: OnceLock<Patterns> = OnceLock::new();
    PATTERNS.get_or_init(|| Patterns {
        // `Authorization: Bearer <token>` and bare `bearer <token>`.
        bearer: compile(r"(?i)\b(bearer\s+)([A-Za-z0-9._\-]{8,})"),
        // `token=...`, `pat: ...`, `api_key=...`, `password = ...`, `secret: ...`
        keyed: compile(
            r"(?i)\b(pat|token|api[_-]?key|access[_-]?token|refresh[_-]?token|client[_-]?secret|secret|password|passwd|pwd|sig|signature)\b(\s*[=:]\s*)([^\s&,;\x22']{3,})",
        ),
        // Secret carried in a URL query string.
        url_query: compile(
            r"(?i)([?&](?:token|access_token|refresh_token|sig|signature|code|key|api_key)=)([^&\s#]+)",
        ),
        // Well-known token shapes.
        known_prefixes: compile(
            r"(?:gh[pousr]_[A-Za-z0-9]{16,}|github_pat_[A-Za-z0-9_]{16,}|sk-[A-Za-z0-9]{16,}|AKIA[0-9A-Z]{16}|xox[baprs]-[A-Za-z0-9\-]{8,})",
        ),
        email: compile(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}"),
        // Formatted CPF only; bare 11 digits collide with ids and timestamps.
        cpf: compile(r"\b\d{3}\.\d{3}\.\d{3}-\d{2}\b"),
    })
}

/// Masks credentials, e-mails and CPFs in `text`.
///
/// Order matters: token shapes first (they can contain `=` and `:`), then
/// keyed assignments, then URL queries, then e-mail, then CPF.
#[must_use]
pub fn redact(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let p = patterns();
    let mut out = text.to_string();

    out = p
        .bearer
        .replace_all(&out, format!("${{1}}{MASKED}").as_str())
        .into_owned();
    out = p.known_prefixes.replace_all(&out, MASKED).into_owned();
    out = p
        .keyed
        .replace_all(&out, format!("${{1}}${{2}}{MASKED}").as_str())
        .into_owned();
    out = p
        .url_query
        .replace_all(&out, format!("${{1}}{MASKED}").as_str())
        .into_owned();
    out = p.email.replace_all(&out, "[EMAIL]").into_owned();
    out = p.cpf.replace_all(&out, "[CPF]").into_owned();
    out
}

/// True when `text` contains something [`redact`] would mask.
#[must_use]
pub fn contains_secret(text: &str) -> bool {
    redact(text) != text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_azure_pat_style_assignments() {
        let out = redact("PAT salvo: token=AbC123XyZ789QqQ");
        assert!(!out.contains("AbC123XyZ789QqQ"), "{out}");
        assert!(out.contains(MASKED), "{out}");
        assert!(
            out.contains("token="),
            "a chave deve continuar legível: {out}"
        );
    }

    #[test]
    fn masks_authorization_header() {
        let out = redact("Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6");
        assert!(!out.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6"), "{out}");
        assert!(out.contains("Bearer"), "{out}");
    }

    #[test]
    fn masks_secret_in_url_query() {
        let out = redact("https://dev.azure.com/org/_apis?api-version=7.0&sig=abcDEF123456%2Fxyz");
        assert!(!out.contains("abcDEF123456"), "{out}");
        assert!(out.contains("sig="), "{out}");
        assert!(
            out.contains("api-version=7.0"),
            "query não secreta preservada: {out}"
        );
    }

    #[test]
    fn masks_known_token_shapes() {
        for token in [
            "ghp_0123456789abcdefghijABCDEFGHIJ012345",
            "github_pat_11ABCDEFG0abcdefghijklmnop",
            "sk-abcdefghijklmnopqrstuvwx",
            "AKIAIOSFODNN7EXAMPLE",
            "xoxb-123456789012-abcdefghijkl",
        ] {
            let out = redact(&format!("chave {token} fim"));
            assert!(!out.contains(token), "{token} vazou: {out}");
        }
    }

    #[test]
    fn masks_email_and_cpf() {
        let out = redact("contato serjota@proton.me doc 123.456.789-01");
        assert!(!out.contains("serjota@proton.me"), "{out}");
        assert!(!out.contains("123.456.789-01"), "{out}");
        assert!(out.contains("[EMAIL]") && out.contains("[CPF]"), "{out}");
    }

    #[test]
    fn leaves_clean_text_untouched() {
        let text = "O build falhou no módulo de pagamento; veja src/pay.rs:42";
        assert_eq!(redact(text), text);
        assert!(!contains_secret(text));
    }

    #[test]
    fn is_idempotent() {
        let once = redact("token=AbC123XyZ789QqQ mail a@b.com cpf 123.456.789-01");
        let twice = redact(&once);
        assert_eq!(once, twice, "redact deve ser idempotente");
    }

    #[test]
    fn does_not_touch_ids_and_numbers() {
        let text = "work item 529511, build 12345678901, versão 1.2.3";
        assert_eq!(redact(text), text);
    }
}
