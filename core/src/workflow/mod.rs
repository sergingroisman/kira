//! Workflow engine — methodology-agnostic front-end for the Waves back-end.
//!
//! A `WorkflowDef` describes a development methodology as a TOML DAG of
//! `PhaseDef`s. The narrow waist is always `tasks.json` — every workflow
//! ends by emitting this, which feeds the `waves` parallel executor.
//!
//! ## Vocab
//! - **Workflow** = a methodology (`workflow.toml`) — a DAG of Phases.
//! - **Phase**    = one step (specify, design, tasks, execute…).
//! - **Wave**     = group of parallel tasks in the Execute phase (`waves.rs`).
//!
//! ## Adding a methodology
//! Author a new `core/workflows/<id>.toml`. No Rust needed unless the
//! methodology has loops that can't be expressed as a DAG (→ custom `Pilot`).

pub mod def;
pub mod loader;
pub mod manager;
pub mod pilot;
pub mod registry;

pub use def::{EdgeCond, PhaseKind, WorkflowDef};
pub use loader::{bundled_workflow_dir, load as load_workflow, load_all as load_all_workflows};
pub use manager::WorkflowManager;
pub use pilot::WorkflowPilot;
pub use registry::{GateResult, LocHeuristicSizer, ServiceRegistry, SizingHint};
