#!/usr/bin/env bun
/**
 * KiraApp v3 — Ink REPL with extracted components, YOLO/Plan mode toggle.
 *
 * Modes:
 *   PLAN           — plan-first workflow, full gates, Judge evaluation
 *   YOLO (default) — direct agent spawn, reduced gates, conversation context
 */

import React, { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { Box, Text, useStdout } from "ink";
import {
  CoreClient,
  CoreError,
  type AgentKind,
} from "../core-client/client.js";
import type { StatsResponse, ModelUsage } from "../core-client/client.js";
import {
  Header,
  StatusBar,
  TurnRenderer,
  SplashScreen,
  hasSeenSplash,
  useTheme,
  InputBar,
  HelpMenu,
  AnimationClockProvider,
  ToastContainer,
  type Turn,
} from "./components/index.js";
import type { ToastItem, ToastType } from "./components/index.js";
import { generateCompanion } from "./mascot/generator.js";
import type { Emotion, MascotConfig } from "./mascot/types.js";
import { Mascot } from "./mascot/Mascot.js";
import { figletArt } from "./art.js";
import {
  parseAttachments,
  generateFakeToolPairs,
  type Attachment,
} from "./attachment-parser.js";
import { loadMotionReduced, saveMotionReduced } from "./theme.js";

// -- Types ---------------------------------------------------------------------

type Mode = "plan" | "yolo";
type Tier = "heavy" | "standard" | "light";

const TIERS: readonly Tier[] = ["heavy", "standard", "light"];
const AGENT_KINDS: readonly AgentKind[] = ["writer", "deployer", "judge", "explorer"];

// -- Turn creation helpers -----------------------------------------------------

function mkChat(user: string, inFlight = true, attachments?: Attachment[]): Turn {
  return { kind: "chat", user, assistant: "", inFlight, attachments, startedAt: Date.now() };
}

function mkInfo(user: string, body: string, inFlight = false, error?: string): Turn {
  return { kind: "info", user, body, inFlight, error, startedAt: inFlight ? Date.now() : undefined };
}

function mkAgent(
  user: string,
  agentKind: AgentKind,
  inFlight = true,
  yolo = false,
  attachments?: Attachment[],
): Turn {
  return { kind: "agent", user, agentKind, inFlight, yolo, attachments, startedAt: Date.now() };
}

function mkPlan(
  user: string,
  agentKind: AgentKind,
  goal: string,
  inFlight = true,
): Turn {
  return { kind: "plan", user, agentKind, goal, note: "drafting\u2026", inFlight, startedAt: inFlight ? Date.now() : undefined };
}

// -- Conversation context builder -----------------------------------------------

/** Extract recent agent turns as context messages for future LLM calls. */
function buildConversationContext(
  turns: Turn[],
  maxTurns = 5,
): { role: string; content: string }[] {
  const context: { role: string; content: string }[] = [];
  const completed = turns.filter(
    (t) => t.kind === "agent" && !t.inFlight && t.result,
  );
  const recent = completed.slice(-maxTurns);
  for (const turn of recent) {
    if (turn.kind === "agent") {
      context.push({ role: "user", content: turn.user });
      if (turn.result) {
        const preview =
          turn.result.length > 600
            ? turn.result.slice(0, 600) + "…"
            : turn.result;
        context.push({ role: "assistant", content: preview });
      }
    }
  }
  return context;
}

// -- Main App ------------------------------------------------------------------

export interface KiraAppProps {
  client?: CoreClient;
  initialTier?: Tier;
  initialMode?: Mode;
}

export function KiraApp({
  client = new CoreClient(),
  initialTier = "standard",
  initialMode = "yolo",
}: KiraAppProps) {
  const { mode: themeMode, setMode: setThemeMode } = useTheme();
  const { stdout } = useStdout();
  const [tier, setTier] = useState<Tier>(initialTier);
  const [mode, setMode] = useState<Mode>(initialMode);
  const [turns, setTurns] = useState<Turn[]>([]);
  const [coreUp, setCoreUp] = useState<boolean | null>(null);
  const [coreVersion, setCoreVersion] = useState<string | null>(null);
  const [workingDir, setWorkingDir] = useState(process.cwd());
  const [showSplash, setShowSplash] = useState(!hasSeenSplash());
  const [showHelp, setShowHelp] = useState(false);
  const [showGoodbye, setShowGoodbye] = useState(false);
  const [showExitHint, setShowExitHint] = useState(false);
  const [goodbyeStats, setGoodbyeStats] = useState<StatsResponse | null>(null);
  const [motionReduced, setMotionReduced] = useState(loadMotionReduced());

  const [toasts, setToasts] = useState<ToastItem[]>([]);
  const addToast = useCallback((type: ToastType, message: string) => {
    const id = String(Date.now());
    setToasts((prev) => {
      const next = [...prev, { id, type, message, createdAt: Date.now() }];
      return next.slice(-5);
    });
  }, []);

  // Session stats
  const sessionStart = useRef(Date.now());
  const turnCount = useRef(0);
  const agentCount = useRef(0);
  const commandCount = useRef(0);

  // Companion
  const sessionId = useMemo(() => `kira-${sessionStart.current.toString(36).slice(0, 12)}`, []);
  const companionConfig = useMemo<MascotConfig>(() => generateCompanion(sessionId), [sessionId]);

  // Poller refs
  const agentPollers = useRef<Map<string, ReturnType<typeof setInterval>>>(
    new Map(),
  );
  const planPollers = useRef<Map<string, ReturnType<typeof setInterval>>>(
    new Map(),
  );

  // Health check
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

  // Cleanup pollers
  useEffect(
    () => () => {
      for (const t of agentPollers.current.values()) clearInterval(t);
      for (const t of planPollers.current.values()) clearInterval(t);
    },
    [],
  );

  // Fetch stats when goodbye is triggered
  useEffect(() => {
    if (!showGoodbye) return;
    let cancelled = false;
    void (async () => {
      try {
        const s = await client.stats();
        if (!cancelled) setGoodbyeStats(s);
      } catch {
        // stats endpoint may not be available; render without it
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [showGoodbye, client]);

  // -- Helpers ----------------------------------------------------------------

  const pushInfo = useCallback(
    (user: string, body: string, error?: string) => {
      setTurns((t) => [...t, mkInfo(user, body, false, error)]);
    },
    [],
  );

  const startAgentPoller = useCallback(
    (turnIdx: number, agentId: string) => {
      const interval = setInterval(async () => {
        try {
          const fresh = await client.getAgent(agentId);
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
            ["completed", "failed", "cancelled", "stopped"].includes(fresh.status)
          ) {
            const t = agentPollers.current.get(agentId);
            if (t) {
              clearInterval(t);
              agentPollers.current.delete(agentId);
            }
          }
        } catch (e) {
          const t = agentPollers.current.get(agentId);
          if (t) {
            clearInterval(t);
            agentPollers.current.delete(agentId);
          }
        }
      }, 2000);
      agentPollers.current.set(agentId, interval);
    },
    [client],
  );

  const startPlanPoller = useCallback(
    (turnIdx: number, planId: string) => {
      const interval = setInterval(async () => {
        try {
          const fresh = await client.getPlan(planId);
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
            const t = planPollers.current.get(planId);
            if (t) {
              clearInterval(t);
              planPollers.current.delete(planId);
            }
          }
        } catch {
          const t = planPollers.current.get(planId);
          if (t) {
            clearInterval(t);
            planPollers.current.delete(planId);
          }
        }
      }, 2000);
      planPollers.current.set(planId, interval);
    },
    [client],
  );

  // -- Slash commands ---------------------------------------------------------

  const handleMode = useCallback(
    (raw: string) => {
      const rest = raw.slice("/mode".length).trim().toLowerCase();
      if (rest === "yolo") {
        setMode("yolo");
        addToast("success", "mode → YOLO (direct agent spawn)");
      } else if (rest === "plan") {
        setMode("plan");
        addToast("success", "mode → PLAN (plan-first, full gates)");
      } else {
        addToast("info", `usage: /mode <plan|yolo> (current: ${mode})`);
      }
    },
    [mode, addToast],
  );

  const handleTier = useCallback(
    (raw: string) => {
      const next = raw.slice("/tier".length).trim() as Tier;
      if ((TIERS as readonly string[]).includes(next)) {
        setTier(next);
        addToast("success", `tier → ${next}`);
      } else {
        addToast("error", `unknown tier '${next}'. Valid: ${TIERS.join(", ")}`);
      }
    },
    [addToast],
  );

  const handleAgentCmd = useCallback(
    async (raw: string) => {
      const rest = raw.slice("/agent".length).trim();
      const space = rest.indexOf(" ");
      if (space < 0) {
        pushInfo(raw, "", `usage: /agent <writer|deployer|judge|explorer> <task>`);
        return;
      }
      const kindStr = rest.slice(0, space).toLowerCase() as AgentKind;
      const task = rest.slice(space + 1).trim();
      if (!AGENT_KINDS.includes(kindStr)) {
        pushInfo(raw, "", `unknown kind. Valid: ${AGENT_KINDS.join(", ")}`);
        return;
      }
      if (!task) {
        pushInfo(raw, "", "task is empty");
        return;
      }
      const turnIdx = turns.length;
      setTurns((t) => [...t, mkAgent(raw, kindStr, true, mode === "yolo")]);
      try {
        const agent = await client.spawnAgent({
          kind: kindStr,
          task,
          worktree_id: null,
          base: null,
          budget_usd: null,
          max_iters: null,
          yolo_mode: mode === "yolo" ? true : null,
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
        startAgentPoller(turnIdx, agent.id);
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
    [client, turns.length, mode, pushInfo, startAgentPoller],
  );

  const handleAgentsList = useCallback(async () => {
    try {
      const all = await client.listAgents();
      if (all.length === 0) {
        pushInfo("/agents", "(no agents)");
        return;
      }
      const body = all
        .map(
          (a) =>
            `${a.id.slice(0, 8)} ${a.kind.padEnd(10)} ${a.status.padEnd(9)} iters=${a.iters} $${a.usd_spent.toFixed(4)}`,
        )
        .join("\n");
      pushInfo("/agents", body);
    } catch (e) {
      pushInfo("/agents", "", String(e));
    }
  }, [client, pushInfo]);

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
        pushInfo(raw, "", "goal is empty");
        return;
      }
      const turnIdx = turns.length;
      setTurns((t) => [...t, mkPlan(raw, kindStr, goal, true)]);
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
          mkInfo(raw, `approving plan ${match.id.slice(0, 8)}…`, true),
        ]);
        const r = await client.approvePlan(match.id);
        setTurns((t) => {
          const next = [...t];
          const cur = next[turnIdx];
          if (cur && cur.kind === "info") {
            next[turnIdx] = {
              ...cur,
              body: `executor: ${r.executor_agent_id.slice(0, 8)}`,
              inFlight: true,
            };
          }
          return next;
        });
        const planFresh = await client.getPlan(match.id);
        startPlanPoller(turnIdx, planFresh.id);
      } catch (e) {
        pushInfo(raw, "", String(e));
      }
    },
    [client, turns.length, pushInfo, startPlanPoller],
  );

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

  const handlePlansList = useCallback(async () => {
    try {
      const all = await client.listPlans();
      if (all.length === 0) {
        pushInfo("/plans", "(no plans)");
        return;
      }
      const body = all
        .map(
          (p) =>
            `${p.id.slice(0, 8)} ${p.agent_kind.padEnd(10)} ${p.status.padEnd(9)} ${p.scope.padEnd(7)} steps=${p.steps.length} ${p.goal.slice(0, 34)}`,
        )
        .join("\n");
      pushInfo("/plans", body);
    } catch (e) {
      pushInfo("/plans", "", String(e));
    }
  }, [client, pushInfo]);

  const handleLessonsList = useCallback(async () => {
    try {
      const all = await client.listLessons();
      if (all.length === 0) {
        pushInfo("/lessons", "(no lessons)");
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
      pushInfo("/lessons", body);
    } catch (e) {
      pushInfo("/lessons", "", String(e));
    }
  }, [client, pushInfo]);

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

  // -- /sdd — Spec-Driven Execution (P1 4-phase pipeline) --------------------

  // Color/emoji per pillar, mirroring docs/deck Excalidraw palette.
  const pillarSigil = useCallback((p: string): { emoji: string; abbr: string } => {
    switch (p) {
      case "p1_sdd":          return { emoji: "🔵", abbr: "P1 SDD" };
      case "p2_coordination": return { emoji: "🟣", abbr: "P2 Coord" };
      case "p3_verification": return { emoji: "🟡", abbr: "P3 Verify" };
      case "p4_gates":        return { emoji: "🟢", abbr: "P4 Gates" };
      case "p5_guardrails":   return { emoji: "🟠", abbr: "P5 Guard" };
      case "p6_resilience":   return { emoji: "🔴", abbr: "P6 Resil" };
      case "p7_cognition":    return { emoji: "⚪", abbr: "P7 Cog" };
      default:                return { emoji: "•",  abbr: p };
    }
  }, []);

  const handleSdd = useCallback(
    async (raw: string) => {
      // Syntax: /sdd <goal>
      //         /sdd kind=bug_fix loc=3 files=1 <goal>
      const rest = raw.slice("/sdd".length).trim();
      if (!rest) {
        pushInfo(raw, "", "usage: /sdd [kind=bug_fix|feature|docs|refactor] [loc=N] [files=N] <goal>");
        return;
      }
      // Parse k=v prefix flags then goal.
      const tokens = rest.split(/\s+/);
      let i = 0;
      const flags: Record<string, string> = {};
      while (i < tokens.length && tokens[i].includes("=")) {
        const [k, v] = tokens[i].split("=", 2);
        flags[k] = v;
        i++;
      }
      const goal = tokens.slice(i).join(" ");
      if (!goal) {
        pushInfo(raw, "", "goal required after flags");
        return;
      }
      const kindMap: Record<string, "bug_fix" | "feature" | "docs" | "refactor" | "unknown"> = {
        bug_fix: "bug_fix", bugfix: "bug_fix", bug: "bug_fix",
        feature: "feature", feat: "feature",
        docs: "docs", doc: "docs",
        refactor: "refactor",
      };
      const req = {
        goal,
        kind: (flags.kind ? (kindMap[flags.kind] ?? "unknown") : null) as
          | "bug_fix" | "feature" | "docs" | "refactor" | "unknown" | null,
        estimated_loc: flags.loc ? Number(flags.loc) : null,
        estimated_files: flags.files ? Number(flags.files) : null,
        force_full_pipeline: flags.full === "true" ? true : null,
        worktree_id: null,
        base: flags.base ?? null,
      };
      const turnIdx = turns.length;
      setTurns((t) => [
        ...t,
        {
          kind: "info",
          user: raw,
          body: "sdd starting…",
          inFlight: true,
        },
      ]);
      let run;
      try {
        run = await client.startSdd(req);
      } catch (e) {
        setTurns((t) => {
          const next = [...t];
          const cur = next[turnIdx];
          if (cur && cur.kind === "info") {
            next[turnIdx] = { ...cur, error: String(e), inFlight: false };
          }
          return next;
        });
        return;
      }
      const idShort = run.id.slice(0, 8);
      const lines: string[] = [
        `🔵 P1 SDD → run ${idShort} · sizing=${run.sizing} · first_phase=${run.current_phase}`,
      ];
      const flush = () => {
        setTurns((t) => {
          const next = [...t];
          const cur = next[turnIdx];
          if (cur && cur.kind === "info") {
            next[turnIdx] = { ...cur, body: lines.join("\n"), inFlight: true };
          }
          return next;
        });
      };
      flush();

      // Stream pillar events filtered by run_id.
      const ctrl = client.streamPillarEvents(
        run.id,
        (ev) => {
          const { emoji, abbr } = pillarSigil(ev.pillar);
          lines.push(`${emoji} ${abbr} → [${ev.phase}] ${ev.message}`);
          flush();
        },
        () => { /* swallow — daemon may have closed */ },
      );

      // Poll terminal state — daemon drives detached so we wait.
      const deadline = Date.now() + 30 * 60 * 1000; // 30min cap
      let finalRun = run;
      while (Date.now() < deadline) {
        await new Promise((r) => setTimeout(r, 2000));
        try {
          finalRun = await client.getSdd(run.id);
          if (finalRun.current_phase === "done" || finalRun.current_phase === "failed") break;
        } catch {
          /* keep polling */
        }
      }
      ctrl.abort();

      // Final summary line.
      const dirRel = `.kira/runs/${run.id}/`;
      lines.push("");
      lines.push(
        `✅ ${finalRun.current_phase} · phases_run=${finalRun.phases_run.join(",") || "(none)"} · skipped=${finalRun.phases_skipped.join(",") || "(none)"} · $${finalRun.usd_spent.toFixed(4)}`,
      );
      lines.push(`artifacts → ${dirRel}`);
      if (finalRun.error) lines.push(`error: ${finalRun.error}`);
      setTurns((t) => {
        const next = [...t];
        const cur = next[turnIdx];
        if (cur && cur.kind === "info") {
          next[turnIdx] = { ...cur, body: lines.join("\n"), inFlight: false };
        }
        return next;
      });
    },
    [client, turns.length, pushInfo, pillarSigil],
  );

  const handleSddList = useCallback(async () => {
    try {
      const r = await client.listSdd();
      if (r.runs.length === 0) {
        pushInfo("/sdds", "(no runs)");
        return;
      }
      const lines = r.runs.map(
        (run) =>
          `${run.id.slice(0, 8)}  ${run.current_phase.padEnd(8)} ${run.kind.padEnd(10)} $${run.usd_spent.toFixed(4)}  ${run.goal.slice(0, 60)}`,
      );
      pushInfo("/sdds", lines.join("\n"));
    } catch (e) {
      pushInfo("/sdds", "", String(e));
    }
  }, [client, pushInfo]);

  // -- /search <query> — SearXNG sovereign search -----------------------------

  const handleSearch = useCallback(
    async (raw: string) => {
      const query = raw.slice("/search".length).trim();
      if (!query) {
        pushInfo(raw, "", "usage: /search <query>");
        return;
      }
      try {
        const r = await client.webSearch({ query });
        if (r.results.length === 0) {
          pushInfo(raw, "(no results)");
          return;
        }
        const lines = r.results.map(
          (s, i) =>
            `${(i + 1).toString().padStart(2)}. [${s.engine.padEnd(12)}] ${s.title}\n    ${s.url}\n    ${s.snippet.slice(0, 120)}`,
        );
        pushInfo(raw, lines.join("\n\n"));
      } catch (e) {
        pushInfo(raw, "", String(e));
      }
    },
    [client, pushInfo],
  );

  // -- /fetch <url> — readability + htmd → markdown ---------------------------

  const handleFetch = useCallback(
    async (raw: string) => {
      const url = raw.slice("/fetch".length).trim();
      if (!url) {
        pushInfo(raw, "", "usage: /fetch <url>");
        return;
      }
      try {
        const r = await client.webFetch({ url });
        const head = `source=${r.result.source} · final=${r.result.final_url} · ${r.result.bytes_read}B`;
        const body = r.result.markdown.slice(0, 1200);
        const more = r.result.markdown.length > 1200 ? `\n\n...(${r.result.markdown.length - 1200} more chars)` : "";
        pushInfo(raw, `${head}\n\n${body}${more}`);
      } catch (e) {
        pushInfo(raw, "", String(e));
      }
    },
    [client, pushInfo],
  );

  // -- Free text / YOLO dispatch ----------------------------------------------

  const handleFreeText = useCallback(
    async (text: string) => {
      // Parse @-mentions for file attachments
      const { text: cleanedText, attachments } = parseAttachments(text, workingDir);
      
      if (mode === "yolo") {
        // YOLO: spawn agent directly, with conversation context
        const contextMsgs = buildConversationContext(turns);
        const fakePairs = generateFakeToolPairs(attachments);
        const messages = [
          ...contextMsgs,
          { role: "user" as const, content: cleanedText },
          ...fakePairs.flat(),
        ];
        const turnIdx = turns.length;
        setTurns((t) => [...t, mkAgent(cleanedText, "writer", true, true, attachments)]);
        try {
          const yolo = await client.chatYolo({
            tier,
            model: null,
            messages,
            mode: "yolo",
          });
          setTurns((t) => {
            const next = [...t];
            const cur = next[turnIdx];
            if (cur && cur.kind === "agent") {
              next[turnIdx] = {
                ...cur,
                agentId: yolo.agent_id,
                status: yolo.status,
              };
            }
            return next;
          });
          startAgentPoller(turnIdx, yolo.agent_id);
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
      } else {
        // PLAN mode: auto-draft a plan with conversation context
        const planContextMsgs = buildConversationContext(turns);
        const contextPreamble = planContextMsgs.length > 0
          ? planContextMsgs.map(m => `${m.role}: ${m.content}`).join('\n') + '\n\nCurrent request: '
          : '';
        const turnIdx = turns.length;
        setTurns((t) => [
          ...t,
          mkPlan(cleanedText, "writer", cleanedText, true),
        ]);
        try {
          const plan = await client.draftPlan({
            goal: contextPreamble + cleanedText,
            agent_kind: "writer",
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
      }
    },
    [client, mode, tier, turns.length, startAgentPoller],
  );

  // -- Main dispatch ----------------------------------------------------------

  const send = useCallback(
    async (raw: string) => {
      const text = raw.trim();
      if (!text) return;

      // Track stats
      turnCount.current++;
      if (text.startsWith("/")) {
        commandCount.current++;
      }

      if (text === "/quit" || text === "/exit") {
        setShowGoodbye(true);
        return;
      }

      if (text === "/health") {
        try {
          const h = await client.health();
          pushInfo(text, `core ${h.version} · uptime ${h.uptime_secs}s`);
        } catch (e) {
          pushInfo(text, "", String(e));
        }
        return;
      }

      if (text === "/help") {
        setShowHelp(true);
        return;
      }

      if (text.startsWith("/mode ")) {
        handleMode(text);
        return;
      }
      if (text.startsWith("/tier ")) {
        handleTier(text);
        return;
      }
      if (text.startsWith("/theme")) {
        const themeArg = text.slice("/theme".length).trim();
        const valid = ["dark", "light", "dark-ansi", "dracula", "catppuccin", "nord", "tokyonight", "gruvbox", "solarized"];
        if (valid.includes(themeArg)) {
          setThemeMode(themeArg);
          addToast("success", `theme → ${themeArg}`);
        } else if (!themeArg) {
          addToast("info", `theme: ${themeMode}. Usage: /theme <${valid.join("|")}>`);
        } else {
          addToast("error", `unknown theme '${themeArg}'. Valid: ${valid.join(", ")}`);
        }
        return;
      }
      if (text.startsWith("/motion")) {
        const arg = text.slice("/motion".length).trim();
        if (arg === "on") {
          setMotionReduced(false);
          saveMotionReduced(false);
          addToast("success", "motion → full");
        } else if (arg === "off") {
          setMotionReduced(true);
          saveMotionReduced(true);
          addToast("success", "motion → reduced (animations paused)");
        } else {
          addToast("info", `motion: ${motionReduced ? "reduced" : "full"}. Usage: /motion <on|off>`);
        }
        return;
      }
      if (text.startsWith("/agent ")) {
        await handleAgentCmd(text);
        return;
      }
      if (text === "/agents") {
        await handleAgentsList();
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
        await handlePlansList();
        return;
      }
      if (text === "/lessons") {
        await handleLessonsList();
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
      if (text === "/sdds") {
        await handleSddList();
        return;
      }
      if (text.startsWith("/sdd")) {
        await handleSdd(text);
        return;
      }
      if (text.startsWith("/search ")) {
        await handleSearch(text);
        return;
      }
      if (text.startsWith("/fetch ")) {
        await handleFetch(text);
        return;
      }

      // Free text → mode dispatch
      await handleFreeText(text);
    },
    [
      client,
      pushInfo,
      handleMode,
      handleTier,
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
      handleSdd,
      handleSddList,
      handleSearch,
      handleFetch,
      handleFreeText,
      setThemeMode,
      themeMode,
    ],
  );

  // -- Render -----------------------------------------------------------------

  const activeAgents = turns.filter(
    (t) => t.kind === "agent" && t.inFlight,
  ).length;
  const completedTurns = turns.filter(
    (t) => t.kind === "agent" && t.status === "completed",
  ).length;
  const failedTurns = turns.filter(
    (t) => t.kind === "agent" && t.status === "failed",
  ).length;
  const mascotEmotion = useMemo<Emotion>(() => {
    if (failedTurns > 0) return "sad";
    if (activeAgents > 0) return "surprised";
    if (completedTurns > 0) return "happy";
    return "neutral";
  }, [activeAgents, completedTurns, failedTurns]);
  const termWidth = stdout?.columns ?? 80;
  const boxWidth = Math.min(86, Math.max(60, termWidth - 4));

  // Format wall time
  function formatTime(ms: number): string {
    const secs = Math.floor(ms / 1000);
    const mins = Math.floor(secs / 60);
    const hrs = Math.floor(mins / 60);
    if (hrs > 0) return `${hrs}h ${mins % 60}m ${secs % 60}s`;
    if (mins > 0) return `${mins}m ${secs % 60}s`;
    return `${secs}s`;
  }

  const wallTime = formatTime(Date.now() - sessionStart.current);
  const totalTurns = turnCount.current;
  const totalAgents = turns.filter((t) => t.kind === "agent").length;
  const completedAgents = turns.filter(
    (t) => t.kind === "agent" && !t.inFlight,
  ).length;
  const totalCommands = commandCount.current;

  const goodbyeArt = useMemo(() => figletArt("GOODBYE", "Small"), []);

  // Show goodbye summary
  if (showGoodbye) {
    const sessionId = `kira-${sessionStart.current.toString(36).slice(0, 12)}`;
    const stats = goodbyeStats;
    const totalToolCalls = totalAgents;
    const successfulAgents = turns.filter(
      (t) => t.kind === "agent" && t.status === "completed",
    ).length;
    const successRate =
      totalAgents > 0
        ? ((successfulAgents / totalAgents) * 100).toFixed(1)
        : "—";

    const fmtNum = (n: number): string => n.toLocaleString();
    const costColor = (usd: number): string => {
      if (usd === 0) return "#666666";
      if (usd < 0.01) return "#4ADE80";
      if (usd < 0.10) return "#FBBF24";
      if (usd < 1.00) return "#F97316";
      return "#EF4444";
    };

    const totalTokensIn = stats
      ? stats.model_usage.reduce((sum, m) => sum + Number(m.tokens_in), 0)
      : 0;
    const totalTokensOut = stats
      ? stats.model_usage.reduce((sum, m) => sum + Number(m.tokens_out), 0)
      : 0;

    const contentWidth = boxWidth - 4;

    const sep = () => (
      <Text color="#444444">{"─".repeat(contentWidth)}</Text>
    );

    const labelRow = (
      label: string,
      value: string,
      opts?: { color?: string },
    ) => (
      <Text>
        <Text color="#888888">{`${label}:`.padEnd(22)}</Text>
        <Text color={opts?.color ?? "#E8E8E8"}>{value}</Text>
      </Text>
    );

    const header = (title: string) => (
      <Text bold color="#D4D4D4">{`  ${title}`}</Text>
    );

    const content = (lines: React.ReactNode) => (
      <Box flexDirection="column" paddingLeft={2}>
        {lines}
      </Box>
    );

    return (
      <Box flexDirection="column" paddingX={1} marginTop={1}>
        <Box
          flexDirection="column"
          borderStyle="classic"
          borderColor="#A855F7"
          width={boxWidth}
          paddingX={1}
          paddingY={0}
        >
          {/* Title */}
          <Box flexDirection="column" paddingLeft={1} paddingTop={0} paddingBottom={1}>
            {goodbyeArt.map((line, i) => (
              <Text key={i} color="#A855F7">{line}</Text>
            ))}
          </Box>

          <Box paddingLeft={1}>{sep()}</Box>

          {/* Interaction Summary */}
          <Box paddingLeft={1} marginTop={0}>{header("Interaction Summary")}</Box>
          <Box paddingLeft={1}>{labelRow("Session ID", sessionId, { color: "#888888" })}</Box>
          <Box paddingLeft={1}>{labelRow("Wall Time", wallTime)}</Box>
          <Box paddingLeft={1}>{labelRow("Total Turns", String(totalTurns))}</Box>
          <Box paddingLeft={1}>{labelRow("Commands", String(totalCommands))}</Box>
          <Box paddingLeft={1}>{labelRow("Agents Spawned", String(totalAgents))}</Box>
          <Box paddingLeft={1}>{labelRow("Agents Completed", String(successfulAgents))}</Box>
          <Box paddingLeft={1}>{labelRow("Success Rate", `${successRate}%`)}</Box>

          <Box paddingLeft={1}>{sep()}</Box>

          {/* Performance */}
          <Box paddingLeft={1} marginTop={0}>{header("Performance")}</Box>
          <Box paddingLeft={1}>{labelRow("API Calls", String(totalToolCalls))}</Box>
          <Box paddingLeft={1}>
            {labelRow("Model Tier", tier, {
              color: tier === "heavy" ? "#E879A8" : tier === "light" ? "#4ADE80" : "#87CEEB",
            })}
          </Box>
          {stats && (
            <Box paddingLeft={1}>{labelRow("USD Spent", `$${stats.usd_spent.toFixed(4)}`, { color: costColor(stats.usd_spent) })}</Box>
          )}

          {/* Model Usage */}
          {stats && stats.model_usage.length > 0 && (
            <>
              <Box paddingLeft={1}>{sep()}</Box>
              <Box paddingLeft={1} marginTop={0}>{header("Model Usage")}</Box>
              <Box paddingLeft={1}>
                <Text bold color="#888888">
                  {`  Model${" ".repeat(14)}Reqs     Input      Output`}
                </Text>
              </Box>
              {stats.model_usage.map((m) => (
                <Box key={m.model} paddingLeft={1}>
                  <Text color="#C8C8C8">{`  ${m.model.padEnd(20)}${String(m.requests).padStart(5)}  ${fmtNum(Number(m.tokens_in)).padStart(10)}  ${fmtNum(Number(m.tokens_out)).padStart(10)}`}</Text>
                </Box>
              ))}
              <Box paddingLeft={1}>
                <Text color="#E8E8E8">{`  Total: ${fmtNum(totalTokensIn)} in / ${fmtNum(totalTokensOut)} out`}</Text>
              </Box>
            </>
          )}

          <Box paddingLeft={1}>{sep()}</Box>

          {/* Restart hint */}
          <Box paddingLeft={1}>
            <Text color="#888888">Restart: bun run src/cli/kira.tsx</Text>
          </Box>

          <Box paddingLeft={0}>{sep()}</Box>

          {/* Dead companion */}
          <Box paddingLeft={1} paddingBottom={1}>
            <Mascot config={companionConfig} emotion="dead" />
            <Box flexDirection="column" marginLeft={2}>
              <Text color="#666666" dimColor>Your companion rests</Text>
              <Text color="#666666" dimColor>until the next write.</Text>
            </Box>
          </Box>
        </Box>
      </Box>
    );
  }

  // Show splash on first launch
  if (showSplash) {
    return (
      <SplashScreen
        coreUp={coreUp}
        coreVersion={coreVersion}
        tier={tier}
        mode={mode}
        workingDir={workingDir}
        onDismiss={() => setShowSplash(false)}
      />
    );
  }

  // Show help menu
  if (showHelp) {
    return (
      <Box flexDirection="column">
        <Header
          coreUp={coreUp}
          coreVersion={coreVersion}
          tier={tier}
          mode={mode}
          workingDir={workingDir}
        />
        <HelpMenu onDismiss={() => setShowHelp(false)} />
      </Box>
    );
  }

  return (
    <AnimationClockProvider>
      <Box flexDirection="column">
        {/* Header */}
        <Header
          coreUp={coreUp}
          coreVersion={coreVersion}
          tier={tier}
          mode={mode}
          workingDir={workingDir}
        />

        {/* Turns */}
        {turns.map((t, i) => (
          <TurnRenderer key={i} turn={t} />
        ))}

        {/* Toasts */}
        <ToastContainer toasts={toasts} />

        {/* Input line */}
        <InputBar
          mode={mode}
          disabled={showHelp}
          onSubmit={send}
          onFirstCtrlC={() => setShowExitHint(true)}
          onDoubleCtrlC={() => setShowGoodbye(true)}
        />

        {/* Status bar */}
        <StatusBar
          tier={tier}
          mode={mode}
          agentCount={totalAgents}
          activeAgents={activeAgents}
          themeMode={themeMode}
          showExitHint={showExitHint}
          onDismissExitHint={() => setShowExitHint(false)}
          newMessageCount={completedTurns}
          mascotConfig={companionConfig}
          mascotEmotion={mascotEmotion}
        />
      </Box>
    </AnimationClockProvider>
  );
}
