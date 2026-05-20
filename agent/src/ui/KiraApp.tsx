/**
 * KiraApp — slim Ink REPL talking to the Rust core daemon.
 *
 * v0 capabilities:
 *   - User types a prompt → POST /v1/chat/stream → tokens stream in
 *   - Conversation history maintained in component state
 *   - /health and /quit slash commands
 *   - Tier selector (default: standard)
 *
 * No LangGraph, no local tools. Tools dispatch comes when sub-agents land
 * (Phase 2). For now REPL is a faithful streaming chat client.
 */

import React, { useState, useEffect, useCallback } from "react";
import { Box, Text, useApp, useInput } from "ink";
import { CoreClient, CoreError, type ChatMessage } from "../core-client/client.js";

const TIERS = ["heavy", "standard", "light"] as const;
type Tier = (typeof TIERS)[number];

interface Turn {
  user: string;
  assistant: string;
  model?: string;
  inFlight: boolean;
  error?: string;
}

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

  // Health probe on mount.
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

  const send = useCallback(
    async (raw: string) => {
      const text = raw.trim();
      if (!text) return;

      // Slash commands
      if (text === "/quit" || text === "/exit") {
        exit();
        return;
      }
      if (text === "/health") {
        try {
          const h = await client.health();
          setTurns((t) => [
            ...t,
            {
              user: "/health",
              assistant: `core ${h.version} • uptime ${h.uptime_secs}s`,
              inFlight: false,
            },
          ]);
        } catch (e) {
          setTurns((t) => [
            ...t,
            { user: "/health", assistant: "", error: String(e), inFlight: false },
          ]);
        }
        return;
      }
      if (text.startsWith("/tier ")) {
        const next = text.slice(6).trim() as Tier;
        if ((TIERS as readonly string[]).includes(next)) {
          setTier(next);
          setTurns((t) => [
            ...t,
            { user: text, assistant: `tier → ${next}`, inFlight: false },
          ]);
        } else {
          setTurns((t) => [
            ...t,
            {
              user: text,
              assistant: "",
              error: `unknown tier '${next}'. Valid: ${TIERS.join(", ")}`,
              inFlight: false,
            },
          ]);
        }
        return;
      }

      // Build messages from history (each turn = user+assistant pair).
      const messages: ChatMessage[] = [];
      for (const t of turns) {
        messages.push({ role: "user", content: t.user });
        if (t.assistant) messages.push({ role: "assistant", content: t.assistant });
      }
      messages.push({ role: "user", content: text });

      const turnIdx = turns.length;
      setTurns((t) => [...t, { user: text, assistant: "", inFlight: true }]);

      try {
        for await (const ev of client.chatStream({ tier, model: null, messages })) {
          if (ev.type === "token") {
            setTurns((t) => {
              const next = [...t];
              const cur = next[turnIdx];
              if (cur) {
                next[turnIdx] = { ...cur, assistant: cur.assistant + ev.delta };
              }
              return next;
            });
          } else if (ev.type === "done") {
            setTurns((t) => {
              const next = [...t];
              const cur = next[turnIdx];
              if (cur) {
                next[turnIdx] = { ...cur, model: ev.model, inFlight: false };
              }
              return next;
            });
          } else if (ev.type === "error") {
            setTurns((t) => {
              const next = [...t];
              const cur = next[turnIdx];
              if (cur) {
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
          if (cur) {
            next[turnIdx] = { ...cur, error: msg, inFlight: false };
          }
          return next;
        });
      }
    },
    [client, tier, turns, exit],
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
          /health · /tier {"<heavy|standard|light>"} · /quit · Ctrl+C
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
