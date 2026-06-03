//! Layers View configuration — thresholds, visibility defaults, validation.

use serde::Deserialize;

use super::state::ConceptualLayer;

/// Configuration for the Layers View, loaded from `[ui.layers]` in tokens.toml.
///
/// Defaults:
/// - `visible_on_start`: false (Layers View hidden until toggled)
/// - `perception_threshold_secs`: 10s
/// - `reasoning_threshold_secs`: 30s
/// - `security_threshold_secs`: 5s
/// - `execution_threshold_secs`: 60s
#[derive(Debug, Clone, Deserialize)]
pub struct LayersConfig {
    /// Whether the Layers View is visible on TUI start.
    #[serde(default)]
    pub visible_on_start: bool,

    /// Duration threshold (seconds) before Perception layer shows bottleneck.
    #[serde(default = "default_perception_threshold")]
    pub perception_threshold_secs: u64,

    /// Duration threshold (seconds) before Reasoning layer shows bottleneck.
    #[serde(default = "default_reasoning_threshold")]
    pub reasoning_threshold_secs: u64,

    /// Duration threshold (seconds) before Security layer shows bottleneck.
    #[serde(default = "default_security_threshold")]
    pub security_threshold_secs: u64,

    /// Duration threshold (seconds) before Execution layer shows bottleneck.
    #[serde(default = "default_execution_threshold")]
    pub execution_threshold_secs: u64,
}

const fn default_perception_threshold() -> u64 {
    10
}
const fn default_reasoning_threshold() -> u64 {
    30
}
const fn default_security_threshold() -> u64 {
    5
}
const fn default_execution_threshold() -> u64 {
    60
}

impl Default for LayersConfig {
    /// Returns the default configuration:
    /// - `visible_on_start`: false
    /// - `perception_threshold_secs`: 10
    /// - `reasoning_threshold_secs`: 30
    /// - `security_threshold_secs`: 5
    /// - `execution_threshold_secs`: 60
    fn default() -> Self {
        Self {
            visible_on_start: false,
            perception_threshold_secs: default_perception_threshold(),
            reasoning_threshold_secs: default_reasoning_threshold(),
            security_threshold_secs: default_security_threshold(),
            execution_threshold_secs: default_execution_threshold(),
        }
    }
}

impl LayersConfig {
    /// Valid range for threshold values (seconds).
    const MIN_THRESHOLD: u64 = 1;
    const MAX_THRESHOLD: u64 = 3600;

    /// Get the bottleneck threshold (in seconds) for a given conceptual layer.
    pub const fn threshold_for(&self, layer: ConceptualLayer) -> u64 {
        match layer {
            ConceptualLayer::Perception => self.perception_threshold_secs,
            ConceptualLayer::Reasoning => self.reasoning_threshold_secs,
            ConceptualLayer::Security => self.security_threshold_secs,
            ConceptualLayer::Execution => self.execution_threshold_secs,
        }
    }

    /// Validate and clamp thresholds to the valid range [1, 3600].
    ///
    /// Any threshold outside the valid range is replaced with its default value.
    /// Returns a list of field names that were clamped (for warning toast generation).
    pub fn validate_and_clamp(&mut self) -> Vec<&'static str> {
        let mut clamped = Vec::new();

        let fields: [(&'static str, &mut u64, u64); 4] = [
            (
                "perception_threshold_secs",
                &mut self.perception_threshold_secs,
                default_perception_threshold(),
            ),
            (
                "reasoning_threshold_secs",
                &mut self.reasoning_threshold_secs,
                default_reasoning_threshold(),
            ),
            (
                "security_threshold_secs",
                &mut self.security_threshold_secs,
                default_security_threshold(),
            ),
            (
                "execution_threshold_secs",
                &mut self.execution_threshold_secs,
                default_execution_threshold(),
            ),
        ];

        for (name, value, default) in fields {
            if !(Self::MIN_THRESHOLD..=Self::MAX_THRESHOLD).contains(value) {
                *value = default;
                clamped.push(name);
            }
        }

        clamped
    }
}
