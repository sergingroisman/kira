import React, { useMemo, useState, useEffect } from "react";
import { Box, Text } from "../../ink/kira-ink.js";
import { useTheme } from "./ThemeProvider.js";
import { md2ansi } from "../markdown.js";
import type { Attachment } from "../attachment-parser.js";

const costColor = (usd: number): string => {
  if (usd === 0) return "#666666";
  if (usd < 0.01) return "#4ADE80";
  if (usd < 0.10) return "#FBBF24";
  if (usd < 1.00) return "#F97316";
  return "#EF4444";
};

interface BaseTurn {
  user: string;
  inFlight: boolean;
  error?: string;
  startedAt?: number;
  tokenCount?: number;
}

interface ChatTurn extends BaseTurn {
  kind: "chat";
  assistant: string;
  model?: string;
  attachments?: Attachment[];
}

interface AgentTurn extends BaseTurn {
  kind: "agent";
  agentId?: string;
  agentKind: string;
  status?: string;
  iters?: number;
  usd?: number;
  model?: string;
  result?: string;
  yolo?: boolean;
  attachments?: Attachment[];
  diff?: string;
}

interface PlanTurn extends BaseTurn {
  kind: "plan";
  planId?: string;
  planStatus?: string;
  goal?: string;
  agentKind?: string;
  scope?: string;
  steps?: { idx: number; description: string; tool?: string | null }[];
  executorAgentId?: string;
  note?: string;
}

interface InfoTurn extends BaseTurn {
  kind: "info";
  body: string;
}

export type Turn = ChatTurn | AgentTurn | PlanTurn | InfoTurn;

const ICONS = {
  chat: "\u2726",
  agent: "\u26A1",
  plan: "\u25C6",
  info: "\u2139",
  error: "\u2717",
} as const;

const COLORS = {
  chat: "#2E7D32",
  agent: "#87CEEB",
  plan: "#B1B9F9",
  info: "#777777",
  error: "#DC143C",
} as const;

const AGENT_KIND_COLORS: Record<string, string> = {
  writer: "#2090F5",
  deployer: "#30D158",
  judge: "#A753AE",
};

const STATUS_COLORS: Record<string, string> = {
  completed: "#2E7D32",
  done: "#2E7D32",
  failed: "#DC143C",
  cancelled: "#B8860B",
  expired: "#B8860B",
  spawning: "#87CEEB",
  active: "#87CEEB",
  stale: "#555555",
  stopped: "#555555",
  draft: "#555555",
  executing: "#87CEEB",
  approving: "#B8860B",
};

function useElapsed(startedAt?: number): string {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (!startedAt) return;
    const interval = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(interval);
  }, [startedAt]);
  if (!startedAt) return "";
  const elapsed = Date.now() - startedAt;
  const secs = Math.floor(elapsed / 1000);
  const mins = Math.floor(secs / 60);
  const hrs = Math.floor(mins / 60);
  if (hrs > 0) return `${hrs}h ${mins % 60}m`;
  if (mins > 0) return `${mins}m ${secs % 60}s`;
  return `${secs}s`;
}

export const TurnRenderer: React.FC<{ turn: Turn }> = ({ turn }) => {
  if (turn.kind === "plan") return <PlanTurnView turn={turn} />;
  if (turn.kind === "agent") return <AgentTurnView turn={turn} />;
  if (turn.kind === "info") return <InfoTurnView turn={turn} />;
  if (turn.kind === "chat") return <ChatTurnView turn={turn} />;
  return null;
};

function ChatTurnView({ turn }: { turn: ChatTurn }) {
  return (
    <Box flexDirection="column" marginBottom={1}>
      <Box>
        <Text color="#555555">{"\u25B8"} </Text>
        <Text color="#C8C8C8">{turn.user}</Text>
      </Box>
      {turn.attachments && turn.attachments.length > 0 && (
        <Box marginLeft={2} flexDirection="column">
          {turn.attachments.map((att, i) => (
            <Text key={i} color="#555555" dimColor>
              {att.type === "file"
                ? `Read ${att.path} (${att.numLines} lines${att.truncated ? ", truncated" : ""})`
                : `Read ${att.path} (${(att.size / 1024).toFixed(1)} KB)`}
            </Text>
          ))}
        </Box>
      )}
      {turn.error ? (
        <Box marginLeft={2}>
          <Text color={COLORS.error}>{ICONS.error} </Text>
          <Text color={COLORS.error}>{turn.error}</Text>
        </Box>
      ) : (
        <Box marginLeft={2}>
          <Text color={COLORS.chat}>{ICONS.chat} </Text>
          <Text color="#C8C8C8">{turn.assistant}</Text>
          {turn.inFlight && <Text color="#B8860B">{" \u258D"}</Text>}
          {turn.model ? <Text color="#555555"> ({turn.model})</Text> : null}
        </Box>
      )}
    </Box>
  );
}

function AgentTurnView({ turn }: { turn: AgentTurn }) {
  const { colors } = useTheme();
  const statusColor = STATUS_COLORS[turn.status ?? ""] ?? "#555555";
  const modeBadge = turn.yolo ? "YOLO" : null;
  const elapsed = useElapsed(turn.startedAt);

  return (
    <Box flexDirection="column" marginBottom={1}>
      <Box>
        <Text color="#555555">{"\u25B8"} </Text>
        <Text color="#C8C8C8">{turn.user}</Text>
      </Box>
      {turn.attachments && turn.attachments.length > 0 && (
        <Box marginLeft={2} flexDirection="column">
          {turn.attachments.map((att, i) => (
            <Text key={i} color="#555555" dimColor>
              {att.type === "file"
                ? `Read ${att.path} (${att.numLines} lines${att.truncated ? ", truncated" : ""})`
                : `Read ${att.path} (${(att.size / 1024).toFixed(1)} KB)`}
            </Text>
          ))}
        </Box>
      )}
      {turn.error ? (
        <Box marginLeft={2}>
          <Text color={COLORS.error}>{ICONS.error} </Text>
          <Text color={COLORS.error}>{turn.error}</Text>
        </Box>
      ) : (
        <>
          <Box marginLeft={2}>
            <Text color={AGENT_KIND_COLORS[turn.agentKind] ?? COLORS.agent}>{ICONS.agent} </Text>
            <Text color={AGENT_KIND_COLORS[turn.agentKind] ?? "#777777"}>
              {turn.agentKind} {turn.agentId?.slice(0, 8) ?? "\u2026"}{" "}
            </Text>
            {turn.inFlight ? (
              <>
                <Text dimColor color={colors.textTertiary}>
                  {turn.status ?? "processing"}…
                </Text>
                <Text dimColor color={colors.textTertiary}>
                  {" "}{elapsed}
                </Text>
                {turn.tokenCount ? (
                  <Text dimColor color={colors.textTertiary}>
                    {" "}{(turn.tokenCount / 1000).toFixed(1)}k tkns
                  </Text>
                ) : null}
                <Text color="#B8860B">{" \u258D"}</Text>
              </>
            ) : (
              <>
                <Text color={statusColor}>
                  {turn.status ?? "spawning"}
                </Text>
                {modeBadge && (
                  <Text color="#DC143C" bold>
                    {" "}[{modeBadge}]
                  </Text>
                )}
              </>
            )}
            {!turn.inFlight && (
              <Text>
                <Text color="#555555">
                  {" iters="}{turn.iters ?? 0}
                </Text>
                <Text color={costColor(turn.usd ?? 0)}>
                  {" $"}
                  {(turn.usd ?? 0).toFixed(4)}
                </Text>
              </Text>
            )}
            {turn.model ? <Text color="#555555"> ({turn.model})</Text> : null}
          </Box>
          {turn.result && !turn.inFlight ? (
            <Box marginLeft={4}>
              <Text color="#C8C8C8">{md2ansi(turn.result)}</Text>
            </Box>
          ) : null}

          {/* Worktree diff — generated files */}
          {turn.diff && !turn.inFlight ? (
            <Box flexDirection="column" marginLeft={4} marginTop={1}>
              {turn.diff.split("\n").map((line, i) => {
                let color = "#C8C8C8";
                if (line.startsWith("+")) color = "#4ADE80";
                else if (line.startsWith("-")) color = "#EF4444";
                else if (line.startsWith("@@")) color = "#60A5FA";
                else if (line.startsWith("diff --git")) color = "#FBBF24";
                else if (line.startsWith("---") || line.startsWith("+++")) color = "#555555";
                return (
                  <Text key={i} color={color}>
                    {line}
                  </Text>
                );
              })}
            </Box>
          ) : null}
        </>
      )}
    </Box>
  );
}

function PlanTurnView({ turn }: { turn: PlanTurn }) {
  const statusColor = STATUS_COLORS[turn.planStatus ?? ""] ?? "#87CEEB";
  const scopeColor =
    turn.scope === "quick"
      ? "#2E7D32"
      : turn.scope === "small"
        ? "#87CEEB"
        : turn.scope === "medium"
          ? "#B8860B"
          : turn.scope === "large"
            ? "#B1B9F9"
            : turn.scope === "complex"
              ? "#DC143C"
              : "#555555";

  return (
    <Box flexDirection="column" marginBottom={1}>
      <Box>
        <Text color="#555555">{"\u25B8"} </Text>
        <Text color="#C8C8C8">{turn.user}</Text>
      </Box>
      {turn.error ? (
        <Box marginLeft={2}>
          <Text color={COLORS.error}>{ICONS.error} </Text>
          <Text color={COLORS.error}>{turn.error}</Text>
        </Box>
      ) : (
        <>
          <Box marginLeft={2}>
            <Text color={COLORS.plan}>{ICONS.plan} </Text>
            <Text color="#777777">
              plan {turn.planId?.slice(0, 8) ?? "\u2026"}{" "}
            </Text>
            <Text color={statusColor}>{turn.planStatus ?? "draft"}</Text>
            {turn.scope ? (
              <Text color={scopeColor}> [{turn.scope}]</Text>
            ) : null}
            {turn.executorAgentId ? (
              <Text color="#555555"> exec={turn.executorAgentId.slice(0, 8)}</Text>
            ) : null}
            {turn.inFlight ? <Text color="#B8860B">{" \u258D"}</Text> : null}
          </Box>
          {turn.note ? (
            <Box marginLeft={4}>
              <Text color="#555555">{turn.note}</Text>
            </Box>
          ) : null}
          {turn.steps && turn.steps.length > 0 ? (
            <Box marginLeft={4} flexDirection="column">
              {turn.steps.map((s) => (
                <Box key={s.idx}>
                  <Text color="#555555">{s.idx}. </Text>
                  <Text color="#C8C8C8">{s.description}</Text>
                  {s.tool ? (
                    <Text color="#555555"> [{s.tool}]</Text>
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

function InfoTurnView({ turn }: { turn: InfoTurn }) {
  return (
    <Box flexDirection="column" marginBottom={1}>
      <Box>
        <Text color="#555555">{"\u25B8"} </Text>
        <Text color="#C8C8C8">{turn.user}</Text>
      </Box>
      {turn.error ? (
        <Box marginLeft={2}>
          <Text color={COLORS.error}>{ICONS.error} </Text>
          <Text color={COLORS.error}>{turn.error}</Text>
        </Box>
      ) : (
        <Box marginLeft={2}>
          <Text color={COLORS.info}>{ICONS.info} </Text>
          <Text color="#C8C8C8">{turn.body}</Text>
          {turn.inFlight ? <Text color="#B8860B">{" \u258D"}</Text> : null}
        </Box>
      )}
    </Box>
  );
}

function truncate(s: string, max: number): string {
  if (s.length <= max) return s;
  return s.slice(0, max) + "\u2026";
}
