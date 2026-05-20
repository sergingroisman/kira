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
} from "../core-client/client.js";

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

type Turn = ChatTurn | AgentTurn | InfoTurn;

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
    [client, tier, turns, exit, pushInfo, handleAgentCmd, handleAgentsList, handleCancel],
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
          chat · /agent {"<kind> <task>"} · /agents · /cancel {"<id>"} · /health
          · /tier · /quit
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
    <Box flexDirection="column" marginBottom={1}>
      <Text bold color="cyan">
        Kira REPL
      </Text>
      <Text dimColor>
        <Text color={color}>{status}</Text> · tier={tier}
      </Text>
    </Box>
  );
}

function TurnView({ turn }: { turn: Turn }) {
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
