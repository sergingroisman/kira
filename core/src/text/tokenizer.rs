//! Tokenizer (M17).
//!
//! v0 uses a **character-based approximation** (`1 token ≈ 4 chars`) — the
//! widely used heuristic from `OpenAI` and Anthropic docs for English/code
//! input. Avoids pulling a heavy `tiktoken-rs`/`tokenizers` dependency at
//! this stage. The trait keeps the door open for swapping in a real BPE
//! tokenizer (Qwen / `DeepSeek`) in v0.5 without touching the chunker /
//! compressor / reranker that depend on it.

/// Common interface for any tokenizer.
pub trait Tokenizer {
    /// Approximate token count for the input text.
    fn count(&self, text: &str) -> usize;
    /// Approximate split into token boundaries — used by the chunker.
    /// Returns char offsets, monotonically increasing, ending at `text.len()`.
    fn boundaries(&self, text: &str, target_tokens: usize) -> Vec<usize>;
}

/// Char-approximation tokenizer (`1 token ≈ chars_per_token` chars).
#[derive(Debug, Clone, Copy)]
pub struct ApproxTokenizer {
    pub chars_per_token: usize,
}

impl Default for ApproxTokenizer {
    fn default() -> Self {
        Self { chars_per_token: 4 }
    }
}

impl Tokenizer for ApproxTokenizer {
    fn count(&self, text: &str) -> usize {
        let cpt = self.chars_per_token.max(1);
        text.chars().count().div_ceil(cpt)
    }

    fn boundaries(&self, text: &str, target_tokens: usize) -> Vec<usize> {
        let cpt = self.chars_per_token.max(1);
        let step_chars = target_tokens.saturating_mul(cpt).max(1);
        let mut out = Vec::new();
        let mut acc_chars = 0;
        for (idx, _ch) in text.char_indices() {
            acc_chars += 1;
            if acc_chars >= step_chars {
                out.push(idx + 1);
                acc_chars = 0;
            }
        }
        if out.last().copied() != Some(text.len()) {
            out.push(text.len());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_zero_for_empty() {
        let t = ApproxTokenizer::default();
        assert_eq!(t.count(""), 0);
    }

    #[test]
    fn count_basic_approx() {
        let t = ApproxTokenizer::default();
        // 8 chars / 4 = 2 tokens
        assert_eq!(t.count("abcdefgh"), 2);
        // 9 chars / 4 = 3 tokens (ceil)
        assert_eq!(t.count("abcdefghi"), 3);
    }

    #[test]
    fn boundaries_chunk_into_pieces() {
        let t = ApproxTokenizer::default();
        // 16 chars, target 2 tokens → step 8 chars → boundaries at [8, 16]
        let b = t.boundaries("abcdefghijklmnop", 2);
        assert_eq!(b, vec![8, 16]);
    }

    #[test]
    fn boundaries_always_end_at_len() {
        let t = ApproxTokenizer::default();
        let txt = "short";
        let b = t.boundaries(txt, 100);
        assert_eq!(*b.last().unwrap(), txt.len());
    }
}
