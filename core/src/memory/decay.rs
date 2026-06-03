//! Exponential Decay Scoring.
//!
//! Computes effective retrieval scores with time-based exponential decay.
//! Decay is applied at retrieval time only — it never modifies persisted data.

use chrono::{DateTime, Utc};

use super::tier::MemoryTier;

/// Configuration for decay scoring.
#[derive(Debug, Clone, Copy)]
pub struct DecayConfig {
    /// Exponential decay factor applied per day (0.5–0.999, default 0.95).
    pub decay_factor: f64,
    /// Minimum effective score floor (0.001–0.1, default 0.01).
    pub decay_floor: f64,
}

impl Default for DecayConfig {
    fn default() -> Self {
        Self {
            decay_factor: 0.95,
            decay_floor: 0.01,
        }
    }
}

/// Compute the effective retrieval score with exponential decay.
///
/// - Procedural-tier fragments are exempt (return `base_score` unchanged).
/// - Fragments never accessed use `created_at` as the reference timestamp.
/// - Result is clamped to `decay_floor` minimum.
/// - `now` is accepted as a parameter for deterministic testing.
pub fn effective_score(
    base_score: f64,
    tier: MemoryTier,
    last_accessed: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    now: DateTime<Utc>,
    config: &DecayConfig,
) -> f64 {
    // Procedural fragments are exempt from decay
    if tier == MemoryTier::Procedural {
        return base_score;
    }

    let reference = last_accessed.unwrap_or(created_at);
    let elapsed = now.signed_duration_since(reference);
    let days = elapsed.num_seconds() as f64 / 86_400.0;
    let days = days.max(0.0); // guard against clock skew

    let decayed = base_score * config.decay_factor.powf(days);
    decayed.max(config.decay_floor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn default_config() -> DecayConfig {
        DecayConfig::default()
    }

    fn base_time() -> DateTime<Utc> {
        chrono::DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
            .expect("valid test timestamp")
            .with_timezone(&Utc)
    }

    #[test]
    fn procedural_tier_exempt_from_decay() {
        let config = default_config();
        let created = base_time();
        let now = created + Duration::days(100);

        let score = effective_score(0.9, MemoryTier::Procedural, None, created, now, &config);
        assert_eq!(score, 0.9);
    }

    #[test]
    fn working_tier_decays_over_time() {
        let config = default_config();
        let created = base_time();
        let now = created + Duration::days(10);

        let score = effective_score(1.0, MemoryTier::Working, None, created, now, &config);
        // 1.0 * 0.95^10 ≈ 0.5987
        let expected = 0.95_f64.powi(10);
        assert!((score - expected).abs() < 1e-10);
    }

    #[test]
    fn episodic_tier_decays() {
        let config = default_config();
        let created = base_time();
        let now = created + Duration::days(5);

        let score = effective_score(0.8, MemoryTier::Episodic, None, created, now, &config);
        let expected = 0.8 * 0.95_f64.powi(5);
        assert!((score - expected).abs() < 1e-10);
    }

    #[test]
    fn semantic_tier_decays() {
        let config = default_config();
        let created = base_time();
        let now = created + Duration::days(3);

        let score = effective_score(0.7, MemoryTier::Semantic, None, created, now, &config);
        let expected = 0.7 * 0.95_f64.powi(3);
        assert!((score - expected).abs() < 1e-10);
    }

    #[test]
    fn uses_last_accessed_when_available() {
        let config = default_config();
        let created = base_time();
        let last_accessed = created + Duration::days(8);
        let now = created + Duration::days(10);

        // Should use last_accessed (2 days ago), not created_at (10 days ago)
        let score = effective_score(
            1.0,
            MemoryTier::Working,
            Some(last_accessed),
            created,
            now,
            &config,
        );
        let expected = 0.95_f64.powi(2);
        assert!((score - expected).abs() < 1e-10);
    }

    #[test]
    fn falls_back_to_created_at_when_never_accessed() {
        let config = default_config();
        let created = base_time();
        let now = created + Duration::days(7);

        let score = effective_score(1.0, MemoryTier::Working, None, created, now, &config);
        let expected = 0.95_f64.powi(7);
        assert!((score - expected).abs() < 1e-10);
    }

    #[test]
    fn floor_clamping_prevents_zero_score() {
        let config = DecayConfig {
            decay_factor: 0.5,
            decay_floor: 0.01,
        };
        let created = base_time();
        let now = created + Duration::days(365);

        let score = effective_score(0.5, MemoryTier::Working, None, created, now, &config);
        // 0.5 * 0.5^365 is astronomically small, should be clamped to floor
        assert_eq!(score, 0.01);
    }

    #[test]
    fn zero_elapsed_time_returns_base_score() {
        let config = default_config();
        let created = base_time();

        let score = effective_score(0.85, MemoryTier::Working, None, created, created, &config);
        // 0.85 * 0.95^0 = 0.85
        assert!((score - 0.85).abs() < 1e-10);
    }

    #[test]
    fn negative_elapsed_time_treated_as_zero() {
        let config = default_config();
        let created = base_time();
        // now is before created_at (clock skew scenario)
        let now = created - Duration::hours(2);

        let score = effective_score(0.85, MemoryTier::Working, None, created, now, &config);
        // days clamped to 0.0, so 0.85 * 0.95^0 = 0.85
        assert!((score - 0.85).abs() < 1e-10);
    }

    #[test]
    fn fractional_days_computed_correctly() {
        let config = default_config();
        let created = base_time();
        let now = created + Duration::hours(12); // 0.5 days

        let score = effective_score(1.0, MemoryTier::Working, None, created, now, &config);
        let expected = 0.95_f64.powf(0.5);
        assert!((score - expected).abs() < 1e-10);
    }

    #[test]
    fn result_never_exceeds_base_score() {
        let config = default_config();
        let created = base_time();
        let now = created + Duration::days(1);

        let base = 0.6;
        let score = effective_score(base, MemoryTier::Episodic, None, created, now, &config);
        assert!(score <= base);
    }

    #[test]
    fn default_config_has_expected_values() {
        let config = DecayConfig::default();
        assert!((config.decay_factor - 0.95).abs() < 1e-10);
        assert!((config.decay_floor - 0.01).abs() < 1e-10);
    }
}
