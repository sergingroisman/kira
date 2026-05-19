# Roadmap: Sergin Harness

**Created:** 2026-05-09
**Phases:** 7 | **Requirements:** 30 | **Coverage:** 100%

| # | Phase | Goal | Reqs | Criteria |
|---|-------|------|------|----------|
| 1 | Agent Foundation | Build core LangGraph agent with REPL, tools, and reflection | 6 | 4 |
| 2 | Advanced Agent | Sub-agents, long-term memory, custom skills | 3 | 3 |
| 3 | MCP Integration | Connect agent to sergin-brain MCP for RAG | 4 | 3 |
| 4 | Infrastructure & Monorepo | Terraform IaC, repo structure, Hostinger setup | 5 | 3 |
| 5 | Site Generator | Static site templates, AI content generation, AdSense | 3 | 3 |
| 6 | Deploy Pipeline | SSH deploy, DNS, multi-domain support | 4 | 3 |
| 7 | Reddit Marketing Agent | Playwright stealth automation, rate-limited posting | 5 | 3 |

---

## Phase 1: Agent Foundation
**Goal:** Build the core LangGraph-based agent that runs as a CLI REPL, can invoke tools (Bash, file ops, web search), reflect on errors, and maintain conversation memory.
**Mode:** mvp
**Success Criteria:**
1. User can run the agent CLI, type a prompt, and get a response
2. Agent can execute `ls`, `git status`, and similar bash commands and return formatted output
3. Agent can read, write, and edit files on the filesystem
4. Agent performs web search and incorporates results into responses
5. When a bash command fails, agent retries or explains the error
6. Conversation history persists within a session (short-term memory)

**Requirements:** AGENT-01, AGENT-02, AGENT-03, AGENT-04, AGENT-05, AGENT-07
**Dependencies:** None (greenfield)
**Plans:** 4 plans

Plans:
- [x] 01-01-PLAN.md — Walking Skeleton: monorepo scaffold + minimal streaming REPL (callLLM → END) — Complete (4 commits, 20 files)
- [x] 01-02-PLAN.md — Tool Slice: ToolRegistry + Bash + File tools + reflect node wired into graph — Complete (4 commits, 10 files, 17 tests)
- [ ] 01-03-PLAN.md — Search + Memory: Tavily WebSearch tool + MemorySaver cross-turn validation
- [ ] 01-04-PLAN.md — Ink UI Polish: streaming batching, spinner, ToolCallPanel, system prompt

## Phase 2: Advanced Agent Capabilities
**Goal:** Extend the agent with sub-agent spawning, long-term memory via summarization, and a plugin system for custom tools.
**Mode:** mvp
**Success Criteria:**
1. Agent can spawn a sub-agent with isolated context for a delegated task
2. Sub-agent returns results to the main agent without polluting its context
3. Agent summarizes and persists session state across CLI restarts
4. User can register custom tools via a configuration file or directory

**Requirements:** AGENT-06, AGENT-08, AGENT-09
**Dependencies:** Phase 1

## Phase 3: MCP Integration
**Goal:** Connect the agent to external MCP servers, specifically sergin-brain for RAG-powered knowledge retrieval.
**Mode:** mvp
**Success Criteria:**
1. Agent acts as an MCP client, connecting to a configured server
2. Agent discovers available tools from the MCP server
3. Agent invokes sergin-brain's RAG search tool and returns results
4. Connection failures are handled gracefully with meaningful error messages

**Requirements:** MCP-01, MCP-02, MCP-03, MCP-04
**Dependencies:** Phase 1, external sergin-brain (WIP)

## Phase 4: Infrastructure & Monorepo
**Goal:** Establish the monorepo structure, Terraform configuration for local infrastructure, Hostinger deployment, and Cloudflare DNS management.
**Mode:** mvp
**Success Criteria:**
1. Monorepo has consistent tooling (lint, test, build across packages)
2. Terraform configures Hostinger SSH deploy target (null_resource)
3. Terraform manages Cloudflare DNS records for custom domains
4. Terraform state is initialized and applies cleanly
5. Local Docker services (if needed) are managed via Terraform

**Requirements:** INFRA-01, INFRA-02, INFRA-03, INFRA-04, INFRA-05
**Dependencies:** None (can run in parallel with Phase 1-3)

## Phase 5: Site Generator
**Goal:** Build a static site generator that produces HTML pages from templates, generates niche content via DeepSeek API, and includes AdSense ad slots.
**Mode:** mvp
**Success Criteria:**
1. Site generator produces valid HTML from a template + content JSON
2. DeepSeek API generates article content for a given niche/topic
3. Generated sites include Google AdSense ad units in templates
4. Output is a complete static site ready for deployment

**Requirements:** SITE-01, SITE-02, SITE-03
**Dependencies:** Phase 4 (deploy target)

## Phase 6: Deploy Pipeline
**Goal:** Automate site deployment to Hostinger via SSH/rsync, manage multiple addon domains, and wire Cloudflare DNS through Terraform.
**Mode:** mvp
**Success Criteria:**
1. `terraform apply` deploys a generated site to Hostinger via SSH/rsync
2. Cloudflare DNS records are created/updated for each new site
3. Multiple addon domains are supported on the same Hostinger plan
4. Full cycle: generate → deploy → DNS works end-to-end

**Requirements:** SITE-04, SITE-05, SITE-06, SITE-07
**Dependencies:** Phase 4, Phase 5

## Phase 7: Reddit Marketing Agent
**Goal:** Build a Playwright-based Reddit automation agent with stealth plugins, persistent sessions, and rate-limited posting.
**Mode:** mvp
**Success Criteria:**
1. Agent logs into Reddit using Playwright with stealth anti-detection
2. Agent posts a link submission to a specified subreddit
3. Session persists across runs (no repeated logins)
4. Configurable rate limits and random delays prevent ban patterns
5. Credentials are stored in environment variables, never hardcoded

**Requirements:** MRKT-01, MRKT-02, MRKT-03, MRKT-04, MRKT-05
**Dependencies:** Phase 4, Phase 6 (needs sites to have content to link)

---

## Execution Plan

**Parallel tracks:**
- Track A (Agent): Phase 1 → Phase 2 → Phase 3
- Track B (Infra): Phase 4 → Phase 5 → Phase 6
- Track C (Marketing): Phase 7 (after Phase 6)

Tracks A and B can run in parallel. Track C depends on B.

**Phases requiring research:** All — each phase involves a specific domain (LangGraph, MCP protocol, Terraform, Playwright) worth investigating for current best practices.
