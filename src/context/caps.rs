//! Token profile (PR40, onda TE): the ONE knob that picks where the packer
//! sits on the cost/quality curve.
//!
//! The profile coordinates the packer CAPS only — it never touches the window
//! ceiling (`KIRA_NUM_CTX` keeps owning `budget_chars`) and it never changes
//! *how* the pack is built (same priority order, same dedup bar, same tags).
//! Caps are a static struct per profile: same profile ⇒ same caps ⇒ same pack
//! (determinism M1 holds per profile).
//!
//! Resolution order (ADR house pattern, no env on purpose): explicit
//! `--profile` flag > `token_profile` key in `config.toml` > default
//! `balanced`. An invalid value is an ACTIONABLE error, never a silent
//! fallback: the owner chose a profile on purpose; guessing one back would
//! silently change the pack they asked for (the doc proposal said
//! `balanced` + `aviso` — the owner's instruction for PR40 tightened it to a
//! hard error listing the valid values).
//!
//! Default stays `balanced`, whose caps are exactly the consts that governed
//! the pack before PR40 — so every existing user keeps a byte-identical pack
//! without touching anything.

use crate::error::Error;

/// Chave do perfil em `config.toml` (`token_profile = "balanced"`).
pub const CONFIG_KEY_TOKEN_PROFILE: &str = "token_profile";

/// The three profiles. `balanced` is both the default and the exact
/// pre-PR40 behavior (regression gate: no user changes anything by accident).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenProfile {
    /// Lean pack: fewer excerpts, fewer pages, shorter excerpts (cheap runs).
    Budget,
    /// Today's behavior — the default.
    Balanced,
    /// Full pack: more excerpts, more pages, longer excerpts (cites more).
    Quality,
}

/// Valid values, in the order they are listed in error messages.
pub const TOKEN_PROFILE_VALUES: [&str; 3] = ["budget", "balanced", "quality"];

impl TokenProfile {
    /// Parses a config value or a flag value. The error message is the
    /// contract: it names the bad value and lists every valid one.
    pub fn parse(value: &str) -> Result<TokenProfile, Error> {
        match value.trim().to_ascii_lowercase().as_str() {
            "budget" => Ok(TokenProfile::Budget),
            "balanced" => Ok(TokenProfile::Balanced),
            "quality" => Ok(TokenProfile::Quality),
            _ => Err(Error::TokenProfile {
                value: value.trim().to_string(),
            }),
        }
    }

    /// `"budget"` | `"balanced"` | `"quality"` — shown by `kira doctor` and
    /// carried in the `--json` report so a golden can name its profile.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            TokenProfile::Budget => "budget",
            TokenProfile::Balanced => "balanced",
            TokenProfile::Quality => "quality",
        }
    }

    /// The packer caps of this profile (D10, proposta default consumida):
    /// `budget {4,2,400,2}` · `balanced {8,3,600,3}` (== antes do PR40) ·
    /// `quality {12,4,800,4}`.
    #[must_use]
    pub fn caps(self) -> Caps {
        match self {
            TokenProfile::Budget => Caps {
                code_excerpts: 4,
                pages: 2,
                excerpt_chars: 400,
                finding_lines: 2,
            },
            TokenProfile::Balanced => Caps {
                code_excerpts: 8,
                pages: 3,
                excerpt_chars: 600,
                finding_lines: 3,
            },
            TokenProfile::Quality => Caps {
                code_excerpts: 12,
                pages: 4,
                excerpt_chars: 800,
                finding_lines: 4,
            },
        }
    }
}

/// Static caps of the packer, per profile. A struct (not consts) because the
/// profile is resolved at runtime, but every value is a constant: same
/// profile ⇒ same caps, sem I/O, sem relógio — determinismo por perfil.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caps {
    /// How many repository excerpts the pack may carry (was `MAX_CODE_EXCERPTS`).
    pub code_excerpts: usize,
    /// How many wiki pages the pack may carry (was `MAX_PAGES`).
    pub pages: usize,
    /// Hard cap for one wiki excerpt, before the global budget (was
    /// `MAX_EXCERPT_CHARS`).
    pub excerpt_chars: usize,
    /// How many individual findings are spelled out (was `MAX_FINDING_LINES`).
    pub finding_lines: usize,
}

impl Default for Caps {
    /// `balanced` — the default profile's caps, byte-identical to the
    /// pre-PR40 consts, so a caller that cannot resolve a profile (tests,
    /// in-process callers) keeps today's behavior exactly.
    fn default() -> Caps {
        TokenProfile::Balanced.caps()
    }
}

/// Resolves the profile: explicit flag > `token_profile` in `config.toml` >
/// `balanced`. Sem env de propósito (justificativa no PR): o perfil é uma
/// escolha de dono, não um ajuste de ambiente — flag e config bastam, e uma
/// env escondida criaria um terceiro lugar para adivinhar.
///
/// Erros são acionáveis e interrompem o comando na camada de cima: um perfil
/// inválido é escolha errada, não degradação silenciosa.
pub fn resolve(flag: Option<&str>) -> Result<TokenProfile, Error> {
    let raw = flag.map(str::to_string).or_else(|| {
        crate::credentials::read_config().ok().and_then(|config| {
            config
                .iter()
                .find(|(key, _)| key == CONFIG_KEY_TOKEN_PROFILE)
                .map(|(_, value)| value.clone())
        })
    });
    match raw {
        None => Ok(TokenProfile::Balanced),
        Some(value) => TokenProfile::parse(&value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// D10: os três perfis, com os valores exatos da proposta consumida.
    #[test]
    fn caps_match_the_d10_table() {
        assert_eq!(
            TokenProfile::Budget.caps(),
            Caps {
                code_excerpts: 4,
                pages: 2,
                excerpt_chars: 400,
                finding_lines: 2,
            }
        );
        assert_eq!(
            TokenProfile::Balanced.caps(),
            Caps {
                code_excerpts: 8,
                pages: 3,
                excerpt_chars: 600,
                finding_lines: 3,
            }
        );
        assert_eq!(
            TokenProfile::Quality.caps(),
            Caps {
                code_excerpts: 12,
                pages: 4,
                excerpt_chars: 800,
                finding_lines: 4,
            }
        );
        // O default de `Caps` é o balanced — é o que mantém o pack de hoje.
        assert_eq!(Caps::default(), TokenProfile::Balanced.caps());
    }

    /// Parse aceita os três valores (e espaços/caixa não quebram o contrato).
    #[test]
    fn parse_accepts_the_three_values_and_normalizes() {
        for value in TOKEN_PROFILE_VALUES {
            assert!(TokenProfile::parse(value).is_ok(), "{value}");
        }
        assert_eq!(
            TokenProfile::parse(" Balanced ").unwrap_or_else(|err| panic!("{err}")),
            TokenProfile::Balanced
        );
    }

    /// Aceite (c) do PR40: valor inválido é erro ACIONÁVEL com a lista dos
    /// válidos — nunca fallback silencioso.
    #[test]
    fn parse_rejects_an_invalid_value_with_the_valid_list() {
        let err = TokenProfile::parse("esquisito").unwrap_err();
        let message = err.to_string();
        assert!(message.contains("esquisito"), "{message}");
        assert!(
            message.contains("budget")
                && message.contains("balanced")
                && message.contains("quality"),
            "lista de válidos no erro: {message}"
        );
    }
}
