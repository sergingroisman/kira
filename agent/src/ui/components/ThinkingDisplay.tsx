import React, { useEffect, useState, useRef, useMemo, useCallback } from "react";
import { Box, Text } from "../../ink/kira-ink.js";
import { interpolateColor } from "../art.js";
import { ShimmerText } from "./ShimmerText.js";
import { useAnimationClock } from "./AnimationClock.js";
import quotes from "../../data/thinking-quotes.json" with { type: "json" };

// ─── Anim constants ─────────────────────────────────────────────────

const SPINNER_CHARS = ["\u25C8", "\u2726", "\u2727", "\u2B17", "\u2B16", "\u25C7"]; // ◈ ✦ ✧ ⬗ ⬖ ◇
const SPINNER_FRAMES = [...SPINNER_CHARS, ...[...SPINNER_CHARS].reverse()];
const SPINNER_MS = 120;
const QUOTE_CYCLE_MIN_MS = 10_000;
const QUOTE_CYCLE_MAX_MS = 12_000;
const SHIMMER_MS = 200;
const STALL_THRESHOLD_MS = 3_000;
const STALL_FADE_MS = 2_000;
const COMPLETION_HOLD_MS = 2_500;
const BELL_CHAR = "\x07";

const KIRA_PURPLE = "#A855F7";
const KIRA_PURPLE_LIGHT = "#C084FC";
const STALL_RED = "#EF4444";
const QUOTE_COLOR = "#9CA3AF";
const STATUS_COLOR = "#6B7280";
const TREE_COLOR = "#4B5563";

const AGENT_COLORS: Record<string, string> = {
  writer: "#2090F5",
  deployer: "#30D158",
  judge: "#A753AE",
  explorer: "#E8963E",
};

// ─── Types ──────────────────────────────────────────────────────────

export interface SubAgentInfo {
  agentId: string;
  kind: string;
  status: string;
  activity: string;
  iterations: number;
  elapsed: number;
}

export type ThinkingCategory = "thinking" | "success" | "failure" | "spawning" | "idle";

export interface ThinkingDisplayProps {
  category: ThinkingCategory;
  agents: SubAgentInfo[];
  totalTokens?: number;
  totalAgents: number;
  sessionStartedAt: number;
  stalled?: boolean;
  /** Called when the completion hold expires (success/failure → idle). */
  onComplete?: () => void;
}

interface Quote {
  text: string;
  author: string;
  source: string;
}

// ─── Helpers ────────────────────────────────────────────────────────

let quoteIdx = Math.floor(Math.random() * 999);

function pickQuote(category: ThinkingCategory): Quote {
  const pool = quotes[category] as Quote[] | undefined;
  if (!pool || pool.length === 0) {
    const fallback = quotes.thinking as Quote[];
    return fallback[quoteIdx % fallback.length]!;
  }
  quoteIdx = (quoteIdx + 1) % pool.length;
  return pool[quoteIdx]!;
}

function fmtTime(ms: number): string {
  const secs = Math.floor(ms / 1000);
  const mins = Math.floor(secs / 60);
  const hrs = Math.floor(mins / 60);
  if (hrs > 0) return `${hrs}h ${mins % 60}m`;
  if (mins > 0) return `${mins}m ${secs % 60}s`;
  return `${secs}s`;
}

function fmtTokens(n: number): string {
  if (n < 1000) return String(n);
  if (n < 1_000_000) return `${(n / 1000).toFixed(1)}k`;
  return `${(n / 1_000_000).toFixed(1)}M`;
}

function treeChar(idx: number, total: number): string {
  if (total === 1) return "\u2514\u2500"; // └─
  if (idx === total - 1) return "\u2514\u2500"; // └─
  if (idx === 0) return "\u255E\u2550"; // ╞═
  return "\u251C\u2500"; // ├─
}

// ─── Component ──────────────────────────────────────────────────────

export const ThinkingDisplay: React.FC<ThinkingDisplayProps> = ({
  category,
  agents,
  totalTokens = 0,
  totalAgents,
  sessionStartedAt,
  stalled = false,
  onComplete,
}) => {
  const { paused } = useAnimationClock();
  const [spinnerFrame, setSpinnerFrame] = useState(0);
  const [currentQuote, setCurrentQuote] = useState<Quote>(() => pickQuote(category));
  const [stallIntensity, setStallIntensity] = useState(0);
  const [elapsed, setElapsed] = useState(0);
  const [completionPhase, setCompletionPhase] = useState<"active" | "holding" | "done">("active");
  const [displayTokens, setDisplayTokens] = useState(0);
  const tokenEaseRef = useRef(0);

  const quoteTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const spinnerTimer = useRef<ReturnType<typeof setInterval> | null>(null);
  const elapsedTimer = useRef<ReturnType<typeof setInterval> | null>(null);
  const completionTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const prevCategory = useRef<ThinkingCategory>(category);

  // ── Spinner animation ──────────────────────────────────────────────
  useEffect(() => {
    if (paused) return;
    spinnerTimer.current = setInterval(() => {
      setSpinnerFrame((f) => (f + 1) % SPINNER_FRAMES.length);
    }, SPINNER_MS);
    return () => {
      if (spinnerTimer.current) clearInterval(spinnerTimer.current);
    };
  }, [paused]);

  // ── Quote cycling ──────────────────────────────────────────────────
  useEffect(() => {
    if (category !== "thinking" && category !== "spawning") return;
    if (paused) return;

    function cycle() {
      setCurrentQuote(pickQuote(category));
      const delay =
        QUOTE_CYCLE_MIN_MS +
        Math.floor(Math.random() * (QUOTE_CYCLE_MAX_MS - QUOTE_CYCLE_MIN_MS));
      quoteTimer.current = setTimeout(cycle, delay);
    }

    const delay =
      QUOTE_CYCLE_MIN_MS +
      Math.floor(Math.random() * (QUOTE_CYCLE_MAX_MS - QUOTE_CYCLE_MIN_MS));
    quoteTimer.current = setTimeout(cycle, delay);

    return () => {
      if (quoteTimer.current) clearTimeout(quoteTimer.current);
    };
  }, [category, paused]);

  // ── Elapsed time ───────────────────────────────────────────────────
  useEffect(() => {
    if (paused) return;
    const tick = () => setElapsed(Date.now() - sessionStartedAt);
    tick();
    elapsedTimer.current = setInterval(tick, 1000);
    return () => {
      if (elapsedTimer.current) clearInterval(elapsedTimer.current);
    };
  }, [sessionStartedAt, paused]);

  // ── Stall intensity lerp ───────────────────────────────────────────
  useEffect(() => {
    if (!stalled) {
      setStallIntensity(0);
      return;
    }
    const startTime = Date.now();
    const timer = setInterval(() => {
      const elapsed = Date.now() - startTime;
      const target = Math.min(1, elapsed / STALL_FADE_MS);
      setStallIntensity(prev => prev + (target - prev) * 0.1);
    }, 50);
    return () => clearInterval(timer);
  }, [stalled]);

  // ── Category change → pick quote + bell + completion hold ──────────
  useEffect(() => {
    const prev = prevCategory.current;
    prevCategory.current = category;

    if (category !== prev) {
      setCurrentQuote(pickQuote(category));

      // Bell on success/failure
      if (category === "success") {
        process.stderr.write(BELL_CHAR);
      } else if (category === "failure") {
        process.stderr.write(BELL_CHAR);
        setTimeout(() => process.stderr.write(BELL_CHAR), 300);
      }

      // Completion hold
      if (category === "success" || category === "failure") {
        setCompletionPhase("holding");
        completionTimer.current = setTimeout(() => {
          setCompletionPhase("done");
          onComplete?.();
        }, COMPLETION_HOLD_MS);
      } else {
        setCompletionPhase("active");
        if (completionTimer.current) {
          clearTimeout(completionTimer.current);
          completionTimer.current = null;
        }
      }
    }

    return () => {
      if (completionTimer.current) clearTimeout(completionTimer.current);
    };
  }, [category, onComplete]);

  // ── Token counter easing ───────────────────────────────────────────
  useEffect(() => {
    if (totalTokens < tokenEaseRef.current) {
      tokenEaseRef.current = totalTokens;
      setDisplayTokens(totalTokens);
    }
    if (paused || totalTokens === tokenEaseRef.current) return;
    const timer = setInterval(() => {
      const gap = totalTokens - tokenEaseRef.current;
      if (gap <= 0) { clearInterval(timer); return; }
      let inc: number;
      if (gap < 100) inc = 3;
      else if (gap < 600) inc = Math.max(8, Math.ceil(gap * 0.1));
      else inc = 100;
      tokenEaseRef.current = Math.min(tokenEaseRef.current + inc, totalTokens);
      setDisplayTokens(tokenEaseRef.current);
    }, 50);
    return () => clearInterval(timer);
  }, [totalTokens, paused]);

  // ── Compute spinner color ──────────────────────────────────────────
  const spinnerColor = useMemo(() => {
    if (stallIntensity > 0) {
      return interpolateColor(KIRA_PURPLE, STALL_RED, stallIntensity);
    }
    return KIRA_PURPLE;
  }, [stallIntensity]);

  const spinnerChar = SPINNER_FRAMES[spinnerFrame]!;
  const showTree = agents.length >= 2;
  const quoteLine = `"${currentQuote.text}" \u2014 ${currentQuote.author}`;

  // ── Render ─────────────────────────────────────────────────────────
  return (
    <Box flexDirection="column" marginBottom={1}>
      {/* Line 1: spinner + quote + status */}
      <Box>
        <Text color={spinnerColor}>{spinnerChar}</Text>
        <Text> </Text>
        <Box flexShrink={1}>
          <ShimmerText
            text={quoteLine}
            baseColor={QUOTE_COLOR}
            shimmerColor={KIRA_PURPLE_LIGHT}
            intervalMs={SHIMMER_MS}
            direction="right-to-left"
            paused={paused}
          />
        </Box>
        <Text>  </Text>
        <Text color={STATUS_COLOR}>
          {"\u26A1"}{totalAgents} agents{" "}
          {totalTokens > 0 ? `\u00B7 ${fmtTokens(displayTokens)} tokens` : ""}
          {" \u00B7 "}{fmtTime(elapsed)}
        </Text>
      </Box>

      {/* Lines 2+: sub-agent tree */}
      {showTree && (
        <Box flexDirection="column" marginLeft={1}>
          {agents.map((a, i) => (
            <Box key={a.agentId}>
              <Text color={TREE_COLOR}>{treeChar(i, agents.length)} </Text>
              <Text color={AGENT_COLORS[a.kind] ?? KIRA_PURPLE}>
                {"\u26A1"} {a.kind} {a.agentId.slice(0, 8)}
              </Text>
              <Text color={STATUS_COLOR}>
                {" "}{a.activity.slice(0, 42)}{a.activity.length > 42 ? "\u2026" : ""}
                {" \u00B7 "}{a.iterations} iters
                {" \u00B7 "}{fmtTime(a.elapsed * 1000)}
              </Text>
            </Box>
          ))}
        </Box>
      )}
    </Box>
  );
};

export default ThinkingDisplay;
