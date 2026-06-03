//! Cross-Thread Semantic Memory — Pilar 2.
//!
//! Persists memory fragments with embeddings for semantic retrieval across
//! threads. Supports two backends: sqlite-vec (default, zero-infra) and
//! postgres+pgvector (production opt-in).

pub mod consolidator;
pub mod decay;
pub mod embed;
pub mod extractor;
pub mod handoff;
pub mod maturation_events;
pub mod retrieval;
pub mod sqlite_backend;
pub mod store;
pub mod tier;

pub use store::{MemoryFragment, MemoryStore, MetadataUpdate};
