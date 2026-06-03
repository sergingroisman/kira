#!/usr/bin/env bun
/**
 * KiraApp v3 — Ink REPL with extracted components, YOLO/Plan mode toggle.
 *
 * Modes:
 *   PLAN           — plan-first workflow, full gates, Judge evaluation
 *   YOLO (default) — direct agent spawn, reduced gates, conversation context
 */

import React, { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { Box, Text, useStdout, AlternateScreen, useTerminalSize } from "../ink/kira-ink.js";
import { FullscreenLayout } from "./components/FullscreenLayout.js";
import {
  CoreClient,
  CoreError,
  type AgentKind,
  type PendingApproval,
  type PlanQuestion,
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
  PomodoroProvider,
  BuddyChat,
  ThinkingDisplay,
  ApprovalPrompt,
  PlanQuestions,
  type Turn,
  type SubAgentInfo,
  type ThinkingCategory,
} from "./components/index.js";
import type { ToastItem, ToastType } from "./components/index.js";
import { generateCompanion, generateSoul } from "./mascot/generator.js";
import type { Emotion, MascotConfig, PersistedCompanionState, TamaAction, PomodoroState, PomodoroPhase } from "./mascot/types.js";
import { Mascot } from "./mascot/Mascot.js";

import { xpToLevel } from "./mascot/types.js";
import { applyDecay, dispatchAction, pomodoroAutoCare, tamaEmotion, setDecayFocusDuration } from "./mascot/tamagotchi.js";
import { loadCompanionState, saveCompanionState } from "./mascot/persistence.js";

import {
  parseAttachments,
  generateFakeToolPairs,
  type Attachment,
} from "./attachment-parser.js";
import { interpolatePalette } from "./art.js";
import { loadMotionReduced, saveMotionReduced } from "./theme.js";
import { readFile } from "node:fs/promises";

// -- Types ---------------------------------------------------------------------

type Mode = "plan" | "yolo";
type Tier = "heavy" | "standard" | "light";

const TIERS: readonly Tier[] = ["heavy", "standard", "light"];
const AGENT_KINDS: readonly AgentKind[] = ["writer", "deployer", "judge", "explorer"];

const GOODBYE_BANNER = [
  "______     ______     ______     _____     ______     __  __     ______    ",
  "/\\  ___\\   /\\  __ \\   /\\  __ \\   /\\  __-.  /\\  == \\   /\\ \\_\\ \\   /\\  ___\\   ",
  "\\ \\ \\__ \\  \\ \\ \\/\\ \\  \\ \\ \\/\\ \\  \\ \\ \\/\\ \\ \\ \\  __<   \\ \\____ \\  \\ \\  __\\   ",
  " \\ \\_____\\  \\ \\_____\\  \\ \\_____\\  \\ \\____-  \\ \\_____\\  \\/\\_____\\  \\ \\_____\\ ",
  "  \\/_____/   \\/_____/   \\/_____/   \\/____/   \\/_____/   \\/_____/   \\/_____/ ",
];

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
  const { mode: themeMode, setMode: setThemeMode, colors } = useTheme();
  const { stdout } = useStdout();
  const { rows: terminalRows } = useTerminalSize();
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

  const [pendingApproval, setPendingApproval] = useState<{
    agentId: string;
    agentKind: string;
    approval: PendingApproval;
  } | null>(null);
  // Tracks approvals already decided by the user so the poller doesn't
  // re-show the same tool while the server is processing the verdict.
  const decidedApprovalRef = useRef<{ agentId: string; toolName: string } | null>(null);

  const [pendingSddQuestions, setPendingSddQuestions] = useState<{
    sddRunId: string;
    questions: PlanQuestion[];
  } | null>(null);

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
  const companionPersistedRef = useRef<PersistedCompanionState>(loadCompanionState());
  const [companionState, setCompanionState] = useState<PersistedCompanionState>(companionPersistedRef.current);
  const [companionWidgetMessage, setCompanionWidgetMessage] = useState<string | null>(null);
  const [thinkingCategory, setThinkingCategory] = useState<ThinkingCategory>("idle");
  const [thinkingStalled, setThinkingStalled] = useState(false);
  const [totalSessionTokens, setTotalSessionTokens] = useState(0);
  const thinkingLastTokenRef = useRef<{ time: number; count: number }>({ time: 0, count: 0 });

  // Pomodoro callbacks
  const pomodoroPrevPhaseRef = useRef<PomodoroPhase>(companionState.pomodoro.phase);
  const handlePomodoroOnTick = useCallback((next: PomodoroState) => {
    const prevPhase = pomodoroPrevPhaseRef.current;
    pomodoroPrevPhaseRef.current = next.phase;
    const prev = companionPersistedRef.current;
    const nextState = { ...prev, pomodoro: next };
    companionPersistedRef.current = nextState;
    setCompanionState(nextState);
    saveCompanionState(nextState);
    if (prevPhase === "focus" && next.phase !== "idle" && next.phase !== "focus") {
      const tama = { ...prev.tama };
      pomodoroAutoCare(tama);
      const xpGain = 100;
      const xp = { ...prev.xp, total: prev.xp.total + xpGain };
      const final = { ...nextState, tama, xp };
      companionPersistedRef.current = final;
      setCompanionState(final);
      saveCompanionState(final);
      const prevLevel = xpToLevel(prev.xp.total).level;
      const newLevel = xpToLevel(final.xp.total).level;
      if (newLevel > prevLevel) {
        setCompanionWidgetMessage(`\u2192 Lv.${newLevel}!`);
      } else {
        setCompanionWidgetMessage(`Pomodoro done! +${xpGain}XP`);
      }
      addToast("success", "\u25CF Pomodoro completed! Coglu is happy.");
    }
    if (prevPhase !== "focus" && next.phase === "idle" && next.completed > 0) {
      addToast("info", "Break over. Ready for next focus!");
    }
  }, [addToast]);

  // Poller refs
  const agentPollers = useRef<Map<string, ReturnType<typeof setInterval>>>(
    new Map(),
  );
  const planPollers = useRef<Map<string, ReturnType<typeof setInterval>>>(
    new Map(),
  );
  // Active pillar-events stream (started by `/events`). One at a time.
  const eventsCtrl = useRef<AbortController | null>(null);

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

  // Auto-start pomodoro after splash is dismissed
  const pomodoroStartedRef = useRef(false);
  useEffect(() => {
    if (!showSplash && !pomodoroStartedRef.current && companionState.companion) {
      pomodoroStartedRef.current = true;
      const now = Date.now();
      const prev = companionPersistedRef.current;
      const nextPomo = { ...prev.pomodoro, phase: "focus" as const, startedAt: now, elapsed: 0 };
      const next = { ...prev, pomodoro: nextPomo };
      companionPersistedRef.current = next;
      setCompanionState(next);
      saveCompanionState(next);
    }
  }, [showSplash]); // eslint-disable-line react-hooks/exhaustive-deps

  // Companion: generate soul on first mount
  useEffect(() => {
    if (!companionState.companion) {
      const soul = generateSoul(sessionId);
      const next = { ...companionPersistedRef.current, companion: soul };
      companionPersistedRef.current = next;
      setCompanionState(next);
      saveCompanionState(next);
    }
    setDecayFocusDuration(companionPersistedRef.current.pomodoro.focusDuration);
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // Companion: token polling (every 30s)
  useEffect(() => {
    const timer = setInterval(async () => {
      try {
        const stats = await client.stats();
        const totalIn = stats.model_usage.reduce((s, m) => s + Number(m.tokens_in), 0);
        const totalOut = stats.model_usage.reduce((s, m) => s + Number(m.tokens_out), 0);
        const totalTokens = totalIn + totalOut;

        const prev = companionPersistedRef.current;
        const deltaIn = Math.max(0, totalIn - prev.xp.lastSeenTokensIn);
        const deltaOut = Math.max(0, totalOut - prev.xp.lastSeenTokensOut);
        if (deltaIn + deltaOut > 0) {
          const xpGain = Math.floor((deltaIn + deltaOut) / 1000);
          if (xpGain <= 0) return;
          const prevLevel = xpToLevel(prev.xp.total).level;
          const next: PersistedCompanionState = {
            ...prev,
            xp: {
              total: prev.xp.total + xpGain,
              lastSeenTokensIn: totalIn,
              lastSeenTokensOut: totalOut,
            },
            tama: { ...prev.tama },
          };
          companionPersistedRef.current = next;
          setCompanionState(next);
          saveCompanionState(next);

          const newLevel = xpToLevel(next.xp.total).level;
          if (newLevel > prevLevel) {
            setCompanionWidgetMessage(`\u2192 Lv.${newLevel}!`);
          } else if (xpGain > 0) {
            setCompanionWidgetMessage(`+${xpGain} XP`);
          }
        }
      } catch {
        // core may be down
      }
    }, 30000);
    return () => clearInterval(timer);
  }, [client]);

  // ThinkingDisplay: fast poller for tokens + stall (5s, only when agents active)
  const fastPollerActiveAgents = turns.filter(
    (t) => t.kind === "agent" && t.inFlight,
  ).length;
  useEffect(() => {
    if (fastPollerActiveAgents === 0) return;
    const timer = setInterval(async () => {
      try {
        const stats = await client.stats();
        const totalIn = stats.model_usage.reduce((s, m) => s + Number(m.tokens_in), 0);
        const totalOut = stats.model_usage.reduce((s, m) => s + Number(m.tokens_out), 0);
        const totalTokens = totalIn + totalOut;
        setTotalSessionTokens(totalTokens);

        const now = Date.now();
        if (totalTokens > thinkingLastTokenRef.current.count) {
          thinkingLastTokenRef.current = { time: now, count: totalTokens };
          setThinkingStalled(false);
        } else if (thinkingLastTokenRef.current.time > 0 &&
                   now - thinkingLastTokenRef.current.time > 3000) {
          setThinkingStalled(true);
        }
      } catch { /* core may be down */ }
    }, 5000);
    return () => clearInterval(timer);
  }, [client, fastPollerActiveAgents]);

  // Companion: tamagotchi decay (every 5s)
  useEffect(() => {
    let persistCounter = 0;
    const timer = setInterval(() => {
      const prev = companionPersistedRef.current;
      if (prev.tama.isDead) return;
      const tama = { ...prev.tama };
      applyDecay(tama, Date.now());
      const next = { ...prev, tama };
      companionPersistedRef.current = next;
      setCompanionState(next);
      persistCounter++;
      if (persistCounter % 6 === 0) saveCompanionState(next);
    }, 5000);
    return () => {
      clearInterval(timer);
      saveCompanionState(companionPersistedRef.current);
    };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // Companion: show widget at end of each turn
  const completedTurnsRef = useRef(0);
  useEffect(() => {
    if (turns.length === 0) { completedTurnsRef.current = 0; return; }
    const doneCount = turns.filter((t) => !t.inFlight).length;
    if (doneCount > completedTurnsRef.current) {
      completedTurnsRef.current = doneCount;
      if (companionState.companion) {
        setCompanionWidgetMessage(null);
      }
    }
  }, [turns]); // eslint-disable-line react-hooks/exhaustive-deps

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

          // SC2 — surface approval prompt when agent awaits HIGH-risk tool.
          if (fresh.awaiting_approval) {
            const decided = decidedApprovalRef.current;
            const alreadyDecided =
              decided?.agentId === agentId &&
              decided?.toolName === fresh.awaiting_approval.tool_name;
            if (!alreadyDecided) {
              setPendingApproval((prev) =>
                prev?.agentId === agentId
                  ? prev
                  : {
                      agentId,
                      agentKind: fresh.kind,
                      approval: fresh.awaiting_approval!,
                    },
              );
            }
          } else {
            // Server cleared awaiting_approval — clear the decided guard too.
            decidedApprovalRef.current = null;
            // Clear if agent no longer needs approval (verdict received).
            setPendingApproval((prev) =>
              prev?.agentId === agentId ? null : prev,
            );
          }

          if (
            ["completed", "failed", "cancelled", "stopped"].includes(fresh.status)
          ) {
            const t = agentPollers.current.get(agentId);
            if (t) {
              clearInterval(t);
              agentPollers.current.delete(agentId);
            }

            // Fetch worktree diff to show generated files
            if (fresh.status === "completed") {
              void (async () => {
                try {
                  const diff = await client.getAgentDiff(agentId);
                  if (diff && diff.trim().length > 0) {
                    setTurns((t) => {
                      const next = [...t];
                      const cur = next[turnIdx];
                      if (cur && cur.kind === "agent") {
                        next[turnIdx] = { ...cur, diff };
                      }
                      return next;
                    });
                  }
                } catch { /* diff may be empty or unavailable */ }
              })();
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

  const handleSddQuestionSubmit = useCallback(
    async (answers: Record<string, string[]>) => {
      if (!pendingSddQuestions) return;
      const { sddRunId } = pendingSddQuestions;
      setPendingSddQuestions(null);
      try {
        await client.answerSddQuestions(sddRunId, answers);
        addToast("success", `Answers submitted for SDD run ${sddRunId.slice(0, 8)}`);
      } catch (e) {
        addToast("error", `Failed to submit answers: ${String(e)}`);
      }
    },
    [pendingSddQuestions, client, addToast],
  );

  const handleApprovalDecide = useCallback(
    async (approved: boolean, feedback?: string) => {
      if (!pendingApproval) return;
      const { agentId, approval } = pendingApproval;
      // Guard against poller re-showing while server processes verdict.
      decidedApprovalRef.current = { agentId, toolName: approval.tool_name };
      setPendingApproval(null);
      try {
        await client.approveAgentTool(agentId, approved, feedback);
        addToast(
          approved ? "success" : "warning",
          approved
            ? `Tool approved for agent ${agentId.slice(0, 8)}`
            : `Tool rejected for agent ${agentId.slice(0, 8)}${feedback ? `: ${feedback}` : ""}`,
        );
      } catch (e) {
        addToast("error", `Approval request failed: ${String(e)}`);
      }
    },
    [pendingApproval, client, addToast],
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

          // Surface pending questions → show PlanQuestions UI.
          if (finalRun.pending_questions && finalRun.pending_questions.length > 0) {
            setPendingSddQuestions((prev) =>
              prev?.sddRunId === run.id
                ? prev
                : { sddRunId: run.id, questions: finalRun.pending_questions },
            );
          } else {
            setPendingSddQuestions((prev) =>
              prev?.sddRunId === run.id ? null : prev,
            );
          }
        } catch {
          /* keep polling */
        }
      }
      ctrl.abort();
      setPendingSddQuestions((prev) => (prev?.sddRunId === run.id ? null : prev));

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

  // -- /review <head> [base] [timeout=N] --------------------------------------

  const handleReview = useCallback(
    async (raw: string) => {
      const rest = raw.slice("/review".length).trim();
      if (!rest) {
        pushInfo(raw, "", "usage: /review <head> [base] [timeout=N]");
        return;
      }
      const tokens = rest.split(/\s+/);
      const flags: Record<string, string> = {};
      const positional: string[] = [];
      for (const tok of tokens) {
        if (tok.includes("=")) {
          const [k, v] = tok.split("=", 2);
          flags[k] = v;
        } else {
          positional.push(tok);
        }
      }
      const head = positional[0];
      const base = positional[1] ?? null;
      const timeout = flags.timeout ? Number(flags.timeout) : null;
      if (!head) {
        pushInfo(raw, "", "head branch required");
        return;
      }
      const turnIdx = turns.length;
      setTurns((t) => [
        ...t,
        { kind: "info", user: raw, body: `🟡 P3 Review → ${head} vs ${base ?? "main"} starting…`, inFlight: true },
      ]);
      const lines: string[] = [`🟡 P3 Review → ${head} vs ${base ?? "main"} starting…`];
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

      // Subscribe to all pillar events (no run_id filter — review fires
      // multiple agent runs, each with its own id).
      const ctrl = client.streamPillarEvents(
        undefined,
        (ev) => {
          const { emoji, abbr } = pillarSigil(ev.pillar);
          lines.push(`${emoji} ${abbr} → [${ev.phase}] ${ev.message}`);
          flush();
        },
        () => { /* swallow */ },
      );

      try {
        const resp = await client.runReview({
          workflow: null,
          head,
          base,
          timeout_secs: timeout != null ? BigInt(timeout) : null,
        });
        ctrl.abort();
        lines.push("");
        lines.push(
          `✅ run ${resp.run_id.slice(0, 8)} · agents=${resp.agents.length} · findings=${resp.total_findings} · ${Number(resp.duration_ms) / 1000}s`,
        );
        for (const a of resp.agents) {
          lines.push(
            `  · ${a.dimension.padEnd(14)} ${a.status.padEnd(9)} kept=${a.findings_kept} dropped=${a.findings_dropped} gate=${a.gate_overall}${a.gate_failed_at ? ` (failed_at=${a.gate_failed_at})` : ""}`,
          );
        }
        lines.push(`REVIEW.md → ${resp.review_md_path}`);
        setTurns((t) => {
          const next = [...t];
          const cur = next[turnIdx];
          if (cur && cur.kind === "info") {
            next[turnIdx] = { ...cur, body: lines.join("\n"), inFlight: false };
          }
          return next;
        });
      } catch (e) {
        ctrl.abort();
        const msg = e instanceof CoreError ? e.message : String(e);
        setTurns((t) => {
          const next = [...t];
          const cur = next[turnIdx];
          if (cur && cur.kind === "info") {
            next[turnIdx] = { ...cur, body: lines.join("\n"), error: msg, inFlight: false };
          }
          return next;
        });
      }
    },
    [client, turns.length, pushInfo, pillarSigil],
  );

  // -- /eval <path-to-json> ---------------------------------------------------

  const handleEvalFile = useCallback(
    async (raw: string) => {
      const path = raw.slice("/eval".length).trim();
      if (!path) {
        pushInfo(raw, "", "usage: /eval <path-to-config.json>  (see docs/eval-config.md)");
        return;
      }
      let cfg: {
        tasks: Array<{ id: string; spec: string; expected_topic?: string | null; models?: string[] | null }>;
        models: string[];
        skip_embedding?: boolean;
        skip_judge?: boolean;
      };
      try {
        const text = await readFile(path, "utf-8");
        cfg = JSON.parse(text);
      } catch (e) {
        pushInfo(raw, "", `read/parse failed: ${e}`);
        return;
      }
      if (!Array.isArray(cfg.tasks) || cfg.tasks.length === 0) {
        pushInfo(raw, "", "config.tasks must be a non-empty array");
        return;
      }
      if (!Array.isArray(cfg.models) || cfg.models.length === 0) {
        pushInfo(raw, "", "config.models must be a non-empty array");
        return;
      }
      const turnIdx = turns.length;
      setTurns((t) => [
        ...t,
        {
          kind: "info",
          user: raw,
          body: `🟢 P4 Eval → ${cfg.tasks.length} task(s) × ${cfg.models.length} model(s) starting…`,
          inFlight: true,
        },
      ]);
      try {
        const resp = await client.runEval({
          tasks: cfg.tasks.map((t) => ({
            id: t.id,
            spec: t.spec,
            expected_topic: t.expected_topic ?? null,
            models: t.models ?? null,
          })),
          models: cfg.models,
          skip_embedding: cfg.skip_embedding ?? false,
          skip_judge: cfg.skip_judge ?? false,
        });
        const lines: string[] = [
          `🟢 P4 Eval ✅ · ${resp.n_tasks} task(s) × ${resp.n_models} model(s) · $${resp.total_cost_usd.toFixed(4)} · ${Number(resp.total_duration_ms) / 1000}s`,
          "",
          "model".padEnd(28) + "n   judge   faith   pass    cost".padEnd(40),
        ];
        for (const s of resp.summary_by_model) {
          const judge = s.avg_judge_score != null ? s.avg_judge_score.toFixed(2) : "  -";
          const faith = s.avg_faithfulness != null ? s.avg_faithfulness.toFixed(2) : "  -";
          lines.push(
            `${s.model.padEnd(28)}${String(s.n_samples).padEnd(4)}${judge.padEnd(8)}${faith.padEnd(8)}${(s.pass_rate * 100).toFixed(0).padStart(3)}%   $${s.total_cost_usd.toFixed(4)}`,
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
        const msg = e instanceof CoreError ? e.message : String(e);
        setTurns((t) => {
          const next = [...t];
          const cur = next[turnIdx];
          if (cur && cur.kind === "info") {
            next[turnIdx] = { ...cur, error: msg, inFlight: false };
          }
          return next;
        });
      }
    },
    [client, turns.length, pushInfo],
  );

  // -- /gates <sdd-run-prefix> ------------------------------------------------

  const handleGates = useCallback(
    async (raw: string) => {
      const prefix = raw.slice("/gates".length).trim();
      if (!prefix) {
        pushInfo(raw, "", "usage: /gates <sdd-run-prefix>  (reuses spec/design from .kira/runs/<id>/)");
        return;
      }
      let run: Awaited<ReturnType<typeof client.getSdd>>;
      try {
        const list = await client.listSdd();
        const match = list.runs.find((r: { id: string }) => r.id.startsWith(prefix));
        if (!match) {
          pushInfo(raw, "", `no sdd run with prefix '${prefix}'`);
          return;
        }
        run = await client.getSdd(match.id);
      } catch (e) {
        pushInfo(raw, "", String(e));
        return;
      }
      if (!run.spec_path || !run.design_path) {
        pushInfo(raw, "", `run ${run.id.slice(0, 8)} missing spec_path or design_path; gates need both`);
        return;
      }
      let spec: string;
      let output: string;
      try {
        spec = await readFile(run.spec_path, "utf-8");
        output = await readFile(run.design_path, "utf-8");
      } catch (e) {
        pushInfo(raw, "", `read artifacts failed: ${e}`);
        return;
      }
      const turnIdx = turns.length;
      setTurns((t) => [
        ...t,
        {
          kind: "info",
          user: raw,
          body: `🟢 P4 Gates → run ${run.id.slice(0, 8)} (spec=${spec.length}B output=${output.length}B)…`,
          inFlight: true,
        },
      ]);
      try {
        const resp = await client.runGates({
          spec,
          output,
          owned_paths: [],
          touched_paths: [],
          estimated_loc: null,
          actual_loc: null,
          skip_embedding: false,
          skip_judge: false,
          haystack: output,
          skip_meta_review: false,
          worktree_path: run.worktree_path,
          skip_structural_lint: true,
          writer_model: null,
        });
        const lines: string[] = [
          `🟢 P4 Gates · run ${run.id.slice(0, 8)} · overall=${resp.overall}${resp.failed_at ? ` (failed_at=${resp.failed_at})` : ""} · ${Number(resp.duration_ms)}ms`,
          "",
        ];
        for (const r of resp.results) {
          const score = r.score != null ? r.score.toFixed(2).padStart(5) : "   - ";
          lines.push(`  ${r.outcome.padEnd(8)} ${r.gate.padEnd(22)} score=${score}  ${r.reason ?? ""}`);
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
        const msg = e instanceof CoreError ? e.message : String(e);
        setTurns((t) => {
          const next = [...t];
          const cur = next[turnIdx];
          if (cur && cur.kind === "info") {
            next[turnIdx] = { ...cur, error: msg, inFlight: false };
          }
          return next;
        });
      }
    },
    [client, turns.length, pushInfo],
  );

  // -- /worktrees, /ownership, /mcp, /stats — read-only observability ---------

  const handleWorktrees = useCallback(async () => {
    try {
      const all = await client.listWorktrees();
      if (all.length === 0) {
        pushInfo("/worktrees", "(no worktrees)");
        return;
      }
      const body = all
        .map(
          (w) =>
            `${w.id.slice(0, 8)} ${w.status.padEnd(10)} ${(w.agent_kind ?? "-").padEnd(9)} ${w.branch.padEnd(28)} ${w.path}`,
        )
        .join("\n");
      pushInfo("/worktrees", body);
    } catch (e) {
      pushInfo("/worktrees", "", String(e));
    }
  }, [client, pushInfo]);

  const handleOwnership = useCallback(
    async (raw: string) => {
      const rest = raw.slice("/ownership".length).trim();
      // /ownership check agent=<id> path=<p>
      if (rest.startsWith("check")) {
        const flags: Record<string, string> = {};
        for (const tok of rest.slice("check".length).trim().split(/\s+/)) {
          if (tok.includes("=")) {
            const [k, v] = tok.split("=", 2);
            flags[k] = v;
          }
        }
        if (!flags.agent || !flags.path) {
          pushInfo(raw, "", "usage: /ownership check agent=<id> path=<path>");
          return;
        }
        try {
          const d = await client.checkOwnership(flags.agent, flags.path);
          if (d.decision === "allowed") {
            pushInfo(raw, `allowed · agent=${flags.agent.slice(0, 8)} path=${flags.path}`);
          } else {
            pushInfo(
              raw,
              `denied · reason=${d.reason}${d.conflicting_agent ? ` conflicting=${d.conflicting_agent.slice(0, 8)}` : ""}`,
            );
          }
        } catch (e) {
          pushInfo(raw, "", String(e));
        }
        return;
      }
      // /ownership — list
      try {
        const all = await client.listOwnership();
        if (all.length === 0) {
          pushInfo("/ownership", "(no claims)");
          return;
        }
        const body = all
          .map(
            (c) =>
              `${c.agent_id.slice(0, 8)} ${c.mode.padEnd(10)} patterns=[${c.patterns.join(",")}] since=${c.created_at}`,
          )
          .join("\n");
        pushInfo("/ownership", body);
      } catch (e) {
        pushInfo("/ownership", "", String(e));
      }
    },
    [client, pushInfo],
  );

  const handleMcp = useCallback(async () => {
    try {
      const r = await client.mcpServers();
      if (r.servers.length === 0) {
        pushInfo("/mcp", `(no MCP servers connected) · total_tools=${r.total_tools}`);
        return;
      }
      const lines: string[] = [`MCP servers=${r.servers.length} · total_tools=${r.total_tools}`];
      for (const s of r.servers) {
        lines.push(`  ${s.name} (${s.tools.length} tools)`);
        for (const t of s.tools.slice(0, 8)) {
          lines.push(`    · ${t.namespaced_name.padEnd(36)} [${t.risk}] ${t.description.slice(0, 60)}`);
        }
        if (s.tools.length > 8) lines.push(`    … +${s.tools.length - 8} more`);
      }
      pushInfo("/mcp", lines.join("\n"));
    } catch (e) {
      pushInfo("/mcp", "", String(e));
    }
  }, [client, pushInfo]);

  const handleStats = useCallback(async () => {
    try {
      const s = await client.stats();
      const lines = [
        `usd_spent=$${s.usd_spent.toFixed(4)} · agents total=${s.total_agents} active=${s.active_agents} completed=${s.completed_agents} finished=${s.finished_agents}`,
      ];
      if (s.model_usage.length > 0) {
        lines.push("");
        lines.push("model".padEnd(28) + "reqs   tokens_in    tokens_out");
        for (const m of s.model_usage) {
          lines.push(
            `${m.model.padEnd(28)}${String(m.requests).padEnd(7)}${String(m.tokens_in).padStart(10)}    ${String(m.tokens_out).padStart(10)}`,
          );
        }
      }
      pushInfo("/stats", lines.join("\n"));
    } catch (e) {
      pushInfo("/stats", "", String(e));
    }
  }, [client, pushInfo]);

  // -- /events [run_id] — tail pillar events; /events stop --------------------

  const handleEvents = useCallback(
    async (raw: string) => {
      const rest = raw.slice("/events".length).trim();
      if (rest === "stop") {
        if (eventsCtrl.current) {
          eventsCtrl.current.abort();
          eventsCtrl.current = null;
          pushInfo(raw, "events stream stopped");
        } else {
          pushInfo(raw, "(no active events stream)");
        }
        return;
      }
      if (eventsCtrl.current) {
        pushInfo(raw, "", "stream already active — use /events stop first");
        return;
      }
      const runId = rest || undefined;
      const turnIdx = turns.length;
      setTurns((t) => [
        ...t,
        {
          kind: "info",
          user: raw,
          body: `📡 events${runId ? ` run=${runId.slice(0, 8)}` : " (all)"} — tailing… /events stop to end`,
          inFlight: true,
        },
      ]);
      const lines: string[] = [
        `📡 events${runId ? ` run=${runId.slice(0, 8)}` : " (all)"} — tailing… /events stop to end`,
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
      eventsCtrl.current = client.streamPillarEvents(
        runId,
        (ev) => {
          const { emoji, abbr } = pillarSigil(ev.pillar);
          const runTag = ev.run_id ? ` ${ev.run_id.slice(0, 8)}` : "";
          lines.push(`${emoji} ${abbr}${runTag} [${ev.phase}] ${ev.message}`);
          flush();
        },
        (e) => {
          lines.push(`(stream error: ${e})`);
          flush();
        },
      );
    },
    [client, turns.length, pushInfo, pillarSigil],
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
          // Fast-path response (H6, 2026-05-25): backend resolved without
          // spawning an agent — `agent_id` empty, `kind` is "chat", `message`
          // holds the model output. Render inline and skip the poller.
          const isFastPath =
            !yolo.agent_id && yolo.kind === "chat" && yolo.status === "completed";
          if (isFastPath) {
            setTurns((t) => {
              const next = [...t];
              const cur = next[turnIdx];
              if (cur && cur.kind === "agent") {
                next[turnIdx] = {
                  ...cur,
                  status: "completed",
                  inFlight: false,
                  result: yolo.message,
                };
              }
              return next;
            });
          } else {
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
          }
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

  // Save Tama state on graceful exit — between-session survival handled by loadCompanionState
  const tuckInTama = useCallback(() => {
    saveCompanionState(companionPersistedRef.current);
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

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
        tuckInTama();
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
      if (text.startsWith("/review")) {
        await handleReview(text);
        return;
      }
      if (text.startsWith("/eval")) {
        await handleEvalFile(text);
        return;
      }
      if (text.startsWith("/gates")) {
        await handleGates(text);
        return;
      }
      if (text === "/worktrees") {
        await handleWorktrees();
        return;
      }
      if (text === "/ownership" || text.startsWith("/ownership ")) {
        await handleOwnership(text);
        return;
      }
      if (text === "/mcp") {
        await handleMcp();
        return;
      }
      if (text === "/stats") {
        await handleStats();
        return;
      }
      if (text === "/events" || text.startsWith("/events ")) {
        await handleEvents(text);
        return;
      }

      // Companion commands (English + Portuguese aliases)
      const companionCmd = text.toLowerCase();
      const tamaCmds = ["/feed", "/play", "/clean", "/medicate", "/sleep", "/wake", "/revive",
                        "/alimentar", "/brincar", "/limpar", "/medicar", "/dormir", "/acordar", "/reviver"];
      if (tamaCmds.includes(companionCmd)) {
        const actionMap: Record<string, TamaAction> = {
          "/feed": "feed", "/play": "play", "/clean": "clean",
          "/medicate": "medicate", "/sleep": "sleep", "/wake": "wake",
          "/revive": "revive",
          "/alimentar": "feed", "/brincar": "play", "/limpar": "clean",
          "/medicar": "medicate", "/dormir": "sleep", "/acordar": "wake",
          "/reviver": "revive",
        };
        const action = actionMap[companionCmd]!;
        const prev = companionPersistedRef.current;
        const tama = { ...prev.tama };
        const msg = dispatchAction(tama, action);
        if (msg) {
          const next = { ...prev, tama };
          companionPersistedRef.current = next;
          setCompanionState(next);
          setCompanionWidgetMessage(msg);
          saveCompanionState(next);
          pushInfo(text, msg);
        } else {
          pushInfo(text, "nothing happens");
        }
        return;
      }
      if (companionCmd === "/companion" || companionCmd.startsWith("/companion ")) {
        const arg = companionCmd.slice("/companion".length).trim();
        if (arg === "on") {
          const next = { ...companionPersistedRef.current, hidden: false };
          companionPersistedRef.current = next;
          setCompanionState(next);
          saveCompanionState(next);
          pushInfo(text, "companion visible");
        } else if (arg === "off") {
          const next = { ...companionPersistedRef.current, hidden: true };
          companionPersistedRef.current = next;
          setCompanionState(next);
          saveCompanionState(next);
          pushInfo(text, "companion hidden");
        } else if (arg === "toggle") {
          const next = { ...companionPersistedRef.current, hidden: !companionPersistedRef.current.hidden };
          companionPersistedRef.current = next;
          setCompanionState(next);
          saveCompanionState(next);
          pushInfo(text, next.hidden ? "companion hidden" : "companion visible");
        } else if (!arg) {
          const soul = companionState.companion;
          if (soul) {
            pushInfo(text, `${soul.name}: ${soul.personality} | XP: ${companionState.xp.total} | Hunger: ${Math.round(companionState.tama.hunger)} | Happiness: ${Math.round(companionState.tama.happiness)}`);
          } else {
            pushInfo(text, "No companion yet.");
          }
        }
        return;
      }

      // Pomodoro commands
      if (text === "/pomodoro start") {
        const prev = companionPersistedRef.current;
        if (prev.pomodoro.phase !== "idle" && prev.pomodoro.phase !== "short_break" && prev.pomodoro.phase !== "long_break") {
          pushInfo(text, "already in focus");
          return;
        }
        const now = Date.now();
        const nextPomo = { ...prev.pomodoro, phase: "focus" as const, startedAt: now, elapsed: 0 };
        const next = { ...prev, pomodoro: nextPomo };
        companionPersistedRef.current = next;
        setCompanionState(next);
        saveCompanionState(next);
        const mins = nextPomo.focusDuration / 60000;
        pushInfo(text, `focus started (${mins}:00)`);
        return;
      }
      if (text === "/pomodoro stop") {
        const prev = companionPersistedRef.current;
        if (prev.pomodoro.phase === "idle") {
          pushInfo(text, "no active session");
          return;
        }
        const nextPomo = { ...prev.pomodoro, phase: "idle" as const, startedAt: null, elapsed: 0 };
        const next = { ...prev, pomodoro: nextPomo };
        companionPersistedRef.current = next;
        setCompanionState(next);
        saveCompanionState(next);
        pushInfo(text, "pomodoro stopped");
        return;
      }
      if (text === "/pomodoro pause") {
        const prev = companionPersistedRef.current;
        if (prev.pomodoro.phase === "idle" || !prev.pomodoro.startedAt) {
          pushInfo(text, "nothing to pause");
          return;
        }
        const elapsed = prev.pomodoro.elapsed + (Date.now() - prev.pomodoro.startedAt);
        const nextPomo = { ...prev.pomodoro, startedAt: null, elapsed };
        const next = { ...prev, pomodoro: nextPomo };
        companionPersistedRef.current = next;
        setCompanionState(next);
        saveCompanionState(next);
        pushInfo(text, "pomodoro paused");
        return;
      }
      if (text === "/pomodoro resume") {
        const prev = companionPersistedRef.current;
        if (prev.pomodoro.phase === "idle" || prev.pomodoro.startedAt) {
          pushInfo(text, prev.pomodoro.startedAt ? "already running" : "no active session");
          return;
        }
        const nextPomo = { ...prev.pomodoro, startedAt: Date.now() };
        const next = { ...prev, pomodoro: nextPomo };
        companionPersistedRef.current = next;
        setCompanionState(next);
        saveCompanionState(next);
        pushInfo(text, "pomodoro resumed");
        return;
      }
      if (text === "/pomodoro status") {
        const p = companionState.pomodoro;
        const phase = p.phase === "idle" ? "idle" : `${p.phase} (${Math.floor(p.elapsed / 60000)}m)`;
        pushInfo(text, `pomodoro: ${phase} | completed: ${p.completed} | durations: ${p.focusDuration / 60000}m focus / ${p.shortBreakDuration / 60000}m break`);
        return;
      }
      if (text.startsWith("/pomodoro config ")) {
        const rest = text.slice("/pomodoro config ".length).trim();
        const parts = rest.split(/\s+/).map(Number);
        if (parts.length >= 2 && parts[0] > 0 && parts[1] > 0) {
          const prev = companionPersistedRef.current;
          const nextPomo = {
            ...prev.pomodoro,
            focusDuration: parts[0] * 60 * 1000,
            shortBreakDuration: parts[1] * 60 * 1000,
            longBreakDuration: Math.min(parts[1] * 3, 30) * 60 * 1000,
          };
          const next = { ...prev, pomodoro: nextPomo };
          companionPersistedRef.current = next;
          setCompanionState(next);
          saveCompanionState(next);
          setDecayFocusDuration(nextPomo.focusDuration);
          pushInfo(text, `pomodoro configured: ${parts[0]}m focus / ${parts[1]}m break`);
        } else {
          pushInfo(text, "", "usage: /pomodoro config <focusMin> <breakMin>");
        }
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
      handleReview,
      handleEvalFile,
      handleGates,
      handleWorktrees,
      handleOwnership,
      handleMcp,
      handleStats,
      handleEvents,
      handleFreeText,
      setThemeMode,
      themeMode,
      companionState,
    ],
  );

  // -- ThinkingDisplay state machine -----------------------------------------

  const prevActiveAgents = useRef(0);
  const prevFailedCount = useRef(0);

  const activeAgentTurns = useMemo(
    () => turns.filter((t): t is Extract<Turn, { kind: "agent" }> => t.kind === "agent" && t.inFlight),
    [turns],
  );

  const failedAgentTurns = useMemo(
    () => turns.filter((t) => t.kind === "agent" && t.status === "failed"),
    [turns],
  );

  const subAgents: SubAgentInfo[] = useMemo(
    () =>
      activeAgentTurns.map((t) => ({
        agentId: t.agentId ?? "unknown",
        kind: t.agentKind,
        status: t.status ?? "active",
        activity: t.user,
        iterations: t.iters ?? 0,
        elapsed: t.startedAt ? Math.floor((Date.now() - t.startedAt) / 1000) : 0,
      })),
    [activeAgentTurns],
  );

  // Category transitions
  useEffect(() => {
    const now = activeAgentTurns.length;
    const prev = prevActiveAgents.current;
    prevActiveAgents.current = now;

    if (now > 0) {
      // Update failed count while agents are active (for comparison later)
      prevFailedCount.current = failedAgentTurns.length;

      const allSpawning = activeAgentTurns.every(
        (t) => t.status === "spawning" || t.status === undefined,
      );
      if (allSpawning) {
        setThinkingCategory("spawning");
      } else {
        setThinkingCategory("thinking");
      }
    } else if (prev > 0) {
      // Just transitioned from active → idle
      setThinkingCategory(
        failedAgentTurns.length > prevFailedCount.current
          ? "failure"
          : "success",
      );
    }
  }, [activeAgentTurns, failedAgentTurns.length]); // eslint-disable-line react-hooks/exhaustive-deps

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
    const tama = tamaEmotion(companionState.tama);
    if (tama !== "neutral" && tama !== "happy") return tama;
    if (failedTurns > 0) return "sad";
    if (activeAgents > 0) return "surprised";
    if (completedTurns > 0) return "happy";
    return tama; // "neutral" or "happy" from tama
  }, [activeAgents, completedTurns, failedTurns, companionState.tama]);
  const bannerRowColors = useMemo(
    () => interpolatePalette(colors.logoGradientK, GOODBYE_BANNER.length),
    [colors.logoGradientK],
  );
  const termWidth = stdout?.columns ?? 80;
  // Format wall time
  function formatTime(ms: number): string {
    const secs = Math.floor(ms / 1000);
    const mins = Math.floor(secs / 60);
    const hrs = Math.floor(mins / 60);
    if (hrs > 0) return `${hrs}h ${mins % 60}m ${secs % 60}s`;
    if (mins > 0) return `${mins}m ${secs % 60}s`;
    return `${secs}s`;
  }

  const totalTurns = turnCount.current;
  const totalAgents = turns.filter((t) => t.kind === "agent").length;
  const completedAgents = turns.filter(
    (t) => t.kind === "agent" && !t.inFlight,
  ).length;
  const totalCommands = commandCount.current;

  // Show goodbye summary
  if (showGoodbye) {
    const wallTime = formatTime(Date.now() - sessionStart.current);
    const sessionId = `kira-${sessionStart.current.toString(36).slice(0, 12)}`;
    const stats = goodbyeStats;
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

    const tierColor =
      tier === "heavy" ? "#E879A8" : tier === "light" ? "#4ADE80" : "#87CEEB";

    const bannerWidth = Math.max(...GOODBYE_BANNER.map((l) => l.length));
    const cardWidth = Math.min(termWidth - 4, Math.max(bannerWidth + 6, 66));
    const leftPad = Math.max(0, Math.floor((termWidth - cardWidth) / 2));

    const row = (items: { label: string; value: string; color?: string }[]) => (
      <Text>
        {items.map((it, i) => (
          <React.Fragment key={i}>
            {i > 0 && <Text color="#444444">{`   `}</Text>}
            <Text color="#777777">{it.label} </Text>
            <Text color={it.color ?? "#E8E8E8"}>{it.value}</Text>
          </React.Fragment>
        ))}
      </Text>
    );

    return (
      <Box flexDirection="column" paddingX={1} marginTop={1} marginLeft={leftPad}>
        <Box
          flexDirection="column"
          borderStyle="round"
          borderColor="#A855F7"
          width={cardWidth}
          paddingX={1}
          paddingY={0}
        >
          {/* Banner */}
          <Box flexDirection="column" paddingX={1} paddingTop={1} paddingBottom={0}>
            {GOODBYE_BANNER.map((line, i) => (
              <Text key={i} color={bannerRowColors[i] ?? colors.logoGradientK[0]}>{line}</Text>
            ))}
          </Box>

          <Box paddingX={1} paddingTop={0} paddingBottom={0}>
            <Text> </Text>
          </Box>

          {/* Session */}
          <Box paddingX={1} paddingTop={0} paddingBottom={0}>
            <Text color="#888888" dimColor>{sessionId}</Text>
          </Box>

          <Box paddingX={1} paddingTop={0} paddingBottom={0}>
            {row([
              { label: "time", value: wallTime },
              { label: "turns", value: String(totalTurns) },
              { label: "agents", value: String(totalAgents) },
            ])}
          </Box>

          {totalCommands > 0 && (
            <Box paddingX={1} paddingTop={0} paddingBottom={0}>
              {row([
                { label: "commands", value: String(totalCommands) },
                { label: "completed", value: `${successfulAgents}/${totalAgents}` },
                { label: "rate", value: `${successRate}%` },
              ])}
            </Box>
          )}

          {/* Model Usage */}
          {stats && stats.model_usage.length > 0 && (
            <>
              <Box paddingX={1} paddingTop={1} paddingBottom={0}>
                <Text bold color="#A855F7">{"\u25C8"} MODELS</Text>
              </Box>
              {stats.model_usage.map((m) => (
                <Box key={m.model} paddingX={1} paddingTop={0} paddingBottom={0}>
                  <Text color="#C8C8C8">
                    <Text color="#777777">{m.model.padEnd(22)}</Text>
                    <Text color="#666666">{`${String(m.requests).padStart(3)} req  `}</Text>
                    <Text color="#666666">{`${fmtNum(Number(m.tokens_in)).padStart(9)} \u2192 `}</Text>
                    <Text color="#666666">{fmtNum(Number(m.tokens_out)).padStart(9)}</Text>
                  </Text>
                </Box>
              ))}
              <Box paddingX={1} paddingTop={0} paddingBottom={0}>
                <Text color="#666666" dimColor>
                  <Text color="#777777">{"total".padEnd(22)}</Text>
                  {`${fmtNum(totalTokensIn).padStart(12)} \u2192 ${fmtNum(totalTokensOut).padStart(9)}`}
                </Text>
              </Box>
            </>
          )}

          {/* Cost & Tier */}
          <Box paddingX={1} paddingTop={1} paddingBottom={0}>
            <Text bold color="#A855F7">{"\u25C8"} BILL</Text>
          </Box>
          <Box paddingX={1} paddingTop={0} paddingBottom={0}>
            {row([
              { label: "tier", value: tier.toUpperCase(), color: tierColor },
              ...(stats ? [{ label: "usd", value: `$${stats.usd_spent.toFixed(4)}`, color: costColor(stats.usd_spent) }] : []),
            ])}
          </Box>

          {/* Companion */}
          <Box paddingX={1} paddingTop={1} paddingBottom={0}>
            <Text bold color="#A855F7">{"\u25C8"} COMPANION</Text>
          </Box>
          <Box paddingX={1} paddingTop={0} paddingBottom={1}>
            {companionState.companion ? (
              <Box alignItems="center">
                <Box marginRight={2}>
                  <Mascot config={companionConfig} emotion="sleepy" />
                </Box>
                <Box flexDirection="column">
                  <Text bold color="#9CA3AF">{companionState.companion.name} <Text dimColor color="#6B7280">zzz</Text></Text>
                  <Text dimColor color="#6B7280">will wake on next write</Text>
                </Box>
              </Box>
            ) : (
              <Box marginLeft={2}>
                <Text dimColor color="#6B7280">companion sleeps until next write</Text>
              </Box>
            )}
          </Box>

          {/* Restart */}
          <Box paddingX={1} paddingBottom={1}>
            <Text color="#444444" dimColor>restart: bun run src/cli/kira.tsx</Text>
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

  const scrollableContent = (
    <>
      {thinkingCategory !== "idle" && (
        <ThinkingDisplay
          category={thinkingCategory}
          agents={subAgents}
          totalTokens={totalSessionTokens}
          totalAgents={totalAgents}
          sessionStartedAt={sessionStart.current}
          stalled={thinkingStalled}
          onComplete={() => setThinkingCategory("idle")}
        />
      )}
      {turns.map((t, i) => (
        <TurnRenderer key={i} turn={t} />
      ))}
      <ToastContainer toasts={toasts} />
      {pendingApproval && (
        <ApprovalPrompt
          agentId={pendingApproval.agentId}
          agentKind={pendingApproval.agentKind}
          approval={pendingApproval.approval}
          onDecide={handleApprovalDecide}
        />
      )}
      {pendingSddQuestions && (
        <PlanQuestions
          sddRunId={pendingSddQuestions.sddRunId}
          questions={pendingSddQuestions.questions}
          onSubmit={handleSddQuestionSubmit}
          onCancel={() => setPendingSddQuestions(null)}
        />
      )}
    </>
  );

  const bottomContent = (
    <AnimationClockProvider>
      <PomodoroProvider
        state={companionState.pomodoro}
        onTick={handlePomodoroOnTick}
      >
        <InputBar
          mode={mode}
          disabled={showHelp || pendingApproval !== null || pendingSddQuestions !== null}
          onSubmit={send}
          onFirstCtrlC={() => setShowExitHint(true)}
          onDoubleCtrlC={() => { tuckInTama(); setShowGoodbye(true); }}
        />
        <StatusBar
          tier={tier}
          mode={mode}
          agentCount={totalAgents}
          activeAgents={activeAgents}
          themeMode={themeMode}
          showExitHint={showExitHint}
          onDismissExitHint={() => setShowExitHint(false)}
          newMessageCount={completedTurns}
        />
      </PomodoroProvider>
    </AnimationClockProvider>
  );

  const headerContent = (
    <Header
      coreUp={coreUp}
      coreVersion={coreVersion}
      tier={tier}
      mode={mode}
      workingDir={workingDir}
      rightSlot={
        companionState.companion && !companionState.hidden ? (
          <BuddyChat
            mascotConfig={companionConfig}
            companionSoul={companionState.companion}
            message={companionWidgetMessage}
            tamaState={companionState.tama}
            level={xpToLevel(companionState.xp.total).level}
            emotion={mascotEmotion}
            onMessageExpired={() => setCompanionWidgetMessage(null)}
          />
        ) : undefined
      }
    />
  );

  return (
    <AlternateScreen mouseTracking={false}>
      <FullscreenLayout
        header={headerContent}
        scrollable={scrollableContent}
        bottom={bottomContent}
        terminalRows={terminalRows}
      />
    </AlternateScreen>
  );
}
