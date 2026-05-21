/**
 * KiraApp — Ink REPL talking to the Rust core daemon.
 *
 * v0 capabilities (Sem 6 + 10):
 *   - User types a prompt → POST /v1/chat/stream → tokens stream in
 *   - /agent <kind> <task>  → spawn agent + live-poll updates
 *   - /agents               → list all agents (status + cost)
 *   - /cancel <id-prefix>   → cancel a running agent
 *   - /health, /tier <h|s|l>, /quit
 */

import React, { useState, useEffect, useCallback, useRef } from "react";
import { Box, Text, useApp, useInput } from "ink";
import {
  CoreClient,
  CoreError,
  type Agent,
  type AgentKind,
  type ChatMessage,
  type Plan,
} from "../core-client/client.js";
import { LogoKira } from "./components/index.js";

const TIERS = ["heavy", "standard", "light"] as const;
type Tier = (typeof TIERS)[number];

const AGENT_KINDS: ReadonlyArray<AgentKind> = ["writer", "deployer", "judge"];

// -- Turn modelling -----------------------------------------------------------

type TurnBase = { user: string; inFlight: boolean; error?: string };

interface ChatTurn extends TurnBase {
  kind: "chat";
  assistant: string;
  model?: string;
}

interface AgentTurn extends TurnBase {
  kind: "agent";
  agentId?: string;
  agentKind: AgentKind;
  status?: string;
  iters?: number;
  usd?: number;
  model?: string;
  result?: string;
}

interface InfoTurn extends TurnBase {
  kind: "info";
  body: string;
}

interface PlanTurn extends TurnBase {
  kind: "plan";
  planId?: string;
  planStatus?: string;
  goal?: string;
  agentKind?: AgentKind;
  scope?: string;
  steps?: { idx: number; description: string; tool?: string | null }[];
  executorAgentId?: string;
  note?: string; // "drafted", "approved", "cancelled"
}

type Turn = ChatTurn | AgentTurn | InfoTurn | PlanTurn;

export interface KiraAppProps {
  client?: CoreClient;
  initialTier?: Tier;
}

export function KiraApp({
  client = new CoreClient(),
  initialTier = "standard",
}: KiraAppProps) {
  const { exit } = useApp();
  const [tier, setTier] = useState<Tier>(initialTier);
  const [input, setInput] = useState("");
  const [turns, setTurns] = useState<Turn[]>([]);
  const [coreUp, setCoreUp] = useState<boolean | null>(null);
  const [coreVersion, setCoreVersion] = useState<string | null>(null);

  // Active agent pollers — keyed by agent id, cleared on completion.
  const agentPollers = useRef<Map<string, ReturnType<typeof setInterval>>>(
    new Map(),
  );

  // Active plan pollers — keyed by plan id, cleared on terminal status.
  const planPollers = useRef<Map<string, ReturnType<typeof setInterval>>>(
    new Map(),
  );

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const h = await client.health();
        if (!cancelled) {
          setCoreUp(true);
          setCoreVersion(h.version);
        }
      } catch {
        if (!cancelled) setCoreUp(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [client]);

  // Clean up pollers on unmount
  useEffect(
    () => () => {
      for (const t of agentPollers.current.values()) clearInterval(t);
      agentPollers.current.clear();
      for (const t of planPollers.current.values()) clearInterval(t);
      planPollers.current.clear();
    },
    [],
  );

  // -- Helpers ---------------------------------------------------------------

  const pushInfo = useCallback((user: string, body: string, error?: string) => {
    setTurns((t) => [
      ...t,
      { kind: "info", user, body, inFlight: false, error },
    ]);
  }, []);

  const startAgentPoller = useCallback(
    (turnIdx: number, agent: Agent) => {
      const id = agent.id;
      const interval = setInterval(async () => {
        try {
          const fresh = await client.getAgent(id);
          setTurns((t) => {
            const next = [...t];
            const cur = next[turnIdx];
            if (cur && cur.kind === "agent") {
              next[turnIdx] = {
                ...cur,
                status: fresh.status,
                iters: fresh.iters,
                usd: fresh.usd_spent,
                model: fresh.model ?? cur.model,
                result: fresh.result ?? cur.result,
                error: fresh.error ?? cur.error,
                inFlight:
                  fresh.status !== "completed" &&
                  fresh.status !== "failed" &&
                  fresh.status !== "cancelled" &&
                  fresh.status !== "stopped",
              };
            }
            return next;
          });
          if (
            fresh.status === "completed" ||
            fresh.status === "failed" ||
            fresh.status === "cancelled" ||
            fresh.status === "stopped"
          ) {
            const t = agentPollers.current.get(id);
            if (t) {
              clearInterval(t);
              agentPollers.current.delete(id);
            }
          }
        } catch (e) {
          setTurns((t) => {
            const next = [...t];
            const cur = next[turnIdx];
            if (cur && cur.kind === "agent") {
              next[turnIdx] = { ...cur, error: String(e), inFlight: false };
            }
            return next;
          });
          const t = agentPollers.current.get(id);
          if (t) {
            clearInterval(t);
            agentPollers.current.delete(id);
          }
        }
      }, 2000);
      agentPollers.current.set(id, interval);
    },
    [client],
  );

  const startPlanPoller = useCallback(
    (turnIdx: number, plan: Plan) => {
      const id = plan.id;
      const interval = setInterval(async () => {
        try {
          const fresh = await client.getPlan(id);
          setTurns((t) => {
            const next = [...t];
            const cur = next[turnIdx];
            if (cur && cur.kind === "plan") {
              next[turnIdx] = {
                ...cur,
                planStatus: fresh.status,
                executorAgentId: fresh.executor_agent_id ?? cur.executorAgentId,
                inFlight: !["done", "failed", "cancelled", "expired"].includes(
                  fresh.status,
                ),
              };
            }
            return next;
          });
          if (["done", "failed", "cancelled", "expired"].includes(fresh.status)) {
            const t = planPollers.current.get(id);
            if (t) {
              clearInterval(t);
              planPollers.current.delete(id);
            }
          }
        } catch (e) {
          const t = planPollers.current.get(id);
          if (t) {
            clearInterval(t);
            planPollers.current.delete(id);
          }
          setTurns((t) => {
            const next = [...t];
            const cur = next[turnIdx];
            if (cur && cur.kind === "plan") {
              next[turnIdx] = { ...cur, error: String(e), inFlight: false };
            }
            return next;
          });
        }
      }, 2000);
      planPollers.current.set(id, interval);
    },
    [client],
  );

  // -- Slash command dispatch ------------------------------------------------

  const handleAgentCmd = useCallback(
    async (raw: string) => {
      // /agent <kind> <task...>
      const rest = raw.slice("/agent".length).trim();
      const space = rest.indexOf(" ");
      if (space < 0) {
        pushInfo(raw, "", `usage: /agent <writer|deployer|judge> <task>`);
        return;
      }
      const kindStr = rest.slice(0, space).toLowerCase() as AgentKind;
      const task = rest.slice(space + 1).trim();
      if (!AGENT_KINDS.includes(kindStr)) {
        pushInfo(
          raw,
          "",
          `unknown kind '${kindStr}'. Valid: ${AGENT_KINDS.join(", ")}`,
        );
        return;
      }
      if (!task) {
        pushInfo(raw, "", `task is empty`);
        return;
      }
      const turnIdx = turns.length;
      setTurns((t) => [
        ...t,
        {
          kind: "agent",
          user: raw,
          agentKind: kindStr,
          inFlight: true,
        },
      ]);
      try {
        const agent = await client.spawnAgent({
          kind: kindStr,
          task,
          worktree_id: null,
          base: null,
          budget_usd: null,
          max_iters: null,
        });
        setTurns((t) => {
          const next = [...t];
          const cur = next[turnIdx];
          if (cur && cur.kind === "agent") {
            next[turnIdx] = {
              ...cur,
              agentId: agent.id,
              status: agent.status,
              iters: agent.iters,
              usd: agent.usd_spent,
              model: agent.model ?? undefined,
            };
          }
          return next;
        });
        startAgentPoller(turnIdx, agent);
      } catch (e) {
        const msg = e instanceof CoreError ? e.message : String(e);
        setTurns((t) => {
          const next = [...t];
          const cur = next[turnIdx];
          if (cur && cur.kind === "agent") {
            next[turnIdx] = { ...cur, error: msg, inFlight: false };
          }
          return next;
        });
      }
    },
    [client, turns.length, pushInfo, startAgentPoller],
  );

  const handleAgentsList = useCallback(
    async (raw: string) => {
      try {
        const all = await client.listAgents();
        if (all.length === 0) {
          pushInfo(raw, "(no agents)");
          return;
        }
        const body = all
          .map(
            (a) =>
              `${a.id.slice(0, 8)} ${a.kind.padEnd(8)} ${a.status.padEnd(9)} iters=${a.iters} $${a.usd_spent.toFixed(4)}`,
          )
          .join("\n");
        pushInfo(raw, body);
      } catch (e) {
        pushInfo(raw, "", String(e));
      }
    },
    [client, pushInfo],
  );

  const handleCancel = useCallback(
    async (raw: string) => {
      const prefix = raw.slice("/cancel".length).trim();
      if (!prefix) {
        pushInfo(raw, "", "usage: /cancel <id-prefix>");
        return;
      }
      try {
        const all = await client.listAgents();
        const match = all.find((a) => a.id.startsWith(prefix));
        if (!match) {
          pushInfo(raw, "", `no agent with prefix '${prefix}'`);
          return;
        }
        const r = await client.cancelAgent(match.id);
        pushInfo(raw, `cancelled=${r.cancelled} id=${match.id.slice(0, 8)}`);
      } catch (e) {
        pushInfo(raw, "", String(e));
      }
    },
    [client, pushInfo],
  );

  // /plan draft <kind> <goal>
  const handlePlanDraft = useCallback(
    async (raw: string) => {
      const rest = raw.slice("/plan draft".length).trim();
      const space = rest.indexOf(" ");
      if (space < 0) {
        pushInfo(raw, "", `usage: /plan draft <writer|deployer|judge> <goal>`);
        return;
      }
      const kindStr = rest.slice(0, space).toLowerCase() as AgentKind;
      const goal = rest.slice(space + 1).trim();
      if (!AGENT_KINDS.includes(kindStr)) {
        pushInfo(raw, "", `unknown kind '${kindStr}'`);
        return;
      }
      if (!goal) {
        pushInfo(raw, "", `goal is empty`);
        return;
      }
      const turnIdx = turns.length;
      setTurns((t) => [
        ...t,
        {
          kind: "plan",
          user: raw,
          agentKind: kindStr,
          goal,
          note: "drafting…",
          inFlight: true,
        },
      ]);
      try {
        const plan = await client.draftPlan({
          goal,
          agent_kind: kindStr,
          ttl_secs: null,
        });
        setTurns((t) => {
          const next = [...t];
          const cur = next[turnIdx];
          if (cur && cur.kind === "plan") {
            next[turnIdx] = {
              ...cur,
              planId: plan.id,
              planStatus: plan.status,
              scope: plan.scope,
              steps: plan.steps.map((s) => ({
                idx: s.idx,
                description: s.description,
                tool: s.tool,
              })),
              note: `drafted — /plan approve ${plan.id.slice(0, 8)}`,
              inFlight: false,
            };
          }
          return next;
        });
      } catch (e) {
        const msg = e instanceof CoreError ? e.message : String(e);
        setTurns((t) => {
          const next = [...t];
          const cur = next[turnIdx];
          if (cur && cur.kind === "plan") {
            next[turnIdx] = { ...cur, error: msg, inFlight: false };
          }
          return next;
        });
      }
    },
    [client, turns.length, pushInfo],
  );

  // /plan approve <id-prefix>
  const handlePlanApprove = useCallback(
    async (raw: string) => {
      const prefix = raw.slice("/plan approve".length).trim();
      if (!prefix) {
        pushInfo(raw, "", "usage: /plan approve <id-prefix>");
        return;
      }
      try {
        const all = await client.listPlans();
        const match = all.find((p) => p.id.startsWith(prefix));
        if (!match) {
          pushInfo(raw, "", `no plan with prefix '${prefix}'`);
          return;
        }
        const turnIdx = turns.length;
        setTurns((t) => [
          ...t,
          {
            kind: "plan",
            user: raw,
            planId: match.id,
            planStatus: "approving…",
            agentKind: match.agent_kind,
            scope: match.scope,
            goal: match.goal,
            note: "approving…",
            inFlight: true,
          },
        ]);
        const r = await client.approvePlan(match.id);
        setTurns((t) => {
          const next = [...t];
          const cur = next[turnIdx];
          if (cur && cur.kind === "plan") {
            next[turnIdx] = {
              ...cur,
              planStatus: "executing",
              executorAgentId: r.executor_agent_id,
              note: `executor: ${r.executor_agent_id.slice(0, 8)}`,
              inFlight: true,
            };
          }
          return next;
        });
        const planFresh = await client.getPlan(match.id);
        startPlanPoller(turnIdx, planFresh);
      } catch (e) {
        pushInfo(raw, "", String(e));
      }
    },
    [client, turns.length, pushInfo, startPlanPoller],
  );

  // /plan cancel <id-prefix>
  const handlePlanCancel = useCallback(
    async (raw: string) => {
      const prefix = raw.slice("/plan cancel".length).trim();
      if (!prefix) {
        pushInfo(raw, "", "usage: /plan cancel <id-prefix>");
        return;
      }
      try {
        const all = await client.listPlans();
        const match = all.find((p) => p.id.startsWith(prefix));
        if (!match) {
          pushInfo(raw, "", `no plan with prefix '${prefix}'`);
          return;
        }
        const r = await client.cancelPlan(match.id);
        pushInfo(raw, `cancelled=${r.cancelled} plan=${match.id.slice(0, 8)}`);
      } catch (e) {
        pushInfo(raw, "", String(e));
      }
    },
    [client, pushInfo],
  );

  // /plans
  const handlePlansList = useCallback(
    async (raw: string) => {
      try {
        const all = await client.listPlans();
        if (all.length === 0) {
          pushInfo(raw, "(no plans)");
          return;
        }
        const body = all
          .map(
            (p) =>
              `${p.id.slice(0, 8)} ${p.agent_kind.padEnd(8)} ${p.status.padEnd(9)} ${p.scope.padEnd(7)} steps=${p.steps.length} ${p.goal.slice(0, 34)}`,
          )
          .join("\n");
        pushInfo(raw, body);
      } catch (e) {
        pushInfo(raw, "", String(e));
      }
    },
    [client, pushInfo],
  );

  // /lessons (list)
  const handleLessonsList = useCallback(
    async (raw: string) => {
      try {
        const all = await client.listLessons();
        if (all.length === 0) {
          pushInfo(raw, "(no lessons)");
          return;
        }
        const body = all
          .map((l) => {
            const scope =
              l.scope.kind === "global"
                ? "global"
                : l.scope.kind === "agent_kind"
                  ? `kind=${l.scope.agent_kind}`
                  : `wf=${l.scope.workflow}`;
            return `${l.id.slice(0, 8)} ${l.status.padEnd(8)} ${l.priority.padEnd(6)} x${l.recurrence} ${scope.padEnd(14)} ${l.anti_pattern.slice(0, 60)}`;
          })
          .join("\n");
        pushInfo(raw, body);
      } catch (e) {
        pushInfo(raw, "", String(e));
      }
    },
    [client, pushInfo],
  );

  // /lessons promote <id-prefix>
  const handleLessonPromote = useCallback(
    async (raw: string) => {
      const prefix = raw.slice("/lessons promote".length).trim();
      if (!prefix) {
        pushInfo(raw, "", "usage: /lessons promote <id-prefix>");
        return;
      }
      try {
        const all = await client.listLessons();
        const match = all.find((l) => l.id.startsWith(prefix));
        if (!match) {
          pushInfo(raw, "", `no lesson with prefix '${prefix}'`);
          return;
        }
        const r = await client.promoteLesson(match.id);
        pushInfo(
          raw,
          r.promoted
            ? `promoted ${match.id.slice(0, 8)} → ${r.written_to}`
            : `already promoted ${match.id.slice(0, 8)}`,
        );
      } catch (e) {
        pushInfo(raw, "", String(e));
      }
    },
    [client, pushInfo],
  );

  // /lessons delete <id-prefix>
  const handleLessonDelete = useCallback(
    async (raw: string) => {
      const prefix = raw.slice("/lessons delete".length).trim();
      if (!prefix) {
        pushInfo(raw, "", "usage: /lessons delete <id-prefix>");
        return;
      }
      try {
        const all = await client.listLessons();
        const match = all.find((l) => l.id.startsWith(prefix));
        if (!match) {
          pushInfo(raw, "", `no lesson with prefix '${prefix}'`);
          return;
        }
        const r = await client.deleteLesson(match.id);
        pushInfo(raw, `deleted=${r.deleted} ${match.id.slice(0, 8)}`);
      } catch (e) {
        pushInfo(raw, "", String(e));
      }
    },
    [client, pushInfo],
  );

  // /eval run <models-csv> | <task-spec>
  const handleEvalRun = useCallback(
    async (raw: string) => {
      const rest = raw.slice("/eval run".length).trim();
      const bar = rest.indexOf("|");
      if (bar < 0) {
        pushInfo(
          raw,
          "",
          "usage: /eval run <model1,model2,...> | <task-spec>",
        );
        return;
      }
      const modelsCsv = rest.slice(0, bar).trim();
      const taskSpec = rest.slice(bar + 1).trim();
      if (!modelsCsv || !taskSpec) {
        pushInfo(raw, "", "models or task is empty");
        return;
      }
      const models = modelsCsv
        .split(",")
        .map((m) => m.trim())
        .filter(Boolean);
      if (models.length === 0) {
        pushInfo(raw, "", "no valid models");
        return;
      }

      const turnIdx = turns.length;
      setTurns((t) => [
        ...t,
        {
          kind: "info",
          user: raw,
          body: `running eval: 1 task × ${models.length} models (judge enabled, may take 30-60s)…`,
          inFlight: true,
        },
      ]);

      try {
        const r = await client.runEval({
          tasks: [{ id: "ad_hoc", spec: taskSpec, expected_topic: null, models: null }],
          models,
          skip_embedding: false,
          skip_judge: false,
        });
        const lines = [
          `${r.n_tasks} task × ${r.n_models} models · total $${r.total_cost_usd.toFixed(5)} · ${r.total_duration_ms}ms`,
          "",
          "model               score  faith   cost      ms     pass",
          "─".repeat(58),
        ];
        for (const m of r.summary_by_model) {
          const score = m.avg_judge_score?.toFixed(1) ?? "—";
          const faith = m.avg_faithfulness?.toFixed(3) ?? "—";
          const pass = (m.pass_rate * 100).toFixed(0) + "%";
          lines.push(
            `${m.model.padEnd(19)} ${score.padStart(5)}  ${faith.padStart(5)}  $${m.total_cost_usd.toFixed(5)}  ${m.avg_duration_ms.toFixed(0).padStart(5)}  ${pass}`,
          );
        }
        setTurns((t) => {
          const next = [...t];
          const cur = next[turnIdx];
          if (cur && cur.kind === "info") {
            next[turnIdx] = { ...cur, body: lines.join("\n"), inFlight: false };
          }
          return next;
        });
      } catch (e) {
        setTurns((t) => {
          const next = [...t];
          const cur = next[turnIdx];
          if (cur && cur.kind === "info") {
            next[turnIdx] = { ...cur, error: String(e), inFlight: false };
          }
          return next;
        });
      }
    },
    [client, turns.length, pushInfo],
  );

  // /gates run <spec> | <output>
  const handleGatesRun = useCallback(
    async (raw: string) => {
      const rest = raw.slice("/gates run".length).trim();
      const bar = rest.indexOf("|");
      if (bar < 0) {
        pushInfo(raw, "", "usage: /gates run <spec> | <output>");
        return;
      }
      const spec = rest.slice(0, bar).trim();
      const output = rest.slice(bar + 1).trim();
      if (!spec || !output) {
        pushInfo(raw, "", "spec or output is empty");
        return;
      }
      try {
        const r = await client.runGates({
          spec,
          output,
          owned_paths: [],
          touched_paths: [],
          estimated_loc: null,
          actual_loc: null,
          skip_embedding: false,
          skip_judge: true, // fast UX path; user can curl for judge
        });
        const lines = [`overall=${r.overall}  failed_at=${r.failed_at ?? "—"}  ${r.duration_ms}ms`];
        for (const g of r.results) {
          const sc = g.score !== null && g.score !== undefined ? ` score=${g.score.toFixed(3)}` : "";
          lines.push(`  ${g.outcome.padEnd(7)} ${g.gate.padEnd(24)}${sc}  ${g.reason ?? ""}`);
        }
        pushInfo(raw, lines.join("\n"));
      } catch (e) {
        pushInfo(raw, "", String(e));
      }
    },
    [client, pushInfo],
  );

  // /review <head> [base]    — TRUST-inspired multi-dim PR review
  const handleReview = useCallback(
    async (raw: string) => {
      const rest = raw.slice("/review".length).trim();
      const parts = rest.split(/\s+/).filter(Boolean);
      if (parts.length === 0) {
        pushInfo(raw, "", "usage: /review <head-branch> [base-branch]");
        return;
      }
      const head = parts[0];
      const base = parts[1] ?? "main";
      const turnIdx = turns.length;
      setTurns((t) => [
        ...t,
        {
          kind: "info",
          user: raw,
          body: `spawning 6 reviewers against ${head}…${base} (~60-180s)…`,
          inFlight: true,
        },
      ]);
      try {
        const r = await client.runReview({
          head,
          base,
          workflow: null,
          timeout_secs: null,
        });
        const lines = [
          `run ${r.run_id.slice(0, 8)} · workflow=${r.workflow} · ${r.duration_ms}ms`,
          `findings (post anti-hallucination): ${r.total_findings}`,
          `report: ${r.review_md_path}`,
          "",
          "dim          status      gate    fail-at        kept  drop",
          "─".repeat(64),
        ];
        for (const o of r.agents) {
          lines.push(
            `${o.dimension.padEnd(12)} ${o.status.padEnd(11)} ${o.gate_overall.padEnd(7)} ${(o.gate_failed_at ?? "—").padEnd(14)} ${String(o.findings_kept).padStart(3)}   ${String(o.findings_dropped).padStart(3)}`,
          );
        }
        setTurns((t) => {
          const next = [...t];
          const cur = next[turnIdx];
          if (cur && cur.kind === "info") {
            next[turnIdx] = { ...cur, body: lines.join("\n"), inFlight: false };
          }
          return next;
        });
      } catch (e) {
        setTurns((t) => {
          const next = [...t];
          const cur = next[turnIdx];
          if (cur && cur.kind === "info") {
            next[turnIdx] = { ...cur, error: String(e), inFlight: false };
          }
          return next;
        });
      }
    },
    [client, turns.length, pushInfo],
  );

  // -- Main send -------------------------------------------------------------

  const send = useCallback(
    async (raw: string) => {
      const text = raw.trim();
      if (!text) return;

      if (text === "/quit" || text === "/exit") {
        exit();
        return;
      }
      if (text === "/health") {
        try {
          const h = await client.health();
          pushInfo(text, `core ${h.version} • uptime ${h.uptime_secs}s`);
        } catch (e) {
          pushInfo(text, "", String(e));
        }
        return;
      }
      if (text.startsWith("/tier ")) {
        const next = text.slice(6).trim() as Tier;
        if ((TIERS as readonly string[]).includes(next)) {
          setTier(next);
          pushInfo(text, `tier → ${next}`);
        } else {
          pushInfo(
            text,
            "",
            `unknown tier '${next}'. Valid: ${TIERS.join(", ")}`,
          );
        }
        return;
      }
      if (text.startsWith("/agent ")) {
        await handleAgentCmd(text);
        return;
      }
      if (text === "/agents") {
        await handleAgentsList(text);
        return;
      }
      if (text.startsWith("/cancel ")) {
        await handleCancel(text);
        return;
      }
      if (text.startsWith("/plan draft ")) {
        await handlePlanDraft(text);
        return;
      }
      if (text.startsWith("/plan approve ")) {
        await handlePlanApprove(text);
        return;
      }
      if (text.startsWith("/plan cancel ")) {
        await handlePlanCancel(text);
        return;
      }
      if (text === "/plans") {
        await handlePlansList(text);
        return;
      }
      if (text === "/lessons") {
        await handleLessonsList(text);
        return;
      }
      if (text.startsWith("/lessons promote ")) {
        await handleLessonPromote(text);
        return;
      }
      if (text.startsWith("/lessons delete ")) {
        await handleLessonDelete(text);
        return;
      }
      if (text.startsWith("/gates run ")) {
        await handleGatesRun(text);
        return;
      }
      if (text.startsWith("/eval run ")) {
        await handleEvalRun(text);
        return;
      }
      if (text.startsWith("/review ") || text === "/review") {
        await handleReview(text);
        return;
      }

      // Free chat — streamed against current tier.
      const history: ChatMessage[] = [];
      for (const t of turns) {
        if (t.kind === "chat") {
          history.push({ role: "user", content: t.user });
          if (t.assistant)
            history.push({ role: "assistant", content: t.assistant });
        }
      }
      history.push({ role: "user", content: text });

      const turnIdx = turns.length;
      setTurns((t) => [
        ...t,
        { kind: "chat", user: text, assistant: "", inFlight: true },
      ]);

      try {
        for await (const ev of client.chatStream({
          tier,
          model: null,
          messages: history,
        })) {
          if (ev.type === "token") {
            setTurns((t) => {
              const next = [...t];
              const cur = next[turnIdx];
              if (cur && cur.kind === "chat") {
                next[turnIdx] = { ...cur, assistant: cur.assistant + ev.delta };
              }
              return next;
            });
          } else if (ev.type === "done") {
            setTurns((t) => {
              const next = [...t];
              const cur = next[turnIdx];
              if (cur && cur.kind === "chat") {
                next[turnIdx] = { ...cur, model: ev.model, inFlight: false };
              }
              return next;
            });
          } else if (ev.type === "error") {
            setTurns((t) => {
              const next = [...t];
              const cur = next[turnIdx];
              if (cur && cur.kind === "chat") {
                next[turnIdx] = { ...cur, error: ev.message, inFlight: false };
              }
              return next;
            });
          }
        }
      } catch (e) {
        const msg = e instanceof CoreError ? e.message : String(e);
        setTurns((t) => {
          const next = [...t];
          const cur = next[turnIdx];
          if (cur && cur.kind === "chat") {
            next[turnIdx] = { ...cur, error: msg, inFlight: false };
          }
          return next;
        });
      }
    },
    [
      client,
      tier,
      turns,
      exit,
      pushInfo,
      handleAgentCmd,
      handleAgentsList,
      handleCancel,
      handlePlanDraft,
      handlePlanApprove,
      handlePlanCancel,
      handlePlansList,
      handleLessonsList,
      handleLessonPromote,
      handleLessonDelete,
      handleGatesRun,
      handleEvalRun,
      handleReview,
    ],
  );

  useInput((ch, key) => {
    if (key.return) {
      const text = input;
      setInput("");
      void send(text);
      return;
    }
    if (key.backspace || key.delete) {
      setInput((s) => s.slice(0, -1));
      return;
    }
    if (key.ctrl && ch === "c") {
      exit();
      return;
    }
    if (ch && !key.meta && !key.ctrl) {
      setInput((s) => s + ch);
    }
  });

  const prompt = `[${tier}] ❯ `;

  return (
    <Box flexDirection="column">
      <Header coreUp={coreUp} coreVersion={coreVersion} tier={tier} />
      {turns.map((t, i) => (
        <TurnView key={i} turn={t} />
      ))}
      <Box>
        <Text color="cyan">{prompt}</Text>
        <Text>{input}</Text>
        <Text color="gray">█</Text>
      </Box>
      <Box marginTop={1}>
        <Text dimColor>
          chat · /agent {"<kind> <task>"} · /agents · /cancel {"<id>"}
        </Text>
      </Box>
      <Box>
        <Text dimColor>
          /plan draft {"<kind> <goal>"} · /plan approve {"<id>"} · /plan cancel · /plans
        </Text>
      </Box>
      <Box>
        <Text dimColor>
          /lessons · /lessons promote {"<id>"} · /lessons delete {"<id>"}
        </Text>
      </Box>
      <Box>
        <Text dimColor>
          /gates run {"<spec> | <output>"} · /eval run {"<models-csv> | <task>"}
        </Text>
      </Box>
      <Box>
        <Text dimColor>
          /review {"<head> [base]"} · /health · /tier · /quit
        </Text>
      </Box>
    </Box>
  );
}

function Header({
  coreUp,
  coreVersion,
  tier,
}: {
  coreUp: boolean | null;
  coreVersion: string | null;
  tier: Tier;
}) {
  const status =
    coreUp === null ? "probing…" : coreUp ? `core ${coreVersion}` : "core DOWN";
  const color = coreUp === null ? "gray" : coreUp ? "green" : "red";
  return (
    <Box flexDirection="column" alignItems="center" marginBottom={1}>
      <LogoKira text="KIRA" />
      <Box marginTop={1}>
        <Text dimColor>
          <Text color={color}>{status}</Text> · tier={tier}
        </Text>
      </Box>
    </Box>
  );
}

function TurnView({ turn }: { turn: Turn }) {
  if (turn.kind === "plan") {
    const statusColor =
      turn.planStatus === "done"
        ? "green"
        : turn.planStatus === "failed"
          ? "red"
          : turn.planStatus === "cancelled" || turn.planStatus === "expired"
            ? "yellow"
            : "cyan";
    return (
      <Box flexDirection="column" marginBottom={1}>
        <Box>
          <Text color="cyan">❯ </Text>
          <Text>{turn.user}</Text>
        </Box>
        {turn.error ? (
          <Box>
            <Text color="red">error: </Text>
            <Text color="red">{turn.error}</Text>
          </Box>
        ) : (
          <>
            <Box>
              <Text color="magenta">▸ </Text>
              <Text dimColor>
                plan {turn.planId?.slice(0, 8) ?? "..."}{" "}
              </Text>
              <Text color={statusColor}>{turn.planStatus ?? "draft"}</Text>
              {turn.scope ? (
                <Text color={scopeColor(turn.scope)}> [{turn.scope}]</Text>
              ) : null}
              {turn.executorAgentId ? (
                <Text dimColor> exec={turn.executorAgentId.slice(0, 8)}</Text>
              ) : null}
              {turn.inFlight ? <Text color="yellow"> ▍</Text> : null}
            </Box>
            {turn.note ? (
              <Box marginLeft={2}>
                <Text dimColor>{turn.note}</Text>
              </Box>
            ) : null}
            {turn.steps && turn.steps.length > 0 ? (
              <Box marginLeft={2} flexDirection="column">
                {turn.steps.map((s) => (
                  <Box key={s.idx}>
                    <Text color="gray">{s.idx}. </Text>
                    <Text>{s.description}</Text>
                    {s.tool ? (
                      <Text dimColor> [{s.tool}]</Text>
                    ) : null}
                  </Box>
                ))}
              </Box>
            ) : null}
          </>
        )}
      </Box>
    );
  }
  if (turn.kind === "info") {
    return (
      <Box flexDirection="column" marginBottom={1}>
        <Box>
          <Text color="cyan">❯ </Text>
          <Text>{turn.user}</Text>
        </Box>
        {turn.error ? (
          <Box>
            <Text color="red">error: </Text>
            <Text color="red">{turn.error}</Text>
          </Box>
        ) : (
          <Box>
            <Text color="yellow">ℹ </Text>
            <Text>{turn.body}</Text>
          </Box>
        )}
      </Box>
    );
  }
  if (turn.kind === "agent") {
    const statusColor =
      turn.status === "completed"
        ? "green"
        : turn.status === "failed"
          ? "red"
          : turn.status === "cancelled"
            ? "yellow"
            : "cyan";
    return (
      <Box flexDirection="column" marginBottom={1}>
        <Box>
          <Text color="cyan">❯ </Text>
          <Text>{turn.user}</Text>
        </Box>
        {turn.error ? (
          <Box>
            <Text color="red">error: </Text>
            <Text color="red">{turn.error}</Text>
          </Box>
        ) : (
          <>
            <Box>
              <Text color="magenta">▸ </Text>
              <Text dimColor>
                {turn.agentKind} {turn.agentId?.slice(0, 8) ?? "..."}{" "}
              </Text>
              <Text color={statusColor}>{turn.status ?? "spawning"}</Text>
              <Text dimColor>
                {" "}
                iters={turn.iters ?? 0} ${(turn.usd ?? 0).toFixed(4)}
                {turn.model ? ` (${turn.model})` : ""}
              </Text>
              {turn.inFlight ? <Text color="yellow"> ▍</Text> : null}
            </Box>
            {turn.result ? (
              <Box marginLeft={2}>
                <Text>{truncate(turn.result, 500)}</Text>
              </Box>
            ) : null}
          </>
        )}
      </Box>
    );
  }
  // chat turn
  return (
    <Box flexDirection="column" marginBottom={1}>
      <Box>
        <Text color="cyan">❯ </Text>
        <Text>{turn.user}</Text>
      </Box>
      {turn.error ? (
        <Box>
          <Text color="red">error: </Text>
          <Text color="red">{turn.error}</Text>
        </Box>
      ) : (
        <Box>
          <Text color="green">⠿ </Text>
          <Text>{turn.assistant}</Text>
          {turn.inFlight ? <Text color="yellow"> ▍</Text> : null}
          {turn.model ? <Text dimColor> ({turn.model})</Text> : null}
        </Box>
      )}
    </Box>
  );
}

function truncate(s: string, max: number): string {
  if (s.length <= max) return s;
  return s.slice(0, max) + "…";
}

function scopeColor(scope: string): string {
  switch (scope) {
    case "quick":
      return "green";
    case "small":
      return "cyan";
    case "medium":
      return "yellow";
    case "large":
      return "magenta";
    case "complex":
      return "red";
    default:
      return "gray";
  }
}
