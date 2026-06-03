//! Cognition Lessons wire types — Spektor P7 / Kira G7 (flywheel).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::proto::agent::AgentKind;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../agent/src/generated/lesson.ts")]
#[serde(rename_all = "snake_case")]
pub enum LessonPriority {
    Low,
    Medium,
    High,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../agent/src/generated/lesson.ts")]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LessonScope {
    /// Apply to every agent.
    Global,
    /// Apply to agents of this kind only.
    AgentKind { agent_kind: AgentKind },
    /// Apply only when this workflow id is active (v0.6+).
    Workflow { workflow: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../agent/src/generated/lesson.ts")]
#[serde(rename_all = "snake_case")]
pub enum LessonStatus {
    Active,
    Archived,
    Promoted,
}

#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/lesson.ts")]
pub struct Lesson {
    pub id: String,
    /// What signal triggered the lesson (e.g. "`judge_failed`: writer truncated output").
    pub trigger: String,
    /// What the agent did wrong (one short sentence, imperative form).
    pub anti_pattern: String,
    /// What to do instead (one short sentence, imperative form).
    pub preferred_pattern: String,
    pub priority: LessonPriority,
    pub scope: LessonScope,
    pub status: LessonStatus,
    /// Times this lesson has been re-triggered (deduped by hash).
    pub recurrence: u32,
    pub created_at: String,
    pub last_seen_at: String,
    #[serde(default)]
    pub promoted_at: Option<String>,
}

#[derive(Deserialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/lesson.ts")]
pub struct CreateLessonRequest {
    pub trigger: String,
    pub anti_pattern: String,
    pub preferred_pattern: String,
    pub priority: LessonPriority,
    pub scope: LessonScope,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../../agent/src/generated/lesson.ts")]
pub struct PromoteResponse {
    pub promoted: bool,
    pub id: String,
    pub written_to: Option<String>,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../../agent/src/generated/lesson.ts")]
pub struct DeleteLessonResponse {
    pub deleted: bool,
    pub id: String,
}
