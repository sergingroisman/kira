//! Config loader — reads `tokens.toml` at startup.
//!
//! Falls back to hardcoded defaults if the file is missing or malformed.
//! v0.5+: watches the file and hot-reloads.

pub mod reload;
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
    #[serde(default)]
    pub memory: MemoryConfig,
    #[serde(default)]
    pub queue: QueueConfig,
    #[serde(default)]
    pub waves: WavesConfig,
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
    /// Max chars of extracted markdown returned to the LLM per fetch call.
    /// Prevents single large page from bloating context window.
    /// ~4000 chars ≈ 1000 tokens — enough for most articles.
    #[serde(default = "default_fetch_output_max_chars")]
    pub fetch_output_max_chars: usize,

    /// Search provider for `web_search`. v0.5 default = `brave` (Rust-only,
    /// no Docker needed). SearXNG remains available for v0.6 deploy when
    /// the operator wants sovereign self-host. Workshop SPEKTOR P5:
    /// "guardrails sensatos por default" — Brave avoids exposing the agent
    /// to scraping anti-bot or unstable HTML changes.
    #[serde(default = "default_search_provider")]
    pub search_provider: String,
    /// Ordered fallback chain for `web_search`. The first provider that
    /// returns results wins; failures (network/quota/empty) fall through to
    /// the next. Default: Brave (fast, free quota) → SearXNG (self-host,
    /// unlimited) → DuckDuckGo (zero-key last resort). When empty, falls back
    /// to the single `search_provider` (legacy behaviour).
    #[serde(default = "default_search_chain")]
    pub search_chain: Vec<String>,
    /// Brave Search API key env var name (read from process env at call
    /// time so the key never lands in tokens.toml). Required when
    /// `search_provider = "brave"`.
    #[serde(default = "default_brave_api_key_env")]
    pub brave_api_key_env: String,
    /// Brave Search API base URL. Override only for testing.
    #[serde(default = "default_brave_base_url")]
    pub brave_base_url: String,

    /// Respect `robots.txt` before issuing `web_fetch`. Default ON
    /// (workshop SPEKTOR P5 guardrails + ethical scraping). Set false only
    /// for whitelisted internal hosts.
    #[serde(default = "default_respect_robots")]
    pub respect_robots: bool,
    /// User-agent string used for both `web_fetch` requests and the
    /// `robots.txt` allowlist match. Identifying the bot lets sites tune
    /// their rules per-agent (texting_robots matches on prefix).
    #[serde(default = "default_user_agent")]
    pub user_agent: String,
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
            fetch_output_max_chars: default_fetch_output_max_chars(),
            search_provider: default_search_provider(),
            search_chain: default_search_chain(),
            brave_api_key_env: default_brave_api_key_env(),
            brave_base_url: default_brave_base_url(),
            respect_robots: default_respect_robots(),
            user_agent: default_user_agent(),
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
const fn default_fetch_output_max_chars() -> usize { 4_000 }
fn default_search_provider() -> String { "brave".into() }
fn default_search_chain() -> Vec<String> {
    // Brave (fast, free 2000/mo) → SearXNG (self-host, unlimited). DuckDuckGo
    // dropped from the default: its HTML endpoint returns a 202 anti-bot
    // challenge to non-browser clients, so it never yields results and only
    // adds latency. Still selectable via `search_chain = [..., "duckduckgo"]`.
    vec!["brave".into(), "searxng".into()]
}
fn default_brave_api_key_env() -> String { "BRAVE_API_KEY".into() }
fn default_brave_base_url() -> String { "https://api.search.brave.com/res/v1/web/search".into() }
const fn default_respect_robots() -> bool { true }
fn default_user_agent() -> String { "KiraBot/0.5 (+https://github.com/sergingroisman/kira)".into() }

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
    /// Seconds to wait for human approval of a HIGH-risk tool before auto-rejecting.
    #[serde(default = "default_approval_timeout")]
    pub approval_timeout_secs: u64,
}

// ─── Budget ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct BudgetConfig {
    #[serde(default = "default_hard_stop")]
    pub monthly_hard_stop_usd: f64,
    #[serde(default = "default_warn_pct")]
    pub warn_pct: f64,
    /// Per-agent budget cap in USD (AP11). Used when `SpawnAgentRequest`
    /// omits an explicit `budget_usd`. Workshop SPEKTOR P5 guardrail:
    /// "esgotou: para. Sem isso, agente entra em loop e queima R$ 200 em
    /// dez minutos." Defaults to $1 — generous for chat, tight for
    /// runaway loops.
    #[serde(default = "default_agent_default_usd")]
    pub agent_default_usd: f64,
}

// ─── Concurrency ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct ConcurrencyConfig {
    #[serde(default = "default_max_worktrees")]
    pub max_worktrees: usize,
    #[serde(default = "default_max_agent_iters")]
    pub max_agent_iters: u32,
    #[serde(default = "default_max_agent_iters_yolo")]
    pub max_agent_iters_yolo: u32,
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
    /// Total HTTP request timeout in seconds (connect + response body).
    /// Reviewer/SDD phases with large context need >60s. Default: 300s.
    #[serde(default = "default_llm_timeout_secs")]
    pub llm_timeout_secs: u64,
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
    /// Local-only tier (Ollama M1). No remote fallback — controlled cost.
    /// Used for conceptual chat / dedup. Optional; empty = unavailable.
    #[serde(default)]
    pub local: Option<TierEntry>,
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
    #[serde(rename = "deepseek-v4-pro-input", default)]
    pub deepseek_v4_pro_input: f64,
    #[serde(rename = "deepseek-v4-pro-output", default)]
    pub deepseek_v4_pro_output: f64,
    #[serde(rename = "deepseek-v4-flash-input", default)]
    pub deepseek_v4_flash_input: f64,
    #[serde(rename = "deepseek-v4-flash-output", default)]
    pub deepseek_v4_flash_output: f64,
    #[serde(rename = "gpt-4o-input", default)]
    pub gpt4o_input: f64,
    #[serde(rename = "gpt-4o-output", default)]
    pub gpt4o_output: f64,
    #[serde(rename = "gpt-4o-mini-input", default)]
    pub gpt4o_mini_input: f64,
    #[serde(rename = "gpt-4o-mini-output", default)]
    pub gpt4o_mini_output: f64,
}

impl PricingConfig {
    /// Return (`input_price`, `output_price`) per 1M tokens for a model name.
    /// Returns (0.0, 0.0) for unknown models.
    pub fn price_for(&self, model: &str) -> (f64, f64) {
        match model {
            "deepseek-v4-pro" => (self.deepseek_v4_pro_input, self.deepseek_v4_pro_output),
            "deepseek-v4-flash" => (self.deepseek_v4_flash_input, self.deepseek_v4_flash_output),
            "gpt-4o" => (self.gpt4o_input, self.gpt4o_output),
            "gpt-4o-mini" => (self.gpt4o_mini_input, self.gpt4o_mini_output),
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
    #[serde(default = "default_prompt_injection_levenshtein")]
    pub prompt_injection_levenshtein: usize,
    #[serde(default)]
    pub bash_executable_allowlist: Vec<String>,
    #[serde(default)]
    pub bash_forbidden_chars: Vec<String>,
    #[serde(default = "default_file_max_bytes")]
    pub file_read_max_bytes: usize,
    #[serde(default = "default_file_max_bytes")]
    pub file_write_max_bytes: usize,
    /// Whether PII redaction is enabled in the Input Guardrail.
    #[serde(default = "default_true")]
    pub pii_redaction_enabled: bool,
    /// Words that trigger immediate block in the Input Guardrail.
    #[serde(default)]
    pub forbidden_words: Vec<String>,
    /// Whether the Output Guardrail (LLM-as-Judge) is enabled.
    #[serde(default = "default_true")]
    pub output_guardrail_enabled: bool,
    /// Max bytes for tool output before truncation (Tool Guardrail).
    #[serde(default = "default_tool_output_max_bytes")]
    pub tool_output_max_bytes: usize,
}

// ─── Memory (Cross-Thread Semantic) ──────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct MemoryConfig {
    /// Backend: "sqlite_vec" (default) or "postgres".
    #[serde(default = "default_memory_backend")]
    pub backend: String,
    /// Path for sqlite-vec database file.
    #[serde(default = "default_memory_sqlite_path")]
    pub sqlite_path: String,
    /// PostgreSQL connection string (required when backend = "postgres").
    #[serde(default)]
    pub postgres_url: Option<String>,
    /// Number of top fragments to retrieve per LLM call.
    #[serde(default = "default_retrieval_top_k")]
    pub retrieval_top_k: usize,
    /// Minimum cosine similarity score to include a fragment.
    #[serde(default = "default_retrieval_min_score")]
    pub retrieval_min_score: f32,
    /// Whether pre-LLM retrieval is enabled.
    #[serde(default = "default_true")]
    pub retrieval_enabled: bool,
    /// Minimum score for extractor to persist a fragment.
    #[serde(default = "default_extractor_min_score")]
    pub extractor_min_score: f32,
    /// Cosine threshold for deduplication.
    #[serde(default = "default_dedup_threshold")]
    pub dedup_threshold: f32,
    /// Max fragments stored per thread_id.
    #[serde(default = "default_max_fragments_per_thread")]
    pub max_fragments_per_thread: usize,
    /// Maturation subsystem configuration (consolidation, decay, handoff).
    #[serde(default)]
    pub maturation: MaturationConfig,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            backend: default_memory_backend(),
            sqlite_path: default_memory_sqlite_path(),
            postgres_url: None,
            retrieval_top_k: default_retrieval_top_k(),
            retrieval_min_score: default_retrieval_min_score(),
            retrieval_enabled: true,
            extractor_min_score: default_extractor_min_score(),
            dedup_threshold: default_dedup_threshold(),
            max_fragments_per_thread: default_max_fragments_per_thread(),
            maturation: MaturationConfig::default(),
        }
    }
}

// ─── Maturation (Consolidation, Decay, Handoff) ──────────────────────────────

/// Configuration for the memory maturation subsystem.
///
/// Parsed from `[memory.maturation]` in `tokens.toml`. All fields have
/// documented defaults and valid ranges. Out-of-range values are replaced
/// with defaults and a `tracing::warn!` is emitted.
#[derive(Debug, Deserialize, Clone)]
pub struct MaturationConfig {
    /// Master switch for consolidation and handoff generation.
    /// When false, decay scoring still applies at retrieval time.
    #[serde(default = "default_maturation_enabled")]
    pub enabled: bool,
    /// Exponential decay factor applied per day since last access.
    /// Range: 0.5–0.999, default: 0.95.
    #[serde(default = "default_decay_factor")]
    pub decay_factor: f64,
    /// Minimum effective score floor (prevents fragments from becoming invisible).
    /// Range: 0.001–0.1, default: 0.01.
    #[serde(default = "default_decay_floor")]
    pub decay_floor: f64,
    /// Minimum working-tier fragments in a session to trigger consolidation.
    /// Range: 2–50, default: 3.
    #[serde(default = "default_min_fragments")]
    pub min_fragments_for_consolidation: u64,
    /// Cosine similarity threshold for single-linkage clustering (episodic→semantic).
    /// Range: 0.5–1.0, default: 0.75.
    #[serde(default = "default_semantic_threshold")]
    pub semantic_similarity_threshold: f64,
    /// Minimum episodic fragments in a cluster to trigger semantic consolidation.
    /// Range: 3–20, default: 5.
    #[serde(default = "default_min_episodic")]
    pub min_episodic_for_semantic: u64,
    /// Hours between periodic semantic consolidation checks.
    /// Range: 1–168, default: 6.
    #[serde(default = "default_semantic_interval")]
    pub semantic_check_interval_hours: u64,
    /// Access count threshold for procedural promotion.
    /// Range: 5–100, default: 10.
    #[serde(default = "default_procedural_access")]
    pub procedural_access_threshold: u64,
    /// Minimum age in days for procedural promotion.
    /// Range: 1–90, default: 7.
    #[serde(default = "default_procedural_age")]
    pub procedural_min_age_days: u64,
    /// Maximum characters for handoff summary.
    /// Range: 500–10000, default: 2000.
    #[serde(default = "default_max_handoff")]
    pub max_handoff_chars: u64,
    /// Maximum fragments promoted to procedural per consolidation cycle.
    /// Range: 1–500, default: 50.
    #[serde(default = "default_max_promotions")]
    pub max_promotions_per_cycle: u64,
}

impl Default for MaturationConfig {
    fn default() -> Self {
        Self {
            enabled: default_maturation_enabled(),
            decay_factor: default_decay_factor(),
            decay_floor: default_decay_floor(),
            min_fragments_for_consolidation: default_min_fragments(),
            semantic_similarity_threshold: default_semantic_threshold(),
            min_episodic_for_semantic: default_min_episodic(),
            semantic_check_interval_hours: default_semantic_interval(),
            procedural_access_threshold: default_procedural_access(),
            procedural_min_age_days: default_procedural_age(),
            max_handoff_chars: default_max_handoff(),
            max_promotions_per_cycle: default_max_promotions(),
        }
    }
}

impl MaturationConfig {
    /// Validate all fields against their documented ranges.
    /// Out-of-range values are replaced with defaults and a `tracing::warn!` is emitted.
    pub fn validated(mut self) -> Self {
        self.decay_factor = validate_f64_range(
            "decay_factor",
            self.decay_factor,
            0.5,
            0.999,
            default_decay_factor(),
        );
        self.decay_floor = validate_f64_range(
            "decay_floor",
            self.decay_floor,
            0.001,
            0.1,
            default_decay_floor(),
        );
        self.min_fragments_for_consolidation = validate_u64_range(
            "min_fragments_for_consolidation",
            self.min_fragments_for_consolidation,
            2,
            50,
            default_min_fragments(),
        );
        self.semantic_similarity_threshold = validate_f64_range(
            "semantic_similarity_threshold",
            self.semantic_similarity_threshold,
            0.5,
            1.0,
            default_semantic_threshold(),
        );
        self.min_episodic_for_semantic = validate_u64_range(
            "min_episodic_for_semantic",
            self.min_episodic_for_semantic,
            3,
            20,
            default_min_episodic(),
        );
        self.semantic_check_interval_hours = validate_u64_range(
            "semantic_check_interval_hours",
            self.semantic_check_interval_hours,
            1,
            168,
            default_semantic_interval(),
        );
        self.procedural_access_threshold = validate_u64_range(
            "procedural_access_threshold",
            self.procedural_access_threshold,
            5,
            100,
            default_procedural_access(),
        );
        self.procedural_min_age_days = validate_u64_range(
            "procedural_min_age_days",
            self.procedural_min_age_days,
            1,
            90,
            default_procedural_age(),
        );
        self.max_handoff_chars = validate_u64_range(
            "max_handoff_chars",
            self.max_handoff_chars,
            500,
            10_000,
            default_max_handoff(),
        );
        self.max_promotions_per_cycle = validate_u64_range(
            "max_promotions_per_cycle",
            self.max_promotions_per_cycle,
            1,
            500,
            default_max_promotions(),
        );
        self
    }
}

// ─── Maturation defaults ─────────────────────────────────────────────────────

const fn default_maturation_enabled() -> bool { true }
const fn default_decay_factor() -> f64 { 0.95 }
const fn default_decay_floor() -> f64 { 0.01 }
const fn default_min_fragments() -> u64 { 3 }
const fn default_semantic_threshold() -> f64 { 0.75 }
const fn default_min_episodic() -> u64 { 5 }
const fn default_semantic_interval() -> u64 { 6 }
const fn default_procedural_access() -> u64 { 10 }
const fn default_procedural_age() -> u64 { 7 }
const fn default_max_handoff() -> u64 { 2000 }
const fn default_max_promotions() -> u64 { 50 }

// ─── Range validation helpers ────────────────────────────────────────────────

/// Validate a `u64` value is within `[min, max]`. If out of range, log a warning
/// and return the default value.
fn validate_u64_range(key: &str, value: u64, min: u64, max: u64, default: u64) -> u64 {
    if value < min || value > max {
        tracing::warn!(
            key = key,
            value = value,
            min = min,
            max = max,
            default = default,
            "maturation config key '{}' has value {} outside valid range [{}, {}]; using default {}",
            key, value, min, max, default,
        );
        default
    } else {
        value
    }
}

/// Validate an `f64` value is within `[min, max]`. If out of range, log a warning
/// and return the default value.
fn validate_f64_range(key: &str, value: f64, min: f64, max: f64, default: f64) -> f64 {
    if value < min || value > max {
        tracing::warn!(
            key = key,
            value = value,
            min = min,
            max = max,
            default = default,
            "maturation config key '{}' has value {} outside valid range [{}, {}]; using default {}",
            key, value, min, max, default,
        );
        default
    } else {
        value
    }
}

// ─── Queue (Resilience Buffer) ───────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct QueueConfig {
    /// Whether the queue is enabled. If false, messages are processed synchronously.
    #[serde(default)]
    pub enabled: bool,
    /// PostgreSQL connection string for the message queue.
    #[serde(default)]
    pub postgres_url: Option<String>,
    /// Max concurrent workers consuming the queue.
    #[serde(default = "default_queue_max_workers")]
    pub max_workers: usize,
    /// Max retry attempts before marking a message as failed.
    #[serde(default = "default_queue_max_attempts")]
    pub max_attempts: u32,
    /// Polling interval in ms when queue is empty.
    #[serde(default = "default_queue_poll_interval")]
    pub poll_interval_ms: u64,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            postgres_url: None,
            max_workers: default_queue_max_workers(),
            max_attempts: default_queue_max_attempts(),
            poll_interval_ms: default_queue_poll_interval(),
        }
    }
}

// ─── Waves (Parallel Execution) ──────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct WavesConfig {
    /// Max agents spawned in parallel per wave.
    #[serde(default = "default_waves_max_parallel")]
    pub max_parallel_agents: usize,
    /// What to do when an agent in a wave fails: "continue", "abort", "retry".
    #[serde(default = "default_waves_failure_policy")]
    pub failure_policy: String,
    /// Max retry attempts per failed agent in a wave.
    #[serde(default = "default_waves_retry_max")]
    pub retry_max_attempts: u32,
}

impl Default for WavesConfig {
    fn default() -> Self {
        Self {
            max_parallel_agents: default_waves_max_parallel(),
            failure_policy: default_waves_failure_policy(),
            retry_max_attempts: default_waves_retry_max(),
        }
    }
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
const fn default_approval_timeout() -> u64 { 900 } // 15 minutes
const fn default_hard_stop() -> f64 { 50.00 }
const fn default_warn_pct() -> f64 { 80.0 }
const fn default_agent_default_usd() -> f64 { 1.00 }
const fn default_max_worktrees() -> usize { 2 }
const fn default_max_agent_iters() -> u32 { 10 }
const fn default_max_agent_iters_yolo() -> u32 { 5 }
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
const fn default_llm_timeout_secs() -> u64 { 300 }

// ─── New defaults (safety/memory/queue/waves) ────────────────────────────────

const fn default_prompt_injection_levenshtein() -> usize { 2 }
const fn default_true() -> bool { true }
const fn default_tool_output_max_bytes() -> usize { 102_400 } // 100 KB
fn default_memory_backend() -> String { "sqlite_vec".into() }
fn default_memory_sqlite_path() -> String { "~/.local/share/kira/memory.db".into() }
const fn default_retrieval_top_k() -> usize { 5 }
const fn default_retrieval_min_score() -> f32 { 0.70 }
const fn default_extractor_min_score() -> f32 { 0.60 }
const fn default_dedup_threshold() -> f32 { 0.95 }
const fn default_max_fragments_per_thread() -> usize { 1000 }
const fn default_queue_max_workers() -> usize { 4 }
const fn default_queue_max_attempts() -> u32 { 3 }
const fn default_queue_poll_interval() -> u64 { 500 }
const fn default_waves_max_parallel() -> usize { 3 }
fn default_waves_failure_policy() -> String { "continue".into() }
const fn default_waves_retry_max() -> u32 { 2 }

// ─── Loader ──────────────────────────────────────────────────────────────────

/// Load `tokens.toml` from the given path (absolute or relative to CWD).
/// Falls back to all defaults if the file doesn't exist.
pub fn load_tokens(path: &Path) -> anyhow::Result<Tokens> {
    let content = std::fs::read_to_string(path)?;
    let mut tokens: Tokens = toml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("failed to parse tokens.toml: {e}"))?;
    // Validate maturation config ranges, replacing invalid values with defaults.
    tokens.memory.maturation = tokens.memory.maturation.validated();
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

    #[test]
    fn maturation_config_defaults() {
        let config = MaturationConfig::default();
        assert!(config.enabled);
        assert_eq!(config.decay_factor, 0.95);
        assert_eq!(config.decay_floor, 0.01);
        assert_eq!(config.min_fragments_for_consolidation, 3);
        assert_eq!(config.semantic_similarity_threshold, 0.75);
        assert_eq!(config.min_episodic_for_semantic, 5);
        assert_eq!(config.semantic_check_interval_hours, 6);
        assert_eq!(config.procedural_access_threshold, 10);
        assert_eq!(config.procedural_min_age_days, 7);
        assert_eq!(config.max_handoff_chars, 2000);
        assert_eq!(config.max_promotions_per_cycle, 50);
    }

    #[test]
    fn maturation_config_validation_clamps_out_of_range() {
        let config = MaturationConfig {
            enabled: true,
            decay_factor: 0.1,       // below 0.5
            decay_floor: 0.5,        // above 0.1
            min_fragments_for_consolidation: 100, // above 50
            semantic_similarity_threshold: 0.2,   // below 0.5
            min_episodic_for_semantic: 0,         // below 3
            semantic_check_interval_hours: 200,   // above 168
            procedural_access_threshold: 200,     // above 100
            procedural_min_age_days: 0,           // below 1
            max_handoff_chars: 100,               // below 500
            max_promotions_per_cycle: 1000,       // above 500
        };
        let validated = config.validated();
        assert_eq!(validated.decay_factor, 0.95);
        assert_eq!(validated.decay_floor, 0.01);
        assert_eq!(validated.min_fragments_for_consolidation, 3);
        assert_eq!(validated.semantic_similarity_threshold, 0.75);
        assert_eq!(validated.min_episodic_for_semantic, 5);
        assert_eq!(validated.semantic_check_interval_hours, 6);
        assert_eq!(validated.procedural_access_threshold, 10);
        assert_eq!(validated.procedural_min_age_days, 7);
        assert_eq!(validated.max_handoff_chars, 2000);
        assert_eq!(validated.max_promotions_per_cycle, 50);
    }

    #[test]
    fn maturation_config_validation_keeps_valid_values() {
        let config = MaturationConfig {
            enabled: false,
            decay_factor: 0.8,
            decay_floor: 0.05,
            min_fragments_for_consolidation: 10,
            semantic_similarity_threshold: 0.9,
            min_episodic_for_semantic: 10,
            semantic_check_interval_hours: 24,
            procedural_access_threshold: 50,
            procedural_min_age_days: 30,
            max_handoff_chars: 5000,
            max_promotions_per_cycle: 100,
        };
        let validated = config.validated();
        assert!(!validated.enabled);
        assert_eq!(validated.decay_factor, 0.8);
        assert_eq!(validated.decay_floor, 0.05);
        assert_eq!(validated.min_fragments_for_consolidation, 10);
        assert_eq!(validated.semantic_similarity_threshold, 0.9);
        assert_eq!(validated.min_episodic_for_semantic, 10);
        assert_eq!(validated.semantic_check_interval_hours, 24);
        assert_eq!(validated.procedural_access_threshold, 50);
        assert_eq!(validated.procedural_min_age_days, 30);
        assert_eq!(validated.max_handoff_chars, 5000);
        assert_eq!(validated.max_promotions_per_cycle, 100);
    }

    #[test]
    fn maturation_config_validation_boundary_values() {
        // Test exact boundary values (should be accepted)
        let config = MaturationConfig {
            enabled: true,
            decay_factor: 0.5,       // min boundary
            decay_floor: 0.001,      // min boundary
            min_fragments_for_consolidation: 2,  // min boundary
            semantic_similarity_threshold: 1.0,  // max boundary
            min_episodic_for_semantic: 20,       // max boundary
            semantic_check_interval_hours: 168,  // max boundary
            procedural_access_threshold: 5,      // min boundary
            procedural_min_age_days: 90,         // max boundary
            max_handoff_chars: 10_000,           // max boundary
            max_promotions_per_cycle: 1,         // min boundary
        };
        let validated = config.validated();
        assert_eq!(validated.decay_factor, 0.5);
        assert_eq!(validated.decay_floor, 0.001);
        assert_eq!(validated.min_fragments_for_consolidation, 2);
        assert_eq!(validated.semantic_similarity_threshold, 1.0);
        assert_eq!(validated.min_episodic_for_semantic, 20);
        assert_eq!(validated.semantic_check_interval_hours, 168);
        assert_eq!(validated.procedural_access_threshold, 5);
        assert_eq!(validated.procedural_min_age_days, 90);
        assert_eq!(validated.max_handoff_chars, 10_000);
        assert_eq!(validated.max_promotions_per_cycle, 1);
    }

    #[test]
    fn maturation_config_deserializes_from_toml() {
        let toml_str = r#"
            [memory.maturation]
            enabled = false
            decay_factor = 0.9
            decay_floor = 0.05
            min_fragments_for_consolidation = 5
            semantic_similarity_threshold = 0.8
            min_episodic_for_semantic = 7
            semantic_check_interval_hours = 12
            procedural_access_threshold = 20
            procedural_min_age_days = 14
            max_handoff_chars = 3000
            max_promotions_per_cycle = 25
        "#;

        #[derive(Debug, Deserialize)]
        struct Wrapper {
            memory: MemoryConfig,
        }

        let wrapper: Wrapper = toml::from_str(toml_str).unwrap();
        let m = wrapper.memory.maturation;
        assert!(!m.enabled);
        assert_eq!(m.decay_factor, 0.9);
        assert_eq!(m.decay_floor, 0.05);
        assert_eq!(m.min_fragments_for_consolidation, 5);
        assert_eq!(m.semantic_similarity_threshold, 0.8);
        assert_eq!(m.min_episodic_for_semantic, 7);
        assert_eq!(m.semantic_check_interval_hours, 12);
        assert_eq!(m.procedural_access_threshold, 20);
        assert_eq!(m.procedural_min_age_days, 14);
        assert_eq!(m.max_handoff_chars, 3000);
        assert_eq!(m.max_promotions_per_cycle, 25);
    }

    #[test]
    fn maturation_config_absent_section_uses_defaults() {
        let toml_str = r#"
            [memory]
            backend = "sqlite_vec"
        "#;

        #[derive(Debug, Deserialize)]
        struct Wrapper {
            #[serde(default)]
            memory: MemoryConfig,
        }

        let wrapper: Wrapper = toml::from_str(toml_str).unwrap();
        let m = wrapper.memory.maturation;
        assert!(m.enabled);
        assert_eq!(m.decay_factor, 0.95);
        assert_eq!(m.decay_floor, 0.01);
        assert_eq!(m.min_fragments_for_consolidation, 3);
    }
}
