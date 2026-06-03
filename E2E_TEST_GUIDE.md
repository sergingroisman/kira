# E2E Manual Testing Guide — Kira Harness

> **Purpose**: Verify all REPL features work correctly after the UX overhaul
> **Tester**: Sergin (dev/owner)
> **Last Updated**: May 22, 2026

## Prerequisites

### Terminal Setup

You need **two terminals** open:

**Terminal 1 — Core Daemon:**
```bash
cd /Users/avanade/Workspace/kira/core
cargo run
```
Expected output: `Listening on 127.0.0.1:7777`

**Terminal 2 — REPL:**
```bash
cd /Users/avanade/Workspace/kira/agent
bun run src/cli/kira.tsx
```
Expected output: Kira welcome screen with logo, mode/tier info, and input prompt

---

## Test 1: Splash Screen + Welcome Flow

| Step | Action | Expected Result | Status |
|------|--------|-----------------|--------|
| 1.1 | Start REPL with `bun run src/cli/kira.tsx` | Classic block banner (monochrome gradient, all letters same color, no red "R") | ☐ |
| 1.2 | Check header layout | Logo on left, mode/tier/dir info on right, separator line spans full width | ☐ |
| 1.3 | Check status bar | Shows `mode=PLAN · tier=standard · agents=0` at bottom | ☐ |
| 1.4 | Press Enter (empty input) | No crash, input prompt remains active | ☐ |

---

## Test 2: Theme System

| Step | Action | Expected Result | Status |
|------|--------|-----------------|--------|
| 2.1 | Type `/theme light` | Theme switches to light mode, status bar updates to `theme=light` | ☐ |
| 2.2 | Type `/theme dark` | Theme switches back to dark mode, status bar updates | ☐ |
| 2.3 | Type `/theme dark-ansi` | Theme switches to 16-color ANSI mode | ☐ |
| 2.4 | Check invalid theme `/theme invalid` | Error message: `unknown theme 'invalid'. Valid: dark, light, dark-ansi`, theme unchanged | ☐ |

---

## Test 3: Core Health (Connect/Disconnect/Reconnect)

| Step | Action | Expected Result | Status |
|------|--------|-----------------|--------|
| 3.1 | Start REPL with core running | Connection established, ready for input | ☐ |
| 3.2 | Kill core (Ctrl+C in Terminal 1) | REPL shows "Kira Core disconnected" | ☐ |
| 3.3 | Type any command while disconnected | Error shown, REPL stays active | ☐ |
| 3.4 | Restart core in Terminal 1 | REPL auto-reconnects within 3 seconds | ☐ |
| 3.5 | Check reconnection | Input works again, no restart needed | ☐ |

---

## Test 4: Chat Streaming + Tier Switching

| Step | Action | Expected Result | Status |
|------|--------|-----------------|--------|
| 4.1 | Type: `what is rust?` | Response streams in chunks, not all at once | ☐ |
| 4.2 | Check tier indicator | Status bar shows `tier=standard` during streaming | ☐ |
| 4.3 | Type `/tier heavy` then `summarize async/await in rust` | Response uses Heavy tier (deepseek-v4-pro or qwen-max) | ☐ |
| 4.4 | Type `/tier light` then `what is cargo?` | Response uses Light tier (deepseek-v4-flash) | ☐ |
| 4.5 | Type `/tier standard` | Tier reverts to Standard, confirmed in status bar | ☐ |

---

## Test 5: Agent Spawn, Polling, Cancel

| Step | Action | Expected Result | Status |
|------|--------|-----------------|--------|
| 5.1 | Type `/agent writer "create a short README.md"` | Agent spawns, shows "Agent running...", worktree created | ☐ |
| 5.2 | Wait for completion | Agent finishes, shows result summary | ☐ |
| 5.3 | Type `/agents` | Shows agent status: completed, with worktree path | ☐ |
| 5.4 | Type `/cancel <prefix>` | Agent cancels, worktree cleaned up, status shows "cancelled" | ☐ |

---

## Test 6: Plan Draft + Approve Workflow

| Step | Action | Expected Result | Status |
|------|--------|-----------------|--------|
| 6.1 | Type `/mode plan` | Mode switches, status bar shows `mode=PLAN` | ☐ |
| 6.2 | Type `/plan draft writer "add type safety to config loader"` | Plan generated with steps | ☐ |
| 6.3 | Review plan output | Shows plan ID, steps, drafted status | ☐ |
| 6.4 | Type `/plan approve <id>` | Agent spawns to execute the plan | ☐ |
| 6.5 | Wait for completion | All planned changes applied | ☐ |
| 6.6 | Type `/mode yolo` | Switches back to direct execution mode, status bar shows `mode=YOLO` | ☐ |
| 6.7 | Type free text in YOLO | Agent spawns directly (no plan) | ☐ |
| 6.8 | Type free text in PLAN | Auto-drafts a plan | ☐ |

---

## Test 7: Explorer Agent (Research)

| Step | Action | Expected Result | Status |
|------|--------|-----------------|--------|
| 7.1 | Type `/agent explorer "find best practices for rust error handling"` | Explorer spawns, shows search query, Tavily results | ☐ |
| 7.2 | Check inline results | Search results formatted inline with sources cited | ☐ |
| 7.3 | Check analysis | Qwen provides synthesized answer based on search results | ☐ |
| 7.4 | Verify routing matches configured tier | Check core logs for `provider configured` lines + per-call `via=<provider>` matches active `[tiers.*]` candidates | ☐ |

---

## Test 8: Help System

| Step | Action | Expected Result | Status |
|------|--------|-----------------|--------|
| 8.1 | Type `/help` | Help menu shows all commands categorized (Core, Agents, Plans, Lessons, Review & Eval, Settings) | ☐ |
| 8.2 | Check help header | Shows `⚡ Kira Help` in purple | ☐ |
| 8.3 | Check footer hint | Shows "Free text → auto-drafts a plan in PLAN mode, spawns agent in YOLO mode" | ☐ |
| 8.4 | Exit help with Escape or Enter | Returns to input prompt | ☐ |
| 8.5 | Type `?` in normal text | Does NOT trigger help (e.g. "what is rust?") | ☐ |

---

## Test 9: Command History

| Step | Action | Expected Result | Status |
|------|--------|-----------------|--------|
| 9.1 | Type 3 commands (any) | Commands stored in history | ☐ |
| 9.2 | Press Up arrow | Previous command appears in input | ☐ |
| 9.3 | Press Down arrow | Next command appears, or empty if at end | ☐ |
| 9.4 | Check history persists across theme changes | History not lost when switching themes | ☐ |

---

## Test 10: Goodbye Session Summary

| Step | Action | Expected Result | Status |
|------|--------|-----------------|--------|
| 10.1 | Type `/quit` or press Ctrl+C twice | Goodbye box appears with purple border and title | ☐ |
| 10.2 | Check "Interaction Summary" section | Shows Session ID, Wall Time, Total Turns, Commands, Agents Spawned/Completed, Success Rate | ☐ |
| 10.3 | Check "Performance" section | Shows API Calls, Model Tier (colored), USD Spent | ☐ |
| 10.4 | Check "Model Usage" section (if agents ran) | Table with Model, Reqs, Input tokens, Output tokens + Total row | ☐ |
| 10.5 | Check restart hint | Shows "Restart: bun run src/cli/kira.tsx" | ☐ |

---

## Test 11: Error Handling

| Step | Action | Expected Result | Status |
|------|--------|-----------------|--------|
| 11.1 | Type invalid command `/invalidcmd` | Treated as free text (PLAN mode: auto-drafts plan, YOLO mode: spawns agent) | ☐ |
| 11.2 | Type `/agent invalid "test"` | Error: unknown agent kind, REPL stays active | ☐ |
| 11.3 | Disconnect core mid-stream | Graceful error, no crash | ☐ |
| 11.4 | Type empty `/tier` | Error: unknown tier, usage hint shown | ☐ |

---

## Test 12: Management Commands

| Step | Action | Expected Result | Status |
|------|--------|-----------------|--------|
| 12.1 | Type `/health` | Shows core version and uptime | ☐ |
| 12.2 | Type `/agents` (with no agents) | Shows empty or "(no agents)" | ☐ |
| 12.3 | Spawn agent, then `/agents` | Shows agent with status, worktree, tier, cost | ☐ |
| 12.4 | Type `/lessons` | Shows learned lessons (empty or populated) | ☐ |
| 12.5 | Type `/plans` | Shows all plans with status | ☐ |
| 12.6 | Type `/review <head>` | Runs TRUST-style PR review | ☐ |
| 12.7 | Type `/gates run` | Runs 6-level gate cascade | ☐ |
| 12.8 | Type `/eval run` | Runs model comparison eval | ☐ |

---

## Test 13: Safety Hooks

| Step | Action | Expected Result | Status |
|------|--------|-----------------|--------|
| 13.1 | In agent task, attempt `rm -rf /` | BLOCKED by PreToolUse hook, error shown | ☐ |
| 13.2 | In agent task, attempt path traversal `../../../etc/passwd` | REJECTED by path validation | ☐ |
| 13.3 | Check core logs | Hook invocations logged with timestamps | ☐ |

---

## Test 14: State Persistence

| Step | Action | Expected Result | Status |
|------|--------|-----------------|--------|
| 14.1 | Complete a few agent tasks | State saved to `~/.local/share/kira/state.redb` | ☐ |
| 14.2 | Exit REPL (`/quit`) and restart | Previous agents still visible via `/agents` | ☐ |
| 14.3 | Check audit logs | JSONL files appended in audit directory | ☐ |
| 14.4 | Check worktrees | `.worktrees/` contains worktrees from completed agents | ☐ |

---

## Results Summary

| Test Category | Passed | Failed | Blocked |
|---------------|--------|--------|---------|
| Splash + Welcome | /4 | /4 | /4 |
| Theme System | /4 | /4 | /4 |
| Core Health | /5 | /5 | /5 |
| Chat + Tier | /5 | /5 | /5 |
| Agent Lifecycle | /4 | /4 | /4 |
| Plan Workflow | /8 | /8 | /8 |
| Explorer Agent | /4 | /4 | /4 |
| Help System | /5 | /5 | /5 |
| Command History | /4 | /4 | /4 |
| Goodbye Summary | /5 | /5 | /5 |
| Error Handling | /4 | /4 | /4 |
| Management Commands | /8 | /8 | /8 |
| Safety Hooks | /3 | /3 | /3 |
| State Persistence | /4 | /4 | /4 |
| **TOTAL** | **/67** | **/67** | **/67** |

---

## Quick Smoke Test (5 minutes)

If you're short on time, run these essential checks:

```bash
# Terminal 1
cd core && cargo run

# Terminal 2
cd agent && bun run src/cli/kira.tsx

# In REPL, run these:
/help                      # Help menu works
/mode plan                 # Mode switch works
/plan draft writer "test"  # Plan generation works
/tier heavy                # Tier switch works
what is rust?              # Free text auto-drafts plan (PLAN mode)
/agents                    # Agent listing works
/quit                      # Goodbye summary shows stats
```

If all 7 work → **Core functionality OK**, proceed with full test suite later.

---

## Notes for Sergin

- Fill in the Status column (☐ = pending, ✅ = passed, ❌ = failed, ⚠️ = blocked)
- For failed tests, note the error in a comment below each section
- Screenshots of failures are helpful for debugging
- Run this guide after any significant REPL changes
- Keep this file updated as new features are added
