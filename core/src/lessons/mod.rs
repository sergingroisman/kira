//! Cognition Lessons — Spektor P7 / Kira G7 flywheel.
//!
//! Lifecycle:
//!   1. Gate fails → `LessonGenerator` builds a compact Lesson via Light-tier LLM.
//!   2. `LessonRegistry.record(lesson)` dedupes by hash(trigger, `anti_pattern)`:
//!      duplicate → increment `recurrence`; new → insert as `Active`.
//!   3. `AgentManager.spawn` calls `LessonRegistry.inject_for(kind)` to fetch
//!      active lessons matching scope, top-N by priority + recency. These get
//!      appended to the kind's system prompt before the LLM sees it.
//!   4. When `recurrence >= LESSON_PROMOTE_THRESHOLD`, optional `promote(id)`
//!      writes the lesson as a markdown file under `<repo>/AGENTS-memory/lessons/`
//!      (versioned by git) and flips status to `Promoted`.
//!
//! Storage: redb table `lessons` (durable). In-memory cache wrapping it like
//! the other Stores.

pub mod generator;
pub mod store;

use anyhow::{Context, Result};
use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use tracing::{info, instrument};
use uuid::Uuid;

use crate::proto::agent::AgentKind;
use crate::proto::lesson::{Lesson, LessonPriority, LessonScope, LessonStatus};
use crate::text::{compress, rerank, CompressOpts};

pub const LESSON_PROMOTE_THRESHOLD: u32 = 5;
pub const INJECT_TOP_N: usize = 5;

use self::store::Store;

#[derive(Clone)]
pub struct LessonRegistry {
    store: Store,
    repo_root: PathBuf,
    /// Dedup key (`hash(trigger + anti_pattern)`) → lesson id.
    /// Avoids a full scan on each `record()` call.
    dedup_index: Arc<Mutex<HashMap<String, String>>>,
}

impl LessonRegistry {
    pub fn new(repo_root: PathBuf, db: Option<crate::state::KiraDb>) -> Self {
        let store = match db {
            Some(d) => Store::with_db(d),
            None => Store::new(),
        };
        // Build dedup index from persisted lessons.
        let mut idx: HashMap<String, String> = HashMap::new();
        for l in store.list() {
            idx.insert(dedup_key(&l.trigger, &l.anti_pattern), l.id.clone());
        }
        Self {
            store,
            repo_root,
            dedup_index: Arc::new(Mutex::new(idx)),
        }
    }

    pub fn list(&self) -> Vec<Lesson> {
        self.store.list()
    }

    pub fn get(&self, id: Uuid) -> Option<Lesson> {
        self.store.get(id)
    }

    pub fn delete(&self, id: Uuid) -> bool {
        let removed = self.store.remove(id);
        if let Some(l) = &removed {
            self.dedup_index
                .lock()
                .expect("lessons dedup poisoned")
                .remove(&dedup_key(&l.trigger, &l.anti_pattern));
        }
        removed.is_some()
    }

    /// Record a lesson. If an active one with the same (trigger, `anti_pattern`)
    /// hash exists, increment `recurrence` and update `last_seen_at`.
    /// Otherwise insert as new Active. Returns the resulting lesson.
    #[instrument(skip(self, trigger, anti_pattern, preferred_pattern), fields(priority = ?priority))]
    pub fn record(
        &self,
        trigger: String,
        anti_pattern: String,
        preferred_pattern: String,
        priority: LessonPriority,
        scope: LessonScope,
    ) -> Lesson {
        let key = dedup_key(&trigger, &anti_pattern);
        let now = Utc::now().to_rfc3339();

        let existing_id = self
            .dedup_index
            .lock()
            .expect("lessons dedup poisoned")
            .get(&key)
            .cloned();

        if let Some(id_str) = existing_id
            && let Ok(uuid) = Uuid::parse_str(&id_str) {
                self.store.update(uuid, |l| {
                    l.recurrence = l.recurrence.saturating_add(1);
                    l.last_seen_at = now.clone();
                });
                if let Some(updated) = self.store.get(uuid) {
                    info!(id = %id_str, recurrence = updated.recurrence, "lesson recurrence++");
                    // Auto-promote when recurrence crosses threshold.
                    if updated.recurrence >= LESSON_PROMOTE_THRESHOLD
                        && updated.status == LessonStatus::Active
                    {
                        match self.promote(uuid) {
                            Ok(Some(path)) => {
                                info!(id = %id_str, path = %path.display(), "lesson auto-promoted (recurrence threshold)");
                            }
                            Ok(None) => {} // already promoted
                            Err(e) => {
                                tracing::warn!(error = ?e, id = %id_str, "auto-promote failed");
                            }
                        }
                        // Return refreshed snapshot reflecting promoted status.
                        if let Some(refreshed) = self.store.get(uuid) {
                            return refreshed;
                        }
                    }
                    return updated;
                }
            }

        let id = Uuid::now_v7();
        let lesson = Lesson {
            id: id.to_string(),
            trigger,
            anti_pattern,
            preferred_pattern,
            priority,
            scope,
            status: LessonStatus::Active,
            recurrence: 1,
            created_at: now.clone(),
            last_seen_at: now,
            promoted_at: None,
        };
        self.store.insert(id, lesson.clone());
        self.dedup_index
            .lock()
            .expect("lessons dedup poisoned")
            .insert(key, lesson.id.clone());
        info!(id = %lesson.id, "lesson created");
        lesson
    }

    /// Get lessons that should be injected into an agent's system prompt.
    ///
    /// Pipeline (M20 — Sprint 3):
    /// 1. Filter: scope + Active status.
    /// 2. Sort: priority desc, then recency desc (deterministic baseline).
    /// 3. If `task` is non-empty: rerank top-`2×INJECT_TOP_N` candidates via
    ///    BM25 against the task text → keeps the most *relevant* lessons.
    ///    BM25 score is blended with priority weight so High-priority lessons
    ///    still beat irrelevant Low ones.
    /// 4. Cap at `INJECT_TOP_N`.
    pub fn inject_for(&self, kind: AgentKind, task: &str) -> Vec<Lesson> {
        let mut matching: Vec<Lesson> = self
            .store
            .list()
            .into_iter()
            .filter(|l| l.status == LessonStatus::Active)
            .filter(|l| scope_matches(&l.scope, kind))
            .collect();

        // Step 2: priority + recency baseline sort.
        matching.sort_by(|a, b| {
            let pa = priority_rank(a.priority);
            let pb = priority_rank(b.priority);
            pb.cmp(&pa)
                .then_with(|| b.last_seen_at.cmp(&a.last_seen_at))
        });

        // Step 3: BM25 rerank when we have a task and more lessons than INJECT_TOP_N.
        // Widen the candidate window to 2×INJECT_TOP_N before reranking so BM25
        // can pick the most relevant from a broader pool.
        if !task.is_empty() && matching.len() > INJECT_TOP_N {
            let pool: Vec<_> = matching.iter().take(INJECT_TOP_N * 2).collect();
            let docs: Vec<String> = pool
                .iter()
                .map(|l| format!("{} {} {}", l.trigger, l.anti_pattern, l.preferred_pattern))
                .collect();
            let ranked = rerank(task, &docs, pool.len());
            // Blend BM25 rank with priority weight so High > Medium > Low.
            let mut blended: Vec<(usize, f64)> = ranked
                .iter()
                .map(|r| {
                    let prio_w = f64::from(priority_rank(pool[r.index].priority));
                    (r.index, prio_w.mul_add(0.5, r.score))
                })
                .collect();
            blended.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            blended.truncate(INJECT_TOP_N);
            matching = blended.into_iter().map(|(i, _)| pool[i].clone()).collect();
        } else {
            matching.truncate(INJECT_TOP_N);
        }

        matching
    }

    /// Promote a lesson: write `<repo>/AGENTS-memory/lessons/<id>.md` and flip
    /// status to `Promoted`. No-op if already promoted.
    pub fn promote(&self, id: Uuid) -> Result<Option<PathBuf>> {
        let lesson = self.store.get(id).context("lesson not found")?;
        if lesson.status == LessonStatus::Promoted {
            return Ok(None);
        }
        let dir = self.repo_root.join("AGENTS-memory").join("lessons");
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("mkdir failed: {}", dir.display()))?;
        let path = dir.join(format!("{}.md", lesson.id));
        std::fs::write(&path, render_promoted_md(&lesson))
            .with_context(|| format!("write failed: {}", path.display()))?;

        let now = Utc::now().to_rfc3339();
        self.store.update(id, |l| {
            l.status = LessonStatus::Promoted;
            l.promoted_at = Some(now);
        });
        info!(id = %id, path = %path.display(), "lesson promoted");
        Ok(Some(path))
    }
}

fn scope_matches(scope: &LessonScope, kind: AgentKind) -> bool {
    match scope {
        LessonScope::Global => true,
        LessonScope::AgentKind { agent_kind } => *agent_kind == kind,
        LessonScope::Workflow { .. } => false, // v0.6
    }
}

const fn priority_rank(p: LessonPriority) -> u8 {
    match p {
        LessonPriority::High => 3,
        LessonPriority::Medium => 2,
        LessonPriority::Low => 1,
    }
}

fn dedup_key(trigger: &str, anti_pattern: &str) -> String {
    // Cheap stable hash — concat + lowercase normalize. Cryptographic strength
    // not required; collisions just mean an extra recurrence bump.
    let s = format!("{}::{}", trigger.trim().to_lowercase(), anti_pattern.trim().to_lowercase());
    // FNV-1a 64-bit
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn render_promoted_md(l: &Lesson) -> String {
    let scope_str = match &l.scope {
        LessonScope::Global => "global".to_string(),
        LessonScope::AgentKind { agent_kind } => format!("agent_kind={agent_kind:?}").to_lowercase(),
        LessonScope::Workflow { workflow } => format!("workflow={workflow}"),
    };
    format!(
        "# Lesson {id}\n\n\
         **Trigger**: {trigger}\n\n\
         **Anti-pattern**: {anti}\n\n\
         **Preferred pattern**: {pref}\n\n\
         **Priority**: {prio:?}\n  \
         **Scope**: {scope}\n  \
         **Recurrence**: {rec}\n  \
         **First seen**: {first}\n  \
         **Promoted at**: {promo}\n  \
         **Promoted by**: kira (auto)\n",
        id = l.id,
        trigger = l.trigger,
        anti = l.anti_pattern,
        pref = l.preferred_pattern,
        prio = l.priority,
        scope = scope_str,
        rec = l.recurrence,
        first = l.created_at,
        promo = l
            .promoted_at
            .clone()
            .unwrap_or_else(|| Utc::now().to_rfc3339()),
    )
}

/// Convert a list of lessons into a snippet to append to a system prompt.
///
/// M19 (Sprint 3): each lesson's `anti_pattern` and `preferred_pattern` texts
/// are compressed (whitespace collapse, comment strip) before injection to
/// prevent prompt bloat when lessons accumulate long descriptions.
///
/// Empty input → empty string (caller can short-circuit).
pub fn render_inject_snippet(lessons: &[Lesson]) -> String {
    if lessons.is_empty() {
        return String::new();
    }
    let opts = CompressOpts {
        target_ratio: 0.8, // 20% reduction max — lessons are already compact
        strip_comments: false, // lesson text is natural language, not code
    };
    let mut out = String::from(
        "\n\n### Lessons learned (apply these to avoid past mistakes):\n",
    );
    for l in lessons {
        let anti = compress(&l.anti_pattern, opts).output;
        let pref = compress(&l.preferred_pattern, opts).output;
        out.push_str(&format!(
            "- (priority={prio:?}, x{rec}) AVOID: {anti}  →  DO: {pref}\n",
            prio = l.priority,
            rec = l.recurrence,
            anti = anti.trim(),
            pref = pref.trim(),
        ));
    }
    out
}

#[allow(dead_code)]
fn _system_time_marker() -> SystemTime {
    SystemTime::now()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> LessonRegistry {
        LessonRegistry::new(std::env::temp_dir(), None)
    }

    #[test]
    fn dedup_increments_recurrence() {
        let r = registry();
        let a = r.record(
            "judge_failed".into(),
            "agent truncated output".into(),
            "set max_tokens higher OR split task".into(),
            LessonPriority::High,
            LessonScope::Global,
        );
        assert_eq!(a.recurrence, 1);
        let b = r.record(
            "judge_failed".into(),
            "agent truncated output".into(),
            "set max_tokens higher OR split task".into(),
            LessonPriority::High,
            LessonScope::Global,
        );
        assert_eq!(a.id, b.id);
        assert_eq!(b.recurrence, 2);
        assert_eq!(r.list().len(), 1);
    }

    #[test]
    fn inject_filters_by_scope() {
        let r = registry();
        r.record(
            "writer issue".into(),
            "anti".into(),
            "do".into(),
            LessonPriority::High,
            LessonScope::AgentKind {
                agent_kind: AgentKind::Writer,
            },
        );
        r.record(
            "global issue".into(),
            "anti g".into(),
            "do g".into(),
            LessonPriority::Low,
            LessonScope::Global,
        );
        let for_writer = r.inject_for(AgentKind::Writer, "");
        assert_eq!(for_writer.len(), 2);
        let for_deployer = r.inject_for(AgentKind::Deployer, "");
        assert_eq!(for_deployer.len(), 1); // only Global
    }

    #[test]
    fn inject_sorts_priority_then_recency() {
        let r = registry();
        r.record(
            "low".into(),
            "anti l".into(),
            "do l".into(),
            LessonPriority::Low,
            LessonScope::Global,
        );
        r.record(
            "high".into(),
            "anti h".into(),
            "do h".into(),
            LessonPriority::High,
            LessonScope::Global,
        );
        let sorted = r.inject_for(AgentKind::Writer, "");
        assert_eq!(sorted[0].priority, LessonPriority::High);
    }

    #[test]
    fn snippet_empty_for_empty_list() {
        assert!(render_inject_snippet(&[]).is_empty());
    }

    #[test]
    fn snippet_includes_each_lesson() {
        let r = registry();
        r.record(
            "x".into(),
            "anti".into(),
            "do".into(),
            LessonPriority::High,
            LessonScope::Global,
        );
        let s = render_inject_snippet(&r.list());
        assert!(s.contains("AVOID: anti"));
        assert!(s.contains("DO: do"));
    }
}
