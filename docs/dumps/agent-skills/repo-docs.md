---
type: Dump
title: gitingest — tech-leads-club/agent-skills
- **Fonte**: https://github.com/tech-leads-club/agent-skills @ commit e74dc8a
- **Licença**: código MIT (Copyright (c) 2026 Tech Leads Club); conteúdo dos SKILL.md
  CC-BY-4.0 — atribuição: Tech Leads Club + autor indicado em `metadata.author` de cada skill.
- **Gerado por**: `docs/dumps/generate-agent-skills-dump.ps1` — caminhada ordenada por caminho;
  mesma entrada = mesmo dump. Fora de escopo: `references/`, `templates/`, código CLI/MCP.
---

# gitingest — agent-skills — documentos do repo

================================================================
File: README.md
================================================================
<p align="center">
  <img src=".github/assets/logo.png" alt="Tech Leads Club" width="400" />
</p>

<p align="center">
  <img src="https://img.shields.io/npm/v/@tech-leads-club/agent-skills?style=flat-square&color=blue" alt="npm version" />
  <img src="https://img.shields.io/npm/dt/@tech-leads-club/agent-skills?style=flat-square&color=blue" alt="total downloads" />
  <img src="https://img.shields.io/npm/dm/@tech-leads-club/agent-skills?style=flat-square&color=blue" alt="monthly downloads" />
  <img src="https://img.shields.io/github/license/tech-leads-club/agent-skills?style=flat-square" alt="license" />
  <img src="https://img.shields.io/github/actions/workflow/status/tech-leads-club/agent-skills/release.yml?style=flat-square" alt="build status" />
</p>

<p align="center">
  <img src="https://img.shields.io/badge/node-%3E%3D22-brightgreen?style=flat-square&logo=node.js" alt="node version" />
  <img src="https://img.shields.io/badge/TypeScript-100%25-blue?style=flat-square&logo=typescript" alt="typescript" />
  <img src="https://img.shields.io/badge/Nx%20Cloud-Enabled-blue?style=flat-square&logo=nx" alt="nx cloud" />
  <img src="https://img.shields.io/badge/%20%20%F0%9F%93%A6%F0%9F%9A%80-semantic--release-e10079.svg?style=flat-square" alt="semantic-release" />
</p>

<p align="center">
  <img src="https://img.shields.io/github/stars/tech-leads-club/agent-skills?style=flat-square&color=yellow" alt="github stars" />
  <img src="https://img.shields.io/github/contributors/tech-leads-club/agent-skills?style=flat-square&color=orange" alt="contributors" />
  <img src="https://img.shields.io/github/last-commit/tech-leads-club/agent-skills?style=flat-square" alt="last commit" />
  <img src="https://img.shields.io/badge/AI-Powered%20Skills-purple?style=flat-square&logo=openai" alt="ai powered" />
</p>

<h1 align="center">🧠 Agent Skills</h1>

<p align="center">
  <strong>The secure, validated skill registry for professional AI coding agents</strong>
</p>

<p align="center">
  In an ecosystem where <a href="https://github.com/snyk/agent-scan/blob/main/.github/reports/skills-report.pdf">over 13% of marketplace skills contain critical vulnerabilities</a>,
  <b>Agent Skills</b> stands apart as a hardened library of <b>verified</b>, <b>tested</b>, and <b>safe</b> capabilities.
  Extend <b>Antigravity</b>, <b>Claude Code</b>, <b>Cursor</b>, and more with absolute confidence.
</p>

<p align="center">
  <a href="https://agent-skills.techleads.club/" target="_blank">https://agent-skills.techleads.club/</a>
</p>

## 📖 Table of Contents

- [✨ What are Skills?](#-what-are-skills)
- [🛡️ Security & Trust](#️-security--trust)
- [🤖 Supported Agents](#-supported-agents)
- [🌟 Featured Skills](#-featured-skills)
- [🚀 Quick Start](#-quick-start)
- [⚡ How It Works](#-how-it-works)
- [🔌 MCP Server](#-mcp-server)
- [🤝 Contributing](#-contributing)
- [🛡️ Content & Authorship](#️-content--authorship)
- [📄 License and Attribution](#-license-and-attribution)

## ✨ What are Skills?

Skills are packaged instructions and resources that extend AI agent capabilities. Think of them as **plugins for your AI assistant** — they teach your agent new workflows, patterns, and specialized knowledge.

```
packages/skills-catalog/skills/
  (category-name)/
    skill/
      SKILL.md          ← Main instructions
      templates/        ← File templates
      references/       ← On-demand documentation
```

## 🛡️ Security & Trust

Your environment's safety is our top priority. Unlike open marketplaces where **13.4% of skills contain critical issues**, `agent-skills` is a managed, hardened library: 100% open source (no binaries), static analysis in CI/CD, immutable integrity via lockfiles and content hashing, and human-curated prompts. The CLI uses defense-in-depth (sanitization, path isolation, symlink guards, atomic lockfile, audit trail); every skill is scanned with [Snyk Agent Scan](https://github.com/snyk/agent-scan) (formerly mcp-scan) before publishing.

→ **Full threat model, implementation details, and vulnerability reporting:** [SECURITY.md](SECURITY.md)

## 🤖 Supported Agents

Install skills to any of these AI coding agents:

<div align="center">
<br />

|                     Tier 1 (Popular)                      |                            Tier 2 (Rising)                             |                   Tier 3 (Enterprise)                   |
| :-------------------------------------------------------: | :--------------------------------------------------------------------: | :-----------------------------------------------------: |
|         **[Claude Code](https://claude.ai/code)**         |                    **[Aider](https://aider.chat)**                     |   **[Amazon Q](https://aws.amazon.com/q/developer/)**   |
|        **[Cline](https://github.com/cline/cline)**        |               **[Antigravity](https://idx.google.com)**                |       **[Augment](https://www.augmentcode.com)**        |
|             **[Cursor](https://cursor.com)**              | **[Gemini CLI](https://ai.google.dev/gemini-api/docs/code-execution)** |    **[Droid (Factory.ai)](https://www.factory.ai)**     |
| **[GitHub Copilot](https://github.com/features/copilot)** |                  **[Kilo Code](https://kilocode.ai)**                  |           **[OpenCode](https://opencode.ai)**           |
|       **[Windsurf](https://codeium.com/windsurf)**        |                     **[Kiro](https://kiro.dev/)**                      |  **[Sourcegraph Cody](https://sourcegraph.com/cody)**   |
|                                                           |    **[OpenAI Codex](https://openai.com/index/introducing-codex/)**     |         **[Tabnine](https://www.tabnine.com)**          |
|                                                           |                    **[Roo Code](https://roo.dev)**                     |                                                         |
|                                                           |                    **[TRAE](https://docs.trae.ai)**                    |                                                         |

</div>

<p align="center">
  <sub>Missing your favorite agent? <a href="https://github.com/tech-leads-club/agent-skills/issues/new"><strong>Open an issue</strong></a> and we'll add support!</sub>
</p>

## 🌟 Featured Skills

A glimpse of what's available in our growing catalog:

| Skill                                                                                              | Category    | Description                                                                                                                                                                        |
| -------------------------------------------------------------------------------------------------- | ----------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **[tlc-spec-driven](<packages/skills-catalog/skills/(development)/tlc-spec-driven>)**              | Development | Project and feature planning with 4 phases: Specify → Design → Tasks → Implement. Creates atomic tasks with verification criteria and maintains persistent memory across sessions. |
| **[aws-advisor](<packages/skills-catalog/skills/(cloud)/aws-advisor>)**                            | Cloud       | Expert AWS Cloud Advisor for architecture design, security review, and implementation guidance. Leverages AWS MCP tools for documentation-backed answers.                          |
| **[playwright-skill](<packages/skills-catalog/skills/(web-automation)/playwright-skill>)**         | Automation  | Complete browser automation with Playwright. Test pages, fill forms, take screenshots, validate UX, and automate any browser task.                                                 |
| **[figma](<packages/skills-catalog/skills/(design)/figma>)**                                       | Design      | Fetch design context from Figma and translate nodes into production code. Design-to-code implementation with MCP integration.                                                      |
| **[security-best-practices](<packages/skills-catalog/skills/(security)/security-best-practices>)** | Security    | Language and framework-specific security reviews. Detect vulnerabilities, generate reports, and suggest secure-by-default fixes.                                                   |

<p align="center">
  <a href="#-quick-start"><strong>→ Browse all skills</strong></a>
</p>

## 🚀 Quick Start

### Install Skills in Your Project

```bash
npx @tech-leads-club/agent-skills
```

This launches an interactive wizard:

1. **Choose Action** — "Install skills" or "Update installed skills"
2. **Browse & Select** — Filter by category or search
3. **Choose agents** — Pick target agents (Cursor, Claude Code, etc.)
4. **Installation method** — Copy (recommended) or Symlink
5. **Scope** — Global (user home) or Local (project only)

Each step shows a **← Back** option to return and revise your choices.

### CLI Options

> **Note**: You can use either `npx @tech-leads-club/agent-skills` or install globally and use `agent-skills` directly.

```bash
# Interactive mode (default)
npx @tech-leads-club/agent-skills
# or: agent-skills (if installed globally)

# List available skills
agent-skills list
agent-skills ls        # Alias

# Install one skill
agent-skills install -s tlc-spec-driven

# Install multiple skills at once
agent-skills install -s aws-advisor coding-guidelines docs-writer

# Install to specific agents
agent-skills install -s my-skill -a cursor claude-code

# Install multiple skills to multiple agents
agent-skills install -s aws-advisor nx-workspace -a cursor windsurf cline

# Install globally (to ~/.gemini, ~/.claude, etc.)
agent-skills install -s my-skill -g

# Use symlink instead of copy
agent-skills install -s my-skill --symlink

# Force re-download (bypass cache)
agent-skills install -s my-skill --force

# Update a specific skill
agent-skills update -s my-skill

# Update all installed skills
agent-skills update

# Remove one skill
agent-skills remove -s my-skill

# Remove multiple skills at once
agent-skills remove -s skill1 skill2 skill3
agent-skills rm -s my-skill    # Alias

# Remove from specific agents
agent-skills remove -s my-skill -a cursor windsurf

# Force removal (bypass lockfile check)
agent-skills remove -s my-skill --force

# Manage cache
agent-skills cache --clear           # Clear all cache
agent-skills cache --clear-registry  # Clear only registry
agent-skills cache --path            # Show cache location

# View audit log
agent-skills audit                   # Show recent operations
agent-skills audit -n 20             # Show last 20 entries
agent-skills audit --path            # Show audit log location

# Show contributors and credits
agent-skills credits

# Show help
agent-skills --help
```

### Global Installation (Optional)

```bash
npm install -g @tech-leads-club/agent-skills
agent-skills  # Use 'agent-skills' instead of 'npx @tech-leads-club/agent-skills'
```

## ⚡ How It Works

The CLI fetches skills **on-demand** from our CDN:

1. **Browse** — The CLI fetches the skills catalog (~45KB)
2. **Select** — You choose the skills you need
3. **Download** — Selected skills are downloaded and cached locally
4. **Install** — Skills are installed to your agent's configuration

### Caching

Downloaded skills are cached in `~/.cache/agent-skills/` for offline use.

```bash
# Clear the cache
rm -rf ~/.cache/agent-skills
```

## 🔌 MCP Server

`@tech-leads-club/agent-skills-mcp` is an MCP server that exposes the skills catalog directly to AI agents via **progressive disclosure** — search first, then fetch only what's needed.

| Tool                | Purpose                              |
| :------------------ | :----------------------------------- |
| `list_skills`       | Browse all skills by category        |
| `search_skills`     | Find skills by intent (fuzzy search) |
| `read_skill`        | Load a skill's main instructions     |
| `fetch_skill_files` | Fetch specific reference files       |

`list_skills` should be called only when the user explicitly asks to browse/list the catalog.

**Quick install** (works with any MCP-compatible client):

```json
{
  "mcpServers": {
    "agent-skills": {
      "command": "npx",
      "args": ["-y", "@tech-leads-club/agent-skills-mcp"]
    }
  }
}
```

→ Full setup for all clients (Cursor, Claude Code, VS Code, etc.), caching, and error reference: **[packages/mcp/README.md](packages/mcp/README.md)**

## 🤝 Contributing

We welcome contributions! Please see our [CONTRIBUTING.md](CONTRIBUTING.md) file for detailed guidelines on how to set up your local environment, create new skills, contribute to the marketplace, and follow our release processes.

## 🛡️ Content & Authorship

This repository is a collection of curated skills intended to benefit the community. We deeply respect the intellectual property and wishes of all creators.

If you are the author of any content included here and would like it **removed** or **updated**, please [open an issue](https://github.com/tech-leads-club/agent-skills/issues/new) or contact the maintainers.

## 📄 License and Attribution

- **Software Engine:** The application source code (CLI, scripts, tools) is licensed under the **[MIT License](LICENSE)**.
- **Tech Leads Club Skills:** Unless otherwise stated, all skill files (`SKILL.md`) authored by the repository maintainers are licensed under the **[Creative Commons Attribution 4.0 International License (CC-BY-4.0)](https://creativecommons.org/licenses/by/4.0/)**.
- **Third-Party Skills:** Some skills included in this catalog are created by the community or original authors. These skills retain their original licenses and copyrights. Please check the individual `SKILL.md` files for specific licensing and author attribution.

_If you use our skills catalog, you **must** provide attribution to Tech Leads Club, regardless of how it is used._

## ⭐ Star History

<p align="center">
  <a href="https://star-history.com/#tech-leads-club/agent-skills&Date">
    <img src="https://api.star-history.com/svg?repos=tech-leads-club/agent-skills&type=Date" alt="Star History Chart" />
  </a>
</p>

---

<p align="center">
  <sub>Built with ❤️ by the Tech Leads Club community</sub>
</p>

================================================================
File: SECURITY.md
================================================================
# 🛡️ Security Policy

## Philosophy

Agent Skills is a **managed, hardened** skill registry. Unlike open marketplaces where [over 13% of skills contain critical issues](https://github.com/snyk/agent-scan/blob/main/.github/reports/skills-report.pdf), every skill and every tool in this repository is designed with security as a first-class constraint — not an afterthought.

Security here means three things: protecting **your environment** (CLI defense-in-depth), protecting **your context window** (MCP progressive disclosure), and protecting **your trust** (supply chain integrity).

## 🎯 Threat Model

We directly address the threats identified in the [Snyk 2026 Agent Threat Report](https://github.com/snyk/agent-scan/blob/main/.github/reports/skills-report.pdf):

| Threat                   | Public Marketplaces                                         | Agent Skills Guarantee                                                                                          |
| :----------------------- | :---------------------------------------------------------- | :-------------------------------------------------------------------------------------------------------------- |
| **Malicious Payloads**   | Obfuscated code, binaries, or "black box" instructions      | **100% Open Source**: No binaries, fully readable text/code. Every line is auditable.                           |
| **Credential Theft**     | Skills silently exfiltrating env vars to remote servers     | **Static Analysis**: CI/CD pipeline blocks skills with suspicious network calls or secret access.               |
| **Supply Chain Attacks** | Authors pushing malicious updates to existing skills        | **Immutable Integrity**: Lockfiles and content-hashing ensure code never changes without your explicit upgrade. |
| **Prompt Injection**     | Hidden instructions to hijack agent behavior ("jailbreaks") | **Human Curation**: Every prompt is manually code-reviewed by maintainers for safety boundaries.                |

## 🔐 CLI Defense-in-Depth

The CLI installer (`packages/cli`) implements multiple independent security layers. Each operation passes through **all** of them — defense-in-depth means no single bypass is enough.

### 1. Input Sanitization

Every skill name and file path is sanitized before use:

```typescript
// packages/cli/src/services/installer.ts
const sanitizeName = (name: string): string => {
  return (
    name
      .replace(/[/\\]/g, '') // Remove path separators
      .replace(/[\0:*?"<>|]/g, '') // Remove null bytes and Windows-forbidden chars
      .replace(/^[.\s]+|[.\s]+$/g, '') // Strip leading/trailing dots and spaces
      .replace(/\.{2,}/g, '') // Collapse consecutive dots (no "..")
      .replace(/^\.+/, '') // No leading dots (hidden files)
      .substring(0, 255) || // Enforce filesystem name length limit
    'unnamed-skill'
  )
}
```

This blocks: `../../../etc/passwd`, `skill\0name`, `/etc/passwd`, `skill:name`, `.hidden`, `a`.repeat(300).

### 2. Filesystem Isolation (Path Traversal Protection)

Even after sanitization, every resolved path is verified to be strictly inside the allowed base directory:

```typescript
// packages/cli/src/services/installer.ts
const isPathSafe = (basePath: string, targetPath: string): boolean => {
  const normalizedBase = normalize(resolve(basePath))
  const normalizedTarget = normalize(resolve(targetPath))
  return normalizedTarget.startsWith(normalizedBase + sep) || normalizedTarget === normalizedBase
}
```

Both paths are **fully resolved** (`resolve()`) before comparison — relative paths, symlinks, and `..` sequences are eliminated before the check. The OS-specific separator (`sep`) is used to prevent tricks like `/allowed/dir../escape`.

This guard is applied at every write, read, and delete operation: `installSkillForAgent()`, `getInstallPath()`, `getCanonicalPath()`, `removeSkill()`, and `isSkillInstalled()`.

### 3. Symlink Guard

Symlinks are treated as untrusted by default:

- **`lstat()` not `stat()`** — detects symlinks without following them, preventing TOCTOU attacks
- **Target validation** — resolves the final target and verifies it is within the allowed base directory, even for chained symlinks
- **Loop detection** — `ELOOP` errors (circular symlink chains) are caught and the link is forcibly removed
- **Windows junctions** — on Windows, directory junctions are used instead of symlinks for better OS-level containment

```typescript
const validateSymlinkTarget = async (linkPath: string, baseDir: string): Promise<boolean> => {
  const stats = await lstat(linkPath) // Never follows the link
  if (stats.isSymbolicLink()) {
    const target = await readlink(linkPath)
    const resolvedTarget = resolve(join(linkPath, '..'), target)
    return isPathSafe(baseDir, resolvedTarget) // Target must stay inside allowed dir
  }
  return true
}
```

### 4. Lockfile Integrity

The lockfile (`.agents/.skill-lock.json`) is the source of truth for installed skills. It is protected by:

**Schema validation (Zod):** Every lockfile read is parsed through a strict Zod schema. Invalid or malformed entries are rejected and the file gracefully migrates to a clean state — no silent corruption.

**Atomic writes:** The lockfile is never written in-place:

```
1. Backup existing file → .skill-lock.json.bak
2. Write new content   → .skill-lock.json.tmp
3. Atomic rename       → .skill-lock.json.tmp → .skill-lock.json
```

If the process is killed mid-write, the old file is intact. If the rename fails, the temp file is cleaned up.

**Content hashing:** Each installed skill records a SHA-256 content hash computed from all its files. This enables tamper detection: if a skill file changes on disk after installation, the hash mismatch is detectable on the next operation.

**Removal authorization:** Skills cannot be removed unless they appear in the lockfile. The `--force` flag bypasses this check and is audit-logged.

### 5. Audit Trail

Every install, update, and remove operation is appended to an audit log at `~/.config/agent-skills/audit.log` (JSON Lines format):

```json
{"action":"install","skillName":"codenavi","agents":["cursor"],"success":1,"failed":0,"timestamp":"2026-02-25T10:00:00Z"}
{"action":"remove","skillName":"codenavi","agents":["cursor"],"success":1,"failed":0,"forced":false,"timestamp":"2026-02-25T11:00:00Z"}
```

The log is **append-only** — entries are never overwritten. It is the forensic record of all agent skill operations on the machine.

## 🔍 Security Scanning

Every skill in the catalog is scanned with [**Snyk Agent Scan**](https://github.com/snyk/agent-scan) (formerly `mcp-scan`) before publishing. The scan is **incremental** — only skills whose content has changed since the last run are re-scanned. The scanner requires a `SNYK_TOKEN` environment variable (see [Security scan setup](docs/security-scan.md) for CI and fork PR behavior).

```bash
export SNYK_TOKEN=your-token   # Required; get it from https://app.snyk.io/account
npm run scan                   # Incremental (default — only changed skills)
npm run scan -- --force        # Force full re-scan of all skills
```

### How It Works

Each skill has a SHA-256 content hash computed from all its files. Results are cached in `.security-scan-cache.json` (gitignored). On the next run:

```
Content hash unchanged → load from cache (fast, no re-scan)
Content hash changed   → re-scan with snyk-agent-scan, update cache
```

This makes the scan fast enough to run on every PR and release without slowing CI/CD.

### Handling False Positives

If the scanner flags a finding that is intentional (e.g. a first-party MCP server integration), add it to the allowlist at `packages/skills-catalog/security-scan-allowlist.yaml`:

```yaml
version: '1.0.0'

entries:
  - skill: my-skill
    code: W011
    reason: >
      Fetches from trusted first-party API at api.example.com — expected behavior,
      not a credential exfiltration vector. Reviewed by maintainers on 2026-01-01.
    allowedBy: github.com/username
    allowedAt: '2026-01-01'
    expiresAt: '2027-01-01' # Optional but strongly recommended
```

**Rules:**

- Matching is `skill + code` — no re-scan needed, takes effect immediately
- `expiresAt` is optional but **strongly recommended** — forces periodic review of exceptions
- Expired entries automatically re-activate the finding, ensuring exceptions don't become permanent
- The allowlist is committed and reviewed in every PR — no exceptions are invisible

### Security Scan in CI/CD

The scan must pass before any release. The release pipeline (`release.yml`) runs `npm run scan` as a required step when the run has access to secrets (same-repo PRs and push to `main`). **PRs from forks do not run the scan** in the normal PR workflow (GitHub does not expose repository secrets to fork workflows). To **block merge until the scan passes** for all PRs (including forks), use [GitHub Merge Queue](docs/security-scan.md#blocking-merge-for-fork-prs-merge-queue) and require the "Security Scan (merge queue)" status check. See [Security scan setup](docs/security-scan.md) for details. A failed scan blocks the release when the scan runs.

## 🔌 MCP Server Security

The `@tech-leads-club/agent-skills-mcp` server (`packages/mcp`) has a narrower threat surface by design:

- **Read-only** — the server has no write access to the registry; it only reads from CDN
- **No authentication** — the server runs locally over stdio; there is no network-exposed endpoint
- **Path validation** — `fetch_skill_files` validates every requested file path against the registry's `files[]` array before any network call; it is impossible to fetch an arbitrary URL
- **No local filesystem access** — the server fetches from CDN only; it never reads or writes local files
- **Stdout reserved for JSON-RPC** — all logging goes exclusively to `stderr` to prevent protocol corruption

## 🛡️ Content Trust & Authorship

This repository is a collection of curated skills intended to benefit the community. We deeply respect the intellectual property and wishes of all creators.

**If you are the author of any content included here** and would like it removed or updated, please [open an issue](https://github.com/tech-leads-club/agent-skills/issues/new) or contact the maintainers directly. We will act promptly.

For licensing details, see **[📄 License and Attribution](README.md#-license-and-attribution)** in the README.

## 🚨 Reporting a Vulnerability

**Please do not open a public GitHub issue for security vulnerabilities.**

To report a vulnerability, open a [GitHub Security Advisory](https://github.com/tech-leads-club/agent-skills/security/advisories/new) (private, only visible to maintainers).

Include:

- A description of the vulnerability
- Steps to reproduce
- Affected component (`cli`, `mcp`, `skills-catalog`, a specific skill)
- Potential impact

We aim to acknowledge reports within **48 hours** and resolve confirmed vulnerabilities within **14 days**. We will credit reporters in the fix commit unless anonymity is requested.

================================================================
File: AGENTS.md
================================================================
# AGENTS.md

Guidance for AI Coding Agents when working with code in this repository.

## Workflow Orchestration

### 1. Plan Mode Default

- Enter plan mode for ANY non-trivial task (3+ steps or architectural decisions)
- If something goes sideways, STOP and re-plan immediately — don't keep pushing
- Use plan mode for verification steps, not just building
- Write detailed specs upfront to reduce ambiguity

### 2. Subagent Strategy

- Use subagents liberally to keep main context window clean
- Offload research, exploration, and parallel analysis to subagents
- For complex problems, throw more compute at it via subagents
- One task per subagent for focused execution

### 3. Verification Before Done

- Never mark a task complete without proving it works
- Run the full test suite before considering work done
- Verify your changes against the existing behavior
- Ask yourself: "Would a staff engineer approve this?"

### 4. Demand Elegance (Balanced)

- For nontrivial changes: pause and ask "is there a more elegant way?"
- If a fix feels hacky: "Knowing everything I know now, implement the elegant solution"
- Skip this for simple, obvious fixes — don't over-engineer
- Challenge your own work before presenting it

### 5. Autonomous Bug Fixing

- When given a bug report: just fix it. Don't ask for hand-holding
- Run tests to identify the root cause
- Zero context switching required from the user
- Go fix failing tests without being told how

## Core Principles

- **Simplicity First**: Make every change as simple as possible. Minimal code impact.
- **No Laziness**: Find root causes. No temporary workarounds. Senior developer standards.
- **Minimal Impact**: Changes should only touch what's necessary. Avoid introducing bugs.

---

## Repository

**Agent Skills Monorepo** — a registry and CLI for distributing "skills" (packaged instructions) to AI coding agents (Claude Code, Cursor, Copilot, Windsurf, Cline, and 14 others).

## Commands

```bash
# Setup
npm ci && npm run build

# Development
npm run start:dev:cli          # Run CLI interactively (tsx, no build needed)
npm run start:dev:mcp          # Build MCP and open Inspector
SKILLS_CDN_REF=main npm run dev -- install  # Test CLI against local registry

# Build & Test
npm run build                  # Build all packages (Nx)
npm run test                   # Run all tests (requires Node --experimental-vm-modules)
npm run test --workspace=@tech-leads-club/agent-skills  # CLI tests only

# Quality
npm run lint                   # ESLint
npm run lint:fix
npm run format:check           # Prettier
npm run format

# Skills
npm run generate:data          # Regenerate skills-registry.json + marketplace data
npm run scan                   # Security scan (snyk-agent-scan, incremental; requires SNYK_TOKEN)
nx g @tech-leads-club/skill-plugin:skill {name} --category={cat}  # Create new skill
```

## Architecture

**Nx monorepo** with independent versioning and conventional commits.

### Packages

- **`packages/cli`** — `@tech-leads-club/agent-skills`. The user-facing installer. Dual-mode: interactive TUI (React + Ink) and non-interactive CLI (Commander.js). Entry: `src/index.ts`. Business logic in `src/services/` (registry fetching, skill installation, lockfile management, agent configs).
- **`packages/skills-catalog`** — Skill definitions in `skills/{(category)}/{skill-name}/SKILL.md` with YAML frontmatter. `src/generate-registry.ts` produces `skills-registry.json` (committed, published to npm, served via jsDelivr CDN).
- **`packages/marketplace`** — Next.js 16 static site deployed to GitHub Pages. Reads from `src/data/skills.json` (generated from the registry).
- **`libs/core`** — `@tech-leads-club/core`. Shared types (`AgentType`, `SkillInfo`, `CategoryInfo`) and constants.
- **`tools/skill-plugin`** — Nx generator for scaffolding new skills.

### Key Flows

**Skill installation**: CLI fetches `skills-registry.json` from CDN → downloads skill files (batched, 10 concurrent) to `~/.cache/agent-skillsls/` → installs via copy or symlink to agent-specific directory → records in `.agents/.skill-lock.json` (v2, Zod-validated, atomic writes).

**Registry generation**: `generate-registry.ts` scans all `SKILL.md` files, parses frontmatter, computes SHA-256 content hashes, outputs `skills-registry.json`.

### Agent Configuration

Canonical agent config is in `packages/cli/src/services/agents.ts`. Each of the 19 supported agents has `skillsDir` (project-local) and `globalSkillsDir` (home directory) paths. Agents are tiered: Tier 1 (Cursor, Claude Code, Copilot, Windsurf, Cline), Tier 2, Tier 3.

## Code Conventions

- **ESM-only** throughout. Jest requires `NODE_OPTIONS='--experimental-vm-modules'`.
- **TypeScript strict mode**. `@typescript-eslint/no-explicit-any: error`. Unused vars with `_` prefix are allowed.
- **Prettier**: no semicolons, single quotes, trailing commas, 120 char width, organize-imports plugin.
- **Node ≥ 24** (monorepo), **Node ≥ 22** (CLI package).
- Tests: Jest 30 + ts-jest. Property-based tests use fast-check (`*.pbt.test.ts`). Security-specific tests exist for installer and lockfile.

## Skill Structure

```
skills/(category)/{skill-name}/
├── SKILL.md          # Required — YAML frontmatter + markdown body
├── scripts/          # Optional executable scripts
├── templates/        # Optional file templates
└── references/       # Optional on-demand documentation
```

Always use the Nx generator (`nx g @tech-leads-club/skill-plugin:skill`) to create skills. Keep `SKILL.md` under 500 lines; offload reference material to `references/`.

### Skill Scripts Must Run on Windows, macOS and Linux

Every script you add to or change in a skill's `scripts/` must work unchanged on all three. Users install skills into any of the supported agents on any OS, and a script that only runs on one platform breaks the skill for everyone else.

- **Language**: Python 3 (standard library only) or Node.js (`.mjs`, no dependencies). No new Bash, PowerShell or other shell-only scripts. The existing `.sh` scripts are legacy: port them when you touch them.
- **Invocation**: `SKILL.md` says how to run each script, defines `<skill-dir>` as the directory containing the `SKILL.md`, and gives the Windows command alongside (`python` or `py -3`, since `python3` may be the Microsoft Store alias).
- **Paths**: build them with `pathlib`/`os.path` (Python) or `node:path` (Node). Never hard-code `/` or `\` separators, `/tmp`, `~` or home-directory layouts.
- **Text I/O**: open files with an explicit `encoding="utf-8"`, accept both LF and CRLF, and make stdout safe for non-ASCII output (`sys.stdout.reconfigure(encoding="utf-8", errors="replace")` in Python), because Windows writes piped output in the ANSI code page and crashes on characters like `→`.
- **Subprocesses**: pass arguments as a list, never through a shell string (`shell=True`, `exec`). Do not assume `bash`, `grep`, `sed`, `which` or other Unix tools exist; resolve executables with `shutil.which` and handle their absence.
- **Exit codes**: `0` for success, non-zero for findings or failure, with a message that says which.
- **Verify**: run the script on the platforms you can, and at least simulate Windows output with `PYTHONIOENCODING=cp1252` (without `-I`, which ignores it) and a CRLF input file.

### Skill Quality Standards

**When creating a new skill or modifying an existing one, you MUST use the `skill-architect` skill** to ensure it follows best practices. If the `skill-architect` skill is not installed, follow these mandatory description rules:

- **Structure**: `[What it does] + [Use when ...] + [Do NOT use for ...]`
- **"Use when"** is mandatory — include actual phrases users would say
- **"Do NOT use for"** is mandatory — add negative triggers to prevent overlap with similar skills
- **Under 1024 characters** — no XML angle brackets (`< >`)
- **User perspective** — write trigger phrases as things the user would actually say, not internal jargon

## Release

Nx Release with conventional commits. `chore`, `test`, `style`, `build`, `ci` prefixes do not trigger version bumps. Security scan must pass before release (`npm run scan`). Two independent release groups: `cli` and `skills-catalog`.

================================================================
File: CLAUDE.md
================================================================
AGENTS.md

================================================================
File: CONTRIBUTING.md
================================================================
# 🤝 Contributing to Agent Skills

First of all, thank you for taking the time to contribute! 🎉

> **Note**: This document covers how contributions are accepted, how to set up your environment, how the project is laid out, and how to create a skill.

Start with the section below — it takes a minute and tells you whether to open an issue or a pull request. Everything after it is setup and reference.

## 🤝 How contributions work

We use an **issue-first** flow. Anyone can open an issue. Pull requests come from Tech Leads Club members.

### Why

This repository was receiving a steady stream of automated pull requests. At volume, a machine-generated PR is not reliably distinguishable from a large human one, and reviewing them was consuming the time that should go into reviewing real contributions. Rather than judge submissions case by case — which is where good contributors get caught in the crossfire — we moved the entry point to issues, where a short exchange settles intent before anyone writes code.

This is not a closed project, and it is not about the quality of your work. Every issue gets read and answered. What changed is the order: talk first, code second.

### If you are not a member

1. **Open an issue** describing what you want to add or change. For a new skill, say what it does, when an agent should reach for it, and which existing skill it might overlap with.
2. **A maintainer replies.** If the idea fits, we say so explicitly, and then either:
   - **you implement it** — we grant you the access needed to open the PR, or
   - **a maintainer implements it** — and you are credited (see below).
3. **Trivial fixes still start with an issue** — a typo, a broken link, a wrong command. Those get resolved quickly; we are not going to make you negotiate over a missing letter.

### If you are a member

Open the pull request directly. Branch, conventional commits, PR — the steps at the end of this section.

### Credit

An issue-first policy is only fair if the person who had the idea is named. When your issue leads to a change:

- your GitHub handle goes in the skill's `metadata.author` field when the contribution is a skill,
- the issue is linked from the commit that implements it, and
- you are credited in the release notes for that version.

If a maintainer implements your proposal and the credit is missing, say so on the issue. That is a mistake on our side, not a favour to ask for.

### AI-assisted contributions

Using an agent to help is fine and expected — this repository is built for agents. Two conditions: you understand and can defend every line you submit, and you say in the issue or PR that an agent was involved. An unreviewed agent output submitted as your own work is the thing this policy exists to filter.

### Steps, once you have the go-ahead

1. **Fork** the repository (or branch directly, if you have write access)
2. **Create** a branch (`git checkout -b feat/amazing-skill`)
3. **Commit** with conventional commits (`git commit -m "feat: add amazing skill"`)
4. **Push** and **open a Pull Request**, linking the issue it implements
5. **Check CI** — `lint`, `test`, `build`, skill validation and the security scan all run on the PR

Pull requests that arrive without a linked issue may be closed with a pointer back to this section. That is a redirect, not a rejection.

## 🛠 Prerequisites

- **Node.js** ≥ 24 — the repo pins `24.18.1` in [`.nvmrc`](.nvmrc), so `nvm use` picks the right one
- **npm** (comes with Node.js)

> **Do not set `NODE_ENV` in your shell.** Each tool sets it for you (`test` for Jest, `production` for a Next build). A `NODE_ENV=development` exported in your profile makes `nx build marketplace` fail with confusing React errors such as `Cannot read properties of null (reading 'useContext')` during prerender. If a build fails only on your machine, check `echo $NODE_ENV` first.

## 🚀 Setup

```bash
git clone https://github.com/tech-leads-club/agent-skills.git
cd agent-skills
npm ci
npm run build
```

## 💻 Development Commands

| Command                            | Description                                  |
| ---------------------------------- | -------------------------------------------- |
| `npm run start:dev:cli`            | Run CLI locally (interactive mode)           |
| `npm run start:dev:mcp`            | Build MCP and open Inspector                 |
| `npm run generate:skill <name>`    | Generate a new skill                         |
| `npm run generate:data`            | Regenerate the registry and marketplace data |
| `npm run validate`                 | Validate all skills                          |
| `npm run build`                    | Build all packages                           |
| `npm run test`                     | Run all tests                                |
| `npm run lint`                     | Lint codebase                                |
| `npm run format`                   | Format code with Prettier                    |
| `npm run format:check`             | Check formatting without writing             |
| `npm run scan`                     | Run incremental security scan                |
| `nx run marketplace:dev`           | Run marketplace locally                      |
| `nx run marketplace:generate-data` | Update marketplace skills data               |

To reproduce what CI runs before opening anything:

```bash
npx nx affected -t lint test build --base=origin/main
npx tsx tools/validate-skills.ts --batch packages/skills-catalog/skills
```

## ⭐ Creating a New Skill

> **Important**: When creating a new skill or adding an external skill to the catalog, you **must** use the **`skill-architect`** skill to guide the process and ensure the skill follows our quality standards. If you're an AI agent, load the `skill-architect` skill before proceeding. If contributing manually, review the [Description Quality Standards](#description-quality-standards) below.

```bash
# With category (recommended)
nx g @tech-leads-club/skill-plugin:skill my-skill --category=development

# Full options
nx g @tech-leads-club/skill-plugin:skill my-skill \
  --description="What my skill does" \
  --category=development \
  --author="github.com/username" \
  --skillVersion="1.0.0"
```

The generator creates:

- `packages/skills-catalog/skills/(development)/my-skill/SKILL.md`

After generating the scaffold, refine the `SKILL.md` content (especially the `description` field) following the quality standards below.

## 📁 Project Structure

```
agent-skills/
├── packages/
│   ├── cli/                          # @tech-leads-club/agent-skills — installs skills into agents
│   ├── mcp/                          # @tech-leads-club/agent-skills-mcp — serves skills over MCP
│   ├── marketplace/                  # Next.js static site for the skill registry
│   └── skills-catalog/               # Skills collection
│       ├── skills/                   # All skill definitions
│       │   ├── (category-name)/      # Categorized skills
│       │   └── _category.json        # Category metadata
│       └── skills-registry.json      # Auto-generated catalog (committed)
├── libs/
│   └── core/                         # @tech-leads-club/core — shared types and services
├── tools/
│   └── skill-plugin/                 # Nx skill generator
├── .github/
│   └── workflows/                    # CI/CD pipelines
└── nx.json                           # Nx configuration
```

There are **two** ways a skill reaches an agent, and both read the same catalog:

- **CLI** — installs skill files into the agent's directory (`.agents/`, `~/.cursor/skills/`, …) and records them in a lockfile.
- **MCP** — serves skills on demand over the Model Context Protocol, with no installation.

A change to a skill's files affects both. See [Contributing to the MCP server](#-contributing-to-the-mcp-server) if you are touching `packages/mcp`.

## 📝 Skill Structure

```
packages/skills-catalog/skills/
├── (category-name)/              # Category folder
│   └── my-skill/                 # Skill folder
│       ├── SKILL.md              # Required: main instructions
│       ├── references/           # Optional: docs the agent reads on demand
│       ├── scripts/              # Optional: executable scripts
│       └── assets/               # Optional: templates and files used in output
└── _category.json                # Category metadata
```

> **Prefer these three names**, so a reader can guess what a folder holds without opening it: `references/` for docs the agent reads, `scripts/` for things it runs, `assets/` for files that end up in the output. They are a convention, not a constraint — every file listed for a skill in the registry is installed by the CLI and served over MCP, whatever folder it sits in. Some existing skills use `rules/` or `templates/` for good reasons.

### SKILL.md Format

```markdown
---
name: my-skill
description: What this skill does in one sentence. Use when user says "trigger phrase", "another trigger", or "third trigger". Do NOT use for things handled by other-skill.
metadata:
  version: 1.0.0
  author: github.com/username
---

# My Skill

Brief description.

## Process

1. Step one
2. Step two
```

### Category Metadata

`_category.json`:

```json
{
  "(development)": {
    "name": "Development",
    "description": "Skills for software development",
    "priority": 1
  }
}
```

### Best Practices

- **Keep SKILL.md under 500 lines** — use `references/` for detailed docs
- **Write specific descriptions** — include trigger phrases
- **Assume the agent is smart** — only add what it doesn't already know
- **Prefer scripts over inline code** — reduces context window usage
- **Use the `skill-architect` skill** — for creating new skills or validating existing ones

### Description Quality Standards

Every skill description **must** follow this structure:

```
[What it does] + [Use when ...] + [Do NOT use for ...]
```

**Mandatory rules:**

| Rule                                                | Example                                                   |
| --------------------------------------------------- | --------------------------------------------------------- |
| Include `Use when` with user-facing trigger phrases | `Use when user says "deploy my app", "push this live"`    |
| Include `Do NOT use for` with negative triggers     | `Do NOT use for Netlify deployments (use netlify-deploy)` |
| Under 1024 characters                               | Keep it concise but complete                              |
| No XML angle brackets (`< >`) in YAML               | Use standard quotes instead                               |
| User perspective, not internal jargon               | "fix my build" not "remediate CI pipeline failures"       |

**Good example:**

```yaml
description: Deploy applications to Vercel. Use when the user requests "deploy my app",
  "push this live", or "create a preview deployment". Do NOT use for deploying to
  Netlify, Cloudflare, or Render (use their respective skills).
```

**Bad example:**

```yaml
# ❌ Missing triggers and negative scope
description: Helps with deployments.
```

## 🔒 Security Scan

Every skill is scanned with [Snyk Agent Scan](https://github.com/snyk/agent-scan) before publishing. The scan is **incremental** — only skills whose content changed since the last run are re-scanned.

```bash
npm run scan              # Incremental (default); requires SNYK_TOKEN
npm run scan -- --force   # Force full re-scan
```

### How it works

Each skill has a SHA-256 content hash (computed from all its files). Results are cached in `.security-scan-cache.json` (gitignored). On the next run, skills whose hash hasn't changed skip re-scanning and load results from cache.

```
Content hash unchanged → load from cache (fast)
Content hash changed   → re-scan with snyk-agent-scan
```

### When CI fails on Security Scan

1. **Open the run** → In the "CI Checks" job you’ll see a step **"Print scan failure summary"** (and/or **"Security Scan"**) with Critical/High counts and affected skills + codes (e.g. `frontend-design: W011`).
2. **Same-repo PRs** → A bot comment on the PR lists the same findings and links to the run.
3. **Fix it:**
   - **Real issue** → Adjust the skill (remove or restrict the flagged behavior).
   - **False positive** → Add an entry to `packages/skills-catalog/security-scan-allowlist.yaml` (see below). Match by `skill` + `code`; add a short `reason` and `allowedBy`/`allowedAt`.
4. **Run locally** (optional): `SNYK_TOKEN=<your-token> npm run scan` to confirm before pushing. PRs from forks don’t run the scan in CI (no secrets); use Merge Queue or run the scan locally.

### Handling false positives

If the scanner flags a finding that is intentional (e.g. a first-party MCP server integration), add it to the allowlist:

**`packages/skills-catalog/security-scan-allowlist.yaml`**

```yaml
version: '1.0.0'

entries:
  - skill: my-skill
    code: W011
    reason: >
      Fetches from trusted first-party API — expected behavior.
    allowedBy: github.com/username
    allowedAt: '2026-01-01'
    expiresAt: '2027-01-01' # Optional but recommended
```

- Match is by `skill + code` — no re-scan needed after adding an entry
- `expiresAt` is optional but recommended — forces periodic review
- Expired entries re-activate the finding automatically
- Use YAML for better readability, comments, and cleaner diffs

The allowlist is committed to the repo and reviewable in PRs.

## 🔄 Release Process

This project uses **Conventional Commits** for automated versioning:

| Commit Prefix | Version Bump  | Example                      |
| ------------- | ------------- | ---------------------------- |
| `feat:`       | Minor (0.X.0) | `feat: add new skill`        |
| `fix:`        | Patch (0.0.X) | `fix: correct symlink path`  |
| `feat!:`      | Major (X.0.0) | `feat!: breaking API change` |
| `docs:`       | No bump       | `docs: update README`        |
| `chore:`      | No bump       | `chore: update deps`         |

Releases are automated via GitHub Actions when merging to `main`.

## 🛍️ Contributing to the Marketplace

The Agent Skills Marketplace is a Next.js static site located in `packages/marketplace`. It serves as the frontend for browsing and discovering agent skills.

To work on the marketplace locally:

```bash
# Parse SKILL.md files and generate the JSON data used by the UI
nx run marketplace:generate-data

# Start the development server (runs with production config matching static export)
nx run marketplace:dev
```

Open `http://localhost:3000` in your browser. For more details on the marketplace architecture, SEO optimization, and Next.js setup, see the [Marketplace README](packages/marketplace/README.md).

## 🔌 Contributing to the MCP Server

`packages/mcp` serves the catalog over the Model Context Protocol. It exposes five tools, arranged so each step of the agent's workflow only pays for what it needs:

| Tool                  | Step                       | Returns                                              |
| --------------------- | -------------------------- | ---------------------------------------------------- |
| `search_skills`       | 1 — find a skill by intent | ranked matches with a short `usage_hint`             |
| `list_skills`         | browse (explicit ask only) | the whole catalog, grouped by category               |
| `read_skill`          | 2 — load instructions      | the `SKILL.md` body plus the list of bundled files   |
| `fetch_skill_files`   | 3 — files meant to be read | the text of `references/` files                      |
| `prepare_skill_files` | 3 — files meant to be run  | `file://` links to files written to the user's cache |

Two conventions hold when changing this package:

- **Keep tool modules thin; put logic in `src/tools/core/`.** Core functions are pure and directly unit-tested. Tool behaviour is tested through the real server: `src/__tests__/server.test.ts` connects to `createServer` in-process with a fake `Catalog`, and `src/__tests__/process.e2e.test.ts` drives the built `dist/packages/mcp/main.js` over stdio. `nx test` builds the package first.
- **Anything that costs the agent tokens or touches the filesystem needs a reason in the code.** The response budget, the frontmatter strip and the revision-keyed staging directory all exist for a measured reason recorded next to them; keep that up to date if you change the behaviour.

`npm run start:dev:mcp` builds the server and opens the MCP Inspector against it.
