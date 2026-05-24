//! LLM providers — protocol‑agnostic facade over multiple wire formats.
//!
//! | Module | Purpose |
//! |---|---|
//! | `types` | Unified types (`ChatOutput`, `ToolDef`, `StreamChunk`) |
//! | `protocol` | Wire‑protocol enum (`OpenAiCompatible`, `Anthropic`) |
//! | `provider` | `LlmProvider` with protocol dispatch |
//! | `embed_cache` | Embedding cache over redb |

pub mod anthropic;
pub mod embed_cache;
pub mod protocol;
pub mod provider;
pub mod types;

pub use protocol::Protocol;
pub use provider::LlmProvider;
pub use types::*;
