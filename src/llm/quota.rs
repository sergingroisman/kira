//! Saúde do provedor de LLM (F2, PR17) — o watchdog 2 da spec W5.
//!
//! Três comportamentos, todos derivados de um único registro por backend na
//! tabela `provider_state` (schema v17):
//!
//! 1. **429 ⇒ 5 min de silêncio.** Depois de um rate limit, o KIRA não insiste:
//!    fica 5 minutos sem chamar aquele backend (ou o `retry-after` do servidor,
//!    se for maior — o servidor manda). Insistir em 429 só gasta tentativa e
//!    piora a fila do provedor.
//! 2. **Stale-ok.** Quando o provedor está em silêncio, o dado antigo continua
//!    VISÍVEL e marcado como `stale` — a resposta é "não sei agora, o último
//!    resultado bom foi há X", nunca um erro que apaga o que já se sabia.
//! 3. **Silêncio é por backend.** O Ollama local caindo não cala o Foundry:
//!    cada um tem sua linha, e um provedor novo nasce sem estado.
//!
//! O estado vive no store (não em memória) porque o ciclo roda em processos
//! separados (`kira loop`, `kira digest`, `kira triage`) — silêncio em memória
//! morreria com o processo e o próximo tick insistiria no 429.

use crate::error::Error;
use crate::store::Store;
use crate::store::{now_rfc3339_utc, rfc3339_utc_from_unix_seconds};

use crate::provider_health::{SILENCE_SECONDS, remaining_silence};

/// O erro é um rate limit (429)? Cobre o erro tipado do Foundry e o texto dos
/// outros backends — o Ollama devolve 429 como erro de HTTP genérico, e a
/// classificação por status é a única comum aos três.
#[must_use]
pub fn is_rate_limited(err: &Error) -> bool {
    matches!(err, Error::FoundryRateLimited(_)) || err.to_string().contains("429")
}

/// Nome estável do backend na tabela (`ollama`, `copilot`, `foundry`).
#[must_use]
pub fn backend_name(kind: crate::llm::LlmKind) -> &'static str {
    match kind {
        crate::llm::LlmKind::Ollama => "ollama",
        crate::llm::LlmKind::Copilot => "copilot",
        crate::llm::LlmKind::Foundry => "foundry",
    }
}

fn now_seconds() -> i64 {
    crate::scheduler::parse_unix_seconds(&now_rfc3339_utc()).unwrap_or(0)
}

/// Registra um sucesso: limpa o silêncio e o erro anterior.
pub fn record_success(store: &Store, backend: &str) -> Result<(), Error> {
    store.record_provider_success(backend, &now_rfc3339_utc())?;
    Ok(())
}

/// Registra um 429: silêncio de `retry_after` quando o servidor manda mais que
/// o piso de 5 min, senão o piso. Devolve quantos segundos ficou calado.
pub fn record_rate_limit(
    store: &Store,
    backend: &str,
    retry_after_seconds: Option<i64>,
    error: &str,
) -> Result<i64, Error> {
    let seconds = retry_after_seconds.unwrap_or(0).max(SILENCE_SECONDS);
    let now = now_seconds();
    let until = rfc3339_utc_from_unix_seconds(now + seconds);
    store.record_provider_cooldown(backend, &until, &crate::redact::redact(error))?;
    Ok(seconds)
}

/// A guarda antes da chamada: `Err(motivo)` quando o backend está em silêncio.
/// O motivo é escrito para virar nota do ciclo (nunca erro fatal: o ciclo
/// continua e apenas adia a fase).
pub fn silence_reason(store: &Store, backend: &str) -> Result<Option<String>, Error> {
    let state = store.provider_state(backend)?;
    let remaining = remaining_silence(state.as_ref(), now_seconds());
    if remaining == 0 {
        return Ok(None);
    }
    let minutes = (remaining + 59) / 60;
    Ok(Some(format!(
        "{backend} em silêncio por mais {minutes} min (429) — nada é chamado até lá; o último resultado bom continua visível (stale)"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;

    fn temp_store() -> (Store, std::path::PathBuf) {
        static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("kira-quota-unit-{}-{seq}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        (Store::open_in_dir(&dir).expect("store"), dir)
    }

    use crate::provider_health::status_line;
    use crate::store::ProviderState;

    #[test]
    fn no_state_means_no_silence() {
        assert_eq!(remaining_silence(None, 1_000), 0);
        let idle = ProviderState {
            backend: "ollama".to_string(),
            cooldown_until: None,
            last_ok_at: None,
            last_error: None,
            consecutive_failures: 0,
        };
        assert_eq!(remaining_silence(Some(&idle), 1_000), 0);
    }

    #[test]
    fn rate_limit_silences_for_five_minutes_and_success_clears_it() {
        let (store, dir) = temp_store();
        let silent =
            record_rate_limit(&store, "ollama", None, "HTTP 429 Too Many Requests").unwrap();
        assert_eq!(silent, SILENCE_SECONDS);

        let state = store.provider_state("ollama").unwrap().expect("estado");
        assert!(
            remaining_silence(Some(&state), now_seconds()) > 0,
            "429 tem de abrir silêncio"
        );
        // O erro guardado é redigido e legível.
        assert!(state.last_error.as_deref().unwrap().contains("429"));

        // A guarda explica o silêncio (vira nota do ciclo, não erro fatal).
        let reason = silence_reason(&store, "ollama").unwrap().expect("motivo");
        assert!(reason.contains("em silêncio"), "{reason}");
        assert!(reason.contains("stale"), "{reason}");

        record_success(&store, "ollama").unwrap();
        assert!(silence_reason(&store, "ollama").unwrap().is_none());
        let state = store.provider_state("ollama").unwrap().expect("estado");
        assert_eq!(state.consecutive_failures, 0);
        assert!(state.last_ok_at.is_some());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn retry_after_longer_than_the_floor_wins() {
        let (store, dir) = temp_store();
        let silent = record_rate_limit(&store, "foundry", Some(1_200), "cota esgotada").unwrap();
        assert_eq!(silent, 1_200, "o servidor manda quando pede mais que 5 min");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn silence_is_per_backend() {
        let (store, dir) = temp_store();
        record_rate_limit(&store, "foundry", None, "429").unwrap();
        assert!(silence_reason(&store, "foundry").unwrap().is_some());
        assert!(
            silence_reason(&store, "ollama").unwrap().is_none(),
            "um provedor calado não pode calar o outro"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn status_line_marks_stale_while_silent() {
        let (store, dir) = temp_store();
        record_success(&store, "ollama").unwrap();
        let state = store.provider_state("ollama").unwrap().expect("estado");
        let ok_line = status_line(Some(&state), now_seconds()).expect("linha");
        assert!(ok_line.contains("ok — último sucesso"), "{ok_line}");
        assert!(!ok_line.contains("[stale]"), "{ok_line}");

        record_rate_limit(&store, "ollama", None, "429").unwrap();
        let state = store.provider_state("ollama").unwrap().expect("estado");
        let stale_line = status_line(Some(&state), now_seconds()).expect("linha");
        assert!(stale_line.contains("[stale]"), "{stale_line}");
        let _ = std::fs::remove_dir_all(dir);
    }
}
