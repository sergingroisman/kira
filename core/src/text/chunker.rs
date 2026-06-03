//! Chunker (M18) — sliding window over token boundaries with overlap.

use serde::Serialize;

use super::tokenizer::{ApproxTokenizer, Tokenizer};

#[derive(Debug, Clone, Serialize)]
pub struct Chunk {
    /// Chunk text.
    pub text: String,
    /// Char offset where this chunk starts in the input.
    pub start: usize,
    /// Char offset where this chunk ends (exclusive).
    pub end: usize,
    /// Approximate token count for this chunk.
    pub tokens: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct ChunkOpts {
    pub target_tokens: usize,
    pub overlap_tokens: usize,
}

impl Default for ChunkOpts {
    fn default() -> Self {
        Self {
            target_tokens: 512,
            overlap_tokens: 50,
        }
    }
}

/// Split `text` into overlapping chunks of approximately `target_tokens` each.
/// Overlap is in tokens (translated to chars via the tokenizer).
pub fn chunk(text: &str, tokenizer: &impl Tokenizer, opts: ChunkOpts) -> Vec<Chunk> {
    if text.is_empty() {
        return Vec::new();
    }
    let total_tokens = tokenizer.count(text);
    if total_tokens <= opts.target_tokens {
        return vec![Chunk {
            text: text.to_string(),
            start: 0,
            end: text.len(),
            tokens: total_tokens,
        }];
    }
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let total_chars = chars.len();
    // Approximate chars per token from the tokenizer's count.
    let cpt = if total_tokens > 0 {
        text.len() / total_tokens.max(1)
    } else {
        4
    };
    let step_chars = opts.target_tokens.saturating_mul(cpt).max(1);
    let overlap_chars = opts.overlap_tokens.saturating_mul(cpt);

    let mut out = Vec::new();
    let mut start_idx = 0usize;
    while start_idx < total_chars {
        let end_idx = (start_idx + step_chars).min(total_chars);
        let start_byte = chars[start_idx].0;
        let end_byte = if end_idx >= total_chars {
            text.len()
        } else {
            chars[end_idx].0
        };
        let slice = &text[start_byte..end_byte];
        out.push(Chunk {
            text: slice.to_string(),
            start: start_byte,
            end: end_byte,
            tokens: tokenizer.count(slice),
        });
        if end_idx >= total_chars {
            break;
        }
        start_idx = end_idx.saturating_sub(overlap_chars);
        if start_idx == 0 && !out.is_empty() {
            // Defensive: avoid infinite loop if overlap == step.
            start_idx = step_chars;
        }
    }
    out
}

/// Convenience: default chunker over the approximation tokenizer.
pub fn chunk_default(text: &str) -> Vec<Chunk> {
    let t = ApproxTokenizer::default();
    chunk(text, &t, ChunkOpts::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input() {
        assert!(chunk_default("").is_empty());
    }

    #[test]
    fn small_input_single_chunk() {
        let txt = "Hello world";
        let chunks = chunk_default(txt);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, txt);
    }

    #[test]
    fn large_input_multiple_chunks() {
        // 10 KB string → with default 512 token target and ~4 chars/token,
        // expect roughly ceil(10240 / 2048) chunks (~5+).
        let txt = "abcd ".repeat(2048); // ~10 KB
        let chunks = chunk_default(&txt);
        assert!(chunks.len() >= 4, "expected multiple chunks, got {}", chunks.len());
    }

    #[test]
    fn chunks_overlap_when_configured() {
        let txt: String = (0..3000).map(|i| ((i % 26) as u8 + b'a') as char).collect();
        let t = ApproxTokenizer::default();
        let opts = ChunkOpts {
            target_tokens: 100,
            overlap_tokens: 20,
        };
        let chunks = chunk(&txt, &t, opts);
        assert!(chunks.len() >= 2);
        // Second chunk should start within the first chunk's end (overlap).
        assert!(chunks[1].start < chunks[0].end);
    }
}
