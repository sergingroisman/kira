/**
 * HelpMenu — formatted help panel with categorized commands.
 *
 * Triggered by /help or Ctrl+H shortcut.
 * Categories: Core, Agents, Plans, Lessons, Review, Settings
 */
import React, { useCallback, useState } from "react";
import { Box, Text, useInput } from "../../ink/kira-ink.js";

interface HelpCategory {
  title: string;
  color: string;
  commands: { cmd: string; desc: string }[];
}

const CATEGORIES: HelpCategory[] = [
  {
    title: "Core",
    color: "#87CEEB",
    commands: [
      { cmd: "/quit, /exit", desc: "Exit Kira REPL" },
      { cmd: "/health", desc: "Check core daemon health" },
      { cmd: "/mode <plan|yolo>", desc: "Switch workflow mode" },
      { cmd: "/tier <heavy|standard|light>", desc: "Switch model tier" },
    ],
  },
  {
    title: "Conversation",
    color: "#E879A8",
    commands: [
      { cmd: "(free text)", desc: "PLAN mode drafts a plan; YOLO mode spawns writer agent" },
      { cmd: "/agent <kind> <task>", desc: "Spawn an agent (writer/deployer/judge/explorer)" },
      { cmd: "/agents", desc: "List all agents with status and cost" },
      { cmd: "/cancel <prefix>", desc: "Cancel a running agent by ID prefix" },
      { cmd: "/plan draft <kind> <goal>", desc: "Draft a step-by-step plan" },
      { cmd: "/plan approve <prefix>", desc: "Approve a plan and spawn executor" },
      { cmd: "/plan cancel <prefix>", desc: "Cancel a plan" },
      { cmd: "/plans", desc: "List all plans" },
    ],
  },
  {
    title: "Workflows",
    color: "#B1B9F9",
    commands: [
      { cmd: "/sdd [kind=...] [loc=N] <goal>", desc: "P1 Spec-Driven Execution (4-phase pipeline)" },
      { cmd: "/sdds", desc: "List SDD runs" },
      { cmd: "/review <head> [base] [timeout=N]", desc: "P3 TRUST-style PR review" },
      { cmd: "/eval <path-to-config.json>", desc: "P4 Eval: compare N models × M tasks from JSON file" },
      { cmd: "/gates <sdd-prefix>", desc: "P4 Gates cascade replayed on an SDD run's spec/design" },
    ],
  },
  {
    title: "Observability",
    color: "#2E7D32",
    commands: [
      { cmd: "/events [run_id]", desc: "Tail pillar events (P1–P7). /events stop to end." },
      { cmd: "/stats", desc: "Budget, agent counts, model usage" },
      { cmd: "/worktrees", desc: "List active git worktrees" },
      { cmd: "/ownership [check agent=.. path=..]", desc: "List ownership claims or check write decision" },
      { cmd: "/mcp", desc: "List connected MCP servers and tools" },
      { cmd: "/lessons", desc: "List active lessons" },
      { cmd: "/lessons promote <prefix>", desc: "Promote a lesson to AGENTS-memory" },
      { cmd: "/lessons delete <prefix>", desc: "Delete a lesson" },
      { cmd: "/search <query>", desc: "Sovereign SearXNG search" },
      { cmd: "/fetch <url>", desc: "Fetch URL as markdown" },
    ],
  },
  {
    title: "Companion",
    color: "#F97316",
    commands: [
      { cmd: "/companion", desc: "Show companion status" },
      { cmd: "/companion on|off|toggle", desc: "Show or hide your companion" },
      { cmd: "/feed", desc: "Feed your companion (+hunger)" },
      { cmd: "/play", desc: "Play with your companion (+happiness)" },
      { cmd: "/clean", desc: "Clean up poop piles" },
      { cmd: "/medicate", desc: "Cure sickness" },
      { cmd: "/sleep", desc: "Put companion to sleep (pauses decay)" },
      { cmd: "/wake", desc: "Wake companion up" },
      { cmd: "/revive", desc: "Revive a dead companion (penalty stats)" },
      { cmd: "F/C/P/M/S", desc: "Quick keys planned: Feed/Clean/Play/Medicate/Sleep" },
      { cmd: "/pomodoro start", desc: "Start focus session" },
      { cmd: "/pomodoro stop", desc: "Stop current session" },
      { cmd: "/pomodoro pause", desc: "Pause timer" },
      { cmd: "/pomodoro resume", desc: "Resume timer" },
      { cmd: "/pomodoro status", desc: "Show pomodoro status" },
      { cmd: "/pomodoro config <focusMin> <breakMin>", desc: "Configure durations" },
    ],
  },
  {
    title: "Appearance",
    color: "#4ECDC4",
    commands: [
      { cmd: "/theme <dark|light|...>", desc: "Switch color theme" },
      { cmd: "/motion <on|off>", desc: "Toggle animations" },
    ],
  },
];

export interface HelpMenuProps {
  onDismiss: () => void;
}

export const HelpMenu: React.FC<HelpMenuProps> = ({ onDismiss }) => {
  useInput(
    useCallback(
      (_ch, key) => {
        if (key.return || key.escape || key.tab) {
          onDismiss();
        }
      },
      [onDismiss],
    ),
    { isActive: true },
  );

  return (
    <Box flexDirection="column">
      <Box>
        <Text bold color="#A855F7">
          {" "}
          ⚡ Kira Help
        </Text>
        <Text color="#555555"> — press Enter/Esc/Ctrl+H to close</Text>
      </Box>
      <Box marginTop={1}>
        <Text color="#333333">{"─".repeat(56)}</Text>
      </Box>
      {CATEGORIES.map((cat) => (
        <Box key={cat.title} flexDirection="column" marginTop={1}>
          <Box>
            <Text bold color={cat.color}>
              {" "}
              {cat.title}
            </Text>
          </Box>
          {cat.commands.map((c) => (
            <Box key={c.cmd} paddingLeft={2}>
              <Text color={cat.color}>
                {c.cmd.padEnd(30)}
              </Text>
              <Text color="#666666">{c.desc}</Text>
            </Box>
          ))}
        </Box>
      ))}
      <Box marginTop={1}>
        <Text color="#333333">{"─".repeat(56)}</Text>
      </Box>
      <Box marginTop={1}>
        <Text color="#666666">Free text → auto-drafts a plan in PLAN mode, spawns agent in YOLO mode</Text>
      </Box>
    </Box>
  );
};

export default HelpMenu;
