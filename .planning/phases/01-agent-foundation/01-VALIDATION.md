---
phase: 1
slug: agent-foundation
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-05-09
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Vitest |
| **Config file** | `agent/vitest.config.ts` — Wave 0 installs |
| **Quick run command** | `npm test --workspace agent -- --run` |
| **Full suite command** | `npm test --workspace agent` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `npm test --workspace agent -- --run`
- **After every plan wave:** Run `npm test --workspace agent`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** ~15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 1-bash | TBD | 1 | AGENT-02 | — | N/A | unit | `npm test --workspace agent -- --run tests/tools/bash.test.ts` | ❌ W0 | ⬜ pending |
| 1-file | TBD | 1 | AGENT-03 | — | N/A | unit | `npm test --workspace agent -- --run tests/tools/file.test.ts` | ❌ W0 | ⬜ pending |
| 1-search | TBD | 1 | AGENT-04 | — | API key via env var only | integration | `npm test --workspace agent -- --run tests/tools/search.test.ts` | ❌ W0 | ⬜ pending |
| 1-reflect | TBD | 2 | AGENT-05 | — | N/A | unit | `npm test --workspace agent -- --run tests/graph/reflect.test.ts` | ❌ W0 | ⬜ pending |
| 1-memory | TBD | 2 | AGENT-07 | — | N/A | integration | `npm test --workspace agent -- --run tests/graph/memory.test.ts` | ❌ W0 | ⬜ pending |
| 1-cli-boot | TBD | 1 | AGENT-01 | — | N/A | smoke | `tsx agent/src/cli/index.ts --help 2>&1; echo $?` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `agent/vitest.config.ts` — test framework config
- [ ] `agent/tests/tools/bash.test.ts` — stubs for AGENT-02
- [ ] `agent/tests/tools/file.test.ts` — stubs for AGENT-03
- [ ] `agent/tests/tools/search.test.ts` — stubs for AGENT-04 (requires TAVILY_API_KEY in env)
- [ ] `agent/tests/graph/reflect.test.ts` — stubs for AGENT-05
- [ ] `agent/tests/graph/memory.test.ts` — stubs for AGENT-07
- [ ] `agent/.env.example` — documents DEEPSEEK_API_KEY, TAVILY_API_KEY

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Ink streaming renders tokens live in terminal | AGENT-01 | Visual rendering cannot be asserted programmatically | Run `sergin`, type a prompt, observe tokens stream character-by-character |
| CLI REPL loop stays open after response | AGENT-01 | Process lifecycle / interactive loop | After response, type another prompt — agent should respond again without restart |
| Conversation references prior turn | AGENT-07 | End-to-end session feel | Turn 1: "My name is Sergio." Turn 2: "What's my name?" — expect "Sergio" in response |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
