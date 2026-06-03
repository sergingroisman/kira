#![forbid(unsafe_code)]

//! HTTP client for kira-cored (/v1/* endpoints).
//!
//! `CoreClient` is Clone-able (cheap Arc internally) and uses per-request
//! timeout overrides: `health_timeout` for health checks, `request_timeout`
//! for all other operational requests.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::config::Config;

// ── Client ────────────────────────────────────────────────────────────────────

/// HTTP client for kira-cored. Clone-able (cheap Arc internally).
#[derive(Clone)]
pub struct CoreClient {
    http: reqwest::Client,
    base_url: Arc<String>,
    health_timeout: Duration,
    request_timeout: Duration,
}

// ── Shared types ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct HealthResponse {
    pub version: String,
    pub status: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Lesson {
    pub id: String,
    pub level: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AgentStatus {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub iterations: u32,
    pub cost_usd: f64,
    pub model: String,
}

#[derive(Debug, Deserialize)]
pub struct ChatAccepted {
    /// Agent UUID (empty string when fast-path conceptual response).
    #[serde(default)]
    pub agent_id: String,
    /// Execution kind: "chat", "agent", etc.
    #[serde(default)]
    pub kind: String,
    /// Status: "completed" for fast-path, "running" for agent spawn.
    #[serde(default)]
    pub status: String,
    /// Direct response content (present only on degraded inline fallback).
    #[serde(default)]
    pub message: Option<String>,
    /// Routing taken by the Core: "lite" | "full" | "fallback".
    #[serde(default)]
    pub route: String,
}

/// Mirror of core `stats_api::StatsResponse` (only the fields the sidebar uses).
#[derive(Debug, Deserialize, Default)]
pub struct StatsResponse {
    #[serde(default)]
    pub usd_spent: f64,
    #[serde(default)]
    pub tokens_used: u64,
    #[serde(default)]
    pub max_budget_usd: f64,
    #[serde(default)]
    pub usage_pct: u8,
    #[serde(default)]
    pub providers: Vec<StatsProvider>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StatsProvider {
    pub name: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct QueueEntry {
    pub id: String,
    pub status: String,
    pub message_preview: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct MemoryFragment {
    pub id: String,
    pub content: String,
    pub similarity: f64,
    pub created_at: String,
}

/// Subset of a workflow run for TUI progress + questions.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct WorkflowRunView {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub current_phase_id: String,
    #[serde(default)]
    pub phases_run: Vec<String>,
    #[serde(default)]
    pub pending_questions: Vec<WfQuestion>,
    #[serde(default)]
    pub finished_at: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    /// True while the run is paused at the plan-approval gate (PLAN mode).
    #[serde(default)]
    pub awaiting_plan_approval: bool,
}

/// Plan-approval matrix (mirrors core `sdd::plan_view::PlanView`).
#[derive(Debug, Deserialize, Clone, Default)]
pub struct PlanMatrixView {
    #[serde(default)]
    pub run_id: String,
    #[serde(default)]
    pub goal: String,
    #[serde(default)]
    pub wave_count: u32,
    #[serde(default)]
    pub tasks: Vec<PlanMatrixTask>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct PlanMatrixTask {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub owned_files: Vec<String>,
    #[serde(default)]
    pub wave: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WfQuestion {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub question: String,
}

// ── Internal request/response types ───────────────────────────────────────────

#[derive(Debug, Serialize)]
struct ChatSubmitRequest<'a> {
    messages: Vec<ChatMessagePayload<'a>>,
    tier: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<&'a str>,
}

#[derive(Debug, Serialize)]
struct ChatMessagePayload<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Deserialize)]
struct ChatYoloResponse {
    agent_id: String,
}

#[derive(Debug, Serialize)]
struct AgentSpawnRequest<'a> {
    kind: &'a str,
    task: &'a str,
    tier: &'a str,
}

#[derive(Debug, Deserialize)]
struct AgentSpawnResponse {
    id: String,
}

#[derive(Debug, Serialize)]
struct ApprovalPayload<'a> {
    approved: bool,
    feedback: Option<&'a str>,
}

// ── CLI-local wire types (slim mirrors of core proto) ─────────────────────────

#[derive(Debug, Deserialize, Serialize)]
pub struct WorkflowRun {
    pub id: String,
    pub goal: String,
    pub workflow_id: String,
    pub current_phase_id: String,
    pub usd_spent: f64,
    pub created_at: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub finished_at: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DetectiveResult {
    pub run_id: String,
    pub verdict: String, // "pass" | "warn" | "block"
    pub verdict_reason: String,
    pub summary: DetectiveSummary,
    pub findings: Vec<DetectiveFinding>,
    pub highlights: Vec<DetectiveHighlight>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DetectiveSummary {
    pub total_findings: usize,
    pub by_severity: std::collections::HashMap<String, usize>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DetectiveFinding {
    pub severity: String,
    pub lenses: Vec<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub line: Option<u32>,
    pub title: String,
    pub description: String,
    pub recommendation: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DetectiveHighlight {
    pub lens: String,
    pub text: String,
}

// ── Path constants ────────────────────────────────────────────────────────────

const PATH_HEALTH: &str = "/v1/health";
const PATH_CHAT: &str = "/v1/chat";
const PATH_CHAT_YOLO: &str = "/v1/chat/yolo";
const PATH_AGENTS_SPAWN: &str = "/v1/agents/spawn";
const PATH_QUEUE: &str = "/v1/queue";
const PATH_LESSONS: &str = "/v1/lessons";
const PATH_MEMORY_SEARCH: &str = "/v1/memory/search";
const PATH_WORKFLOW: &str = "/v1/workflow";
const PATH_WORKFLOW_LIST: &str = "/v1/workflow/list";
const PATH_DETECTIVE_RUN: &str = "/v1/detective/run";
const PATH_STATS: &str = "/v1/stats";

// ── Implementation ────────────────────────────────────────────────────────────

impl CoreClient {
    /// Construct a new `CoreClient` from the loaded configuration.
    ///
    /// Validates that `core_url` is a valid HTTP or HTTPS URL.
    /// Returns `Err` if the URL is invalid or the HTTP client cannot be built.
    pub fn new(cfg: &Config) -> Result<Self> {
        let raw_url = cfg.core_url.trim_end_matches('/').to_string();

        // Validate URL: must be valid and use http or https scheme.
        let parsed = reqwest::Url::parse(&raw_url)
            .with_context(|| format!("invalid core_url: {raw_url}"))?;

        match parsed.scheme() {
            "http" | "https" => {}
            scheme => bail!("invalid core_url scheme '{scheme}': must be http or https"),
        }

        let http = reqwest::Client::builder()
            .build()
            .context("building HTTP client")?;

        let health_timeout = Duration::from_secs(cfg.time.health_timeout_secs);
        let request_timeout = Duration::from_secs(cfg.time.request_timeout_secs);

        Ok(Self {
            http,
            base_url: Arc::new(raw_url),
            health_timeout,
            request_timeout,
        })
    }

    /// Returns the base URL (e.g. `http://127.0.0.1:7777`).
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    // ── Health ────────────────────────────────────────────────────────────────

    /// GET /v1/health (uses health_timeout)
    pub async fn health(&self) -> Result<HealthResponse> {
        let resp = self
            .http
            .get(self.url(PATH_HEALTH))
            .timeout(self.health_timeout)
            .send()
            .await
            .context("GET /v1/health")?;

        if !resp.status().is_success() {
            bail!("health check failed: {}", resp.status());
        }

        resp.json::<HealthResponse>()
            .await
            .context("parsing health response")
    }

    /// GET /v1/stats — budget + provider snapshot for the sidebar.
    pub async fn stats(&self) -> Result<StatsResponse> {
        let resp = self
            .http
            .get(self.url(PATH_STATS))
            .timeout(self.request_timeout)
            .send()
            .await
            .context("GET /v1/stats")?;

        if !resp.status().is_success() {
            bail!("stats failed: {}", resp.status());
        }

        resp.json::<StatsResponse>()
            .await
            .context("parsing stats response")
    }

    // ── Chat ──────────────────────────────────────────────────────────────────

    /// POST /v1/chat/yolo — submit a message, returns agent_id.
    pub async fn chat_submit(
        &self,
        history: &[(String, String)],
        tier: &str,
    ) -> Result<ChatAccepted> {
        let messages: Vec<ChatMessagePayload> = history
            .iter()
            .map(|(role, content)| ChatMessagePayload {
                role: role.as_str(),
                content: content.as_str(),
            })
            .collect();
        let payload = ChatSubmitRequest {
            messages,
            tier,
            mode: Some("yolo"),
        };
        let resp = self
            .http
            .post(self.url(PATH_CHAT_YOLO))
            .json(&payload)
            .timeout(self.request_timeout)
            .send()
            .await
            .context("POST /v1/chat/yolo")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("chat submit failed {status}: {body}");
        }

        resp.json::<ChatAccepted>()
            .await
            .context("parsing chat accepted response")
    }

    /// POST /v1/chat/yolo — send message, poll until done, return output.
    pub async fn chat_yolo(&self, message: &str, tier: &str) -> Result<String> {
        let payload = ChatSubmitRequest {
            messages: vec![ChatMessagePayload {
                role: "user",
                content: message,
            }],
            tier,
            mode: Some("yolo"),
        };
        let resp = self
            .http
            .post(self.url(PATH_CHAT_YOLO))
            .json(&payload)
            .timeout(self.request_timeout)
            .send()
            .await
            .context("POST /v1/chat/yolo")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("chat/yolo failed {status}: {body}");
        }

        let ChatYoloResponse { agent_id } = resp.json().await.context("parsing chat response")?;

        // Poll until terminal status
        let deadline = std::time::Instant::now() + Duration::from_mins(5);
        loop {
            if std::time::Instant::now() > deadline {
                bail!("timeout waiting for agent {agent_id}");
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
            let status = self.agent_status_raw(&agent_id).await?;
            match status.status.as_str() {
                "completed" => return Ok(format!("{agent_id}: done")),
                "failed" | "cancelled" | "stopped" => {
                    bail!("agent {agent_id} ended with status: {}", status.status)
                }
                _ => {}
            }
        }
    }

    // ── Agents ────────────────────────────────────────────────────────────────

    /// POST /v1/agents/spawn
    pub async fn agent_spawn(&self, kind: &str, task: &str, tier: &str) -> Result<String> {
        let resp = self
            .http
            .post(self.url(PATH_AGENTS_SPAWN))
            .json(&AgentSpawnRequest { kind, task, tier })
            .timeout(self.request_timeout)
            .send()
            .await
            .context("POST /v1/agents/spawn")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("agent spawn failed {status}: {body}");
        }

        let AgentSpawnResponse { id } = resp.json().await.context("parsing spawn response")?;
        Ok(id)
    }

    /// GET /v1/agents/{id} — returns pretty-printed JSON string.
    pub async fn agent_status(&self, id: &str) -> Result<String> {
        let status = self.agent_status_raw(id).await?;
        serde_json::to_string_pretty(&status).context("serializing agent status")
    }

    async fn agent_status_raw(&self, id: &str) -> Result<AgentStatus> {
        let resp = self
            .http
            .get(self.url(&format!("/v1/agents/{id}")))
            .timeout(self.request_timeout)
            .send()
            .await
            .context("GET /v1/agents/<id>")?;

        if !resp.status().is_success() {
            bail!("agent status failed: {}", resp.status());
        }

        resp.json::<AgentStatus>()
            .await
            .context("parsing agent status")
    }

    /// POST /v1/agents/{id}/cancel
    pub async fn agent_cancel(&self, id: &str) -> Result<()> {
        let resp = self
            .http
            .post(self.url(&format!("/v1/agents/{id}/cancel")))
            .timeout(self.request_timeout)
            .send()
            .await
            .context("POST /v1/agents/<id>/cancel")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("agent cancel failed {status}: {body}");
        }

        Ok(())
    }

    /// POST /v1/agents/{id}/approval
    pub async fn agent_approve(
        &self,
        id: &str,
        approved: bool,
        feedback: Option<&str>,
    ) -> Result<()> {
        let resp = self
            .http
            .post(self.url(&format!("/v1/agents/{id}/approval")))
            .json(&ApprovalPayload { approved, feedback })
            .timeout(self.request_timeout)
            .send()
            .await
            .context("POST /v1/agents/<id>/approval")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("agent approval failed {status}: {body}");
        }

        Ok(())
    }

    // ── Queue ─────────────────────────────────────────────────────────────────

    /// GET /v1/queue
    pub async fn queue_list(&self) -> Result<Vec<QueueEntry>> {
        let resp = self
            .http
            .get(self.url(PATH_QUEUE))
            .timeout(self.request_timeout)
            .send()
            .await
            .context("GET /v1/queue")?;

        if !resp.status().is_success() {
            bail!("queue list failed: {}", resp.status());
        }

        resp.json::<Vec<QueueEntry>>()
            .await
            .context("parsing queue list")
    }

    /// DELETE /v1/queue/{id}
    pub async fn queue_cancel(&self, id: &str) -> Result<()> {
        let resp = self
            .http
            .delete(self.url(&format!("/v1/queue/{id}")))
            .timeout(self.request_timeout)
            .send()
            .await
            .context("DELETE /v1/queue/<id>")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("queue cancel failed {status}: {body}");
        }

        Ok(())
    }

    // ── Memory ────────────────────────────────────────────────────────────────

    /// GET /v1/memory/search?q=<query>
    pub async fn memory_search(&self, query: &str) -> Result<Vec<MemoryFragment>> {
        let resp = self
            .http
            .get(self.url(PATH_MEMORY_SEARCH))
            .query(&[("q", query)])
            .timeout(self.request_timeout)
            .send()
            .await
            .context("GET /v1/memory/search")?;

        if !resp.status().is_success() {
            bail!("memory search failed: {}", resp.status());
        }

        resp.json::<Vec<MemoryFragment>>()
            .await
            .context("parsing memory search results")
    }

    /// GET /v1/memory/{run_id}/context
    pub async fn memory_context(&self, run_id: &str) -> Result<Vec<MemoryFragment>> {
        let resp = self
            .http
            .get(self.url(&format!("/v1/memory/{run_id}/context")))
            .timeout(self.request_timeout)
            .send()
            .await
            .context("GET /v1/memory/<run_id>/context")?;

        if !resp.status().is_success() {
            bail!("memory context failed: {}", resp.status());
        }

        resp.json::<Vec<MemoryFragment>>()
            .await
            .context("parsing memory context")
    }

    // ── Workflow ──────────────────────────────────────────────────────────────

    /// POST /v1/workflow — start a workflow run.
    /// `require_plan_approval` gates EXECUTE behind an operator y/n (PLAN mode).
    pub async fn workflow_start(
        &self,
        goal: &str,
        workflow: &str,
        kind: Option<&str>,
        require_plan_approval: bool,
    ) -> Result<WorkflowRun> {
        let body = serde_json::json!({
            "goal": goal,
            "workflow": workflow,
            "kind": kind,
            "require_plan_approval": require_plan_approval,
        });
        let resp = self
            .http
            .post(self.url(PATH_WORKFLOW))
            .json(&body)
            .timeout(self.request_timeout)
            .send()
            .await
            .context("POST /v1/workflow")?;
        if !resp.status().is_success() {
            bail!("workflow start failed: {}", resp.status());
        }
        resp.json::<WorkflowRun>().await.context("parsing workflow run")
    }

    /// GET /v1/workflow/{id} — current run status (phase, questions, done).
    pub async fn workflow_get(&self, id: &str) -> Result<WorkflowRunView> {
        let resp = self
            .http
            .get(self.url(&format!("/v1/workflow/{id}")))
            .timeout(self.request_timeout)
            .send()
            .await
            .context("GET /v1/workflow/{id}")?;
        if !resp.status().is_success() {
            bail!("workflow get failed: {}", resp.status());
        }
        resp.json::<WorkflowRunView>().await.context("parsing workflow run view")
    }

    /// POST /v1/workflow/{id}/questions — answer clarification questions.
    pub async fn workflow_answer(
        &self,
        id: &str,
        answers: std::collections::HashMap<String, Vec<String>>,
    ) -> Result<()> {
        let body = serde_json::json!({ "answers": answers });
        let resp = self
            .http
            .post(self.url(&format!("/v1/workflow/{id}/questions")))
            .json(&body)
            .timeout(self.request_timeout)
            .send()
            .await
            .context("POST /v1/workflow/{id}/questions")?;
        if !resp.status().is_success() {
            bail!("workflow answer failed: {}", resp.status());
        }
        Ok(())
    }

    /// GET /v1/workflow/{id}/plan — fetch the plan-approval matrix.
    pub async fn workflow_plan(&self, id: &str) -> Result<PlanMatrixView> {
        let resp = self
            .http
            .get(self.url(&format!("/v1/workflow/{id}/plan")))
            .timeout(self.request_timeout)
            .send()
            .await
            .context("GET /v1/workflow/{id}/plan")?;
        if !resp.status().is_success() {
            bail!("workflow plan failed: {}", resp.status());
        }
        resp.json::<PlanMatrixView>().await.context("parsing plan matrix")
    }

    /// POST /v1/workflow/{id}/approve_plan — resolve the plan-approval gate.
    pub async fn workflow_approve_plan(&self, id: &str, approved: bool) -> Result<()> {
        let body = serde_json::json!({ "approved": approved });
        let resp = self
            .http
            .post(self.url(&format!("/v1/workflow/{id}/approve_plan")))
            .json(&body)
            .timeout(self.request_timeout)
            .send()
            .await
            .context("POST /v1/workflow/{id}/approve_plan")?;
        if !resp.status().is_success() {
            bail!("workflow approve_plan failed: {}", resp.status());
        }
        Ok(())
    }

    /// GET /v1/workflow — list runs.
    pub async fn workflow_list(&self) -> Result<Vec<WorkflowRun>> {
        #[derive(Deserialize)]
        struct ListResp { runs: Vec<WorkflowRun> }
        let resp = self
            .http
            .get(self.url(PATH_WORKFLOW))
            .timeout(self.request_timeout)
            .send()
            .await
            .context("GET /v1/workflow")?;
        if !resp.status().is_success() {
            bail!("workflow list failed: {}", resp.status());
        }
        Ok(resp.json::<ListResp>().await.context("parsing workflow list")?.runs)
    }

    /// GET /v1/workflow/list — available workflow ids.
    pub async fn workflow_ids(&self) -> Result<Vec<String>> {
        #[derive(Deserialize)]
        struct Resp { workflows: Vec<String> }
        let resp = self
            .http
            .get(self.url(PATH_WORKFLOW_LIST))
            .timeout(self.request_timeout)
            .send()
            .await
            .context("GET /v1/workflow/list")?;
        if !resp.status().is_success() {
            bail!("workflow ids failed: {}", resp.status());
        }
        Ok(resp.json::<Resp>().await.context("parsing workflow ids")?.workflows)
    }

    // ── Detective ─────────────────────────────────────────────────────────────

    /// POST /v1/detective/run — run code review.
    /// Returns (verdict, summary_text).
    pub async fn detective_run(
        &self,
        diff: Option<&str>,
        worktree_id: Option<&str>,
        spec_path: Option<&str>,
        tasks_path: Option<&str>,
    ) -> Result<DetectiveResult> {
        let body = serde_json::json!({
            "diff": diff,
            "worktree_id": worktree_id,
            "spec_path": spec_path,
            "tasks_path": tasks_path,
        });
        let resp = self
            .http
            .post(self.url(PATH_DETECTIVE_RUN))
            .json(&body)
            .timeout(Duration::from_mins(5)) // detective is slow
            .send()
            .await
            .context("POST /v1/detective/run")?;
        if !resp.status().is_success() {
            bail!("detective run failed: {}", resp.status());
        }
        resp.json::<DetectiveResult>().await.context("parsing detective result")
    }

    // ── Lessons ───────────────────────────────────────────────────────────────

    /// GET /v1/lessons
    pub async fn lessons_list(&self) -> Result<Vec<Lesson>> {
        let resp = self
            .http
            .get(self.url(PATH_LESSONS))
            .timeout(self.request_timeout)
            .send()
            .await
            .context("GET /v1/lessons")?;

        if !resp.status().is_success() {
            bail!("lessons list failed: {}", resp.status());
        }

        resp.json::<Vec<Lesson>>().await.context("parsing lessons")
    }
}
