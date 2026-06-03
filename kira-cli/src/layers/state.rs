#![allow(unused)]
//! Layer state management — per-layer status, event buffers, and flow tracking.

use std::collections::VecDeque;
use std::time::Instant;

// ---------------------------------------------------------------------------
// Core Enums
// ---------------------------------------------------------------------------

/// The four conceptual layers in the agentic pipeline (ordered top-to-bottom).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum ConceptualLayer {
    Perception = 0,
    Reasoning = 1,
    Security = 2,
    Execution = 3,
}

impl ConceptualLayer {
    /// Convert a usize index (0–3) to the corresponding layer.
    /// Panics if index > 3 in debug; clamps to Execution in release.
    pub const fn from_index(idx: usize) -> Self {
        match idx {
            0 => Self::Perception,
            1 => Self::Reasoning,
            2 => Self::Security,
            _ => Self::Execution,
        }
    }

    /// Human-readable layer name for status display.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Perception => "Perception",
            Self::Reasoning => "Reasoning",
            Self::Security => "Security",
            Self::Execution => "Execution",
        }
    }
}

/// Current status of a single layer (drives indicator rendering).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerStatus {
    Idle,
    Active,
    Blocked,
    Errored,
}

// ---------------------------------------------------------------------------
// Pipeline Event
// ---------------------------------------------------------------------------

/// A classified pipeline event stored in the per-layer buffer.
#[derive(Debug, Clone)]
pub struct PipelineEvent {
    /// Display timestamp in HH:MM:SS format.
    pub timestamp: String,
    /// The PillarEvent phase (start, step, fallback, fail, done).
    pub phase: String,
    /// Human-readable message (truncated to 60 chars at insertion).
    pub message: String,
    /// Raw instant for rolling window calculations.
    pub raw_ts: Instant,
}

// ---------------------------------------------------------------------------
// Single Layer State
// ---------------------------------------------------------------------------

/// State for a single conceptual layer.
#[derive(Debug, Clone)]
pub struct SingleLayerState {
    /// Current status (idle/active/blocked/errored).
    pub status: LayerStatus,
    /// When the layer last transitioned to Active.
    pub active_since: Option<Instant>,
    /// Last recorded duration in seconds (retained after transition to idle/errored).
    pub last_duration_secs: u64,
    /// Rolling event buffer (FIFO, max 50 entries).
    pub events: VecDeque<PipelineEvent>,
    /// Layer-specific metadata for the detail panel (populated by event loop).
    pub detail: Option<LayerDetail>,
}

impl SingleLayerState {
    /// Create a new idle layer state with empty event buffer.
    pub const fn new() -> Self {
        Self {
            status: LayerStatus::Idle,
            active_since: None,
            last_duration_secs: 0,
            events: VecDeque::new(),
            detail: None,
        }
    }
}

impl Default for SingleLayerState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Flow State
// ---------------------------------------------------------------------------

/// Tracks data flow between two adjacent layers.
#[derive(Debug, Clone)]
pub struct FlowState {
    /// When the upstream layer last emitted a "done" phase.
    pub upstream_done_at: Option<Instant>,
    /// Whether flow is currently active (visual animation).
    pub flowing: bool,
    /// Timestamps of flow events within the 30-second rolling window.
    pub flow_times: VecDeque<Instant>,
    /// Whether bottleneck highlight is active on this arrow.
    pub bottleneck: bool,
    /// When the last flow event occurred (for staleness detection).
    pub last_flow_at: Option<Instant>,
}

impl FlowState {
    /// Create a new flow state with no recorded flow.
    pub const fn new() -> Self {
        Self {
            upstream_done_at: None,
            flowing: false,
            flow_times: VecDeque::new(),
            bottleneck: false,
            last_flow_at: None,
        }
    }
}

impl Default for FlowState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Layer Detail (per-layer metadata for drill-down panel)
// ---------------------------------------------------------------------------

/// Layer-specific metadata shown in the detail panel.
#[derive(Debug, Clone)]
pub enum LayerDetail {
    Perception {
        memory_fragments_count: u32,
        files_read_count: u32,
        rag_query: Option<String>,
    },
    Reasoning {
        sdd_phase: Option<String>,
        last_intent: Option<String>,
        active_model: Option<String>,
    },
    Security {
        last_guardrail_reason: Option<String>,
        current_risk: Option<String>,
        hitl_status: Option<String>,
    },
    Execution {
        current_tool: Option<String>,
        agent_short_id: Option<String>,
        elapsed_secs: Option<u64>,
    },
}

// ---------------------------------------------------------------------------
// Top-Level Layers State
// ---------------------------------------------------------------------------

/// Top-level state for the entire Layers View.
#[derive(Debug, Clone)]
pub struct LayersState {
    /// Per-layer state indexed by ConceptualLayer ordinal (0=Perception..3=Execution).
    pub layers: [SingleLayerState; 4],
    /// Flow state for 3 forward arrows (P→R, R→S, S→E).
    pub flows: [FlowState; 3],
    /// Whether the Core is currently disconnected.
    pub disconnected: bool,
    /// Currently expanded detail panel (None = collapsed, Some(0..4) = layer index).
    pub detail_expanded: Option<usize>,
    /// Whether the first-show toast has been displayed this session.
    pub first_show_toast_shown: bool,
    /// Pre-allocated render buffer for layer labels (avoids per-frame allocation).
    pub render_scratch: Vec<u8>,
}

impl LayersState {
    /// Maximum events per layer buffer.
    pub const MAX_EVENTS_PER_LAYER: usize = 50;

    /// Maximum message length in characters (truncated at insertion).
    const MAX_MESSAGE_CHARS: usize = 60;

    /// Create a new LayersState with all layers idle and empty buffers.
    pub fn new() -> Self {
        Self {
            layers: [
                SingleLayerState::new(),
                SingleLayerState::new(),
                SingleLayerState::new(),
                SingleLayerState::new(),
            ],
            flows: [FlowState::new(), FlowState::new(), FlowState::new()],
            disconnected: false,
            detail_expanded: None,
            first_show_toast_shown: false,
            // Pre-allocate 256 bytes for render scratch space to avoid per-frame allocation.
            render_scratch: Vec::with_capacity(256),
        }
    }

    /// Apply a classified event to the appropriate layer.
    ///
    /// - phase "start" → Active (refreshes active_since if already Active)
    /// - phase "done" → Idle (records duration, marks upstream_done_at on downstream flow)
    /// - phase "fail" → Errored (records duration)
    /// - any other phase → Active (refreshes if not already Active)
    pub fn apply_event(&mut self, layer: ConceptualLayer, phase: &str, mut event: PipelineEvent) {
        let idx = layer as usize;

        // Truncate message to 60 chars at insertion.
        event.message = event.message.chars().take(Self::MAX_MESSAGE_CHARS).collect();

        let layer_state = &mut self.layers[idx];

        // Push event to buffer (evict oldest if at cap).
        if layer_state.events.len() >= Self::MAX_EVENTS_PER_LAYER {
            layer_state.events.pop_front();
        }
        layer_state.events.push_back(event.clone());

        // State transition based on phase.
        match phase {
            "start" => {
                // On "start" when already Active: refresh active_since.
                layer_state.status = LayerStatus::Active;
                layer_state.active_since = Some(event.raw_ts);
            }
            "done" => {
                // Record duration before transitioning.
                if let Some(start) = layer_state.active_since {
                    layer_state.last_duration_secs =
                        event.raw_ts.duration_since(start).as_secs();
                }
                layer_state.status = LayerStatus::Idle;
                layer_state.active_since = None;

                // Mark upstream_done_at on downstream flow arrow.
                if idx < 3 {
                    self.flows[idx].upstream_done_at = Some(event.raw_ts);
                }
            }
            "fail" => {
                // Record duration before transitioning.
                if let Some(start) = layer_state.active_since {
                    layer_state.last_duration_secs =
                        event.raw_ts.duration_since(start).as_secs();
                }
                layer_state.status = LayerStatus::Errored;
                layer_state.active_since = None;
            }
            _ => {
                // "step", "fallback", or any other phase — refresh Active.
                if layer_state.status != LayerStatus::Active {
                    layer_state.status = LayerStatus::Active;
                    layer_state.active_since = Some(event.raw_ts);
                }
            }
        }
    }

    /// Set Security layer to Blocked (called on ApprovalRequired).
    pub const fn set_security_blocked(&mut self) {
        self.layers[ConceptualLayer::Security as usize].status = LayerStatus::Blocked;
    }

    /// Clear Security blocked state (called on approval resolution).
    /// Only transitions if currently Blocked, preserving other states.
    pub fn clear_security_blocked(&mut self) {
        let sec = &mut self.layers[ConceptualLayer::Security as usize];
        if sec.status == LayerStatus::Blocked {
            sec.status = LayerStatus::Idle;
        }
    }

    /// Reset all layers to Idle (called on CoreDisconnected).
    /// Clears active_since for all layers and sets disconnected=true.
    pub fn reset_all_idle(&mut self) {
        for layer in &mut self.layers {
            layer.status = LayerStatus::Idle;
            layer.active_since = None;
        }
        self.disconnected = true;
    }

    /// Finalize all still-active layers (called when an agent terminates).
    /// Freezes each active layer's elapsed time and returns it to Idle so the
    /// timers stop counting. Unlike `reset_all_idle`, does NOT mark the core
    /// disconnected — the agent simply finished its run.
    pub fn finalize_all_active(&mut self) {
        let now = Instant::now();
        for layer in &mut self.layers {
            if let Some(start) = layer.active_since {
                layer.last_duration_secs = now.duration_since(start).as_secs();
                layer.active_since = None;
            }
            if layer.status == LayerStatus::Active {
                layer.status = LayerStatus::Idle;
            }
        }
    }

    /// Clear disconnected indicator (called on CoreConnected).
    pub const fn clear_disconnected(&mut self) {
        self.disconnected = false;
    }

    /// Detect flow on downstream layer activation.
    ///
    /// Called when a layer transitions to Active. Checks if the upstream
    /// layer completed within 500ms. If so, records a flow event.
    pub fn detect_flow(&mut self, layer: ConceptualLayer, now: Instant) {
        let idx = layer as usize;
        if idx == 0 {
            return; // Perception has no upstream flow arrow
        }

        let flow_idx = idx - 1;
        let flow = &mut self.flows[flow_idx];

        if let Some(done_at) = flow.upstream_done_at
            && now.duration_since(done_at).as_millis() <= 500
        {
            flow.flowing = true;
            flow.flow_times.push_back(now);
            flow.last_flow_at = Some(now);
            flow.bottleneck = false;
            flow.upstream_done_at = None;
        }
    }

    /// Prune stale flow timestamps outside the 30-second rolling window.
    pub fn prune_flow_windows(&mut self, now: Instant) {
        for flow in &mut self.flows {
            while let Some(&front) = flow.flow_times.front() {
                if now.duration_since(front).as_secs() > 30 {
                    flow.flow_times.pop_front();
                } else {
                    break;
                }
            }
        }
    }

    /// Compute throughput for a flow arrow (events in last 30s window).
    pub fn throughput(&self, flow_idx: usize) -> usize {
        self.flows[flow_idx].flow_times.len()
    }

    /// The currently most-relevant active layer and its latest event, for the
    /// "what is it doing now" status strip.
    ///
    /// Prefers the Active layer that transitioned most recently (largest
    /// `active_since`); falls back to any Errored/Blocked layer with events.
    /// Returns `None` when the pipeline is idle.
    pub fn latest_active(&self) -> Option<(ConceptualLayer, &PipelineEvent)> {
        let mut best: Option<(usize, Instant)> = None;
        for (idx, layer) in self.layers.iter().enumerate() {
            if layer.status == LayerStatus::Active
                && let Some(since) = layer.active_since
            {
                match best {
                    Some((_, b)) if b >= since => {}
                    _ => best = Some((idx, since)),
                }
            }
        }
        let idx = best.map(|(i, _)| i)?;
        let event = self.layers[idx].events.back()?;
        Some((ConceptualLayer::from_index(idx), event))
    }
}

impl Default for LayersState {
    fn default() -> Self {
        Self::new()
    }
}
