//! Config loader — reads `tokens.toml` at startup.
//!
//! Falls back to hardcoded defaults if the file is missing or malformed.
//! v0.5+: watches the file and hot-reloads.

pub mod runtime;

use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ─── Root config ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct Tokens {
    pub time: TimeConfig,
    pub budget: BudgetConfig,
    pub concurrency: ConcurrencyConfig,
    pub thresholds: ThresholdConfig,
    pub providers: ProviderConfig,
    pub tiers: TiersConfig,
    pub pricing: PricingConfig,
    pub models: ModelsConfig,
    pub paths: PathConfig,
    pub bind: BindConfig,
    pub embedding: EmbeddingConfig,
    pub safety: SafetyConfig,
    pub text_pipeline: TextPipelineConfig,
    #[serde(default)]
    pub web_tools: WebToolsConfig,
}

// ─── Web Tools (P0 + SearXNG + readability) ──────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct WebToolsConfig {
    #[serde(default = "default_searxng_url")]
    pub searxng_base_url: String,
    #[serde(default = "default_search_timeout")]
    pub search_timeout_secs: u64,
    #[serde(default = "default_search_max_results")]
    pub search_max_results: u32,
    #[serde(default = "default_search_lang")]
    pub search_default_lang: String,
    #[serde(default = "default_fetch_timeout")]
    pub fetch_timeout_secs: u64,
    #[serde(default = "default_fetch_max_bytes")]
    pub fetch_max_bytes: usize,
    #[serde(default = "default_fetch_max_redirects")]
    pub fetch_max_redirects: u32,
    #[serde(default = "default_jina_enabled")]
    pub jina_fallback_enabled: bool,
    #[serde(default = "default_jina_url")]
    pub jina_base_url: String,
    #[serde(default = "default_web_calls_per_min")]
    pub web_calls_per_min: u32,
}

impl Default for WebToolsConfig {
    fn default() -> Self {
        Self {
            searxng_base_url: default_searxng_url(),
            search_timeout_secs: default_search_timeout(),
            search_max_results: default_search_max_results(),
            search_default_lang: default_search_lang(),
            fetch_timeout_secs: default_fetch_timeout(),
            fetch_max_bytes: default_fetch_max_bytes(),
            fetch_max_redirects: default_fetch_max_redirects(),
            jina_fallback_enabled: default_jina_enabled(),
            jina_base_url: default_jina_url(),
            web_calls_per_min: default_web_calls_per_min(),
        }
    }
}

fn default_searxng_url() -> String { "http://127.0.0.1:8888".into() }
const fn default_search_timeout() -> u64 { 15 }
const fn default_search_max_results() -> u32 { 5 }
fn default_search_lang() -> String { "pt-BR".into() }
const fn default_fetch_timeout() -> u64 { 20 }
const fn default_fetch_max_bytes() -> usize { 5 * 1024 * 1024 }
const fn default_fetch_max_redirects() -> u32 { 3 }
const fn default_jina_enabled() -> bool { true }
fn default_jina_url() -> String { "https://r.jina.ai/".into() }
const fn default_web_calls_per_min() -> u32 { 30 }

// ─── Time ────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct TimeConfig {
    #[serde(default = "default_tool_timeout")]
    pub tool_timeout_default: u64,
    #[serde(default = "default_bash_timeout")]
    pub bash_timeout_max: u64,
    #[serde(default = "default_hook_timeout")]
    pub hook_timeout_default: u64,
    #[serde(default = "default_agent_timeout")]
    pub agent_timeout_default: u64,
    #[serde(default = "default_heartbeat_interval")]
    pub agent_heartbeat_interval: u64,
    #[serde(default = "default_stale_minutes")]
    pub agent_stale_after_minutes: u64,
    #[serde(default = "default_plan_ttl")]
    pub plan_ttl_default: u64,
    #[serde(default = "default_cleanup_minutes")]
    pub worktree_cleanup_after_min: u64,
    #[serde(default = "default_breaker_window")]
    pub breaker_window: u64,
    #[serde(default = "default_breaker_open")]
    pub breaker_open_duration: u64,
    #[serde(default = "default_heartbeat_scanner")]
    pub heartbeat_scanner_period: u64,
    #[serde(default = "default_sse_keepalive")]
    pub sse_keepalive: u64,
    #[serde(default = "default_eval_poll_ms")]
    pub eval_poll_interval_ms: u64,
    #[serde(default = "default_eval_timeout")]
    pub eval_default_timeout: u64,
    #[serde(default = "default_rate_limit")]
    pub session_rate_limit_per_min: u32,
}

// ─── Budget ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct BudgetConfig {
    #[serde(default = "default_hard_stop")]
    pub monthly_hard_stop_usd: f64,
    #[serde(default = "default_warn_pct")]
    pub warn_pct: f64,
}

// ─── Concurrency ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct ConcurrencyConfig {
    #[serde(default = "default_max_worktrees")]
    pub max_worktrees: usize,
    #[serde(default = "default_max_agent_iters")]
    pub max_agent_iters: u32,
    #[serde(default = "default_max_agent_iters_quick")]
    pub max_agent_iters_quick: u32,
    #[serde(default = "default_max_agent_iters_small")]
    pub max_agent_iters_small: u32,
    #[serde(default = "default_max_agent_iters_complex")]
    pub max_agent_iters_complex: u32,
    #[serde(default = "default_breaker_threshold")]
    pub breaker_failure_threshold: u32,
    #[serde(default = "default_stall_threshold")]
    pub stall_same_tool_threshold: usize,
    /// Maximum agents that may run concurrently (across all worktrees).
    /// Excess spawns block on an `Arc<Semaphore>` until a slot frees.
    #[serde(default = "default_max_agents")]
    pub max_agents: usize,
}

// ─── Thresholds ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct ThresholdConfig {
    #[serde(default = "default_faithfulness_fail")]
    pub faithfulness_pass_below: f64,
    #[serde(default = "default_faithfulness_escalate")]
    pub faithfulness_escalate: f64,
    #[serde(default = "default_judge_pass")]
    pub judge_pass_score: f64,
    #[serde(default = "default_change_suff_min")]
    pub change_sufficiency_min: f64,
    #[serde(default = "default_change_suff_max")]
    pub change_sufficiency_max: f64,
    #[serde(default = "default_lesson_promote")]
    pub lesson_promote_count: u32,
    #[serde(default = "default_inject_top_n")]
    pub inject_top_n: usize,
    #[serde(default = "default_embed_cache_max")]
    pub embed_cache_max_entries: u64,
    #[serde(default = "default_meta_review_coverage")]
    pub meta_review_coverage: f64,
}

// ─── Providers ───────────────────────────────────────────────────────────────

/// Map of provider names to their configuration entries.
/// In TOML: `[providers.deepseek]`, `[providers.openai]`, etc.
#[derive(Debug, Deserialize, Clone)]
pub struct ProviderConfig {
    #[serde(flatten)]
    pub entries: HashMap<String, ProviderEntry>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ProviderEntry {
    pub base_url: Option<String>,
    pub api_key_env: Option<String>,
    /// Wire protocol: `"openai-compatible"` (default) or `"anthropic"`.
    #[serde(default)]
    pub protocol: Option<String>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub priority: u8,
    /// Anthropic-only: `max_tokens` for /v1/messages requests.
    #[serde(default = "default_anthropic_max_tokens")]
    pub max_tokens: u32,
    /// Anthropic-only: API version header (e.g. "2023-06-01").
    #[serde(default = "default_anthropic_api_version")]
    pub api_version: String,
}

// ─── Tiers (config‑driven candidate lists) ───────────────────────────────────

/// Tier names mapped to ordered candidate lists of `"provider:model"` strings.
#[derive(Debug, Deserialize, Clone)]
pub struct TiersConfig {
    pub heavy: TierEntry,
    pub standard: TierEntry,
    pub light: TierEntry,
    pub long_context: TierEntry,
    /// Dedicated judge tier (P3). Optional in TOML — defaults to a list
    /// disjoint from `heavy` so `judge_model ≠ writer_model` holds by default.
    #[serde(default)]
    pub judge: Option<TierEntry>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TierEntry {
    pub candidates: Vec<String>,
}

// ─── Models ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct ModelConfig {
    pub heavy: ModelTier,
    pub standard: ModelTier,
    pub light: ModelTier,
    pub long_context: ModelTier,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ModelTier {
    pub primary: String,
    pub fallback_1: Option<String>,
    pub fallback_2: Option<String>,
}

// ─── Pricing ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone, Copy, Default)]
pub struct PricingConfig {
    #[serde(rename = "qwen-max-input")]
    pub qwen_max_input: f64,
    #[serde(rename = "qwen-max-output")]
    pub qwen_max_output: f64,
    #[serde(rename = "qwen-plus-input")]
    pub qwen_plus_input: f64,
    #[serde(rename = "qwen-plus-output")]
    pub qwen_plus_output: f64,
    #[serde(rename = "qwen-flash-input")]
    pub qwen_flash_input: f64,
    #[serde(rename = "qwen-flash-output")]
    pub qwen_flash_output: f64,
    #[serde(rename = "deepseek-v4-pro-input")]
    pub deepseek_v4_pro_input: f64,
    #[serde(rename = "deepseek-v4-pro-output")]
    pub deepseek_v4_pro_output: f64,
    #[serde(rename = "deepseek-v4-flash-input")]
    pub deepseek_v4_flash_input: f64,
    #[serde(rename = "deepseek-v4-flash-output")]
    pub deepseek_v4_flash_output: f64,
}

impl PricingConfig {
    /// Return (`input_price`, `output_price`) per 1M tokens for a model name.
    /// Returns (0.0, 0.0) for unknown models.
    pub fn price_for(&self, model: &str) -> (f64, f64) {
        match model {
            "qwen-max" => (self.qwen_max_input, self.qwen_max_output),
            "qwen-plus" => (self.qwen_plus_input, self.qwen_plus_output),
            "qwen-flash" => (self.qwen_flash_input, self.qwen_flash_output),
            "deepseek-v4-pro" => (self.deepseek_v4_pro_input, self.deepseek_v4_pro_output),
            "deepseek-v4-flash" => (self.deepseek_v4_flash_input, self.deepseek_v4_flash_output),
            _ => (0.0, 0.0),
        }
    }
}

// ─── Models (context limits etc.) ──────────────────────────────────────────

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ModelsConfig {
    #[serde(flatten)]
    pub entries: HashMap<String, ModelEntry>,
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct ModelEntry {
    #[serde(default = "default_context_limit")]
    pub context_limit: usize,
}

const fn default_context_limit() -> usize { 131_072 }

// ─── Paths ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct PathConfig {
    #[serde(default = "default_state_db_mac")]
    pub state_db_mac: String,
    #[serde(default = "default_state_db_linux")]
    pub state_db_linux: String,
    #[serde(default = "default_worktrees_subdir")]
    pub worktrees_subdir: String,
    #[serde(default = "default_mailbox_subdir")]
    pub mailbox_subdir: String,
    #[serde(default = "default_runs_subdir")]
    pub runs_subdir: String,
    #[serde(default = "default_prompts_dir")]
    pub prompts_dir: String,
    #[serde(default = "default_workflows_dir")]
    pub workflows_dir: String,
    #[serde(default = "default_branch_prefix")]
    pub branch_prefix: String,
    #[serde(default = "default_wt_prefix")]
    pub wt_prefix: String,
}

// ─── Bind ───────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct BindConfig {
    #[serde(default = "default_bind_host")]
    pub host_dev: String,
    #[serde(default = "default_bind_port")]
    pub port_default: u16,
}

// ─── Embedding ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct EmbeddingConfig {
    #[serde(default = "default_embed_model")]
    pub model: String,
}

// ─── Safety ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct SafetyConfig {
    #[serde(default)]
    pub prompt_injection_keywords: Vec<String>,
    #[serde(default)]
    pub bash_executable_allowlist: Vec<String>,
    #[serde(default)]
    pub bash_forbidden_chars: Vec<String>,
    #[serde(default = "default_file_max_bytes")]
    pub file_read_max_bytes: usize,
    #[serde(default = "default_file_max_bytes")]
    pub file_write_max_bytes: usize,
}

// ─── Text Pipeline ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct TextPipelineConfig {
    #[serde(default = "default_chunker_target")]
    pub chunker_target_tokens: u32,
    #[serde(default = "default_chunker_overlap")]
    pub chunker_overlap_tokens: u32,
    #[serde(default)]
    pub preserve_entities: bool,
    #[serde(default = "default_rerank_top_k")]
    pub rerank_top_k: u32,
}

// ─── Defaults ────────────────────────────────────────────────────────────────

const fn default_tool_timeout() -> u64 { 30 }
const fn default_bash_timeout() -> u64 { 300 }
const fn default_hook_timeout() -> u64 { 5 }
const fn default_agent_timeout() -> u64 { 3600 }
const fn default_heartbeat_interval() -> u64 { 30 }
const fn default_stale_minutes() -> u64 { 5 }
const fn default_plan_ttl() -> u64 { 3600 }
const fn default_cleanup_minutes() -> u64 { 10 }
const fn default_breaker_window() -> u64 { 60 }
const fn default_breaker_open() -> u64 { 30 }
const fn default_heartbeat_scanner() -> u64 { 60 }
const fn default_sse_keepalive() -> u64 { 15 }
const fn default_eval_poll_ms() -> u64 { 500 }
const fn default_eval_timeout() -> u64 { 300 }
const fn default_rate_limit() -> u32 { 30 }
const fn default_hard_stop() -> f64 { 50.00 }
const fn default_warn_pct() -> f64 { 80.0 }
const fn default_max_worktrees() -> usize { 2 }
const fn default_max_agent_iters() -> u32 { 10 }
const fn default_max_agent_iters_quick() -> u32 { 3 }
const fn default_max_agent_iters_small() -> u32 { 5 }
const fn default_max_agent_iters_complex() -> u32 { 20 }
const fn default_breaker_threshold() -> u32 { 5 }
const fn default_stall_threshold() -> usize { 3 }
const fn default_max_agents() -> usize { 4 }
const fn default_faithfulness_fail() -> f64 { 0.55 }
const fn default_faithfulness_escalate() -> f64 { 0.80 }
const fn default_judge_pass() -> f64 { 7.0 }
const fn default_change_suff_min() -> f64 { 0.3 }
const fn default_change_suff_max() -> f64 { 3.0 }
const fn default_lesson_promote() -> u32 { 5 }
const fn default_inject_top_n() -> usize { 5 }
const fn default_embed_cache_max() -> u64 { 5000 }
const fn default_meta_review_coverage() -> f64 { 100.0 }
fn default_state_db_mac() -> String { "~/.local/share/kira/state.redb".into() }
fn default_state_db_linux() -> String { "/var/lib/kira/state.redb".into() }
fn default_worktrees_subdir() -> String { ".worktrees".into() }
fn default_mailbox_subdir() -> String { ".kira/mailbox".into() }
fn default_runs_subdir() -> String { ".kira/runs".into() }
fn default_prompts_dir() -> String { "core/agents/prompts".into() }
fn default_workflows_dir() -> String { ".kira/workflows".into() }
fn default_branch_prefix() -> String { "kira/".into() }
fn default_wt_prefix() -> String { "wt-".into() }
fn default_bind_host() -> String { "127.0.0.1".into() }
const fn default_bind_port() -> u16 { 7777 }
fn default_embed_model() -> String { "text-embedding-v3".into() }
const fn default_file_max_bytes() -> usize { 5 * 1024 * 1024 }
const fn default_chunker_target() -> u32 { 512 }
const fn default_chunker_overlap() -> u32 { 50 }
const fn default_rerank_top_k() -> u32 { 5 }
const fn default_anthropic_max_tokens() -> u32 { 4096 }
fn default_anthropic_api_version() -> String { "2023-06-01".into() }

// ─── Loader ──────────────────────────────────────────────────────────────────

/// Load `tokens.toml` from the given path (absolute or relative to CWD).
/// Falls back to all defaults if the file doesn't exist.
pub fn load_tokens(path: &Path) -> anyhow::Result<Tokens> {
    let content = std::fs::read_to_string(path)?;
    let tokens: Tokens = toml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("failed to parse tokens.toml: {e}"))?;
    Ok(tokens)
}

/// Resolve home path — replaces leading `~` with `$HOME`.
pub fn resolve_home(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        PathBuf::from(home).join(&path[2..])
    } else {
        PathBuf::from(path)
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_compile() {
        let _ = default_tool_timeout();
        let _ = default_hard_stop();
        let _ = default_faithfulness_fail();
    }

    #[test]
    fn resolve_home_works() {
        let p = resolve_home("~/.local/share/test");
        assert!(!p.starts_with("~"));
        assert!(p.to_string_lossy().contains(".local"));
    }
}
