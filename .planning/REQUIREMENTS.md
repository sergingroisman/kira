# Requirements: Sergin Harness

**Defined:** 2026-05-09
**Core Value:** An autonomous agent system that creates, deploys, and markets niche websites to generate ad revenue, using its own financial performance to decide when and where to expand.

## v1 Requirements

Requirements for initial release. Each maps to roadmap phases.

### Sergin Agent Core

- [x] **AGENT-01**: Agent runs as a CLI REPL accepting user prompts and tool calls
- [x] **AGENT-02**: Agent can invoke Bash commands as a tool and return output to conversation
- [x] **AGENT-03**: Agent can read, write, and edit files on the local filesystem
- [ ] **AGENT-04**: Agent can perform web searches via an integrated search tool
- [x] **AGENT-05**: Agent reflects on tool outputs and retries or corrects on error
- [ ] **AGENT-06**: Agent can spawn isolated sub-agents for delegated tasks
- [x] **AGENT-07**: Agent maintains short-term memory via conversation history
- [ ] **AGENT-08**: Agent maintains long-term memory via periodic summarization
- [ ] **AGENT-09**: Agent supports user-defined custom tools/skills loaded at runtime

### MCP Integration

- [ ] **MCP-01**: Agent can connect to an MCP server as a client
- [ ] **MCP-02**: Agent discovers and invokes tools exposed by the MCP server
- [ ] **MCP-03**: Agent uses sergin-brain MCP for RAG queries (knowledge retrieval)
- [ ] **MCP-04**: Agent handles MCP connection errors gracefully

### Site Generator & Deploy Pipeline

- [ ] **SITE-01**: System generates static HTML sites from configurable templates
- [ ] **SITE-02**: Content generation uses DeepSeek API (or Ollama) for article writing
- [ ] **SITE-03**: Site templates include Google AdSense ad slots
- [ ] **SITE-04**: System deploys sites to Hostinger via SSH/rsync
- [ ] **SITE-05**: Deploy pipeline is managed by Terraform (null_resource + SSH)
- [ ] **SITE-06**: Cloudflare DNS records are managed by Terraform
- [ ] **SITE-07**: Multiple addon domains supported on single Hostinger plan

### Reddit Marketing Agent

- [ ] **MRKT-01**: Agent can log into Reddit using Playwright with stealth plugins
- [ ] **MRKT-02**: Agent can post link submissions to specified subreddits
- [ ] **MRKT-03**: Agent uses persistent sessions to avoid repeated logins
- [ ] **MRKT-04**: Agent respects rate limits and random delays between posts
- [ ] **MRKT-05**: Agent stores credentials securely via environment variables

### Infrastructure

- [ ] **INFRA-01**: Terraform manages local Docker infrastructure
- [ ] **INFRA-02**: Terraform manages Hostinger SSH deploy (null_resource)
- [ ] **INFRA-03**: Terraform manages Cloudflare DNS records
- [ ] **INFRA-04**: Terraform state is stored locally or in a configured backend
- [ ] **INFRA-05**: Repository follows monorepo structure with shared tooling

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Paperclip Loop

- **LOOP-01**: Revenue monitoring reads AdSense earnings automatically
- **LOOP-02**: Autonomous expansion decisions based on revenue thresholds
- **LOOP-03**: Budget-aware orchestration (max $X/mo spend, stop if unprofitable)
- **LOOP-04**: Automatic domain registration via Cloudflare/Namecheap API

### Enhanced Brain Integration

- **MBRAIN-01**: Bidirectional sync with sergin-brain for persistent memory
- **MBRAIN-02**: Agent stores site performance data in brain's vector DB
- **MBRAIN-03**: Brain provides SEO optimization suggestions

### Advanced Marketing

- **MRKT2-01**: Multi-account rotation for Reddit
- **MRKT2-02**: Karma-building automation (genuine engagement before posting)
- **MRKT2-03**: Cross-platform posting (Twitter, Pinterest)
- **MRKT2-04**: A/B testing of post titles and formats

## Out of Scope

| Feature | Reason |
|---------|--------|
| Mobile apps or native clients | Web-only; AdSense is the monetization channel |
| WordPress or CMS | Static HTML sufficient for niche sites; simpler deploy |
| E-commerce / affiliate marketing | AdSense-only for v1; affiliate adds complexity |
| Real-time analytics dashboard | Server logs + AdSense reports sufficient initially |
| Cloud Kubernetes (EKS/GKE) | Local MacBook + Hostinger keeps costs at $10-20/mo |
| Social platforms beyond Reddit | Reddit is highest ROI for niche content marketing |
| User authentication system | Single-user system; no multi-tenant needed |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| AGENT-01 | Phase 1 | Complete |
| AGENT-02 | Phase 1 | Complete |
| AGENT-03 | Phase 1 | Complete |
| AGENT-04 | Phase 1 | Pending |
| AGENT-05 | Phase 1 | Complete |
| AGENT-06 | Phase 2 | Pending |
| AGENT-07 | Phase 1 | Complete |
| AGENT-08 | Phase 2 | Pending |
| AGENT-09 | Phase 2 | Pending |
| MCP-01 | Phase 3 | Pending |
| MCP-02 | Phase 3 | Pending |
| MCP-03 | Phase 3 | Pending |
| MCP-04 | Phase 3 | Pending |
| SITE-01 | Phase 5 | Pending |
| SITE-02 | Phase 5 | Pending |
| SITE-03 | Phase 5 | Pending |
| SITE-04 | Phase 6 | Pending |
| SITE-05 | Phase 6 | Pending |
| SITE-06 | Phase 6 | Pending |
| SITE-07 | Phase 6 | Pending |
| MRKT-01 | Phase 7 | Pending |
| MRKT-02 | Phase 7 | Pending |
| MRKT-03 | Phase 7 | Pending |
| MRKT-04 | Phase 7 | Pending |
| MRKT-05 | Phase 7 | Pending |
| INFRA-01 | Phase 4 | Pending |
| INFRA-02 | Phase 4 | Pending |
| INFRA-03 | Phase 4 | Pending |
| INFRA-04 | Phase 4 | Pending |
| INFRA-05 | Phase 4 | Pending |

**Coverage:**
- v1 requirements: 30 total
- Mapped to phases: 30
- Unmapped: 0 ✓

---
*Requirements defined: 2026-05-09*
*Last updated: 2026-05-09 after initial definition*
