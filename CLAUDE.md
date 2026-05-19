# Sergin Harness — Project Guide

**Core Value:** An autonomous agent system that creates, deploys, and markets niche websites to generate ad revenue.

## Quick Reference

- **Workflow**: YOLO mode — auto-approve, just execute
- **Granularity**: Standard (5-8 phases, 3-5 plans each)
- **Execution**: Parallel where independent
- **Instrastructure**: Local-first (MacBook), Hostinger for static hosting
- **Stack**: LangGraph + DeepSeek API + TypeScript (monorepo)
- **Budget**: $10-20/mo

## Commands

- `/gsd-progress` — Check phase status and advance workflow
- `/gsd-plan-phase N` — Plan a specific phase
- `/gsd-execute-phase N` — Execute all plans in a phase
- `/gsd-discuss-phase N` — Gather context and clarify approach before planning
- `/gsd-settings` — Update workflow toggles and model profile

## Key References

- `.planning/PROJECT.md` — Project context and goals
- `.planning/REQUIREMENTS.md` — 30 v1 requirements
- `.planning/ROADMAP.md` — 7 phases
- `.planning/STATE.md` — Current project state

## Architecture

```
sergin-harness/         ← Monorepo root
├── agent/              ← Sergin Agent (LangGraph CLI)
├── brain-client/       ← MCP client for sergin-brain
├── sites/              ← Site generator + templates
├── deploy/             ← Terraform configurations
├── marketing/          ← Reddit automation
└── shared/             ← Shared types and utilities
```

## External Dependencies

- `sergin-brain` — MCP server at `/Users/avanade/Workspace/sergin-brain` (NestJS/TS)
- `claude-mythos` — Claude Code reference at `/Users/avanade/Workspace/claude-mythos`
- DeepSeek API — Primary LLM engine
- Hostinger — Static site hosting
- Cloudflare — DNS + CDN

## GSD Workflow Notes

- Research agents not installed — run `npx get-shit-done-cc@latest --global` to enable
- Git root is the parent workspace (`/Users/avanade/Workspace`)
- Planning docs are committed to the workspace git repo
