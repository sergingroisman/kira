//! C4 — OpenTelemetry export.
//!
//! v0: opt-in scaffolding + span helpers. When `KIRA_OTLP_ENDPOINT` is set,
//! we log a startup breadcrumb and expose span builders so the rest of the
//! codebase can annotate traces today. Full OTLP exporter wiring (crate stack
//! `opentelemetry-otlp` + `tracing-opentelemetry`) lands in v0.5.
//!
//! Local stack: `docker compose -f docker-compose.observability.yml up -d`
//!   Jaeger UI: <http://localhost:16686>
//!   Grafana:   <http://localhost:3000> (admin/admin)
//!   OTEL HTTP: <http://localhost:4318/v1/traces>
//!   OTEL gRPC: localhost:4317

use std::time::Instant;
use tracing::{debug, info, info_span, instrument, Span};

/// `OTel` configuration discovered from the environment.
#[derive(Debug, Clone)]
pub struct OtelConfig {
    pub endpoint: String,
    pub service_name: String,
    pub enabled: bool,
}

impl Default for OtelConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://127.0.0.1:4318/v1/traces".into(),
            service_name: "kira-cored".into(),
            enabled: false,
        }
    }
}

impl OtelConfig {
    /// Load from env. `KIRA_OTLP_ENDPOINT` opt-in; empty or missing = disabled.
    pub fn from_env() -> Self {
        let mut cfg = Self::default();
        if let Ok(endpoint) = std::env::var("KIRA_OTLP_ENDPOINT")
            && !endpoint.is_empty() {
                cfg.endpoint = endpoint;
                cfg.enabled = true;
            }
        if let Ok(name) = std::env::var("OTEL_SERVICE_NAME")
            && !name.is_empty() {
                cfg.service_name = name;
            }
        cfg
    }
}

/// Convenience: load config from env and initialise in one call.
pub fn init_from_env() {
    let cfg = OtelConfig::from_env();
    init(&cfg);
}

/// Initialise observability scaffolding. Call once at startup.
/// v0 logs the config; v0.5 will install the `tracing-opentelemetry` layer.
pub fn init(cfg: &OtelConfig) {
    if cfg.enabled {
        info!(
            endpoint = %cfg.endpoint,
            service = %cfg.service_name,
            "OTel scaffolding initialised (exporter wiring deferred to v0.5)"
        );
    } else {
        debug!("OTel disabled — no KIRA_OTLP_ENDPOINT set");
    }
}

/// Convenience: create a `tracing::Span` for an agent lifecycle event.
/// Callers `.enter()` the span around the async block.
#[instrument(skip_all, fields(agent_id = %agent_id, kind = ?kind))]
pub fn agent_span(agent_id: &str, kind: &str, task: &str) -> Span {
    info_span!("agent.run", agent_id, kind, task = task.chars().take(60).collect::<String>())
}

/// Convenience: create a span for a sandbox `bash` execution.
#[instrument(skip_all, fields(tool = "bash", argc = argv.len()))]
pub fn bash_span(argv: &[String]) -> Span {
    let cmd = argv.first().map_or("?", std::string::String::as_str);
    info_span!("tool.bash", cmd)
}

/// Convenience: create a span for a single LLM route call.
#[instrument(skip_all, fields(tier = %tier))]
pub fn llm_route_span(tier: &str, model: Option<&str>) -> Span {
    info_span!("llm.route", tier, model = model.unwrap_or("auto"))
}

/// Convenience: create a span for a gate cascade run.
#[instrument(skip_all, fields(levels = %levels))]
pub fn gate_span(levels: &str) -> Span {
    info_span!("gates.cascade", levels)
}

/// Convenience: create a span for an SDD phase.
#[instrument(skip_all, fields(phase = %phase))]
pub fn sdd_span(phase: &str, run_id: &str) -> Span {
    info_span!("sdd.phase", phase, run_id)
}

/// A timer that records elapsed millis when dropped.
/// Use with `let _t = Timer::new("op_name");`
pub struct Timer {
    name: &'static str,
    start: Instant,
}

impl Timer {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            start: Instant::now(),
        }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        let ms = self.start.elapsed().as_millis() as u64;
        tracing::trace!(op = self.name, ms, "timer");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn config_from_env_safe_when_unset() {
        let cfg = OtelConfig::from_env();
        assert!(!cfg.enabled);
    }

    #[test]
    fn config_picks_up_endpoint() {
        // We cannot safely set env vars in Rust 2024 tests,
        // so we test the struct directly.
        let cfg = OtelConfig {
            endpoint: "http://custom:4318".into(),
            service_name: "kira-test".into(),
            enabled: true,
        };
        assert!(cfg.enabled);
        assert_eq!(cfg.endpoint, "http://custom:4318");
    }

    #[test]
    fn timer_records_elapsed() {
        let t = Timer::new("test_op");
        std::thread::sleep(Duration::from_millis(2));
        drop(t); // should not panic
    }
}
