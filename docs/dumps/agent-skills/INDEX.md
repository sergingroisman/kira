---
type: Dump
title: gitingest — tech-leads-club/agent-skills
- **Fonte**: https://github.com/tech-leads-club/agent-skills @ commit e74dc8a
- **Licença**: código MIT (Copyright (c) 2026 Tech Leads Club); conteúdo dos SKILL.md
  CC-BY-4.0 — atribuição: Tech Leads Club + autor indicado em `metadata.author` de cada skill.
- **Gerado por**: `docs/dumps/generate-agent-skills-dump.ps1` — caminhada ordenada por caminho;
  mesma entrada = mesmo dump. Fora de escopo: `references/`, `templates/`, código CLI/MCP.
---

# gitingest — agent-skills — índice (94 SKILL.md, 1328509 bytes)

| categoria | skill | versão | autor | bytes | descrição (trecho) |
|---|---|---|---|---|---|
| (architecture) | [component-common-domain-detection](architecture.md) | — | — | 18066 | Finds duplicate business logic spread across multiple components and suggests consolidation. Use when asking "where is this logic duplicated?", "find common cod… |
| (architecture) | [component-flattening-analysis](architecture.md) | — | — | 18804 | Detects misplaced classes and fixes component hierarchy problems — finds code that should belong inside a component but sits at the root level. Use when asking … |
| (architecture) | [component-identification-sizing](architecture.md) | — | — | 14310 | Maps architectural components in a codebase and measures their size to identify what should be extracted first. Use when asking "how big is each module?", "what… |
| (architecture) | [coupling-analysis](architecture.md) | — | — | 16219 | Analyzes coupling between modules using the three-dimensional model (strength, distance, volatility) from "Balancing Coupling in Software Design". Use when aski… |
| (architecture) | [decomposition-planning-roadmap](architecture.md) | — | — | 17922 | Creates step-by-step decomposition plans and migration roadmaps for breaking apart monolithic applications. Use when asking "what order should I extract service… |
| (architecture) | [domain-analysis](architecture.md) | — | — | 12198 | Maps business domains and suggests service boundaries in any codebase using DDD Strategic Design. Use when asking "what are the domains in this codebase?", "whe… |
| (architecture) | [domain-identification-grouping](architecture.md) | — | — | 22111 | Groups existing components into logical business domains to plan service-based architecture. Use when asking "which components belong together?", "group these i… |
| (architecture) | [evolutionary-modular-architecture](architecture.md) | — | — | 15264 | Guides design and implementation of evolutionary modular-monolith platforms with DDD (strategic + tactical), flat-by-aggregate organization, an Anti-Corruption … |
| (architecture) | [frontend-blueprint](architecture.md) | — | — | 21793 | AI frontend specialist and design consultant that guides users through a structured discovery process before generating any code. Collects visual references, de… |
| (architecture) | [legacy-migration-planner](architecture.md) | — | — | 7523 | Use when planning legacy system migrations, codebase modernization, monolith decomposition, microservices consolidation, cross-language rewrites, or framework u… |
| (architecture) | [modular-decomposition](architecture.md) | — | — | 7067 | Runs a sequenced monolith-to-modular pipeline that sizes and inventories components, finds shared domain duplication, addresses flattening and hierarchy issues,… |
| (architecture) | [modular-design-principles](architecture.md) | — | — | 12634 | > |
| (architecture) | [react-composition-patterns](architecture.md) | — | — | 3049 | React composition patterns that scale. Use when refactoring components with boolean prop proliferation, building flexible component libraries, or designing reus… |
| (architecture) | [tactical-ddd](architecture.md) | — | — | 4055 | Detects anemic domain models, validates and refactors them into rich domain models, and enforces tactical DDD patterns (Entities, Value Objects, Aggregates, Dom… |
| (cloud) | [aws-advisor](cloud.md) | — | — | 8557 | Expert AWS Cloud Advisor for architecture design, security review, and implementation guidance. Leverages AWS MCP tools for accurate, documentation-backed answe… |
| (cloud) | [cloudflare-deploy](cloud.md) | — | — | 9086 | Deploy applications and infrastructure to Cloudflare using Workers, Pages, and related platform services. Use when the user asks to deploy, host, publish, or se… |
| (cloud) | [netlify-deploy](cloud.md) | — | — | 7432 | Deploy web projects to Netlify using the Netlify CLI (`npx netlify`). Use when the user asks to deploy, host, publish, or link a site/repo on Netlify, including… |
| (cloud) | [render-deploy](cloud.md) | — | — | 19523 | Deploy applications to Render by analyzing codebases, generating render.yaml Blueprints, and providing Dashboard deeplinks. Use when the user wants to deploy, h… |
| (cloud) | [vercel-deploy](cloud.md) | — | — | 2429 | Deploy applications and websites to Vercel. Use when the user requests deployment actions like "deploy my app", "deploy and give me the link", "push this live",… |
| (creation) | [create-adr](creation.md) | — | — | 14422 | Creates Architecture Decision Records (ADRs) to document significant architectural choices and their rationale for future team members. Use when the user says "… |
| (creation) | [create-rfc](creation.md) | — | — | 11846 | Creates structured Request for Comments (RFC) documents for proposing and deciding on significant changes. Use when the user says "write an RFC", "create a prop… |
| (creation) | [create-technical-design-doc](creation.md) | — | — | 53504 | Creates comprehensive Technical Design Documents (TDD) with mandatory and optional sections through interactive discovery. Use when user asks to "write a design… |
| (creation) | [cursor-subagent-creator](creation.md) | — | — | 19971 | Creates Cursor-specific AI subagents with isolated context for complex multi-step workflows. Use when creating subagents for Cursor editor specifically, followi… |
| (creation) | [skill-architect](creation.md) | — | — | 14210 | Expert guide for designing and building high-quality skills from scratch through structured conversation. Use when someone wants to create a new skill, build a … |
| (creation) | [subagent-creator](creation.md) | — | — | 7406 | Guide for creating AI subagents with isolated context for complex multi-step workflows. Use when users want to create a subagent, specialized agent, verifier, d… |
| (decision-making) | [the-fool](decision-making.md) | — | — | 8842 | Use when challenging ideas, plans, decisions, or proposals. Invoke to play devil's advocate, run a pre-mortem, red team, stress test assumptions, audit evidence… |
| (decision-making) | [the-jury](decision-making.md) | — | — | 18260 | Use when a question, decision, plan, tradeoff, or claim needs a rigorous verdict and one perspective is not enough. Spawns a panel of 3 to 5 subagent jurors tha… |
| (design) | [figma](design.md) | — | — | 3472 | Use the Figma MCP server to fetch design context, screenshots, variables, and assets from Figma, and to translate Figma nodes into production code. Use when a t… |
| (design) | [figma-implement-design](design.md) | — | — | 11910 | Translate Figma nodes into production-ready code with 1:1 visual fidelity using the Figma MCP workflow (design context, screenshots, assets, and project-convent… |
| (design) | [frontend-design](design.md) | — | — | 10020 | Create distinctive, production-grade frontend interfaces with high design quality. Use this skill when the user asks to build web components, pages, artifacts, … |
| (design) | [web-design-guidelines](design.md) | — | — | 2100 | Review UI code for Web Interface Guidelines compliance. Use when asked to "review my UI", "check accessibility", "audit design", "review UX", or "check my site … |
| (development) | [codenavi](development.md) | — | — | 13841 | Your pathfinder for navigating unknown codebases. Investigates with precision, implements surgically, and never assumes — if it doesn't know, it says so. Mainta… |
| (development) | [coding-guidelines](development.md) | — | — | 2652 | Behavioral guidelines to reduce common LLM coding mistakes. Use when writing, modifying, or reviewing code — implementation tasks, code changes, refactoring, bu… |
| (development) | [confluence-assistant](development.md) | — | — | 5386 | Expert in Confluence operations using Atlassian MCP. Use when the user says "search Confluence", "create a Confluence page", "update a page", "find documentatio… |
| (development) | [docs-writer](development.md) | — | — | 3389 | Write, review, and edit documentation files with consistent structure, tone, and technical accuracy. Use when creating docs, reviewing markdown files, writing R… |
| (development) | [gh-address-comments](development.md) | — | — | 1442 | Address review and issue comments on the open GitHub PR for the current branch using gh CLI. Use when user says "address PR comments", "fix review feedback", "r… |
| (development) | [harness-eval](development.md) | — | — | 15857 | "Evaluate a repo agent harness (AGENTS.md, rules, skills, skill refs) for broken paths/commands, redundant instructions, and usefulness using a stack-agnostic d… |
| (development) | [jira-assistant](development.md) | — | — | 9356 | Manage Jira issues via Atlassian MCP — search, create, update, transition status, and handle sprint tasks. Auto-detects workspace configuration. Use when user s… |
| (development) | [nestjs-modular-monolith](development.md) | — | — | 16227 | Specialist in designing and implementing scalable modular monolith architectures using NestJS with DDD, Clean Architecture, and CQRS patterns. Use when building… |
| (development) | [not-your-babysitter](development.md) | — | — | 14582 | Autonomous senior-operator mode for AI agents that resolve tasks end to end without babysitting and never create new problems. The agent verifies every claim ag… |
| (development) | [rails-dev](development.md) | — | — | 8877 | "Opinionated Rails conventions: rich models, concerns, CRUD-everything, state-as-records, minimal dependencies, Minitest with fixtures. Load this skill BEFORE a… |
| (development) | [react-native-expert](development.md) | — | — | 13135 | Senior React Native and Expo engineer for building production-ready cross-platform mobile apps. Use when building React Native components, implementing navigati… |
| (development) | [shopify-developer](development.md) | — | — | 7619 | Complete Shopify development reference covering Liquid templating, OS 2.0 themes, GraphQL APIs, Hydrogen, Functions, and performance optimization (API v2026-01)… |
| (development) | [spec-driven-eval](development.md) | — | — | 35816 | Scores how completely an implementation fulfills a PRD/spec, case by case, and produces a single comparable final grade. Invoke only when explicitly named (e.g.… |
| (development) | [tlc-discover](development.md) | — | — | 64466 | 'Thinks through new or large projects: turns a vague idea into a design doc with a clear product problem and the technical solution. Use when exploring a new pr… |
| (development) | [tlc-implement](development.md) | — | — | 10431 | 'Implements work already planned - a plan.md from tlc-plan, or a small ticket - by freezing it into checks with proofs, building, and proving every check with a… |
| (development) | [tlc-plan](development.md) | — | — | 7481 | 'Turns a request, ticket, PRD, design or thread into one plan a human approves and a builder can act on without the conversation: asks every gray area until no … |
| (development) | [tlc-prd](development.md) | — | — | 15411 | 'Interviews an idea into a PRD file covering situation, problem, success, boundary, and journey states, and stops before any technical design. Use when the user… |
| (development) | [tlc-spec-driven](development.md) | — | — | 17870 | Feature planning and implementation with 4 adaptive phases (Specify, Design, Tasks, Execute). Auto-sizes depth by complexity. Asks every product decision before… |
| (development) | [tlc-spec-lean](development.md) | — | — | 10891 | 'Spec-driven feature work that freezes obligations instead of the plan: one human-reviewed plan with EARS criteria, path, entities, interface and one-way doors,… |
| (gtm) | [ai-cold-outreach](gtm.md) | — | — | 22967 | "When the user wants to build an AI-powered outreach system, write cold emails, improve deliverability, or scale personalized outreach. Also use when the user m… |
| (gtm) | [ai-pricing](gtm.md) | — | — | 14830 | "When the user wants to price an AI product, choose a charge metric, design pricing tiers, or optimize margins. Also use when the user mentions 'AI pricing,' 'u… |
| (gtm) | [ai-sdr](gtm.md) | — | — | 17987 | "When the user wants to deploy AI sales development reps, automate sales qualification, build signal-to-action routing, or design AI agent architecture for sale… |
| (gtm) | [ai-seo](gtm.md) | — | — | 22312 | "When the user wants to build programmatic SEO with AI, create competitor alternative pages, optimize for AI Overviews, or scale content production. Also use wh… |
| (gtm) | [ai-ugc-ads](gtm.md) | — | — | 20473 | "When the user wants to create UGC ad campaigns, recruit UGC creators, generate AI UGC content, or scale with user-generated content. Also use when the user men… |
| (gtm) | [content-to-pipeline](gtm.md) | — | — | 25773 | "When the user wants to turn content into revenue, build a content-led GTM motion, reverse engineer distribution, or repurpose content across platforms. Also us… |
| (gtm) | [expansion-retention](gtm.md) | — | — | 22756 | "When the user wants to reduce churn, build expansion revenue, automate customer success, or optimize net revenue retention. Also use when the user mentions 'ch… |
| (gtm) | [gtm-engineering](gtm.md) | — | — | 19363 | "When the user wants to build GTM automation with code, design workflow architectures, use AI agents for GTM tasks, or implement the 'architecture over tools' p… |
| (gtm) | [gtm-metrics](gtm.md) | — | — | 21373 | "When the user wants to define GTM metrics, build a metrics dashboard, measure pipeline efficiency, or track AI product performance. Also use when the user ment… |
| (gtm) | [lead-enrichment](gtm.md) | — | — | 21865 | "When the user wants to build data enrichment workflows, score leads against ICP, set up Clay waterfalls, or improve contact data quality. Also use when the use… |
| (gtm) | [multi-platform-launch](gtm.md) | — | — | 15877 | "When the user wants to launch a product across multiple platforms, plan a Product Hunt launch, build a waitlist, or execute a multi-channel launch strategy. Al… |
| (gtm) | [paid-creative-ai](gtm.md) | — | — | 26150 | "When the user wants to create AI-generated ad creative, test performance creative, manage creative fatigue, or optimize paid media with AI tools. Also use when… |
| (gtm) | [partner-affiliate](gtm.md) | — | — | 16117 | "When the user wants to build a partner program, launch an affiliate program, design integration partnerships, or create distribution partnerships. Also use whe… |
| (gtm) | [positioning-icp](gtm.md) | — | — | 27985 | "When the user wants to define their ideal customer profile, position an AI product, build messaging architecture, or validate product-market fit. Also use when… |
| (gtm) | [sales-motion-design](gtm.md) | — | — | 23334 | "When the user wants to choose between PLG and sales-led, design a sales motion, optimize time-to-first-value, or build a value-before-purchase experience. Also… |
| (gtm) | [social-selling](gtm.md) | — | — | 20014 | "When the user wants to sell through social media, optimize LinkedIn for sales, build DM sequences, or convert content engagement into pipeline. Also use when t… |
| (gtm) | [solo-founder-gtm](gtm.md) | — | — | 24818 | "When the user is a solo founder building their GTM motion, wants to scale without hiring, or needs to design an AI agent team for go-to-market. Also use when t… |
| (gtm) | [video-outreach](gtm.md) | — | — | 27097 | "When the user wants to build video-first cold outreach, create personalized video at scale, implement async selling, or use AI demo generation for prospecting.… |
| (learning) | [learning-opportunities](learning.md) | — | — | 5174 | Facilitates deliberate skill development during AI-assisted coding. Offers interactive learning exercises after architectural work (new files, schema changes, r… |
| (monitoring) | [sentry](monitoring.md) | — | — | 4162 | Inspect Sentry issues, summarize production errors, and pull health data via the Sentry API (read-only). Use when user says "check Sentry", "what errors in prod… |
| (performance) | [core-web-vitals](performance.md) | — | — | 12467 | Optimize Core Web Vitals (LCP, INP, CLS) for better page experience and search ranking. Use when asked to "improve Core Web Vitals", "fix LCP", "reduce CLS", "o… |
| (performance) | [perf-astro](performance.md) | — | — | 3840 | 'Astro-specific performance optimizations for 95+ Lighthouse scores. Covers critical CSS inlining, compression, font loading, and LCP optimization. Use when opt… |
| (performance) | [perf-lighthouse](performance.md) | — | — | 6640 | 'Run Lighthouse audits locally via CLI or Node API, parse and interpret reports, and set performance budgets. Use when measuring site performance, understanding… |
| (performance) | [perf-web-optimization](performance.md) | — | — | 4332 | 'Optimize web performance: bundle size, images, caching, lazy loading, and overall page speed. Use when site is slow, reducing bundle size, fixing layout shifts… |
| (quality) | [react-best-practices](quality.md) | — | — | 6574 | React and Next.js performance optimization guidelines from Vercel Engineering. Use when writing, reviewing, or refactoring React/Next.js code to ensure optimal … |
| (quality) | [seo](quality.md) | — | — | 12764 | Optimize for search engine visibility and ranking. Use when asked to "improve SEO", "optimize for search", "fix meta tags", "add structured data", "sitemap opti… |
| (quality) | [the-judge](quality.md) | — | — | 20285 | Evidence-first pull request judge that reviews a PR and posts one consolidated GitHub review with inline comments via the gh CLI. Runs the repo's own determinis… |
| (quality) | [tlc-generative-engine-optimization](quality.md) | — | — | 10281 | "Generative Engine Optimization (GEO) specialist — the technical, on-page publishing work that makes a given page or site discoverable, understandable, trustwor… |
| (quality) | [web-accessibility](quality.md) | — | — | 13624 | Audit and improve web accessibility following WCAG 2.1 guidelines. Use when asked to "improve accessibility", "a11y audit", "WCAG compliance", "screen reader su… |
| (quality) | [web-best-practices](quality.md) | — | — | 13398 | Apply modern web development best practices for security, compatibility, and code quality. Use when asked to "apply best practices", "security audit", "moderniz… |
| (quality) | [web-quality-audit](quality.md) | — | — | 7201 | Comprehensive web quality audit covering performance, accessibility, SEO, and best practices in a single review. Use when asked to "audit my site", "review web … |
| (security) | [security-best-practices](security.md) | — | — | 8748 | Perform language and framework specific security best-practice reviews and suggest improvements. Use when the user explicitly requests security best practices g… |
| (security) | [security-ownership-map](security.md) | — | — | 9042 | 'Analyze git repositories to build a security ownership topology (people-to-file), compute bus factor and sensitive-code ownership, and export CSV/JSON for grap… |
| (security) | [security-threat-model](security.md) | — | — | 5755 | Repository-grounded threat modeling that enumerates trust boundaries, assets, attacker capabilities, abuse paths, and mitigations, and writes a concise Markdown… |
| (tooling) | [chrome-devtools](tooling.md) | — | — | 5319 | Browser debugging, performance profiling, and automation via Chrome DevTools MCP. Use when user says "debug this page", "take a screenshot", "check network requ… |
| (tooling) | [excalidraw-studio](tooling.md) | — | — | 14622 | Generate Excalidraw diagrams from natural language descriptions. Outputs .excalidraw JSON files openable in Excalidraw. Use when asked to "create a diagram", "m… |
| (tooling) | [gh-fix-ci](tooling.md) | — | — | 3897 | Use when a user asks to debug or fix failing GitHub PR checks that run in GitHub Actions. Uses `gh` to inspect checks and logs, summarize failure context, draft… |
| (tooling) | [mermaid-studio](tooling.md) | — | — | 18983 | Expert Mermaid diagram creation, validation, and rendering with dual-engine output (SVG/PNG/ASCII). Supports all 20+ diagram types including C4 architecture, AW… |
| (tooling) | [nx-ci-monitor](tooling.md) | — | — | 21617 | Monitor Nx Cloud CI pipeline status and handle self-healing fixes automatically. Use when user says "watch CI", "monitor pipeline", "check CI status", "fix CI f… |
| (tooling) | [nx-generate](tooling.md) | — | — | 9101 | Generate code using Nx generators — scaffold projects, libraries, features, or run workspace-specific generators with proper discovery, validation, and verifica… |
| (tooling) | [nx-run-tasks](tooling.md) | — | — | 2709 | Execute build, test, lint, serve, and other tasks in an Nx workspace using single runs, run-many, and affected commands. Use when user says "run tests", "build … |
| (tooling) | [nx-workspace](tooling.md) | — | — | 4149 | Configure, explore, and optimize Nx monorepo workspaces. Use when setting up Nx, exploring workspace structure, configuring project boundaries, analyzing affect… |
| (web-automation) | [demo](web-automation.md) | — | — | 9097 | Record a demo video of a web app feature or flow on a real desktop (Docker, Xvfb, XFCE, real X cursor), producing an mp4 plus a click/hover/scroll/type telemetr… |
| (web-automation) | [playwright-skill](web-automation.md) | — | — | 14903 | Complete browser automation with Playwright. Auto-detects dev servers, writes clean test scripts to /tmp. Test pages, fill forms, take screenshots, check respon… |

Cada categoria está em um arquivo próprio neste diretório. Skills destacadas na
análise do KIRA: `create-adr`, `the-judge`, `the-jury`, `the-fool`, `tlc-spec-driven`,
`not-your-babysitter`, `harness-eval`, `spec-driven-eval`.
