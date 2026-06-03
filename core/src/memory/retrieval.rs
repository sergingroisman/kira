//! Memory Retrieval — Pre-LLM semantic search and injection.
//!
//! Before each LLM call, searches the memory store for the K most relevant
//! fragments by cosine similarity and injects them into the prompt wrapped
//! in `<RAG_CONTEXT source="memory" untrusted="true">`.
//!
//! After cosine search, exponential decay scoring is applied to re-rank
//! candidates by recency. Only fragments in the final result set have their
//! access metadata updated.
//!
//! Latency target: P95 < 200ms for bases with up to 10K fragments.

use chrono::Utc;
use tracing::{debug, warn};

use super::decay::{self, DecayConfig};
use super::store::{MemoryFragment, MemoryStore, MetadataUpdate};
use crate::safety::delimiters;

/// Configuration for memory retrieval.
#[derive(Debug, Clone)]
pub struct RetrievalConfig {
    /// Whether retrieval is enabled.
    pub enabled: bool,
    /// Number of top fragments to retrieve.
    pub top_k: usize,
    /// Minimum cosine similarity score to include a fragment.
    pub min_score: f32,
}

impl Default for RetrievalConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            top_k: 5,
            min_score: 0.70,
        }
    }
}

/// Result of a retrieval operation.
#[derive(Debug, Clone)]
pub struct RetrievalResult {
    /// Fragments retrieved and ready for injection.
    pub fragments: Vec<MemoryFragment>,
    /// The formatted RAG context block to inject into the prompt.
    /// Empty string if no fragments found or retrieval disabled.
    pub rag_block: String,
}

/// Retrieve relevant memory fragments and format them for prompt injection.
///
/// `query_embedding`: the embedding of the current user query/task.
/// `store`: the memory store backend.
/// `config`: retrieval configuration.
/// `decay_config`: decay scoring configuration (decay_factor, decay_floor).
///
/// Returns a `RetrievalResult` with fragments and the formatted RAG block.
/// On store failure, returns empty result (graceful degradation).
pub async fn retrieve<S: MemoryStore>(
    query_embedding: &[f32],
    store: &S,
    config: &RetrievalConfig,
    decay_config: &DecayConfig,
) -> RetrievalResult {
    if !config.enabled {
        return RetrievalResult {
            fragments: Vec::new(),
            rag_block: String::new(),
        };
    }

    // Fetch more candidates than top_k so decay re-ranking has room to reorder.
    // We request 2x top_k (minimum top_k) to give decay a meaningful pool.
    let search_limit = config.top_k * 2;

    let mut candidates = match store.search(query_embedding, search_limit, config.min_score).await {
        Ok(frags) => frags,
        Err(e) => {
            warn!(error = %e, "memory retrieval failed — proceeding without fragments");
            return RetrievalResult {
                fragments: Vec::new(),
                rag_block: String::new(),
            };
        }
    };

    if candidates.is_empty() {
        return RetrievalResult {
            fragments: Vec::new(),
            rag_block: String::new(),
        };
    }

    // Apply decay scoring: compute effective_score for each candidate and
    // store it in the fragment's `score` field for ranking purposes.
    let now = Utc::now();
    for frag in &mut candidates {
        let base_score = f64::from(frag.score);
        let effective = decay::effective_score(
            base_score,
            frag.tier,
            frag.last_accessed,
            frag.created_at,
            now,
            decay_config,
        );
        frag.score = effective as f32;
    }

    // Re-sort by effective score (descending) — this may reorder differently
    // from the raw cosine ranking.
    candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    // Take the final top-K results.
    candidates.truncate(config.top_k);

    debug!(
        fragment_count = candidates.len(),
        "memory retrieval: injecting fragments into prompt (decay-adjusted)"
    );

    // Update access_count and last_accessed for ONLY the returned fragments.
    for frag in &candidates {
        let update = MetadataUpdate {
            access_count: Some(frag.access_count + 1),
            last_accessed: Some(now),
            ..Default::default()
        };
        if let Err(e) = store.update_metadata(frag.fragment_id, update).await {
            warn!(
                fragment_id = %frag.fragment_id,
                error = %e,
                "failed to update access metadata for retrieved fragment"
            );
        }
    }

    let rag_block = format_rag_block(&candidates);

    RetrievalResult {
        fragments: candidates,
        rag_block,
    }
}

/// Format retrieved fragments into a RAG_CONTEXT block for prompt injection.
fn format_rag_block(fragments: &[MemoryFragment]) -> String {
    let mut content = String::new();
    for (i, frag) in fragments.iter().enumerate() {
        content.push_str(&format!(
            "[Memory #{} | tier: {} | score: {:.2} | thread={}]\n{}\n\n",
            i + 1,
            frag.tier.as_str(),
            frag.score,
            &frag.thread_id.to_string()[..8],
            frag.content
        ));
    }
    delimiters::wrap_rag_context("memory", content.trim_end())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::store::InMemoryStore;
    use chrono::Utc;
    use uuid::Uuid;

    fn make_fragment(content: &str, embedding: Vec<f32>) -> MemoryFragment {
        MemoryFragment {
            fragment_id: Uuid::now_v7(),
            content: content.to_string(),
            embedding,
            thread_id: Uuid::now_v7(),
            run_id: Uuid::now_v7(),
            created_at: Utc::now(),
            score: 0.0,
            tier: Default::default(),
            access_count: 0,
            last_accessed: None,
            consolidated_into: None,
            promoted_at: None,
            session_id: None,
        }
    }

    fn default_decay() -> DecayConfig {
        DecayConfig::default()
    }

    #[tokio::test]
    async fn retrieves_relevant_fragments() {
        let store = InMemoryStore::new(100);
        let f1 = make_fragment("user prefers dark mode", vec![1.0, 0.0, 0.0]);
        let f2 = make_fragment("project uses Rust", vec![0.9, 0.1, 0.0]);
        let f3 = make_fragment("unrelated info", vec![0.0, 1.0, 0.0]);

        store.persist(&f1).await.unwrap();
        store.persist(&f2).await.unwrap();
        store.persist(&f3).await.unwrap();

        let config = RetrievalConfig {
            enabled: true,
            top_k: 2,
            min_score: 0.5,
        };

        let query = vec![1.0, 0.0, 0.0];
        let result = retrieve(&query, &store, &config, &default_decay()).await;

        assert_eq!(result.fragments.len(), 2);
        assert!(result.rag_block.contains("RAG_CONTEXT"));
        assert!(result.rag_block.contains("user prefers dark mode"));
        assert!(result.rag_block.contains("project uses Rust"));
        assert!(!result.rag_block.contains("unrelated info"));
    }

    #[tokio::test]
    async fn returns_empty_when_disabled() {
        let store = InMemoryStore::new(100);
        let f1 = make_fragment("data", vec![1.0, 0.0, 0.0]);
        store.persist(&f1).await.unwrap();

        let config = RetrievalConfig {
            enabled: false,
            top_k: 5,
            min_score: 0.0,
        };

        let result = retrieve(&[1.0, 0.0, 0.0], &store, &config, &default_decay()).await;
        assert!(result.fragments.is_empty());
        assert!(result.rag_block.is_empty());
    }

    #[tokio::test]
    async fn returns_empty_when_no_matches() {
        let store = InMemoryStore::new(100);
        let f1 = make_fragment("orthogonal", vec![0.0, 1.0, 0.0]);
        store.persist(&f1).await.unwrap();

        let config = RetrievalConfig::default(); // min_score 0.70

        let query = vec![1.0, 0.0, 0.0]; // orthogonal to f1
        let result = retrieve(&query, &store, &config, &default_decay()).await;
        assert!(result.fragments.is_empty());
        assert!(result.rag_block.is_empty());
    }

    #[tokio::test]
    async fn rag_block_is_wrapped_with_delimiters() {
        let store = InMemoryStore::new(100);
        let f1 = make_fragment("important fact", vec![1.0, 0.0, 0.0]);
        store.persist(&f1).await.unwrap();

        let config = RetrievalConfig {
            enabled: true,
            top_k: 5,
            min_score: 0.0,
        };

        let result = retrieve(&[1.0, 0.0, 0.0], &store, &config, &default_decay()).await;
        assert!(result.rag_block.starts_with("<RAG_CONTEXT source=\"memory\" untrusted=\"true\">"));
        assert!(result.rag_block.ends_with("</RAG_CONTEXT>"));
    }

    #[tokio::test]
    async fn rag_block_includes_tier_in_metadata() {
        let store = InMemoryStore::new(100);
        let mut f1 = make_fragment("semantic knowledge", vec![1.0, 0.0, 0.0]);
        f1.tier = crate::memory::tier::MemoryTier::Semantic;
        store.persist(&f1).await.unwrap();

        let config = RetrievalConfig {
            enabled: true,
            top_k: 5,
            min_score: 0.0,
        };

        let result = retrieve(&[1.0, 0.0, 0.0], &store, &config, &default_decay()).await;
        assert!(result.rag_block.contains("tier: semantic"));
    }

    #[tokio::test]
    async fn fragments_subset_of_store() {
        // Property: search results are always a subset of stored fragments
        let store = InMemoryStore::new(100);
        let frags: Vec<MemoryFragment> = (0..10)
            .map(|i| {
                let mut emb = vec![0.0; 3];
                emb[i % 3] = 1.0;
                make_fragment(&format!("frag {i}"), emb)
            })
            .collect();

        for f in &frags {
            store.persist(f).await.unwrap();
        }

        let config = RetrievalConfig {
            enabled: true,
            top_k: 5,
            min_score: 0.0,
        };

        let result = retrieve(&[1.0, 0.0, 0.0], &store, &config, &default_decay()).await;
        let stored_ids: Vec<Uuid> = frags.iter().map(|f| f.fragment_id).collect();
        for retrieved in &result.fragments {
            assert!(stored_ids.contains(&retrieved.fragment_id));
        }
    }

    #[tokio::test]
    async fn updates_access_metadata_for_returned_fragments_only() {
        let store = InMemoryStore::new(100);
        let f1 = make_fragment("relevant", vec![1.0, 0.0, 0.0]);
        let f2 = make_fragment("also relevant", vec![0.95, 0.05, 0.0]);
        let f3 = make_fragment("orthogonal", vec![0.0, 1.0, 0.0]);
        let f1_id = f1.fragment_id;
        let f2_id = f2.fragment_id;
        let f3_id = f3.fragment_id;

        store.persist(&f1).await.unwrap();
        store.persist(&f2).await.unwrap();
        store.persist(&f3).await.unwrap();

        let config = RetrievalConfig {
            enabled: true,
            top_k: 2,
            min_score: 0.5,
        };

        let _result = retrieve(&[1.0, 0.0, 0.0], &store, &config, &default_decay()).await;

        // Returned fragments should have access_count incremented
        let updated_f1 = store.get_by_id(f1_id).await.unwrap().unwrap();
        let updated_f2 = store.get_by_id(f2_id).await.unwrap().unwrap();
        assert_eq!(updated_f1.access_count, 1);
        assert_eq!(updated_f2.access_count, 1);
        assert!(updated_f1.last_accessed.is_some());
        assert!(updated_f2.last_accessed.is_some());

        // Non-returned fragment should be unchanged
        let updated_f3 = store.get_by_id(f3_id).await.unwrap().unwrap();
        assert_eq!(updated_f3.access_count, 0);
        assert!(updated_f3.last_accessed.is_none());
    }

    #[tokio::test]
    async fn decay_reorders_by_recency() {
        use chrono::Duration;
        use crate::memory::tier::MemoryTier;

        let store = InMemoryStore::new(100);

        // f1: high cosine but old (should decay more)
        let mut f1 = make_fragment("old high score", vec![1.0, 0.0, 0.0]);
        f1.created_at = Utc::now() - Duration::days(30);
        f1.tier = MemoryTier::Working;

        // f2: slightly lower cosine but recent (should decay less)
        let mut f2 = make_fragment("recent lower score", vec![0.95, 0.05, 0.0]);
        f2.created_at = Utc::now() - Duration::hours(1);
        f2.tier = MemoryTier::Working;

        store.persist(&f1).await.unwrap();
        store.persist(&f2).await.unwrap();

        let config = RetrievalConfig {
            enabled: true,
            top_k: 2,
            min_score: 0.0,
        };

        let decay_cfg = DecayConfig {
            decay_factor: 0.95,
            decay_floor: 0.01,
        };

        let result = retrieve(&[1.0, 0.0, 0.0], &store, &config, &decay_cfg).await;

        // The recent fragment should rank higher after decay is applied
        assert_eq!(result.fragments.len(), 2);
        assert_eq!(result.fragments[0].content, "recent lower score");
    }

    #[tokio::test]
    async fn procedural_tier_exempt_from_decay() {
        use chrono::Duration;
        use crate::memory::tier::MemoryTier;

        let store = InMemoryStore::new(100);

        // Procedural fragment: old but should NOT decay
        let mut f1 = make_fragment("procedural knowledge", vec![1.0, 0.0, 0.0]);
        f1.created_at = Utc::now() - Duration::days(100);
        f1.tier = MemoryTier::Procedural;

        // Working fragment: recent
        let mut f2 = make_fragment("recent working", vec![0.9, 0.1, 0.0]);
        f2.created_at = Utc::now() - Duration::hours(1);
        f2.tier = MemoryTier::Working;

        store.persist(&f1).await.unwrap();
        store.persist(&f2).await.unwrap();

        let config = RetrievalConfig {
            enabled: true,
            top_k: 2,
            min_score: 0.0,
        };

        let result = retrieve(&[1.0, 0.0, 0.0], &store, &config, &default_decay()).await;

        // Procedural fragment should still rank first (no decay applied)
        assert_eq!(result.fragments[0].content, "procedural knowledge");
    }
}
