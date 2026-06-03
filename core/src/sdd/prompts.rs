//! Per-phase prompt fragments injected into the Writer agent task.
//!
//! Each constant is `include_str!`'d from `core/src/agents/prompts/sdd/*.md`
//! so we get hot-reload by rebuild + zero runtime IO.

pub const SPECIFY: &str = include_str!("../agents/prompts/sdd/specify.md");
pub const DESIGN: &str = include_str!("../agents/prompts/sdd/design.md");
pub const TASKS: &str = include_str!("../agents/prompts/sdd/tasks.md");
