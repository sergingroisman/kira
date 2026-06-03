//! Memory Store — persistence layer for semantic memory fragments.
//!
//! Trait-based design allows swapping backends without changing callers.
//! Default: sqlite-vec (local file, zero infra).
//! Optional: postgres+pgvector (production, requires PostgreSQL).

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::tier::MemoryTier;

/// A single unit of cross-thread semantic memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryFragment {
    pub fragment_id: Uuid,
    pub content: String,
    pub embedding: Vec<f32>,
    pub thread_id: Uuid,
    pub run_id: Uuid,
    pub created_at: DateTime<Utc>,
    /// Relevance score (0.0–1.0), set during extraction or retrieval.
    pub score: f32,
    // ── New fields (memory maturation) ──
    /// The maturation tier of this fragment.
    #[serde(default)]
    pub tier: MemoryTier,
    /// Number of times this fragment has been accessed (returned in search results).
    #[serde(default)]
    pub access_count: u64,
    /// Timestamp of the last time this fragment was accessed.
    #[serde(default)]
    pub last_accessed: Option<DateTime<Utc>>,
    /// If consolidated, the ID of the higher-tier fragment this was merged into.
    #[serde(default)]
    pub consolidated_into: Option<Uuid>,
    /// Timestamp when this fragment was promoted to its current tier.
    #[serde(default)]
    pub promoted_at: Option<DateTime<Utc>>,
    /// The session that produced this fragment.
    #[serde(default)]
    pub session_id: Option<Uuid>,
}

/// Partial update to a fragment's maturation metadata fields.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetadataUpdate {
    /// Update the tier classification.
    pub tier: Option<MemoryTier>,
    /// Update the access count.
    pub access_count: Option<u64>,
    /// Update the last accessed timestamp.
    pub last_accessed: Option<DateTime<Utc>>,
    /// Update the consolidated_into reference.
    pub consolidated_into: Option<Option<Uuid>>,
    /// Update the promoted_at timestamp.
    pub promoted_at: Option<Option<DateTime<Utc>>>,
}

/// Configuration for the memory store.
#[derive(Debug, Clone)]
pub struct MemoryStoreConfig {
    pub backend: MemoryBackend,
    pub sqlite_path: String,
    pub postgres_url: Option<String>,
    pub max_fragments_per_thread: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryBackend {
    SqliteVec,
    Postgres,
}

impl Default for MemoryStoreConfig {
    fn default() -> Self {
        Self {
            backend: MemoryBackend::SqliteVec,
            sqlite_path: "~/.local/share/kira/memory.db".to_string(),
            postgres_url: None,
            max_fragments_per_thread: 1000,
        }
    }
}

/// Trait for memory storage backends.
#[allow(async_fn_in_trait)]
pub trait MemoryStore: Send + Sync {
    /// Persist a new fragment. If max_fragments_per_thread is exceeded,
    /// the oldest fragment for that thread_id is evicted.
    async fn persist(&self, fragment: &MemoryFragment) -> Result<()>;

    /// Search for the top-K most similar fragments by cosine similarity.
    async fn search(
        &self,
        embedding: &[f32],
        top_k: usize,
        min_score: f32,
    ) -> Result<Vec<MemoryFragment>>;

    /// Get all fragments associated with a specific run_id.
    async fn get_by_run(&self, run_id: Uuid) -> Result<Vec<MemoryFragment>>;

    /// Delete a fragment by its ID. Returns true if found and deleted.
    async fn delete(&self, fragment_id: Uuid) -> Result<bool>;

    /// Count fragments for a given thread_id.
    async fn count_by_thread(&self, thread_id: Uuid) -> Result<usize>;

    /// Get a single fragment by its ID.
    async fn get_by_id(&self, fragment_id: Uuid) -> Result<Option<MemoryFragment>>;

    /// Get all fragments associated with a specific session.
    async fn get_by_session(&self, session_id: Uuid) -> Result<Vec<MemoryFragment>>;

    /// Get all fragments of a given maturation tier.
    async fn get_by_tier(&self, tier: MemoryTier) -> Result<Vec<MemoryFragment>>;

    /// Update maturation metadata fields for a fragment.
    async fn update_metadata(&self, fragment_id: Uuid, updates: MetadataUpdate) -> Result<()>;
}

// ── In-Memory Implementation (for tests and dev) ──────────────────────────────

use std::sync::Mutex;

/// In-memory store — useful for tests and when no backend is configured.
pub struct InMemoryStore {
    fragments: Mutex<Vec<MemoryFragment>>,
    max_per_thread: usize,
}

impl InMemoryStore {
    pub const fn new(max_per_thread: usize) -> Self {
        Self {
            fragments: Mutex::new(Vec::new()),
            max_per_thread,
        }
    }
}

impl MemoryStore for InMemoryStore {
    async fn persist(&self, fragment: &MemoryFragment) -> Result<()> {
        let mut frags = self.fragments.lock().expect("memory store poisoned");

        // Evict oldest if at capacity for this thread
        let thread_count = frags
            .iter()
            .filter(|f| f.thread_id == fragment.thread_id)
            .count();
        if thread_count >= self.max_per_thread {
            // Find and remove the oldest for this thread
            if let Some(oldest_idx) = frags
                .iter()
                .enumerate()
                .filter(|(_, f)| f.thread_id == fragment.thread_id)
                .min_by_key(|(_, f)| f.created_at)
                .map(|(i, _)| i)
            {
                frags.remove(oldest_idx);
            }
        }

        frags.push(fragment.clone());
        Ok(())
    }

    async fn search(
        &self,
        embedding: &[f32],
        top_k: usize,
        min_score: f32,
    ) -> Result<Vec<MemoryFragment>> {
        let frags = self.fragments.lock().expect("memory store poisoned");

        let mut scored: Vec<(f32, &MemoryFragment)> = frags
            .iter()
            .map(|f| (cosine_similarity(embedding, &f.embedding), f))
            .filter(|(score, _)| *score >= min_score)
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);

        Ok(scored
            .into_iter()
            .map(|(score, f)| {
                let mut frag = f.clone();
                frag.score = score;
                frag
            })
            .collect())
    }

    async fn get_by_run(&self, run_id: Uuid) -> Result<Vec<MemoryFragment>> {
        let frags = self.fragments.lock().expect("memory store poisoned");
        Ok(frags
            .iter()
            .filter(|f| f.run_id == run_id)
            .cloned()
            .collect())
    }

    async fn delete(&self, fragment_id: Uuid) -> Result<bool> {
        let mut frags = self.fragments.lock().expect("memory store poisoned");
        let len_before = frags.len();
        frags.retain(|f| f.fragment_id != fragment_id);
        Ok(frags.len() < len_before)
    }

    async fn count_by_thread(&self, thread_id: Uuid) -> Result<usize> {
        let frags = self.fragments.lock().expect("memory store poisoned");
        Ok(frags.iter().filter(|f| f.thread_id == thread_id).count())
    }

    async fn get_by_id(&self, fragment_id: Uuid) -> Result<Option<MemoryFragment>> {
        let frags = self.fragments.lock().expect("memory store poisoned");
        Ok(frags.iter().find(|f| f.fragment_id == fragment_id).cloned())
    }

    async fn get_by_session(&self, session_id: Uuid) -> Result<Vec<MemoryFragment>> {
        let frags = self.fragments.lock().expect("memory store poisoned");
        Ok(frags
            .iter()
            .filter(|f| f.session_id == Some(session_id))
            .cloned()
            .collect())
    }

    async fn get_by_tier(&self, tier: MemoryTier) -> Result<Vec<MemoryFragment>> {
        let frags = self.fragments.lock().expect("memory store poisoned");
        Ok(frags.iter().filter(|f| f.tier == tier).cloned().collect())
    }

    async fn update_metadata(&self, fragment_id: Uuid, updates: MetadataUpdate) -> Result<()> {
        let mut frags = self.fragments.lock().expect("memory store poisoned");
        let fragment = frags
            .iter_mut()
            .find(|f| f.fragment_id == fragment_id)
            .ok_or_else(|| anyhow::anyhow!("fragment not found: {fragment_id}"))?;

        if let Some(tier) = updates.tier {
            fragment.tier = tier;
        }
        if let Some(access_count) = updates.access_count {
            fragment.access_count = access_count;
        }
        if let Some(last_accessed) = updates.last_accessed {
            fragment.last_accessed = Some(last_accessed);
        }
        if let Some(consolidated_into) = updates.consolidated_into {
            fragment.consolidated_into = consolidated_into;
        }
        if let Some(promoted_at) = updates.promoted_at {
            fragment.promoted_at = promoted_at;
        }

        Ok(())
    }
}

/// Cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 {
        return 0.0;
    }
    dot / (mag_a * mag_b)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_fragment(content: &str, embedding: Vec<f32>, thread_id: Uuid, run_id: Uuid) -> MemoryFragment {
        MemoryFragment {
            fragment_id: Uuid::now_v7(),
            content: content.to_string(),
            embedding,
            thread_id,
            run_id,
            created_at: Utc::now(),
            score: 0.0,
            tier: MemoryTier::default(),
            access_count: 0,
            last_accessed: None,
            consolidated_into: None,
            promoted_at: None,
            session_id: None,
        }
    }

    #[tokio::test]
    async fn persist_and_get_by_run() {
        let store = InMemoryStore::new(100);
        let run_id = Uuid::now_v7();
        let thread_id = Uuid::now_v7();
        let frag = make_fragment("hello", vec![1.0, 0.0, 0.0], thread_id, run_id);

        store.persist(&frag).await.unwrap();
        let results = store.get_by_run(run_id).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "hello");
    }

    #[tokio::test]
    async fn round_trip_property() {
        let store = InMemoryStore::new(100);
        let run_id = Uuid::now_v7();
        let thread_id = Uuid::now_v7();
        let frag = make_fragment("test content", vec![0.5, 0.5, 0.5], thread_id, run_id);
        let frag_id = frag.fragment_id;

        store.persist(&frag).await.unwrap();
        let results = store.get_by_run(run_id).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].fragment_id, frag_id);
        assert_eq!(results[0].content, "test content");
        assert_eq!(results[0].embedding, vec![0.5, 0.5, 0.5]);
    }

    #[tokio::test]
    async fn search_returns_top_k_above_min_score() {
        let store = InMemoryStore::new(100);
        let thread_id = Uuid::now_v7();

        // Insert fragments with different embeddings
        let f1 = make_fragment("similar", vec![1.0, 0.0, 0.0], thread_id, Uuid::now_v7());
        let f2 = make_fragment("orthogonal", vec![0.0, 1.0, 0.0], thread_id, Uuid::now_v7());
        let f3 = make_fragment("also similar", vec![0.9, 0.1, 0.0], thread_id, Uuid::now_v7());

        store.persist(&f1).await.unwrap();
        store.persist(&f2).await.unwrap();
        store.persist(&f3).await.unwrap();

        // Search with query similar to f1 and f3
        let query = vec![1.0, 0.0, 0.0];
        let results = store.search(&query, 2, 0.5).await.unwrap();

        assert_eq!(results.len(), 2);
        // f1 should be first (exact match)
        assert_eq!(results[0].content, "similar");
        assert_eq!(results[1].content, "also similar");
        // Scores should be above min
        assert!(results[0].score >= 0.5);
        assert!(results[1].score >= 0.5);
    }

    #[tokio::test]
    async fn search_filters_below_min_score() {
        let store = InMemoryStore::new(100);
        let thread_id = Uuid::now_v7();

        let f1 = make_fragment("orthogonal", vec![0.0, 1.0, 0.0], thread_id, Uuid::now_v7());
        store.persist(&f1).await.unwrap();

        let query = vec![1.0, 0.0, 0.0];
        let results = store.search(&query, 10, 0.5).await.unwrap();
        assert_eq!(results.len(), 0); // orthogonal → cosine = 0
    }

    #[tokio::test]
    async fn delete_removes_fragment() {
        let store = InMemoryStore::new(100);
        let run_id = Uuid::now_v7();
        let thread_id = Uuid::now_v7();
        let frag = make_fragment("to delete", vec![1.0, 0.0, 0.0], thread_id, run_id);
        let frag_id = frag.fragment_id;

        store.persist(&frag).await.unwrap();
        assert!(store.delete(frag_id).await.unwrap());
        assert_eq!(store.get_by_run(run_id).await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn delete_nonexistent_returns_false() {
        let store = InMemoryStore::new(100);
        assert!(!store.delete(Uuid::now_v7()).await.unwrap());
    }

    #[tokio::test]
    async fn evicts_oldest_when_max_per_thread_reached() {
        let store = InMemoryStore::new(2); // max 2 per thread
        let thread_id = Uuid::now_v7();

        let f1 = make_fragment("first", vec![1.0, 0.0, 0.0], thread_id, Uuid::now_v7());
        let f2 = make_fragment("second", vec![0.0, 1.0, 0.0], thread_id, Uuid::now_v7());
        let f3 = make_fragment("third", vec![0.0, 0.0, 1.0], thread_id, Uuid::now_v7());

        store.persist(&f1).await.unwrap();
        store.persist(&f2).await.unwrap();
        store.persist(&f3).await.unwrap(); // should evict f1

        let count = store.count_by_thread(thread_id).await.unwrap();
        assert_eq!(count, 2);

        // f1 should be gone
        assert!(!store.delete(f1.fragment_id).await.unwrap());
        // f2 and f3 should remain
        assert!(store.delete(f2.fragment_id).await.unwrap());
        assert!(store.delete(f3.fragment_id).await.unwrap());
    }

    #[test]
    fn cosine_similarity_identical_vectors() {
        let a = vec![1.0, 2.0, 3.0];
        assert!((cosine_similarity(&a, &a) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_orthogonal_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &b)).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_empty_vectors() {
        assert_eq!(cosine_similarity(&[], &[]), 0.0);
    }

    #[test]
    fn cosine_similarity_mismatched_lengths() {
        assert_eq!(cosine_similarity(&[1.0, 2.0], &[1.0]), 0.0);
    }

    // ── Tests for new maturation trait methods ──

    #[tokio::test]
    async fn get_by_id_returns_fragment() {
        let store = InMemoryStore::new(100);
        let thread_id = Uuid::now_v7();
        let run_id = Uuid::now_v7();
        let frag = make_fragment("find me", vec![1.0, 0.0, 0.0], thread_id, run_id);
        let frag_id = frag.fragment_id;

        store.persist(&frag).await.unwrap();
        let result = store.get_by_id(frag_id).await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().content, "find me");
    }

    #[tokio::test]
    async fn get_by_id_returns_none_for_missing() {
        let store = InMemoryStore::new(100);
        let result = store.get_by_id(Uuid::now_v7()).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn get_by_session_returns_matching_fragments() {
        let store = InMemoryStore::new(100);
        let session_id = Uuid::now_v7();
        let thread_id = Uuid::now_v7();

        let mut f1 = make_fragment("session frag 1", vec![1.0, 0.0, 0.0], thread_id, Uuid::now_v7());
        f1.session_id = Some(session_id);
        let mut f2 = make_fragment("session frag 2", vec![0.0, 1.0, 0.0], thread_id, Uuid::now_v7());
        f2.session_id = Some(session_id);
        let f3 = make_fragment("other session", vec![0.0, 0.0, 1.0], thread_id, Uuid::now_v7());
        // f3 has no session_id

        store.persist(&f1).await.unwrap();
        store.persist(&f2).await.unwrap();
        store.persist(&f3).await.unwrap();

        let results = store.get_by_session(session_id).await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn get_by_tier_returns_matching_fragments() {
        let store = InMemoryStore::new(100);
        let thread_id = Uuid::now_v7();

        let mut f1 = make_fragment("working frag", vec![1.0, 0.0, 0.0], thread_id, Uuid::now_v7());
        f1.tier = MemoryTier::Working;
        let mut f2 = make_fragment("episodic frag", vec![0.0, 1.0, 0.0], thread_id, Uuid::now_v7());
        f2.tier = MemoryTier::Episodic;
        let mut f3 = make_fragment("another working", vec![0.0, 0.0, 1.0], thread_id, Uuid::now_v7());
        f3.tier = MemoryTier::Working;

        store.persist(&f1).await.unwrap();
        store.persist(&f2).await.unwrap();
        store.persist(&f3).await.unwrap();

        let working = store.get_by_tier(MemoryTier::Working).await.unwrap();
        assert_eq!(working.len(), 2);

        let episodic = store.get_by_tier(MemoryTier::Episodic).await.unwrap();
        assert_eq!(episodic.len(), 1);
        assert_eq!(episodic[0].content, "episodic frag");

        let semantic = store.get_by_tier(MemoryTier::Semantic).await.unwrap();
        assert_eq!(semantic.len(), 0);
    }

    #[tokio::test]
    async fn update_metadata_updates_fields() {
        let store = InMemoryStore::new(100);
        let thread_id = Uuid::now_v7();
        let run_id = Uuid::now_v7();
        let frag = make_fragment("updatable", vec![1.0, 0.0, 0.0], thread_id, run_id);
        let frag_id = frag.fragment_id;

        store.persist(&frag).await.unwrap();

        let now = Utc::now();
        let consolidated_id = Uuid::now_v7();
        let updates = MetadataUpdate {
            tier: Some(MemoryTier::Episodic),
            access_count: Some(5),
            last_accessed: Some(now),
            consolidated_into: Some(Some(consolidated_id)),
            promoted_at: Some(Some(now)),
        };

        store.update_metadata(frag_id, updates).await.unwrap();

        let updated = store.get_by_id(frag_id).await.unwrap().unwrap();
        assert_eq!(updated.tier, MemoryTier::Episodic);
        assert_eq!(updated.access_count, 5);
        assert_eq!(updated.last_accessed, Some(now));
        assert_eq!(updated.consolidated_into, Some(consolidated_id));
        assert_eq!(updated.promoted_at, Some(now));
    }

    #[tokio::test]
    async fn update_metadata_partial_update() {
        let store = InMemoryStore::new(100);
        let thread_id = Uuid::now_v7();
        let run_id = Uuid::now_v7();
        let frag = make_fragment("partial", vec![1.0, 0.0, 0.0], thread_id, run_id);
        let frag_id = frag.fragment_id;

        store.persist(&frag).await.unwrap();

        // Only update access_count
        let updates = MetadataUpdate {
            access_count: Some(3),
            ..Default::default()
        };

        store.update_metadata(frag_id, updates).await.unwrap();

        let updated = store.get_by_id(frag_id).await.unwrap().unwrap();
        assert_eq!(updated.tier, MemoryTier::Working); // unchanged
        assert_eq!(updated.access_count, 3); // updated
        assert_eq!(updated.last_accessed, None); // unchanged
    }

    #[tokio::test]
    async fn update_metadata_nonexistent_fragment_errors() {
        let store = InMemoryStore::new(100);
        let updates = MetadataUpdate {
            tier: Some(MemoryTier::Episodic),
            ..Default::default()
        };

        let result = store.update_metadata(Uuid::now_v7(), updates).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn serde_backward_compat_missing_new_fields() {
        // Simulate deserializing a pre-existing fragment without maturation fields
        let json = serde_json::json!({
            "fragment_id": "01234567-89ab-cdef-0123-456789abcdef",
            "content": "old fragment",
            "embedding": [1.0, 0.0, 0.0],
            "thread_id": "01234567-89ab-cdef-0123-456789abcdef",
            "run_id": "01234567-89ab-cdef-0123-456789abcdef",
            "created_at": "2024-01-01T00:00:00Z",
            "score": 0.5
        });

        let frag: MemoryFragment = serde_json::from_value(json).unwrap();
        assert_eq!(frag.tier, MemoryTier::Working); // default
        assert_eq!(frag.access_count, 0); // default
        assert_eq!(frag.last_accessed, None); // default
        assert_eq!(frag.consolidated_into, None); // default
        assert_eq!(frag.promoted_at, None); // default
        assert_eq!(frag.session_id, None); // default
    }
}
