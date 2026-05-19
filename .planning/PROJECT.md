# Sergin Harness

## What This Is

A personal AI orchestration platform — a "paperclip maximizer" that operates a portfolio of niche websites to generate ad revenue autonomously. The system centers on a custom LangGraph-based CLI agent (Sergin Agent), inspired by Claude Code's architecture, that orchestrates content generation, site deployment, and marketing. It integrates with an existing MCP server (`sergin-brain`) for RAG and long-term memory, deploys static sites to Hostinger, and uses DeepSeek API as its reasoning engine.

Built by a senior software engineer for maximum control, minimal cost ($10-20/mo), and full ownership of the stack.

## Core Value

An autonomous agent system that creates, deploys, and markets niche websites to generate ad revenue, using its own financial performance to decide when and where to expand — running entirely on infrastructure the owner controls.

## Requirements

### Validated

<!-- Existing components already built and working -->

- ✓ **sergin-brain MCP server exists** (NestJS + LangChain + DeepSeek + Qdrant + Prisma) — WIP at `/Users/avanade/Workspace/sergin-brain`
- ✓ **Claude Code reference repo available** (`/Users/avanade/Workspace/claude-mythos`) — for architectural reverse engineering

### Active

<!-- Current scope. Building toward these. -->

- [ ] **AGENT-01**: Custom CLI agent (Sergin Agent) built with LangGraph + DeepSeek, inspired by Claude Code architecture — tool-calling, reflection, sub-agents, session memory
- [ ] **AGENT-02**: Agent can invoke Bash, file operations, and web search as tools
- [ ] **AGENT-03**: Agent has short-term (conversation) and long-term (summarized) memory
- [ ] **AGENT-04**: Agent connects to `sergin-brain` MCP server for RAG and persistent knowledge
- [ ] **BRAIN-01**: `sergin-brain` MCP server fully operational with RAG pipeline (Qdrant + embeddings)
- [ ] **BRAIN-02**: `sergin-brain` exposes tools for content generation context (niche research, SEO data, AdSense policies)
- [ ] **INFRA-01**: Infrastructure as Code with Terraform (local-first, MacBook)
- [ ] **INFRA-02**: Static site deployment pipeline to Hostinger via SSH/rsync
- [ ] **INFRA-03**: Cloudflare DNS management for custom domains
- [ ] **SITES-01**: AI-powered niche site generator (templates + DeepSeek content)
- [ ] **SITES-02**: Google AdSense integration in site templates
- [ ] **SITES-03**: Automated content refresh pipeline
- [ ] **MARKET-01**: Reddit marketing agent using Playwright + stealth automation
- [ ] **MARKET-02**: Rate-limited, karma-aware posting strategy
- [ ] **LOOP-01**: Revenue monitoring reads AdSense earnings
- [ ] **LOOP-02**: Autonomous expansion decisions based on revenue thresholds
- [ ] **LOOP-03**: Budget-aware orchestration (max $X/mo spend, stop if unprofitable)

### Out of Scope

- Mobile apps or native clients — web-only
- Social platforms beyond Reddit (Twitter, Instagram, TikTok) for v1
- E-commerce or affiliate marketing — AdSense-only for now
- Real-time user analytics — server logs are sufficient
- WordPress or CMS-heavy solutions — static HTML is the target

## Context

**Background:**
- Senior software engineer building for own use — no external users or clients
- Existing `sergin-brain` (NestJS/TS) at `/Users/avanade/Workspace/sergin-brain` provides RAG, vector search (Qdrant), and MCP protocol server — work in progress
- Leaked Claude Code repo at `/Users/avanade/Workspace/claude-mythos` available for architectural reference
- DeepSeek API credits already available for LLM inference
- Ollama available locally for potential offline inference

**Approach:**
- Agent-first: build the Sergin Agent core before infrastructure automation
- Monorepo in `sergin-harness` containing all components
- CLI-first interface for the agent (inspired by Claude Code)
- Horizontal layers: Agent Core → MCP/RAG → Infra → Sites → Marketing

**Existing ecosystem:**
- `/Users/avanade/Workspace/sergin-brain` — NestJS MCP server with LangChain, DeepSeek, Qdrant, Prisma/PostgreSQL
- `/Users/avanade/Workspace/claude-mythos` — Claude Code reference for architecture extraction
- DeepSeek API — primary LLM engine
- Hostinger (planned) — static site hosting
- Cloudflare (planned) — DNS + CDN

## Constraints

- **Budget**: $10-20/month max — shapes Hostinger plan choice, no GPU instances, local-first
- **Infrastructure**: MacBook-local for dev and runtime initially (no cloud Kubernetes)
- **Monorepo**: All code lives in this single repo (`sergin-harness`)
- **Language**: TypeScript preferred (matching `sergin-brain` ecosystem), Python acceptable for agent-specific work
- **Content quality**: Must avoid AI-generated-content penalties from AdSense — requires human-in-loop or quality gates
- **Reddit risk**: Anti-bot measures require stealth approach — accounts and domains can be burned

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Agent-first build order | Core orchestrator needed before infrastructure automation | — Pending |
| LangGraph + DeepSeek for agent | Leverages existing DeepSeek API credits, function calling support | — Pending |
| TypeScript monorepo | Matches sergin-brain, enables shared types and MCP client | — Pending |
| Static HTML sites (no CMS) | Lowest cost, simplest deploy, sufficient for niche content | — Pending |
| Hostinger shared hosting | Cheapest option with SSH access for multiple addon domains | — Pending |
| Local-first (no cloud K8s) | $0 additional infra cost, simplifies initial development | — Pending |
| CLI-first agent interface | Matches Claude Code paradigm, simpler than building UI | — Pending |
| Monorepo | Single source of truth, shared tooling, easier orchestration | — Pending |

---
*Last updated: 2026-05-09 after project initialization*
