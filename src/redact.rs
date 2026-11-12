//! Privacy boundary — masks secrets before they reach a log, an error message
//! or an LLM prompt (`AGENTS.md`: nunca enviar segredos ao LLM; W1-PR2).
//!
//! The function is pure and idempotent: `redact(redact(x)) == redact(x)`, so it
//! is safe to apply at more than one layer without piling up markers.
//!
//! W7.1 (ADR-0024 R1): before masking, a closed set of invisible characters
//! (zero-width and bidi controls) is stripped — a zero-width inside a secret's
//! shape (`token\u{200B}=…`, `ghp\u{200B}_…`) used to break the masking regexes
//! and cross the boundary unmasked, proven by the 12/Nov/2026 red team probe.
//!
//! Design: placeholders (`[REDACTED]`, `[EMAIL]`, `[CPF]`) instead of partial
//! masks. A partial mask ("a***@b.com") still leaks length and shape, and a
//! leaked token shape is what makes guessing worthwhile.

use regex::Regex;
use std::sync::OnceLock;

/// Marker for a masked credential.
pub const MASKED: &str = "[REDACTED]";

/// Conjunto fechado de caracteres invisíveis removidos antes do masking
/// (ADR-0024, requisito R1). Cada grupo existe por um motivo:
/// - `U+00AD` (soft hyphen): largura zero condicional — invisível no
///   renderizador e no revisor;
/// - `U+200B`/`U+200C`/`U+200D` (zero-width space, non-joiner, joiner):
///   o bypass do probe de 12/Nov — entre a chave e o `=`, ou dentro de um
///   shape conhecido, quebram o regex sem que ninguém veja nada;
/// - `U+202A`–`U+202E` (bidi embedding/override): reordenam o texto
///   renderizado — podem inverter o lado visível do `=` e escondem
///   instrução/segredo do revisor humano;
/// - `U+2060`–`U+2064` (word joiner e afins) e `U+FEFF` (BOM no meio do
///   texto): mesma família de formato Cf, mesmo efeito de bypass.
///
/// Espaços de verdade (`U+00A0` e a família `White_Space`) NÃO entram:
/// `\s` do regex já os casa (não há bypass) e removê-los destruiria texto
/// legítimo. Normalização NFKC também fica de fora — mudaria identificadores
/// reais de código (decisão registrada no "Rejeitado" da ADR-0024).
const INVISIBLES: &[char] = &[
    '\u{00AD}', // soft hyphen
    '\u{200B}', // zero-width space
    '\u{200C}', // zero-width non-joiner
    '\u{200D}', // zero-width joiner
    '\u{202A}', // bidi: LRE
    '\u{202B}', // bidi: RLE
    '\u{202C}', // bidi: PDF
    '\u{202D}', // bidi: LRO
    '\u{202E}', // bidi: RLO
    '\u{2060}', // word joiner
    '\u{2061}', // function application (invisível)
    '\u{2062}', // invisible times
    '\u{2063}', // invisible separator
    '\u{2064}', // invisible plus
    '\u{2066}', // bidi: LRI
    '\u{2067}', // bidi: RLI
    '\u{2068}', // bidi: FSI
    '\u{2069}', // bidi: PDI
    '\u{FEFF}', // BOM / zero-width no-break space
];

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
        // — including the quoted form that is the norm in source code
        // (`password = "..."`), since PR5 sends repository lines to the prompt.
        keyed: compile(
            r#"(?i)\b(pat|token|api[_-]?key|access[_-]?token|refresh[_-]?token|client[_-]?secret|secret|password|passwd|pwd|sig|signature)\b(\s*[=:]\s*)(["']?)([^\s&,;"']{3,})"#,
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

/// Remove os caracteres invisíveis do conjunto fechado [`INVISIBLES`].
///
/// Uma passada por `char`: segura em boundary multibyte (emoji e acentos
/// ficam intactos), preserva a ordem da entrada (determinismo do pack —
/// mesma entrada, mesma saída) e é idempotente por construção — o que sai
/// não volta e o que fica já não contém nada do conjunto.
fn strip_invisibles(text: &str) -> String {
    text.chars().filter(|c| !INVISIBLES.contains(c)).collect()
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
    // ADR-0024 (R1): a normalização vem ANTES do masking — os regex só
    // enxergam o texto visível, então zero-width dentro do shape não escapa
    // mais. O texto visível não muda: só Cf/bidi sai.
    let mut out = strip_invisibles(text);

    out = p
        .bearer
        .replace_all(&out, format!("${{1}}{MASKED}").as_str())
        .into_owned();
    out = p.known_prefixes.replace_all(&out, MASKED).into_owned();
    out = p
        .keyed
        .replace_all(&out, format!("${{1}}${{2}}${{3}}{MASKED}").as_str())
        .into_owned();
    out = p
        .url_query
        .replace_all(&out, format!("${{1}}{MASKED}").as_str())
        .into_owned();
    out = p.email.replace_all(&out, "[EMAIL]").into_owned();
    out = p.cpf.replace_all(&out, "[CPF]").into_owned();
    out
}

/// True when `text` contains something [`redact`] would mask or strip
/// (ADR-0024 R1: caracteres invisíveis contam — um texto que só os tem não
/// sai da fronteira intacto). A normalização acontece por herança, dentro
/// do `redact`: a comparação é contra o texto já saneado.
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
    fn masks_quoted_assignments_in_source_code() {
        let out = redact("let pat = \"AbC123XyZ789QqQ\";");
        assert!(!out.contains("AbC123XyZ789QqQ"), "{out}");
        assert!(out.contains("pat = \""), "a chave e a aspa ficam: {out}");
        assert!(out.contains(MASKED), "{out}");
        // Valor curto demais para ser segredo não é mascarado.
        assert_eq!(redact("let token = \"{}\";"), "let token = \"{}\";");
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

    // ===== W7.1 (ADR-0024 R1): payload com caracteres invisíveis =====
    // Payload novo só entra com fixture versionada (regra do W7) — as 5
    // famílias vazantes do probe de 12/Nov/2026 viram arquivos aqui.

    const FIX_ZWSP_TOKEN: &str = include_str!("../tests/fixtures/redact/zwsp_token.txt");
    const FIX_ZWNJ_TOKEN: &str = include_str!("../tests/fixtures/redact/zwnj_token.txt");
    const FIX_BIDI_OVERRIDE: &str = include_str!("../tests/fixtures/redact/bidi_override.txt");
    const FIX_BOM: &str = include_str!("../tests/fixtures/redact/bom.txt");
    const FIX_ZERO_WIDTH_BETWEEN: &str =
        include_str!("../tests/fixtures/redact/zero_width_between_chars.txt");
    const FIX_CLEAN_CONTROL: &str = include_str!("../tests/fixtures/redact/clean_control.txt");

    /// Fixture vazante -> segredo que NÃO pode sobreviver na saída.
    const LEAKING: &[(&str, &str)] = &[
        (FIX_ZWSP_TOKEN, "AbC123XyZ789QqQ"),
        (FIX_ZWNJ_TOKEN, "0123456789abcdefghij"),
        (FIX_BIDI_OVERRIDE, "eyJhbGciOiJIUzI1NiIsInR5cCI6"),
        (FIX_BOM, "AbC123XyZ789QqQ"),
        (FIX_ZERO_WIDTH_BETWEEN, "AbC123XyZ789QqQ"),
    ];

    #[test]
    fn masks_fixture_payloads_with_invisibles() {
        for (input, secret) in LEAKING {
            let out = redact(input);
            assert!(!out.contains(secret), "segredo vazou: {out:?}");
            assert!(out.contains(MASKED), "sem máscara: {out:?}");
            // Contrato do R1: a saída NÃO contém mais nenhum invisível.
            assert!(
                !out.chars().any(|c| INVISIBLES.contains(&c)),
                "invisível sobreviveu: {out:?}"
            );
            assert!(
                contains_secret(input),
                "contains_secret virou falso negativo: {input:?}"
            );
        }
    }

    #[test]
    fn clean_control_fixture_is_untouched() {
        // Controle negativo: texto limpo e sem invisível sai idêntico.
        assert_eq!(redact(FIX_CLEAN_CONTROL), FIX_CLEAN_CONTROL);
        assert!(!contains_secret(FIX_CLEAN_CONTROL));
    }

    #[test]
    fn is_idempotent_on_fixture_payloads() {
        for (input, _) in LEAKING {
            let once = redact(input);
            assert_eq!(once, redact(&once), "não idempotente para {input:?}");
        }
    }

    #[test]
    fn empty_and_only_invisible_collapse_to_empty() {
        assert_eq!(redact(""), "");
        assert_eq!(redact("\u{200B}\u{FEFF}\u{202E}"), "");
        assert!(!contains_secret(""));
    }

    #[test]
    fn multibyte_visible_text_survives() {
        // Sem invisíveis: nada muda — emoji, acento e travessão intactos.
        let text = "ação 🚀 já feita — repositório https://x.org/p?q=1";
        assert_eq!(redact(text), text);
        // Com invisíveis: só eles saem; o multibyte visível permanece.
        assert_eq!(redact("a\u{200B}ção \u{200D}🚀 já"), "ação 🚀 já");
    }

    #[test]
    fn masks_probe_email_and_cpf_variants() {
        // Vazamento parcial do probe: e-mail com zero-width no local-part
        // antes saía "serjo\u{200B}[EMAIL]" — o prefixo vazava.
        assert_eq!(redact("serjo\u{200B}ta@proton.me"), "[EMAIL]");
        // CPF dividido por invisível era vazante completa no probe.
        let cpf = redact("doc 123\u{200B}.456.789-01");
        assert!(!cpf.contains("456.789-01"), "{cpf}");
        assert!(cpf.contains("[CPF]"), "{cpf}");
    }

    #[test]
    fn does_not_touch_ids_and_numbers() {
        let text = "work item 529511, build 12345678901, versão 1.2.3";
        assert_eq!(redact(text), text);
    }
}
