//! Embedding cache backed by redb.
//!
//! Key:   `<model>::<fnv1a64(text)>`
//! Value: JSON-encoded `Vec<f32>` (serde compact)
//!
//! Strategy: on `embed_or_compute(...)`, look up each input. Misses become
//! a single batched call to the provider with only the missing inputs. Hits
//! are merged back in original input order. Without `db`, falls through to
//! provider directly (no-op cache).

use anyhow::Result;
use std::sync::Arc;
use tracing::{debug, instrument, warn};

use crate::llm::LlmProvider;
use crate::state::{KiraDb, TBL_EMBED_CACHE};

/// Cap on cached embeddings. Exceed → reactive eviction.
const MAX_ENTRIES: u64 = 5_000;
/// Rows removed per eviction sweep (iter order, no LRU).
const EVICT_BATCH: usize = 500;

#[derive(Clone)]
pub struct EmbedCache {
    db: Option<KiraDb>,
}

impl EmbedCache {
    pub fn new(db: Option<KiraDb>) -> Self {
        Self { db }
    }

    /// Embed `inputs` using the cache when available. Provider is called
    /// only for cache misses (single batched request).
    #[instrument(skip(self, provider, inputs), fields(model = %model, n = inputs.len()))]
    pub async fn embed_or_compute(
        &self,
        provider: &Arc<LlmProvider>,
        model: &str,
        inputs: Vec<String>,
    ) -> Result<Vec<Vec<f32>>> {
        let Some(db) = &self.db else {
            return provider.embed(model, inputs).await;
        };

        let mut results: Vec<Option<Vec<f32>>> = vec![None; inputs.len()];
        let mut miss_indices: Vec<usize> = Vec::new();
        let mut miss_inputs: Vec<String> = Vec::new();

        // Lookup phase.
        for (i, text) in inputs.iter().enumerate() {
            let key = cache_key(model, text);
            match db.list_one(TBL_EMBED_CACHE, &key) {
                Ok(Some(json)) => match serde_json::from_str::<Vec<f32>>(&json) {
                    Ok(v) => {
                        results[i] = Some(v);
                    }
                    Err(_) => {
                        miss_indices.push(i);
                        miss_inputs.push(text.clone());
                    }
                },
                _ => {
                    miss_indices.push(i);
                    miss_inputs.push(text.clone());
                }
            }
        }

        let hits = inputs.len() - miss_inputs.len();
        debug!(hits, misses = miss_inputs.len(), "embed cache lookup");

        // Compute misses in a single batched call.
        if !miss_inputs.is_empty() {
            let fetched = provider.embed(model, miss_inputs.clone()).await?;
            if fetched.len() != miss_indices.len() {
                anyhow::bail!(
                    "provider returned {} vectors, expected {}",
                    fetched.len(),
                    miss_indices.len()
                );
            }
            for (slot, (orig_idx, vec)) in
                miss_indices.iter().zip(fetched.into_iter()).enumerate()
            {
                let key = cache_key(model, &inputs[*orig_idx]);
                let json = serde_json::to_string(&vec).unwrap_or_else(|_| "[]".into());
                if let Err(e) = db.put_raw(TBL_EMBED_CACHE, &key, &json) {
                    warn!(error = ?e, key = %key, "embed cache write failed");
                }
                results[*orig_idx] = Some(vec);
                let _ = slot;
            }

            // Reactive eviction — keep table bounded.
            match db.count(TBL_EMBED_CACHE) {
                Ok(c) if c > MAX_ENTRIES => {
                    match db.delete_n(TBL_EMBED_CACHE, EVICT_BATCH) {
                        Ok(n) => debug!(removed = n, total = c, "embed cache evicted"),
                        Err(e) => warn!(error = ?e, "embed cache evict failed"),
                    }
                }
                Ok(_) => {}
                Err(e) => warn!(error = ?e, "embed cache count failed"),
            }
        }

        // Final assembly — every slot must be filled by now.
        results
            .into_iter()
            .map(|o| o.ok_or_else(|| anyhow::anyhow!("missing embedding slot — shouldn't happen")))
            .collect()
    }
}

fn cache_key(model: &str, text: &str) -> String {
    // FNV-1a 64-bit, hex. Cheap, stable across runs.
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in model.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash ^= u64::from(b':');
    hash = hash.wrapping_mul(0x100000001b3);
    for b in text.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{model}::{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_stable() {
        assert_eq!(cache_key("a", "hello"), cache_key("a", "hello"));
        assert_ne!(cache_key("a", "hello"), cache_key("b", "hello"));
        assert_ne!(cache_key("a", "hello"), cache_key("a", "world"));
    }

    #[test]
    fn key_format() {
        let k = cache_key("text-embedding-v3", "x");
        assert!(k.starts_with("text-embedding-v3::"));
        assert_eq!(k.len(), "text-embedding-v3::".len() + 16);
    }
}
