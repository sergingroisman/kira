//! Reranker (M20) — in-memory BM25 over query + candidate documents.
//!
//! BM25 parameters use the Lucene defaults (`k1 = 1.5`, `b = 0.75`). No
//! external crate — keeps the dependency surface lean.

use serde::Serialize;
use std::collections::HashMap;

const K1: f64 = 1.5;
const B: f64 = 0.75;

#[derive(Debug, Clone, Serialize)]
pub struct RankedDoc {
    pub index: usize,
    pub score: f64,
    pub text: String,
}

/// Rank `documents` against `query` by BM25, return top `k` (or all if `k=0`).
pub fn rerank(query: &str, documents: &[String], k: usize) -> Vec<RankedDoc> {
    if documents.is_empty() {
        return Vec::new();
    }
    let q_terms = tokenize(query);
    if q_terms.is_empty() {
        return Vec::new();
    }
    let docs_tok: Vec<Vec<String>> = documents.iter().map(|d| tokenize(d)).collect();
    let n_docs = docs_tok.len() as f64;
    let avg_len: f64 =
        docs_tok.iter().map(|d| d.len() as f64).sum::<f64>() / n_docs.max(1.0);

    // Document frequency for each query term.
    let mut df: HashMap<&str, f64> = HashMap::new();
    for term in &q_terms {
        let count = docs_tok
            .iter()
            .filter(|d| d.iter().any(|t| t == term))
            .count();
        df.insert(term.as_str(), count as f64);
    }

    let mut scored: Vec<RankedDoc> = docs_tok
        .iter()
        .enumerate()
        .map(|(i, doc)| {
            let len = doc.len() as f64;
            let mut score = 0.0;
            // Term frequency in this doc.
            let mut tf: HashMap<&str, f64> = HashMap::new();
            for t in doc {
                *tf.entry(t.as_str()).or_insert(0.0) += 1.0;
            }
            for term in &q_terms {
                let term_str = term.as_str();
                let f = *tf.get(term_str).unwrap_or(&0.0);
                if f == 0.0 {
                    continue;
                }
                let df_term = *df.get(term_str).unwrap_or(&0.0);
                let idf = ((n_docs - df_term + 0.5) / (df_term + 0.5)).ln_1p();
                let denom = K1.mul_add(1.0 - B + B * len / avg_len.max(1e-9), f);
                score += idf * (f * (K1 + 1.0)) / denom.max(1e-9);
            }
            RankedDoc {
                index: i,
                score,
                text: documents[i].clone(),
            }
        })
        .collect();

    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    if k > 0 && scored.len() > k {
        scored.truncate(k);
    }
    scored
}

/// Tokenize text — lowercase + split on non-alphanumeric.
fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_docs_empty_result() {
        assert!(rerank("query", &[], 5).is_empty());
    }

    #[test]
    fn relevant_doc_ranks_first() {
        let docs = vec![
            "the quick brown fox jumps over the lazy dog".to_string(),
            "rust async tokio runtime executor".to_string(),
            "javascript node express middleware router".to_string(),
        ];
        let r = rerank("rust tokio async", &docs, 0);
        assert_eq!(r[0].index, 1);
        assert!(r[0].score > r[1].score);
    }

    #[test]
    fn top_k_truncates() {
        let docs: Vec<String> = (0..10).map(|i| format!("doc {i} with content")).collect();
        let r = rerank("doc content", &docs, 3);
        assert_eq!(r.len(), 3);
    }

    #[test]
    fn no_matching_query_terms_returns_zero_scores() {
        let docs = vec!["completely unrelated text".to_string()];
        let r = rerank("xyzzy", &docs, 0);
        assert_eq!(r.len(), 1);
        assert!(r[0].score.abs() < f64::EPSILON);
    }
}
