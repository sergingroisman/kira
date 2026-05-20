//! BudgetGuard — monthly USD hard-stop + warn threshold.
//!
//! v0: in-memory counter. Resets when daemon restarts. Good enough for solo Mac.
//! v0.5: persisted in redb `cost_daily` table; reconstruct on start.

use anyhow::Result;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::warn;

/// Per-model pricing (USD per 1M tokens). Plan T4 table.
pub fn price_per_million(model: &str) -> Option<(f64, f64)> {
    // (input_usd_per_1m, output_usd_per_1m)
    match model {
        "qwen-max" => Some((1.60, 6.40)),
        "qwen-plus" => Some((0.40, 1.20)),
        "qwen-flash" => Some((0.15, 0.60)),
        "deepseek-v4-pro" => Some((0.43, 0.87)),
        "deepseek-v4-flash" => Some((0.14, 0.28)),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BudgetConfig {
    pub monthly_hard_stop_usd: f64,
    pub warn_pct: f64, // 0..100
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            monthly_hard_stop_usd: 50.00,
            warn_pct: 80.0,
        }
    }
}

/// Tracks cumulative USD spend in-memory.
/// Stores microcents (1/100 of a cent = 1e-6 USD) as u64 for lock-free updates.
#[derive(Debug)]
pub struct BudgetGuard {
    config: BudgetConfig,
    spent_microcents: AtomicU64,
}

impl BudgetGuard {
    pub fn new(config: BudgetConfig) -> Self {
        Self {
            config,
            spent_microcents: AtomicU64::new(0),
        }
    }

    fn microcents(usd: f64) -> u64 {
        (usd * 1_000_000.0).round() as u64
    }

    fn from_microcents(mc: u64) -> f64 {
        mc as f64 / 1_000_000.0
    }

    /// Returns current total spend (USD).
    pub fn spent_usd(&self) -> f64 {
        Self::from_microcents(self.spent_microcents.load(Ordering::Acquire))
    }

    /// Hard-stop check — fails before issuing a call if already over budget.
    pub fn check(&self) -> Result<()> {
        let spent = self.spent_usd();
        if spent >= self.config.monthly_hard_stop_usd {
            anyhow::bail!(
                "budget hard-stop: ${:.4} >= ${:.2}",
                spent,
                self.config.monthly_hard_stop_usd
            );
        }
        Ok(())
    }

    /// Add a call's cost. Computes from token usage + model pricing.
    pub fn track_call(&self, model: &str, tokens_in: u32, tokens_out: u32) {
        let Some((in_price, out_price)) = price_per_million(model) else {
            warn!(model, "no pricing entry — skipping budget track");
            return;
        };
        let cost_usd = (f64::from(tokens_in) * in_price + f64::from(tokens_out) * out_price)
            / 1_000_000.0;
        let mc = Self::microcents(cost_usd);
        let prev = self.spent_microcents.fetch_add(mc, Ordering::AcqRel);
        let new_total = Self::from_microcents(prev + mc);

        // Warn threshold
        let warn_threshold = self.config.monthly_hard_stop_usd * (self.config.warn_pct / 100.0);
        if new_total >= warn_threshold {
            warn!(
                spent_usd = new_total,
                limit_usd = self.config.monthly_hard_stop_usd,
                pct = (new_total / self.config.monthly_hard_stop_usd) * 100.0,
                "budget warn threshold crossed"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pricing_lookup() {
        assert!(price_per_million("deepseek-v4-pro").is_some());
        assert!(price_per_million("unknown-model").is_none());
    }

    #[test]
    fn track_accumulates() {
        let b = BudgetGuard::new(BudgetConfig::default());
        b.track_call("deepseek-v4-pro", 1_000_000, 1_000_000);
        // 0.43 + 0.87 = 1.30 USD
        let s = b.spent_usd();
        assert!((s - 1.30).abs() < 0.001, "expected ~1.30, got {s}");
    }

    #[test]
    fn check_blocks_over_budget() {
        let cfg = BudgetConfig {
            monthly_hard_stop_usd: 1.0,
            warn_pct: 80.0,
        };
        let b = BudgetGuard::new(cfg);
        b.track_call("deepseek-v4-pro", 1_000_000, 1_000_000); // 1.30 > 1.00
        assert!(b.check().is_err());
    }

    #[test]
    fn unknown_model_is_skipped() {
        let b = BudgetGuard::new(BudgetConfig::default());
        b.track_call("ghost-model", 999, 999);
        assert_eq!(b.spent_usd(), 0.0);
    }
}
