//! Ollama local health-check + first-run model bootstrap (H9, 2026-05-25).
//!
//! On daemon startup we probe `http://127.0.0.1:11434/api/tags`. Outcomes:
//!
//! * **Ollama reachable + target model installed** → log info; tier `local`
//!   is fully viable.
//! * **Ollama reachable + model missing** → spawn `ollama pull <model>` in
//!   the background. The tier stays available; the first call may either
//!   wait briefly or fall back via the FallbackChain.
//! * **Ollama unreachable** → log warn with install hint. Tier `local`
//!   remains configured but every call will fail and escalate (acceptable —
//!   the user just hasn't started Ollama yet).
//!
//! Fire-and-forget: we never block the daemon's startup on a network probe.

use std::time::Duration;
use tracing::{info, warn};

const OLLAMA_TAGS_URL: &str = "http://127.0.0.1:11434/api/tags";
const PROBE_TIMEOUT: Duration = Duration::from_millis(500);
pub const DEFAULT_LOCAL_MODEL: &str = "qwen2.5:3b-instruct-q4_K_M";

/// Schedule a non-blocking probe. Safe to call before `tokio::serve` — we
/// spawn the check on the runtime and return immediately.
pub fn schedule_probe() {
    tokio::spawn(async move {
        probe_and_bootstrap(DEFAULT_LOCAL_MODEL).await;
    });
}

async fn probe_and_bootstrap(target_model: &str) {
    let client = match reqwest::Client::builder()
        .timeout(PROBE_TIMEOUT)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, "ollama probe: reqwest client build failed — skipping");
            return;
        }
    };

    let resp = match client.get(OLLAMA_TAGS_URL).send().await {
        Ok(r) => r,
        Err(e) => {
            warn!(
                error = %e,
                "ollama not reachable at 127.0.0.1:11434 — `brew install ollama && ollama serve` \
                 to enable local tier. Tier=local calls will fail until Ollama is up."
            );
            return;
        }
    };

    let body = match resp.text().await {
        Ok(b) => b,
        Err(e) => {
            warn!(error = %e, "ollama probe: failed to read /api/tags body");
            return;
        }
    };

    if body.contains(target_model) {
        info!(model = target_model, "ollama ready — tier=local active");
        return;
    }

    // Model absent — try to pull in the background. We invoke the `ollama`
    // CLI rather than the HTTP pull API because the CLI streams progress to
    // its own stderr and handles resume on restart for free.
    info!(
        model = target_model,
        "ollama running but target model missing — starting `ollama pull` in background \
         (~2GB, first-run only)"
    );
    let model_owned = target_model.to_string();
    tokio::spawn(async move {
        match tokio::process::Command::new("ollama")
            .arg("pull")
            .arg(&model_owned)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await
        {
            Ok(s) if s.success() => {
                info!(model = %model_owned, "ollama pull complete — tier=local now ready");
            }
            Ok(s) => {
                warn!(model = %model_owned, status = ?s, "ollama pull exited non-zero");
            }
            Err(e) => {
                warn!(model = %model_owned, error = %e, "ollama pull failed to spawn");
            }
        }
    });
}
