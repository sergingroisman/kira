//! Memory Extractor — Post-run async extraction of persistible fragments.
//!
//! After a run completes successfully, analyzes the transcript to identify
//! information worth persisting as memory fragments (preferences, facts,
//! patterns, decisions). Uses tier `light` LLM for classification.
//!
//! Runs asynchronously — never blocks the response to the user.

use anyhow::Result;
use chrono::Utc;
use tracing::{debug, warn};
use uuid::Uuid;

use super::store::{MemoryFragment, MemoryStore};
use super::tier::MemoryTier;

/// Configuration for the memory extractor.
#[derive(Debug, Clone)]
pub struct ExtractorConfig {
    /// Minimum relevance score for a fragment to be persisted.
    pub min_score: f32,
    /// Cosine similarity threshold for deduplication.
    pub dedup_threshold: f32,
}

impl Default for ExtractorConfig {
    fn default() -> Self {
        Self {
            min_score: 0.60,
            dedup_threshold: 0.95,
        }
    }
}

/// A candidate fragment identified from a transcript.
#[derive(Debug, Clone)]
pub struct ExtractionCandidate {
    pub content: String,
    pub score: f32,
    pub embedding: Vec<f32>,
}

/// Trait for the LLM-based extraction classifier.
/// Implementations: real LLM call (production) or mock (tests).
#[allow(async_fn_in_trait)]
pub trait ExtractionClassifier: Send + Sync {
    /// Analyze a transcript and return candidate fragments with scores.
    async fn classify(&self, transcript: &str) -> Result<Vec<ExtractionCandidate>>;
}

/// Extract and persist memory fragments from a completed run's transcript.
///
/// This function is designed to be called via `tokio::spawn` after a run
/// completes — it never blocks the main response path.
pub async fn extract_and_persist<S: MemoryStore, C: ExtractionClassifier>(
    transcript: &str,
    thread_id: Uuid,
    run_id: Uuid,
    store: &S,
    classifier: &C,
    config: &ExtractorConfig,
) -> Result<u32> {
    // 1. Classify transcript into candidates
    let candidates = match classifier.classify(transcript).await {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, run_id = %run_id, "memory extractor: classification failed");
            return Ok(0);
        }
    };

    if candidates.is_empty() {
        debug!(run_id = %run_id, "memory extractor: no candidates found");
        return Ok(0);
    }

    // 2. Filter by min_score
    let above_threshold: Vec<&ExtractionCandidate> = candidates
        .iter()
        .filter(|c| c.score >= config.min_score)
        .collect();

    if above_threshold.is_empty() {
        debug!(run_id = %run_id, "memory extractor: all candidates below min_score");
        return Ok(0);
    }

    // 3. Deduplicate against existing fragments
    let mut persisted = 0u32;
    for candidate in above_threshold {
        if is_duplicate(candidate, store, config.dedup_threshold).await {
            debug!(
                content_preview = &candidate.content[..candidate.content.len().min(40)],
                "memory extractor: skipping duplicate"
            );
            continue;
        }

        // 4. Persist
        let fragment = MemoryFragment {
            fragment_id: Uuid::now_v7(),
            content: candidate.content.clone(),
            embedding: candidate.embedding.clone(),
            thread_id,
            run_id,
            created_at: Utc::now(),
            score: candidate.score,
            tier: MemoryTier::default(),
            access_count: 0,
            last_accessed: None,
            consolidated_into: None,
            promoted_at: None,
            session_id: None,
        };

        if let Err(e) = store.persist(&fragment).await {
            warn!(error = %e, "memory extractor: failed to persist fragment");
            continue;
        }

        persisted += 1;
    }

    debug!(
        run_id = %run_id,
        persisted,
        total_candidates = candidates.len(),
        "memory extractor: extraction complete"
    );

    Ok(persisted)
}

/// Check if a candidate is a duplicate of an existing fragment.
async fn is_duplicate<S: MemoryStore>(
    candidate: &ExtractionCandidate,
    store: &S,
    threshold: f32,
) -> bool {
    match store.search(&candidate.embedding, 1, threshold).await {
        Ok(results) => !results.is_empty(),
        Err(_) => false, // On error, assume not duplicate (fail-open)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::store::InMemoryStore;

    struct MockClassifier {
        candidates: Vec<ExtractionCandidate>,
    }

    impl ExtractionClassifier for MockClassifier {
        async fn classify(&self, _transcript: &str) -> Result<Vec<ExtractionCandidate>> {
            Ok(self.candidates.clone())
        }
    }

    struct FailingClassifier;

    impl ExtractionClassifier for FailingClassifier {
        async fn classify(&self, _transcript: &str) -> Result<Vec<ExtractionCandidate>> {
            anyhow::bail!("LLM timeout")
        }
    }

    #[tokio::test]
    async fn extracts_and_persists_above_threshold() {
        let store = InMemoryStore::new(100);
        let classifier = MockClassifier {
            candidates: vec![
                ExtractionCandidate {
                    content: "user prefers dark mode".to_string(),
                    score: 0.85,
                    embedding: vec![1.0, 0.0, 0.0],
                },
                ExtractionCandidate {
                    content: "low relevance".to_string(),
                    score: 0.30, // below threshold
                    embedding: vec![0.0, 1.0, 0.0],
                },
            ],
        };
        let config = ExtractorConfig::default(); // min_score 0.60

        let thread_id = Uuid::now_v7();
        let run_id = Uuid::now_v7();
        let count = extract_and_persist(
            "transcript text",
            thread_id,
            run_id,
            &store,
            &classifier,
            &config,
        )
        .await
        .unwrap();

        assert_eq!(count, 1);
        let frags = store.get_by_run(run_id).await.unwrap();
        assert_eq!(frags.len(), 1);
        assert_eq!(frags[0].content, "user prefers dark mode");
    }

    #[tokio::test]
    async fn deduplicates_existing_fragments() {
        let store = InMemoryStore::new(100);

        // Pre-existing fragment with same embedding
        let existing = MemoryFragment {
            fragment_id: Uuid::now_v7(),
            content: "user prefers dark mode".to_string(),
            embedding: vec![1.0, 0.0, 0.0],
            thread_id: Uuid::now_v7(),
            run_id: Uuid::now_v7(),
            created_at: Utc::now(),
            score: 0.9,
            tier: MemoryTier::default(),
            access_count: 0,
            last_accessed: None,
            consolidated_into: None,
            promoted_at: None,
            session_id: None,
        };
        store.persist(&existing).await.unwrap();

        let classifier = MockClassifier {
            candidates: vec![ExtractionCandidate {
                content: "user likes dark theme".to_string(), // semantically same
                score: 0.80,
                embedding: vec![1.0, 0.0, 0.0], // identical embedding → cosine = 1.0
            }],
        };
        let config = ExtractorConfig {
            min_score: 0.60,
            dedup_threshold: 0.95,
        };

        let count = extract_and_persist(
            "transcript",
            Uuid::now_v7(),
            Uuid::now_v7(),
            &store,
            &classifier,
            &config,
        )
        .await
        .unwrap();

        assert_eq!(count, 0); // deduplicated
    }

    #[tokio::test]
    async fn graceful_on_classifier_failure() {
        let store = InMemoryStore::new(100);
        let classifier = FailingClassifier;
        let config = ExtractorConfig::default();

        let count = extract_and_persist(
            "transcript",
            Uuid::now_v7(),
            Uuid::now_v7(),
            &store,
            &classifier,
            &config,
        )
        .await
        .unwrap();

        assert_eq!(count, 0); // no panic, graceful
    }

    #[tokio::test]
    async fn returns_zero_on_empty_candidates() {
        let store = InMemoryStore::new(100);
        let classifier = MockClassifier {
            candidates: Vec::new(),
        };
        let config = ExtractorConfig::default();

        let count = extract_and_persist(
            "transcript",
            Uuid::now_v7(),
            Uuid::now_v7(),
            &store,
            &classifier,
            &config,
        )
        .await
        .unwrap();

        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn does_not_block_on_store_failure() {
        // InMemoryStore won't fail, but the function handles errors gracefully
        let store = InMemoryStore::new(100);
        let classifier = MockClassifier {
            candidates: vec![
                ExtractionCandidate {
                    content: "fact 1".to_string(),
                    score: 0.90,
                    embedding: vec![1.0, 0.0, 0.0],
                },
                ExtractionCandidate {
                    content: "fact 2".to_string(),
                    score: 0.85,
                    embedding: vec![0.0, 1.0, 0.0],
                },
            ],
        };
        let config = ExtractorConfig::default();

        let count = extract_and_persist(
            "transcript",
            Uuid::now_v7(),
            Uuid::now_v7(),
            &store,
            &classifier,
            &config,
        )
        .await
        .unwrap();

        assert_eq!(count, 2);
    }
}
