//! Saúde do provedor de LLM (F2, PR17) — a parte PURA, fora da camada `llm/`.
//!
//! Aqui mora só a aritmética do silêncio pós-429 e o texto que o `doctor`
//! mostra. A camada `llm/` é a única que decide chamar modelo; o `doctor` só
//! LÊ o estado gravado, e por isso não pode (nem deve) importar `crate::llm` —
//! é o que o `tests/llm_allowlist.rs` cobra (ADR-0016). Manter a conta aqui é o
//! que permite o diagnóstico mostrar `stale` sem virar um segundo decisor.

use crate::store::ProviderState;

/// Silêncio mínimo após um 429, em segundos (spec W5, watchdog 2).
pub const SILENCE_SECONDS: i64 = 300;

fn parse(stamp: &str) -> Option<i64> {
    crate::scheduler::parse_unix_seconds(stamp)
}

/// Quanto falta de silêncio, em segundos. `0` = pode chamar.
///
/// Carimbo ilegível devolve `0` de propósito: silêncio eterno travaria o ciclo
/// por causa de uma linha corrompida, e a próxima chamada reescreve o estado.
#[must_use]
pub fn remaining_silence(state: Option<&ProviderState>, now: i64) -> i64 {
    let Some(state) = state else {
        return 0;
    };
    let Some(until) = state.cooldown_until.as_deref() else {
        return 0;
    };
    match parse(until) {
        Some(until) => (until - now).max(0),
        None => 0,
    }
}

fn minutes(seconds: i64) -> i64 {
    (seconds + 59) / 60
}

/// Linha de saúde do backend para o `doctor` — `[stale]` quando o provedor
/// está calado. `None` = backend nunca chamado (nada a dizer).
#[must_use]
pub fn status_line(state: Option<&ProviderState>, now: i64) -> Option<String> {
    let state = state?;
    let remaining = remaining_silence(Some(state), now);
    let last_ok = state
        .last_ok_at
        .as_deref()
        .and_then(parse)
        .map(|ok| {
            let age = (now - ok).max(0);
            if age >= 86_400 {
                format!("há {}d", age / 86_400)
            } else if age >= 3_600 {
                format!("há {}h", age / 3_600)
            } else {
                format!("há {}min", (age / 60).max(1))
            }
        })
        .unwrap_or_else(|| "nunca".to_string());
    if remaining > 0 {
        return Some(format!(
            "{}: em silêncio por mais {} min (429) — último sucesso {last_ok} [stale]",
            state.backend,
            minutes(remaining)
        ));
    }
    Some(format!("{}: ok — último sucesso {last_ok}", state.backend))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(cooldown: Option<&str>, last_ok: Option<&str>) -> ProviderState {
        ProviderState {
            backend: "ollama".to_string(),
            cooldown_until: cooldown.map(ToString::to_string),
            last_ok_at: last_ok.map(ToString::to_string),
            last_error: None,
            consecutive_failures: 0,
        }
    }

    #[test]
    fn no_state_and_no_cooldown_mean_no_silence() {
        assert_eq!(remaining_silence(None, 1_000), 0);
        assert_eq!(remaining_silence(Some(&state(None, None)), 1_000), 0);
    }

    #[test]
    fn cooldown_in_the_future_counts_down_and_clamps_at_zero() {
        let row = state(Some("2026-11-16T13:05:00Z"), None);
        let start = parse("2026-11-16T13:00:00Z").unwrap();
        assert_eq!(remaining_silence(Some(&row), start), 300);
        assert_eq!(remaining_silence(Some(&row), start + 120), 180);
        assert_eq!(remaining_silence(Some(&row), start + 3_600), 0);
    }

    #[test]
    fn unreadable_stamp_never_locks_the_cycle() {
        let row = state(Some("ontem às 5"), None);
        assert_eq!(remaining_silence(Some(&row), 1_000), 0);
    }

    #[test]
    fn status_line_marks_stale_only_while_silent() {
        let now = parse("2026-11-16T13:10:00Z").unwrap();
        let ok = state(None, Some("2026-11-16T13:05:00Z"));
        let line = status_line(Some(&ok), now).expect("linha");
        assert!(line.contains("ok — último sucesso há 5min"), "{line}");
        assert!(!line.contains("[stale]"), "{line}");

        let silent = state(Some("2026-11-16T13:12:00Z"), Some("2026-11-16T13:05:00Z"));
        let line = status_line(Some(&silent), now).expect("linha");
        assert!(line.contains("em silêncio por mais 2 min"), "{line}");
        assert!(line.contains("[stale]"), "{line}");

        // Sem sucesso registrado, o texto diz "nunca" em vez de inventar idade.
        let never = status_line(Some(&state(None, None)), now).expect("linha");
        assert!(never.contains("último sucesso nunca"), "{never}");
    }
}
