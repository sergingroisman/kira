//! Text pipeline (M17-M20).
//!
//! - **M17 Tokenizer** (`tokenizer`): trait + approximation (4 chars/token).
//! - **M18 Chunker** (`chunker`): sliding window with overlap.
//! - **M19 Compressor** (`compressor`): rule-based whitespace + comment strip.
//! - **M20 Reranker** (`reranker`): in-memory BM25.
//!
//! v0 ships approximations + rule-based ops with zero new deps. v0.5 swaps
//! the tokenizer for a real BPE (Qwen / `DeepSeek`) and adds embedding-based
//! reranking. The trait-based design keeps callers stable across versions.

pub mod chunker;
pub mod compressor;
pub mod reranker;
pub mod tokenizer;

pub use chunker::{chunk, chunk_default, Chunk, ChunkOpts};
pub use compressor::{compress, CompressOpts, CompressionResult};
pub use reranker::{rerank, RankedDoc};
pub use tokenizer::{ApproxTokenizer, Tokenizer};
