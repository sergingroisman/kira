# Kira Harness - Quick Start Guide

## What is Kira?

Kira is your **personal agentic harness** — an AI agent system that can spawn autonomous agents to complete tasks, write code, and manage worktrees. Think of it as having AI assistants that work in isolated git worktrees.

## Architecture Overview

Kira has two main components that run simultaneously:

1. **Core Daemon (Rust)** — Runs on `http://127.0.0.1:7777`, handles agent spawning, model routing, state management
2. **REPL (TypeScript + Bun + Ink)** — Your terminal interface to interact with the core

## How to Start Kira

### Step 1: Start the Core Daemon

```bash
cd /Users/avanade/Workspace/kira/core
cargo run
```

You should see something like:
```
Listening on 127.0.0.1:7777
```

**Keep this terminal running!** The core must stay up for the REPL to work.

### Step 2: Start the REPL (in a new terminal)

```bash
cd /Users/avanade/Workspace/kira/agent
bun install  # only needed first time
bun run src/cli/index.tsx
```

You'll see the Kira REPL interface with your screenshot's command menu.

## Using the REPL Commands

### Interactive Mode (Default)

Just type naturally! The REPL will route your request to the appropriate agent:

```
gera artigo sobre Rust agents
```
This spawns a `writer` agent to generate an article.

### Slash Commands

| Command | Description | Example |
|---------|-------------|---------|
| `/mode <plan\|yolo>` | Switch between cautious planning mode or direct execution | `/mode yolo` |
| `/tier <h\|s\|l>` | Switch model tier (heavy/standard/light) | `/tier h` |
| `/agent <kind> <task>` | Manually spawn an agent | `/agent writer "create README.md"` |
| `/plan draft <kind> <goal>` | Draft a plan without executing | `/plan draft writer "add tests"` |
| `/plan approve <id>` | Approve and execute a drafted plan | `/plan approve abc123` |
| `/agents` | List active agents | `/agents` |
| `/lessons` | View/manage learned lessons | `/lessons` |
| `/quit` | Exit the REPL | `/quit` |

### Agent Kinds

- **writer** — Generates content (docs, code, articles). Uses Standard tier by default.
- **deployer** — Handles deployment scripts and dist output. Uses Light tier.
- **judge** — Evaluates output quality. Uses Heavy tier. **Never** same model as writer (avoids self-enhancement bias).

## Testing the Intelligence

### 1. Simple Test: Generate Documentation

```
gera documentação para a função router
```

This will:
1. Spawn a `writer` agent in a git worktree
2. Analyze the router code
3. Generate documentation
4. Commit the result

### 2. Test with Explicit Agent Spawn

```
/agent writer "criar testes unitários para o módulo de configuração"
```

### 3. Test Plan Mode (Cautious)

```
/mode plan
/plan draft writer "refatorar o módulo de hooks"
```

Review the plan, then approve:
```
/plan approve <id-from-output>
```

### 4. Test Different Model Tiers

```
/tier h   # Heavy (deepseek-v4-pro or qwen-max)
gera análise de performance do sistema

/tier l   # Light (deepseek-v4-flash or qwen-flash)
gera resumo rápido das mudanças recentes
```

### 5. Check Agent Status

```
/agents
```

Shows running agents, their worktrees, and status.

## Understanding the Workflow

```
User Input → REPL → Core Daemon → Router → LLM (provider-agnostic via tiers) → Agent Task → Git Worktree → Result
```

1. You type a command or natural language request
2. REPL sends it to the core via HTTP
3. Core routes to the appropriate model tier
4. An agent task is spawned in an isolated git worktree
5. Agent works on the task (can take minutes)
6. Results are committed to the worktree
7. Audit log is updated in `~/.local/share/kira/state.redb`

## Model Stack (Chinese Sovereignty)

Kira uses **only Chinese models** on the hot path:

- **DeepSeek** (primary)
- **Qwen/Alibaba** (fallback)
- **Ollama local** (dev only)

**No Anthropic or OpenAI** calls are made during normal operation.

## Where to See Results

- **Git worktrees**: `.worktrees/<wt_id>/` — isolated working copies per agent
- **State database**: `~/.local/share/kira/state.redb` — durable state
- **Audit logs**: Append-only JSONL for all agent actions
- **Mailbox**: `.kira/mailbox/<agent_id>/` — inter-agent communication (v0.5+)

## Common Workflow Example

```bash
# Terminal 1: Start core
cd /Users/avanade/Workspace/kira/core && cargo run

# Terminal 2: Start REPL
cd /Users/avanade/Workspace/kira/agent && bun run src/cli/index.tsx

# In REPL:
/mode plan
/plan draft writer "adicionar endpoint de saúde na API"
# Review plan output...
/plan approve <id>
# Wait for agent...
/agents
# See completed work...
/quit

# Check results:
cd .worktrees/<latest-wt-id>
git log --oneline
```

## Troubleshooting

| Issue | Solution |
|-------|----------|
| REPL can't connect to core | Ensure `cargo run` is running in another terminal |
| Agent hangs | Check `~/.local/share/kira/state.redb` for stuck state |
| Model errors | Verify `tokens.toml` has valid API keys |
| Worktree conflicts | Run `git worktree list` to see existing worktrees |

## Advanced: Hooks & Safety

Kira has built-in safety hooks configured in `kira-core.toml`:
- Blocks dangerous commands like `rm -rf`
- Auto-formats Rust code after file writes
- Pre-commit secret scanning

Test safety:
```
# In REPL, try to execute: rm -rf /
# Should be BLOCKED by hooks
```

## Next Steps

1. **Read the full plan**: `/Users/avanade/.claude/plans/users-avanade-documents-sergin-doc-vaul-mellow-moler.md`
2. **Explore workflows**: `.kira/workflows/*.yaml` (v0.5+)
3. **Customize prompts**: `core/agents/prompts/*.md`
4. **Adjust thresholds**: `tokens.toml` — single source of truth for config

---

**Remember**: Kira follows the "on the loop" philosophy — when agent output is bad, edit the harness (prompts, workflows, hooks), not the output itself.
