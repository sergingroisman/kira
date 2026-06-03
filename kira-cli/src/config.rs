#![forbid(unsafe_code)]

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::path::PathBuf;

use crate::layers::config::LayersConfig;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default = "default_core_url")]
    pub core_url: String,

    #[serde(default)]
    pub time: TimeConfig,

    #[serde(default)]
    pub budget: BudgetConfig,

    #[serde(default)]
    pub paths: PathsConfig,

    #[serde(default)]
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TimeConfig {
    #[serde(default = "default_approval_timeout")]
    pub approval_timeout_secs: u64,

    #[serde(default = "default_health_timeout")]
    pub health_timeout_secs: u64,

    #[serde(default = "default_request_timeout")]
    pub request_timeout_secs: u64,

    #[serde(default = "default_health_interval")]
    pub health_interval_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BudgetConfig {
    pub monthly_hard_stop_usd: Option<f64>,

    #[serde(default = "default_warn_pct")]
    pub warn_pct: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PathsConfig {
    #[serde(default = "default_history_file")]
    pub history_file: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_toast_duration")]
    pub toast_duration_secs: u64,

    #[serde(default = "default_theme")]
    pub theme: String,

    #[serde(default = "default_scroll_lines_per_key")]
    pub scroll_lines_per_key: u16,

    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: u16,

    #[serde(default = "default_sidebar_auto_threshold")]
    pub sidebar_auto_threshold: u16,

    #[serde(default = "default_input_min_height")]
    pub input_min_height: u16,

    #[serde(default = "default_input_max_height")]
    pub input_max_height: u16,

    #[serde(default = "default_which_key_delay_ms")]
    pub which_key_delay_ms: u64,

    #[serde(default = "default_max_output_lines")]
    pub max_output_lines: usize,

    #[serde(default = "default_task_refresh_interval_secs")]
    pub task_refresh_interval_secs: u64,

    #[serde(default)]
    pub layers: LayersConfig,
}

fn default_core_url() -> String {
    std::env::var("KIRA_CORE_URL").unwrap_or_else(|_| "http://127.0.0.1:7777".to_string())
}
#[allow(clippy::missing_const_for_fn)]
fn default_approval_timeout() -> u64 {
    120
}
#[allow(clippy::missing_const_for_fn)]
fn default_health_timeout() -> u64 {
    5
}
#[allow(clippy::missing_const_for_fn)]
fn default_request_timeout() -> u64 {
    30
}
#[allow(clippy::missing_const_for_fn)]
fn default_health_interval() -> u64 {
    30
}
#[allow(clippy::missing_const_for_fn)]
fn default_warn_pct() -> f64 {
    80.0
}
fn default_history_file() -> String {
    dirs_next::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("~/.local/share"))
        .join("kira")
        .join("history")
        .to_string_lossy()
        .to_string()
}
#[allow(clippy::missing_const_for_fn)]
fn default_toast_duration() -> u64 {
    4
}
fn default_theme() -> String {
    "dark".to_string()
}
#[allow(clippy::missing_const_for_fn)]
fn default_scroll_lines_per_key() -> u16 {
    3
}
#[allow(clippy::missing_const_for_fn)]
fn default_sidebar_width() -> u16 {
    42
}
#[allow(clippy::missing_const_for_fn)]
fn default_sidebar_auto_threshold() -> u16 {
    120
}
#[allow(clippy::missing_const_for_fn)]
fn default_input_min_height() -> u16 {
    3
}
#[allow(clippy::missing_const_for_fn)]
fn default_input_max_height() -> u16 {
    10
}
#[allow(clippy::missing_const_for_fn)]
fn default_which_key_delay_ms() -> u64 {
    500
}
#[allow(clippy::missing_const_for_fn)]
fn default_max_output_lines() -> usize {
    10_000
}
#[allow(clippy::missing_const_for_fn)]
fn default_task_refresh_interval_secs() -> u64 {
    2
}

impl Default for TimeConfig {
    fn default() -> Self {
        Self {
            approval_timeout_secs: default_approval_timeout(),
            health_timeout_secs: default_health_timeout(),
            request_timeout_secs: default_request_timeout(),
            health_interval_secs: default_health_interval(),
        }
    }
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            monthly_hard_stop_usd: None,
            warn_pct: default_warn_pct(),
        }
    }
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            history_file: default_history_file(),
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            toast_duration_secs: default_toast_duration(),
            theme: default_theme(),
            scroll_lines_per_key: default_scroll_lines_per_key(),
            sidebar_width: default_sidebar_width(),
            sidebar_auto_threshold: default_sidebar_auto_threshold(),
            input_min_height: default_input_min_height(),
            input_max_height: default_input_max_height(),
            which_key_delay_ms: default_which_key_delay_ms(),
            max_output_lines: default_max_output_lines(),
            task_refresh_interval_secs: default_task_refresh_interval_secs(),
            layers: LayersConfig::default(),
        }
    }
}

impl Config {
    /// Load config from tokens.toml (cwd first, then ~/.config/kira/tokens.toml).
    /// Falls back to defaults if no file is found.
    /// Returns an error if any configured value falls outside its valid range.
    pub fn load(core_url_override: Option<&str>) -> Result<Self> {
        let mut cfg = Self::load_file()?;

        // CLI flag / env var takes precedence over file
        if let Some(url) = core_url_override {
            cfg.core_url = url.to_string();
        }

        cfg.validate()?;

        Ok(cfg)
    }

    /// Validate all configuration fields against their defined valid ranges.
    /// Returns `Err` naming the invalid field and the acceptable range.
    pub fn validate(&self) -> Result<()> {
        if !(1..=300).contains(&self.time.health_timeout_secs) {
            bail!(
                "invalid config: health_timeout_secs={} is outside valid range 1..=300",
                self.time.health_timeout_secs
            );
        }
        if !(10..=3600).contains(&self.time.approval_timeout_secs) {
            bail!(
                "invalid config: approval_timeout_secs={} is outside valid range 10..=3600",
                self.time.approval_timeout_secs
            );
        }
        if !(1..=300).contains(&self.time.request_timeout_secs) {
            bail!(
                "invalid config: request_timeout_secs={} is outside valid range 1..=300",
                self.time.request_timeout_secs
            );
        }
        if !(5..=600).contains(&self.time.health_interval_secs) {
            bail!(
                "invalid config: health_interval_secs={} is outside valid range 5..=600",
                self.time.health_interval_secs
            );
        }
        Ok(())
    }

    fn load_file() -> Result<Self> {
        let candidates = [
            PathBuf::from("tokens.toml"),
            dirs_next::config_dir()
                .unwrap_or_else(|| PathBuf::from("~/.config"))
                .join("kira")
                .join("tokens.toml"),
        ];

        for path in &candidates {
            if path.exists() {
                let content = std::fs::read_to_string(path)
                    .with_context(|| format!("reading {}", path.display()))?;
                let cfg: Self = toml::from_str(&content)
                    .with_context(|| format!("parsing {}", path.display()))?;
                return Ok(cfg);
            }
        }

        // No file found — use defaults
        tracing::warn!("tokens.toml not found; using defaults");
        Ok(Self::default())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            core_url: default_core_url(),
            time: TimeConfig::default(),
            budget: BudgetConfig::default(),
            paths: PathsConfig::default(),
            ui: UiConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // Feature: tui-wiring, Property 19: Config Range Validation
    // **Validates: Requirements 10.2, 10.4, 10.6**

    /// Helper to build a Config with specific time values for testing validation.
    fn config_with_time(
        health_timeout: u64,
        approval_timeout: u64,
        request_timeout: u64,
        health_interval: u64,
    ) -> Config {
        Config {
            core_url: "http://127.0.0.1:7777".to_string(),
            time: TimeConfig {
                health_timeout_secs: health_timeout,
                approval_timeout_secs: approval_timeout,
                request_timeout_secs: request_timeout,
                health_interval_secs: health_interval,
            },
            budget: BudgetConfig::default(),
            paths: PathsConfig::default(),
            ui: UiConfig::default(),
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property 19: Values inside valid ranges produce Ok from validate().
        #[test]
        fn config_valid_ranges_produce_ok(
            health_timeout in 1u64..=300,
            approval_timeout in 10u64..=600,
            request_timeout in 1u64..=300,
            health_interval in 5u64..=600,
        ) {
            let cfg = config_with_time(
                health_timeout,
                approval_timeout,
                request_timeout,
                health_interval,
            );
            prop_assert!(cfg.validate().is_ok());
        }

        /// Property 19: health_timeout_secs outside 1..=300 produces Err naming the field.
        #[test]
        fn config_invalid_health_timeout_produces_err(
            health_timeout in prop_oneof![0u64..1, 301u64..=u64::MAX],
        ) {
            let cfg = config_with_time(
                health_timeout,
                120, // valid
                30,  // valid
                30,  // valid
            );
            let err = cfg.validate().unwrap_err();
            let msg = err.to_string();
            prop_assert!(msg.contains("health_timeout_secs"), "Error should name the field, got: {}", msg);
        }

        /// Property 19: approval_timeout_secs outside 10..=3600 produces Err naming the field.
        #[test]
        fn config_invalid_approval_timeout_produces_err(
            approval_timeout in prop_oneof![0u64..10, 3601u64..=u64::MAX],
        ) {
            let cfg = config_with_time(
                5,   // valid
                approval_timeout,
                30,  // valid
                30,  // valid
            );
            let err = cfg.validate().unwrap_err();
            let msg = err.to_string();
            prop_assert!(msg.contains("approval_timeout_secs"), "Error should name the field, got: {}", msg);
        }

        /// Property 19: request_timeout_secs outside 1..=300 produces Err naming the field.
        #[test]
        fn config_invalid_request_timeout_produces_err(
            request_timeout in prop_oneof![0u64..1, 301u64..=u64::MAX],
        ) {
            let cfg = config_with_time(
                5,   // valid
                120, // valid
                request_timeout,
                30,  // valid
            );
            let err = cfg.validate().unwrap_err();
            let msg = err.to_string();
            prop_assert!(msg.contains("request_timeout_secs"), "Error should name the field, got: {}", msg);
        }

        /// Property 19: health_interval_secs outside 5..=600 produces Err naming the field.
        #[test]
        fn config_invalid_health_interval_produces_err(
            health_interval in prop_oneof![0u64..5, 601u64..=u64::MAX],
        ) {
            let cfg = config_with_time(
                5,   // valid
                120, // valid
                30,  // valid
                health_interval,
            );
            let err = cfg.validate().unwrap_err();
            let msg = err.to_string();
            prop_assert!(msg.contains("health_interval_secs"), "Error should name the field, got: {}", msg);
        }
    }
}
