Directory structure:
└── sergingroisman-gsd-2/
    ├── CONTEXT.md
    ├── CONTRIBUTING.md
    ├── Dockerfile
    ├── LICENSE
    ├── package.json
    ├── tsconfig.extensions.json
    ├── tsconfig.json
    ├── tsconfig.resources.json
    ├── tsconfig.test.json
    ├── VISION.md
    ├── .dockerignore
    ├── .npmignore
    ├── .npmrc
    ├── .prompt-injection-scanignore
    ├── .secretscanignore
    ├── docker/
    │   ├── README.md
    │   ├── bootstrap.sh
    │   ├── docker-compose.full.yaml
    │   ├── docker-compose.yaml
    │   ├── Dockerfile.ci-builder
    │   ├── Dockerfile.sandbox
    │   ├── entrypoint.sh
    │   └── .env.example
    ├── docs/
    │   ├── README.md
    │   ├── db-map.md
    │   ├── prompt-db-combined-map.md
    │   ├── prompt-map.md
    │   ├── token-consumption-savings-evidence.md
    │   ├── adr/
    │   │   └── .gitkeep
    │   ├── agents/
    │   │   ├── domain.md
    │   │   ├── issue-tracker.md
    │   │   └── triage-labels.md
    │   ├── dev/
    │   │   ├── 2026-04-24-swarm-delivery-implementation-plan.md
    │   │   ├── 2026-05-03-long-running-refactor-plan-of-plans.md
    │   │   ├── ADR-001-branchless-worktree-architecture.md
    │   │   ├── ADR-002-external-state-directory.md
    │   │   ├── ADR-004-capability-aware-model-routing.md
    │   │   ├── ADR-005-multi-model-provider-tool-strategy.md
    │   │   ├── ADR-006-extension-modularization.md
    │   │   ├── ADR-007-model-catalog-split.md
    │   │   ├── ADR-008-gsd-tools-over-mcp-for-provider-parity.md
    │   │   ├── ADR-008-IMPLEMENTATION-PLAN.md
    │   │   ├── ADR-009-IMPLEMENTATION-PLAN.md
    │   │   ├── ADR-009-orchestration-kernel-refactor.md
    │   │   ├── ADR-010-pi-clean-seam-architecture.md
    │   │   ├── ADR-011-progressive-planning-escalation.md
    │   │   ├── ADR-012-provider-id-vs-api-shape.md
    │   │   ├── ADR-013-memory-store-consolidation.md
    │   │   ├── ADR-014-auto-orchestration-deep-module.md
    │   │   ├── ADR-015-runtime-invariant-modules.md
    │   │   ├── ADR-016-phase-2-design.md
    │   │   ├── ADR-016-worktree-lifecycle-and-projection.md
    │   │   ├── ADR-016-worktree-safety-fail-closed.md
    │   │   ├── ADR-017-state-reconciliation-drift-driven.md
    │   │   ├── ADR-018-project-authority-contract.md
    │   │   ├── agent-knowledge-index.md
    │   │   ├── architecture.md
    │   │   ├── ci-cd-pipeline.md
    │   │   ├── FRONTIER-TECHNIQUES.md
    │   │   ├── pi-context-optimization-opportunities.md
    │   │   ├── PRD-branchless-worktree-architecture.md
    │   │   ├── PRD-pi-clean-seam-refactor.md
    │   │   ├── refactor-baseline-runbook.md
    │   │   ├── tui-recommended-design.html
    │   │   ├── warp-auto-disconnect-findings.md
    │   │   ├── building-coding-agents/
    │   │   │   ├── README.md
    │   │   │   ├── 01-work-decomposition.md
    │   │   │   ├── 02-what-to-keep-discard-from-human-engineering.md
    │   │   │   ├── 03-state-machine-context-management.md
    │   │   │   ├── 04-optimal-storage-for-project-context.md
    │   │   │   ├── 05-parallelization-strategy.md
    │   │   │   ├── 06-maximizing-agent-autonomy-superpowers.md
    │   │   │   ├── 07-system-prompt-llm-vs-deterministic-split.md
    │   │   │   ├── 08-speed-optimization.md
    │   │   │   ├── 09-top-10-tips-for-a-world-class-agent.md
    │   │   │   ├── 10-top-10-pitfalls-to-avoid.md
    │   │   │   ├── 11-god-tier-context-engineering.md
    │   │   │   ├── 12-handling-ambiguity-contradiction.md
    │   │   │   ├── 13-long-running-memory-fidelity.md
    │   │   │   ├── 14-multi-agent-semantic-conflict-resolution.md
    │   │   │   ├── 15-legacy-code-brownfield-onboarding.md
    │   │   │   ├── 16-encoding-taste-aesthetics.md
    │   │   │   ├── 17-irreversible-operations-safety-architecture.md
    │   │   │   ├── 18-the-handoff-problem-agent-human-maintainability.md
    │   │   │   ├── 19-when-to-scrap-and-start-over.md
    │   │   │   ├── 20-error-taxonomy-routing.md
    │   │   │   ├── 21-cost-quality-tradeoff-model-routing.md
    │   │   │   ├── 22-cross-project-learning-reusable-intelligence.md
    │   │   │   ├── 23-evolution-across-project-scale.md
    │   │   │   ├── 24-security-trust-boundaries.md
    │   │   │   ├── 25-designing-for-non-technical-users-vibe-coders.md
    │   │   │   └── 26-cross-cutting-themes-where-all-4-models-converge.md
    │   │   ├── context-and-hooks/
    │   │   │   ├── README.md
    │   │   │   ├── 01-the-context-pipeline.md
    │   │   │   ├── 02-hook-reference.md
    │   │   │   ├── 03-context-injection-patterns.md
    │   │   │   ├── 04-message-types-and-llm-visibility.md
    │   │   │   ├── 05-inter-extension-communication.md
    │   │   │   ├── 06-advanced-patterns-from-source.md
    │   │   │   └── 07-the-system-prompt-anatomy.md
    │   │   ├── extending-pi/
    │   │   │   ├── README.md
    │   │   │   ├── 01-what-are-extensions.md
    │   │   │   ├── 02-architecture-mental-model.md
    │   │   │   ├── 03-getting-started.md
    │   │   │   ├── 04-extension-locations-discovery.md
    │   │   │   ├── 05-extension-structure-styles.md
    │   │   │   ├── 06-the-extension-lifecycle.md
    │   │   │   ├── 07-events-the-nervous-system.md
    │   │   │   ├── 08-extensioncontext-what-you-can-access.md
    │   │   │   ├── 09-extensionapi-what-you-can-do.md
    │   │   │   ├── 10-custom-tools-giving-the-llm-new-abilities.md
    │   │   │   ├── 11-custom-commands-user-facing-actions.md
    │   │   │   ├── 12-custom-ui-visual-components.md
    │   │   │   ├── 13-state-management-persistence.md
    │   │   │   ├── 14-custom-rendering-controlling-what-the-user-sees.md
    │   │   │   ├── 15-system-prompt-modification.md
    │   │   │   ├── 16-compaction-session-control.md
    │   │   │   ├── 17-model-provider-management.md
    │   │   │   ├── 18-remote-execution-tool-overrides.md
    │   │   │   ├── 19-packaging-distribution.md
    │   │   │   ├── 20-mode-behavior.md
    │   │   │   ├── 21-error-handling.md
    │   │   │   ├── 22-key-rules-gotchas.md
    │   │   │   ├── 23-file-reference-documentation.md
    │   │   │   ├── 24-file-reference-example-extensions.md
    │   │   │   ├── 25-slash-command-subcommand-patterns.md
    │   │   │   ├── 26-extension-manifest-spec.md
    │   │   │   └── 27-testing-extensions.md
    │   │   ├── pi-ui-tui/
    │   │   │   ├── README.md
    │   │   │   ├── 01-the-ui-architecture.md
    │   │   │   ├── 02-the-component-interface-foundation-of-everything.md
    │   │   │   ├── 03-entry-points-how-ui-gets-on-screen.md
    │   │   │   ├── 04-built-in-dialog-methods.md
    │   │   │   ├── 05-persistent-ui-elements.md
    │   │   │   ├── 06-ctx-ui-custom-full-custom-components.md
    │   │   │   ├── 07-built-in-components-the-building-blocks.md
    │   │   │   ├── 08-high-level-components-from-pi-coding-agent.md
    │   │   │   ├── 09-keyboard-input-how-to-handle-keys.md
    │   │   │   ├── 10-line-width-the-cardinal-rule.md
    │   │   │   ├── 11-theming-colors-and-styles.md
    │   │   │   ├── 12-overlays-floating-modals-and-panels.md
    │   │   │   ├── 13-custom-editors-replacing-the-input.md
    │   │   │   ├── 14-tool-rendering-custom-tool-display.md
    │   │   │   ├── 15-message-rendering-custom-message-display.md
    │   │   │   ├── 16-performance-caching-and-invalidation.md
    │   │   │   ├── 17-theme-changes-and-invalidation.md
    │   │   │   ├── 18-ime-support-the-focusable-interface.md
    │   │   │   ├── 19-building-a-complete-component-step-by-step.md
    │   │   │   ├── 20-real-world-patterns-from-examples.md
    │   │   │   ├── 21-common-mistakes-and-how-to-avoid-them.md
    │   │   │   ├── 22-quick-reference-all-ui-apis.md
    │   │   │   └── 23-file-reference-example-extensions-with-ui.md
    │   │   ├── proposals/
    │   │   │   ├── 698-browser-tools-feature-additions.md
    │   │   │   ├── rfc-gitops-branching-strategy.md
    │   │   │   └── workflows/
    │   │   │       ├── README.md
    │   │   │       ├── backmerge.yml
    │   │   │       ├── create-release.yml
    │   │   │       └── sync-next.yml
    │   │   ├── superpowers/
    │   │   │   ├── plans/
    │   │   │   │   └── 2026-03-17-cicd-pipeline.md
    │   │   │   └── specs/
    │   │   │       └── 2026-03-17-cicd-pipeline-design.md
    │   │   └── what-is-pi/
    │   │       ├── README.md
    │   │       ├── 01-what-pi-is.md
    │   │       ├── 02-design-philosophy.md
    │   │       ├── 03-the-four-modes-of-operation.md
    │   │       ├── 04-the-architecture-how-everything-fits-together.md
    │   │       ├── 05-the-agent-loop-how-pi-thinks.md
    │   │       ├── 06-tools-how-pi-acts-on-the-world.md
    │   │       ├── 07-sessions-memory-that-branches.md
    │   │       ├── 08-compaction-how-pi-manages-context-limits.md
    │   │       ├── 09-the-customization-stack.md
    │   │       ├── 10-providers-models-multi-model-by-default.md
    │   │       ├── 11-the-interactive-tui.md
    │   │       ├── 12-the-message-queue-talking-while-pi-thinks.md
    │   │       ├── 13-context-files-project-instructions.md
    │   │       ├── 14-the-sdk-rpc-embedding-pi.md
    │   │       ├── 15-pi-packages-the-ecosystem.md
    │   │       ├── 16-why-pi-matters-what-makes-it-different.md
    │   │       ├── 17-file-reference-all-documentation.md
    │   │       ├── 18-quick-reference-commands-shortcuts.md
    │   │       └── 19-building-branded-apps-on-top-of-pi.md
    │   ├── extension-sdk/
    │   │   ├── README.md
    │   │   ├── api-reference.md
    │   │   ├── building-extensions.md
    │   │   ├── manifest-spec.md
    │   │   ├── rules.md
    │   │   └── testing.md
    │   ├── user-docs/
    │   │   ├── auto-mode.md
    │   │   ├── captures-triage.md
    │   │   ├── claude-code-auth-compliance.md
    │   │   ├── commands.md
    │   │   ├── configuration.md
    │   │   ├── cost-management.md
    │   │   ├── custom-models.md
    │   │   ├── debug.md
    │   │   ├── dynamic-model-routing.md
    │   │   ├── eval-review.md
    │   │   ├── getting-started.md
    │   │   ├── git-strategy.md
    │   │   ├── hooks.md
    │   │   ├── migration.md
    │   │   ├── node-lts-macos.md
    │   │   ├── parallel-orchestration.md
    │   │   ├── providers.md
    │   │   ├── remote-questions.md
    │   │   ├── skills.md
    │   │   ├── subagents.md
    │   │   ├── token-optimization.md
    │   │   ├── troubleshooting.md
    │   │   ├── visualizer.md
    │   │   ├── web-interface.md
    │   │   └── working-in-teams.md
    │   └── zh-CN/
    │       ├── README.md
    │       └── user-docs/
    │           ├── auto-mode.md
    │           ├── captures-triage.md
    │           ├── claude-code-auth-compliance.md
    │           ├── commands.md
    │           ├── configuration.md
    │           ├── cost-management.md
    │           ├── custom-models.md
    │           ├── debug.md
    │           ├── dynamic-model-routing.md
    │           ├── getting-started.md
    │           ├── git-strategy.md
    │           ├── migration.md
    │           ├── node-lts-macos.md
    │           ├── parallel-orchestration.md
    │           ├── providers.md
    │           ├── remote-questions.md
    │           ├── skills.md
    │           ├── token-optimization.md
    │           ├── troubleshooting.md
    │           ├── visualizer.md
    │           ├── web-interface.md
    │           └── working-in-teams.md
    ├── extensions/
    │   └── google-search/
    │       ├── extension-manifest.json
    │       ├── index.ts
    │       └── package.json
    ├── gitbook/
    │   ├── README.md
    │   ├── SUMMARY.md
    │   ├── configuration/
    │   │   ├── custom-models.md
    │   │   ├── git-settings.md
    │   │   ├── mcp-servers.md
    │   │   ├── notifications.md
    │   │   ├── preferences.md
    │   │   └── providers.md
    │   ├── core-concepts/
    │   │   ├── auto-mode.md
    │   │   ├── project-structure.md
    │   │   └── step-mode.md
    │   ├── features/
    │   │   ├── captures.md
    │   │   ├── cost-management.md
    │   │   ├── debug.md
    │   │   ├── dynamic-model-routing.md
    │   │   ├── github-sync.md
    │   │   ├── headless.md
    │   │   ├── parallel.md
    │   │   ├── remote-questions.md
    │   │   ├── skills.md
    │   │   ├── teams.md
    │   │   ├── token-optimization.md
    │   │   ├── visualizer.md
    │   │   ├── web-interface.md
    │   │   └── workflow-templates.md
    │   ├── getting-started/
    │   │   ├── choosing-a-model.md
    │   │   ├── first-project.md
    │   │   └── installation.md
    │   └── reference/
    │       ├── cli-flags.md
    │       ├── commands.md
    │       ├── environment-variables.md
    │       ├── keyboard-shortcuts.md
    │       ├── migration.md
    │       └── troubleshooting.md
    ├── gsd-orchestrator/
    │   ├── SKILL.md
    │   ├── references/
    │   │   ├── answer-injection.md
    │   │   ├── commands.md
    │   │   └── json-result.md
    │   ├── templates/
    │   │   └── spec.md
    │   └── workflows/
    │       ├── build-from-spec.md
    │       ├── monitor-and-poll.md
    │       └── step-by-step.md
    ├── mintlify-docs/
    │   ├── docs.json
    │   ├── getting-started.mdx
    │   ├── introduction.mdx
    │   └── guides/
    │       ├── auto-mode.mdx
    │       ├── captures-triage.mdx
    │       ├── change-management.mdx
    │       ├── commands.mdx
    │       ├── configuration.mdx
    │       ├── cost-management.mdx
    │       ├── custom-models.mdx
    │       ├── debug.mdx
    │       ├── dynamic-model-routing.mdx
    │       ├── git-strategy.mdx
    │       ├── migration.mdx
    │       ├── parallel-orchestration.mdx
    │       ├── remote-questions.mdx
    │       ├── skills.mdx
    │       ├── token-optimization.mdx
    │       ├── troubleshooting.mdx
    │       ├── visualizer.mdx
    │       ├── web-interface.mdx
    │       └── working-in-teams.mdx
    ├── native/
    │   ├── README.md
    │   ├── Cargo.toml
    │   ├── .npmignore
    │   ├── crates/
    │   │   ├── ast/
    │   │   │   ├── Cargo.toml
    │   │   │   └── src/
    │   │   │       ├── ast.rs
    │   │   │       ├── glob_util.rs
    │   │   │       ├── lib.rs
    │   │   │       └── language/
    │   │   │           ├── mod.rs
    │   │   │           └── parsers.rs
    │   │   ├── engine/
    │   │   │   ├── build.rs
    │   │   │   ├── Cargo.toml
    │   │   │   └── src/
    │   │   │       ├── ast.rs
    │   │   │       ├── clipboard.rs
    │   │   │       ├── diff.rs
    │   │   │       ├── fd.rs
    │   │   │       ├── fs_cache.rs
    │   │   │       ├── glob.rs
    │   │   │       ├── glob_util.rs
    │   │   │       ├── grep.rs
    │   │   │       ├── highlight.rs
    │   │   │       ├── html.rs
    │   │   │       ├── image.rs
    │   │   │       ├── json_parse.rs
    │   │   │       ├── lib.rs
    │   │   │       ├── ps.rs
    │   │   │       ├── stream_process.rs
    │   │   │       ├── task.rs
    │   │   │       ├── text.rs
    │   │   │       ├── truncate.rs
    │   │   │       ├── ttsr.rs
    │   │   │       └── xxhash.rs
    │   │   └── grep/
    │   │       ├── Cargo.toml
    │   │       └── src/
    │   │           └── lib.rs
    │   ├── npm/
    │   │   ├── darwin-arm64/
    │   │   │   └── package.json
    │   │   ├── darwin-x64/
    │   │   │   └── package.json
    │   │   ├── linux-arm64-gnu/
    │   │   │   └── package.json
    │   │   ├── linux-x64-gnu/
    │   │   │   └── package.json
    │   │   └── win32-x64-msvc/
    │   │       └── package.json
    │   ├── scripts/
    │   │   ├── build.js
    │   │   └── sync-platform-versions.cjs
    │   └── .cargo/
    │       └── config.toml
    ├── packages/
    │   ├── contracts/
    │   │   ├── package.json
    │   │   ├── tsconfig.json
    │   │   └── src/
    │   │       ├── index.ts
    │   │       ├── rpc.test.ts
    │   │       ├── rpc.ts
    │   │       └── workflow.ts
    │   ├── daemon/
    │   │   ├── package.json
    │   │   ├── tsconfig.json
    │   │   └── src/
    │   │       ├── channel-manager.ts
    │   │       ├── cli.ts
    │   │       ├── commands.ts
    │   │       ├── config.ts
    │   │       ├── daemon.test.ts
    │   │       ├── daemon.ts
    │   │       ├── discord-bot.test.ts
    │   │       ├── discord-bot.ts
    │   │       ├── event-bridge.test.ts
    │   │       ├── event-bridge.ts
    │   │       ├── event-formatter.test.ts
    │   │       ├── event-formatter.ts
    │   │       ├── index.ts
    │   │       ├── launchd.test.ts
    │   │       ├── launchd.ts
    │   │       ├── logger.ts
    │   │       ├── message-batcher.test.ts
    │   │       ├── message-batcher.ts
    │   │       ├── orchestrator.test.ts
    │   │       ├── orchestrator.ts
    │   │       ├── project-scanner.test.ts
    │   │       ├── project-scanner.ts
    │   │       ├── session-manager.test.ts
    │   │       ├── session-manager.ts
    │   │       ├── types.ts
    │   │       ├── verbosity.test.ts
    │   │       └── verbosity.ts
    │   ├── mcp-server/
    │   │   ├── README.md
    │   │   ├── package.json
    │   │   ├── tsconfig.json
    │   │   ├── tsconfig.test.json
    │   │   ├── .npmignore
    │   │   └── src/
    │   │       ├── alias-telemetry.test.ts
    │   │       ├── alias-telemetry.ts
    │   │       ├── cli.ts
    │   │       ├── env-writer.test.ts
    │   │       ├── env-writer.ts
    │   │       ├── import-candidates.test.ts
    │   │       ├── index.ts
    │   │       ├── mcp-server.test.ts
    │   │       ├── parse-workflow-args.test.ts
    │   │       ├── remote-questions.test.ts
    │   │       ├── remote-questions.ts
    │   │       ├── secure-env-collect.test.ts
    │   │       ├── session-manager.ts
    │   │       ├── tool-credentials.test.ts
    │   │       ├── tool-credentials.ts
    │   │       ├── types.ts
    │   │       ├── workflow-tools-parity.test.ts
    │   │       └── readers/
    │   │           ├── captures.ts
    │   │           ├── doctor-lite.ts
    │   │           ├── graph.test.ts
    │   │           ├── graph.ts
    │   │           ├── index.ts
    │   │           ├── knowledge.ts
    │   │           ├── metrics.ts
    │   │           ├── paths.test.ts
    │   │           ├── paths.ts
    │   │           ├── readers.test.ts
    │   │           ├── roadmap.ts
    │   │           └── state.ts
    │   ├── native/
    │   │   ├── package.json
    │   │   ├── tsconfig.json
    │   │   └── src/
    │   │       ├── index.ts
    │   │       ├── native.ts
    │   │       ├── __tests__/
    │   │       │   ├── _test-coverage-guard.test.mjs
    │   │       │   ├── clipboard.test.mjs
    │   │       │   ├── diff.test.mjs
    │   │       │   ├── fd.test.mjs
    │   │       │   ├── glob.test.mjs
    │   │       │   ├── grep.test.mjs
    │   │       │   ├── highlight.test.mjs
    │   │       │   ├── html.test.mjs
    │   │       │   ├── image.test.mjs
    │   │       │   ├── json-parse.test.mjs
    │   │       │   ├── module-compat.test.mjs
    │   │       │   ├── ps.test.mjs
    │   │       │   ├── stream-process.test.mjs
    │   │       │   ├── text.test.mjs
    │   │       │   ├── truncate.test.mjs
    │   │       │   ├── ttsr.test.mjs
    │   │       │   └── xxhash.test.mjs
    │   │       ├── ast/
    │   │       │   ├── index.ts
    │   │       │   └── types.ts
    │   │       ├── clipboard/
    │   │       │   ├── index.ts
    │   │       │   └── types.ts
    │   │       ├── diff/
    │   │       │   ├── index.ts
    │   │       │   └── types.ts
    │   │       ├── fd/
    │   │       │   ├── index.ts
    │   │       │   └── types.ts
    │   │       ├── glob/
    │   │       │   ├── index.ts
    │   │       │   └── types.ts
    │   │       ├── grep/
    │   │       │   ├── index.ts
    │   │       │   └── types.ts
    │   │       ├── gsd-parser/
    │   │       │   ├── index.ts
    │   │       │   └── types.ts
    │   │       ├── highlight/
    │   │       │   ├── index.ts
    │   │       │   └── types.ts
    │   │       ├── html/
    │   │       │   ├── index.ts
    │   │       │   └── types.ts
    │   │       ├── image/
    │   │       │   ├── index.ts
    │   │       │   └── types.ts
    │   │       ├── json-parse/
    │   │       │   └── index.ts
    │   │       ├── ps/
    │   │       │   └── index.ts
    │   │       ├── stream-process/
    │   │       │   └── index.ts
    │   │       ├── text/
    │   │       │   ├── index.ts
    │   │       │   └── types.ts
    │   │       ├── truncate/
    │   │       │   └── index.ts
    │   │       ├── ttsr/
    │   │       │   ├── index.ts
    │   │       │   └── types.ts
    │   │       └── xxhash/
    │   │           └── index.ts
    │   ├── pi-agent-core/
    │   │   ├── package.json
    │   │   ├── tsconfig.json
    │   │   └── src/
    │   │       ├── agent-loop.test.ts
    │   │       ├── agent-loop.ts
    │   │       ├── agent.test.ts
    │   │       ├── agent.ts
    │   │       ├── index.ts
    │   │       ├── proxy.ts
    │   │       ├── token-audit.test.ts
    │   │       ├── token-audit.ts
    │   │       └── types.ts
    │   ├── pi-ai/
    │   │   ├── bedrock-provider.d.ts
    │   │   ├── bedrock-provider.js
    │   │   ├── package.json
    │   │   ├── tsconfig.json
    │   │   ├── scripts/
    │   │   │   └── generate-models.ts
    │   │   └── src/
    │   │       ├── api-registry.ts
    │   │       ├── bedrock-provider.ts
    │   │       ├── cli.ts
    │   │       ├── env-api-keys.ts
    │   │       ├── index.ts
    │   │       ├── models.generated.test.ts
    │   │       ├── models.test.ts
    │   │       ├── models.ts
    │   │       ├── oauth.ts
    │   │       ├── stream.ts
    │   │       ├── types.ts
    │   │       ├── web-runtime-env-api-keys.ts
    │   │       ├── web-runtime-oauth.ts
    │   │       ├── models/
    │   │       │   ├── capability-patches.ts
    │   │       │   ├── custom.ts
    │   │       │   ├── fake-model.ts
    │   │       │   ├── index.ts
    │   │       │   └── generated/
    │   │       │       ├── amazon-bedrock.ts
    │   │       │       ├── anthropic.ts
    │   │       │       ├── azure-openai-responses.ts
    │   │       │       ├── cerebras.ts
    │   │       │       ├── github-copilot.ts
    │   │       │       ├── google-antigravity.ts
    │   │       │       ├── google-gemini-cli.ts
    │   │       │       ├── google-vertex.ts
    │   │       │       ├── google.ts
    │   │       │       ├── groq.ts
    │   │       │       ├── huggingface.ts
    │   │       │       ├── index.ts
    │   │       │       ├── kimi-coding.ts
    │   │       │       ├── minimax-cn.ts
    │   │       │       ├── minimax.ts
    │   │       │       ├── mistral.ts
    │   │       │       ├── openai-codex.ts
    │   │       │       ├── openai.ts
    │   │       │       ├── opencode-go.ts
    │   │       │       ├── opencode.ts
    │   │       │       ├── xai.ts
    │   │       │       └── zai.ts
    │   │       ├── providers/
    │   │       │   ├── amazon-bedrock.test.ts
    │   │       │   ├── amazon-bedrock.ts
    │   │       │   ├── anthropic-auth.test.ts
    │   │       │   ├── anthropic-bearer-auth.test.ts
    │   │       │   ├── anthropic-shared.cache-breakpoint.test.ts
    │   │       │   ├── anthropic-shared.test.ts
    │   │       │   ├── anthropic-shared.ts
    │   │       │   ├── anthropic-vertex.ts
    │   │       │   ├── anthropic.ts
    │   │       │   ├── api-family.test.ts
    │   │       │   ├── api-family.ts
    │   │       │   ├── azure-openai-responses.ts
    │   │       │   ├── fake.ts
    │   │       │   ├── github-copilot-headers.ts
    │   │       │   ├── google-gemini-cli.test.ts
    │   │       │   ├── google-gemini-cli.ts
    │   │       │   ├── google-shared.test.ts
    │   │       │   ├── google-shared.ts
    │   │       │   ├── google-vertex.ts
    │   │       │   ├── google.ts
    │   │       │   ├── minimax-tool-name.test.ts
    │   │       │   ├── mistral.ts
    │   │       │   ├── openai-codex-responses.test.ts
    │   │       │   ├── openai-codex-responses.ts
    │   │       │   ├── openai-completions.ts
    │   │       │   ├── openai-responses-shared.ts
    │   │       │   ├── openai-responses.ts
    │   │       │   ├── openai-shared.ts
    │   │       │   ├── provider-capabilities.test.ts
    │   │       │   ├── provider-capabilities.ts
    │   │       │   ├── register-builtins.ts
    │   │       │   ├── simple-options.test.ts
    │   │       │   ├── simple-options.ts
    │   │       │   ├── think-tag-parser.test.ts
    │   │       │   ├── think-tag-parser.ts
    │   │       │   ├── transform-messages-report.test.ts
    │   │       │   └── transform-messages.ts
    │   │       └── utils/
    │   │           ├── event-stream.ts
    │   │           ├── hash.ts
    │   │           ├── json-parse.ts
    │   │           ├── overflow.ts
    │   │           ├── repair-tool-json.ts
    │   │           ├── sanitize-unicode.ts
    │   │           ├── typebox-helpers.ts
    │   │           ├── validation.ts
    │   │           ├── oauth/
    │   │           │   ├── github-copilot.test.ts
    │   │           │   ├── github-copilot.ts
    │   │           │   ├── google-antigravity.test.ts
    │   │           │   ├── google-antigravity.ts
    │   │           │   ├── google-gemini-cli.test.ts
    │   │           │   ├── google-gemini-cli.ts
    │   │           │   ├── google-oauth-utils.ts
    │   │           │   ├── index.ts
    │   │           │   ├── openai-codex.ts
    │   │           │   ├── pkce.ts
    │   │           │   └── types.ts
    │   │           └── tests/
    │   │               ├── json-parse.test.ts
    │   │               ├── overflow.test.ts
    │   │               └── repair-tool-json.test.ts
    │   ├── pi-coding-agent/
    │   │   ├── package.json
    │   │   ├── tsconfig.json
    │   │   ├── scripts/
    │   │   │   └── copy-assets.cjs
    │   │   └── src/
    │   │       ├── cli.ts
    │   │       ├── config.ts
    │   │       ├── index.ts
    │   │       ├── main.ts
    │   │       ├── migrations.ts
    │   │       ├── cli/
    │   │       │   ├── args.test.ts
    │   │       │   ├── args.ts
    │   │       │   ├── config-selector.ts
    │   │       │   ├── file-processor.ts
    │   │       │   ├── list-models.ts
    │   │       │   └── session-picker.ts
    │   │       ├── core/
    │   │       │   ├── agent-session-abort-order.test.ts
    │   │       │   ├── agent-session-model-switch.test.ts
    │   │       │   ├── agent-session-renderable-tools.test.ts
    │   │       │   ├── agent-session-thinking-level.test.ts
    │   │       │   ├── agent-session-tool-refresh.test.ts
    │   │       │   ├── artifact-manager.ts
    │   │       │   ├── auth-storage.test.ts
    │   │       │   ├── auth-storage.ts
    │   │       │   ├── bash-executor.ts
    │   │       │   ├── blob-store.ts
    │   │       │   ├── compaction-orchestrator.test.ts
    │   │       │   ├── compaction-orchestrator.ts
    │   │       │   ├── compaction-threshold.test.ts
    │   │       │   ├── compaction-utils.test.ts
    │   │       │   ├── constants.ts
    │   │       │   ├── contextual-tips.test.ts
    │   │       │   ├── contextual-tips.ts
    │   │       │   ├── db-snapshot.test.ts
    │   │       │   ├── db-snapshot.ts
    │   │       │   ├── defaults.ts
    │   │       │   ├── diagnostics.ts
    │   │       │   ├── discovery-cache.test.ts
    │   │       │   ├── discovery-cache.ts
    │   │       │   ├── event-bus.ts
    │   │       │   ├── exec.ts
    │   │       │   ├── fallback-resolver.test.ts
    │   │       │   ├── fallback-resolver.ts
    │   │       │   ├── footer-data-provider.ts
    │   │       │   ├── fs-utils.test.ts
    │   │       │   ├── fs-utils.ts
    │   │       │   ├── hooks-runner.test.ts
    │   │       │   ├── hooks-runner.ts
    │   │       │   ├── image-overflow-recovery.test.ts
    │   │       │   ├── image-overflow-recovery.ts
    │   │       │   ├── index.ts
    │   │       │   ├── keybindings.ts
    │   │       │   ├── lifecycle-hooks.test.ts
    │   │       │   ├── lifecycle-hooks.ts
    │   │       │   ├── local-model-check.ts
    │   │       │   ├── lock-utils.ts
    │   │       │   ├── messages.test.ts
    │   │       │   ├── messages.ts
    │   │       │   ├── model-discovery.test.ts
    │   │       │   ├── model-discovery.ts
    │   │       │   ├── model-registry-auth-header.test.ts
    │   │       │   ├── model-registry-auth-mode.test.ts
    │   │       │   ├── model-registry-custom-caps.test.ts
    │   │       │   ├── model-registry-discovery.test.ts
    │   │       │   ├── model-registry-env-fallback.test.ts
    │   │       │   ├── model-registry.ts
    │   │       │   ├── model-resolver-initial-model-auth.test.ts
    │   │       │   ├── model-resolver.test.ts
    │   │       │   ├── model-resolver.ts
    │   │       │   ├── models-json-writer.test.ts
    │   │       │   ├── models-json-writer.ts
    │   │       │   ├── package-commands.test.ts
    │   │       │   ├── package-commands.ts
    │   │       │   ├── prompt-templates.ts
    │   │       │   ├── redact-secrets.test.ts
    │   │       │   ├── redact-secrets.ts
    │   │       │   ├── resolve-config-value.test.ts
    │   │       │   ├── resolve-config-value.ts
    │   │       │   ├── resource-loader-cache-reset.test.ts
    │   │       │   ├── resource-loader.ts
    │   │       │   ├── retry-handler.test.ts
    │   │       │   ├── retry-handler.ts
    │   │       │   ├── retryable-error-regex.ts
    │   │       │   ├── sdk-tool-filter.test.ts
    │   │       │   ├── sdk.test.ts
    │   │       │   ├── sdk.ts
    │   │       │   ├── session-manager.test.ts
    │   │       │   ├── session-manager.ts
    │   │       │   ├── settings-manager-security.test.ts
    │   │       │   ├── settings-manager.ts
    │   │       │   ├── skill-tool.test.ts
    │   │       │   ├── skills.ts
    │   │       │   ├── slash-commands.ts
    │   │       │   ├── system-prompt.ts
    │   │       │   ├── timings.ts
    │   │       │   ├── token-telemetry.ts
    │   │       │   ├── compaction/
    │   │       │   │   ├── branch-summarization.ts
    │   │       │   │   ├── compaction.test.ts
    │   │       │   │   ├── compaction.ts
    │   │       │   │   ├── index.ts
    │   │       │   │   └── utils.ts
    │   │       │   ├── export-html/
    │   │       │   │   ├── ansi-to-html.ts
    │   │       │   │   ├── index.ts
    │   │       │   │   ├── template.css
    │   │       │   │   ├── template.html
    │   │       │   │   └── tool-renderer.ts
    │   │       │   ├── extensions/
    │   │       │   │   ├── extension-discovery.ts
    │   │       │   │   ├── extension-manifest.test.ts
    │   │       │   │   ├── extension-manifest.ts
    │   │       │   │   ├── extension-registry.ts
    │   │       │   │   ├── extension-sort.test.ts
    │   │       │   │   ├── extension-sort.ts
    │   │       │   │   ├── index.ts
    │   │       │   │   ├── loader.test.ts
    │   │       │   │   ├── loader.ts
    │   │       │   │   ├── project-trust.ts
    │   │       │   │   ├── provider-registration.test.ts
    │   │       │   │   ├── runner.test.ts
    │   │       │   │   ├── runner.ts
    │   │       │   │   └── wrapper.ts
    │   │       │   ├── lsp/
    │   │       │   │   ├── client.ts
    │   │       │   │   ├── config.ts
    │   │       │   │   ├── defaults.json
    │   │       │   │   ├── edits.ts
    │   │       │   │   ├── helpers.ts
    │   │       │   │   ├── index.ts
    │   │       │   │   ├── lsp-integration.test.ts
    │   │       │   │   ├── lsp-legacy-alias.test.ts
    │   │       │   │   ├── lsp.md
    │   │       │   │   ├── lspmux.ts
    │   │       │   │   ├── types.ts
    │   │       │   │   └── utils.ts
    │   │       │   └── tools/
    │   │       │       ├── bash-background.test.ts
    │   │       │       ├── bash-interceptor.test.ts
    │   │       │       ├── bash-interceptor.ts
    │   │       │       ├── bash-spawn-windows.test.ts
    │   │       │       ├── bash.ts
    │   │       │       ├── edit-diff.test.ts
    │   │       │       ├── edit-diff.ts
    │   │       │       ├── edit.ts
    │   │       │       ├── find.ts
    │   │       │       ├── grep.ts
    │   │       │       ├── hashline-edit.ts
    │   │       │       ├── hashline-read.ts
    │   │       │       ├── hashline.test.ts
    │   │       │       ├── hashline.ts
    │   │       │       ├── index.ts
    │   │       │       ├── ls.ts
    │   │       │       ├── path-utils.test.ts
    │   │       │       ├── path-utils.ts
    │   │       │       ├── read.ts
    │   │       │       ├── spawn-shell-windows.test.ts
    │   │       │       ├── tool-compatibility-registry.ts
    │   │       │       ├── tool-target-metadata.test.ts
    │   │       │       ├── tool-target.ts
    │   │       │       ├── truncate.ts
    │   │       │       └── write.ts
    │   │       ├── modes/
    │   │       │   ├── index.ts
    │   │       │   ├── print-mode.ts
    │   │       │   ├── interactive/
    │   │       │   │   ├── interactive-mode-ordering.test.ts
    │   │       │   │   ├── interactive-mode-state.ts
    │   │       │   │   ├── slash-command-handlers.test.ts
    │   │       │   │   ├── slash-command-handlers.ts
    │   │       │   │   ├── tui-mode.test.ts
    │   │       │   │   ├── tui-mode.ts
    │   │       │   │   ├── components/
    │   │       │   │   │   ├── adaptive-layout.test.ts
    │   │       │   │   │   ├── adaptive-layout.ts
    │   │       │   │   │   ├── armin.ts
    │   │       │   │   │   ├── assistant-message.ts
    │   │       │   │   │   ├── bash-execution.ts
    │   │       │   │   │   ├── bordered-loader.ts
    │   │       │   │   │   ├── branch-summary-message.ts
    │   │       │   │   │   ├── chat-frame.ts
    │   │       │   │   │   ├── compaction-summary-message.ts
    │   │       │   │   │   ├── config-selector.ts
    │   │       │   │   │   ├── countdown-timer.ts
    │   │       │   │   │   ├── custom-editor.ts
    │   │       │   │   │   ├── custom-message.ts
    │   │       │   │   │   ├── daxnuts.ts
    │   │       │   │   │   ├── diff.ts
    │   │       │   │   │   ├── dynamic-border.test.ts
    │   │       │   │   │   ├── dynamic-border.ts
    │   │       │   │   │   ├── extension-editor.ts
    │   │       │   │   │   ├── extension-input.ts
    │   │       │   │   │   ├── extension-selector.ts
    │   │       │   │   │   ├── footer.ts
    │   │       │   │   │   ├── index.ts
    │   │       │   │   │   ├── keybinding-hints.ts
    │   │       │   │   │   ├── login-dialog.ts
    │   │       │   │   │   ├── model-selector.ts
    │   │       │   │   │   ├── oauth-selector.ts
    │   │       │   │   │   ├── provider-manager.ts
    │   │       │   │   │   ├── scoped-models-selector.ts
    │   │       │   │   │   ├── session-selector-search.ts
    │   │       │   │   │   ├── session-selector.ts
    │   │       │   │   │   ├── settings-selector.ts
    │   │       │   │   │   ├── show-images-selector.ts
    │   │       │   │   │   ├── skill-invocation-message.ts
    │   │       │   │   │   ├── theme-selector.ts
    │   │       │   │   │   ├── thinking-selector.ts
    │   │       │   │   │   ├── timestamp.ts
    │   │       │   │   │   ├── tool-card-cleanup-and-success-runtime.test.ts
    │   │       │   │   │   ├── transcript-design.ts
    │   │       │   │   │   ├── tree-render-utils.ts
    │   │       │   │   │   ├── tree-selector.ts
    │   │       │   │   │   ├── tui-style-kit.ts
    │   │       │   │   │   ├── user-message-selector.ts
    │   │       │   │   │   ├── user-message.ts
    │   │       │   │   │   ├── visual-truncate.ts
    │   │       │   │   │   └── __tests__/
    │   │       │   │   │       ├── assistant-message-design.test.ts
    │   │       │   │   │       ├── chat-frame-compaction-tone.test.ts
    │   │       │   │   │       ├── login-dialog.test.ts
    │   │       │   │   │       ├── provider-display-name.test.ts
    │   │       │   │   │       ├── timestamp.test.ts
    │   │       │   │   │       ├── tool-execution.test.ts
    │   │       │   │   │       └── user-message-design.test.ts
    │   │       │   │   ├── controllers/
    │   │       │   │   │   ├── chat-controller.test.ts
    │   │       │   │   │   ├── chat-controller.ts
    │   │       │   │   │   ├── extension-ui-controller.ts
    │   │       │   │   │   ├── input-controller.test.ts
    │   │       │   │   │   ├── input-controller.ts
    │   │       │   │   │   └── model-controller.ts
    │   │       │   │   ├── theme/
    │   │       │   │   │   ├── theme-highlight.test.ts
    │   │       │   │   │   ├── theme-schema.ts
    │   │       │   │   │   ├── theme.ts
    │   │       │   │   │   └── themes.ts
    │   │       │   │   └── utils/
    │   │       │   │       └── shorten-path.ts
    │   │       │   ├── rpc/
    │   │       │   │   ├── jsonl.ts
    │   │       │   │   ├── remote-terminal.ts
    │   │       │   │   ├── rpc-client.ts
    │   │       │   │   ├── rpc-mode.ts
    │   │       │   │   ├── rpc-protocol-v2.test.ts
    │   │       │   │   └── rpc-types.ts
    │   │       │   └── shared/
    │   │       │       └── command-context-actions.ts
    │   │       ├── resources/
    │   │       │   └── extensions/
    │   │       │       └── memory/
    │   │       │           ├── index.ts
    │   │       │           ├── pipeline.ts
    │   │       │           ├── storage-safety-guard.test.ts
    │   │       │           ├── storage.test.ts
    │   │       │           └── storage.ts
    │   │       ├── tests/
    │   │       │   ├── path-display.test.ts
    │   │       │   ├── system-prompt-cache-stability.test.ts
    │   │       │   ├── system-prompt-file-safety.test.ts
    │   │       │   ├── system-prompt-skill-filter.test.ts
    │   │       │   └── token-telemetry.test.ts
    │   │       ├── types/
    │   │       │   └── ambient-modules.d.ts
    │   │       └── utils/
    │   │           ├── changelog.ts
    │   │           ├── clipboard-image.ts
    │   │           ├── clipboard-native.ts
    │   │           ├── clipboard.ts
    │   │           ├── error.ts
    │   │           ├── frontmatter.ts
    │   │           ├── git.ts
    │   │           ├── image-convert.ts
    │   │           ├── image-resize.ts
    │   │           ├── mime.ts
    │   │           ├── path-display.ts
    │   │           ├── photon.ts
    │   │           ├── shell.ts
    │   │           ├── sleep.ts
    │   │           └── tools-manager.ts
    │   ├── pi-tui/
    │   │   ├── package.json
    │   │   ├── tsconfig.json
    │   │   └── src/
    │   │       ├── autocomplete.ts
    │   │       ├── editor-component.ts
    │   │       ├── fuzzy.ts
    │   │       ├── index.ts
    │   │       ├── keybindings.ts
    │   │       ├── keys.ts
    │   │       ├── kill-ring.ts
    │   │       ├── overlay-layout.ts
    │   │       ├── stdin-buffer.ts
    │   │       ├── style.ts
    │   │       ├── terminal-image.ts
    │   │       ├── terminal.ts
    │   │       ├── tui.ts
    │   │       ├── undo-stack.ts
    │   │       ├── utils.ts
    │   │       ├── __tests__/
    │   │       │   ├── autocomplete.test.ts
    │   │       │   ├── fuzzy.test.ts
    │   │       │   ├── overlay-layout.test.ts
    │   │       │   ├── stdin-buffer.test.ts
    │   │       │   ├── style.test.ts
    │   │       │   ├── terminal.test.ts
    │   │       │   └── tui.test.ts
    │   │       └── components/
    │   │           ├── box.ts
    │   │           ├── cancellable-loader.ts
    │   │           ├── image.test.ts
    │   │           ├── image.ts
    │   │           ├── input.ts
    │   │           ├── loader.ts
    │   │           ├── markdown.ts
    │   │           ├── select-list.ts
    │   │           ├── settings-list.ts
    │   │           ├── spacer.ts
    │   │           ├── text.ts
    │   │           ├── truncated-text.ts
    │   │           └── __tests__/
    │   │               ├── cancellable-loader.test.ts
    │   │               ├── editor.test.ts
    │   │               ├── input.test.ts
    │   │               ├── leak-fixes-runtime.test.ts
    │   │               ├── loader.test.ts
    │   │               └── markdown-maxlines.test.ts
    │   └── rpc-client/
    │       ├── README.md
    │       ├── package.json
    │       ├── tsconfig.examples.json
    │       ├── tsconfig.json
    │       ├── .npmignore
    │       ├── examples/
    │       │   └── basic-usage.ts
    │       └── src/
    │           ├── index.ts
    │           ├── jsonl.ts
    │           ├── rpc-client.test.ts
    │           ├── rpc-client.ts
    │           └── rpc-types.ts
    ├── scripts/
    │   ├── base64-scan.sh
    │   ├── build-web-if-stale.cjs
    │   ├── bump-version.mjs
    │   ├── check-coderabbit-themes.mjs
    │   ├── check-skill-references.mjs
    │   ├── check-source-grep-tests.sh
    │   ├── ci_monitor.cjs
    │   ├── ci_monitor.md
    │   ├── compile-tests.mjs
    │   ├── copy-export-html.cjs
    │   ├── copy-resources.cjs
    │   ├── copy-themes.cjs
    │   ├── dev-cli-helpers.mjs
    │   ├── dev-cli.js
    │   ├── dev.js
    │   ├── dist-test-resolve.mjs
    │   ├── docs-prompt-injection-scan.sh
    │   ├── ensure-workspace-builds.cjs
    │   ├── generate-changelog.mjs
    │   ├── install-hooks.mjs
    │   ├── install-hooks.sh
    │   ├── install-pi-global.js
    │   ├── install.js
    │   ├── legacy-cleanup-evidence.mjs
    │   ├── legacy-cleanup-gate.mjs
    │   ├── link-workspace-packages.cjs
    │   ├── parallel-monitor.mjs
    │   ├── postinstall.js
    │   ├── pr-risk-check.mjs
    │   ├── prepublish-check.mjs
    │   ├── preview-dashboard.ts
    │   ├── recover-gsd-1364.ps1
    │   ├── recover-gsd-1364.sh
    │   ├── recover-gsd-1668.ps1
    │   ├── recover-gsd-1668.sh
    │   ├── refactor-baseline.mjs
    │   ├── require-tests.sh
    │   ├── rtk-benchmark.mjs
    │   ├── run-package-tests.cjs
    │   ├── secret-scan.mjs
    │   ├── secret-scan.sh
    │   ├── stage-web-standalone.cjs
    │   ├── sync-pkg-version.cjs
    │   ├── test-reporter-compact.mjs
    │   ├── uninstall-pi-global.js
    │   ├── update-changelog.mjs
    │   ├── validate-pack.js
    │   ├── validate-pack.sh
    │   ├── verify-changed-src-tests.mjs
    │   ├── verify-s03.sh
    │   ├── verify-s04.sh
    │   ├── verify-workspace-coverage.cjs
    │   ├── version-stamp.mjs
    │   ├── watch-resources.js
    │   ├── with-env.mjs
    │   ├── __fixtures__/
    │   │   └── coderabbit-themes/
    │   │       ├── bad-mjs-ts-import.mjs
    │   │       ├── bad-once-after-trigger.ts
    │   │       ├── bad-sqlite-null-guard.ts
    │   │       ├── good-mjs-no-ts-import.mjs
    │   │       ├── good-once-after-trigger.ts
    │   │       └── good-sqlite-null-guard.ts
    │   ├── __tests__/
    │   │   ├── check-coderabbit-themes.test.mjs
    │   │   ├── compile-tests-cache.test.mjs
    │   │   ├── verify-changed-src-tests.git.test.mjs
    │   │   └── verify-changed-src-tests.test.mjs
    │   └── lib/
    │       └── workspace-manifest.cjs
    ├── src/
    │   ├── app-paths.ts
    │   ├── bundled-extension-paths.ts
    │   ├── bundled-resource-path.ts
    │   ├── claude-cli-check.ts
    │   ├── cli-auto-routing.ts
    │   ├── cli-policy.ts
    │   ├── cli-web-branch.ts
    │   ├── cli.ts
    │   ├── extension-discovery.ts
    │   ├── extension-registry.ts
    │   ├── extension-sort.ts
    │   ├── extension-validator.ts
    │   ├── headless-answers.ts
    │   ├── headless-context.ts
    │   ├── headless-events.ts
    │   ├── headless-query.ts
    │   ├── headless-recover.ts
    │   ├── headless-types.ts
    │   ├── headless-ui.ts
    │   ├── headless.ts
    │   ├── help-text.ts
    │   ├── loader-entrypoint.ts
    │   ├── loader.ts
    │   ├── logo.ts
    │   ├── mcp-server.ts
    │   ├── models-resolver.ts
    │   ├── onboarding.ts
    │   ├── pi-migration.ts
    │   ├── project-sessions.ts
    │   ├── provider-migrations.ts
    │   ├── remote-questions-config.ts
    │   ├── resource-loader.ts
    │   ├── rtk-shared.js
    │   ├── rtk-shared.ts
    │   ├── rtk.ts
    │   ├── runtime-checks.ts
    │   ├── security-overrides.ts
    │   ├── startup-model-validation.ts
    │   ├── startup-timings.ts
    │   ├── tool-bootstrap.ts
    │   ├── update-check.ts
    │   ├── update-cmd.ts
    │   ├── web-mode.ts
    │   ├── welcome-screen.ts
    │   ├── wizard.ts
    │   ├── worktree-cli.ts
    │   ├── worktree-name-gen.ts
    │   ├── worktree-status-banner.ts
    │   ├── resources/
    │   │   ├── GSD-WORKFLOW.md
    │   │   ├── agents/
    │   │   │   ├── debugger.md
    │   │   │   ├── doc-writer.md
    │   │   │   ├── git-ops.md
    │   │   │   ├── javascript-pro.md
    │   │   │   ├── planner.md
    │   │   │   ├── refactorer.md
    │   │   │   ├── researcher.md
    │   │   │   ├── reviewer.md
    │   │   │   ├── scout.md
    │   │   │   ├── security.md
    │   │   │   ├── tester.md
    │   │   │   ├── typescript-pro.md
    │   │   │   └── worker.md
    │   │   ├── extensions/
    │   │   │   ├── ask-user-questions.ts
    │   │   │   ├── get-secrets-from-user.ts
    │   │   │   ├── package.json
    │   │   │   ├── async-jobs/
    │   │   │   │   ├── async-bash-timeout.test.ts
    │   │   │   │   ├── async-bash-tool.ts
    │   │   │   │   ├── await-tool.test.ts
    │   │   │   │   ├── await-tool.ts
    │   │   │   │   ├── cancel-job-tool.ts
    │   │   │   │   ├── extension-manifest.json
    │   │   │   │   ├── index.ts
    │   │   │   │   └── job-manager.ts
    │   │   │   ├── aws-auth/
    │   │   │   │   └── index.ts
    │   │   │   ├── bg-shell/
    │   │   │   │   ├── bg-shell-command.ts
    │   │   │   │   ├── bg-shell-lifecycle.ts
    │   │   │   │   ├── bg-shell-tool.ts
    │   │   │   │   ├── extension-manifest.json
    │   │   │   │   ├── index.ts
    │   │   │   │   ├── interaction.ts
    │   │   │   │   ├── output-formatter.ts
    │   │   │   │   ├── overlay.ts
    │   │   │   │   ├── process-manager.ts
    │   │   │   │   ├── readiness-detector.ts
    │   │   │   │   ├── types.ts
    │   │   │   │   └── utilities.ts
    │   │   │   ├── browser-tools/
    │   │   │   │   ├── BROWSER-TOOLS-V2-PROPOSAL.md
    │   │   │   │   ├── capture.ts
    │   │   │   │   ├── core.ts
    │   │   │   │   ├── evaluate-helpers.ts
    │   │   │   │   ├── extension-manifest.json
    │   │   │   │   ├── index.ts
    │   │   │   │   ├── lifecycle.ts
    │   │   │   │   ├── package.json
    │   │   │   │   ├── refs.ts
    │   │   │   │   ├── settle.ts
    │   │   │   │   ├── state.ts
    │   │   │   │   ├── utils.ts
    │   │   │   │   ├── tests/
    │   │   │   │   │   ├── browser-tools-integration.test.mjs
    │   │   │   │   │   ├── browser-tools-unit.test.cjs
    │   │   │   │   │   └── capture-sharp-optional.test.cjs
    │   │   │   │   └── tools/
    │   │   │   │       ├── action-cache.ts
    │   │   │   │       ├── assertions.ts
    │   │   │   │       ├── codegen.ts
    │   │   │   │       ├── device.ts
    │   │   │   │       ├── extract.ts
    │   │   │   │       ├── forms.ts
    │   │   │   │       ├── injection-detect.ts
    │   │   │   │       ├── inspection.ts
    │   │   │   │       ├── intent.ts
    │   │   │   │       ├── interaction.ts
    │   │   │   │       ├── navigation.ts
    │   │   │   │       ├── network-mock.ts
    │   │   │   │       ├── pages.ts
    │   │   │   │       ├── pdf.ts
    │   │   │   │       ├── refs.ts
    │   │   │   │       ├── screenshot.ts
    │   │   │   │       ├── session.ts
    │   │   │   │       ├── state-persistence.ts
    │   │   │   │       ├── verify.ts
    │   │   │   │       ├── visual-diff.ts
    │   │   │   │       ├── wait.ts
    │   │   │   │       └── zoom.ts
    │   │   │   ├── claude-code-cli/
    │   │   │   │   ├── index.ts
    │   │   │   │   ├── models.ts
    │   │   │   │   ├── package.json
    │   │   │   │   ├── partial-builder.ts
    │   │   │   │   ├── readiness.ts
    │   │   │   │   ├── sdk-types.ts
    │   │   │   │   └── tests/
    │   │   │   │       └── partial-builder.test.ts
    │   │   │   ├── cmux/
    │   │   │   │   ├── index.ts
    │   │   │   │   └── package.json
    │   │   │   ├── context7/
    │   │   │   │   ├── extension-manifest.json
    │   │   │   │   ├── index.ts
    │   │   │   │   └── package.json
    │   │   │   ├── github-sync/
    │   │   │   │   ├── cli.ts
    │   │   │   │   ├── index.ts
    │   │   │   │   ├── mapping.ts
    │   │   │   │   ├── sync.ts
    │   │   │   │   ├── templates.ts
    │   │   │   │   ├── types.ts
    │   │   │   │   └── tests/
    │   │   │   │       ├── cli.test.ts
    │   │   │   │       ├── commit-linking.test.ts
    │   │   │   │       ├── inline-code.test.ts
    │   │   │   │       ├── mapping.test.ts
    │   │   │   │       ├── sync-source.test.ts
    │   │   │   │       └── templates.test.ts
    │   │   │   ├── google-search/
    │   │   │   │   ├── extension-manifest.json
    │   │   │   │   ├── index.ts
    │   │   │   │   └── package.json
    │   │   │   ├── gsd/
    │   │   │   │   ├── abandon-detect.ts
    │   │   │   │   ├── activity-log.ts
    │   │   │   │   ├── atomic-write.ts
    │   │   │   │   ├── auto-artifact-paths.ts
    │   │   │   │   ├── auto-budget.ts
    │   │   │   │   ├── auto-dashboard.ts
    │   │   │   │   ├── auto-direct-dispatch.ts
    │   │   │   │   ├── auto-model-selection.ts
    │   │   │   │   ├── auto-recovery.ts
    │   │   │   │   ├── auto-runtime-state.ts
    │   │   │   │   ├── auto-supervisor.ts
    │   │   │   │   ├── auto-timeout-recovery.ts
    │   │   │   │   ├── auto-timers.ts
    │   │   │   │   ├── auto-tool-tracking.ts
    │   │   │   │   ├── auto-unit-closeout.ts
    │   │   │   │   ├── auto-utils.ts
    │   │   │   │   ├── auto-verification.ts
    │   │   │   │   ├── blocked-models.ts
    │   │   │   │   ├── branch-patterns.ts
    │   │   │   │   ├── cache.ts
    │   │   │   │   ├── captures.ts
    │   │   │   │   ├── changelog.ts
    │   │   │   │   ├── claude-import.ts
    │   │   │   │   ├── clean-root-preflight.ts
    │   │   │   │   ├── codebase-generator.ts
    │   │   │   │   ├── collision-diagnostics.ts
    │   │   │   │   ├── commands-add-tests.ts
    │   │   │   │   ├── commands-backlog.ts
    │   │   │   │   ├── commands-bootstrap.ts
    │   │   │   │   ├── commands-cmux.ts
    │   │   │   │   ├── commands-codebase.ts
    │   │   │   │   ├── commands-config.ts
    │   │   │   │   ├── commands-debug.ts
    │   │   │   │   ├── commands-do.ts
    │   │   │   │   ├── commands-eval-review.ts
    │   │   │   │   ├── commands-extensions.ts
    │   │   │   │   ├── commands-extract-learnings.ts
    │   │   │   │   ├── commands-handlers.ts
    │   │   │   │   ├── commands-inspect.ts
    │   │   │   │   ├── commands-logs.ts
    │   │   │   │   ├── commands-maintenance.ts
    │   │   │   │   ├── commands-mcp-status.ts
    │   │   │   │   ├── commands-memory.ts
    │   │   │   │   ├── commands-pr-branch.ts
    │   │   │   │   ├── commands-rate.ts
    │   │   │   │   ├── commands-scan.ts
    │   │   │   │   ├── commands-session-report.ts
    │   │   │   │   ├── commands-ship.ts
    │   │   │   │   ├── commands-verdict.ts
    │   │   │   │   ├── commands-workflow-templates.ts
    │   │   │   │   ├── commands-worktree.ts
    │   │   │   │   ├── commands.ts
    │   │   │   │   ├── compaction-snapshot.ts
    │   │   │   │   ├── complexity-classifier.ts
    │   │   │   │   ├── component-loader.ts
    │   │   │   │   ├── component-types.ts
    │   │   │   │   ├── config-overlay.ts
    │   │   │   │   ├── constants.ts
    │   │   │   │   ├── context-budget.ts
    │   │   │   │   ├── context-injector.ts
    │   │   │   │   ├── context-masker.ts
    │   │   │   │   ├── context-store.ts
    │   │   │   │   ├── crash-recovery.ts
    │   │   │   │   ├── custom-execution-policy.ts
    │   │   │   │   ├── custom-verification.ts
    │   │   │   │   ├── custom-workflow-engine.ts
    │   │   │   │   ├── dashboard-overlay.ts
    │   │   │   │   ├── db-adapter.ts
    │   │   │   │   ├── db-base-schema.ts
    │   │   │   │   ├── db-connection-cache.ts
    │   │   │   │   ├── db-coordination-schema.ts
    │   │   │   │   ├── db-decision-requirement-rows.ts
    │   │   │   │   ├── db-gate-rows.ts
    │   │   │   │   ├── db-lightweight-query-rows.ts
    │   │   │   │   ├── db-memory-fts-schema.ts
    │   │   │   │   ├── db-migration-backup.ts
    │   │   │   │   ├── db-migration-steps.ts
    │   │   │   │   ├── db-milestone-artifact-rows.ts
    │   │   │   │   ├── db-open-state.ts
    │   │   │   │   ├── db-provider.ts
    │   │   │   │   ├── db-runtime-kv-schema.ts
    │   │   │   │   ├── db-schema-metadata.ts
    │   │   │   │   ├── db-task-slice-rows.ts
    │   │   │   │   ├── db-transaction.ts
    │   │   │   │   ├── db-verification-evidence-rows.ts
    │   │   │   │   ├── db-verification-evidence-schema.ts
    │   │   │   │   ├── db-writer.ts
    │   │   │   │   ├── debug-logger.ts
    │   │   │   │   ├── debug-session-store.ts
    │   │   │   │   ├── deep-project-setup-policy.ts
    │   │   │   │   ├── definition-io.ts
    │   │   │   │   ├── definition-loader.ts
    │   │   │   │   ├── delegation-policy.ts
    │   │   │   │   ├── detection.ts
    │   │   │   │   ├── dev-execution-policy.ts
    │   │   │   │   ├── dev-workflow-engine.ts
    │   │   │   │   ├── diff-context.ts
    │   │   │   │   ├── dispatch-guard.ts
    │   │   │   │   ├── doctor-checks.ts
    │   │   │   │   ├── doctor-engine-checks.ts
    │   │   │   │   ├── doctor-environment.ts
    │   │   │   │   ├── doctor-format.ts
    │   │   │   │   ├── doctor-git-checks.ts
    │   │   │   │   ├── doctor-global-checks.ts
    │   │   │   │   ├── doctor-proactive.ts
    │   │   │   │   ├── doctor-providers.ts
    │   │   │   │   ├── doctor-runtime-checks.ts
    │   │   │   │   ├── doctor-types.ts
    │   │   │   │   ├── doctor.ts
    │   │   │   │   ├── engine-resolver.ts
    │   │   │   │   ├── engine-types.ts
    │   │   │   │   ├── env-utils.ts
    │   │   │   │   ├── error-classifier.ts
    │   │   │   │   ├── error-utils.ts
    │   │   │   │   ├── errors.ts
    │   │   │   │   ├── escalation.ts
    │   │   │   │   ├── eval-review-schema.ts
    │   │   │   │   ├── exec-history.ts
    │   │   │   │   ├── exec-sandbox.ts
    │   │   │   │   ├── execution-policy.ts
    │   │   │   │   ├── exit-command.ts
    │   │   │   │   ├── export-html.ts
    │   │   │   │   ├── export.ts
    │   │   │   │   ├── extension-manifest.json
    │   │   │   │   ├── file-lock.ts
    │   │   │   │   ├── files.ts
    │   │   │   │   ├── gate-registry.ts
    │   │   │   │   ├── git-constants.ts
    │   │   │   │   ├── git-self-heal.ts
    │   │   │   │   ├── gitignore.ts
    │   │   │   │   ├── graph-context.ts
    │   │   │   │   ├── graph.ts
    │   │   │   │   ├── gsd-home.ts
    │   │   │   │   ├── guided-flow-queue.ts
    │   │   │   │   ├── guided-unit-context.ts
    │   │   │   │   ├── health-widget-core.ts
    │   │   │   │   ├── health-widget.ts
    │   │   │   │   ├── history.ts
    │   │   │   │   ├── hook-emitter.ts
    │   │   │   │   ├── index.ts
    │   │   │   │   ├── init-wizard.ts
    │   │   │   │   ├── interrupted-session.ts
    │   │   │   │   ├── journal.ts
    │   │   │   │   ├── json-persistence.ts
    │   │   │   │   ├── jsonl-utils.ts
    │   │   │   │   ├── key-manager.ts
    │   │   │   │   ├── knowledge-backfill.ts
    │   │   │   │   ├── knowledge-capture.ts
    │   │   │   │   ├── knowledge-parser.ts
    │   │   │   │   ├── knowledge-projection.ts
    │   │   │   │   ├── legacy-telemetry.ts
    │   │   │   │   ├── markdown-renderer.ts
    │   │   │   │   ├── marketplace-discovery.ts
    │   │   │   │   ├── mcp-filter.ts
    │   │   │   │   ├── mcp-project-config.ts
    │   │   │   │   ├── md-importer.ts
    │   │   │   │   ├── memory-backfill.ts
    │   │   │   │   ├── memory-consolidation-scanner.ts
    │   │   │   │   ├── memory-embeddings.ts
    │   │   │   │   ├── memory-extractor.ts
    │   │   │   │   ├── memory-ingest.ts
    │   │   │   │   ├── memory-relations.ts
    │   │   │   │   ├── memory-source-store.ts
    │   │   │   │   ├── memory-store.ts
    │   │   │   │   ├── metrics.ts
    │   │   │   │   ├── migrate-external.ts
    │   │   │   │   ├── migration-auto-check.ts
    │   │   │   │   ├── milestone-actions.ts
    │   │   │   │   ├── milestone-id-reservation.ts
    │   │   │   │   ├── milestone-id-utils.ts
    │   │   │   │   ├── milestone-ids.ts
    │   │   │   │   ├── milestone-scope-classifier.ts
    │   │   │   │   ├── milestone-summary-classifier.ts
    │   │   │   │   ├── milestone-validation-gates.ts
    │   │   │   │   ├── model-cost-table.ts
    │   │   │   │   ├── model-router.ts
    │   │   │   │   ├── namespaced-registry.ts
    │   │   │   │   ├── namespaced-resolver.ts
    │   │   │   │   ├── native-git-bridge.ts
    │   │   │   │   ├── native-parser-bridge.ts
    │   │   │   │   ├── notification-overlay.ts
    │   │   │   │   ├── notification-store.ts
    │   │   │   │   ├── notification-widget.ts
    │   │   │   │   ├── notifications.ts
    │   │   │   │   ├── observability-validator.ts
    │   │   │   │   ├── onboarding-state.ts
    │   │   │   │   ├── orphan-stash-audit.ts
    │   │   │   │   ├── package.json
    │   │   │   │   ├── parallel-eligibility.ts
    │   │   │   │   ├── parallel-merge.ts
    │   │   │   │   ├── parallel-monitor-overlay.ts
    │   │   │   │   ├── parallel-orchestrator.ts
    │   │   │   │   ├── parsers-legacy.ts
    │   │   │   │   ├── paths.ts
    │   │   │   │   ├── pending-auto-start.ts
    │   │   │   │   ├── phase-anchor.ts
    │   │   │   │   ├── planning-depth.ts
    │   │   │   │   ├── planning-path-scope.ts
    │   │   │   │   ├── plugin-importer.ts
    │   │   │   │   ├── post-execution-checks.ts
    │   │   │   │   ├── post-unit-hooks.ts
    │   │   │   │   ├── pr-evidence.ts
    │   │   │   │   ├── pre-execution-checks.ts
    │   │   │   │   ├── preferences-mcp.ts
    │   │   │   │   ├── preferences-models.ts
    │   │   │   │   ├── preferences-skills.ts
    │   │   │   │   ├── preferences-types.ts
    │   │   │   │   ├── preferences.ts
    │   │   │   │   ├── preparation.ts
    │   │   │   │   ├── process-task-path.ts
    │   │   │   │   ├── progress-score.ts
    │   │   │   │   ├── project-research-policy.ts
    │   │   │   │   ├── prompt-cache-optimizer.ts
    │   │   │   │   ├── prompt-loader.ts
    │   │   │   │   ├── prompt-ordering.ts
    │   │   │   │   ├── prompt-validation.ts
    │   │   │   │   ├── provider-error-pause.ts
    │   │   │   │   ├── provider-switch-observer.ts
    │   │   │   │   ├── python-resolver.ts
    │   │   │   │   ├── queue-order.ts
    │   │   │   │   ├── queue-reorder-ui.ts
    │   │   │   │   ├── quick.ts
    │   │   │   │   ├── reactive-graph.ts
    │   │   │   │   ├── recovery-classification.ts
    │   │   │   │   ├── repo-identity.ts
    │   │   │   │   ├── reports.ts
    │   │   │   │   ├── repository-registry.ts
    │   │   │   │   ├── rethink.ts
    │   │   │   │   ├── roadmap-mutations.ts
    │   │   │   │   ├── roadmap-slices.ts
    │   │   │   │   ├── routing-history.ts
    │   │   │   │   ├── rule-registry.ts
    │   │   │   │   ├── rule-types.ts
    │   │   │   │   ├── run-manager.ts
    │   │   │   │   ├── safe-fs.ts
    │   │   │   │   ├── service-tier.ts
    │   │   │   │   ├── session-forensics.ts
    │   │   │   │   ├── session-lock.ts
    │   │   │   │   ├── session-model-override.ts
    │   │   │   │   ├── session-status-io.ts
    │   │   │   │   ├── setup-catalog.ts
    │   │   │   │   ├── shortcut-defs.ts
    │   │   │   │   ├── skill-catalog.ts
    │   │   │   │   ├── skill-discovery.ts
    │   │   │   │   ├── skill-health.ts
    │   │   │   │   ├── skill-manifest.ts
    │   │   │   │   ├── skill-telemetry.ts
    │   │   │   │   ├── slice-cadence.ts
    │   │   │   │   ├── slice-parallel-conflict.ts
    │   │   │   │   ├── slice-parallel-eligibility.ts
    │   │   │   │   ├── slice-parallel-orchestrator.ts
    │   │   │   │   ├── smart-entry-routing.ts
    │   │   │   │   ├── state-reconciliation.ts
    │   │   │   │   ├── state-transition-matrix.ts
    │   │   │   │   ├── status-guards.ts
    │   │   │   │   ├── structured-data-formatter.ts
    │   │   │   │   ├── sync-lock.ts
    │   │   │   │   ├── token-counter.ts
    │   │   │   │   ├── tool-contract.ts
    │   │   │   │   ├── triage-resolution.ts
    │   │   │   │   ├── triage-ui.ts
    │   │   │   │   ├── types.ts
    │   │   │   │   ├── undo.ts
    │   │   │   │   ├── unit-context-composer.ts
    │   │   │   │   ├── unit-context-manifest.ts
    │   │   │   │   ├── unit-id.ts
    │   │   │   │   ├── unit-ownership.ts
    │   │   │   │   ├── unit-runtime.ts
    │   │   │   │   ├── user-input-boundary.ts
    │   │   │   │   ├── validate-directory.ts
    │   │   │   │   ├── validation.ts
    │   │   │   │   ├── verdict-parser.ts
    │   │   │   │   ├── verification-evidence.ts
    │   │   │   │   ├── verification-gate.ts
    │   │   │   │   ├── verification-verdict.ts
    │   │   │   │   ├── visualizer-data.ts
    │   │   │   │   ├── visualizer-overlay.ts
    │   │   │   │   ├── visualizer-views.ts
    │   │   │   │   ├── workflow-dispatch.ts
    │   │   │   │   ├── workflow-engine.ts
    │   │   │   │   ├── workflow-events.ts
    │   │   │   │   ├── workflow-install.ts
    │   │   │   │   ├── workflow-logger.ts
    │   │   │   │   ├── workflow-manifest.ts
    │   │   │   │   ├── workflow-mcp-auto-prep.ts
    │   │   │   │   ├── workflow-mcp.ts
    │   │   │   │   ├── workflow-migration.ts
    │   │   │   │   ├── workflow-plugins.ts
    │   │   │   │   ├── workflow-projections.ts
    │   │   │   │   ├── workflow-protocol.ts
    │   │   │   │   ├── workflow-reconcile.ts
    │   │   │   │   ├── workflow-templates.ts
    │   │   │   │   ├── working-output-messages.ts
    │   │   │   │   ├── workspace-index.ts
    │   │   │   │   ├── workspace.ts
    │   │   │   │   ├── worktree-command-bootstrap.ts
    │   │   │   │   ├── worktree-command.ts
    │   │   │   │   ├── worktree-health.ts
    │   │   │   │   ├── worktree-manager.ts
    │   │   │   │   ├── worktree-root.ts
    │   │   │   │   ├── worktree-safety.ts
    │   │   │   │   ├── worktree-session-state.ts
    │   │   │   │   ├── worktree-state-projection.ts
    │   │   │   │   ├── worktree-telemetry.ts
    │   │   │   │   ├── worktree.ts
    │   │   │   │   ├── write-intercept.ts
    │   │   │   │   ├── auto/
    │   │   │   │   │   ├── contracts.ts
    │   │   │   │   │   ├── custom-verify-retry-store.ts
    │   │   │   │   │   ├── detect-stuck.ts
    │   │   │   │   │   ├── finalize-timeout.ts
    │   │   │   │   │   ├── infra-errors.ts
    │   │   │   │   │   ├── loop-deps.ts
    │   │   │   │   │   ├── loop.ts
    │   │   │   │   │   ├── orchestrator.ts
    │   │   │   │   │   ├── resolve.ts
    │   │   │   │   │   ├── run-unit.ts
    │   │   │   │   │   ├── session.ts
    │   │   │   │   │   ├── turn-epoch.ts
    │   │   │   │   │   ├── types.ts
    │   │   │   │   │   ├── unit-runner-events.ts
    │   │   │   │   │   ├── verification-retry-policy.ts
    │   │   │   │   │   ├── workflow-custom-engine-dispatch-outcome.ts
    │   │   │   │   │   ├── workflow-custom-engine-iteration.ts
    │   │   │   │   │   ├── workflow-custom-engine-reconcile-outcome.ts
    │   │   │   │   │   ├── workflow-custom-engine-reconcile.ts
    │   │   │   │   │   ├── workflow-custom-engine-retry.ts
    │   │   │   │   │   ├── workflow-custom-engine-verify-outcome.ts
    │   │   │   │   │   ├── workflow-dispatch-claim.ts
    │   │   │   │   │   ├── workflow-dispatch-ledger.ts
    │   │   │   │   │   ├── workflow-iteration-completion.ts
    │   │   │   │   │   ├── workflow-journal-reporter.ts
    │   │   │   │   │   ├── workflow-kernel.ts
    │   │   │   │   │   ├── workflow-memory-pressure.ts
    │   │   │   │   │   ├── workflow-phase-reporter.ts
    │   │   │   │   │   ├── workflow-session-lock.ts
    │   │   │   │   │   ├── workflow-sidecar-iteration.ts
    │   │   │   │   │   ├── workflow-sidecar-queue.ts
    │   │   │   │   │   ├── workflow-turn-reporter.ts
    │   │   │   │   │   ├── workflow-unit-dispatch.ts
    │   │   │   │   │   └── workflow-worker-heartbeat.ts
    │   │   │   │   ├── bootstrap/
    │   │   │   │   │   ├── agent-end-recovery.ts
    │   │   │   │   │   ├── crash-log.ts
    │   │   │   │   │   ├── dynamic-tools.ts
    │   │   │   │   │   ├── exec-tools.ts
    │   │   │   │   │   ├── journal-tools.ts
    │   │   │   │   │   ├── memory-tools.ts
    │   │   │   │   │   ├── notify-interceptor.ts
    │   │   │   │   │   ├── provider-error-resume.ts
    │   │   │   │   │   ├── query-tools.ts
    │   │   │   │   │   ├── register-extension.ts
    │   │   │   │   │   ├── register-hooks.ts
    │   │   │   │   │   ├── register-shortcuts.ts
    │   │   │   │   │   ├── sanitize-complete-milestone.ts
    │   │   │   │   │   ├── subagent-input.ts
    │   │   │   │   │   ├── system-context.ts
    │   │   │   │   │   ├── tool-call-loop-guard.ts
    │   │   │   │   │   ├── write-gate.ts
    │   │   │   │   │   └── tests/
    │   │   │   │   │       ├── write-gate-basepath.test.ts
    │   │   │   │   │       └── write-gate-shouldblock-basepath.test.ts
    │   │   │   │   ├── commands/
    │   │   │   │   │   ├── catalog.ts
    │   │   │   │   │   ├── context.ts
    │   │   │   │   │   ├── dispatcher.ts
    │   │   │   │   │   ├── index.ts
    │   │   │   │   │   └── handlers/
    │   │   │   │   │       ├── auto.ts
    │   │   │   │   │       ├── core.ts
    │   │   │   │   │       ├── escalate.ts
    │   │   │   │   │       ├── notifications-handler.ts
    │   │   │   │   │       ├── onboarding.ts
    │   │   │   │   │       ├── ops.ts
    │   │   │   │   │       ├── parallel.ts
    │   │   │   │   │       └── workflow.ts
    │   │   │   │   ├── db/
    │   │   │   │   │   ├── auto-workers.ts
    │   │   │   │   │   ├── command-queue.ts
    │   │   │   │   │   ├── milestone-leases.ts
    │   │   │   │   │   ├── runtime-kv.ts
    │   │   │   │   │   └── unit-dispatches.ts
    │   │   │   │   ├── docs/
    │   │   │   │   │   ├── claude-marketplace-import.md
    │   │   │   │   │   ├── COORDINATION.md
    │   │   │   │   │   └── preferences-reference.md
    │   │   │   │   ├── ecosystem/
    │   │   │   │   │   ├── gsd-extension-api.ts
    │   │   │   │   │   └── loader.ts
    │   │   │   │   ├── migrate/
    │   │   │   │   │   ├── command.ts
    │   │   │   │   │   ├── index.ts
    │   │   │   │   │   ├── parser.ts
    │   │   │   │   │   ├── parsers.ts
    │   │   │   │   │   ├── preview.ts
    │   │   │   │   │   ├── transformer.ts
    │   │   │   │   │   ├── types.ts
    │   │   │   │   │   ├── validator.ts
    │   │   │   │   │   └── writer.ts
    │   │   │   │   ├── prompts/
    │   │   │   │   │   ├── add-tests.md
    │   │   │   │   │   ├── complete-milestone.md
    │   │   │   │   │   ├── complete-slice.md
    │   │   │   │   │   ├── debug-diagnose.md
    │   │   │   │   │   ├── debug-session-manager.md
    │   │   │   │   │   ├── discuss-headless.md
    │   │   │   │   │   ├── discuss.md
    │   │   │   │   │   ├── doctor-heal.md
    │   │   │   │   │   ├── execute-task.md
    │   │   │   │   │   ├── forensics.md
    │   │   │   │   │   ├── gate-evaluate.md
    │   │   │   │   │   ├── guided-discuss-milestone.md
    │   │   │   │   │   ├── guided-discuss-project.md
    │   │   │   │   │   ├── guided-discuss-requirements.md
    │   │   │   │   │   ├── guided-discuss-slice.md
    │   │   │   │   │   ├── guided-research-decision.md
    │   │   │   │   │   ├── guided-research-project.md
    │   │   │   │   │   ├── guided-research-slice.md
    │   │   │   │   │   ├── guided-resume-task.md
    │   │   │   │   │   ├── guided-workflow-preferences.md
    │   │   │   │   │   ├── heal-skill.md
    │   │   │   │   │   ├── parallel-research-slices.md
    │   │   │   │   │   ├── plan-milestone.md
    │   │   │   │   │   ├── plan-slice.md
    │   │   │   │   │   ├── queue.md
    │   │   │   │   │   ├── quick-task.md
    │   │   │   │   │   ├── reactive-execute.md
    │   │   │   │   │   ├── reassess-roadmap.md
    │   │   │   │   │   ├── refine-slice.md
    │   │   │   │   │   ├── replan-slice.md
    │   │   │   │   │   ├── research-milestone.md
    │   │   │   │   │   ├── research-slice.md
    │   │   │   │   │   ├── rethink.md
    │   │   │   │   │   ├── review-migration.md
    │   │   │   │   │   ├── rewrite-docs.md
    │   │   │   │   │   ├── run-uat.md
    │   │   │   │   │   ├── scan.md
    │   │   │   │   │   ├── system.md
    │   │   │   │   │   ├── triage-captures.md
    │   │   │   │   │   ├── validate-milestone.md
    │   │   │   │   │   ├── workflow-oneshot.md
    │   │   │   │   │   ├── workflow-start.md
    │   │   │   │   │   └── worktree-merge.md
    │   │   │   │   ├── safety/
    │   │   │   │   │   ├── content-validator.ts
    │   │   │   │   │   ├── destructive-guard.ts
    │   │   │   │   │   ├── evidence-collector.ts
    │   │   │   │   │   ├── evidence-cross-ref.ts
    │   │   │   │   │   ├── file-change-validator.ts
    │   │   │   │   │   ├── git-checkpoint.ts
    │   │   │   │   │   └── safety-harness.ts
    │   │   │   │   ├── schemas/
    │   │   │   │   │   ├── parsers.ts
    │   │   │   │   │   ├── validate.ts
    │   │   │   │   │   └── __fixtures__/
    │   │   │   │   │       ├── valid-project.md
    │   │   │   │   │       ├── valid-requirements.md
    │   │   │   │   │       └── valid-roadmap.md
    │   │   │   │   ├── skills/
    │   │   │   │   │   └── gsd-headless/
    │   │   │   │   │       ├── SKILL.md
    │   │   │   │   │       └── references/
    │   │   │   │   │           ├── answer-injection.md
    │   │   │   │   │           ├── commands.md
    │   │   │   │   │           └── multi-session.md
    │   │   │   │   ├── state-reconciliation/
    │   │   │   │   │   ├── errors.ts
    │   │   │   │   │   ├── index.ts
    │   │   │   │   │   ├── registry.ts
    │   │   │   │   │   ├── spawn-gate.ts
    │   │   │   │   │   ├── types.ts
    │   │   │   │   │   └── drift/
    │   │   │   │   │       ├── completion.ts
    │   │   │   │   │       ├── merge-state.ts
    │   │   │   │   │       ├── project-md.ts
    │   │   │   │   │       ├── roadmap.ts
    │   │   │   │   │       ├── sketch-flag.ts
    │   │   │   │   │       ├── stale-render.ts
    │   │   │   │   │       └── stale-worker.ts
    │   │   │   │   ├── templates/
    │   │   │   │   │   ├── context.md
    │   │   │   │   │   ├── decisions.md
    │   │   │   │   │   ├── knowledge.md
    │   │   │   │   │   ├── milestone-summary.md
    │   │   │   │   │   ├── milestone-validation.md
    │   │   │   │   │   ├── plan.md
    │   │   │   │   │   ├── PREFERENCES.md
    │   │   │   │   │   ├── project.md
    │   │   │   │   │   ├── reassessment.md
    │   │   │   │   │   ├── requirements.md
    │   │   │   │   │   ├── research.md
    │   │   │   │   │   ├── roadmap.md
    │   │   │   │   │   ├── runtime.md
    │   │   │   │   │   ├── secrets-manifest.md
    │   │   │   │   │   ├── slice-context.md
    │   │   │   │   │   ├── slice-summary.md
    │   │   │   │   │   ├── state.md
    │   │   │   │   │   ├── task-plan.md
    │   │   │   │   │   ├── task-summary.md
    │   │   │   │   │   └── uat.md
    │   │   │   │   ├── tests/
    │   │   │   │   │   ├── active-milestone-id-guard.test.ts
    │   │   │   │   │   ├── activity-log.test.ts
    │   │   │   │   │   ├── agent-end-retry.test.ts
    │   │   │   │   │   ├── artifact-corruption-2630.test.ts
    │   │   │   │   │   ├── artifact-retry-cap.test.ts
    │   │   │   │   │   ├── artifact-validators.test.ts
    │   │   │   │   │   ├── artifacts-table-preserved-on-cache-invalidate.test.ts
    │   │   │   │   │   ├── ask-user-questions-dedup.test.ts
    │   │   │   │   │   ├── atomic-write.test.ts
    │   │   │   │   │   ├── auto-abort-pause-regression.test.ts
    │   │   │   │   │   ├── auto-budget-alerts.test.ts
    │   │   │   │   │   ├── auto-dashboard.test.ts
    │   │   │   │   │   ├── auto-deterministic-error-classification-4973.test.ts
    │   │   │   │   │   ├── auto-discuss-milestone-deadlock-4973.test.ts
    │   │   │   │   │   ├── auto-loop-no-copy-artifacts.test.ts
    │   │   │   │   │   ├── auto-loop-symlink-worktree.test.ts
    │   │   │   │   │   ├── auto-migrating-recovery.test.ts
    │   │   │   │   │   ├── auto-milestone-target.test.ts
    │   │   │   │   │   ├── auto-mode-guards.test.ts
    │   │   │   │   │   ├── auto-mode-interactive-guard.test.ts
    │   │   │   │   │   ├── auto-model-selection-tool-poisoning.test.ts
    │   │   │   │   │   ├── auto-model-selection.test.ts
    │   │   │   │   │   ├── auto-orchestrator.test.ts
    │   │   │   │   │   ├── auto-paused-session-validation.test.ts
    │   │   │   │   │   ├── auto-paused-ui-cleanup.test.ts
    │   │   │   │   │   ├── auto-phases-lifecycle.test.ts
    │   │   │   │   │   ├── auto-post-unit-step-message.test.ts
    │   │   │   │   │   ├── auto-pr-bugs.test.ts
    │   │   │   │   │   ├── auto-project-root-env.test.ts
    │   │   │   │   │   ├── auto-prompts-fallback.test.ts
    │   │   │   │   │   ├── auto-remediate-slice-status.test.ts
    │   │   │   │   │   ├── auto-retry-mcp-churn-fixes.test.ts
    │   │   │   │   │   ├── auto-runtime-state.test.ts
    │   │   │   │   │   ├── auto-session-encapsulation.test.ts
    │   │   │   │   │   ├── auto-session-scope.test.ts
    │   │   │   │   │   ├── auto-start-bootstrap-await-3420.test.ts
    │   │   │   │   │   ├── auto-start-clean-runtime-db-gated.test.ts
    │   │   │   │   │   ├── auto-start-cold-db-bootstrap.test.ts
    │   │   │   │   │   ├── auto-start-index-lock.test.ts
    │   │   │   │   │   ├── auto-start-needs-discussion.test.ts
    │   │   │   │   │   ├── auto-start-orphan-bootstrap.test.ts
    │   │   │   │   │   ├── auto-start-time-persistence.test.ts
    │   │   │   │   │   ├── auto-start-worktree-db-path.test.ts
    │   │   │   │   │   ├── auto-stop-notification.test.ts
    │   │   │   │   │   ├── auto-supervisor.test.mjs
    │   │   │   │   │   ├── auto-thinking-restore.test.ts
    │   │   │   │   │   ├── auto-unit-closeout.test.ts
    │   │   │   │   │   ├── auto-warning-noise-regression.test.ts
    │   │   │   │   │   ├── auto-workers.test.ts
    │   │   │   │   │   ├── auto-worktree-auto-resolve.test.ts
    │   │   │   │   │   ├── auto-worktree-registry.test.ts
    │   │   │   │   │   ├── auto-wrapup-inflight-guard.test.ts
    │   │   │   │   │   ├── autocomplete-regressions-1675.test.ts
    │   │   │   │   │   ├── block-db-writes.test.ts
    │   │   │   │   │   ├── blocked-models.test.ts
    │   │   │   │   │   ├── bootstrap-derive-state-db-open.test.ts
    │   │   │   │   │   ├── brief-command.test.ts
    │   │   │   │   │   ├── browser-teardown.test.ts
    │   │   │   │   │   ├── browser-tools-compatibility-declarations.test.ts
    │   │   │   │   │   ├── budget-prediction.test.ts
    │   │   │   │   │   ├── bundled-skill-triggers.test.ts
    │   │   │   │   │   ├── bundled-workflow-defs.test.ts
    │   │   │   │   │   ├── cache-staleness-regression.test.ts
    │   │   │   │   │   ├── canonical-milestone-root.test.ts
    │   │   │   │   │   ├── capability-router.test.ts
    │   │   │   │   │   ├── captures.test.ts
    │   │   │   │   │   ├── check-auto-start-pending-gate.test.ts
    │   │   │   │   │   ├── check-auto-start-ready-guard.test.ts
    │   │   │   │   │   ├── checkout-branch-stash-guard.test.ts
    │   │   │   │   │   ├── claude-import-marketplace-discovery.test.ts
    │   │   │   │   │   ├── claude-import-tui.test.ts
    │   │   │   │   │   ├── claude-skill-dirs.test.ts
    │   │   │   │   │   ├── clean-root-preflight.test.ts
    │   │   │   │   │   ├── clear-stale-autostart.test.ts
    │   │   │   │   │   ├── cli-provider-rate-limit.test.ts
    │   │   │   │   │   ├── closeout-git-deferral.test.ts
    │   │   │   │   │   ├── cmux.test.ts
    │   │   │   │   │   ├── codebase-generator.test.ts
    │   │   │   │   │   ├── cold-resume-db-reopen.test.ts
    │   │   │   │   │   ├── collect-from-manifest.test.ts
    │   │   │   │   │   ├── collision-diagnostics.test.ts
    │   │   │   │   │   ├── command-queue.test.ts
    │   │   │   │   │   ├── commands-backlog.test.ts
    │   │   │   │   │   ├── commands-config.test.ts
    │   │   │   │   │   ├── commands-do.test.ts
    │   │   │   │   │   ├── commands-eval-review.test.ts
    │   │   │   │   │   ├── commands-extensions-version-compare.test.ts
    │   │   │   │   │   ├── commands-extract-learnings.test.ts
    │   │   │   │   │   ├── commands-inspect-open-db.test.ts
    │   │   │   │   │   ├── commands-logs.test.ts
    │   │   │   │   │   ├── commands-pr-branch.test.ts
    │   │   │   │   │   ├── commands-scan.test.ts
    │   │   │   │   │   ├── commands-session-report.test.ts
    │   │   │   │   │   ├── commands-ship-eval-warn.test.ts
    │   │   │   │   │   ├── commands-ship.test.ts
    │   │   │   │   │   ├── commands-verdict.test.ts
    │   │   │   │   │   ├── commands-workflow-custom.test.ts
    │   │   │   │   │   ├── commands-worktree-clean.test.ts
    │   │   │   │   │   ├── compaction-snapshot.test.ts
    │   │   │   │   │   ├── complete-milestone-excerpt.test.ts
    │   │   │   │   │   ├── complete-milestone-prompt-rendering.test.ts
    │   │   │   │   │   ├── complete-milestone.test.ts
    │   │   │   │   │   ├── complete-slice-composer.test.ts
    │   │   │   │   │   ├── complete-slice-gate-closure.test.ts
    │   │   │   │   │   ├── complete-slice-prompt-task-summary-layout.test.ts
    │   │   │   │   │   ├── complete-slice-string-coercion.test.ts
    │   │   │   │   │   ├── complete-slice-verification-gate.test.ts
    │   │   │   │   │   ├── complete-slice.test.ts
    │   │   │   │   │   ├── complete-task-normalize-lists.test.ts
    │   │   │   │   │   ├── complete-task-rollback-evidence.test.ts
    │   │   │   │   │   ├── complete-task.test.ts
    │   │   │   │   │   ├── completed-at-reconcile.test.ts
    │   │   │   │   │   ├── completed-units-metrics-sync.test.ts
    │   │   │   │   │   ├── completion-hierarchy-guards.test.ts
    │   │   │   │   │   ├── complexity-classifier.test.ts
    │   │   │   │   │   ├── component-loader.test.ts
    │   │   │   │   │   ├── component-types.test.ts
    │   │   │   │   │   ├── context-budget.test.ts
    │   │   │   │   │   ├── context-injector.test.ts
    │   │   │   │   │   ├── context-masker.test.ts
    │   │   │   │   │   ├── context-store-decisions-from-memories.test.ts
    │   │   │   │   │   ├── context-store.test.ts
    │   │   │   │   │   ├── core-overlay-fallback.test.ts
    │   │   │   │   │   ├── cost-projection.test.ts
    │   │   │   │   │   ├── crash-handler-secondary.test.ts
    │   │   │   │   │   ├── crash-recovery-via-db.test.ts
    │   │   │   │   │   ├── crash-recovery.test.ts
    │   │   │   │   │   ├── current-directory-root-homedir-fallback.test.ts
    │   │   │   │   │   ├── custom-engine-loop-integration.test.ts
    │   │   │   │   │   ├── custom-verification.test.ts
    │   │   │   │   │   ├── custom-verify-retry-store.test.ts
    │   │   │   │   │   ├── custom-workflow-engine.test.ts
    │   │   │   │   │   ├── cwd-fallback-hardening.test.ts
    │   │   │   │   │   ├── dashboard-budget.test.ts
    │   │   │   │   │   ├── dashboard-custom-engine.test.ts
    │   │   │   │   │   ├── db-access-guardrails.test.ts
    │   │   │   │   │   ├── db-adapter.test.ts
    │   │   │   │   │   ├── db-authority-regression.test.ts
    │   │   │   │   │   ├── db-base-schema.test.ts
    │   │   │   │   │   ├── db-connection-cache.test.ts
    │   │   │   │   │   ├── db-coordination-schema.test.ts
    │   │   │   │   │   ├── db-decision-requirement-rows.test.ts
    │   │   │   │   │   ├── db-gate-rows.test.ts
    │   │   │   │   │   ├── db-lightweight-query-rows.test.ts
    │   │   │   │   │   ├── db-memory-fts-schema.test.ts
    │   │   │   │   │   ├── db-migration-backup.test.ts
    │   │   │   │   │   ├── db-migration-steps.integration.test.ts
    │   │   │   │   │   ├── db-migration-steps.test.ts
    │   │   │   │   │   ├── db-milestone-artifact-rows.test.ts
    │   │   │   │   │   ├── db-open-state.test.ts
    │   │   │   │   │   ├── db-path-worktree-symlink.test.ts
    │   │   │   │   │   ├── db-provider.test.ts
    │   │   │   │   │   ├── db-runtime-kv-schema.test.ts
    │   │   │   │   │   ├── db-schema-metadata.test.ts
    │   │   │   │   │   ├── db-task-slice-rows.test.ts
    │   │   │   │   │   ├── db-transaction.test.ts
    │   │   │   │   │   ├── db-verification-evidence-schema.test.ts
    │   │   │   │   │   ├── db-writer-path-containment.test.ts
    │   │   │   │   │   ├── db-writer-root-artifact.test.ts
    │   │   │   │   │   ├── db-writer-scope.test.ts
    │   │   │   │   │   ├── db-writer.test.ts
    │   │   │   │   │   ├── debug-command-handler.test.ts
    │   │   │   │   │   ├── debug-logger.test.ts
    │   │   │   │   │   ├── debug-session-store.test.ts
    │   │   │   │   │   ├── decision-scope-cascade.test.ts
    │   │   │   │   │   ├── decisions-projection-from-memories.test.ts
    │   │   │   │   │   ├── decisions-stop-table-writes.test.ts
    │   │   │   │   │   ├── deep-mode-integration.test.ts
    │   │   │   │   │   ├── deep-planning-mode-dispatch.test.ts
    │   │   │   │   │   ├── deep-project-setup-policy.test.ts
    │   │   │   │   │   ├── defer-milestone-stamp.test.ts
    │   │   │   │   │   ├── deferred-milestone-dir-4996.test.ts
    │   │   │   │   │   ├── deferred-slice-dispatch.test.ts
    │   │   │   │   │   ├── definition-io.test.ts
    │   │   │   │   │   ├── definition-loader.test.ts
    │   │   │   │   │   ├── delegation-policy.test.ts
    │   │   │   │   │   ├── derive-state-crossval.test.ts
    │   │   │   │   │   ├── derive-state-db-disk-reconcile.test.ts
    │   │   │   │   │   ├── derive-state-deps.test.ts
    │   │   │   │   │   ├── derive-state-draft.test.ts
    │   │   │   │   │   ├── derive-state-helpers.test.ts
    │   │   │   │   │   ├── derive-state.test.ts
    │   │   │   │   │   ├── detect-stuck-respects-retry.test.ts
    │   │   │   │   │   ├── dev-engine-wrapper.test.ts
    │   │   │   │   │   ├── diff-context.test.ts
    │   │   │   │   │   ├── discord-invite-links.test.ts
    │   │   │   │   │   ├── discuss-empty-db-fallback.test.ts
    │   │   │   │   │   ├── discuss-headless-rendering.test.ts
    │   │   │   │   │   ├── discuss-incremental-persistence.test.ts
    │   │   │   │   │   ├── discuss-milestone-structured-questions.test.ts
    │   │   │   │   │   ├── discuss-prompt.test.ts
    │   │   │   │   │   ├── discuss-queued-milestones.test.ts
    │   │   │   │   │   ├── discuss-tool-scope-leak.test.ts
    │   │   │   │   │   ├── discuss-tool-scoping.test.ts
    │   │   │   │   │   ├── dispatch-backgroundable-annotation.test.ts
    │   │   │   │   │   ├── dispatch-complete-milestone-guard.test.ts
    │   │   │   │   │   ├── dispatch-guard-closed-status.test.ts
    │   │   │   │   │   ├── dispatch-guard-summary-db-mismatch.test.ts
    │   │   │   │   │   ├── dispatch-guard.test.ts
    │   │   │   │   │   ├── dispatch-missing-task-plans.test.ts
    │   │   │   │   │   ├── dispatch-rule-coverage.test.ts
    │   │   │   │   │   ├── dispatch-uat-last-completed.test.ts
    │   │   │   │   │   ├── dispatcher-stuck-planning.test.ts
    │   │   │   │   │   ├── dist-redirect.mjs
    │   │   │   │   │   ├── doctor-empty-worktree.test.ts
    │   │   │   │   │   ├── doctor-fix-flag.test.ts
    │   │   │   │   │   ├── doctor-forensics-db-open-regression.test.ts
    │   │   │   │   │   ├── doctor-heal-fixable-warnings.test.ts
    │   │   │   │   │   ├── doctor-orphan-milestone-4996.test.ts
    │   │   │   │   │   ├── doctor-providers.test.ts
    │   │   │   │   │   ├── doctor-scope-db-unavailable.test.ts
    │   │   │   │   │   ├── double-merge-guard.test.ts
    │   │   │   │   │   ├── draft-promotion.test.ts
    │   │   │   │   │   ├── dynamic-routing-default.test.ts
    │   │   │   │   │   ├── engine-interfaces-contract.test.ts
    │   │   │   │   │   ├── enhanced-verification-integration.test.ts
    │   │   │   │   │   ├── ensure-db-open.test.ts
    │   │   │   │   │   ├── ensure-preconditions-guard-4996.test.ts
    │   │   │   │   │   ├── error-success-mask.test.ts
    │   │   │   │   │   ├── escalation.test.ts
    │   │   │   │   │   ├── est-annotation-timeout.test.ts
    │   │   │   │   │   ├── eval-review-schema.test.ts
    │   │   │   │   │   ├── event-replay-idempotency.test.ts
    │   │   │   │   │   ├── evidence-cross-ref.test.ts
    │   │   │   │   │   ├── exec-history.test.ts
    │   │   │   │   │   ├── exec-sandbox.test.ts
    │   │   │   │   │   ├── execute-summary-save-empty-project.test.ts
    │   │   │   │   │   ├── execute-task-prompt-existing-artifact-guard.test.ts
    │   │   │   │   │   ├── execute-task-rendering.test.ts
    │   │   │   │   │   ├── execution-entry-missing-context-4671.test.ts
    │   │   │   │   │   ├── exit-command.test.ts
    │   │   │   │   │   ├── export-html-all.test.ts
    │   │   │   │   │   ├── export-html-enhancements.test.ts
    │   │   │   │   │   ├── extension-bootstrap-isolation.test.ts
    │   │   │   │   │   ├── extension-selector-separator.test.ts
    │   │   │   │   │   ├── false-degraded-mode-warning.test.ts
    │   │   │   │   │   ├── fast-forward-reused-milestone-branch.test.ts
    │   │   │   │   │   ├── file-change-validator.test.ts
    │   │   │   │   │   ├── file-lock.test.ts
    │   │   │   │   │   ├── files-loadfile-eisdir.test.ts
    │   │   │   │   │   ├── finalize-survivor-branch.test.ts
    │   │   │   │   │   ├── finalize-timeout-guard.test.ts
    │   │   │   │   │   ├── find-missing-summaries-closed-runtime.test.ts
    │   │   │   │   │   ├── flag-file-db.test.ts
    │   │   │   │   │   ├── flat-rate-routing-guard.test.ts
    │   │   │   │   │   ├── forensics-error-filter.test.ts
    │   │   │   │   │   ├── forensics-issue-routing.test.ts
    │   │   │   │   │   ├── forensics-prompt-rendering.test.ts
    │   │   │   │   │   ├── forensics-stuck-loops.test.ts
    │   │   │   │   │   ├── format-shortcut.test.ts
    │   │   │   │   │   ├── freeform-decisions.test.ts
    │   │   │   │   │   ├── frontmatter-parse-noise.test.ts
    │   │   │   │   │   ├── gate-1b-orphan-discrimination.test.ts
    │   │   │   │   │   ├── gate-1b-recovery-bound-corrections.test.ts
    │   │   │   │   │   ├── gate-1b-recovery-bound.test.ts
    │   │   │   │   │   ├── gate-dispatch.test.ts
    │   │   │   │   │   ├── gate-registry.test.ts
    │   │   │   │   │   ├── gate-state-canonicalization.test.ts
    │   │   │   │   │   ├── gate-storage.test.ts
    │   │   │   │   │   ├── git-checkpoint.test.ts
    │   │   │   │   │   ├── gitignore-bg-shell-runtime.test.ts
    │   │   │   │   │   ├── google-search-stub.test.ts
    │   │   │   │   │   ├── graph-context.test.ts
    │   │   │   │   │   ├── graph-operations.test.ts
    │   │   │   │   │   ├── gsd-db-failed-open-restore.test.ts
    │   │   │   │   │   ├── gsd-db-workspace-scope.test.ts
    │   │   │   │   │   ├── gsd-db.test.ts
    │   │   │   │   │   ├── gsd-home.test.ts
    │   │   │   │   │   ├── gsd-inspect.test.ts
    │   │   │   │   │   ├── gsd-no-project-error-runtime.test.ts
    │   │   │   │   │   ├── gsd-recover.test.ts
    │   │   │   │   │   ├── gsd-root-canonical.test.ts
    │   │   │   │   │   ├── gsd-root-home-guard.test.ts
    │   │   │   │   │   ├── gsd-tools.test.ts
    │   │   │   │   │   ├── gsdroot-worktree-detection.test.ts
    │   │   │   │   │   ├── guided-discuss-milestone-prompt-rendering.test.ts
    │   │   │   │   │   ├── guided-discuss-project-prompt-rendering.test.ts
    │   │   │   │   │   ├── guided-discuss-requirements-prompt-rendering.test.ts
    │   │   │   │   │   ├── guided-dispatch-root.test.ts
    │   │   │   │   │   ├── guided-flow-dynamic-routing.test.ts
    │   │   │   │   │   ├── guided-flow-prompt-consolidation.test.ts
    │   │   │   │   │   ├── guided-flow-session-isolation.test.ts
    │   │   │   │   │   ├── guided-flow-state-rebuild.test.ts
    │   │   │   │   │   ├── guided-flow.test.ts
    │   │   │   │   │   ├── guided-tool-contract.test.ts
    │   │   │   │   │   ├── handler-worktree-write-isolation.test.ts
    │   │   │   │   │   ├── has-pending-deep-stage.test.ts
    │   │   │   │   │   ├── header-renderer.test.ts
    │   │   │   │   │   ├── headless-answers.test.ts
    │   │   │   │   │   ├── headless-milestone-parity.test.ts
    │   │   │   │   │   ├── headless-query.test.ts
    │   │   │   │   │   ├── health-widget.test.ts
    │   │   │   │   │   ├── help-menu-coverage.test.ts
    │   │   │   │   │   ├── hook-key-parsing.test.ts
    │   │   │   │   │   ├── hook-model-resolution.test.ts
    │   │   │   │   │   ├── import-done-milestones-runtime.test.ts
    │   │   │   │   │   ├── in-flight-tool-tracking.test.ts
    │   │   │   │   │   ├── infra-error.test.ts
    │   │   │   │   │   ├── infra-errors-cooldown.test.ts
    │   │   │   │   │   ├── init-prefs-routing.test.ts
    │   │   │   │   │   ├── init-skip-git.test.ts
    │   │   │   │   │   ├── init-wizard.test.ts
    │   │   │   │   │   ├── insert-slice-no-wipe.test.ts
    │   │   │   │   │   ├── integration-edge.test.ts
    │   │   │   │   │   ├── interactive-routing-bypass.test.ts
    │   │   │   │   │   ├── interactive-tool-idle-exemption.test.ts
    │   │   │   │   │   ├── interrupted-session-auto.test.ts
    │   │   │   │   │   ├── interrupted-session-ui.test.ts
    │   │   │   │   │   ├── isolation-none-branch-guard.test.ts
    │   │   │   │   │   ├── issue-4540-regressions.test.ts
    │   │   │   │   │   ├── iterate-engine-integration.test.ts
    │   │   │   │   │   ├── journal-query-tool.test.ts
    │   │   │   │   │   ├── journal.test.ts
    │   │   │   │   │   ├── json-persistence-atomic.test.ts
    │   │   │   │   │   ├── key-manager.test.ts
    │   │   │   │   │   ├── knowledge-backfill-projection.test.ts
    │   │   │   │   │   ├── knowledge-capture.test.ts
    │   │   │   │   │   ├── knowledge.test.ts
    │   │   │   │   │   ├── lazy-pi-tui-import.test.ts
    │   │   │   │   │   ├── legacy-component-format-telemetry.test.ts
    │   │   │   │   │   ├── legacy-telemetry.test.ts
    │   │   │   │   │   ├── load-knowledge-block-rules-only.test.ts
    │   │   │   │   │   ├── load-memory-block.test.ts
    │   │   │   │   │   ├── manifest-status.test.ts
    │   │   │   │   │   ├── marketplace-test-fixtures.ts
    │   │   │   │   │   ├── mcp-client-security.test.ts
    │   │   │   │   │   ├── mcp-filter.test.ts
    │   │   │   │   │   ├── mcp-project-config.test.ts
    │   │   │   │   │   ├── mcp-status.test.ts
    │   │   │   │   │   ├── md-importer.test.ts
    │   │   │   │   │   ├── measurement.test.ts
    │   │   │   │   │   ├── memory-consolidation-scanner.test.ts
    │   │   │   │   │   ├── memory-decay-factor.test.ts
    │   │   │   │   │   ├── memory-embeddings.test.ts
    │   │   │   │   │   ├── memory-extractor.test.ts
    │   │   │   │   │   ├── memory-ingest.test.ts
    │   │   │   │   │   ├── memory-leak-guards.test.ts
    │   │   │   │   │   ├── memory-maintenance.test.ts
    │   │   │   │   │   ├── memory-pressure-stuck-state.test.ts
    │   │   │   │   │   ├── memory-relations.test.ts
    │   │   │   │   │   ├── memory-store.test.ts
    │   │   │   │   │   ├── memory-tools.test.ts
    │   │   │   │   │   ├── merge-conflict-stops-loop.test.ts
    │   │   │   │   │   ├── merge-db-cycle.test.ts
    │   │   │   │   │   ├── merge-self-branch-guard.test.ts
    │   │   │   │   │   ├── metrics-atomic-merge.test.ts
    │   │   │   │   │   ├── metrics-lock-hardening.test.ts
    │   │   │   │   │   ├── metrics-lock-not-acquired.test.ts
    │   │   │   │   │   ├── metrics-lock-retry-sleep.test.ts
    │   │   │   │   │   ├── metrics-prune-cache-invalidation.test.ts
    │   │   │   │   │   ├── metrics-scope.test.ts
    │   │   │   │   │   ├── metrics.test.ts
    │   │   │   │   │   ├── migrate-external-worktree.test.ts
    │   │   │   │   │   ├── migrate-hierarchy.test.ts
    │   │   │   │   │   ├── migrate-parser.test.ts
    │   │   │   │   │   ├── migrate-transformer.test.ts
    │   │   │   │   │   ├── migrate-validator-parsers.test.ts
    │   │   │   │   │   ├── migrate-writer-integration.test.ts
    │   │   │   │   │   ├── migrate-writer.test.ts
    │   │   │   │   │   ├── migration-auto-check.test.ts
    │   │   │   │   │   ├── milestone-id-gap-reuse-4996.test.ts
    │   │   │   │   │   ├── milestone-id-reservation.test.ts
    │   │   │   │   │   ├── milestone-leases.test.ts
    │   │   │   │   │   ├── milestone-merge-stash-restore.test.ts
    │   │   │   │   │   ├── milestone-report-path.test.ts
    │   │   │   │   │   ├── milestone-scope-classifier.test.ts
    │   │   │   │   │   ├── milestone-status-authoritative.test.ts
    │   │   │   │   │   ├── milestone-status-tool.test.ts
    │   │   │   │   │   ├── milestone-summary-classifier.test.ts
    │   │   │   │   │   ├── milestone-transition-state-rebuild.test.ts
    │   │   │   │   │   ├── model-cost-table.test.ts
    │   │   │   │   │   ├── model-isolation.test.ts
    │   │   │   │   │   ├── model-router.test.ts
    │   │   │   │   │   ├── model-unittype-mapping.test.ts
    │   │   │   │   │   ├── must-have-parser.test.ts
    │   │   │   │   │   ├── namespaced-registry.test.ts
    │   │   │   │   │   ├── namespaced-resolver.test.ts
    │   │   │   │   │   ├── native-git-bridge-exec-fallback.test.ts
    │   │   │   │   │   ├── native-git-infra-errors.test.ts
    │   │   │   │   │   ├── native-has-changes-cache.test.ts
    │   │   │   │   │   ├── needs-remediation-revalidation.test.ts
    │   │   │   │   │   ├── next-milestone-id.test.ts
    │   │   │   │   │   ├── none-mode-gates.test.ts
    │   │   │   │   │   ├── note-captures-executed.test.ts
    │   │   │   │   │   ├── notification-overlay.test.ts
    │   │   │   │   │   ├── notification-store.test.ts
    │   │   │   │   │   ├── notification-widget.test.ts
    │   │   │   │   │   ├── notifications-handler.test.ts
    │   │   │   │   │   ├── notifications.test.ts
    │   │   │   │   │   ├── onboarding-state.test.ts
    │   │   │   │   │   ├── originalbase-path-comparison.test.ts
    │   │   │   │   │   ├── orphan-merge-bootstrap.test.ts
    │   │   │   │   │   ├── orphan-stash-audit.test.ts
    │   │   │   │   │   ├── orphaned-worktree-audit.test.ts
    │   │   │   │   │   ├── overrides.test.ts
    │   │   │   │   │   ├── parallel-budget-atomicity.test.ts
    │   │   │   │   │   ├── parallel-commit-scope.test.ts
    │   │   │   │   │   ├── parallel-crash-recovery.test.ts
    │   │   │   │   │   ├── parallel-eligibility-ghost.test.ts
    │   │   │   │   │   ├── parallel-milestone-isolation.test.ts
    │   │   │   │   │   ├── parallel-monitor-overlay.test.ts
    │   │   │   │   │   ├── parallel-orchestration.test.ts
    │   │   │   │   │   ├── parallel-orchestrator-fast-forward.test.ts
    │   │   │   │   │   ├── parallel-orchestrator-zombie-cleanup.test.ts
    │   │   │   │   │   ├── parallel-research-dispatch.test.ts
    │   │   │   │   │   ├── parallel-skill-prompt-integration.test.ts
    │   │   │   │   │   ├── parallel-worker-lock-contention.test.ts
    │   │   │   │   │   ├── parallel-worker-monitoring.test.ts
    │   │   │   │   │   ├── park-db-sync.test.ts
    │   │   │   │   │   ├── park-edge-cases.test.ts
    │   │   │   │   │   ├── park-milestone.test.ts
    │   │   │   │   │   ├── path-cache-decoupled.test.ts
    │   │   │   │   │   ├── path-normalization-unified.test.ts
    │   │   │   │   │   ├── paths-cache.test.ts
    │   │   │   │   │   ├── paused-session-via-db.test.ts
    │   │   │   │   │   ├── pending-autostart-scope.test.ts
    │   │   │   │   │   ├── phantom-ghost-detection.test.ts
    │   │   │   │   │   ├── phantom-milestone-default-queued.test.ts
    │   │   │   │   │   ├── phase-anchor.test.ts
    │   │   │   │   │   ├── phases-merge-error-stops-auto.test.ts
    │   │   │   │   │   ├── pipeline-variant-dispatch.test.ts
    │   │   │   │   │   ├── plan-gate-failed-doctor-heal-hint.test.ts
    │   │   │   │   │   ├── plan-milestone-artifact-verification.test.ts
    │   │   │   │   │   ├── plan-milestone-boundary-map-preservation.test.ts
    │   │   │   │   │   ├── plan-milestone-queue-context.test.ts
    │   │   │   │   │   ├── plan-milestone-rendering.test.ts
    │   │   │   │   │   ├── plan-milestone-sketch-render.test.ts
    │   │   │   │   │   ├── plan-milestone-title.test.ts
    │   │   │   │   │   ├── plan-milestone.test.ts
    │   │   │   │   │   ├── plan-quality-validator.test.ts
    │   │   │   │   │   ├── plan-slice-prompt.test.ts
    │   │   │   │   │   ├── plan-slice.test.ts
    │   │   │   │   │   ├── plan-task.test.ts
    │   │   │   │   │   ├── planning-crossval.test.ts
    │   │   │   │   │   ├── planning-depth-setter.test.ts
    │   │   │   │   │   ├── plugin-importer.test.ts
    │   │   │   │   │   ├── post-exec-retry-bypass.test.ts
    │   │   │   │   │   ├── post-execution-checks.test.ts
    │   │   │   │   │   ├── post-mutation-hook.test.ts
    │   │   │   │   │   ├── post-unit-git-failure.test.ts
    │   │   │   │   │   ├── post-unit-hooks.test.ts
    │   │   │   │   │   ├── post-unit-state-rebuild.test.ts
    │   │   │   │   │   ├── pr-evidence-equivalence.test.ts
    │   │   │   │   │   ├── pr-evidence-hardening.test.ts
    │   │   │   │   │   ├── pr-evidence.test.ts
    │   │   │   │   │   ├── pre-exec-backtick-strip.test.ts
    │   │   │   │   │   ├── pre-exec-gate-loop.test.ts
    │   │   │   │   │   ├── pre-execution-fail-closed.test.ts
    │   │   │   │   │   ├── pre-execution-pause-wiring.test.ts
    │   │   │   │   │   ├── preferences-formatting.test.ts
    │   │   │   │   │   ├── preferences-mcp.test.ts
    │   │   │   │   │   ├── preferences-worktree-sync.test.ts
    │   │   │   │   │   ├── preferences.test.ts
    │   │   │   │   │   ├── preflight-context-draft-filter.test.ts
    │   │   │   │   │   ├── prefs-wizard-coverage.test.ts
    │   │   │   │   │   ├── process-task-path.test.ts
    │   │   │   │   │   ├── progressive-planning.test.ts
    │   │   │   │   │   ├── project-relocation-recovery.test.ts
    │   │   │   │   │   ├── project-root-cwd-crash.test.ts
    │   │   │   │   │   ├── projection-no-plan-overwrite.test.ts
    │   │   │   │   │   ├── projection-regression.test.ts
    │   │   │   │   │   ├── prompt-budget-enforcement.test.ts
    │   │   │   │   │   ├── prompt-cache-optimizer.test.ts
    │   │   │   │   │   ├── prompt-contracts.test.ts
    │   │   │   │   │   ├── prompt-db.test.ts
    │   │   │   │   │   ├── prompt-duplication-cuts.test.ts
    │   │   │   │   │   ├── prompt-loader-extension-dir.test.ts
    │   │   │   │   │   ├── prompt-loader-replacement.test.ts
    │   │   │   │   │   ├── prompt-loader-working-directory.test.ts
    │   │   │   │   │   ├── prompt-loader.test.ts
    │   │   │   │   │   ├── prompt-ordering.test.ts
    │   │   │   │   │   ├── prompt-path-audit.test.ts
    │   │   │   │   │   ├── prompt-step-ordering.test.ts
    │   │   │   │   │   ├── prompt-system-gate-coverage.test.ts
    │   │   │   │   │   ├── prompt-tool-names.test.ts
    │   │   │   │   │   ├── prompts-no-gitignored-test-refs.test.ts
    │   │   │   │   │   ├── provider-errors.test.ts
    │   │   │   │   │   ├── provider-switch-observer.test.ts
    │   │   │   │   │   ├── python-resolver.test.ts
    │   │   │   │   │   ├── quality-gates.test.ts
    │   │   │   │   │   ├── query-tools-db-open.test.ts
    │   │   │   │   │   ├── queue-auto-guard.test.ts
    │   │   │   │   │   ├── queue-draft-detection.test.ts
    │   │   │   │   │   ├── queue-execution-guard.test.ts
    │   │   │   │   │   ├── queue-order.test.ts
    │   │   │   │   │   ├── queue-prompt-rendering.test.ts
    │   │   │   │   │   ├── queue-reorder-ui.test.ts
    │   │   │   │   │   ├── queued-discuss-fast-path.test.ts
    │   │   │   │   │   ├── quick-auto-guard.test.ts
    │   │   │   │   │   ├── quick-external-gsd.test.ts
    │   │   │   │   │   ├── quick-turn-end-cleanup.test.ts
    │   │   │   │   │   ├── rate-limit-model-fallback.test.ts
    │   │   │   │   │   ├── reactive-executor.test.ts
    │   │   │   │   │   ├── reactive-graph.test.ts
    │   │   │   │   │   ├── ready-phrase-no-files-4573.test.ts
    │   │   │   │   │   ├── reassess-default-optin.test.ts
    │   │   │   │   │   ├── reassess-detection.test.ts
    │   │   │   │   │   ├── reassess-handler.test.ts
    │   │   │   │   │   ├── reassess-prompt.test.ts
    │   │   │   │   │   ├── reconciliation-edge-cases.test.ts
    │   │   │   │   │   ├── recovery-attempts-reset.test.ts
    │   │   │   │   │   ├── regex-hardening.test.ts
    │   │   │   │   │   ├── register-extension-guard.test.ts
    │   │   │   │   │   ├── register-hooks-compaction-checkpoint.test.ts
    │   │   │   │   │   ├── register-hooks-depth-verification.test.ts
    │   │   │   │   │   ├── register-shortcuts.test.ts
    │   │   │   │   │   ├── remediation-completion-guard.test.ts
    │   │   │   │   │   ├── remote-notification-from-desktop.test.ts
    │   │   │   │   │   ├── remote-questions.test.ts
    │   │   │   │   │   ├── remote-status.test.ts
    │   │   │   │   │   ├── reopen-slice.test.ts
    │   │   │   │   │   ├── reopen-task.test.ts
    │   │   │   │   │   ├── replan-handler.test.ts
    │   │   │   │   │   ├── replan-slice.test.ts
    │   │   │   │   │   ├── repo-identity-worktree.test.ts
    │   │   │   │   │   ├── repository-registry.test.ts
    │   │   │   │   │   ├── require-slice-discussion-dispatch.test.ts
    │   │   │   │   │   ├── requirements.test.ts
    │   │   │   │   │   ├── research-milestone-composer.test.ts
    │   │   │   │   │   ├── resolve-ts-hooks.mjs
    │   │   │   │   │   ├── resolve-ts.mjs
    │   │   │   │   │   ├── resource-loader-import-path.test.ts
    │   │   │   │   │   ├── restore-tools-after-discuss.test.ts
    │   │   │   │   │   ├── resume-dispatch-worktree.test.ts
    │   │   │   │   │   ├── resume-missing-worktree-warning.test.ts
    │   │   │   │   │   ├── retry-diagnostic-reasoning.test.ts
    │   │   │   │   │   ├── retry-state-reset.test.ts
    │   │   │   │   │   ├── rewrite-count-persist.test.ts
    │   │   │   │   │   ├── rewrite-docs-abandon-detect.test.ts
    │   │   │   │   │   ├── right-sized-workflow-prompts.test.ts
    │   │   │   │   │   ├── roadmap-parse-regression.test.ts
    │   │   │   │   │   ├── roadmap-slices.test.ts
    │   │   │   │   │   ├── rogue-file-detection.test.ts
    │   │   │   │   │   ├── routing-history.test.ts
    │   │   │   │   │   ├── rule-registry.test.ts
    │   │   │   │   │   ├── run-manager.test.ts
    │   │   │   │   │   ├── run-uat-composer.test.ts
    │   │   │   │   │   ├── run-uat-replay-cap.test.ts
    │   │   │   │   │   ├── runtime-invariant-modules.test.ts
    │   │   │   │   │   ├── runtime-kv.test.ts
    │   │   │   │   │   ├── safety-harness-false-positives.test.ts
    │   │   │   │   │   ├── save-gate-result-render.test.ts
    │   │   │   │   │   ├── schema-v21-sequence.test.ts
    │   │   │   │   │   ├── schema-v27-v28-sequence.test.ts
    │   │   │   │   │   ├── schema-v9-sequence.test.ts
    │   │   │   │   │   ├── secure-env-collect.test.ts
    │   │   │   │   │   ├── select-resumable-milestone.test.ts
    │   │   │   │   │   ├── service-tier.test.ts
    │   │   │   │   │   ├── session-lock-multipath.test.ts
    │   │   │   │   │   ├── session-lock-regression.test.ts
    │   │   │   │   │   ├── session-lock-transient-read.test.ts
    │   │   │   │   │   ├── session-model-override.test.ts
    │   │   │   │   │   ├── session-start-footer.test.ts
    │   │   │   │   │   ├── session-switch-abort-misclassification.test.ts
    │   │   │   │   │   ├── shared-wal.test.ts
    │   │   │   │   │   ├── show-config-command.test.ts
    │   │   │   │   │   ├── sidecar-queue.test.ts
    │   │   │   │   │   ├── signal-handlers.test.ts
    │   │   │   │   │   ├── silent-catch-diagnostics.test.ts
    │   │   │   │   │   ├── single-writer-invariant.test.ts
    │   │   │   │   │   ├── single-writer-v3-tool-surface.test.ts
    │   │   │   │   │   ├── skill-activation.test.ts
    │   │   │   │   │   ├── skill-catalog.test.ts
    │   │   │   │   │   ├── skill-lifecycle.test.ts
    │   │   │   │   │   ├── skill-manifest.test.ts
    │   │   │   │   │   ├── skip-slice-cascades-tasks.test.ts
    │   │   │   │   │   ├── skip-slice-state-rebuild.test.ts
    │   │   │   │   │   ├── skipped-validation-completion.test.ts
    │   │   │   │   │   ├── skipped-validation-db-atomicity.test.ts
    │   │   │   │   │   ├── slice-cadence.test.ts
    │   │   │   │   │   ├── slice-context-injection.test.ts
    │   │   │   │   │   ├── slice-disk-reconcile.test.ts
    │   │   │   │   │   ├── slice-parallel-conflict.test.ts
    │   │   │   │   │   ├── slice-parallel-eligibility.test.ts
    │   │   │   │   │   ├── slice-parallel-orchestrator.test.ts
    │   │   │   │   │   ├── slice-sequence-insert.test.ts
    │   │   │   │   │   ├── smart-entry-complete.test.ts
    │   │   │   │   │   ├── smart-entry-draft.test.ts
    │   │   │   │   │   ├── smart-entry-routing.test.ts
    │   │   │   │   │   ├── sqlite-unavailable-gate.test.ts
    │   │   │   │   │   ├── stale-dirlistcache-4648.test.ts
    │   │   │   │   │   ├── stale-lockfile-recovery.test.ts
    │   │   │   │   │   ├── stale-milestone-id-reservation.test.ts
    │   │   │   │   │   ├── stale-queued-milestone.test.ts
    │   │   │   │   │   ├── stale-slice-rows.test.ts
    │   │   │   │   │   ├── stale-worktree-cwd.test.ts
    │   │   │   │   │   ├── stalled-tool-recovery.test.ts
    │   │   │   │   │   ├── start-auto-detached.test.ts
    │   │   │   │   │   ├── stash-pop-gsd-conflict.test.ts
    │   │   │   │   │   ├── stash-queued-context-files.test.ts
    │   │   │   │   │   ├── state-corruption-2945.test.ts
    │   │   │   │   │   ├── state-derivation-parity.test.ts
    │   │   │   │   │   ├── state-reconciliation-drift.test.ts
    │   │   │   │   │   ├── state-transition-matrix.test.ts
    │   │   │   │   │   ├── status-db-open.test.ts
    │   │   │   │   │   ├── status-guards.test.ts
    │   │   │   │   │   ├── steer-worktree-path.test.ts
    │   │   │   │   │   ├── stop-auto-merge-back.test.ts
    │   │   │   │   │   ├── stop-auto-race-null-unit.test.ts
    │   │   │   │   │   ├── stop-backtrack.test.ts
    │   │   │   │   │   ├── structured-data-formatter.test.ts
    │   │   │   │   │   ├── stuck-detection-coverage.test.ts
    │   │   │   │   │   ├── stuck-state-via-db.test.ts
    │   │   │   │   │   ├── subagent-agent-discovery.test.ts
    │   │   │   │   │   ├── subagent-model-dispatch.test.ts
    │   │   │   │   │   ├── summary-render-parity.test.ts
    │   │   │   │   │   ├── survivor-branch-complete.test.ts
    │   │   │   │   │   ├── symlink-extension-discovery.test.ts
    │   │   │   │   │   ├── symlink-numbered-variants.test.ts
    │   │   │   │   │   ├── sync-lock.test.ts
    │   │   │   │   │   ├── sync-worktree-skip-current.test.ts
    │   │   │   │   │   ├── system-context-memory.test.ts
    │   │   │   │   │   ├── system-context-message-routing.test.ts
    │   │   │   │   │   ├── teardown-chdir-failure-clears-registry.test.ts
    │   │   │   │   │   ├── teardown-cleanup-parity.test.ts
    │   │   │   │   │   ├── teardown-failure-clears-registry.test.ts
    │   │   │   │   │   ├── terminated-transient.test.ts
    │   │   │   │   │   ├── test-helpers.test.ts
    │   │   │   │   │   ├── test-helpers.ts
    │   │   │   │   │   ├── test-utils.ts
    │   │   │   │   │   ├── token-cost-display.test.ts
    │   │   │   │   │   ├── token-counter.test.ts
    │   │   │   │   │   ├── token-profile.test.ts
    │   │   │   │   │   ├── token-tool-gating.test.ts
    │   │   │   │   │   ├── tool-call-loop-guard.test.ts
    │   │   │   │   │   ├── tool-compatibility.test.ts
    │   │   │   │   │   ├── tool-invocation-error-loop-break.test.ts
    │   │   │   │   │   ├── tool-naming.test.ts
    │   │   │   │   │   ├── tool-param-optionality.test.ts
    │   │   │   │   │   ├── triage-dispatch.test.ts
    │   │   │   │   │   ├── triage-resolution.test.ts
    │   │   │   │   │   ├── tui-header-lifecycle.test.ts
    │   │   │   │   │   ├── tui-render-kit.test.ts
    │   │   │   │   │   ├── turn-epoch.test.ts
    │   │   │   │   │   ├── uat-stuck-loop-orphaned-worktree.test.ts
    │   │   │   │   │   ├── unborn-branch.test.ts
    │   │   │   │   │   ├── undo.test.ts
    │   │   │   │   │   ├── unique-milestone-ids.test.ts
    │   │   │   │   │   ├── unit-context-composer.test.ts
    │   │   │   │   │   ├── unit-context-manifest.test.ts
    │   │   │   │   │   ├── unit-dispatches.test.ts
    │   │   │   │   │   ├── unit-ownership.test.ts
    │   │   │   │   │   ├── unit-runtime.test.ts
    │   │   │   │   │   ├── unstructured-continue-context-injection.test.ts
    │   │   │   │   │   ├── uok-audit-unified.test.ts
    │   │   │   │   │   ├── uok-contracts.test.ts
    │   │   │   │   │   ├── uok-execution-graph.test.ts
    │   │   │   │   │   ├── uok-flags.test.ts
    │   │   │   │   │   ├── uok-gate-runner.test.ts
    │   │   │   │   │   ├── uok-gitops-turn-action.test.ts
    │   │   │   │   │   ├── uok-gitops-wiring.test.ts
    │   │   │   │   │   ├── uok-kernel-path.test.ts
    │   │   │   │   │   ├── uok-loop-adapter-writer.test.ts
    │   │   │   │   │   ├── uok-model-policy.test.ts
    │   │   │   │   │   ├── uok-parity-report.test.ts
    │   │   │   │   │   ├── uok-plan-v2-wiring.test.ts
    │   │   │   │   │   ├── uok-preferences.test.ts
    │   │   │   │   │   ├── uok-writer.test.ts
    │   │   │   │   │   ├── update-command.test.ts
    │   │   │   │   │   ├── vacuous-truth-slices.test.ts
    │   │   │   │   │   ├── vacuum-recovery.test.ts
    │   │   │   │   │   ├── validate-directory.test.ts
    │   │   │   │   │   ├── validate-extension-package.test.ts
    │   │   │   │   │   ├── validate-milestone-prompt-verification-classes.test.ts
    │   │   │   │   │   ├── validate-milestone-stuck-guard.test.ts
    │   │   │   │   │   ├── validate-milestone-write-order.test.ts
    │   │   │   │   │   ├── validate-milestone.test.ts
    │   │   │   │   │   ├── validation-gate-patterns.test.ts
    │   │   │   │   │   ├── validation.test.ts
    │   │   │   │   │   ├── validator-scope-parity.test.ts
    │   │   │   │   │   ├── verdict-parser.test.ts
    │   │   │   │   │   ├── verification-evidence.test.ts
    │   │   │   │   │   ├── verification-gate.test.ts
    │   │   │   │   │   ├── verification-operational-gate.test.ts
    │   │   │   │   │   ├── verification-retry-policy.test.ts
    │   │   │   │   │   ├── verification-verdict.test.ts
    │   │   │   │   │   ├── verify-artifact-tightened.test.ts
    │   │   │   │   │   ├── visualizer-critical-path.test.ts
    │   │   │   │   │   ├── visualizer-data.test.ts
    │   │   │   │   │   ├── visualizer-overlay.test.ts
    │   │   │   │   │   ├── visualizer-views.test.ts
    │   │   │   │   │   ├── wave1-critical-regressions.test.ts
    │   │   │   │   │   ├── wave2-events-regressions.test.ts
    │   │   │   │   │   ├── wave3-session-regressions.test.ts
    │   │   │   │   │   ├── wave4-write-safety-regressions.test.ts
    │   │   │   │   │   ├── wave5-consistency-regressions.test.ts
    │   │   │   │   │   ├── windows-path-normalization.test.ts
    │   │   │   │   │   ├── worker-model-override.test.ts
    │   │   │   │   │   ├── worker-registry.test.ts
    │   │   │   │   │   ├── workflow-custom-engine-dispatch-outcome.test.ts
    │   │   │   │   │   ├── workflow-custom-engine-iteration.test.ts
    │   │   │   │   │   ├── workflow-custom-engine-reconcile-outcome.test.ts
    │   │   │   │   │   ├── workflow-custom-engine-reconcile.test.ts
    │   │   │   │   │   ├── workflow-custom-engine-retry.test.ts
    │   │   │   │   │   ├── workflow-custom-engine-verify-outcome.test.ts
    │   │   │   │   │   ├── workflow-dispatch-claim.test.ts
    │   │   │   │   │   ├── workflow-dispatch-ledger.test.ts
    │   │   │   │   │   ├── workflow-events.test.ts
    │   │   │   │   │   ├── workflow-install.test.ts
    │   │   │   │   │   ├── workflow-iteration-completion.test.ts
    │   │   │   │   │   ├── workflow-journal-reporter.test.ts
    │   │   │   │   │   ├── workflow-kernel.test.ts
    │   │   │   │   │   ├── workflow-logger-audit.test.ts
    │   │   │   │   │   ├── workflow-logger-wiring.test.ts
    │   │   │   │   │   ├── workflow-logger.test.ts
    │   │   │   │   │   ├── workflow-manifest.test.ts
    │   │   │   │   │   ├── workflow-mcp-auto-prep.test.ts
    │   │   │   │   │   ├── workflow-mcp.test.ts
    │   │   │   │   │   ├── workflow-memory-pressure.test.ts
    │   │   │   │   │   ├── workflow-phase-reporter.test.ts
    │   │   │   │   │   ├── workflow-plugins.test.ts
    │   │   │   │   │   ├── workflow-projections.test.ts
    │   │   │   │   │   ├── workflow-protocol-excerpt.test.ts
    │   │   │   │   │   ├── workflow-reconcile.test.ts
    │   │   │   │   │   ├── workflow-session-lock.test.ts
    │   │   │   │   │   ├── workflow-sidecar-iteration.test.ts
    │   │   │   │   │   ├── workflow-sidecar-queue.test.ts
    │   │   │   │   │   ├── workflow-templates.test.ts
    │   │   │   │   │   ├── workflow-tool-executors.test.ts
    │   │   │   │   │   ├── workflow-turn-reporter.test.ts
    │   │   │   │   │   ├── workflow-unit-dispatch.test.ts
    │   │   │   │   │   ├── workflow-worker-heartbeat.test.ts
    │   │   │   │   │   ├── working-output-messages.test.ts
    │   │   │   │   │   ├── workspace-index.test.ts
    │   │   │   │   │   ├── workspace.test.ts
    │   │   │   │   │   ├── worktree-bugfix.test.ts
    │   │   │   │   │   ├── worktree-db-integration.test.ts
    │   │   │   │   │   ├── worktree-db-respawn-truncation.test.ts
    │   │   │   │   │   ├── worktree-db-same-file.test.ts
    │   │   │   │   │   ├── worktree-db.test.ts
    │   │   │   │   │   ├── worktree-expected-warnings.test.ts
    │   │   │   │   │   ├── worktree-git-pathspec.test.ts
    │   │   │   │   │   ├── worktree-health-dispatch.test.ts
    │   │   │   │   │   ├── worktree-health-monorepo.test.ts
    │   │   │   │   │   ├── worktree-health.test.ts
    │   │   │   │   │   ├── worktree-integration.test.ts
    │   │   │   │   │   ├── worktree-journal-events.test.ts
    │   │   │   │   │   ├── worktree-lifecycle.test.ts
    │   │   │   │   │   ├── worktree-main-branch.test.ts
    │   │   │   │   │   ├── worktree-manager.test.ts
    │   │   │   │   │   ├── worktree-nested-git-safety.test.ts
    │   │   │   │   │   ├── worktree-path-injection.test.ts
    │   │   │   │   │   ├── worktree-post-create-hook.test.ts
    │   │   │   │   │   ├── worktree-preferences-sync.test.ts
    │   │   │   │   │   ├── worktree-project-root-degrade.test.ts
    │   │   │   │   │   ├── worktree-root-resolution.test.ts
    │   │   │   │   │   ├── worktree-root.test.ts
    │   │   │   │   │   ├── worktree-safety.test.ts
    │   │   │   │   │   ├── worktree-state-projection.test.ts
    │   │   │   │   │   ├── worktree-submodule-safety.test.ts
    │   │   │   │   │   ├── worktree-symlink-removal.test.ts
    │   │   │   │   │   ├── worktree-sync-milestones.test.ts
    │   │   │   │   │   ├── worktree-sync-overwrite-loop.test.ts
    │   │   │   │   │   ├── worktree-sync-tasks.test.ts
    │   │   │   │   │   ├── worktree-teardown-safety.test.ts
    │   │   │   │   │   ├── worktree-telemetry.test.ts
    │   │   │   │   │   ├── worktree-write-gate.test.ts
    │   │   │   │   │   ├── worktree.test.ts
    │   │   │   │   │   ├── write-gate-planning-unit.test.ts
    │   │   │   │   │   ├── write-gate-predicates.test.ts
    │   │   │   │   │   ├── write-gate.test.ts
    │   │   │   │   │   ├── write-intercept.test.ts
    │   │   │   │   │   ├── zero-slice-roadmap-guided.test.ts
    │   │   │   │   │   ├── zombie-gsd-state.test.ts
    │   │   │   │   │   ├── fixtures/
    │   │   │   │   │   │   └── pr-body/
    │   │   │   │   │   │       ├── commands-ship-basic.md
    │   │   │   │   │   │       ├── commands-ship-empty-optionals.md
    │   │   │   │   │   │       ├── swarm-lane-no-blockers.md
    │   │   │   │   │   │       └── swarm-lane-with-blockers.md
    │   │   │   │   │   └── integration/
    │   │   │   │   │       ├── all-milestones-complete-merge.test.ts
    │   │   │   │   │       ├── atomic-task-closeout.test.ts
    │   │   │   │   │       ├── auto-preflight.test.ts
    │   │   │   │   │       ├── auto-recovery.test.ts
    │   │   │   │   │       ├── auto-secrets-gate.test.ts
    │   │   │   │   │       ├── auto-stash-merge.test.ts
    │   │   │   │   │       ├── auto-worktree-milestone-merge.test.ts
    │   │   │   │   │       ├── auto-worktree.test.ts
    │   │   │   │   │       ├── commands-eval-review.integration.test.ts
    │   │   │   │   │       ├── continue-here.test.ts
    │   │   │   │   │       ├── doctor-completion-deferral.test.ts
    │   │   │   │   │       ├── doctor-delimiter-fix.test.ts
    │   │   │   │   │       ├── doctor-enhancements.test.ts
    │   │   │   │   │       ├── doctor-environment-worktree.test.ts
    │   │   │   │   │       ├── doctor-environment.test.ts
    │   │   │   │   │       ├── doctor-false-positives.test.ts
    │   │   │   │   │       ├── doctor-fixlevel.test.ts
    │   │   │   │   │       ├── doctor-git-symlink-cwd.test.ts
    │   │   │   │   │       ├── doctor-git.test.ts
    │   │   │   │   │       ├── doctor-proactive.test.ts
    │   │   │   │   │       ├── doctor-roadmap-summary-atomicity.test.ts
    │   │   │   │   │       ├── doctor-runtime.test.ts
    │   │   │   │   │       ├── doctor.test.ts
    │   │   │   │   │       ├── e2e-workflow-pipeline-integration.test.ts
    │   │   │   │   │       ├── feature-branch-lifecycle-integration.test.ts
    │   │   │   │   │       ├── git-locale.test.ts
    │   │   │   │   │       ├── git-self-heal.test.ts
    │   │   │   │   │       ├── gitignore-staging-2570.test.ts
    │   │   │   │   │       ├── gitignore-tracked-gsd.test.ts
    │   │   │   │   │       ├── headless-command.ts
    │   │   │   │   │       ├── idle-recovery.test.ts
    │   │   │   │   │       ├── inherited-repo-home-dir.test.ts
    │   │   │   │   │       ├── integration-lifecycle.test.ts
    │   │   │   │   │       ├── integration-mixed-milestones.test.ts
    │   │   │   │   │       ├── integration-proof.test.ts
    │   │   │   │   │       ├── merge-cwd-restore.test.ts
    │   │   │   │   │       ├── migrate-command.test.ts
    │   │   │   │   │       ├── milestone-transition-worktree.test.ts
    │   │   │   │   │       ├── parallel-merge.test.ts
    │   │   │   │   │       ├── parallel-workers-multi-milestone-e2e.test.ts
    │   │   │   │   │       ├── paths.test.ts
    │   │   │   │   │       ├── plugin-importer-live.test.ts
    │   │   │   │   │       ├── queue-completed-milestone-perf.test.ts
    │   │   │   │   │       ├── queue-reorder-e2e.test.ts
    │   │   │   │   │       ├── quick-branch-lifecycle.test.ts
    │   │   │   │   │       ├── run-uat.test.ts
    │   │   │   │   │       ├── state-machine-live-validation.test.ts
    │   │   │   │   │       ├── state-machine-runtime-failures.test.ts
    │   │   │   │   │       ├── test-isolation.ts
    │   │   │   │   │       ├── token-savings.test.ts
    │   │   │   │   │       ├── workspace-collapse-integration.test.ts
    │   │   │   │   │       └── worktree-e2e.test.ts
    │   │   │   │   ├── tools/
    │   │   │   │   │   ├── complete-milestone.ts
    │   │   │   │   │   ├── complete-slice.ts
    │   │   │   │   │   ├── complete-task.ts
    │   │   │   │   │   ├── context-mode-tool-result.ts
    │   │   │   │   │   ├── exec-search-tool.ts
    │   │   │   │   │   ├── exec-tool.ts
    │   │   │   │   │   ├── memory-tools.ts
    │   │   │   │   │   ├── plan-milestone.ts
    │   │   │   │   │   ├── plan-slice.ts
    │   │   │   │   │   ├── plan-task.ts
    │   │   │   │   │   ├── reassess-roadmap.ts
    │   │   │   │   │   ├── reopen-milestone.ts
    │   │   │   │   │   ├── reopen-slice.ts
    │   │   │   │   │   ├── reopen-task.ts
    │   │   │   │   │   ├── replan-slice.ts
    │   │   │   │   │   ├── resume-tool.ts
    │   │   │   │   │   ├── skip-slice.ts
    │   │   │   │   │   ├── validate-milestone.ts
    │   │   │   │   │   └── workflow-tool-executors.ts
    │   │   │   │   ├── tui/
    │   │   │   │   │   └── render-kit.ts
    │   │   │   │   ├── uok/
    │   │   │   │   │   ├── audit-toggle.ts
    │   │   │   │   │   ├── audit.ts
    │   │   │   │   │   ├── contracts.ts
    │   │   │   │   │   ├── dispatch-envelope.ts
    │   │   │   │   │   ├── execution-graph.ts
    │   │   │   │   │   ├── flags.ts
    │   │   │   │   │   ├── gate-runner.ts
    │   │   │   │   │   ├── gitops.ts
    │   │   │   │   │   ├── kernel.ts
    │   │   │   │   │   ├── loop-adapter.ts
    │   │   │   │   │   ├── model-policy.ts
    │   │   │   │   │   ├── parity-report.ts
    │   │   │   │   │   ├── plan-v2.ts
    │   │   │   │   │   ├── timeline.ts
    │   │   │   │   │   └── writer.ts
    │   │   │   │   ├── watch/
    │   │   │   │   │   ├── header-renderer.ts
    │   │   │   │   │   └── splash-palette.ts
    │   │   │   │   └── workflow-templates/
    │   │   │   │       ├── accessibility-audit.md
    │   │   │   │       ├── api-breaking-change.md
    │   │   │   │       ├── bugfix.md
    │   │   │   │       ├── changelog-gen.md
    │   │   │   │       ├── ci-bootstrap.md
    │   │   │   │       ├── dead-code.md
    │   │   │   │       ├── dep-upgrade.md
    │   │   │   │       ├── docs-sync.yaml
    │   │   │   │       ├── env-audit.yaml
    │   │   │   │       ├── full-project.md
    │   │   │   │       ├── hotfix.md
    │   │   │   │       ├── issue-triage.md
    │   │   │   │       ├── observability-setup.md
    │   │   │   │       ├── onboarding-check.md
    │   │   │   │       ├── performance-audit.md
    │   │   │   │       ├── pr-review.md
    │   │   │   │       ├── pr-triage.md
    │   │   │   │       ├── refactor.md
    │   │   │   │       ├── registry.json
    │   │   │   │       ├── release.md
    │   │   │   │       ├── rename-symbol.yaml
    │   │   │   │       ├── security-audit.md
    │   │   │   │       ├── small-feature.md
    │   │   │   │       ├── spike.md
    │   │   │   │       └── test-backfill.yaml
    │   │   │   ├── mac-tools/
    │   │   │   │   ├── extension-manifest.json
    │   │   │   │   ├── index.ts
    │   │   │   │   └── swift-cli/
    │   │   │   │       ├── Package.swift
    │   │   │   │       └── Sources/
    │   │   │   │           └── main.swift
    │   │   │   ├── mcp-client/
    │   │   │   │   ├── auth.ts
    │   │   │   │   ├── index.ts
    │   │   │   │   └── tests/
    │   │   │   │       ├── global-config.test.ts
    │   │   │   │       └── server-name-spaces.test.ts
    │   │   │   ├── ollama/
    │   │   │   │   ├── index.ts
    │   │   │   │   ├── model-capabilities.ts
    │   │   │   │   ├── ndjson-stream.ts
    │   │   │   │   ├── ollama-auth-mode.test.ts
    │   │   │   │   ├── ollama-chat-provider.ts
    │   │   │   │   ├── ollama-client.ts
    │   │   │   │   ├── ollama-commands.ts
    │   │   │   │   ├── ollama-discovery.ts
    │   │   │   │   ├── ollama-status-indicator.test.ts
    │   │   │   │   ├── ollama-tool.ts
    │   │   │   │   ├── thinking-parser.ts
    │   │   │   │   ├── types.ts
    │   │   │   │   └── tests/
    │   │   │   │       ├── model-capabilities.test.ts
    │   │   │   │       ├── ollama-chat-provider-stream.test.ts
    │   │   │   │       ├── ollama-client-timeout-env.test.ts
    │   │   │   │       ├── ollama-client.test.ts
    │   │   │   │       └── ollama-discovery.test.ts
    │   │   │   ├── remote-questions/
    │   │   │   │   ├── commands.ts
    │   │   │   │   ├── config.ts
    │   │   │   │   ├── discord-adapter.ts
    │   │   │   │   ├── extension-manifest.json
    │   │   │   │   ├── format.ts
    │   │   │   │   ├── http-client.ts
    │   │   │   │   ├── manager.ts
    │   │   │   │   ├── mod.ts
    │   │   │   │   ├── notify.ts
    │   │   │   │   ├── remote-command.ts
    │   │   │   │   ├── slack-adapter.ts
    │   │   │   │   ├── status.ts
    │   │   │   │   ├── store.ts
    │   │   │   │   ├── telegram-adapter.ts
    │   │   │   │   ├── types.ts
    │   │   │   │   └── tests/
    │   │   │   │       ├── command-polling.test.ts
    │   │   │   │       ├── remote-answer-normalization.test.ts
    │   │   │   │       └── telegram-commands.test.ts
    │   │   │   ├── search-the-web/
    │   │   │   │   ├── cache.ts
    │   │   │   │   ├── command-search-provider.ts
    │   │   │   │   ├── extension-manifest.json
    │   │   │   │   ├── format.ts
    │   │   │   │   ├── http.ts
    │   │   │   │   ├── index.ts
    │   │   │   │   ├── native-search.ts
    │   │   │   │   ├── provider.ts
    │   │   │   │   ├── tavily.ts
    │   │   │   │   ├── tool-fetch-page.ts
    │   │   │   │   ├── tool-llm-context.ts
    │   │   │   │   ├── tool-search.ts
    │   │   │   │   └── url-utils.ts
    │   │   │   ├── shared/
    │   │   │   │   ├── cmux-events.ts
    │   │   │   │   ├── confirm-ui.ts
    │   │   │   │   ├── format-utils.ts
    │   │   │   │   ├── frontmatter.ts
    │   │   │   │   ├── gsd-phase-state.ts
    │   │   │   │   ├── html-shell.ts
    │   │   │   │   ├── interview-ui.ts
    │   │   │   │   ├── layout-utils.ts
    │   │   │   │   ├── mod.ts
    │   │   │   │   ├── next-action-ui.ts
    │   │   │   │   ├── path-display.ts
    │   │   │   │   ├── rtk-session-stats.ts
    │   │   │   │   ├── rtk-shared.ts
    │   │   │   │   ├── rtk.ts
    │   │   │   │   ├── sanitize.ts
    │   │   │   │   ├── terminal.ts
    │   │   │   │   ├── tui.ts
    │   │   │   │   ├── ui.ts
    │   │   │   │   └── tests/
    │   │   │   │       ├── ask-user-freetext.test.ts
    │   │   │   │       ├── format-utils.test.ts
    │   │   │   │       ├── gsd-phase-state.test.ts
    │   │   │   │       ├── interview-notes-loop.test.ts
    │   │   │   │       ├── interview-preview.test.ts
    │   │   │   │       ├── next-action-ui-hasui.test.ts
    │   │   │   │       └── preview-layout.test.ts
    │   │   │   ├── slash-commands/
    │   │   │   │   ├── audit.ts
    │   │   │   │   ├── clear.ts
    │   │   │   │   ├── create-extension.ts
    │   │   │   │   ├── create-slash-command.ts
    │   │   │   │   ├── extension-manifest.json
    │   │   │   │   └── index.ts
    │   │   │   ├── subagent/
    │   │   │   │   ├── agents.ts
    │   │   │   │   ├── extension-manifest.json
    │   │   │   │   ├── isolation.ts
    │   │   │   │   ├── launch.ts
    │   │   │   │   ├── run-store.ts
    │   │   │   │   ├── worker-registry.ts
    │   │   │   │   └── tests/
    │   │   │   │       ├── agents-conflicts.test.ts
    │   │   │   │       ├── launch.test.ts
    │   │   │   │       ├── model-override.test.ts
    │   │   │   │       └── run-store.test.ts
    │   │   │   ├── ttsr/
    │   │   │   │   ├── extension-manifest.json
    │   │   │   │   ├── index.ts
    │   │   │   │   ├── rule-loader.ts
    │   │   │   │   ├── ttsr-interrupt.md
    │   │   │   │   └── ttsr-manager.ts
    │   │   │   ├── universal-config/
    │   │   │   │   ├── discovery.ts
    │   │   │   │   ├── extension-manifest.json
    │   │   │   │   ├── format.ts
    │   │   │   │   ├── index.ts
    │   │   │   │   ├── package.json
    │   │   │   │   ├── scanners.ts
    │   │   │   │   ├── tools.ts
    │   │   │   │   ├── types.ts
    │   │   │   │   └── tests/
    │   │   │   │       ├── discovery.test.ts
    │   │   │   │       ├── format.test.ts
    │   │   │   │       └── scanners.test.ts
    │   │   │   ├── visual-brief/
    │   │   │   │   ├── artifact-policy.ts
    │   │   │   │   ├── extension-manifest.json
    │   │   │   │   ├── index.ts
    │   │   │   │   ├── page-contract.ts
    │   │   │   │   ├── prompts.ts
    │   │   │   │   └── tests/
    │   │   │   │       └── visual-brief.test.ts
    │   │   │   └── voice/
    │   │   │       ├── extension-manifest.json
    │   │   │       ├── index.ts
    │   │   │       ├── linux-ready.ts
    │   │   │       ├── speech-recognizer.py
    │   │   │       ├── speech-recognizer.swift
    │   │   │       └── tests/
    │   │   │           └── linux-ready.test.ts
    │   │   └── skills/
    │   │       ├── accessibility/
    │   │       │   ├── SKILL.md
    │   │       │   └── references/
    │   │       │       └── WCAG.md
    │   │       ├── agent-browser/
    │   │       │   ├── SKILL.md
    │   │       │   ├── references/
    │   │       │   │   ├── authentication.md
    │   │       │   │   ├── commands.md
    │   │       │   │   ├── profiling.md
    │   │       │   │   ├── proxy-support.md
    │   │       │   │   ├── session-management.md
    │   │       │   │   ├── snapshot-refs.md
    │   │       │   │   └── video-recording.md
    │   │       │   └── templates/
    │   │       │       ├── authenticated-session.sh
    │   │       │       ├── capture-workflow.sh
    │   │       │       └── form-automation.sh
    │   │       ├── api-design/
    │   │       │   └── SKILL.md
    │   │       ├── best-practices/
    │   │       │   └── SKILL.md
    │   │       ├── btw/
    │   │       │   └── SKILL.md
    │   │       ├── code-optimizer/
    │   │       │   ├── SKILL.md
    │   │       │   └── references/
    │   │       │       ├── algorithmic-complexity.md
    │   │       │       ├── build-compilation.md
    │   │       │       ├── bundle-dependencies.md
    │   │       │       ├── caching-memoization.md
    │   │       │       ├── concurrency-async.md
    │   │       │       ├── config-infra.md
    │   │       │       ├── data-structures.md
    │   │       │       ├── database-queries.md
    │   │       │       ├── dead-code-redundancy.md
    │   │       │       ├── error-resilience.md
    │   │       │       ├── io-network.md
    │   │       │       ├── logging-observability.md
    │   │       │       ├── memory-resources.md
    │   │       │       ├── rendering-ui.md
    │   │       │       └── security-performance.md
    │   │       ├── core-web-vitals/
    │   │       │   ├── SKILL.md
    │   │       │   └── references/
    │   │       │       └── LCP.md
    │   │       ├── create-gsd-extension/
    │   │       │   ├── SKILL.md
    │   │       │   ├── references/
    │   │       │   │   ├── compaction-session-control.md
    │   │       │   │   ├── custom-commands.md
    │   │       │   │   ├── custom-rendering.md
    │   │       │   │   ├── custom-tools.md
    │   │       │   │   ├── custom-ui.md
    │   │       │   │   ├── events-reference.md
    │   │       │   │   ├── extension-lifecycle.md
    │   │       │   │   ├── extensionapi-reference.md
    │   │       │   │   ├── extensioncontext-reference.md
    │   │       │   │   ├── key-rules-gotchas.md
    │   │       │   │   ├── mode-behavior.md
    │   │       │   │   ├── model-provider-management.md
    │   │       │   │   ├── packaging-distribution.md
    │   │       │   │   ├── remote-execution-overrides.md
    │   │       │   │   ├── state-management.md
    │   │       │   │   └── system-prompt-modification.md
    │   │       │   ├── templates/
    │   │       │   │   ├── extension-skeleton.ts
    │   │       │   │   ├── stateful-tool-skeleton.ts
    │   │       │   │   └── templates.test.ts
    │   │       │   └── workflows/
    │   │       │       ├── add-capability.md
    │   │       │       ├── create-extension.md
    │   │       │       └── debug-extension.md
    │   │       ├── create-mcp-server/
    │   │       │   └── SKILL.md
    │   │       ├── create-skill/
    │   │       │   ├── SKILL.md
    │   │       │   ├── references/
    │   │       │   │   ├── api-security.md
    │   │       │   │   ├── be-clear-and-direct.md
    │   │       │   │   ├── common-patterns.md
    │   │       │   │   ├── core-principles.md
    │   │       │   │   ├── executable-code.md
    │   │       │   │   ├── gsd-skill-ecosystem.md
    │   │       │   │   ├── iteration-and-testing.md
    │   │       │   │   ├── recommended-structure.md
    │   │       │   │   ├── skill-structure.md
    │   │       │   │   ├── use-xml-tags.md
    │   │       │   │   ├── using-scripts.md
    │   │       │   │   ├── using-templates.md
    │   │       │   │   └── workflows-and-validation.md
    │   │       │   ├── templates/
    │   │       │   │   ├── router-skill.md
    │   │       │   │   └── simple-skill.md
    │   │       │   └── workflows/
    │   │       │       ├── add-reference.md
    │   │       │       ├── add-script.md
    │   │       │       ├── add-template.md
    │   │       │       ├── add-workflow.md
    │   │       │       ├── audit-skill.md
    │   │       │       ├── create-new-skill.md
    │   │       │       ├── get-guidance.md
    │   │       │       ├── upgrade-to-router.md
    │   │       │       └── verify-skill.md
    │   │       ├── create-workflow/
    │   │       │   ├── SKILL.md
    │   │       │   ├── references/
    │   │       │   │   ├── feature-patterns.md
    │   │       │   │   ├── verification-policies.md
    │   │       │   │   └── yaml-schema-v1.md
    │   │       │   ├── templates/
    │   │       │   │   ├── blog-post-pipeline.yaml
    │   │       │   │   ├── code-audit.yaml
    │   │       │   │   ├── release-checklist.yaml
    │   │       │   │   └── workflow-definition.yaml
    │   │       │   └── workflows/
    │   │       │       ├── create-from-scratch.md
    │   │       │       └── create-from-template.md
    │   │       ├── debug-like-expert/
    │   │       │   ├── SKILL.md
    │   │       │   └── references/
    │   │       │       ├── debugging-mindset.md
    │   │       │       ├── hypothesis-testing.md
    │   │       │       ├── investigation-techniques.md
    │   │       │       ├── verification-patterns.md
    │   │       │       └── when-to-research.md
    │   │       ├── decompose-into-slices/
    │   │       │   └── SKILL.md
    │   │       ├── dependency-upgrade/
    │   │       │   └── SKILL.md
    │   │       ├── design-an-interface/
    │   │       │   └── SKILL.md
    │   │       ├── forensics/
    │   │       │   └── SKILL.md
    │   │       ├── frontend-design/
    │   │       │   └── SKILL.md
    │   │       ├── github-workflows/
    │   │       │   ├── SKILL.md
    │   │       │   └── references/
    │   │       │       └── gh/
    │   │       │           ├── SKILL.md
    │   │       │           ├── references/
    │   │       │           │   ├── issue-stories.md
    │   │       │           │   ├── labels.md
    │   │       │           │   ├── milestones.md
    │   │       │           │   └── projects-v2.md
    │   │       │           ├── scripts/
    │   │       │           │   ├── experiment_cleanup.py
    │   │       │           │   └── github_project_setup.py
    │   │       │           └── tests/
    │   │       │               ├── __init__.py
    │   │       │               └── test_github_project_setup.py
    │   │       ├── grill-me/
    │   │       │   └── SKILL.md
    │   │       ├── handoff/
    │   │       │   └── SKILL.md
    │   │       ├── lint/
    │   │       │   └── SKILL.md
    │   │       ├── make-interfaces-feel-better/
    │   │       │   ├── animations.md
    │   │       │   ├── performance.md
    │   │       │   ├── SKILL.md
    │   │       │   ├── surfaces.md
    │   │       │   └── typography.md
    │   │       ├── observability/
    │   │       │   └── SKILL.md
    │   │       ├── react-best-practices/
    │   │       │   ├── README.md
    │   │       │   ├── metadata.json
    │   │       │   ├── SKILL.md
    │   │       │   └── rules/
    │   │       │       ├── _sections.md
    │   │       │       ├── _template.md
    │   │       │       ├── advanced-event-handler-refs.md
    │   │       │       ├── advanced-init-once.md
    │   │       │       ├── advanced-use-latest.md
    │   │       │       ├── async-api-routes.md
    │   │       │       ├── async-defer-await.md
    │   │       │       ├── async-dependencies.md
    │   │       │       ├── async-parallel.md
    │   │       │       ├── async-suspense-boundaries.md
    │   │       │       ├── bundle-barrel-imports.md
    │   │       │       ├── bundle-conditional.md
    │   │       │       ├── bundle-defer-third-party.md
    │   │       │       ├── bundle-dynamic-imports.md
    │   │       │       ├── bundle-preload.md
    │   │       │       ├── client-event-listeners.md
    │   │       │       ├── client-localstorage-schema.md
    │   │       │       ├── client-passive-event-listeners.md
    │   │       │       ├── client-swr-dedup.md
    │   │       │       ├── js-batch-dom-css.md
    │   │       │       ├── js-cache-function-results.md
    │   │       │       ├── js-cache-property-access.md
    │   │       │       ├── js-cache-storage.md
    │   │       │       ├── js-combine-iterations.md
    │   │       │       ├── js-early-exit.md
    │   │       │       ├── js-hoist-regexp.md
    │   │       │       ├── js-index-maps.md
    │   │       │       ├── js-length-check-first.md
    │   │       │       ├── js-min-max-loop.md
    │   │       │       ├── js-set-map-lookups.md
    │   │       │       ├── js-tosorted-immutable.md
    │   │       │       ├── rendering-activity.md
    │   │       │       ├── rendering-animate-svg-wrapper.md
    │   │       │       ├── rendering-conditional-render.md
    │   │       │       ├── rendering-content-visibility.md
    │   │       │       ├── rendering-hoist-jsx.md
    │   │       │       ├── rendering-hydration-no-flicker.md
    │   │       │       ├── rendering-hydration-suppress-warning.md
    │   │       │       ├── rendering-svg-precision.md
    │   │       │       ├── rendering-usetransition-loading.md
    │   │       │       ├── rerender-defer-reads.md
    │   │       │       ├── rerender-dependencies.md
    │   │       │       ├── rerender-derived-state-no-effect.md
    │   │       │       ├── rerender-derived-state.md
    │   │       │       ├── rerender-functional-setstate.md
    │   │       │       ├── rerender-lazy-state-init.md
    │   │       │       ├── rerender-memo-with-default-value.md
    │   │       │       ├── rerender-memo.md
    │   │       │       ├── rerender-move-effect-to-event.md
    │   │       │       ├── rerender-simple-expression-in-memo.md
    │   │       │       ├── rerender-transitions.md
    │   │       │       ├── rerender-use-ref-transient-values.md
    │   │       │       ├── server-after-nonblocking.md
    │   │       │       ├── server-auth-actions.md
    │   │       │       ├── server-cache-lru.md
    │   │       │       ├── server-cache-react.md
    │   │       │       ├── server-dedup-props.md
    │   │       │       ├── server-parallel-fetching.md
    │   │       │       └── server-serialization.md
    │   │       ├── review/
    │   │       │   └── SKILL.md
    │   │       ├── security-review/
    │   │       │   └── SKILL.md
    │   │       ├── spike-wrap-up/
    │   │       │   └── SKILL.md
    │   │       ├── tdd/
    │   │       │   └── SKILL.md
    │   │       ├── test/
    │   │       │   └── SKILL.md
    │   │       ├── userinterface-wiki/
    │   │       │   ├── SKILL.md
    │   │       │   └── rules/
    │   │       │       ├── _sections.md
    │   │       │       ├── _template.md
    │   │       │       ├── a11y-reduced-motion-check.md
    │   │       │       ├── a11y-toggle-setting.md
    │   │       │       ├── a11y-visual-equivalent.md
    │   │       │       ├── a11y-volume-control.md
    │   │       │       ├── appropriate-confirmations-only.md
    │   │       │       ├── appropriate-errors-warnings.md
    │   │       │       ├── appropriate-no-decorative.md
    │   │       │       ├── appropriate-no-high-frequency.md
    │   │       │       ├── appropriate-no-punishing.md
    │   │       │       ├── container-callback-ref.md
    │   │       │       ├── container-guard-initial-zero.md
    │   │       │       ├── container-no-excessive-use.md
    │   │       │       ├── container-overflow-hidden.md
    │   │       │       ├── container-transition-delay.md
    │   │       │       ├── container-two-div-pattern.md
    │   │       │       ├── container-use-resize-observer.md
    │   │       │       ├── context-cleanup-nodes.md
    │   │       │       ├── context-resume-suspended.md
    │   │       │       ├── context-reuse-single.md
    │   │       │       ├── design-filter-for-character.md
    │   │       │       ├── design-noise-for-percussion.md
    │   │       │       ├── design-oscillator-for-tonal.md
    │   │       │       ├── duration-max-300ms.md
    │   │       │       ├── duration-press-hover.md
    │   │       │       ├── duration-shorten-before-curve.md
    │   │       │       ├── duration-small-state.md
    │   │       │       ├── easing-entrance-ease-out.md
    │   │       │       ├── easing-exit-ease-in.md
    │   │       │       ├── easing-for-state-change.md
    │   │       │       ├── easing-linear-only-progress.md
    │   │       │       ├── easing-natural-decay.md
    │   │       │       ├── easing-no-linear-motion.md
    │   │       │       ├── easing-transition-ease-in-out.md
    │   │       │       ├── envelope-exponential-decay.md
    │   │       │       ├── envelope-no-zero-target.md
    │   │       │       ├── envelope-set-initial-value.md
    │   │       │       ├── exit-key-required.md
    │   │       │       ├── exit-matches-initial.md
    │   │       │       ├── exit-prop-required.md
    │   │       │       ├── exit-requires-wrapper.md
    │   │       │       ├── impl-default-subtle.md
    │   │       │       ├── impl-preload-audio.md
    │   │       │       ├── impl-reset-current-time.md
    │   │       │       ├── mode-pop-layout-for-lists.md
    │   │       │       ├── mode-sync-layout-conflict.md
    │   │       │       ├── mode-wait-doubles-duration.md
    │   │       │       ├── morphing-aria-hidden.md
    │   │       │       ├── morphing-consistent-viewbox.md
    │   │       │       ├── morphing-group-variants.md
    │   │       │       ├── morphing-jump-non-grouped.md
    │   │       │       ├── morphing-reduced-motion.md
    │   │       │       ├── morphing-spring-rotation.md
    │   │       │       ├── morphing-strokelinecap-round.md
    │   │       │       ├── morphing-three-lines.md
    │   │       │       ├── morphing-use-collapsed.md
    │   │       │       ├── native-backdrop-styling.md
    │   │       │       ├── native-placeholder-styling.md
    │   │       │       ├── native-selection-styling.md
    │   │       │       ├── nested-consistent-timing.md
    │   │       │       ├── nested-propagate-required.md
    │   │       │       ├── none-context-menu-entrance.md
    │   │       │       ├── none-high-frequency.md
    │   │       │       ├── none-keyboard-navigation.md
    │   │       │       ├── param-click-duration.md
    │   │       │       ├── param-filter-frequency-range.md
    │   │       │       ├── param-q-value-range.md
    │   │       │       ├── param-reasonable-gain.md
    │   │       │       ├── physics-active-state.md
    │   │       │       ├── physics-no-excessive-stagger.md
    │   │       │       ├── physics-spring-for-overshoot.md
    │   │       │       ├── physics-subtle-deformation.md
    │   │       │       ├── prefetch-hit-slop.md
    │   │       │       ├── prefetch-keyboard-tab.md
    │   │       │       ├── prefetch-not-everything.md
    │   │       │       ├── prefetch-touch-fallback.md
    │   │       │       ├── prefetch-trajectory-over-hover.md
    │   │       │       ├── prefetch-use-selectively.md
    │   │       │       ├── presence-disable-interactions.md
    │   │       │       ├── presence-hook-in-child.md
    │   │       │       ├── presence-safe-to-remove.md
    │   │       │       ├── pseudo-content-required.md
    │   │       │       ├── pseudo-first-line-styling.md
    │   │       │       ├── pseudo-hit-target-expansion.md
    │   │       │       ├── pseudo-marker-styling.md
    │   │       │       ├── pseudo-over-dom-node.md
    │   │       │       ├── pseudo-position-relative-parent.md
    │   │       │       ├── pseudo-z-index-layering.md
    │   │       │       ├── spring-for-gestures.md
    │   │       │       ├── spring-for-interruptible.md
    │   │       │       ├── spring-params-balanced.md
    │   │       │       ├── spring-preserves-velocity.md
    │   │       │       ├── staging-dim-background.md
    │   │       │       ├── staging-one-focal-point.md
    │   │       │       ├── staging-z-index-hierarchy.md
    │   │       │       ├── timing-consistent.md
    │   │       │       ├── timing-no-entrance-context-menu.md
    │   │       │       ├── timing-under-300ms.md
    │   │       │       ├── transition-name-cleanup.md
    │   │       │       ├── transition-name-required.md
    │   │       │       ├── transition-name-unique.md
    │   │       │       ├── transition-over-js-library.md
    │   │       │       ├── transition-style-pseudo-elements.md
    │   │       │       ├── type-antialiased-on-retina.md
    │   │       │       ├── type-disambiguation-stylistic-set.md
    │   │       │       ├── type-font-display-swap.md
    │   │       │       ├── type-justify-with-hyphens.md
    │   │       │       ├── type-letter-spacing-uppercase.md
    │   │       │       ├── type-no-font-synthesis.md
    │   │       │       ├── type-oldstyle-nums-for-prose.md
    │   │       │       ├── type-opentype-contextual-alternates.md
    │   │       │       ├── type-optical-sizing-auto.md
    │   │       │       ├── type-proper-fractions.md
    │   │       │       ├── type-slashed-zero.md
    │   │       │       ├── type-tabular-nums-for-data.md
    │   │       │       ├── type-text-wrap-balance-headings.md
    │   │       │       ├── type-text-wrap-pretty.md
    │   │       │       ├── type-underline-offset.md
    │   │       │       ├── type-variable-weight-continuous.md
    │   │       │       ├── ux-aesthetic-usability.md
    │   │       │       ├── ux-cognitive-load-reduce.md
    │   │       │       ├── ux-common-region-boundaries.md
    │   │       │       ├── ux-doherty-perceived-speed.md
    │   │       │       ├── ux-doherty-under-400ms.md
    │   │       │       ├── ux-fitts-hit-area.md
    │   │       │       ├── ux-fitts-target-size.md
    │   │       │       ├── ux-goal-gradient-progress.md
    │   │       │       ├── ux-hicks-minimize-choices.md
    │   │       │       ├── ux-jakobs-familiar-patterns.md
    │   │       │       ├── ux-millers-chunking.md
    │   │       │       ├── ux-pareto-prioritize-features.md
    │   │       │       ├── ux-peak-end-finish-strong.md
    │   │       │       ├── ux-postels-accept-messy-input.md
    │   │       │       ├── ux-pragnanz-simplify.md
    │   │       │       ├── ux-progressive-disclosure.md
    │   │       │       ├── ux-proximity-grouping.md
    │   │       │       ├── ux-serial-position.md
    │   │       │       ├── ux-similarity-consistency.md
    │   │       │       ├── ux-teslers-complexity.md
    │   │       │       ├── ux-uniform-connectedness.md
    │   │       │       ├── ux-von-restorff-emphasis.md
    │   │       │       ├── ux-zeigarnik-show-incomplete.md
    │   │       │       ├── visual-animate-shadow-pseudo.md
    │   │       │       ├── visual-border-alpha-colors.md
    │   │       │       ├── visual-button-shadow-anatomy.md
    │   │       │       ├── visual-concentric-radius.md
    │   │       │       ├── visual-consistent-spacing-scale.md
    │   │       │       ├── visual-layered-shadows.md
    │   │       │       ├── visual-no-pure-black-shadow.md
    │   │       │       ├── visual-shadow-direction.md
    │   │       │       ├── visual-shadow-matches-elevation.md
    │   │       │       ├── weight-duration-matches-action.md
    │   │       │       └── weight-match-action.md
    │   │       ├── verify-before-complete/
    │   │       │   └── SKILL.md
    │   │       ├── web-design-guidelines/
    │   │       │   └── SKILL.md
    │   │       ├── web-quality-audit/
    │   │       │   ├── SKILL.md
    │   │       │   └── scripts/
    │   │       │       └── analyze.sh
    │   │       ├── write-docs/
    │   │       │   └── SKILL.md
    │   │       └── write-milestone-brief/
    │   │           └── SKILL.md
    │   ├── shared/
    │   │   └── workspace-types.ts
    │   ├── tests/
    │   │   ├── app-smoke.test.ts
    │   │   ├── artifact-manager.test.ts
    │   │   ├── assistant-message-thinking-visibility.test.ts
    │   │   ├── auto-budget.test.ts
    │   │   ├── auto-mode-piped.test.ts
    │   │   ├── auto-piped-io.test.ts
    │   │   ├── auto-resume-resource-loader.test.ts
    │   │   ├── auto-tool-tracking.test.ts
    │   │   ├── bg-shell-persistence-cwd.test.ts
    │   │   ├── bg-shell-session-cleanup.test.ts
    │   │   ├── blob-store.test.ts
    │   │   ├── bridge-package-root.test.ts
    │   │   ├── build-script-contract.test.ts
    │   │   ├── bundled-extension-paths.test.ts
    │   │   ├── bundled-resource-path.test.ts
    │   │   ├── claude-cli-dep0190-regression.test.ts
    │   │   ├── claude-cli-windows-detection.test.ts
    │   │   ├── claude-exe-windows-detection.test.ts
    │   │   ├── claude-readiness-regression-4997.test.ts
    │   │   ├── cli-model-override-startup.test.ts
    │   │   ├── cli-onboarding-custom-provider.test.ts
    │   │   ├── commands-worktree.test.ts
    │   │   ├── contracts-rpc-fixtures.test.ts
    │   │   ├── create-gsd-extension-paths.test.ts
    │   │   ├── cross-platform-filesystem-safety.test.ts
    │   │   ├── ecosystem-extensions.test.ts
    │   │   ├── ensure-workspace-builds.test.ts
    │   │   ├── extension-discovery.test.ts
    │   │   ├── extension-load-perf.test.ts
    │   │   ├── extension-model-validation.test.ts
    │   │   ├── extension-smoke.test.ts
    │   │   ├── extension-sort.test.ts
    │   │   ├── extension-validator.test.ts
    │   │   ├── fetch-test-helpers.ts
    │   │   ├── footer-component.test.ts
    │   │   ├── google-search-auth.repro.test.ts
    │   │   ├── google-search-oauth-shape.test.ts
    │   │   ├── gsd-web-launcher-contract.test.ts
    │   │   ├── headless-cli-surface.test.ts
    │   │   ├── headless-detection.test.ts
    │   │   ├── headless-doctor.test.ts
    │   │   ├── headless-events.test.ts
    │   │   ├── headless-multi-turn.test.ts
    │   │   ├── headless-progress.test.ts
    │   │   ├── headless-query-db-open.test.ts
    │   │   ├── headless-query-extension-path.test.ts
    │   │   ├── headless-recover.test.ts
    │   │   ├── headless-v2-migration.test.ts
    │   │   ├── initial-gsd-header-filter.test.ts
    │   │   ├── legacy-cleanup-evidence.test.ts
    │   │   ├── legacy-cleanup-gate.test.ts
    │   │   ├── llm-context-tavily.test.ts
    │   │   ├── marketplace-discovery.test.ts
    │   │   ├── mcp-client-oauth.test.ts
    │   │   ├── mcp-client-schema.test.ts
    │   │   ├── mcp-createRequire.test.ts
    │   │   ├── mcp-server.test.ts
    │   │   ├── model-registry-custom-provider.test.ts
    │   │   ├── native-search.test.ts
    │   │   ├── node-modules-symlink.test.ts
    │   │   ├── non-extension-library.test.ts
    │   │   ├── offline-mode.test.ts
    │   │   ├── onboarding-claude-cli-provider.test.ts
    │   │   ├── onboarding-default-provider-persistence.test.ts
    │   │   ├── onboarding-minimax-first-class.test.ts
    │   │   ├── onboarding-reentry-no-double-header.test.ts
    │   │   ├── package-mcp-server-elicitation.test.ts
    │   │   ├── parse-cli-args.test.ts
    │   │   ├── pi-ai-event-stream-factory.test.ts
    │   │   ├── pi-migration-exports.test.ts
    │   │   ├── postinstall.test.ts
    │   │   ├── prompt-golden-fixtures.test.ts
    │   │   ├── provider-equality-allowlist.test.ts
    │   │   ├── provider-help-text.test.ts
    │   │   ├── provider-manager-enter-key.test.ts
    │   │   ├── provider-manager-externalcli-routing.test.ts
    │   │   ├── provider-manager-remove.test.ts
    │   │   ├── provider-migrations.test.ts
    │   │   ├── provider.test.ts
    │   │   ├── pty-chat-parser.test.ts
    │   │   ├── read-tool-offset-clamp.test.ts
    │   │   ├── refactor-baseline.test.ts
    │   │   ├── resolve-ts-loader.test.ts
    │   │   ├── resource-loader-conflicts.test.ts
    │   │   ├── resource-loader-content-hash.test.ts
    │   │   ├── resource-loader.test.ts
    │   │   ├── resource-sync-staleness.test.ts
    │   │   ├── rpc-golden-fixtures.test.ts
    │   │   ├── rtk-execution-seams.test.ts
    │   │   ├── rtk-session-stats.test.ts
    │   │   ├── rtk-test-utils.ts
    │   │   ├── rtk.test.ts
    │   │   ├── search-loop-guard.test.ts
    │   │   ├── search-provider-command.test.ts
    │   │   ├── search-tavily.test.ts
    │   │   ├── secret-scan.test.ts
    │   │   ├── security-overrides.test.ts
    │   │   ├── startup-model-validation.test.ts
    │   │   ├── startup-perf.test.ts
    │   │   ├── tavily-helpers.test.ts
    │   │   ├── terminal-cmux.test.ts
    │   │   ├── token-counter.test.ts
    │   │   ├── tool-bootstrap.test.ts
    │   │   ├── ttsr-manager.test.ts
    │   │   ├── ttsr-rule-loader.test.ts
    │   │   ├── tui-autocomplete-ghost-lines.test.ts
    │   │   ├── tui-classic-theme.test.ts
    │   │   ├── tui-content-cursor-desync.test.ts
    │   │   ├── tui-non-tty-render-loop.test.ts
    │   │   ├── tui-pin-to-bottom-on-clear.test.ts
    │   │   ├── update-check.test.ts
    │   │   ├── update-cmd-diagnostics.test.ts
    │   │   ├── url-utils.test.ts
    │   │   ├── vscode-startup-security.test.ts
    │   │   ├── web-files-symlink.test.ts
    │   │   ├── web-subprocess-runner.test.ts
    │   │   ├── welcome-screen.test.ts
    │   │   ├── windows-portability.test.ts
    │   │   ├── workspace-manifest.test.ts
    │   │   ├── worktree-cli-root.test.ts
    │   │   ├── xterm-theme.test.ts
    │   │   ├── fixtures/
    │   │   │   ├── prompt-golden-fixtures.ts
    │   │   │   └── rpc-golden-fixtures.ts
    │   │   └── integration/
    │   │       ├── ci_monitor.test.ts
    │   │       ├── e2e-headless.test.ts
    │   │       ├── e2e-smoke.test.ts
    │   │       ├── pack-install.test.ts
    │   │       ├── web-auth-token.test.ts
    │   │       ├── web-auto-dashboard-lock-reconciliation.test.ts
    │   │       ├── web-boot-node24.test.ts
    │   │       ├── web-bridge-contract.test.ts
    │   │       ├── web-bridge-package-root.test.ts
    │   │       ├── web-bridge-terminal-contract.test.ts
    │   │       ├── web-cli-entry.test.ts
    │   │       ├── web-command-parity-contract.test.ts
    │   │       ├── web-dashboard-rtk-contract.test.ts
    │   │       ├── web-diagnostics-contract.test.ts
    │   │       ├── web-live-interaction-contract.test.ts
    │   │       ├── web-live-state-contract.test.ts
    │   │       ├── web-mode-assembled.test.ts
    │   │       ├── web-mode-cli.test.ts
    │   │       ├── web-mode-network-flags.test.ts
    │   │       ├── web-mode-onboarding.test.ts
    │   │       ├── web-mode-runtime-fixtures.ts
    │   │       ├── web-mode-runtime-harness.ts
    │   │       ├── web-mode-windows-hide.test.ts
    │   │       ├── web-multi-project-contract.test.ts
    │   │       ├── web-onboarding-contract.test.ts
    │   │       ├── web-onboarding-presentation.test.ts
    │   │       ├── web-project-discovery-contract.test.ts
    │   │       ├── web-project-tab-preservation.test.ts
    │   │       ├── web-project-url.test.ts
    │   │       ├── web-recovery-diagnostics-contract.test.ts
    │   │       ├── web-session-parity-contract.test.ts
    │   │       ├── web-state-surfaces-contract.test.ts
    │   │       ├── web-subprocess-module-resolution.test.ts
    │   │       ├── web-switch-project.test.ts
    │   │       ├── web-terminal-allowlist.test.ts
    │   │       ├── web-terminal-preservation.test.ts
    │   │       ├── web-workflow-action-execution.test.ts
    │   │       └── web-workflow-controls-contract.test.ts
    │   └── web/
    │       ├── auto-dashboard-service.ts
    │       ├── auto-dashboard-types.ts
    │       ├── captures-service.ts
    │       ├── cleanup-service.ts
    │       ├── cli-entry.ts
    │       ├── doctor-service.ts
    │       ├── export-service.ts
    │       ├── forensics-service.ts
    │       ├── git-summary-service.ts
    │       ├── history-service.ts
    │       ├── hooks-service.ts
    │       ├── inspect-service.ts
    │       ├── knowledge-service.ts
    │       ├── notifications-service.ts
    │       ├── onboarding-service.ts
    │       ├── project-discovery-service.ts
    │       ├── recovery-diagnostics-service.ts
    │       ├── safe-import-meta-resolve.ts
    │       ├── settings-service.ts
    │       ├── skill-health-service.ts
    │       ├── subprocess-runner.ts
    │       ├── ts-subprocess-flags.ts
    │       ├── undo-service.ts
    │       ├── update-service.ts
    │       ├── visualizer-service.ts
    │       └── web-auth-storage.ts
    ├── studio/
    │   ├── electron.vite.config.ts
    │   ├── package.json
    │   ├── tsconfig.json
    │   ├── tsconfig.node.json
    │   ├── tsconfig.web.json
    │   ├── src/
    │   │   ├── main/
    │   │   │   └── index.ts
    │   │   ├── preload/
    │   │   │   ├── index.d.ts
    │   │   │   └── index.ts
    │   │   └── renderer/
    │   │       ├── index.html
    │   │       └── src/
    │   │           ├── App.tsx
    │   │           ├── main.tsx
    │   │           ├── lib/
    │   │           │   └── theme/
    │   │           │       └── tokens.ts
    │   │           └── styles/
    │   │               └── index.css
    │   └── test/
    │       ├── tokens.test.mjs
    │       └── e2e/
    │           └── electron-launch.e2e.test.mjs
    ├── tests/
    │   ├── e2e/
    │   │   ├── README.md
    │   │   ├── agent-loop-multiturn.e2e.test.ts
    │   │   ├── agent-loop.e2e.test.ts
    │   │   ├── mcp-server.e2e.test.ts
    │   │   ├── migration.e2e.test.ts
    │   │   ├── native-abi.e2e.test.ts
    │   │   ├── sanity.e2e.test.ts
    │   │   ├── _shared/
    │   │   │   ├── artifacts.ts
    │   │   │   ├── fake-llm.ts
    │   │   │   ├── index.ts
    │   │   │   ├── spawn.ts
    │   │   │   └── tmp-project.ts
    │   │   └── docker/
    │   │       └── runtime.e2e.test.ts
    │   ├── live/
    │   │   ├── run.ts
    │   │   ├── test-anthropic-roundtrip.ts
    │   │   └── test-openai-roundtrip.ts
    │   ├── live-regression/
    │   │   └── run.ts
    │   └── smoke/
    │       ├── run.ts
    │       ├── test-help.ts
    │       └── test-version.ts
    ├── vscode-extension/
    │   ├── README.md
    │   ├── CHANGELOG.md
    │   ├── LICENSE
    │   ├── package.json
    │   ├── tsconfig.json
    │   ├── .vscodeignore
    │   ├── src/
    │   │   ├── activity-feed.ts
    │   │   ├── approval-mode.ts
    │   │   ├── bash-terminal.ts
    │   │   ├── change-tracker-core.ts
    │   │   ├── change-tracker.ts
    │   │   ├── chat-participant.ts
    │   │   ├── checkpoints.ts
    │   │   ├── code-lens.ts
    │   │   ├── conversation-history.ts
    │   │   ├── diagnostics.ts
    │   │   ├── extension.ts
    │   │   ├── file-decorations.ts
    │   │   ├── git-args.ts
    │   │   ├── git-integration.ts
    │   │   ├── gsd-client-spawn.ts
    │   │   ├── gsd-client.ts
    │   │   ├── line-decorations.ts
    │   │   ├── permissions.ts
    │   │   ├── plan-viewer.ts
    │   │   ├── rpc-display.ts
    │   │   ├── scm-provider.ts
    │   │   ├── session-tree.ts
    │   │   ├── sidebar.ts
    │   │   ├── slash-completion.ts
    │   │   └── trusted-config.ts
    │   └── test/
    │       ├── change-tracker-source.test.ts
    │       ├── manifest.test.ts
    │       └── rpc-display-contract.test.ts
    ├── web/
    │   ├── components.json
    │   ├── eslint.config.mjs
    │   ├── next-env.d.ts
    │   ├── next.config.mjs
    │   ├── package.json
    │   ├── postcss.config.mjs
    │   ├── proxy.ts
    │   ├── tsconfig.json
    │   ├── app/
    │   │   ├── globals.css
    │   │   ├── layout.tsx
    │   │   ├── page.tsx
    │   │   └── api/
    │   │       ├── boot/
    │   │       │   └── route.ts
    │   │       ├── bridge-terminal/
    │   │       │   ├── input/
    │   │       │   │   └── route.ts
    │   │       │   ├── resize/
    │   │       │   │   └── route.ts
    │   │       │   └── stream/
    │   │       │       └── route.ts
    │   │       ├── browse-directories/
    │   │       │   └── route.ts
    │   │       ├── captures/
    │   │       │   └── route.ts
    │   │       ├── cleanup/
    │   │       │   └── route.ts
    │   │       ├── dev-mode/
    │   │       │   └── route.ts
    │   │       ├── doctor/
    │   │       │   └── route.ts
    │   │       ├── experimental/
    │   │       │   └── route.ts
    │   │       ├── export-data/
    │   │       │   └── route.ts
    │   │       ├── files/
    │   │       │   └── route.ts
    │   │       ├── forensics/
    │   │       │   └── route.ts
    │   │       ├── git/
    │   │       │   └── route.ts
    │   │       ├── history/
    │   │       │   └── route.ts
    │   │       ├── hooks/
    │   │       │   └── route.ts
    │   │       ├── inspect/
    │   │       │   └── route.ts
    │   │       ├── knowledge/
    │   │       │   └── route.ts
    │   │       ├── live-state/
    │   │       │   └── route.ts
    │   │       ├── notifications/
    │   │       │   └── route.ts
    │   │       ├── onboarding/
    │   │       │   └── route.ts
    │   │       ├── preferences/
    │   │       │   └── route.ts
    │   │       ├── projects/
    │   │       │   └── route.ts
    │   │       ├── recovery/
    │   │       │   └── route.ts
    │   │       ├── remote-questions/
    │   │       │   └── route.ts
    │   │       ├── session/
    │   │       │   ├── browser/
    │   │       │   │   └── route.ts
    │   │       │   ├── command/
    │   │       │   │   └── route.ts
    │   │       │   ├── events/
    │   │       │   │   └── route.ts
    │   │       │   └── manage/
    │   │       │       └── route.ts
    │   │       ├── settings-data/
    │   │       │   └── route.ts
    │   │       ├── shutdown/
    │   │       │   └── route.ts
    │   │       ├── skill-health/
    │   │       │   └── route.ts
    │   │       ├── steer/
    │   │       │   └── route.ts
    │   │       ├── switch-root/
    │   │       │   └── route.ts
    │   │       ├── terminal/
    │   │       │   ├── input/
    │   │       │   │   └── route.ts
    │   │       │   ├── resize/
    │   │       │   │   └── route.ts
    │   │       │   ├── sessions/
    │   │       │   │   └── route.ts
    │   │       │   ├── stream/
    │   │       │   │   └── route.ts
    │   │       │   └── upload/
    │   │       │       └── route.ts
    │   │       ├── undo/
    │   │       │   └── route.ts
    │   │       ├── update/
    │   │       │   └── route.ts
    │   │       └── visualizer/
    │   │           └── route.ts
    │   ├── components/
    │   │   ├── theme-provider.tsx
    │   │   ├── gsd/
    │   │   │   ├── activity-view.tsx
    │   │   │   ├── app-shell.tsx
    │   │   │   ├── code-editor.tsx
    │   │   │   ├── dashboard.tsx
    │   │   │   ├── diagnostics-panels.tsx
    │   │   │   ├── dual-terminal.tsx
    │   │   │   ├── file-content-viewer.tsx
    │   │   │   ├── files-view.tsx
    │   │   │   ├── focused-panel.tsx
    │   │   │   ├── guided-dialog.tsx
    │   │   │   ├── knowledge-captures-panel.tsx
    │   │   │   ├── loading-skeletons.tsx
    │   │   │   ├── main-session-terminal.tsx
    │   │   │   ├── onboarding-gate.tsx
    │   │   │   ├── project-welcome.tsx
    │   │   │   ├── projects-view.tsx
    │   │   │   ├── roadmap.tsx
    │   │   │   ├── scope-badge.tsx
    │   │   │   ├── shell-terminal.tsx
    │   │   │   ├── sidebar.tsx
    │   │   │   ├── status-bar.tsx
    │   │   │   ├── terminal.tsx
    │   │   │   ├── update-banner.tsx
    │   │   │   └── onboarding/
    │   │   │       ├── step-authenticate.tsx
    │   │   │       ├── step-dev-root.tsx
    │   │   │       ├── step-mode.tsx
    │   │   │       ├── step-optional.tsx
    │   │   │       ├── step-project.tsx
    │   │   │       ├── step-provider.tsx
    │   │   │       ├── step-ready.tsx
    │   │   │       ├── step-remote.tsx
    │   │   │       ├── step-welcome.tsx
    │   │   │       └── wizard-stepper.tsx
    │   │   └── ui/
    │   │       ├── accordion.tsx
    │   │       ├── alert-dialog.tsx
    │   │       ├── alert.tsx
    │   │       ├── aspect-ratio.tsx
    │   │       ├── avatar.tsx
    │   │       ├── badge.tsx
    │   │       ├── breadcrumb.tsx
    │   │       ├── button-group.tsx
    │   │       ├── button.tsx
    │   │       ├── calendar.tsx
    │   │       ├── card.tsx
    │   │       ├── carousel.tsx
    │   │       ├── chart.tsx
    │   │       ├── checkbox.tsx
    │   │       ├── collapsible.tsx
    │   │       ├── command.tsx
    │   │       ├── context-menu.tsx
    │   │       ├── dialog.tsx
    │   │       ├── drawer.tsx
    │   │       ├── dropdown-menu.tsx
    │   │       ├── empty.tsx
    │   │       ├── field.tsx
    │   │       ├── form.tsx
    │   │       ├── hover-card.tsx
    │   │       ├── input-group.tsx
    │   │       ├── input-otp.tsx
    │   │       ├── input.tsx
    │   │       ├── item.tsx
    │   │       ├── kbd.tsx
    │   │       ├── label.tsx
    │   │       ├── menubar.tsx
    │   │       ├── navigation-menu.tsx
    │   │       ├── pagination.tsx
    │   │       ├── popover.tsx
    │   │       ├── progress.tsx
    │   │       ├── radio-group.tsx
    │   │       ├── resizable.tsx
    │   │       ├── scroll-area.tsx
    │   │       ├── select.tsx
    │   │       ├── separator.tsx
    │   │       ├── sheet.tsx
    │   │       ├── sidebar.tsx
    │   │       ├── skeleton.tsx
    │   │       ├── slider.tsx
    │   │       ├── sonner.tsx
    │   │       ├── spinner.tsx
    │   │       ├── switch.tsx
    │   │       ├── table.tsx
    │   │       ├── tabs.tsx
    │   │       ├── textarea.tsx
    │   │       ├── toast.tsx
    │   │       ├── toaster.tsx
    │   │       ├── toggle-group.tsx
    │   │       ├── toggle.tsx
    │   │       ├── tooltip.tsx
    │   │       ├── use-mobile.tsx
    │   │       └── use-toast.ts
    │   ├── hooks/
    │   │   ├── use-mobile.ts
    │   │   └── use-toast.ts
    │   ├── lib/
    │   │   ├── auth-guard.ts
    │   │   ├── auth.ts
    │   │   ├── browser-slash-command-dispatch.ts
    │   │   ├── command-surface-contract.ts
    │   │   ├── dashboard-metrics.ts
    │   │   ├── dev-overrides.tsx
    │   │   ├── diagnostics-types.ts
    │   │   ├── git-summary-contract.ts
    │   │   ├── image-utils.ts
    │   │   ├── initial-gsd-header-filter.ts
    │   │   ├── knowledge-captures-types.ts
    │   │   ├── project-store-manager.tsx
    │   │   ├── project-url.ts
    │   │   ├── proxy-auth.ts
    │   │   ├── pty-chat-parser.ts
    │   │   ├── pty-manager.ts
    │   │   ├── remaining-command-types.ts
    │   │   ├── secure-path.ts
    │   │   ├── session-browser-contract.ts
    │   │   ├── settings-types.ts
    │   │   ├── shutdown-gate.ts
    │   │   ├── use-editor-font-size.ts
    │   │   ├── use-terminal-font-size.ts
    │   │   ├── use-user-mode.ts
    │   │   ├── utils.ts
    │   │   ├── visualizer-types.ts
    │   │   ├── workflow-action-execution.ts
    │   │   ├── workflow-actions.ts
    │   │   ├── workspace-status.ts
    │   │   ├── workspace-types.ts
    │   │   ├── xterm-theme.ts
    │   │   └── __tests__/
    │   │       ├── dashboard-metrics-fallback.test.ts
    │   │       ├── extension-ui-contract.test.ts
    │   │       ├── shutdown-gate.test.ts
    │   │       └── visualizer-types-contract.test.ts
    │   └── styles/
    │       └── globals.css
    ├── .github/
    │   ├── CODEOWNERS
    │   ├── dependabot.yml
    │   ├── FUNDING.yml
    │   ├── PULL_REQUEST_TEMPLATE.md
    │   ├── ISSUE_TEMPLATE/
    │   │   ├── bug_report.yml
    │   │   ├── config.yml
    │   │   └── feature_request.yml
    │   └── workflows/
    │       ├── ai-triage.yml
    │       ├── build-native.yml
    │       ├── ci.yml
    │       ├── cleanup-dev-versions.yml
    │       ├── dev-publish.yml
    │       ├── forensics-check.yml
    │       ├── next-publish.yml
    │       ├── pipeline.yml
    │       ├── pr-risk.yml
    │       ├── prod-release.yml
    │       ├── security-audit.yml
    │       └── version-check.yml
    └── .plans/
        ├── api-key-manager.md
        ├── autocomplete-qol-improvements.md
        ├── directory-safeguards.md
        ├── doctor-cleanup-consolidation.md
        ├── dynamic-model-discovery.md
        ├── extension-loading-multi-path.md
        ├── fix-high-cpu-process-lifecycle.md
        ├── issue-125-provider-fallback.md
        ├── issue-524-git2-migration.md
        ├── issue-575-dynamic-model-routing.md
        ├── issue-672-parallel-milestone-orchestration.md
        ├── left-native-tui-main-session-plan.md
        ├── native-perf-optimizations.md
        ├── ollama-native-provider.md
        ├── onboarding-detection-wizard.md
        ├── preferences-wizard-completeness.md
        ├── single-writer-engine-v3-control-plane.md
        ├── startup-performance.md
        ├── token-optimization-suite.md
        ├── tui-dashboard-cleanup.md
        └── workflow-templates.md


Files Content:

(Files content cropped to 300k characters, download full ingest to see more)
================================================
FILE: CONTEXT.md
================================================
# CONTEXT

## Domain glossary

- **Auto Orchestration**: runtime coordination of GSD auto-mode units from start to completion, including dispatch and stop/resume behavior; unit-execution failure recovery is classified by the Recovery Classification module.
- **Unit**: the smallest executable workflow step (e.g., plan slice, execute task, complete slice).
- **Unit progression**: movement from one Unit to the next under orchestration rules.
- **Dispatch decision**: selection of the next Unit plus rationale and preconditions.
- **Recovery decision**: retry/escalate/abort choice after runtime failure.
- **Runtime persistence**: lock state, transition journal, and any persisted execution state required for safe resume.
- **DB snapshot persistence**: crash-safe persistence of a full SQLite image exported from `sql.js`, written as a same-directory temporary file and atomically renamed over the live database path.
- **Worktree Lifecycle**: creation, entry, teardown, and merge of an auto-mode worktree, including `s.basePath` mutation, `process.chdir` discipline, and milestone lease coordination.
- **Worktree State Projection**: directional flow of state files between the project root and the auto-worktree, where one side is authoritative per file class (e.g., project root is authoritative for `completed-units.json` after crash recovery; worktree is authoritative for in-flight artifacts).
- **Drift**: a state-shape mismatch between DB rows, disk artifacts, and in-memory state that has a known repair. Distinct from a `blocker`, which describes a terminal condition needing human attention or recovery escalation.
- **Drift catalog**: the discriminated union of drift kinds the State Reconciliation Module recognizes and can repair.

## Architecture terms adopted for this area

- **Auto Orchestration module**: the module that owns the pre-dispatch invariant pipeline and lifecycle telemetry. It runs the resource-version guard and pre-dispatch health gate before reconciliation, then gates whether a Unit may dispatch (resource-version guard → pre-dispatch health gate → State Reconciliation → Dispatch decision → Tool Contract → Worktree Safety) and journals lifecycle transitions, but does not execute the Unit or own runtime recovery for Unit-execution failures. The auto-loop runs the Unit and calls Recovery Classification directly when it fails.
- **Dispatch adapter**: adapter behind the Dispatch seam.
- **Recovery adapter**: adapter behind the Recovery seam.
- **Worktree adapter**: adapter behind the Worktree seam.
- **Health adapter**: adapter behind the Health seam.
- **Runtime persistence adapter**: adapter behind the Runtime persistence seam.
- **Notification adapter**: adapter behind the Notification seam.
- **DB snapshot persistence module**: the deep module that owns `sql.js` snapshot write semantics, including temp-file naming, fsync, cleanup, and rename ordering.
- **State Reconciliation module**: module that runs `reconcileBeforeDispatch` before any Dispatch decision or worker spawn. Surfaces terminal `blockers: string[]` and machine-actionable `DriftRecord[]`. Owns the drift catalog (detectors and idempotent repairs). Throws `ReconciliationFailedError` to Recovery Classification on persistent or repair-failed drift. See `docs/dev/ADR-017-state-reconciliation-drift-driven.md`.
- **Worktree Safety module**: module that validates project root, worktree registration, lease ownership, and git health before a source-writing Unit runs.
- **Worktree Lifecycle module**: module that owns worktree create/enter/teardown/merge verbs, `s.basePath` mutation, and `process.chdir` discipline. Sole owner of these mutations across single-loop and parallel callers.
- **Worktree State Projection module**: module that owns the direction-and-rules of state file flow between project root and auto-worktree. Encodes the bug-hardened invariants (additive milestone copy, ASSESSMENT verdict overwrite, completed-units forward-sync, WAL/SHM cleanup) that `syncProjectRootToWorktree` and `syncStateToProjectRoot` carry today.
- **Recovery Classification module**: module that maps provider, tool, policy, git, worktree, runtime, and reconciliation-drift failures to a Recovery decision.
- **Tool Contract module**: module that keeps Unit prompts, tool schemas, tool policy, and pre-dispatch validation aligned.
- **DriftRecord**: typed, machine-actionable signal of a single drift instance. Discriminated union over drift kinds; carries the identifiers (e.g., milestone id, slice id) the matching repair needs.
- **Drift repair**: idempotent function that resolves one `DriftRecord`. Repairs are owned by the State Reconciliation Module's `drift/` folder; owning modules retain raw primitives (DB writes, file IO) but not the detection-and-repair composition.
- **Reconciliation pass**: one cycle of derive → detect drift → apply repairs → re-derive, performed by `reconcileBeforeDispatch`. Capped at 2 passes per call; loops only when the prior pass fully succeeded but new drift surfaces in the re-derive.

## Current decision in force

- Auto-mode architecture should deepen around a single Auto Orchestration module with interface:
  - `start(sessionContext)`
  - `advance()`
  - `resume()`
  - `stop(reason)`
  - `getStatus()`

See `docs/dev/ADR-014-auto-orchestration-deep-module.md`.

- Runtime invariants should deepen into four first-class modules: State Reconciliation, Worktree Safety, Recovery Classification, and Tool Contract.

See `docs/dev/ADR-015-runtime-invariant-modules.md`.

- Auto Orchestration `advance()` should call invariant modules explicitly in sequence rather than hiding the pre-dispatch pipeline inside the Dispatch adapter:
  - State Reconciliation
  - Dispatch decision
  - Tool Contract
  - Worktree Safety
  - Runtime persistence/journal

Dispatch remains responsible for selecting the next Unit from reconciled state. It should not own DB/disk repair, tool-policy compilation, or worktree root preparation.

- Worktree Safety should fail closed for source-writing Units under worktree isolation. A Unit whose Tool Contract permits writes outside `.gsd/**` must run in a proven milestone worktree root; it must not silently degrade to project-root source writes when the worktree is missing, empty, unregistered, on the wrong branch, or no longer lease-owned. Planning-only Units may continue to write `.gsd/**` artifacts at the project root.

- State Reconciliation should be drift-driven. The Module surfaces terminal `blockers: string[]` and machine-actionable `DriftRecord[]`. Each pre-dispatch and pre-spawn site calls `reconcileBeforeDispatch` (strict closure). Drift catalog includes sketch-flag, merge-state, stale-render, stale-worker, unregistered-milestone, roadmap-divergence, missing-completion-timestamp. Repairs are idempotent. Re-derive is capped at 2 passes (loops only on cascading-drift success path). Persistent or repair-failed drift throws `ReconciliationFailedError` to Recovery Classification (kind `reconciliation-drift`).

  See `docs/dev/ADR-017-state-reconciliation-drift-driven.md`.

## Current implementation snapshot (phase 1)

- `auto.ts` now wires a concrete Auto Orchestration module through `createWiredAutoOrchestrationModule(...)`.
- Session state now carries orchestration status via `AutoSession.orchestration`.
- Runtime snapshot exports orchestration telemetry (`orchestrationPhase`, `orchestrationTransitionCount`, `orchestrationLastTransitionAt`).
- Initial adapters are live for Dispatch, Health, and Runtime persistence seams.
- Main auto-loop dispatch is still the existing path; orchestration seam is integrated incrementally for lifecycle and observability.

## Triage synthesis (2026-05-05)

Recent triage showed repeated failures concentrated in orchestration state coherence, worktree hygiene, and tool-surface contracts.

### Common issue families

- **State drift between DB, disk artifacts, and in-memory loop state**
- Stale flags/rows repeatedly re-dispatch units (`is_sketch`, stale worker/lock, stale sequence/dependency rows)
- Disk artifacts exist but DB status lags or never reconciles (`PROJECT.md` milestone registration, completion timestamps, roadmap divergence)
- Recovery helpers exist but are not wired into dispatch/state derivation paths

- **Worktree lifecycle and path-root ambiguity**
- Units dispatch into ghost/invalid worktree roots (`.git` missing, fallback path-only creation, non-worktree git operations)
- Health checks are unit-specific instead of lifecycle-wide, allowing earlier units to write in invalid roots
- Worktree exit/merge decisions rely on brittle artifact signals instead of authoritative branch/commit state

- **Auto-loop recovery policy gaps**
- Deterministic and schema-validation failure modes are misclassified as generic provider failures
- Retry counters and stuck-loop controls are inconsistently keyed or reset across pause/resume boundaries
- Terminal guardrails are bypassed in side branches (e.g., complete-milestone placeholder behavior)

- **Tool contract mismatches**
- Prompt/tool/schema drift causes repeated invalid calls (`gsd_exec` runtime enums, closeout prompts vs policy constraints)
- Tool availability/surface inconsistencies under session boot/registration timing
- Validation happens too late (pre-exec catches issues that planner tools should reject upfront)

- **Provider/platform integration edge cases**
- Windows process/pipe semantics (`EOF`/abort timing) not normalized with POSIX assumptions
- Provider-specific metadata/capabilities not fully surfaced (reasoning support, context budgeting semantics, model override behavior)

- **Telemetry and diagnosis blind spots**
- Exit reasons collapse into `other`, masking repeatable failure classes
- Missing/imbalanced lifecycle events (`iteration-end`, dispatch/settlement gaps) weaken forensics and automated recovery decisions

### Priority review focus areas

- **Dispatch and state derivation invariants**
- Verify every state gate has a deterministic DB+disk reconciliation path before dispatch
- Ensure sketch/refine/plan transitions clear lifecycle flags atomically

- **Recovery and error classification**
- Add explicit classes for tool-schema overload, deterministic policy blocks, stale worker states, and worktree invalidity
- Ensure each class maps to an intentional action (`retry`, `pause with remediation`, `self-heal`, `stop`)

- **Worktree safety envelope**
- Enforce root validity checks for all source-writing unit types, not only `execute-task`
- Fail closed on worktree creation/registration errors; do not spawn workers into unresolved paths

- **Prompt-policy-tool alignment**
- Review every unit prompt against effective tools policy and schema enums
- Remove contradictory instructions (e.g., “fix failures” where policy forbids writes)

- **Migration and reconciliation**
- Add startup and pre-dispatch reconciliation for PROJECT/ROADMAP/DB drift
- Persist completion metadata consistently during recover/import flows

- **Observability completeness**
- Normalize exit reasons with dedicated buckets
- Guarantee dispatch lifecycle event pairs and settlement records for each unit attempt

### Deepening opportunities from triage

#### State Reconciliation module

- Files: `state.ts`, `gsd-db.ts`, `db-writer.ts`, `md-importer.ts`, `auto-recovery.ts`, PROJECT/ROADMAP parsers
- Problem: DB rows, markdown projections, and cached state are reconciled opportunistically. Helpers such as sketch-flag repair exist but are not wired into the state path, so bugs reappear as wrong Dispatch decisions.
- Refactor target: expose one pre-dispatch reconciliation Interface, e.g. `reconcileBeforeDispatch(basePath)`, that refreshes DB reads, repairs known projection drift, returns blocking inconsistencies, and invalidates derived-state caches.
- Leverage: Dispatch and recovery callers stop needing to know each artifact-specific repair rule.
- Locality: sketch flags, PROJECT milestone registration, ROADMAP sequence/dependency sync, completion timestamps, and artifact/DB mismatch handling move into one module.
- Test focus: call the Interface with DB+disk fixture states and assert the resulting state changes or blockers.

#### Worktree Safety module

- Files: `worktree-resolver.ts`, `auto/phases.ts`, `auto-worktree.ts`, `worktree-manager.ts`, parallel and slice-parallel orchestrators, git helpers
- Problem: worktree validity is checked in scattered, unit-specific places. Some paths build a worktree path string without proving it is registered, has `.git`, owns the lease, and matches `GSD_PROJECT_ROOT`.
- Refactor target: expose one Interface for source-writing Units, e.g. `prepareUnitRoot(unitType, unitId)`, that returns a valid root or a typed `worktree-invalid` Recovery decision.
- Leverage: every source-writing Unit receives the same root validation, lease fencing, and failure classification.
- Locality: ghost worktree, missing `.git`, stale worktree, branch/HEAD mismatch, and worktree cleanup logic are reviewed in one module.
- Test focus: invalid root, missing `.git`, stale path-only fallback, branch mismatch, and `GSD_PROJECT_ROOT` cases.

#### Recovery Classification module

- Files: `error-classifier.ts`, `bootstrap/agent-end-recovery.ts`, `auto-post-unit.ts`, `auto-timeout-recovery.ts`, `crash-recovery.ts`, `provider-error-pause.ts`
- Problem: recovery behavior is distributed across provider handlers, post-unit verification, timeout recovery, and crash cleanup. New deterministic failures often fall through as generic provider errors or `other` exit reasons.
- Refactor target: expose one failure taxonomy Interface, e.g. `classifyFailure(input) -> Recovery decision`, with explicit classes for tool schema, deterministic policy, stale worker, worktree invalid, provider quota, network, and verification drift.
- Leverage: callers ask for a Recovery decision instead of re-implementing retry/pause/stop semantics.
- Locality: bounded retries, pause messages, auto-resume behavior, exit reason normalization, and telemetry buckets are changed together.
- Test focus: table-driven classification and action tests covering every known triage failure family.

#### Tool Contract module

- Files: `unit-context-manifest.ts`, prompts under `prompts/`, `bootstrap/write-gate.ts`, `bootstrap/exec-tools.ts`, `workflow-tool-executors.ts`, `pre-execution-checks.ts`, `tools/plan-slice.ts`
- Problem: Unit prompts, tool schemas, and tool policy drift independently. The model can be instructed to do work the policy blocks, or call a schema value the tool rejects. Some validation waits until after planning artifacts are committed.
- Refactor target: compile a Unit Tool Contract before dispatch that includes prompt obligations, allowed tools, schema enum values, validation requirements, and closeout tools.
- Leverage: prompt authors and dispatch code get one reviewable contract per Unit type.
- Locality: prompt wording, policy gates, schema descriptions, and planner-time validation stop drifting across files.
- Test focus: prompt/policy/schema parity tests and planner tool validation tests for concrete task inputs.

#### Auto Orchestration adapter depth pass

- Files: `auto/orchestrator.ts`, `auto/contracts.ts`, `auto/phases.ts`, `auto.ts`, `auto-post-unit.ts`
- Problem: ADR-014 introduced adapter seams, but adapter boundaries can become too shallow if Dispatch hides unrelated pre-dispatch invariants. That would make `advance()` look simple while preserving the same cross-cutting state repair, tool-contract, and worktree-safety coupling behind a larger Dispatch adapter.
- Refactor target: keep `advance()` as the explicit lifecycle pipeline owner. It should call State Reconciliation before Dispatch, then call Tool Contract and Worktree Safety checks for the selected Unit before persisting/journaling the transition.
- Leverage: reviewers can inspect orchestration ordering in one place, tests can assert the invariant sequence directly, and each adapter stays deep around one concern.
- Locality: orchestration flow remains in Auto Orchestration; invariant modules own their own policy; Dispatch only selects the next Unit from reconciled state.
- Test focus: contract tests for `advance()` ordering, short-circuit behavior, idempotency for the same reconciled snapshot, and typed failure handoff to Recovery Classification.

### Refactor order

- Start with the **Auto Orchestration adapter depth pass** so `advance()` has an explicit invariant pipeline before individual modules are extracted underneath it.
- Then implement the **State Reconciliation module** and **Worktree Safety module**. They address the highest-cost loops and prevent invalid Dispatch decisions before a model turn is launched.
- Follow with the **Recovery Classification module** to normalize outcomes once invalid runtime states are no longer the dominant source of noise.
- Then add the **Tool Contract module** to prevent prompt/schema/policy drift from creating new recovery cases.

### Standing review checklist for this context

- Is DB state authoritative, and if yes, where is disk->DB reconciliation guaranteed?
- Can this unit dispatch into an invalid basePath/worktree and still mutate artifacts?
- Are retry/stuck-loop counters stable across pause/resume and keyed consistently by unit identity?
- Do prompt instructions require tools or writes blocked by the current policy?
- Can tool schema/documentation mismatch induce repeated invalid calls?
- Does each abnormal stop path produce a distinct reason code and actionable remediation?



================================================
FILE: CONTRIBUTING.md
================================================
# Contributing to GSD-2

We're glad you're here. GSD-2 is an open project and contributions are welcome across the entire codebase. We hold a high bar for what gets merged — not to be gatekeepers, but because every change ships to real users and stability matters.

Read [VISION.md](VISION.md) before contributing. It defines what GSD-2 is, what it isn't, and what we won't accept.

## Before you start

1. **Check existing issues.** Someone may already be working on it.
2. **Claim the issue.** Comment on the issue to get it assigned to you before writing code. This prevents duplicate work and wasted effort.
3. **No issue? Create one first** for new features. Bug fixes for obvious problems can skip this step.
4. **Architectural changes require an RFC.** If your change touches core systems (auto-mode, agent-core, orchestration), open an issue describing your approach and get approval before writing code. We use Architecture Decision Records (ADRs) for significant decisions.

### First-time contributors

We are not a fan of drive-by first-time contributions. If this is your first PR to GSD-2, you **must** open an issue first describing the problem or feature, wait for a maintainer response, and link the issue in your PR. First-time PRs that show up with no prior issue will be closed without review. This is not optional — it exists because triaging unsolicited code from unknown contributors is more expensive than the contribution is worth.

Once you have one merged PR, this requirement no longer applies to you.

## Local development

### One-time setup (after cloning)

```bash
npm ci                              # Install dependencies — MUST run first
npm run secret-scan:install-hook    # Install git hooks — run once
```

`npm ci` creates workspace symlinks in `node_modules/@gsd/*` and `node_modules/@gsd-build/*` pointing to `packages/`. These are required for builds and tests to resolve packages correctly.

Run `npm run secret-scan:install-hook` once after cloning. It installs a pre-commit hook that blocks commits containing hardcoded secrets or credentials. Conventional Commits format is validated by CI on push.

### Day-to-day development

```bash
npm run build    # Build
npm test         # Run tests
```

If `npm run build` fails after running tests (e.g. `Cannot find module '@gsd/*'` errors), run `npm ci` first to restore workspace symlinks, then try again.

### Before pushing

```bash
npm run verify:pr    # Local preflight: build:core → typecheck:extensions → test:unit
```

If `verify:pr` fails after running tests (e.g. `Cannot find module '@gsd/*'` errors), run `npm ci` first to restore workspace symlinks, then try again.

### Cross-platform note

GSD-2 runs on macOS, Linux, and Windows:

- **Git hooks** are executed by Git's bundled shell, so they work from any terminal (CMD, PowerShell, Git Bash).
- **Shell scripts** in `scripts/` use `#!/usr/bin/env bash`. On Windows, Git may convert these to CRLF line endings, which breaks the shebang. If you hit shebang errors when running lint scripts locally, create a `.gitattributes` file in the repo root with `*.sh text eol=lf` to force LF checkout. Do not commit this file.

## Branching and commits

Always work on a dedicated branch. Never push directly to `main`.

**Branch naming:** `<type>/<short-description>`

| Type        | When to use                            |
| ----------- | -------------------------------------- |
| `feat/`     | New functionality                      |
| `fix/`      | Bug or defect correction               |
| `refactor/` | Code restructuring, no behavior change |
| `test/`     | Adding or updating tests               |
| `docs/`     | Documentation only                     |
| `chore/`    | Dependencies, tooling, housekeeping    |
| `ci/`       | CI/CD configuration                    |

**Commit messages** must follow [Conventional Commits](https://www.conventionalcommits.org/). The commit-msg hook enforces this locally; CI enforces it on push.

```
<type>(<scope>): <short summary>
```

Valid types: `feat` `fix` `docs` `chore` `refactor` `test` `infra` `ci` `perf` `build` `revert`

```
feat(pi-agent-core): add streaming output for long-running tasks
fix(pi-ai): resolve null pointer on empty provider response
chore(deps): bump typescript from 5.3.0 to 5.4.2
```

Keep branches current by rebasing onto `main` — do not merge `main` into your feature branch:

```bash
git fetch origin
git rebase origin/main
```

CI must pass before your PR will be reviewed. Run `npm run verify:pr` locally before pushing to catch issues early.

## Working with GSD (team workflow)

GSD uses worktree-based isolation for multi-developer work. If you're contributing with GSD running, enable team mode in your project preferences:

```yaml
# .gsd/PREFERENCES.md
---
version: 1
mode: team
---
```

This enables unique milestone IDs, branch pushing, and pre-merge checks — preventing milestone ID collisions when multiple contributors run auto-mode simultaneously. Each developer gets their own isolated worktree; squash merges to `main` happen independently.

For full details see [docs/working-in-teams.md](docs/working-in-teams.md) and [docs/git-strategy.md](docs/git-strategy.md).

## Opening a pull request

### PR description format

Every PR needs a **TL;DR** and a **detailed explanation**. Use this structure:

```
## TL;DR

**What:** One sentence — what does this change?
**Why:** One sentence — why is it needed?
**How:** One sentence — what's the approach?

## What

Detailed description of the change. What files, modules, or systems are affected?

## Why

The motivation. What problem does this solve? What was broken, missing, or suboptimal?
Link issues where applicable: `Closes #123`

## How

The approach. How does the implementation work? What were the key decisions?
If this is a non-trivial change, explain the design and any alternatives you considered.
```

### Requirements

- **CI must pass.** If your PR breaks tests, fix them before requesting review.
- **Run `npm run verify:pr` locally before pushing.** See [Local development](#local-development) for setup.
- **One concern per PR.** A bug fix is a bug fix. A feature is a feature. Don't bundle unrelated changes.
- **No drive-by formatting.** Don't reformat code you didn't change. Don't reorder imports in files you're not modifying.
- **Link issues when relevant.** Not mandatory for every PR, but if an issue exists, reference it.

### Change type checklist

Include in your PR:

- [ ] `feat` — New feature or capability
- [ ] `fix` — Bug fix
- [ ] `refactor` — Code restructuring (no behavior change)
- [ ] `test` — Adding or updating tests
- [ ] `docs` — Documentation only
- [ ] `chore` — Build, CI, or tooling changes

### Breaking changes

If your PR changes any public API, CLI behavior, config format, or file structure, say so explicitly. Breaking changes need extra scrutiny and may need migration guidance.

## AI-assisted contributions

AI-generated PRs are first-class citizens here. We welcome them. We just ask for transparency:

- **Disclose it.** Note that the PR is AI-assisted in your description. Do not credit the AI tool as an author or co-author in the commit or PR.
- **Test it.** AI-generated code must be tested to the same standard as human-written code. "The AI said it works" is not a test plan.
- **Understand it.** You should be able to explain what the code does and why. If a reviewer asks a question, "I'll ask the AI" is not an answer.

AI agents opening PRs must follow the same workflow as human contributors: clean working tree, new branch per task, CI passing before requesting review. Multi-phase work should start as a Draft PR and only move to Ready when complete.

AI PRs go through the same review process as any other PR. No special treatment in either direction.

## Architecture guidelines

Before writing code, understand these principles:

- **Extension-first.** Can this be an extension instead of a core change? If yes, build it as an extension.
- **Simplicity wins.** Don't add abstractions, helpers, or utilities for one-time operations. Don't design for hypothetical future requirements.
- **Tests are the contract.** Changed behavior? The test suite tells you what you broke.

See [VISION.md](VISION.md) for the full list of what we won't accept.

## Extension contributions

GSD is extension-first. If your feature can be an extension, build it as an extension. See [docs/extension-sdk/](docs/extension-sdk/) for the authoritative guide.

### Extension tiers

| Tier | Ships with GSD | Can be disabled | Review bar |
|------|---------------|-----------------|------------|
| **core** | Yes | No | RFC required. Reserved for foundational systems (GSD workflow, built-in tools). |
| **bundled** | Yes | Yes | Standard PR review. Default for new features shipped with GSD. |
| **community** | No | Yes | User-installed. No review needed — lives in `~/.gsd/agent/extensions/` or `.gsd/extensions/`. |

### Required for bundled extension PRs

1. **`extension-manifest.json`** — every extension directory must include one. See [Manifest Spec](docs/extension-sdk/manifest-spec.md).
2. **Tests** — at minimum, test tool registration, command handling, and any event hooks. See [Testing](docs/extension-sdk/testing.md).
3. **State reconstruction** — if your extension has stateful tools, handle `session_start`, `session_switch`, and `session_tree` events. See [Building Extensions](docs/extension-sdk/building-extensions.md#state-management).
4. **`provides` must be accurate** — every tool, command, hook, and shortcut your extension registers must be listed in the manifest.
5. **Use `@gsd/pi-coding-agent`** for types, `@sinclair/typebox` for schemas, `@gsd/pi-tui` for UI. Do not import from `@mariozechner/*` (legacy).

### Promoting community → bundled

If you've built a community extension that should ship with GSD:

1. Open an issue describing the extension and why it should be bundled.
2. Move the source into `src/resources/extensions/<name>/`.
3. Change the tier to `"bundled"` in the manifest.
4. Add tests meeting the standards above.
5. Open a PR following the normal process.

## Scope areas

The codebase is organized into these areas. All are open to contributions:

| Area              | Path                            | Notes                                          |
| ----------------- | ------------------------------- | ---------------------------------------------- |
| Terminal UI       | `packages/pi-tui`               | Components, themes, rendering                  |
| AI/LLM layer      | `packages/pi-ai`                | Provider integrations, model handling          |
| Agent core        | `packages/pi-agent-core`        | Agent orchestration — RFC required for changes |
| Coding agent      | `packages/pi-coding-agent`      | The main coding agent                          |
| MCP server        | `packages/mcp-server`           | Project state tools and MCP protocol           |
| GSD extension     | `src/resources/extensions/gsd/` | GSD workflow — RFC required for auto-mode      |
| Other extensions  | `src/resources/extensions/`     | Browser, search, voice, MCP client, etc.       |
| Native engine     | `native/`                       | Rust N-API modules (grep, git, AST, etc.)      |
| VS Code extension | `vscode-extension/`             | Chat participant, sidebar, RPC integration     |
| Web interface     | `web/`                          | Browser-based dashboard                        |
| CI/Build          | `.github/`, `scripts/`          | Workflows, build scripts                       |
| Documentation     | `docs/`                         | User guides, ADRs, SDK docs                    |

## Review process

PRs go through automated review first, then human review. To help us review efficiently:

- Keep PRs focused and reasonably sized. Massive PRs take longer to review and are more likely to be sent back.
- Respond to review comments. If you disagree, explain why — discussion is welcome.
- If your PR has been open for a while without review, ping in Discord. We're a small team and things slip.

### Stale or unreviewable PRs

Keep open PRs in a reviewable, merge-ready state. For PR authors below maintainer level, PRs that remain blocked by failing CI, merge conflicts, or requested follow-up for more than 3 days may be closed without merge. Prior contributor status is not an exemption; the exemption is maintainer-level repository access.

If your PR is closed for staleness, you are welcome to resubmit when you are ready to bring it back in an issue-free, reviewable state. Closing stale PRs is queue management, not a rejection of future work on the same idea.

Maintainer-level PRs are not auto-closed under this policy, but they may receive a reminder comment asking the author to finish the PR or close it if it is no longer moving.

### 72-hour response policy

Once a maintainer leaves review feedback, you have **72 hours** to respond — either with a code update, a question, or a comment explaining your timeline. We reserve the right to close PRs that go silent past 72 hours. Closed PRs can be reopened once you're ready to engage; we're not trying to throw away your work, we're trying to keep the review queue honest about what's actually moving.

If you know you'll be unavailable, say so in the PR — a one-line "out until Monday" is enough to pause the clock.

### What reviewers verify

Reading a diff is not the same as verifying a change. Our review standard is execution-based, not static-analysis-based.

**What reviewers do:**

1. **Check out the branch** — check out the PR branch locally (or in a worktree). Don't review from the diff view alone.
2. **Build the branch** — run `npm run build`. A diff that doesn't compile is not reviewable.
3. **Run the test suite** — run `npm test`. CI status is a signal, not a substitute for local verification.
4. **Trace root cause for bug fixes** — confirm the diff addresses the root cause described in the issue, not just the symptom.
5. **Check for a regression test** — bug fixes must include a test that would have caught the original bug. If it's absent, the fix is incomplete.
6. **Reject source-grep tests** — any test that reads a source file with `readFileSync` (or equivalent) and asserts via regex/string match/AST inspection is not a test. Send it back. See "No source-grep tests" under Testing standards.

Only after completing these steps should a reviewer make claims about correctness.

**What "looks right" means:**

"Looks right" is the starting point for review, not the conclusion. "The tests pass" only means the tests pass — not that the claimed bug is fixed or the feature works as described. A well-written commit message on a broken change is still a broken change.

### What contributors must provide to unblock review

- **Bug fixes** — include a regression test. A fix without a test is an assertion, not a proof.
- **Features** — include tests covering the primary success path and at least one failure path.
- **Behavior changes** — update or replace any existing tests that cover the changed behavior. Don't leave passing-but-wrong tests in place.

If your PR claims to fix issue #N, reviewers will verify the fix addresses the root cause described in #N — not just that CI is green.

## Testing standards

This project uses Node.js built-in `node:test` as the test runner. All new tests must follow these patterns:

### Use `node:test` and `node:assert/strict`

```typescript
import { describe, test, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
```

Do not use `createTestContext()` from `test-helpers.ts` (legacy, being removed). Do not introduce Jest, Vitest, or other test frameworks.

### Use `beforeEach`/`afterEach` or `t.after()` for cleanup — never `try`/`finally`

```typescript
// ✅ CORRECT — shared fixture with beforeEach/afterEach
describe("feature", () => {
  let tmp: string;
  beforeEach(() => {
    tmp = mkdtempSync(join(tmpdir(), "test-"));
  });
  afterEach(() => {
    rmSync(tmp, { recursive: true, force: true });
  });

  test("case", () => {
    /* clean test body */
  });
});

// ✅ CORRECT — per-test cleanup with t.after()
test("case", (t) => {
  const tmp = mkdtempSync(join(tmpdir(), "test-"));
  t.after(() => {
    rmSync(tmp, { recursive: true, force: true });
  });
  // test body
});

// ❌ WRONG — inline try/finally
test("case", () => {
  const tmp = mkdtempSync(join(tmpdir(), "test-"));
  try {
    // test body
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
});
```

**When to use which:**

- `beforeEach`/`afterEach` — when all tests in a `describe` block share the same setup/teardown pattern
- `t.after()` — when each test has unique cleanup (different fixtures, env vars, etc.)
- `try`/`finally` — only inside standalone helper functions that don't have access to the test context `t` (e.g., `withEnv()`, `capture()`)

### Template literal fixture data

When constructing multi-line fixture content (markdown, YAML, etc.) inside indented test blocks, use array join to avoid unintended leading whitespace:

```typescript
// ✅ CORRECT — no indentation leakage
const content = [
  "## Slices",
  "- [x] **S01: First slice**",
  "- [ ] **S02: Second slice**",
].join("\n");

// ❌ WRONG — template literal inside describe/test adds leading spaces
const content = `
  ## Slices
  - [x] **S01: First slice**
`;
// Each line now has 2 leading spaces, breaking ^## regex anchors
```

### Test-first for bug fixes

Bug fixes must include a regression test that fails before the fix and passes after. Write the test first, confirm it fails, then apply the fix. See the `test-first-bugfix` skill.

### No source-grep tests

A test must execute the code under test. Reading a source file with `readFileSync` (or any equivalent) and asserting against its contents with regex, string matching, or AST inspection is **not a test** — it asserts that the code was *written a certain way*, not that it *behaves correctly*. This is pure Goodhart's Law: the metric (test count, coverage) gets satisfied while the actual property (correctness) is untouched.

```typescript
// ❌ WRONG — source-grep test. Passes if the string exists, regardless of behavior.
test("handles null input", () => {
  const source = readFileSync("src/parser.ts", "utf8");
  assert.match(source, /if \(input === null\)/);
});

// ❌ WRONG — same anti-pattern with AST or string includes.
test("exports the function", () => {
  const source = readFileSync("src/index.ts", "utf8");
  assert.ok(source.includes("export function parse"));
});

// ✅ CORRECT — import the code and exercise it.
import { parse } from "../src/parser.ts";
test("handles null input", () => {
  assert.equal(parse(null), undefined);
});
```

PRs containing source-grep tests will be sent back. CI enforces this via `scripts/check-source-grep-tests.sh` (wired into the `lint` job) — it scans changed test files for `readFileSync` / `readFile` calls whose path argument points into `src/` or `packages/`. If the code under test is genuinely hard to invoke (e.g., a build script, a CLI entry point), invoke it as a subprocess and assert on its real output — not on its source text.

The narrow exception: tests that legitimately verify *file structure* as the actual product (e.g., a code generator's output, a config-file linter, a script that produces a manifest). In those cases the file contents *are* the behavior. Opt out with a same-line or preceding-line marker:

```typescript
// allow-source-grep: this test verifies the codegen output, which IS the product
const generated = readFileSync("packages/codegen/dist/manifest.ts", "utf8");
assert.match(generated, /export const ROUTES =/);
```

The reason becomes part of the diff and is visible at review. If you're not sure whether your case qualifies, it doesn't.

### Three recurring defect classes (CodeRabbit themes)

CI enforces three specific patterns that have caused real bugs in merged PRs (see issue #4931). All three are checked by `scripts/check-coderabbit-themes.mjs`.

**1. `Statement#get()` returns `undefined`, not `null`.** better-sqlite3's `Statement#get(…)` returns `undefined` when no row matches. A `!== null` guard passes for `undefined` too (`undefined !== null` is `true`), so the guard is always truthy and downstream property access crashes.

```typescript
// ❌ WRONG
const row = stmt.get(id);
if (row !== null) {
  use(row.v);  // row is undefined → crash
}

// ✅ CORRECT
const row = stmt.get(id);
if (row != null) { use(row.v); }        // catches both null and undefined
// or
if (row === undefined) return null;
// or
return row ?? defaultValue;
```

**2. Attach `once()` listeners BEFORE the triggering syscall.** If the event fires synchronously (fast exit, already-emitted event), a listener attached after the trigger misses it.

```typescript
// ❌ WRONG — listener can miss sync exit
proc.kill("SIGINT");
await new Promise(resolve => proc.once("exit", resolve));

// ✅ CORRECT — listener first, trigger after
const done = new Promise(resolve => proc.once("exit", resolve));
proc.kill("SIGINT");
await done;
```

**3. `.mjs` importing `.ts` requires `--experimental-strip-types`.** Without the flag (or `--import tsx`, `--loader ts-node`, etc.), Node throws `ERR_UNKNOWN_FILE_EXTENSION`. If your `.mjs` harness imports any `.ts` module, every script that runs it must pass the flag.

```json
// ❌ WRONG
"test:tokens": "node --test studio/test/tokens.test.mjs"

// ✅ CORRECT
"test:tokens": "node --experimental-strip-types --test studio/test/tokens.test.mjs"
```

**Opt-out marker**: `// allow-coderabbit-theme: <reason>` on the same or preceding line, same convention as `allow-source-grep:`. The reason appears in the diff.

## Security

If you find a security vulnerability, **do not open a public issue.** Email the maintainers directly or use GitHub's private vulnerability reporting.

## Questions?

Open a discussion on GitHub or ask in the Discord `#maintainers` channel.



================================================
FILE: Dockerfile
================================================
# ──────────────────────────────────────────────
# Runtime
# Image: ghcr.io/gsd-build/gsd-pi
# Used by: end users via docker run
# ──────────────────────────────────────────────
FROM node:24-slim AS runtime

# Git is required for GSD's git operations
RUN apt-get update && apt-get install -y --no-install-recommends \
    git \
    && rm -rf /var/lib/apt/lists/*

# Install GSD globally — version is controlled by the build arg
ARG GSD_VERSION=latest
RUN npm install -g gsd-pi@${GSD_VERSION}

# Default working directory for user projects
WORKDIR /workspace

ENTRYPOINT ["gsd"]
CMD ["--help"]

# ──────────────────────────────────────────────
# Runtime (local build)
# Image: ghcr.io/gsd-build/gsd-pi:local
# Used by: PR-time e2e smoke, builds the *current source* into an image
# instead of pulling from npm. Lets `tests/e2e/docker/` exercise the actual
# runtime container produced by this branch's code.
# Build with:  docker build --target runtime-local \
#                --build-arg TARBALL=gsd-pi-<version>.tgz -t gsd-pi:local .
# The tarball must be in the build context (created by `npm pack`).
# ──────────────────────────────────────────────
FROM node:24-slim AS runtime-local

RUN apt-get update && apt-get install -y --no-install-recommends \
    git \
    && rm -rf /var/lib/apt/lists/*

ARG TARBALL
COPY ${TARBALL} /tmp/gsd-pi.tgz
# Install with --ignore-scripts: postinstall is unnecessary for the
# version / help smoke this image is built for, and skipping it removes
# a class of network-dependent failures.
#
# Diagnostic output: list /usr/local/bin and the installed package layout
# so a regression here fails loudly at build time with the actual file
# state visible in CI logs (instead of silently producing an image that
# fails at run time with exit 127 / command not found).
#
# Verify the loader is invokable. We pin to `node /path/to/loader.js`
# (not the bin shim) because the npm bin shim is fragile against npm
# prefix drift inside slim images; running the loader directly always
# works as long as dist/ is in the tarball.
RUN npm install -g --ignore-scripts /tmp/gsd-pi.tgz \
    && rm /tmp/gsd-pi.tgz \
    && echo "--- /usr/local/bin ---" \
    && ls -la /usr/local/bin | grep -i gsd || echo "(no gsd entries in /usr/local/bin)" \
    && echo "--- /usr/local/lib/node_modules/gsd-pi ---" \
    && ls -la /usr/local/lib/node_modules/gsd-pi 2>/dev/null | head -10 \
    && test -f /usr/local/lib/node_modules/gsd-pi/dist/loader.js \
    && node /usr/local/lib/node_modules/gsd-pi/dist/loader.js --version

WORKDIR /workspace

# Invoke the loader directly. Avoids any dependency on the npm bin shim
# being placed correctly in /usr/local/bin (which is platform/prefix
# dependent and has been the source of spurious exit-127 failures).
ENTRYPOINT ["node", "/usr/local/lib/node_modules/gsd-pi/dist/loader.js"]
CMD ["--help"]



================================================
FILE: LICENSE
================================================
MIT License

Copyright (c) 2026 Lex Christopherson

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.



================================================
FILE: package.json
================================================
{
  "name": "gsd-pi",
  "version": "3.0.0",
  "description": "GSD — Get Shit Done coding agent",
  "license": "MIT",
  "repository": {
    "type": "git",
    "url": "https://github.com/gsd-build/gsd-2.git"
  },
  "homepage": "https://github.com/gsd-build/gsd-2#readme",
  "bugs": {
    "url": "https://github.com/gsd-build/gsd-2/issues"
  },
  "type": "module",
  "workspaces": [
    "packages/*",
    "extensions/*"
  ],
  "bin": {
    "gsd": "dist/loader.js",
    "gsd-cli": "dist/loader.js",
    "gsd-pi": "scripts/install.js"
  },
  "files": [
    "dist",
    "dist/web",
    "packages",
    "pkg",
    "src/resources",
    "scripts/postinstall.js",
    "scripts/install.js",
    "scripts/link-workspace-packages.cjs",
    "scripts/ensure-workspace-builds.cjs",
    "scripts/lib/workspace-manifest.cjs",
    "package.json",
    "README.md"
  ],
  "piConfig": {
    "name": "gsd",
    "configDir": ".gsd"
  },
  "engines": {
    "node": ">=22.0.0"
  },
  "packageManager": "npm@10.9.3",
  "scripts": {
    "build:pi-tui": "npm run build -w @gsd/pi-tui",
    "build:pi-ai": "npm run build -w @gsd/pi-ai",
    "build:pi-agent-core": "npm run build -w @gsd/pi-agent-core",
    "build:pi-coding-agent": "npm run build:contracts && npm run build -w @gsd/pi-coding-agent",
    "build:native-pkg": "npm run build -w @gsd/native",
    "build:contracts": "npm run build -w @gsd-build/contracts",
    "build:rpc-client": "npm run build -w @gsd-build/rpc-client",
    "build:pi": "npm run build:contracts && npm run build:native-pkg && npm run build:pi-tui && npm run build:pi-ai && npm run build:pi-agent-core && npm run build:pi-coding-agent",
    "build:mcp-server": "npm run build -w @gsd-build/mcp-server",
    "build:core": "npm run build:contracts && npm run build:pi && npm run build:rpc-client && npm run build:mcp-server && tsc && npm run copy-resources && npm run copy-themes && npm run copy-export-html",
    "build": "npm run build:core && node scripts/build-web-if-stale.cjs",
    "stage:web-host": "node scripts/stage-web-standalone.cjs",
    "build:web-host": "cross-env NODE_ENV=production npm --prefix web run build && npm run stage:web-host",
    "copy-resources": "node scripts/copy-resources.cjs",
    "copy-themes": "node scripts/copy-themes.cjs",
    "copy-export-html": "node scripts/copy-export-html.cjs",
    "test:compile": "node scripts/compile-tests.mjs",
    "baseline:refactor": "node scripts/refactor-baseline.mjs",
    "baseline:refactor:gate": "node --import ./src/resources/extensions/gsd/tests/resolve-ts.mjs --experimental-strip-types --test src/tests/refactor-baseline.test.ts src/tests/rpc-golden-fixtures.test.ts src/tests/prompt-golden-fixtures.test.ts src/tests/contracts-rpc-fixtures.test.ts",
    "baseline:refactor:phase0": "npm run baseline:refactor:gate && node --import ./src/resources/extensions/gsd/tests/resolve-ts.mjs --experimental-strip-types --test src/resources/extensions/gsd/tests/derive-state-db.test.ts src/resources/extensions/gsd/tests/single-writer-invariant.test.ts src/resources/extensions/gsd/tests/auto-recovery.test.ts src/resources/extensions/gsd/tests/auto-worktree-registry.test.ts",
    "legacy:cleanup:gate": "node scripts/legacy-cleanup-gate.mjs",
    "legacy:cleanup:evidence": "node scripts/legacy-cleanup-evidence.mjs",
    "test:unit:compiled": "node --import ./scripts/dist-test-resolve.mjs --experimental-test-isolation=process --test-reporter=./scripts/test-reporter-compact.mjs --test \"dist-test/src/tests/*.test.js\" \"dist-test/src/resources/extensions/gsd/tests/*.test.js\" \"dist-test/src/resources/extensions/gsd/tests/*.test.mjs\" \"dist-test/src/resources/extensions/shared/tests/*.test.js\" \"dist-test/src/resources/extensions/subagent/tests/*.test.js\" \"dist-test/src/resources/extensions/claude-code-cli/tests/*.test.js\" \"dist-test/src/resources/extensions/github-sync/tests/*.test.js\" \"dist-test/src/resources/extensions/universal-config/tests/*.test.js\" \"dist-test/src/resources/extensions/visual-brief/tests/*.test.js\" \"dist-test/src/resources/extensions/voice/tests/*.test.js\" \"dist-test/src/resources/extensions/mcp-client/tests/*.test.js\" \"dist-test/src/resources/extensions/remote-questions/tests/*.test.js\"",
    "test:unit": "npm run test:compile && npm run test:unit:compiled",
    "test:changed:src": "node scripts/verify-changed-src-tests.mjs",
    "test:packages": "npm run test:compile && npm run test:packages:compiled",
    "test:packages:compiled": "node scripts/run-package-tests.cjs",
    "verify:workspace-coverage": "node scripts/verify-workspace-coverage.cjs",
    "test:marketplace": "node scripts/with-env.mjs GSD_TEST_CLONE_MARKETPLACES=1 -- node --import ./src/resources/extensions/gsd/tests/resolve-ts.mjs --experimental-strip-types --test src/resources/extensions/gsd/tests/claude-import-tui.test.ts src/resources/extensions/gsd/tests/plugin-importer-live.test.ts src/tests/marketplace-discovery.test.ts",
    "test:coverage": "cross-env NODE_OPTIONS=--max-old-space-size=8192 c8 --reporter=text --reporter=lcov --exclude=\"src/resources/extensions/gsd/tests/**\" --exclude=\"src/tests/**\" --exclude=\"scripts/**\" --exclude=\"native/**\" --exclude=\"node_modules/**\" --check-coverage --statements=40 --lines=40 --branches=20 --functions=20 node --import ./src/resources/extensions/gsd/tests/resolve-ts.mjs --experimental-strip-types --experimental-test-isolation=process --test src/resources/extensions/gsd/tests/*.test.ts src/resources/extensions/gsd/tests/*.test.mjs src/tests/*.test.ts src/resources/extensions/shared/tests/*.test.ts",
    "test:integration": "node --import ./src/resources/extensions/gsd/tests/resolve-ts.mjs --experimental-strip-types --test \"src/tests/integration/*.test.ts\" \"src/resources/extensions/gsd/tests/integration/*.test.ts\" \"src/resources/extensions/async-jobs/*.test.ts\" \"src/resources/extensions/browser-tools/tests/*.test.mjs\"",
    "pretest": "npm run typecheck:extensions",
    "test": "npm run test:unit && npm run test:integration && npm run test:packages",
    "test:smoke": "node --experimental-strip-types tests/smoke/run.ts",
    "test:live": "node scripts/with-env.mjs GSD_LIVE_TESTS=1 -- node --experimental-strip-types tests/live/run.ts",
    "test:browser-tools": "node --test src/resources/extensions/browser-tools/tests/browser-tools-unit.test.cjs src/resources/extensions/browser-tools/tests/browser-tools-integration.test.mjs",
    "test:native": "npm run test -w @gsd/native",
    "test:secret-scan": "node --import ./src/resources/extensions/gsd/tests/resolve-ts.mjs --experimental-strip-types --test src/tests/secret-scan.test.ts",
    "secret-scan": "node scripts/secret-scan.mjs",
    "secret-scan:install-hook": "node scripts/install-hooks.mjs",
    "build:native": "node native/scripts/build.js",
    "build:native:dev": "node native/scripts/build.js --dev",
    "dev": "node scripts/dev.js",
    "gsd": "node scripts/dev-cli.js",
    "gsd:web": "npm run build:contracts && npm run build:pi && npm run copy-resources && node scripts/build-web-if-stale.cjs && node scripts/dev-cli.js --web",
    "gsd:web:stop": "node scripts/dev-cli.js web stop",
    "gsd:web:stop:all": "node scripts/dev-cli.js web stop all",
    "postinstall": "node scripts/install.js",
    "pi:install-global": "node scripts/install-pi-global.js",
    "pi:uninstall-global": "node scripts/uninstall-pi-global.js",
    "sync-pkg-version": "node scripts/sync-pkg-version.cjs",
    "sync-platform-versions": "node native/scripts/sync-platform-versions.cjs",
    "validate-pack": "node scripts/validate-pack.js",
    "typecheck:extensions": "tsc --noEmit --project tsconfig.extensions.json",
    "verify:pr": "npm run build:core && npm run typecheck:extensions && npm run test:unit",
    "pipeline:version-stamp": "node scripts/version-stamp.mjs",
    "release:changelog": "node scripts/generate-changelog.mjs",
    "release:bump": "node scripts/bump-version.mjs",
    "release:update-changelog": "node scripts/update-changelog.mjs",
    "docker:build-runtime": "docker build --target runtime -t ghcr.io/gsd-build/gsd-pi .",
    "docker:build-builder": "docker build --target builder -t ghcr.io/gsd-build/gsd-ci-builder .",
    "prepublishOnly": "npm run sync-pkg-version && npm run sync-platform-versions && node scripts/prepublish-check.mjs && npm run build && npm run typecheck:extensions && npm run validate-pack",
    "test:live-regression": "node --experimental-strip-types tests/live-regression/run.ts",
    "test:e2e": "node --experimental-strip-types --test \"tests/e2e/*.e2e.test.ts\"",
    "test:e2e:windows-smoke": "node --experimental-strip-types --test tests/e2e/sanity.e2e.test.ts tests/e2e/migration.e2e.test.ts tests/e2e/native-abi.e2e.test.ts",
    "test:e2e:docker": "node --experimental-strip-types --test \"tests/e2e/docker/**/*.e2e.test.ts\""
  },
  "dependencies": {
    "@anthropic-ai/sdk": "^0.90.0",
    "@anthropic-ai/vertex-sdk": "^0.14.4",
    "@aws-sdk/client-bedrock-runtime": "^3.983.0",
    "@clack/prompts": "^1.1.0",
    "@google/genai": "^1.40.0",
    "@mariozechner/jiti": "^2.6.2",
    "@mistralai/mistralai": "^1.14.1",
    "@modelcontextprotocol/sdk": "^1.27.1",
    "@silvia-odwyer/photon-node": "^0.3.4",
    "@sinclair/typebox": "^0.34.41",
    "ajv": "^8.17.1",
    "ajv-formats": "^3.0.1",
    "chalk": "^5.6.2",
    "chokidar": "^5.0.0",
    "diff": "^8.0.2",
    "extract-zip": "^2.0.1",
    "file-type": "^21.1.1",
    "glob": "^13.0.1",
    "hosted-git-info": "^9.0.2",
    "ignore": "^7.0.5",
    "marked": "^15.0.12",
    "minimatch": "^10.2.3",
    "openai": "^6.26.0",
    "picomatch": "^4.0.3",
    "playwright": "^1.58.2",
    "proper-lockfile": "^4.1.2",
    "proxy-agent": "^6.5.0",
    "sharp": "^0.34.5",
    "sql.js": "^1.14.1",
    "strip-ansi": "^7.1.0",
    "undici": "^7.24.2",
    "yaml": "^2.8.2"
  },
  "devDependencies": {
    "@types/node": "^24.12.0",
    "@types/picomatch": "^4.0.2",
    "c8": "^11.0.0",
    "cross-env": "^7.0.3",
    "esbuild": "^0.25.12",
    "jiti": "^2.6.1",
    "typescript": "^5.9.3"
  },
  "optionalDependencies": {
    "@anthropic-ai/claude-agent-sdk": "0.2.83",
    "@gsd-build/engine-darwin-arm64": ">=2.10.2",
    "@gsd-build/engine-darwin-x64": ">=2.10.2",
    "@gsd-build/engine-linux-arm64-gnu": ">=2.10.2",
    "@gsd-build/engine-linux-x64-gnu": ">=2.10.2",
    "@gsd-build/engine-win32-x64-msvc": ">=2.10.2",
    "fsevents": "~2.3.3",
    "koffi": "^2.9.0"
  },
  "overrides": {
    "gaxios": "7.1.4"
  }
}



================================================
FILE: tsconfig.extensions.json
================================================
{
  "extends": "./tsconfig.json",
  "compilerOptions": {
    "noEmit": true,
    "allowImportingTsExtensions": true,
    "allowJs": true,
    "checkJs": false,
    "rootDir": "."
  },
  "include": ["src/resources/extensions", "extensions"],
  "exclude": []
}



================================================
FILE: tsconfig.json
================================================
{
  "compilerOptions": {
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "target": "ES2022",
    "outDir": "dist",
    "rootDir": "src",
    "strict": true,
    "declaration": true,
    "incremental": true,
    "esModuleInterop": true,
    "skipLibCheck": true
  },
  "include": ["src"],
  "exclude": ["src/resources", "src/tests", "src/web"]
}



================================================
FILE: tsconfig.resources.json
================================================
{
  "extends": "./tsconfig.json",
  "compilerOptions": {
    "incremental": false,
    "rootDir": "src/resources",
    "outDir": "dist/resources",
    "declaration": false,
    "declarationMap": false,
    "sourceMap": false
  },
  "include": ["src/resources/extensions/**/*.ts"],
  "exclude": [
    "src/resources/extensions/**/tests/**",
    "src/resources/extensions/**/*.test.ts",
    "src/resources/extensions/**/*.test.mts"
  ]
}



================================================
FILE: tsconfig.test.json
================================================
{
  "extends": "./tsconfig.json",
  "compilerOptions": {
    "declaration": false,
    "noEmit": false
  },
  "include": [
    "src/tests/headless-cli-surface.test.ts",
    "src/tests/ensure-workspace-builds.test.ts",
    "src/headless-events.ts",
    "src/headless-types.ts",
    "src/tests/google-search-oauth-shape.test.ts",
    "src/tests/google-search-auth.repro.test.ts"
  ],
  "exclude": []
}



================================================
FILE: VISION.md
================================================
# Vision

GSD-2 is the orchestration layer between you and AI coding agents. It handles planning, execution, verification, and shipping so you can focus on what to build, not how to wrangle the tools.

## Who it's for

Anyone who codes with AI agents — solo developers shipping faster, open-source maintainers handling scale, vibe coders who think in outcomes. GSD adapts to skill level and workflow.

## Principles

**Extension-first.** If it can be an extension, it should be. Core stays lean. New capabilities belong in extensions, skills, and plugins unless they fundamentally require core integration.

**Simplicity over abstraction.** The codebase was aggressively cleaned up. Every line earns its place. Don't add helpers, utilities, or abstractions unless they eliminate real duplication or solve a real problem. Three similar lines of code is better than a premature abstraction.

**Tests are the contract.** If you change behavior, the tests tell you what you broke. Write tests for new behavior. Trust the test suite.

**Ship fast, fix fast.** Get it out, iterate quickly, don't let perfect be the enemy of good. Every release should work, but we'd rather ship and patch than delay and accumulate.

**Provider-agnostic.** GSD works with any LLM provider. No architectural decisions should privilege one provider over another.

## What we won't accept

These save everyone time. Don't open PRs for:

- **Enterprise patterns.** Dependency injection containers, abstract factories, strategy-pattern-for-the-sake-of-it, over-engineered config systems. This is a CLI tool, not a Spring application.

- **Framework swaps.** Rewriting working code in a different library or framework without a clear, measurable improvement in performance or maintainability. "I prefer X" is not sufficient motivation.

- **Cosmetic refactors.** Renaming variables to your preferred style, reordering imports, reformatting code that works. This is pure churn that creates merge conflicts and review burden for zero user value.

- **Complexity without user value.** If a change adds abstraction, indirection, or configuration but doesn't improve something a user can see or feel, it doesn't belong here.

- **Heavy orchestration layers.** Don't duplicate what the agent infrastructure already provides. Build on top of it, don't wrap it.

## Relationship to GSD-1

GSD-2 is the future. GSD-1 continues to serve its community but GSD-2 is where active development, new features, and architectural investment happen. The goal is to eventually migrate GSD-1 users to GSD-2.



================================================
FILE: .dockerignore
================================================
# ── Build artifacts ──
dist/
build/
coverage/
*.tsbuildinfo

# ── Dependencies ──
node_modules/
packages/*/node_modules/

# ── Environment & secrets ──
.env
.env.*
!.env.example
.gsd/

# ── IDE & OS ──
.idea/
.vscode/
*.code-workspace
.DS_Store
Thumbs.db

# ── Git ──
.git/
.github/

# ── Development files ──
.claude/
.plans/
.artifacts/
.bg-shell/
.bg_shell
*.log
*.swp
*.swo
*~
tmp/
.cache/

# ── Native build artifacts ──
native/
target/

# ── Test fixtures ──
tests/

# ── Lock files (npm is canonical via package-lock.json) ──
pnpm-lock.yaml
bun.lock

# ── Tarballs ──
*.tgz
# Allow the npm pack output for the runtime-local Dockerfile target,
# which is exercised by tests/e2e/docker/runtime.e2e.test.ts.
!gsd-pi-*.tgz



================================================
FILE: .npmignore
================================================
# .npmignore — override .gitignore for npm pack
# When this file exists, npm ignores .gitignore entirely.
# The "files" field in package.json determines what's included.
# This file only needs to exclude things NOT covered by "files".

# Everything not in the "files" array is excluded by default.
# This file exists solely to prevent .gitignore from interfering
# with npm pack (e.g., .gitignore excludes dist/ which we need).



================================================
FILE: .npmrc
================================================
engine-strict=true



================================================
FILE: .prompt-injection-scanignore
================================================
# False positives in GSD prompt templates — these are legitimate LLM instructions, not injection
src/resources/extensions/gsd/prompts/doctor-heal.md:You are now responsible



================================================
FILE: .secretscanignore
================================================
# .secretscanignore — patterns to exclude from secret scanning
#
# Format:
#   filepath:regex   — ignore matches of regex only in the given file
#   regex            — ignore matches of regex in all files
#
# Examples:
#   tests/fixtures/fake-creds.json:AKIA.*
#   EXAMPLE_KEY_DO_NOT_USE
#   src/config.example.ts:password\s*=\s*"changeme"

# Secret scanner test file (contains intentional fake secrets as test inputs)
src/tests/secret-scan.test.ts:.*

# Test fixtures with dummy credentials
tests/*:AKIA_EXAMPLE
tests/*:test-secret-value
tests/*:fake[-_]?(password|secret|token|key)

# Web contract/integration test dummy API keys (not real secrets)
src/tests/integration/web-mode-assembled.test.ts:sk-assembled-test-key
src/tests/integration/web-mode-runtime-fixtures.ts:sk-runtime-recovery-secret
src/tests/web-onboarding-contract.test.ts:sk-test-secret

# Doctor environment tests use dummy localhost DB URLs
src/resources/extensions/gsd/tests/doctor-environment.test.ts:postgres://localhost


# Documentation examples
*.md:AKIA[0-9A-Z]{16}
*.md:sk_(live|test)_

# Environment variable references (not actual values)
process\.env\.\w+
\$\{?\w+_KEY\}?
\$\{?\w+_SECRET\}?
\$\{?\w+_TOKEN\}?

# Placeholder/example values
changeme
your[-_]?api[-_]?key[-_]?here
REPLACE_ME
xxx+
TODO.*secret

# Google OAuth Desktop client public credentials (intentionally public)
# These are known public OAuth client secrets for Desktop App flows.
# See: https://developers.google.com/identity/protocols/oauth2/native-app
packages/pi-ai/src/utils/oauth/google-gemini-cli.ts:CLIENT_SECRET\s*=\s*['"]GOCSPX-[A-Za-z0-9_-]+['"]
packages/pi-ai/src/utils/oauth/google-antigravity.ts:CLIENT_SECRET\s*=\s*['"]GOCSPX-[A-Za-z0-9_-]+['"]



================================================
FILE: docker/README.md
================================================
# GSD Docker Sandbox

Run GSD auto mode inside an isolated Docker sandbox so it cannot touch your host filesystem, SSH keys, or other projects.

## Prerequisites

- Docker Desktop 4.58+ (macOS or Windows; Linux support is experimental)
- At least one LLM provider API key

## Docker Images

| File | Purpose |
|------|---------|
| `Dockerfile.sandbox` | Runtime sandbox with entrypoint (UID remapping, bootstrap) |
| `Dockerfile.ci-builder` | CI builds — includes build tools, no entrypoint magic |

## Compose Files

| File | Purpose |
|------|---------|
| `docker-compose.yaml` | Minimal zero-config setup — just works with sensible defaults |
| `docker-compose.full.yaml` | Fully documented reference with all options, resource limits, health checks |

Start with `docker-compose.yaml`. Copy options from `docker-compose.full.yaml` when you need them.

## Quick Start

### Option A: Docker Sandbox CLI (recommended)

Docker Sandboxes provide MicroVM isolation — each sandbox runs in a lightweight VM with its own kernel and private Docker daemon.

```bash
# Create a sandbox from the template
docker sandbox create --template ./docker --name gsd-sandbox

# Shell into the sandbox
docker sandbox exec -it gsd-sandbox bash

# Inside the sandbox, run GSD
gsd auto "implement the feature described in issue #42"
```

### Option B: Docker Compose

For environments without Docker Sandbox support, use Compose for container-level isolation:

```bash
# 1. Configure API keys
cp docker/.env.example docker/.env
# Edit docker/.env with your keys

# 2. Start the sandbox
docker compose -f docker/docker-compose.yaml up -d

# 3. Shell into the container
docker exec -it gsd-sandbox bash

# 4. Run GSD inside the container
gsd auto "implement the feature described in issue #42"
```

## UID/GID Remapping

The entrypoint handles UID/GID remapping via `PUID` and `PGID` environment variables. This avoids permission issues on bind-mounted volumes by matching the container's `gsd` user to your host UID/GID.

```bash
# Find your host UID/GID
id -u  # PUID
id -g  # PGID
```

Set these in your `.env` file or in the `environment` section of the compose file. Defaults to `1000:1000`.

## Entrypoint Behavior

The container entrypoint (`entrypoint.sh`) runs four steps on every start:

1. **UID/GID remapping** — adjusts the `gsd` user to match `PUID`/`PGID`
2. **Pre-create critical files** — prevents Docker bind-mount from creating directories where files are expected
3. **Sentinel-based bootstrap** — runs `bootstrap.sh` exactly once on first boot
4. **Drop privileges** — `exec gosu gsd` for proper PID 1 signal forwarding

No hardcoded `user:` directive in compose — the entrypoint starts as root, remaps, then drops to `gsd`.

## Two-Terminal Workflow

GSD's recommended workflow uses two terminals — one for auto mode, one for interactive discussion:

```bash
# Terminal 1: auto mode
docker sandbox exec -it gsd-sandbox bash
gsd auto "your task description"

# Terminal 2: discuss / monitor
docker sandbox exec -it gsd-sandbox bash
gsd discuss
```

With Docker Compose, replace `docker sandbox exec` with `docker exec`.

## Credential Injection

### Docker Sandbox (automatic)

Docker's proxy layer forwards API keys set in your host shell config (`~/.bashrc`, `~/.zshrc`) into the sandbox automatically. Keys are never stored inside the sandbox.

### Docker Compose (manual)

Copy `docker/.env.example` to `docker/.env` and fill in your keys. The `.env` file is gitignored and never committed.

## Network Allowlisting

If you restrict outbound network access in your sandbox, GSD needs these endpoints:

| Purpose | Endpoints |
|---------|-----------|
| LLM APIs | `api.anthropic.com`, `api.openai.com`, `generativelanguage.googleapis.com`, `openrouter.ai` |
| Package registry | `registry.npmjs.org` |
| Research tools | `api.search.brave.com`, `api.tavily.com`, `r.jina.ai` |
| GitHub | `api.github.com`, `github.com` |

## Customizing the Image

Build with a specific GSD version:

```bash
docker compose -f docker/docker-compose.yaml build --build-arg GSD_VERSION=2.51.0
```

## Cleanup

```bash
# Docker Sandbox
docker sandbox rm gsd-sandbox

# Docker Compose
docker compose -f docker/docker-compose.yaml down -v
```

## Known Limitations

- **macOS/Windows only**: Docker Sandboxes require Docker Desktop 4.58+. Linux sandbox support is experimental.
- **Environment parity**: The sandbox runs Ubuntu (Debian). macOS-only dependencies may not work inside the sandbox.
- **Named agent registration**: Docker Desktop's built-in named agents (claude, codex, etc.) are registered by Docker itself. Third-party tools cannot register new named agents. GSD uses the generic shell sandbox type with a custom template instead.



================================================
FILE: docker/bootstrap.sh
================================================
#!/bin/bash
set -e

# ──────────────────────────────────────────────
# GSD First-Boot Bootstrap
#
# Runs once on initial container creation.
# Called by entrypoint.sh as the gsd user.
#
# This script is idempotent — safe to run multiple
# times, but the sentinel in entrypoint.sh ensures
# it only runs once in practice.
# ──────────────────────────────────────────────

# ── Git Identity ────────────────────────────────────────
# Without this, git commits inside the container will fail
# or use garbage defaults.

if [ -n "${GIT_AUTHOR_NAME}" ]; then
    git config --global user.name "${GIT_AUTHOR_NAME}"
fi

if [ -n "${GIT_AUTHOR_EMAIL}" ]; then
    git config --global user.email "${GIT_AUTHOR_EMAIL}"
fi

echo "Bootstrap complete."



================================================
FILE: docker/docker-compose.full.yaml
================================================
services:
  gsd:
    build:
      context: .                        # Build context is the docker/ directory
      dockerfile: Dockerfile.sandbox    # Runtime sandbox image with entrypoint
      args:
        GSD_VERSION: latest             # Pin a specific version: GSD_VERSION=2.51.0

    container_name: gsd-sandbox

    ports:
      - "3000:3000"                     # GSD web UI

    volumes:
      - ../:/workspace                  # Project root mounted into the container
      - gsd-state:/home/gsd/.gsd        # Persistent GSD state across restarts
      # - ~/.ssh:/home/gsd/.ssh:ro      # SSH keys for git operations (read-only)
      # - ~/.gitconfig:/home/gsd/.gitconfig:ro  # Host git config

    env_file:
      - .env                            # API keys and secrets (see .env.example)

    environment:
      - NODE_ENV=development
      # UID/GID remapping — match your host user to avoid permission issues
      # on bind-mounted volumes. The entrypoint remaps the container's gsd
      # user to these IDs at startup. Run `id -u` / `id -g` to find yours.
      - PUID=1000
      - PGID=1000
      # Git identity inside the container (overrides .env if set here)
      # - GIT_AUTHOR_NAME=Your Name
      # - GIT_AUTHOR_EMAIL=you@example.com

    stdin_open: true                    # Keep stdin open for interactive use
    tty: true                           # Allocate a pseudo-TTY

    # Health check — verify GSD is installed and responsive
    healthcheck:
      test: ["CMD", "gsd", "--version"]
      interval: 30s
      timeout: 5s
      retries: 3
      start_period: 10s

    # Resource limits — uncomment to constrain container resources
    # deploy:
    #   resources:
    #     limits:
    #       cpus: "4.0"
    #       memory: 8G
    #     reservations:
    #       cpus: "1.0"
    #       memory: 2G

    # Network mode — uncomment ONE if you need host networking
    # network_mode: host               # Full host network access (no port mapping needed)
    # network_mode: bridge             # Default Docker bridge (already the default)

volumes:
  gsd-state:
    driver: local



================================================
FILE: docker/docker-compose.yaml
================================================
services:
  gsd:
    build:
      context: .
      dockerfile: Dockerfile.sandbox
      args:
        GSD_VERSION: latest
    container_name: gsd-sandbox
    ports:
      - "3000:3000"
    volumes:
      - ../:/workspace
      - gsd-state:/home/gsd/.gsd
    env_file:
      - .env
    environment:
      - NODE_ENV=development
    stdin_open: true
    tty: true

volumes:
  gsd-state:
    driver: local



================================================
FILE: docker/Dockerfile.ci-builder
================================================
# ──────────────────────────────────────────────
# CI Builder
# Image: ghcr.io/gsd-build/gsd-ci-builder
# Used by: pipeline.yml Dev stage
# ──────────────────────────────────────────────
FROM node:24-bookworm

# Rust toolchain (stable, minimal profile)
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal
ENV PATH="/root/.cargo/bin:${PATH}"

# Cross-compilation for linux-arm64
RUN apt-get update && apt-get install -y --no-install-recommends \
    gcc-aarch64-linux-gnu \
    g++-aarch64-linux-gnu \
    && rustup target add aarch64-unknown-linux-gnu \
    && rm -rf /var/lib/apt/lists/*

# Verify toolchain
RUN node --version && rustc --version && cargo --version



================================================
FILE: docker/Dockerfile.sandbox
================================================
# ──────────────────────────────────────────────
# GSD Docker Sandbox Template
# Base: docker/sandbox-templates:shell
# Purpose: Isolated environment for GSD auto mode
# Usage: docker sandbox create --template ./docker
# ──────────────────────────────────────────────
FROM node:24-bookworm-slim

# System dependencies required by GSD
RUN apt-get update && apt-get install -y --no-install-recommends \
    git \
    curl \
    ca-certificates \
    openssh-client \
    gosu \
    && rm -rf /var/lib/apt/lists/*

# Install GSD globally — version controlled via build arg
ARG GSD_VERSION=latest
RUN npm install -g gsd-pi@${GSD_VERSION}

# Create non-root user for sandbox isolation
RUN groupadd --gid 1000 gsd \
    && useradd --uid 1000 --gid gsd --shell /bin/bash --create-home gsd

# Persistent GSD state directory
RUN mkdir -p /home/gsd/.gsd && chown -R gsd:gsd /home/gsd/.gsd

# Workspace directory — synced from host via Docker sandbox
WORKDIR /workspace
RUN chown gsd:gsd /workspace

# Entrypoint handles UID/GID remapping, bootstrap, and drops to gsd user
COPY entrypoint.sh /usr/local/bin/entrypoint.sh
COPY bootstrap.sh /usr/local/bin/bootstrap.sh
RUN chmod +x /usr/local/bin/entrypoint.sh /usr/local/bin/bootstrap.sh

# Expose default GSD web UI port
EXPOSE 3000

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
CMD ["gsd", "--help"]



================================================
FILE: docker/entrypoint.sh
================================================
#!/bin/bash
set -e

# ──────────────────────────────────────────────
# GSD Container Entrypoint
#
# Responsibilities:
#   1. UID/GID remapping — match host user via PUID/PGID
#   2. Pre-create critical files — prevent Docker bind-mount
#      from creating directories where files are expected
#   3. Sentinel-based bootstrap — one-time first-boot setup
#   4. Signal forwarding — exec into the final process
# ──────────────────────────────────────────────

GSD_USER="gsd"
GSD_HOME="/home/${GSD_USER}"
GSD_DIR="${GSD_HOME}/.gsd"

# ── 1. UID/GID Remapping ────────────────────────────────
# Accept PUID/PGID from the environment so the container
# can run with the same UID/GID as the host user, avoiding
# permission headaches on bind-mounted volumes.

PUID="${PUID:-1000}"
PGID="${PGID:-1000}"

CURRENT_UID=$(id -u "${GSD_USER}")
CURRENT_GID=$(id -g "${GSD_USER}")

REMAPPED=0

if [ "${PGID}" != "${CURRENT_GID}" ]; then
    groupmod -o -g "${PGID}" "${GSD_USER}"
    REMAPPED=1
fi

if [ "${PUID}" != "${CURRENT_UID}" ]; then
    usermod -o -u "${PUID}" "${GSD_USER}"
    REMAPPED=1
fi

# Fix ownership only when UID/GID actually changed
if [ "${REMAPPED}" -eq 1 ]; then
    chown -R "${PUID}:${PGID}" "${GSD_HOME}"
    chown "${PUID}:${PGID}" /workspace
fi

# ── 2. Pre-create Critical Files ────────────────────────
# Docker bind-mounts will create a *directory* if the target
# path doesn't exist. We need these to be files, so touch
# them before Docker gets a chance to mangle things.

mkdir -p "${GSD_DIR}"

if [ ! -f "${GSD_DIR}/settings.json" ]; then
    echo '{}' > "${GSD_DIR}/settings.json"
fi

chown "${PUID}:${PGID}" "${GSD_DIR}" "${GSD_DIR}/settings.json"

# ── 3. Sentinel-based Bootstrap ─────────────────────────
# Run first-boot setup exactly once. Subsequent container
# starts (or restarts) skip this entirely.

SENTINEL="${GSD_DIR}/.bootstrapped"

if [ ! -f "${SENTINEL}" ]; then
    if [ -x /usr/local/bin/bootstrap.sh ]; then
        # Run bootstrap as the gsd user so files get correct ownership
        gosu "${GSD_USER}" /usr/local/bin/bootstrap.sh
    fi
    touch "${SENTINEL}"
    chown "${PUID}:${PGID}" "${SENTINEL}"
fi

# ── 4. Drop Privileges & Exec ──────────────────────────
# Replace this shell process with the final command running
# as the gsd user. exec + gosu = proper PID 1 = proper
# signal forwarding (SIGTERM, SIGINT, etc.).

exec gosu "${GSD_USER}" "$@"



================================================
FILE: docker/.env.example
================================================
[Binary file]


================================================
FILE: docs/README.md
================================================
# GSD Documentation

Welcome to the GSD documentation. This covers everything from getting started to advanced configuration, auto-mode internals, and extending GSD with the Pi SDK.

## User Documentation

Guides for installing, configuring, and using GSD day-to-day. Located in [`user-docs/`](./user-docs/).

Simplified Chinese translation: [`zh-CN/`](./zh-CN/).

| Guide | Description |
|-------|-------------|
| [Getting Started](./user-docs/getting-started.md) | Installation, first run, and basic usage |
| [Auto Mode](./user-docs/auto-mode.md) | How autonomous execution works — the state machine, crash recovery, and steering |
| [Commands Reference](./user-docs/commands.md) | All commands, keyboard shortcuts, and CLI flags |
| [Remote Questions](./user-docs/remote-questions.md) | Discord and Slack integration for headless auto-mode |
| [Configuration](./user-docs/configuration.md) | Preferences, model selection, git settings, and token profiles |
| [Provider Setup](./user-docs/providers.md) | Step-by-step setup for OpenRouter, Ollama, LM Studio, vLLM, and all supported providers |
| [Custom Models](./user-docs/custom-models.md) | Advanced model configuration — models.json schema, compat flags, overrides |
| [Token Optimization](./user-docs/token-optimization.md) | Token profiles, context compression, complexity routing, and adaptive learning (v2.17) |
| [Dynamic Model Routing](./user-docs/dynamic-model-routing.md) | Complexity-based model selection, cost tables, escalation, and budget pressure (v2.19) |
| [Captures & Triage](./user-docs/captures-triage.md) | Fire-and-forget thought capture during auto-mode with automated triage (v2.19) |
| [Workflow Visualizer](./user-docs/visualizer.md) | Interactive TUI overlay for progress, dependencies, metrics, and timeline (v2.19) |
| [Cost Management](./user-docs/cost-management.md) | Budget ceilings, cost tracking, projections, and enforcement modes |
| [Git Strategy](./user-docs/git-strategy.md) | Worktree isolation, branching model, and merge behavior |
| [Parallel Orchestration](./user-docs/parallel-orchestration.md) | Run multiple milestones simultaneously with worker isolation and coordination |
| [Subagents](./user-docs/subagents.md) | Delegate, inspect, and resume isolated child-agent runs |
| [Working in Teams](./user-docs/working-in-teams.md) | Unique milestone IDs, `.gitignore` setup, and shared planning artifacts |
| [Skills](./user-docs/skills.md) | Bundled skills, skill discovery, and custom skill authoring |
| [Migration from v1](./user-docs/migration.md) | Migrating `.planning` directories from the original GSD |
| [Troubleshooting](./user-docs/troubleshooting.md) | Common issues, `/gsd doctor` (real-time visibility v2.40), `/gsd forensics` (full debugger v2.40), and recovery procedures |
| [Web Interface](./user-docs/web-interface.md) | Browser-based project management with `gsd --web` (v2.41) |
| [VS Code Extension](../vscode-extension/README.md) | Chat participant, sidebar dashboard, and RPC integration for VS Code |
| [Release Notes](../CHANGELOG.md) | Current v2.81 release notes and full version history |

## Architecture & Internals

Design documents, ADRs, and internal references. Located in [`dev/`](./dev/).

| Guide | Description |
|-------|-------------|
| [Architecture Overview](./dev/architecture.md) | System design, extension model, state-on-disk, and dispatch pipeline |
| [Native Engine](../native/README.md) | Rust N-API modules for performance-critical operations |
| [ADR-001: Branchless Worktree Architecture](./dev/ADR-001-branchless-worktree-architecture.md) | Decision record for the v2.14 git architecture |
| [ADR-003: Pipeline Simplification](./dev/ADR-003-pipeline-simplification.md) | Research merged into planning, mechanical completion (v2.30) |
| [ADR-004: Capability-Aware Model Routing](./dev/ADR-004-capability-aware-model-routing.md) | Extend routing from tier/cost selection to task-capability matching |
| [ADR-007: Model Catalog Split](./dev/ADR-007-model-catalog-split.md) | Separate model metadata from routing logic for extensibility |
| [ADR-008: GSD Tools over MCP](./dev/ADR-008-gsd-tools-over-mcp-for-provider-parity.md) | Native tools over MCP for provider parity |
| [ADR-008: Implementation Plan](./dev/ADR-008-IMPLEMENTATION-PLAN.md) | Implementation plan for ADR-008 |
| [Context Optimization Opportunities](./dev/pi-context-optimization-opportunities.md) | Analysis of context window usage and optimization strategies |
| [File System Map](./dev/FILE-SYSTEM-MAP.md) | Complete file system reference |
| [CI/CD Pipeline](./dev/ci-cd-pipeline.md) | Continuous integration and deployment pipeline |
| [Frontier Techniques](./dev/FRONTIER-TECHNIQUES.md) | Advanced techniques and research |
| [PRD: Branchless Worktree](./dev/PRD-branchless-worktree-architecture.md) | Product requirements for branchless worktree architecture |
| [Agent Knowledge Index](./dev/agent-knowledge-index.md) | Index of agent knowledge resources |

## Pi SDK Documentation

Guides for the underlying Pi SDK that GSD is built on. Located in [`dev/`](./dev/).

| Guide | Description |
|-------|-------------|
| [What is Pi](./dev/what-is-pi/README.md) | Core concepts — modes, agent loop, sessions, tools, providers |
| [Extending Pi](./dev/extending-pi/README.md) | Building extensions — tools, commands, UI, events, state |
| [Context & Hooks](./dev/context-and-hooks/README.md) | Context pipeline, hook reference, inter-extension communication |
| [Pi UI / TUI](./dev/pi-ui-tui/README.md) | Terminal UI components, theming, keyboard input, rendering |

## Research

| Guide | Description |
|-------|-------------|
| [Building Coding Agents](./dev/building-coding-agents/README.md) | Research notes on agent design — decomposition, context engineering, cost/quality tradeoffs |
| [Proposals](./dev/proposals/) | Feature proposals and workflow definitions |
| [Superpowers](./dev/superpowers/) | Plans and specs for superpower features |



================================================
FILE: docs/db-map.md
================================================
# GSD-2 Database Map

> Complete schema, access layer, migration history, and cross-reference to the prompt system.

---

## 1. Database Infrastructure Stack

```
gsd_* tool call (from LLM)
       │
       ▼
bootstrap/db-tools.ts          ← tool registration + input parsing
       │
       ▼
tools/workflow-tool-executors.ts  ← business logic
       │
       ├── validation reads (milestones, slices, tasks)
       │
       ▼
gsd-db.ts  ← typed write API, transaction wrapper
       │
       ├── transaction()  (db-transaction.ts — depth counter, no nested BEGIN)
       │
       ▼
db-adapter.ts  ← normalized prepared-statement cache
       │
       ▼
db-provider.ts  ← node:sqlite (primary) or better-sqlite3 (fallback)
       │
       ▼
SQLite WAL  (.gsd/gsd.db)
       │
       ▼
After commit: regenerate markdown artifacts → write to disk → invalidate cache
```

**Connection scoping (db-connection-cache.ts):**
- Keyed by workspace `identityKey` (realpath of project root)
- Sibling worktrees share the same `.gsd/gsd.db` via SQLite WAL
- Only one connection is "active" at a time; others cached for fast re-activation
- On process exit: checkpoint WAL → vacuum → close

**Provider fallback chain:**
1. `node:sqlite` (Node ≥ 22 built-in) — preferred
2. `better-sqlite3` (npm) — fallback if node:sqlite unavailable
3. null → DB unavailable (non-fatal; GSD degrades gracefully)

---

## 2. Schema Version History

Current version: **V28**

| Version | What Changed |
|---------|-------------|
| V1 | schema_version + decisions + requirements tables |
| V2 | artifacts table |
| V3 | memories + memory_processed_units; FTS3 |
| V4 | decisions.made_by column |
| V5 | **Core hierarchy**: milestones, slices, tasks, verification_evidence |
| V6 | slices.full_summary_md, full_uat_md |
| V7 | slices.depends, demo; milestones.depends_on |
| V8 | Deep planning fields on milestones/slices/tasks; replan_history; assessments |
| V9 | sequence ordering on slices + tasks |
| V10 | slices.replan_triggered_at |
| V11 | tasks.full_plan_md; replan_history unique index |
| V12 | quality_gates table (broken DDL, fixed in V22) |
| V13 | Hot-path indexes; verification_evidence dedup index |
| V14 | slice_dependencies table |
| V15 | gate_runs, turn_git_transactions, audit_events, audit_turn_index |
| V16 | slices.is_sketch, sketch_scope (ADR-011); decisions.source |
| V17 | tasks escalation columns (blocker_source, escalation_*) |
| V18 | memory_sources; memories.scope + tags |
| V19 | memory_embeddings; memories_fts (FTS5 virtual table + triggers) |
| V20 | memory_relations |
| V21 | memories.structured_fields |
| V22 | quality_gates table repair (task_id constraint); scope column |
| V23 | milestones.sequence |
| V24 | **Auto-mode coordination**: workers, milestone_leases, unit_dispatches, cancellation_requests, command_queue |
| V25 | runtime_kv (soft state KV with global/worker/milestone scope) |
| V26 | milestone_commit_attributions |
| V27 | artifacts.content_hash (SHA-256 of full_content, computed on every insertArtifact) |
| V28 | memories.last_hit_at; incrementMemoryHitCount sets it; queryMemoriesRanked applies time-decay (1.0 → 0.7 floor over 90 days) |

---

## 3. Complete Table Inventory

### 3a. Core Hierarchy (V1, V5–V11)

#### `schema_version`
```
version    INTEGER NOT NULL
applied_at TEXT NOT NULL
```
Tracks which migrations have run.

---

#### `decisions`
```
seq            INTEGER PRIMARY KEY AUTOINCREMENT
id             TEXT NOT NULL UNIQUE
when_context   TEXT NOT NULL DEFAULT ''
scope          TEXT NOT NULL DEFAULT ''
decision       TEXT NOT NULL DEFAULT ''
choice         TEXT NOT NULL DEFAULT ''
rationale      TEXT NOT NULL DEFAULT ''
revisable      TEXT NOT NULL DEFAULT ''
made_by        TEXT NOT NULL DEFAULT 'agent'     ← V4
source         TEXT NOT NULL DEFAULT 'discussion' ← V16
superseded_by  TEXT DEFAULT NULL
```
- Index: `idx_memories_active` (superseded_by)
- View: `active_decisions` WHERE superseded_by IS NULL

---

#### `requirements`
```
id                TEXT PRIMARY KEY
class             TEXT NOT NULL DEFAULT ''
status            TEXT NOT NULL DEFAULT ''
description       TEXT NOT NULL DEFAULT ''
why               TEXT NOT NULL DEFAULT ''
source            TEXT NOT NULL DEFAULT ''
primary_owner     TEXT NOT NULL DEFAULT ''
supporting_slices TEXT NOT NULL DEFAULT ''
validation        TEXT NOT NULL DEFAULT ''
notes             TEXT NOT NULL DEFAULT ''
full_content      TEXT NOT NULL DEFAULT ''
superseded_by     TEXT DEFAULT NULL
```
- View: `active_requirements` WHERE superseded_by IS NULL

---

#### `artifacts` (V2)
```
path          TEXT PRIMARY KEY
artifact_type TEXT NOT NULL DEFAULT ''
milestone_id  TEXT DEFAULT NULL
slice_id      TEXT DEFAULT NULL
task_id       TEXT DEFAULT NULL
full_content  TEXT NOT NULL DEFAULT ''
imported_at   TEXT NOT NULL DEFAULT ''
content_hash  TEXT DEFAULT NULL                  ← V27, SHA-256 of full_content
```
Stores markdown artifact content (PROJECT, REQUIREMENTS, SUMMARY, RESEARCH, CONTEXT, etc.).
V27: `content_hash` is computed and stored on every `insertArtifact` for integrity fingerprinting.

---

#### `milestones` (V5)
```
id                      TEXT PRIMARY KEY
title                   TEXT NOT NULL DEFAULT ''
status                  TEXT NOT NULL DEFAULT 'active'
depends_on              TEXT NOT NULL DEFAULT '[]'   ← JSON array, V7
created_at              TEXT NOT NULL DEFAULT ''
completed_at            TEXT DEFAULT NULL
vision                  TEXT NOT NULL DEFAULT ''           ← V8
success_criteria        TEXT NOT NULL DEFAULT '[]'         ← V8, JSON
key_risks               TEXT NOT NULL DEFAULT '[]'         ← V8, JSON
proof_strategy          TEXT NOT NULL DEFAULT '[]'         ← V8, JSON
verification_contract   TEXT NOT NULL DEFAULT ''           ← V8
verification_integration TEXT NOT NULL DEFAULT ''          ← V8
verification_operational TEXT NOT NULL DEFAULT ''          ← V8
verification_uat        TEXT NOT NULL DEFAULT ''           ← V8
definition_of_done      TEXT NOT NULL DEFAULT '[]'         ← V8, JSON
requirement_coverage    TEXT NOT NULL DEFAULT ''           ← V8
boundary_map_markdown   TEXT NOT NULL DEFAULT ''           ← V8
sequence                INTEGER DEFAULT 0                  ← V23
```
- Index: `idx_milestones_status` (status)
- Status values: `active`, `closed`, `queued`

---

#### `slices` (V5)
```
milestone_id         TEXT NOT NULL
id                   TEXT NOT NULL
title                TEXT NOT NULL DEFAULT ''
status               TEXT NOT NULL DEFAULT 'pending'
risk                 TEXT NOT NULL DEFAULT 'medium'
depends              TEXT NOT NULL DEFAULT '[]'         ← V7, JSON
demo                 TEXT NOT NULL DEFAULT ''           ← V7
created_at           TEXT NOT NULL DEFAULT ''
completed_at         TEXT DEFAULT NULL
full_summary_md      TEXT NOT NULL DEFAULT ''           ← V6
full_uat_md          TEXT NOT NULL DEFAULT ''           ← V6
goal                 TEXT NOT NULL DEFAULT ''           ← V8
success_criteria     TEXT NOT NULL DEFAULT ''           ← V8
proof_level          TEXT NOT NULL DEFAULT ''           ← V8
integration_closure  TEXT NOT NULL DEFAULT ''           ← V8
observability_impact TEXT NOT NULL DEFAULT ''           ← V8
sequence             INTEGER DEFAULT 0                  ← V9
replan_triggered_at  TEXT DEFAULT NULL                  ← V10
is_sketch            INTEGER NOT NULL DEFAULT 0         ← V16
sketch_scope         TEXT NOT NULL DEFAULT ''           ← V16
PRIMARY KEY (milestone_id, id)
FOREIGN KEY milestone_id → milestones(id)
```
- Index: `idx_slices_active` (milestone_id, status)
- Status values: `pending`, `in_progress`, `complete`, `skipped`

---

#### `tasks` (V5)
```
milestone_id                TEXT NOT NULL
slice_id                    TEXT NOT NULL
id                          TEXT NOT NULL
title                       TEXT NOT NULL DEFAULT ''
status                      TEXT NOT NULL DEFAULT 'pending'
one_liner                   TEXT NOT NULL DEFAULT ''
narrative                   TEXT NOT NULL DEFAULT ''
verification_result         TEXT NOT NULL DEFAULT ''
duration                    TEXT NOT NULL DEFAULT ''
completed_at                TEXT DEFAULT NULL
blocker_discovered          INTEGER DEFAULT 0
blocker_source              TEXT NOT NULL DEFAULT ''           ← V17
escalation_pending          INTEGER NOT NULL DEFAULT 0         ← V17
escalation_awaiting_review  INTEGER NOT NULL DEFAULT 0         ← V17
escalation_artifact_path    TEXT DEFAULT NULL                  ← V17
escalation_override_applied_at TEXT DEFAULT NULL              ← V17
deviations                  TEXT NOT NULL DEFAULT ''
known_issues                TEXT NOT NULL DEFAULT ''
key_files                   TEXT NOT NULL DEFAULT '[]'         ← JSON
key_decisions               TEXT NOT NULL DEFAULT '[]'         ← JSON
full_summary_md             TEXT NOT NULL DEFAULT ''
description                 TEXT NOT NULL DEFAULT ''           ← V8
estimate                    TEXT NOT NULL DEFAULT ''           ← V8
files                       TEXT NOT NULL DEFAULT '[]'         ← V8, JSON
verify                      TEXT NOT NULL DEFAULT ''           ← V8
inputs                      TEXT NOT NULL DEFAULT '[]'         ← V8, JSON
expected_output             TEXT NOT NULL DEFAULT '[]'         ← V8, JSON
observability_impact        TEXT NOT NULL DEFAULT ''           ← V8
full_plan_md                TEXT NOT NULL DEFAULT ''           ← V11
sequence                    INTEGER DEFAULT 0                  ← V9
PRIMARY KEY (milestone_id, slice_id, id)
FOREIGN KEY (milestone_id, slice_id) → slices(milestone_id, id)
```
- Indexes: `idx_tasks_active` (milestone_id, slice_id, status), `idx_tasks_escalation_pending`
- Status values: `pending`, `in_progress`, `complete`, `skipped`, `blocked`

---

#### `verification_evidence` (V5)
```
id           INTEGER PRIMARY KEY AUTOINCREMENT
task_id      TEXT NOT NULL DEFAULT ''
slice_id     TEXT NOT NULL DEFAULT ''
milestone_id TEXT NOT NULL DEFAULT ''
command      TEXT NOT NULL DEFAULT ''
exit_code    INTEGER DEFAULT 0
verdict      TEXT NOT NULL DEFAULT ''
duration_ms  INTEGER DEFAULT 0
created_at   TEXT NOT NULL DEFAULT ''
FOREIGN KEY (milestone_id, slice_id, task_id) → tasks
```
- Indexes: `idx_verification_evidence_task`, unique dedup index (V13)

---

#### `replan_history` (V8)
```
id                       INTEGER PRIMARY KEY AUTOINCREMENT
milestone_id             TEXT NOT NULL
slice_id                 TEXT DEFAULT NULL
task_id                  TEXT DEFAULT NULL
summary                  TEXT NOT NULL DEFAULT ''
previous_artifact_path   TEXT DEFAULT NULL
replacement_artifact_path TEXT DEFAULT NULL
created_at               TEXT NOT NULL DEFAULT ''
FOREIGN KEY milestone_id → milestones(id)
```

---

#### `assessments` (V8)
```
path         TEXT PRIMARY KEY
milestone_id TEXT NOT NULL DEFAULT ''
slice_id     TEXT DEFAULT NULL
task_id      TEXT DEFAULT NULL
status       TEXT NOT NULL DEFAULT ''
scope        TEXT NOT NULL DEFAULT ''
full_content TEXT NOT NULL DEFAULT ''
created_at   TEXT NOT NULL DEFAULT ''
FOREIGN KEY milestone_id → milestones(id)
```

---

#### `quality_gates` (V12, repaired V22)
```
milestone_id TEXT NOT NULL
slice_id     TEXT NOT NULL
gate_id      TEXT NOT NULL
scope        TEXT NOT NULL DEFAULT 'slice'   ← V22
task_id      TEXT NOT NULL DEFAULT ''        ← V22 (was broken)
status       TEXT NOT NULL DEFAULT 'pending'
verdict      TEXT NOT NULL DEFAULT ''
rationale    TEXT NOT NULL DEFAULT ''
findings     TEXT NOT NULL DEFAULT ''
evaluated_at TEXT DEFAULT NULL
PRIMARY KEY (milestone_id, slice_id, gate_id, task_id)
FOREIGN KEY (milestone_id, slice_id) → slices
```
- Index: `idx_quality_gates_pending`

---

#### `slice_dependencies` (V14)
```
milestone_id        TEXT NOT NULL
slice_id            TEXT NOT NULL
depends_on_slice_id TEXT NOT NULL
PRIMARY KEY (milestone_id, slice_id, depends_on_slice_id)
FOREIGN KEY (milestone_id, slice_id) → slices
FOREIGN KEY (milestone_id, depends_on_slice_id) → slices
```
- Index: `idx_slice_deps_target`
- Maintained from the milestone `ROADMAP.md` slice `depends` declarations. The
  ADR-017 `roadmap-divergence` reconciliation repair re-imports the roadmap as
  the source of truth, then refreshes this junction table so dependency checks
  see the same edges as the markdown projection.

---

#### `gate_runs` (V15)
```
id            INTEGER PRIMARY KEY AUTOINCREMENT
trace_id      TEXT NOT NULL
turn_id       TEXT NOT NULL
gate_id       TEXT NOT NULL
gate_type     TEXT NOT NULL DEFAULT ''
unit_type     TEXT DEFAULT NULL
unit_id       TEXT DEFAULT NULL
milestone_id  TEXT DEFAULT NULL
slice_id      TEXT DEFAULT NULL
task_id       TEXT DEFAULT NULL
outcome       TEXT NOT NULL DEFAULT 'pass'
failure_class TEXT NOT NULL DEFAULT 'none'
rationale     TEXT NOT NULL DEFAULT ''
findings      TEXT NOT NULL DEFAULT ''
attempt       INTEGER NOT NULL DEFAULT 1
max_attempts  INTEGER NOT NULL DEFAULT 1
retryable     INTEGER NOT NULL DEFAULT 0
evaluated_at  TEXT NOT NULL DEFAULT ''
```
- Indexes: `idx_gate_runs_turn`, `idx_gate_runs_lookup`

---

#### `turn_git_transactions` (V15)
```
trace_id      TEXT NOT NULL
turn_id       TEXT NOT NULL
unit_type     TEXT DEFAULT NULL
unit_id       TEXT DEFAULT NULL
stage         TEXT NOT NULL DEFAULT 'turn-start'
action        TEXT NOT NULL DEFAULT 'status-only'
push          INTEGER NOT NULL DEFAULT 0
status        TEXT NOT NULL DEFAULT 'ok'
error         TEXT DEFAULT NULL
metadata_json TEXT NOT NULL DEFAULT '{}'
updated_at    TEXT NOT NULL DEFAULT ''
PRIMARY KEY (trace_id, turn_id, stage)
```
- Index: `idx_turn_git_tx_turn`

---

#### `audit_events` (V15)
```
event_id     TEXT PRIMARY KEY
trace_id     TEXT NOT NULL
turn_id      TEXT DEFAULT NULL
caused_by    TEXT DEFAULT NULL
category     TEXT NOT NULL
type         TEXT NOT NULL
ts           TEXT NOT NULL
payload_json TEXT NOT NULL DEFAULT '{}'
```
- Indexes: `idx_audit_events_trace`, `idx_audit_events_turn`

---

#### `audit_turn_index` (V15)
```
trace_id    TEXT NOT NULL
turn_id     TEXT NOT NULL
first_ts    TEXT NOT NULL
last_ts     TEXT NOT NULL
event_count INTEGER NOT NULL DEFAULT 0
PRIMARY KEY (trace_id, turn_id)
```

---

#### `milestone_commit_attributions` (V26)
```
commit_sha   TEXT NOT NULL
milestone_id TEXT NOT NULL
slice_id     TEXT DEFAULT NULL
task_id      TEXT DEFAULT NULL
source       TEXT NOT NULL DEFAULT 'recorded'
confidence   REAL NOT NULL DEFAULT 1.0
files_json   TEXT NOT NULL DEFAULT '[]'
created_at   TEXT NOT NULL DEFAULT ''
PRIMARY KEY (commit_sha, milestone_id)
```
- Index: `idx_milestone_commit_attr_milestone`

---

### 3b. Memory & Knowledge Layer (V3, V18–V21)

#### `memories` (V3)
```
seq               INTEGER PRIMARY KEY AUTOINCREMENT
id                TEXT NOT NULL UNIQUE
category          TEXT NOT NULL
content           TEXT NOT NULL
confidence        REAL NOT NULL DEFAULT 0.8
source_unit_type  TEXT
source_unit_id    TEXT
created_at        TEXT NOT NULL
updated_at        TEXT NOT NULL
superseded_by     TEXT DEFAULT NULL
hit_count         INTEGER NOT NULL DEFAULT 0
scope             TEXT NOT NULL DEFAULT 'project'   ← V18
tags              TEXT NOT NULL DEFAULT '[]'         ← V18, JSON
structured_fields TEXT DEFAULT NULL                  ← V21, JSON
last_hit_at       TEXT DEFAULT NULL                  ← V28, set by incrementMemoryHitCount
```
- Index: `idx_memories_active` (superseded_by), `idx_memories_scope` (scope)
- View: `active_memories` WHERE superseded_by IS NULL
- FTS: `memories_fts` virtual table (V19)
- V28: `queryMemoriesRanked` applies `memoryDecayFactor(last_hit_at)` — linear decay from 1.0 (≤0 days) to 0.7 floor (≥90 days)

---

#### `memory_processed_units` (V3)
```
unit_key     TEXT PRIMARY KEY
activity_file TEXT
processed_at TEXT NOT NULL
```

---

#### `memory_sources` (V18)
```
id           TEXT PRIMARY KEY
kind         TEXT NOT NULL
uri          TEXT
title        TEXT
content      TEXT NOT NULL
content_hash TEXT NOT NULL UNIQUE
imported_at  TEXT NOT NULL
scope        TEXT NOT NULL DEFAULT 'project'
tags         TEXT NOT NULL DEFAULT '[]'
```
- Indexes: `idx_memory_sources_kind`, `idx_memory_sources_scope`

---

#### `memory_embeddings` (V19)
```
memory_id  TEXT PRIMARY KEY
model      TEXT NOT NULL
dim        INTEGER NOT NULL
vector     BLOB NOT NULL
updated_at TEXT NOT NULL
```

---

#### `memory_relations` (V20)
```
from_id    TEXT NOT NULL
to_id      TEXT NOT NULL
rel        TEXT NOT NULL
confidence REAL NOT NULL DEFAULT 0.8
created_at TEXT NOT NULL
PRIMARY KEY (from_id, to_id, rel)
```
- Indexes: `idx_memory_relations_from`, `idx_memory_relations_to`

---

#### `memories_fts` (V19, Virtual)
```
FTS5 virtual table
Content: memories.content
Tokenizer: porter unicode61
Triggers: memories_ai, memories_ad, memories_au (keep in sync)
Fallback: LIKE scan if FTS5 unavailable
```

---

### 3c. Auto-Mode Coordination (V24)

#### `workers`
```
worker_id              TEXT PRIMARY KEY
host                   TEXT NOT NULL
pid                    INTEGER NOT NULL
started_at             TEXT NOT NULL
version                TEXT NOT NULL
last_heartbeat_at      TEXT NOT NULL
status                 TEXT NOT NULL
project_root_realpath  TEXT NOT NULL
```

---

#### `milestone_leases`
```
milestone_id   TEXT PRIMARY KEY
worker_id      TEXT NOT NULL
fencing_token  INTEGER NOT NULL
acquired_at    TEXT NOT NULL
expires_at     TEXT NOT NULL
status         TEXT NOT NULL
FOREIGN KEY worker_id → workers(worker_id)
FOREIGN KEY milestone_id → milestones(id)
```

---

#### `unit_dispatches`
```
id                      INTEGER PRIMARY KEY AUTOINCREMENT
trace_id                TEXT NOT NULL
turn_id                 TEXT
worker_id               TEXT NOT NULL
milestone_lease_token   INTEGER NOT NULL
milestone_id            TEXT NOT NULL
slice_id                TEXT
task_id                 TEXT
unit_type               TEXT NOT NULL
unit_id                 TEXT NOT NULL
status                  TEXT NOT NULL
attempt_n               INTEGER NOT NULL DEFAULT 1
started_at              TEXT NOT NULL
ended_at                TEXT
exit_reason             TEXT
error_summary           TEXT
verification_evidence_id INTEGER
next_run_at             TEXT
retry_after_ms          INTEGER
max_attempts            INTEGER NOT NULL DEFAULT 3
last_error_code         TEXT
last_error_at           TEXT
FOREIGN KEY worker_id → workers
FOREIGN KEY verification_evidence_id → verification_evidence(id)
```
- Indexes: `idx_unit_dispatches_active`, `idx_unit_dispatches_trace`
- Unique partial index: `idx_unit_dispatches_active_per_unit` ON unit_id WHERE status IN ('claimed','running') — prevents double-claim

---

#### `cancellation_requests`
```
id              INTEGER PRIMARY KEY AUTOINCREMENT
requested_at    TEXT NOT NULL
requested_by    TEXT NOT NULL
scope           TEXT NOT NULL
scope_id        TEXT NOT NULL
dispatch_id     INTEGER
reason          TEXT NOT NULL
status          TEXT NOT NULL
acked_at        TEXT
acked_worker_id TEXT
FOREIGN KEY dispatch_id → unit_dispatches(id)
FOREIGN KEY acked_worker_id → workers(worker_id)
```

---

#### `command_queue`
```
id           INTEGER PRIMARY KEY AUTOINCREMENT
target_worker TEXT     ← NULL = broadcast to all workers
command      TEXT NOT NULL
args_json    TEXT NOT NULL DEFAULT '{}'
enqueued_at  TEXT NOT NULL
claimed_at   TEXT
claimed_by   TEXT
completed_at TEXT
result_json  TEXT
```
- Index: `idx_command_queue_pending` (target_worker, claimed_at)

---

### 3d. Soft State (V25)

#### `runtime_kv`
```
scope      TEXT NOT NULL    ← 'global' | 'worker' | 'milestone'
scope_id   TEXT NOT NULL DEFAULT ''
key        TEXT NOT NULL
value_json TEXT NOT NULL
updated_at TEXT NOT NULL
PRIMARY KEY (scope, scope_id, key)
```
Non-correctness-critical state: UI cursors, dashboard caches, resume pointers. Safe to lose.

---

## 4. Entity Relationship Diagram

```
milestones ──┐
  │ id        │ (depends_on → milestones.id, via JSON)
  │           │
  ▼           │
slices ───────┘
  │ (milestone_id, id) PRIMARY KEY
  │
  ├──► slice_dependencies (milestone_id, slice_id, depends_on_slice_id)
  │
  ▼
tasks
  │ (milestone_id, slice_id, id) PRIMARY KEY
  │
  ├──► verification_evidence (milestone_id, slice_id, task_id)
  ├──► quality_gates (milestone_id, slice_id, gate_id, task_id)
  └──► unit_dispatches.task_id (via coordination layer)

milestones ──► replan_history (milestone_id)
milestones ──► assessments (milestone_id)
milestones ──► milestone_leases (milestone_id) ◄── workers
milestones ──► unit_dispatches (milestone_id) ◄── workers
milestones ──► milestone_commit_attributions (milestone_id)

memories ──► memories_fts (FTS5 virtual, via triggers)
memories ──► memory_embeddings (memory_id)
memories ──► memory_relations (from_id, to_id)
memory_sources ──► (imported content, feeds memories)

unit_dispatches ──► cancellation_requests (dispatch_id)
unit_dispatches ──► verification_evidence (verification_evidence_id)

decisions  (independent, supersedable)
requirements  (independent, supersedable)
artifacts  (independent, keyed by path)
gate_runs  (audit, keyed by trace_id + turn_id + gate_id)
turn_git_transactions  (audit, keyed by trace_id + turn_id + stage)
audit_events  (append-only audit log)
audit_turn_index  (turn-level index into audit_events)
runtime_kv  (soft state KV)
```

---

## 5. Complete gsd_* Tool → Table Map

| Tool | Tables READ | Tables WRITTEN | Disk Artifacts |
|------|------------|----------------|----------------|
| `gsd_decision_save` | memories | memories (`category = "architecture"`) | DECISIONS.md (projection) |
| `gsd_requirement_save` | requirements | requirements | REQUIREMENTS.md |
| `gsd_requirement_update` | requirements | requirements | REQUIREMENTS.md |
| `gsd_summary_save` | milestones, slices, tasks | artifacts | M##/S##/T## artifact files |
| `gsd_milestone_generate_id` | milestones | milestones (INSERT OR IGNORE, queued) | — |
| `gsd_plan_milestone` | milestones, slices | milestones, slices, tasks, replan_history | ROADMAP.md |
| `gsd_plan_slice` | slices, tasks | slices, tasks | S##-PLAN.md |
| `gsd_plan_task` | slices, tasks | tasks | T##-PLAN.md |
| `gsd_task_complete` | tasks, slices | tasks, verification_evidence | T##-SUMMARY.md; toggles checkbox in S##-PLAN.md |
| `gsd_slice_complete` | tasks, slices | slices, tasks (cascade skipped) | S##-SUMMARY.md, S##-UAT.md; toggles checkpoint in ROADMAP.md |
| `gsd_complete_milestone` | milestones, slices, tasks | milestones | M##-SUMMARY.md |
| `gsd_validate_milestone` | milestones, slices, tasks | assessments | VALIDATION.md |
| `gsd_reassess_roadmap` | milestones, slices | milestones, slices, assessments | ROADMAP.md, ASSESSMENT.md |
| `gsd_replan_slice` | slices, tasks | slices, tasks, replan_history, quality_gates | S##-PLAN.md, S##-REPLAN.md |
| `gsd_skip_slice` | slices, tasks | slices, tasks | STATE.md (via rebuildState) |
| `gsd_task_reopen` | tasks, slices, milestones | tasks | deletes T##-SUMMARY.md |
| `gsd_slice_reopen` | slices, tasks, milestones | slices, tasks | deletes S##-SUMMARY.md, UAT, all T##-SUMMARY.md |
| `gsd_milestone_reopen` | milestones, slices, tasks | milestones, slices, tasks | deletes all summaries |
| `gsd_save_gate_result` | quality_gates | quality_gates, gate_runs | — |
| `capture_thought` | memories | memories | KNOWLEDGE.md projection for Patterns/Lessons (both backfilled and newly captured) |
| `memory_query` | memories, memories_fts, memory_embeddings | memories (hit_count++) | — |

---

## 6. DB State → Dispatch Rule Mapping

`auto-dispatch.ts` reads DB state to decide which prompt to run. Here's exactly which tables each dispatch rule queries:

| Dispatch Rule | Tables Queried | Condition |
|--------------|----------------|-----------|
| `workflow-preferences` | — | PREFERENCES.md file missing |
| `discuss-project` | artifacts | PROJECT artifact absent |
| `discuss-requirements` | artifacts | REQUIREMENTS artifact absent |
| `research-decision` | runtime_kv | research-decision key absent/unresolved |
| `research-project` | artifacts, milestones | deep mode ON + RESEARCH absent |
| `discuss-milestone` | milestones, artifacts | active milestone + CONTEXT absent |
| `research-milestone` | milestones, artifacts | CONTEXT present + RESEARCH absent (if needed) |
| `plan-milestone` | milestones, slices | CONTEXT present + no slices exist |
| `parallel-research-slices` | slices, artifacts | slices exist + RESEARCH artifacts missing |
| `guided-discuss-slice` | slices, artifacts | slice CONTEXT absent |
| `plan-slice` | slices, tasks | CONTEXT present + no tasks |
| `refine-slice` | slices | `is_sketch = 1` |
| `reactive-execute` | tasks | ≥ 3 tasks WHERE status = 'pending' |
| `execute-task` | tasks | 1–2 tasks WHERE status = 'pending' |
| `gate-evaluate` | quality_gates | status = 'pending' |
| `run-uat` | slices, assessments | tasks all done + UAT absent |
| `complete-slice` | tasks, slices | all tasks complete + slice status ≠ 'complete' |
| `reassess-roadmap` | slices, milestones | slice just completed + roadmap needs update |
| `validate-milestone` | slices, milestones, assessments | all slices complete + VALIDATION absent |
| `complete-milestone` | milestones, assessments | VALIDATION present + milestone status ≠ 'closed' |

---

## 7. Write Path Invariants

1. **Single-writer rule**: all writes go through typed wrappers in `gsd-db.ts`. No raw SQL escapes to the adapter from outside this file. Enforced by structural test.

2. **Transaction wrapping**: every multi-table write uses `transaction()`. Rollback on any error. Re-entrant: nested calls increment depth counter; no nested BEGIN.

3. **Cascade semantics**:
   - `gsd_slice_complete` cascades `pending` tasks → `skipped`
   - `gsd_skip_slice` cascades `pending`/`active` tasks → `skipped`, preserves `complete`
   - `gsd_milestone_reopen` cascades all slices → `in_progress`, all tasks → `pending`

4. **Conflict guards**: `insertSlice`, `insertTask` use `ON CONFLICT` to preserve existing completed status and non-empty fields. Fresh INSERT of an already-complete row is a no-op.

5. **FTS fallback**: if FTS5 unavailable, `memory_query` falls back to LIKE scan on `memories.content`.

6. **Workspace isolation**: same `.gsd/gsd.db` for all worktrees of one project; separate `.gsd/gsd.db` per project root. Coordination tables assume single-host shared WAL. Multi-host needs external coordinator.



================================================
FILE: docs/prompt-db-combined-map.md
================================================
[Binary file]


================================================
FILE: docs/prompt-map.md
================================================
# GSD-2 Prompt System Map

> Complete dependency graph of all prompts, how they're loaded, assembled, dispatched, and how they chain into each other.

---

## 1. Pipeline Overview

```
User / gsd auto
      │
      ▼
 auto.ts  ──── reads STATE.md ──► GSDState
      │
      ▼
 auto-dispatch.ts
   DISPATCH_RULES[]  (first match wins)
      │
      ├── resolves → unitType + promptBuilder + backgroundable flag
      │
      ▼
 auto-prompts.ts
   buildXxxPrompt()
      │
      ├── loadPrompt(name, vars)          ← prompt-loader.ts (template cache)
      ├── composeInlinedContext()         ← unit-context-composer.ts
      ├── reorderForCaching()             ← prompt-ordering.ts
      └── filterSkillsByManifest()        ← skill-manifest.ts
      │
      ▼
 Pi SDK session.run(prompt)
      │
      ▼
 LLM executes → calls gsd_* tools → writes artifacts → STATE.md updated
      │
      ▼
 Loop back to auto.ts
```

---

## 2. Prompt Loading Infrastructure

| File | Role |
|------|------|
| `prompt-loader.ts` | Reads all `prompts/*.md` at startup into `templateCache`. Substitutes `{{varName}}` placeholders. Falls back to lazy read if cache misses. Preloads `templatesDir`, `taskSummaryTemplatePath`, `skillActivation` as defaults. |
| `prompt-ordering.ts` | Splits assembled prompt into `## sections`, classifies each as `static / semi-static / dynamic`, reorders to maximize LLM cache prefix stability. |
| `prompt-validation.ts` | Validates that all `{{vars}}` declared in a template have values provided before substitution fires. |
| `prompt-cache-optimizer.ts` | Tracks cache hit/miss rates per prompt; adjusts section ordering hints over time. |

**Template resolution priority** (highest wins):
1. `~/.agents/gsd/prompts/` (user-local, written by `initResources()`)
2. Module-relative `prompts/` (npm package fallback)

---

## 3. Shared Injected Variables (every prompt gets these for free)

```
{{templatesDir}}              path to templates/ dir
{{planTemplatePath}}          templates/plan.md
{{taskPlanTemplatePath}}      templates/task-plan.md
{{taskSummaryTemplatePath}}   templates/task-summary.md
{{skillActivation}}           standard skill-loading instruction block
```

---

## 4. Context Composition Stack

Every `buildXxxPrompt()` call assembles context via these layers (in order):

```
Preamble  (system.md rules, skill activation block)
    │
Static section
    ├── PROJECT.md
    ├── REQUIREMENTS.md
    └── DECISIONS.md

Semi-static section
    ├── KNOWLEDGE.md  (manual rules only — patterns/lessons stripped; ADR-013 Stage 2c)
    ├── memories      (prompt-relevant patterns, gotchas, decisions — canonical for patterns/lessons)
    ├── PREFERENCES.md
    └── Prior slice/milestone RESEARCH.md

Dynamic section
    ├── Active M##-CONTEXT.md
    ├── Active S##-PLAN.md
    ├── Active T##-PLAN.md
    ├── Task summary from prior run (resume)
    ├── Carry-forward captures
    └── Gate list to close
```

Before this map is assembled, `buildBeforeAgentStartResult()` runs the
session-start KNOWLEDGE backfill/projection path and then calls
`loadKnowledgeBlock()`. That helper inlines only manual Rules from the project
`.gsd/KNOWLEDGE.md` file; projected patterns and lessons are supplied through
the memories layer.

Budget enforcement: `context-budget.ts` computes `preambleBudgetChars`, `summaryBudgetChars`, `verificationBudgetChars` from the model's context window. Sections are truncated at markdown section boundaries, not mid-sentence.

### 4a. Tool Policy Modes

Auto-mode unit manifests declare a runtime-enforced `tools` policy. `write-gate.ts` checks the active unit before each tool call.

| Mode | Allowed surface |
|------|-----------------|
| `all` | Read, source writes, Bash, and subagents. Used by execution units that run in milestone worktrees. |
| `read-only` | Read tools only. No shell, writes, or subagents. |
| `planning` | Read tools, `.gsd/**` writes, and safe read-only Bash. No subagents. |
| `planning-dispatch` | Same as `planning`, plus subagents explicitly listed by the manifest. |
| `docs` | Same as `planning`, plus writes to configured documentation globs. No subagents. |
| `verification` | Read tools and Bash for build/test verification commands such as `npm run build`, `npm test`, `pnpm test`, `vitest`, `jest`, and `go test`; writes remain restricted to `.gsd/**`, and subagents are blocked. |

---

## 5. The 44 Prompt Files — Full Inventory

### 5a. System & Foundation

| Prompt | Purpose | Reads | Writes |
|--------|---------|-------|--------|
| `system.md` | Hard rules, isolation model, naming conventions, skills table, execution heuristics. Bundled into every prompt as preamble. | — | — |
| `heal-skill.md` | Post-unit skill drift analysis. Never edits skill files directly. | Skill activation block | `.gsd/skill-review-queue.md` |

### 5b. Project Setup Flow (runs once, sequentially)

```
guided-workflow-preferences
         │
         ▼
guided-discuss-project
         │
         ▼
guided-discuss-requirements
         │
         ▼
research-decision  (gate: deep mode opt-in)
         │
         ▼
guided-research-project  (deep mode only — 4 parallel subagents)
```

| Prompt | Purpose | Key Tools Called |
|--------|---------|-----------------|
| `guided-workflow-preferences.md` | Write `.gsd/PREFERENCES.md` with defaults; pre-seeds `research-decision.json`. No user questions. | — |
| `guided-discuss-project.md` | Interview-style project scoping. Classifies project shape (tiny/small/medium/large). | `ask_user_questions`, `gsd_summary_save(PROJECT)` |
| `guided-discuss-requirements.md` | Interview-style requirements capture. | `ask_user_questions`, `gsd_requirement_save`, `gsd_summary_save(REQUIREMENTS)` |
| `guided-research-decision.md` | Single fixed-question gate: opt into deep research or proceed lean. | `ask_user_questions` → writes `runtime/research-decision.json` |
| `guided-research-project.md` | Spawns 4 parallel scout subagents (stack, features, architecture, pitfalls). Headless. | `subagent` × 4 |

### 5c. Milestone Planning Flow

```
discuss-milestone  OR  discuss-headless  (headless = no questions)
         │
         ▼
research-milestone  (optional, based on complexity)
         │
         ▼
plan-milestone
         │
         ▼
parallel-research-slices  (all slices at once)
         │
         ▼
plan-slice  (per slice, sequential)
```

| Prompt | Purpose | Key Tools Called |
|--------|---------|-----------------|
| `discuss.md` | Interactive milestone discussion. Layered Q&A: Scope → Architecture → Error States → Quality Bar. | `ask_user_questions`, `gsd_summary_save(CONTEXT)` |
| `guided-discuss-milestone.md` | Same as discuss.md but interview-driven, with draft saves. | `ask_user_questions`, `gsd_summary_save(CONTEXT)` |
| `discuss-headless.md` | Create milestone CONTEXT from spec with no user interaction. | `gsd_plan_milestone`, `gsd_decision_save` |
| `research-milestone.md` | Strategic research before planning. Narrates findings. | `gsd_summary_save(RESEARCH)` |
| `plan-milestone.md` | Decompose milestone into slices. Plans first slice inline if single-slice. | `gsd_plan_milestone`, `gsd_decision_save` |
| `parallel-research-slices.md` | Spawn one scout subagent per slice simultaneously. Retries once on failure. | `subagent` × N |
| `plan-slice.md` | Decompose single slice into tasks. Progressive planning: sketches for S02+. | `memory_query`, `gsd_plan_slice` |
| `refine-slice.md` | Expand sketched slice plan into full task breakdown. | `gsd_plan_slice` |
| `guided-discuss-slice.md` | Interview-driven slice scoping. | `ask_user_questions`, `gsd_summary_save(CONTEXT)` |
| `guided-research-slice.md` | Scout a slice. | `memory_query`, `gsd_summary_save(RESEARCH)` |
| `research-slice.md` | Research a slice (non-guided, auto-mode). | `memory_query`, `gsd_summary_save(RESEARCH)` |

### 5d. Execution Flow

```
reactive-execute  (≥3 ready tasks → parallel)
    OR
execute-task  (single task → sequential)
         │
         ▼
guided-resume-task  (if task was interrupted)
```

| Prompt | Purpose | Key Tools Called |
|--------|---------|-----------------|
| `execute-task.md` | Execute a single task. Inlines full context stack. | `memory_query`, `gsd_task_complete` |
| `reactive-execute.md` | Dispatch all ready tasks in parallel subagents. Records failures only when no summary left. | `subagent` × N |
| `guided-resume-task.md` | Resume interrupted task. Reads `{{sliceId}}-CONTINUE.md` for continuation context. | `gsd_task_complete` |
| `quick-task.md` | Lightweight task outside milestone structure. No DB tools. | writes `{{summaryPath}}` directly |

### 5e. Quality Gates

```
gate-evaluate  (parallel gate subagents)
         │
         ▼
validate-milestone  (3 parallel reviewers)
         │
         ▼
run-uat  (user acceptance tests)
```

| Prompt | Purpose | Key Tools Called |
|--------|---------|-----------------|
| `gate-evaluate.md` | Spawn one subagent per quality gate in parallel. Verifies `gsd_save_gate_result` called. | `subagent` × N |
| `validate-milestone.md` | 3 parallel reviewers: (A) requirements, (B) integration, (C) acceptance. | `subagent` × 3, `gsd_validate_milestone` |
| `run-uat.md` | Execute UAT. Modes: artifact-driven, runtime, browser, human-experience. Runs under `verification` tools policy, so Bash is limited to read-only inspection and build/test verification commands. | `gsd_summary_save(ASSESSMENT)`, verification Bash |

`run-uat` completion verification requires a canonical verdict in the written `S##-ASSESSMENT.md` (for example `verdict: PASS | FAIL | PARTIAL`). A pre-existing assessment file without `verdict` does not satisfy artifact verification.

### 5f. Completion Flow

```
complete-slice
         │
         ▼
reassess-roadmap  (after each slice)
         │
         ▼
complete-milestone
```

| Prompt | Purpose | Key Tools Called |
|--------|---------|-----------------|
| `complete-slice.md` | Close slice after tasks pass. Compress summary. | `gsd_slice_complete`, `gsd_requirement_update` |
| `reassess-roadmap.md` | Review roadmap post-slice. Validates success-criterion coverage. | `gsd_reassess_roadmap`, `gsd_requirement_update` |
| `complete-milestone.md` | Close milestone. Persist to DB. | `gsd_complete_milestone`, `gsd_requirement_update`, `capture_thought` |

### 5g. Maintenance & Repair

| Prompt | Purpose | Key Tools Called |
|--------|---------|-----------------|
| `replan-slice.md` | Replan after blocker discovered mid-slice. Preserves completed tasks. | `gsd_replan_slice` |
| `rethink.md` | Reorder, park, unpark, skip, or discard milestones. | `gsd_skip_slice`, writes `QUEUE-ORDER.json` |
| `reassess-roadmap.md` | *(see Completion Flow above)* | — |
| `rewrite-docs.md` | Apply OVERRIDES.md changes across all planning docs. | — |
| `review-migration.md` | Audit `.planning → .gsd` migration correctness. | `deriveState` |
| `doctor-heal.md` | Repair broken GSD artifacts (summaries, UAT, CONTEXT). | — |
| `scan.md` | Codebase scan → STACK.md, INTEGRATIONS.md, ARCHITECTURE.md. No tool calls. | writes `{{outputDir}}` |
| `forensics.md` | Debug GSD engine failures. Map failures to source files. | reads activity logs, journal, metrics |
| `debug-diagnose.md` | Root-cause analysis for reported bugs. | `capture_thought`, `memory_query` |
| `debug-session-manager.md` | Manage debug session with checkpoint protocol. Structured return headers. | — |
| `add-tests.md` | Generate tests for completed slices. | skill activation |
| `triage-captures.md` | Classify user thoughts captured with `capture_thought`. | `ask_user_questions`, updates `CAPTURES.md` |
| `queue.md` | Add future milestones to queue. | `gsd_milestone_generate_id`, `gsd_summary_save(CONTEXT)`, updates `QUEUE.md` |

### 5h. Workflow Execution (one-off workflows, not milestone-driven)

| Prompt | Purpose | Notes |
|--------|---------|-------|
| `workflow-start.md` | Execute a templated workflow (phases, complexity gates, artifact directory). | Follows phases in order, writes artifacts, atomic commits |
| `workflow-oneshot.md` | Execute a oneshot workflow (no STATE.json). | prompt-only, no scaffolding |

---

## 6. Full Dependency Graph

### 6a. Sequential Chains

```
STATE.md
  └─► auto.ts
        └─► auto-dispatch.ts (DISPATCH_RULES, first match)
              │
              ├── [setup] guided-workflow-preferences
              │              │ writes PREFERENCES.md
              │              │
              ├── [setup] guided-discuss-project
              │              │ writes PROJECT.md
              │              │
              ├── [setup] guided-discuss-requirements
              │              │ writes REQUIREMENTS.md
              │              │
              ├── [gate]  guided-research-decision
              │              │ writes research-decision.json
              │              │
              ├── [deep]  guided-research-project ──► 4× subagent
              │              │ writes RESEARCH artifacts
              │              │
              ├── [ms]    discuss / guided-discuss-milestone / discuss-headless
              │              │ writes M##-CONTEXT.md
              │              │
              ├── [ms]    research-milestone
              │              │ writes M##-RESEARCH.md
              │              │
              ├── [ms]    plan-milestone
              │              │ writes M##-ROADMAP.md + S##-PLAN sketches
              │              │
              ├── [sl]    parallel-research-slices ──► N× subagent (research-slice)
              │              │ writes S##-RESEARCH.md
              │              │
              ├── [sl]    guided-discuss-slice
              │              │ writes S##-CONTEXT.md
              │              │
              ├── [sl]    plan-slice / refine-slice
              │              │ writes S##-PLAN.md + T##-PLAN.md
              │              │
              ├── [task]  reactive-execute ──────────► N× subagent (execute-task)
              │    OR                                     │ writes T##-SUMMARY.md
              ├── [task]  execute-task                    │
              │              │ reads T##-PLAN.md, S##-PLAN.md excerpt
              │              │ writes T##-SUMMARY.md
              │              │
              ├── [gate]  gate-evaluate ────────────► N× subagent
              │              │ writes gate results
              │              │
              ├── [sl]    run-uat
              │              │ writes S##-ASSESSMENT.md
              │              │
              ├── [sl]    complete-slice
              │              │ writes S##-SUMMARY.md
              │              │
              ├── [ms]    reassess-roadmap
              │              │ updates M##-ROADMAP.md
              │              │
              ├── [ms]    validate-milestone ────────► 3× subagent
              │              │ writes validation verdict
              │              │
              └── [ms]    complete-milestone
                             │ writes M##-SUMMARY.md
                             └─► loop back to next milestone
```

### 6b. Parallel Dispatch Map

| Orchestrator Prompt | Subagents Spawned | How Many |
|--------------------|-------------------|---------|
| `guided-research-project.md` | stack scout, features scout, architecture scout, pitfalls scout | 4 (fixed) |
| `parallel-research-slices.md` | `research-slice` (one per slice) | N slices |
| `reactive-execute.md` | `execute-task` (one per ready task) | N ready tasks |
| `gate-evaluate.md` | one gate evaluator per gate | N gates |
| `validate-milestone.md` | reviewer-A (requirements), reviewer-B (integration), reviewer-C (acceptance) | 3 (fixed) |

### 6c. Recovery / Detour Chains

```
execute-task  ──[interrupted]──► guided-resume-task
                                    reads {{sliceId}}-CONTINUE.md

execute-task  ──[blocker]──────► replan-slice
                                    rewrites incomplete tasks only

plan-milestone ──[any]─────────► rethink
                                    reorders / parks / discards milestones

auto.ts ────────[drift]────────► heal-skill
                                    writes skill-review-queue.md

auto.ts ────────[doctor]───────► doctor-heal
                                    repairs CONTEXT, UAT, SUMMARY artifacts

any prompt ─────[failure]──────► forensics / debug-diagnose / debug-session-manager
```

---

## 7. Artifact Flow (What Each Phase Writes)

```
Phase                   Artifact Written
─────────────────────────────────────────────────────
guided-workflow-preferences  →  .gsd/PREFERENCES.md
guided-discuss-project       →  .gsd/PROJECT.md
guided-discuss-requirements  →  .gsd/REQUIREMENTS.md
guided-research-decision     →  .gsd/runtime/research-decision.json
guided-research-project      →  .gsd/milestones/M##/M##-RESEARCH.md (×4 aspects)

discuss / guided-discuss-milestone  →  .gsd/milestones/M##/M##-CONTEXT.md
research-milestone           →  .gsd/milestones/M##/M##-RESEARCH.md
plan-milestone               →  .gsd/milestones/M##/M##-ROADMAP.md
                                 .gsd/milestones/M##/slices/S##/S##-PLAN.md (sketches)

research-slice               →  .gsd/milestones/M##/slices/S##/S##-RESEARCH.md
guided-discuss-slice         →  .gsd/milestones/M##/slices/S##/S##-CONTEXT.md
plan-slice / refine-slice    →  .gsd/milestones/M##/slices/S##/S##-PLAN.md
                                 .gsd/milestones/M##/slices/S##/tasks/T##-PLAN.md

execute-task                 →  .gsd/milestones/M##/slices/S##/tasks/T##-SUMMARY.md
gate-evaluate                →  gate results (DB + artifact)
run-uat                      →  .gsd/milestones/M##/slices/S##/S##-ASSESSMENT.md
complete-slice               →  .gsd/milestones/M##/slices/S##/S##-SUMMARY.md
reassess-roadmap             →  updates M##-ROADMAP.md (slice statuses)
validate-milestone           →  validation verdict (DB)
complete-milestone           →  .gsd/milestones/M##/M##-SUMMARY.md

triage-captures              →  .gsd/CAPTURES.md (classification metadata)
queue                        →  .gsd/QUEUE.md, updates PROJECT.md
scan                         →  {{outputDir}}/STACK.md, INTEGRATIONS.md, ARCHITECTURE.md
rewrite-docs                 →  DECISIONS.md, task plans, REQUIREMENTS.md, PROJECT.md
```

---

## 8. Skill System Dependency

```
skill-catalog.ts   (tech-stack → repo + skill names)
       │
       ▼
skill-discovery.ts (resolves installed skills for current project)
       │
       ▼
skill-manifest.ts  (allowlist per unit type)
       │             e.g. plan-milestone → [decompose-into-slices, api-design, tdd, ...]
       │             e.g. execute-task   → wildcard (all skills eligible)
       ▼
{{skillActivation}} placeholder in every prompt
       │
       ▼
LLM sees: "load these skill files and follow their rules for this unit"
```

---

## 9. Tool → DB Write Map

| Tool | Persists To |
|------|------------|
| `gsd_plan_milestone` | milestones table, slices table |
| `gsd_plan_slice` | slices table, tasks table |
| `gsd_task_complete` | tasks table, T##-SUMMARY.md |
| `gsd_slice_complete` | slices table, S##-SUMMARY.md |
| `gsd_complete_milestone` | milestones table, M##-SUMMARY.md |
| `gsd_validate_milestone` | milestones table (validation verdict) |
| `gsd_reassess_roadmap` | slices table (reorder, add, remove) |
| `gsd_replan_slice` | tasks table (replace incomplete tasks) |
| `gsd_skip_slice` | slices table (status = skipped) |
| `gsd_requirement_save` | requirements table |
| `gsd_requirement_update` | requirements table |
| `gsd_summary_save` | artifact files + DB reference |
| `gsd_decision_save` | memories table (`architecture` rows) + DECISIONS.md projection |
| `capture_thought` | memories table; KNOWLEDGE.md projection for Patterns/Lessons |
| `memory_query` | READ — queries memories / memory indexes |
| `ask_user_questions` | blocks until user responds; no DB write |
| `subagent` | spins up child Pi session with given prompt |

---

## 10. Dispatch Rule Priority Order

`auto-dispatch.ts` evaluates 29 rules top-to-bottom, first match wins. Source of
truth is the `DISPATCH_RULES` array in `auto-dispatch.ts`; the canary test
`tests/dispatch-rule-coverage.test.ts` pins the count at 29.

```
Priority  Rule                                          Fires When
────────  ────────────────────────────────────────────  ─────────────────────────
 1        escalating-task → pause-for-escalation        a task escalation is awaiting user review
 2        rewrite-docs (override gate)                  OVERRIDES.md present and unprocessed
 3        execution-entry phase (no context) → discuss  re-entry into a milestone with no CONTEXT
 4        summarizing → complete-slice                  slice in 'summarizing' phase
 5        run-uat (post-completion)                     tasks done, UAT pending
 6        uat-verdict-gate (non-PASS blocks)            UAT non-PASS — block until resolved
 7        reassess-roadmap (post-completion)            slice closed, roadmap needs update
 8        needs-discussion → discuss-milestone          milestone explicitly flagged for discussion
 9        deep: workflow-preferences                    deep mode + PREFERENCES.md missing
10        deep: discuss-project                         deep mode + PROJECT artifact missing
11        deep: discuss-requirements                    deep mode + REQUIREMENTS missing
12        deep: research-decision                       deep mode + research decision not made
13        deep: research-project                        deep mode + research approved, files missing
14        pre-planning (no context) → discuss-milestone active milestone, CONTEXT missing
15        pre-planning (no research) → research-mile…   CONTEXT done, RESEARCH missing
16        pre-planning (has research) → plan-milestone  CONTEXT + RESEARCH done, ROADMAP missing
17        planning (require_slice_discussion) → pause   slice flagged for discussion (#3454)
18        planning (multi slices need research) → par…  ROADMAP done, slice RESEARCH missing × ≥2
19        planning (no research, not S01) → research…   single slice needs RESEARCH
20        refining → refine-slice                       slice is sketch, needs expansion
21        planning → plan-slice                         slice CONTEXT done, PLAN missing
22        evaluating-gates → gate-evaluate              gates pending evaluation
23        replanning-slice → replan-slice               slice in 'replanning' phase
24        executing → reactive-execute (parallel)       ≥3 tasks ready (parallel mode)
25        executing → execute-task (recover plan)       task plan missing — recover via plan-slice
26        executing → execute-task                      1–2 tasks ready (sequential mode)
27        validating-milestone → validate-milestone     all slices closed, not yet validated
28        completing-milestone → complete-milestone     validated, not yet completed
29        complete → stop                               nothing left to do
```

---

## 11. How to Read the Map

- **Box** = a prompt file (`prompts/X.md`)
- **Arrow →** = "produces" or "writes"
- **Dashed →** = "reads from" 
- **×N** = spawns N parallel subagents each running that prompt
- **[gate]** = requires explicit user confirmation before proceeding
- **DB** = persists to `gsd.db` via a `gsd_*` tool call
- **Headless** = no `ask_user_questions` calls; autonomous judgment



================================================
FILE: docs/token-consumption-savings-evidence.md
================================================
<!-- Project/App: GSD-2 -->
<!-- File Purpose: Token consumption savings PR evidence summary. -->

# Token Consumption Savings Evidence

This PR uses `PI_TOKEN_AUDIT=1` audit output to target repeat prompt cost at the final provider boundary without logging raw prompt, system, tool, tool-result, or user content.

## Measured Baselines

| Log | Rows | Estimated input tokens avg | Tool count avg | Tool schema chars avg | Custom chars avg | Notes |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| `/tmp/gsd-token-audit-min-tools.log` | n/a | 45,504 | 44 | 43,971 | 55,657 | Initial minimal-tools sample still had high custom/context cost. |
| `/tmp/gsd-token-audit-min-tools-context-cap.log` | 14 | n/a | 138 | 137,296 | 20,210 | Context cap helped custom payloads, but full tool schema still dominated. |
| `/tmp/gsd-token-audit-auto-tools-before-start2.log` | 12 | n/a | 105 | 80,272 | 25,025 | Auto still carried broad workflow/tool surfaces. |
| `/tmp/gsd-token-audit-auto-tools-before-start3.log` | 7 | n/a | 15 | 17,224 | 22,026 | Strict auto scoping brought normal auto turns near the target surface. |
| `/tmp/gsd-token-audit-auto-tools-before-start4.log` | 4 | n/a | 15 | 16,313 | 38,707 | Auto tool schema stayed low; custom closeout context still spiked. |

Run 5 exposed the remaining workflow bucket:

| Surface | Rows | Estimated input tokens avg | Tool count avg | Tool schema chars avg | Custom chars avg |
| --- | ---: | ---: | ---: | ---: | ---: |
| `gsd-auto` | 5 | 20,775 | 15 | 16,818 | 16,499 |
| `gsd-run` | 7 | 73,367 | 102 | 76,520 | 104,122 |
| `gsd-doctor-heal` | 1 | 46,309 | 103 | 76,953 | 29,468 |

`toolResultChars` stayed at zero in the sampled logs, so tool-result replay compression remains deferred.

## Savings Implemented

- Final provider-boundary audit: `PI_TOKEN_AUDIT=1` reports metadata-only section sizes after context transforms and final tool filtering.
- UI semantics: session token/cost totals stay visible, but they are not presented as live context percentage.
- Provider request-time filtering: provider compatibility is applied immediately before send, then GSD workflow/auto scoping narrows the remaining compatible tools.
- GSD auto/workflow tool scoping: auto units, guided workflow dispatch, `gsd-run`, and `gsd-doctor-heal` use scoped tool surfaces with a full-tools escape hatch.
- Prompt/context cuts: hidden GSD context defaults to a cap, `KNOWLEDGE.md` defaults to a 12k cap, auto preambles stay bounded, and malformed summaries no longer inline unbounded legacy bodies.
- Workflow/doctor cuts: workflow dispatch uses a capped section-aware protocol excerpt, and doctor heal sends summary plus top actionable issues instead of duplicating the full report.
- Skill prompt visibility: GSD narrows `<available_skills>` per unit using the existing `skillFilter` prompt seam while keeping skills loaded and invocable.

## Local Verification

Targeted commands run in `.worktrees/token-consumption-savings-pr`:

```bash
node --import ./src/resources/extensions/gsd/tests/resolve-ts.mjs --experimental-strip-types --test src/resources/extensions/gsd/tests/token-tool-gating.test.ts src/resources/extensions/gsd/tests/system-context-memory.test.ts src/resources/extensions/gsd/tests/prompt-duplication-cuts.test.ts src/resources/extensions/gsd/tests/complete-milestone-excerpt.test.ts src/resources/extensions/gsd/tests/knowledge.test.ts src/resources/extensions/gsd/tests/workflow-protocol-excerpt.test.ts
node --import ./src/resources/extensions/gsd/tests/resolve-ts.mjs --experimental-strip-types --test vscode-extension/test/rpc-display-contract.test.ts
node --import ./src/resources/extensions/gsd/tests/resolve-ts.mjs --experimental-strip-types --test packages/pi-agent-core/src/token-audit.test.ts packages/pi-coding-agent/src/core/skill-tool.test.ts packages/pi-coding-agent/src/core/sdk-tool-filter.test.ts
npm run test:compile
node --test dist-test/packages/pi-agent-core/src/token-audit.test.js dist-test/packages/pi-agent-core/src/agent-loop.test.js
npm run typecheck:extensions
```



================================================
FILE: docs/adr/.gitkeep
================================================
[Empty file]


================================================
FILE: docs/agents/domain.md
================================================
# Domain Docs

How the engineering skills should consume this repo's domain documentation when exploring the codebase.

## Before exploring, read these

- **`CONTEXT.md`** at the repo root.
- **`docs/adr/`** — read ADRs that touch the area you're about to work in.

If any of these files don't exist, **proceed silently**. Don't flag their absence; don't suggest creating them upfront. The producer skill (`/grill-with-docs`) creates them lazily when terms or decisions actually get resolved.

## File structure

Single-context layout (this repo):

```text
/
├── CONTEXT.md
├── docs/adr/
│   ├── 0001-...md
│   └── 0002-...md
└── src/
```

## Use the glossary's vocabulary

When your output names a domain concept (in an issue title, a refactor proposal, a hypothesis, a test name), use the term as defined in `CONTEXT.md`. Don't drift to synonyms the glossary explicitly avoids.

If the concept you need isn't in the glossary yet, that's a signal — either you're inventing language the project doesn't use (reconsider) or there's a real gap (note it for `/grill-with-docs`).

## Flag ADR conflicts

If your output contradicts an existing ADR, surface it explicitly rather than silently overriding:

> _Contradicts ADR-0007 (event-sourced orders) — but worth reopening because…_



================================================
FILE: docs/agents/issue-tracker.md
================================================
# Issue tracker: GitHub

Issues and PRDs for this repo live as GitHub issues on **`gsd-build/gsd-2`** (the `upstream` remote). Use the `gh` CLI for all operations.

This clone may have multiple remotes (`origin` is a personal fork, `upstream` is the canonical repo). Always pass `-R gsd-build/gsd-2` so commands hit the canonical tracker rather than auto-resolving to the fork.

## Conventions

- **Create an issue**: `gh issue create -R gsd-build/gsd-2 --title "..." --body "..."`. Use a heredoc for multi-line bodies.
- **Read an issue**: `gh issue view <number> -R gsd-build/gsd-2 --json number,title,body,labels,comments --jq '{number, title, body, labels: [.labels[].name], comments: [.comments[].body]}'`.
- **List issues**: `gh issue list -R gsd-build/gsd-2 --state open --json number,title,body,labels,comments --jq '[.[] | {number, title, body, labels: [.labels[].name], comments: [.comments[].body]}]'` with appropriate `--label` and `--state` filters.
- **Comment on an issue**: `gh issue comment <number> -R gsd-build/gsd-2 --body "..."`
- **Apply / remove labels**: `gh issue edit <number> -R gsd-build/gsd-2 --add-label "..."` / `--remove-label "..."`
- **Close**: `gh issue close <number> -R gsd-build/gsd-2 --comment "..."`

## When a skill says "publish to the issue tracker"

Create a GitHub issue on `gsd-build/gsd-2`.

## When a skill says "fetch the relevant ticket"

Run `gh issue view <number> -R gsd-build/gsd-2 --comments`.



================================================
FILE: docs/agents/triage-labels.md
================================================
# Triage Labels

The skills speak in terms of five canonical triage roles. This file maps those roles to the actual label strings used in this repo's issue tracker.

| Label in mattpocock/skills | Label in our tracker | Meaning                                  |
| -------------------------- | -------------------- | ---------------------------------------- |
| `needs-triage`             | `needs-triage`       | Maintainer needs to evaluate this issue  |
| `needs-info`               | `needs-info`         | Waiting on reporter for more information |
| `ready-for-agent`          | `ready-for-agent`    | Fully specified, ready for an AFK agent  |
| `ready-for-human`          | `ready-for-human`    | Requires human implementation            |
| `wontfix`                  | `wontfix`            | Will not be actioned                     |

When a skill mentions a role (e.g. "apply the AFK-ready triage label"), use the corresponding label string from this table.

`needs-triage` and `needs-info` already exist on `gsd-build/gsd-2`. The other three (`ready-for-agent`, `ready-for-human`, `wontfix`) will be created on first use:

```bash
gh label create ready-for-agent -R gsd-build/gsd-2 --description "Fully specified, ready for an AFK agent" --color 0E8A16
gh label create ready-for-human -R gsd-build/gsd-2 --description "Requires human implementation" --color 1D76DB
gh label create wontfix         -R gsd-build/gsd-2 --description "Will not be actioned"          --color CCCCCC
```

Edit the right-hand column to match whatever vocabulary you actually use.



================================================
FILE: docs/dev/2026-04-24-swarm-delivery-implementation-plan.md
================================================
# Swarm Delivery Implementation Plan for UOK Hardening

**Status:** In progress  
**Date:** 2026-04-24  
**Source spec:** https://raw.githubusercontent.com/jeremymcs/gsd-2/2540b00211c61daa0574bd3419afec5ceef36ba4/docs/dev/specs/2026-04-24-swarm-delivery-plan-uok.md  
**Related:** `docs/dev/ADR-009-orchestration-kernel-refactor.md`, `docs/dev/ADR-009-IMPLEMENTATION-PLAN.md`, `docs/dev/proposals/rfc-gitops-branching-strategy.md`

## Objective

Turn the swarm delivery spec into an executable hardening plan for the current UOK baseline. ADR-009 already marks the Unified Orchestration Kernel as implemented and default-on, so this plan does not reopen the whole migration. It focuses on the remaining operational guarantees needed for parallel lane delivery:

1. One scheduler contract and explainable dispatch decisions.
2. Explicit, replayable state transitions.
3. Deterministic single-writer coordination.
4. UOK release proof packs and emergency fallback discipline.
5. GitHub routines that let lanes move quickly without lowering safety.

## Non-Goals

- Replacing the current UOK modules with a new architecture.
- Rewriting CLI, web, MCP, or package entrypoints.
- Changing model/provider auth or compliance behavior outside the policy gate.
- Enforcing organization-level GitHub branch protection through this repository's own `.github/` files. GitHub routines in this plan belong to GSD's product code and generated workflow surfaces.

## Current Baseline

The repo already has the main UOK spine:

- `src/resources/extensions/gsd/uok/contracts.ts` - turn, gate, audit, graph contracts.
- `src/resources/extensions/gsd/uok/kernel.ts` - UOK entrypoint and emergency legacy fallback routing.
- `src/resources/extensions/gsd/uok/gate-runner.ts` - unified gate runner and retry matrix.
- `src/resources/extensions/gsd/uok/execution-graph.ts` - shared graph scheduler primitives.
- `src/resources/extensions/gsd/uok/gitops.ts` - turn git transaction projection.
- `src/resources/extensions/gsd/uok/audit.ts` - append-only audit envelope.
- `src/resources/extensions/gsd/uok/plan-v2.ts` - compile-to-unit-graph path.
- `src/resources/extensions/github-sync/*` - product-owned GitHub issue, PR, milestone, and generated routine surface.

The swarm work should harden the edges around these modules, add missing source-of-truth artifacts, and prove the behavior with targeted tests and release reports.

## Execution Progress

- 2026-04-24: Landed the contract-freeze foundation:
  - dispatch reason/envelope contract and formatter
  - execution graph snapshot helper
  - checked-in state transition matrix and validation tests
  - UOK writer token and monotonic sequence helper
  - turn observer writer-sequence wiring for audit/gitops metadata
  - parity report helper
  - GitHub Sync swarm PR body and release checklist routine scaffolding

## Delivery Topology

Use one hub branch and five lane branches:

| Role | Branch | Primary ownership |
| --- | --- | --- |
| Hub integrator | `integration/uok-swarm` | merge arbitration, final proof pack, release candidate |
| Workflow lane | `lane/workflow-engine` | scheduler contract, execution graph plumbing, dispatch explainability |
| State lane | `lane/state-machine` | transition matrix, guards, replay and crash recovery |
| Writer lane | `lane/single-writer` | write token, write adapter, sequence continuity |
| UOK lane | `lane/uok-control-planes` | flags, fallback controls, parity report, control plane boundaries |
| GitHub lane | `lane/github-routines` | generated labels, PR body routines, check summaries, release checklist |

Only the hub merges lane PRs into `integration/uok-swarm`. Lane PRs target the integration branch, not `main`.

## Global Guardrails

- Contract-first: each lane lands typed contracts, source artifacts, and tests before broad implementation rewires.
- Fail-closed: ambiguous dispatch, transition, write, or parity state stops in a blocked/manual-attention path.
- Traceable by default: every dispatch and write must include `traceId`, `turnId`, `unitType`, `unitId`, timestamp, source commit when available, and a reason code.
- Single writer of record: only the hub merges integration state. Runtime writes flow through one adapter or explicitly documented compatibility shim.
- Small PRs: lane work should land in slices that can be reviewed independently.

## Phase 0: Integration Setup

**Goal:** Create the shared branch, baseline checks, and lane ownership before feature work starts.

### Tasks

1. Create `integration/uok-swarm` from current `main`.
2. Open five tracking issues, one per lane, each linking this plan and the source spec.
3. Confirm CI on the integration branch runs the same required checks as `main`.
4. Add temporary labels:
   - `lane/workflow`
   - `lane/state`
   - `lane/writer`
   - `lane/uok`
   - `lane/github`
   - `uok-swarm`
5. Add an integration status comment template for daily hub updates:
   - lane branch
   - latest commit
   - changed contracts
   - test evidence
   - blockers

### Exit Criteria

- All lane issues exist.
- All lanes know their write scopes.
- The hub can produce a daily integration status without manual archaeology.

### Verification

- `git fetch origin`
- `git branch --contains origin/integration/uok-swarm`
- GitHub label list includes the lane labels.
- CI is green or failures are documented with owners.

## Phase 1: Contract Freeze

**Goal:** Freeze the shared contracts that all lanes will depend on.

### 1A. Workflow Engine Contract

Primary files:

- `src/resources/extensions/gsd/uok/contracts.ts`
- `src/resources/extensions/gsd/uok/execution-graph.ts`
- `src/resources/extensions/gsd/workflow-engine.ts`
- `src/resources/extensions/gsd/engine-types.ts`
- `src/resources/extensions/gsd/auto/loop.ts`

Deliverables:

- Add a normalized dispatch envelope that covers `unit`, `hook`, `subagent`, `team-worker`, `verification`, `reprocess`, and `refine`.
- Add dispatch reason codes:
  - `policy`
  - `state`
  - `recovery`
  - `manual`
  - `dependency`
  - `conflict`
  - `retry`
- Add a query-facing explanation object so `/gsd query` can answer why a unit ran or why it is blocked.

Tests:

- Extend `src/resources/extensions/gsd/tests/uok-contracts.test.ts`.
- Extend `src/resources/extensions/gsd/tests/uok-execution-graph.test.ts`.
- Add or extend a query/explainability test near `src/resources/extensions/gsd/tests/active-milestone-id-guard.test.ts` or `src/resources/extensions/gsd/tests/dev-engine-wrapper.test.ts`.

### 1B. State Transition Matrix

Primary files:

- `src/resources/extensions/gsd/state.ts`
- `src/resources/extensions/gsd/auto/phases.ts`
- `src/resources/extensions/gsd/tools/*.ts`
- new source artifact under `src/resources/extensions/gsd/state-transition-matrix.*`

Deliverables:

- Add a checked-in transition matrix with:
  - `from`
  - `event`
  - `guard`
  - `to`
  - `onFail`
  - `reasonCode`
- Add a validator that fails if hot-path transitions are not represented in the matrix.
- Map known recovery paths into explicit events instead of hidden side effects.

Tests:

- Extend `src/resources/extensions/gsd/tests/state-machine-full-walkthrough.test.ts`.
- Extend `src/resources/extensions/gsd/tests/state-derivation-parity.test.ts`.
- Add a matrix coverage test.

### 1C. Single Writer Contract

Primary files:

- `src/resources/extensions/gsd/uok/contracts.ts`
- `src/resources/extensions/gsd/uok/audit.ts`
- `src/resources/extensions/gsd/uok/gitops.ts`
- `src/resources/extensions/gsd/bootstrap/write-gate.ts`
- `src/resources/extensions/gsd/sync-lock.ts`
- `src/resources/extensions/gsd/atomic-write.ts`

Deliverables:

- Define `WriterToken`, `WriteSequence`, and `WriteRecord`.
- Define one active writer token per turn.
- Define monotonic per-turn sequence ids used by state, audit, and gitops writes.
- Document which legacy writes remain outside the adapter and why.

Tests:

- Extend `src/resources/extensions/gsd/tests/write-gate.test.ts`.
- Extend `src/resources/extensions/gsd/tests/uok-audit-unified.test.ts`.
- Extend `src/resources/extensions/gsd/tests/uok-gitops-turn-action.test.ts`.
- Add a concurrent write rejection or deterministic queue test.

### Phase 1 Exit Criteria

- Contracts compile.
- Matrix and writer contracts are checked in.
- No lane needs to invent its own dispatch, transition, or write metadata shape.

### Verification

- `npm run typecheck:extensions`
- Targeted UOK and state tests:
  - `node --import ./src/resources/extensions/gsd/tests/resolve-ts.mjs --experimental-strip-types --test src/resources/extensions/gsd/tests/uok-contracts.test.ts src/resources/extensions/gsd/tests/uok-execution-graph.test.ts src/resources/extensions/gsd/tests/state-machine-full-walkthrough.test.ts src/resources/extensions/gsd/tests/write-gate.test.ts`

## Phase 2: Lane Implementation

**Goal:** Implement lane-owned behavior behind stable contracts.

### Lane A: Workflow Engine

Implement:

- `resolveDispatch` returns the normalized dispatch envelope.
- Execution graph snapshots are persisted before and after each dispatched unit.
- Dispatch explanation is exposed through the existing query/status surface.
- Recovery dispatch uses reason code `recovery` and includes the prior failed turn id.

Done criteria:

- One contract function chooses the scheduler path.
- Every execution kind has unit coverage.
- Query output can explain "why this unit ran now" and "why this unit is blocked."

Suggested tests:

- `src/resources/extensions/gsd/tests/custom-workflow-engine.test.ts`
- `src/resources/extensions/gsd/tests/workflow-dispatch.test.ts`
- `src/resources/extensions/gsd/tests/uok-execution-graph.test.ts`
- `src/resources/extensions/gsd/tests/dispatch-missing-task-plans.test.ts`

### Lane B: State Machine

Implement:

- Transition matrix is used by hot-path completion and recovery tools.
- Repeated `query`, `next`, interrupted `auto`, and crash recovery are idempotent.
- Hidden transition side effects are either removed or wrapped in explicit matrix events.
- Failed guards route to blocked/reassess/manual-attention states, never implicit success.

Done criteria:

- No known hot-path transition is missing from the matrix.
- Replay suite proves deterministic state for seeded scenarios.
- Crash recovery resumes from disk and DB truth without double-dispatching.

Suggested tests:

- `src/resources/extensions/gsd/tests/state-machine-full-walkthrough.test.ts`
- `src/resources/extensions/gsd/tests/crash-recovery.test.ts`
- `src/resources/extensions/gsd/tests/derive-state-db-disk-reconcile.test.ts`
- `src/resources/extensions/gsd/tests/recovery-attempts-reset.test.ts`

### Lane C: Single Writer

Implement:

- Runtime writes enter through a single write adapter when UOK is enabled.
- Active writer token is acquired at turn start and released at closeout.
- Concurrent write attempts are rejected or queued deterministically.
- Audit and gitops records include the same monotonic sequence id.
- Restart recovery continues the sequence without overwriting prior records.

Done criteria:

- Audit log can reconstruct exact write order.
- State, audit, and gitops writes are correlated by sequence id.
- Stale or superseded turn writes are dropped with an auditable reason.

Suggested tests:

- `src/resources/extensions/gsd/tests/write-gate.test.ts`
- `src/resources/extensions/gsd/tests/stale-lockfile-recovery.test.ts`
- `src/resources/extensions/gsd/tests/uok-audit-unified.test.ts`
- `src/resources/extensions/gsd/tests/uok-gitops-wiring.test.ts`

### Lane D: UOK Control Planes

Implement:

- Confirm default-on UOK flags match release intent.
- Restrict legacy fallback to emergency controls only.
- Add a parity replay pack for planning, dispatch, gitops, and audit events.
- Generate a parity report artifact for integration/release candidate branches.
- Harden typed boundaries between gate, model, gitops, audit, execution graph, and plan v2 planes.

Done criteria:

- UOK path remains default in stable builds.
- Legacy path is only reachable through explicit emergency controls.
- Parity report is generated for release candidates and blocks release on critical mismatch.

Suggested tests:

- `src/resources/extensions/gsd/tests/uok-flags.test.ts`
- `src/resources/extensions/gsd/tests/uok-kernel-path.test.ts`
- `src/resources/extensions/gsd/tests/uok-model-policy.test.ts`
- `src/resources/extensions/gsd/tests/uok-plan-v2-wiring.test.ts`

### Lane E: GitHub Routines

Implement:

- Add lane labels to the GitHub Sync routine contract and document their use.
- Add generated PR body sections through `src/resources/extensions/github-sync/templates.ts`:
  - impact area
  - transition risks
  - rollback plan
  - test evidence
  - lane label
- Add lane-aware check summary fields for generated integration PR/status bodies.
- Add a generated release checklist issue body for UOK swarm cutover validation.
- Add generated branch protection instructions for `integration/uok-swarm` and release branches.

Done criteria:

- Lane PRs have clear required evidence.
- Integration PR has a machine-generated or checklist-generated summary of lane deltas.
- Required checks and reviewer expectations are documented and enforceable.

Suggested tests:

- Existing CI workflow syntax checks through GitHub.
- Local validation where possible:
  - `npm run typecheck:extensions`
  - `node scripts/pr-risk-check.mjs --json < /tmp/changed-files.txt`

### Phase 2 Exit Criteria

- All lane branches have passing targeted tests.
- All lane PRs include transition risks, rollback plan, and test evidence.
- Hub integration branch contains no unresolved contract drift.

## Phase 3: Integration Hardening

**Goal:** Prove the lanes work together before release candidate cut.

### Tasks

1. Hub merges one lane PR at a time into `integration/uok-swarm`.
2. After each merge, run targeted tests for the touched lane.
3. After all lanes merge, run:
   - `npm run typecheck:extensions`
   - `npm run test:unit`
   - `npm run test:integration`
4. Generate UOK parity report for:
   - normal dispatch
   - blocked dispatch
   - recovery dispatch
   - gitops closeout
   - audit reconstruction
5. Run one rollback drill:
   - enable emergency legacy fallback
   - confirm dispatch path label changes to `legacy-fallback`
   - disable fallback
   - confirm UOK resumes as default

### Exit Criteria

- Full integration suite passes or failures are owned with a release-blocking decision.
- Parity report has no critical mismatches.
- Rollback drill completes within the agreed SLO.
- Integration branch has a release candidate summary.

## Phase 4: Release Candidate

**Goal:** Prepare a safe cutover from integration to the normal release path.

### Tasks

1. Create a release candidate PR from `integration/uok-swarm`.
2. Attach:
   - lane summary table
   - changed contracts
   - transition matrix diff
   - writer sequence proof
   - parity report
   - rollback drill evidence
3. Require maintainer review for:
   - `src/resources/extensions/gsd/uok/**`
   - `src/resources/extensions/gsd/state*`
   - `src/resources/extensions/gsd/auto/**`
   - `src/resources/extensions/github-sync/**`
4. Confirm docs explain emergency fallback:
   - preference flag
   - environment variable
   - expected audit/parity signal

### Exit Criteria

- Release candidate PR is approved.
- CI and required checks pass.
- No unresolved P0/P1 risks remain.

## Phase 5: Post-Merge Monitoring

**Goal:** Catch regressions caused by real-world dispatch volume and team workflow use.

### Monitoring Signals

- Dispatches without reason codes.
- Transition attempts not covered by the matrix.
- Duplicate writer sequence ids.
- Audit/gitops sequence gaps.
- Parity mismatches.
- PR cycle time per lane.
- Emergency fallback invocations.

### Follow-Up Windows

- 24 hours after merge: review first production/dogfood parity and audit signals.
- 72 hours after merge: review open regression issues and PR lead time.
- One release cycle after merge: decide whether any compatibility shims can be removed.

## Risk Register

| Risk | Detection | Mitigation |
| --- | --- | --- |
| Hidden state transition remains | matrix coverage test or replay mismatch | block merge until transition is modeled or explicitly exempted |
| Dual writer race | duplicate or out-of-order sequence id | writer token plus queue/reject policy |
| Dispatch contract drift | typecheck or envelope fixture failure | contract-first PR order and hub arbitration |
| UOK/legacy drift | parity report mismatch | block release unless mismatch is documented non-critical |
| GitHub routine bottleneck | PR lead time over 24h | smaller lane PRs and targeted required checks |
| Emergency fallback overuse | fallback audit events | require issue link and postmortem for each fallback use |

## Definition of Done

- Scheduler path is chosen from one typed contract.
- State transition matrix is checked in and covered by tests.
- Single writer token and sequence records correlate state, audit, and gitops writes.
- UOK is default-on, with legacy fallback emergency-only.
- Parity report is generated for release candidate branches.
- GitHub Sync lane labels, PR evidence body routines, and release checklist body routines are in place.
- Full integration verification passes or has explicit, owned release-blocking exceptions.

## Hub Command Routine

```bash
git fetch origin

# Review and test a lane branch.
git checkout lane/<name>
npm run typecheck:extensions
node --import ./src/resources/extensions/gsd/tests/resolve-ts.mjs --experimental-strip-types --test <lane-targeted-tests>

# Merge to integration branch. Hub only.
git checkout integration/uok-swarm
git merge --no-ff lane/<name>
npm run typecheck:extensions
npm run test:unit

# Publish integration status.
git push origin integration/uok-swarm
```

## Minimum Targeted Test Pack by Lane

```bash
# Workflow engine lane
node --import ./src/resources/extensions/gsd/tests/resolve-ts.mjs --experimental-strip-types --test \
  src/resources/extensions/gsd/tests/uok-contracts.test.ts \
  src/resources/extensions/gsd/tests/uok-execution-graph.test.ts \
  src/resources/extensions/gsd/tests/custom-workflow-engine.test.ts \
  src/resources/extensions/gsd/tests/workflow-dispatch.test.ts

# State machine lane
node --import ./src/resources/extensions/gsd/tests/resolve-ts.mjs --experimental-strip-types --test \
  src/resources/extensions/gsd/tests/state-machine-full-walkthrough.test.ts \
  src/resources/extensions/gsd/tests/state-derivation-parity.test.ts \
  src/resources/extensions/gsd/tests/crash-recovery.test.ts \
  src/resources/extensions/gsd/tests/derive-state-db-disk-reconcile.test.ts

# Single writer lane
node --import ./src/resources/extensions/gsd/tests/resolve-ts.mjs --experimental-strip-types --test \
  src/resources/extensions/gsd/tests/write-gate.test.ts \
  src/resources/extensions/gsd/tests/stale-lockfile-recovery.test.ts \
  src/resources/extensions/gsd/tests/uok-audit-unified.test.ts \
  src/resources/extensions/gsd/tests/uok-gitops-turn-action.test.ts

# UOK control planes lane
node --import ./src/resources/extensions/gsd/tests/resolve-ts.mjs --experimental-strip-types --test \
  src/resources/extensions/gsd/tests/uok-flags.test.ts \
  src/resources/extensions/gsd/tests/uok-kernel-path.test.ts \
  src/resources/extensions/gsd/tests/uok-model-policy.test.ts \
  src/resources/extensions/gsd/tests/uok-plan-v2-wiring.test.ts
```



================================================
FILE: docs/dev/2026-05-03-long-running-refactor-plan-of-plans.md
================================================
# GSD-2 Long-Running Refactor Plan-Of-Plans

Project/App: GSD-2
File Purpose: Complete phase-by-phase implementation roadmap for reducing complexity, token usage, build/test time, and app/process drift without a big-bang rewrite.

**Status:** Replanned - SRC-first execution
**Date:** 2026-05-03
**Delivery model:** Sequential phase gates with limited parallel work inside a phase
**Primary priorities:** `src/` token/context reduction, `src/` workflow simplification, `src/` DB split, behavior preservation, telemetry-gated legacy removal

## Objective

Create a decision-complete implementation roadmap for the long-running GSD-2 refactor. This document plans the full program before implementation starts. It is not a mandate to execute every phase at the same time.

The refactor should:

1. Reduce prompt and context size while improving output quality.
2. Establish only the shared contracts needed by `src/` runtime behavior before touching app surfaces.
3. Speed up local build/test loops without weakening `npm run verify:pr`.
4. Simplify auto-mode into a smaller workflow kernel with explicit adapters.
5. Preserve the single-writer DB invariant while splitting the DB monolith.
6. Consolidate duplicated process/shipping paths after the `src/` runtime is simpler.
7. Retire legacy paths only after telemetry and tests prove they are safe to remove.

## Active Replan: SRC-First Focus

The previous execution spent too much time migrating web, VS Code, daemon, and other app-adapter surfaces. Those migrations are valid cleanup, but they are not the current bottleneck. The active program now prioritizes `src/` because that is where the largest token, workflow, DB, prompt, and output-quality wins live.

**Do now:**

1. Finish only the contract work required to stabilize `src/` runtime fixtures and prevent drift in source-owned RPC/headless/auto behavior.
2. Move immediately into Phase 2 token/context reduction under `src/resources/extensions/gsd/**`, `src/headless*.ts`, and source-owned prompt/context builders.
3. Pull Phase 4 workflow-kernel work ahead of broad app-surface consolidation once prompt lanes and fixtures are stable.
4. Pull Phase 5 DB split directly after the kernel boundary, because `src/resources/extensions/gsd/gsd-db.ts` and state derivation are core complexity hotspots.
5. Keep Phase 3 build/test speed scoped to `src` test compile and source verification loops.

**Stop doing for now:**

1. No more web store, web bridge, VS Code, Studio, or daemon DTO migrations unless a `src` change requires the adapter to keep tests passing.
2. No app-surface polish or parity work.
3. No broad package-level contract chasing for its own sake.
4. No Phase 6 work until Phases 2, 4, and 5 have landed behind tests.

**Current active order:**

1. Phase 0: complete and keep as the safety gate.
2. Phase 1A: close `src`-owned contract fixture gaps only.
3. Phase 2: token and context reduction in `src`.
4. Phase 4: workflow kernel extraction in `src/resources/extensions/gsd`.
5. Phase 5: DB split in `src/resources/extensions/gsd`.
6. Phase 3: source-scoped build/test speed work, interleaved only when it accelerates the current `src` phase.
7. Phase 7: process consolidation after runtime simplification.
8. Phase 8: telemetry-gated cleanup.
9. Phase 6: app surfaces, explicitly deferred.

## Program Rules

- Complete phase plans before starting implementation for that phase.
- Execute phases in dependency order.
- Allow parallel work only within the current phase when file ownership is disjoint.
- Use stacked PRs, one concern per PR.
- Keep each PR reviewable and independently reversible.
- Do not overwrite existing dirty worktree changes.
- Follow `CONTRIBUTING.md`: architectural changes require RFC/ADR approval, tests use `node:test` and `node:assert/strict`, and source-grep tests are not acceptable for new coverage.
- Keep implementation provider-agnostic. Provider quirks belong behind capability metadata or adapters, not in workflow assumptions.

## Phase Dependency Map

| Phase | Depends on | Unlocks |
| --- | --- | --- |
| 0. Baseline and Safety | None | All later phase measurement and gates |
| 1A. SRC Contracts Closure | Phase 0 | Source-owned golden fixtures, safer prompt/kernel/DB work |
| 2. Token and Context Reduction | Phase 0, Phase 1A | Prompt reductions, quality evals, context compiler migration |
| 3. SRC Build/Test Speed | Phase 0 | Faster source iteration for later phases |
| 4. Workflow Kernel | Phases 0-2 | Auto-mode simplification, adapter boundaries |
| 5. DB Split | Phases 0-1 | Repository boundaries, state/query speed work |
| 6. App Surface Deferred | Phases 2, 4, 5 | Web/MCP/VS Code/Studio contract convergence after source simplification |
| 7. Process Consolidation | Phases 2, 4, 5 | Unified ship/PR evidence and docs alignment |
| 8. Legacy Cleanup | Phases 0-7 telemetry | Safe deletion of compatibility paths |

## Phase 0: Baseline And Safety Plan

**Goal:** Establish measurement, current behavior characterization, and safety gates before behavior changes.

**Owned areas:**

- Metrics commands and local reporting scripts.
- Prompt-size fixture harness.
- Build/test timing harness.
- Startup timing collection.
- `dist-test` size and copy-count reporting.

**Implementation plan:**

1. Add a read-only baseline command that prints a stable report with:
   - prompt chars by unit type
   - rendered skill count and skill catalog chars
   - context/tool/system/user-message chars where available
   - build/test command wall time
   - startup phase timing when `GSD_STARTUP_TIMING=1`
   - `dist-test` file count and byte size
2. Add fixtures for representative current behavior:
   - RPC JSONL state/stats/bash/UI events
   - prompt construction for discuss, plan-slice, execute-task, complete-slice, validate-milestone, reactive-execute
   - auto pause/resume/recovery/worktree/milestone transitions
   - DB state derivation and migration fixtures
3. Add a baseline report schema so later phases can compare before/after metrics without hand parsing.
4. Document how to run the baseline locally and in CI-like mode.

Baseline usage is documented in `docs/dev/refactor-baseline-runbook.md`.

**Acceptance criteria:**

- Baseline command works on a clean checkout and produces stable JSON plus human-readable summary.
- Current prompt, RPC, auto-mode, and DB behaviors have characterization coverage.
- No production behavior changes.

**Test matrix:**

| Area | Required tests |
| --- | --- |
| Baseline command | command exits 0, emits required sections, handles missing optional data |
| Prompt fixtures | representative unit prompts render and required sections are present |
| RPC fixtures | runtime and client fixtures parse the same JSONL payloads |
| Auto behavior | pause, resume, recoverable failure, worktree isolation, milestone transition |
| DB behavior | state derivation, migration fixture, single-writer guardrail |

**Metrics dashboard fields:**

- `prompt.systemChars`
- `prompt.skillCatalogChars`
- `prompt.unitInstructionChars`
- `prompt.volatileContextChars`
- `prompt.totalChars`
- `prompt.omittedSectionCount`
- `tokens.input`
- `tokens.cacheRead`
- `tokens.cacheWrite`
- `tokens.output`
- `build.command`
- `build.wallMs`
- `tests.command`
- `tests.wallMs`
- `startup.phase`
- `startup.wallMs`
- `distTest.fileCount`
- `distTest.bytes`

**Rollback/compatibility notes:**

- This phase is additive and read-only at runtime.
- Rollback is removal of measurement commands and fixtures only.

**Exit gate:** Baseline metrics exist, current behavior is covered, and later phases can compare before/after results.

**Implemented gate command:** `npm run baseline:refactor:phase0`

## Phase 1A: SRC Contracts Closure Plan

**Goal:** Close only the shared contract gaps needed by `src` runtime behavior and source-owned tests before moving to token/context reduction.

**Owned areas:**

- New `packages/contracts` workspace package named `@gsd-build/contracts`.
- Runtime RPC protocol types.
- `@gsd-build/rpc-client` compatibility exports when required by source tests.
- `src/headless*.ts`, `src/headless-events.ts`, `src/web/bridge-service.ts` only where source-owned runtime fixtures depend on them.
- `src/resources/extensions/gsd/**` contract fixtures.

**Implementation plan:**

1. Create `@gsd-build/contracts` with:
   - thinking levels including all currently supported levels
   - RPC command/event/response envelopes
   - session state and session stats
   - bash command result shape
   - UI request/response shapes, including secure input
   - MCP blocker request/response shapes
   - workflow tool registry metadata: canonical name, aliases, schema id, executor id, write policy, audit metadata
2. Move only stable public DTOs first. Do not move implementation services into the contracts package.
3. Make runtime RPC and `@gsd-build/rpc-client` import or re-export canonical contracts.
4. Add golden JSONL fixtures for source-owned runtime/headless/auto behavior.
5. Stop Phase 1A when `src` fixtures are stable. Defer MCP/web/VS Code/Studio/daemon DTO migration unless required by source tests.

**Implemented so far:**

- Created `packages/contracts` as `@gsd-build/contracts`.
- Added canonical RPC constants and DTOs for commands, responses, v2 events, session state, session stats, bash results, and extension UI request/response payloads.
- Migrated runtime RPC and `@gsd-build/rpc-client` to re-export the shared RPC contracts.
- Added Phase 1 fixture coverage with `src/tests/contracts-rpc-fixtures.test.ts`.
- Updated the web parity contract test to assert retry state through the shared `RpcSessionState` type instead of reading RPC type source text.

**Acceptance criteria:**

- Runtime, rpc-client compatibility exports, and source-owned fixtures share the same contract types for golden fixtures.
- `src` no longer depends on hand-rolled incompatible RPC/bash/UI shapes in the active runtime paths being refactored.
- Existing public package exports remain compatible or explicitly re-exported.

**Test matrix:**

| Area | Required tests |
| --- | --- |
| Contracts package | type-level compile coverage and runtime fixture validation helpers |
| RPC runtime | golden `get_state`, `get_session_stats`, bash, slash command, UI request events |
| RPC client | parses the same golden fixtures as runtime |
| Headless/source runtime | source-owned headless and UI event fixtures use canonical contracts |
| Deferred adapters | no new coverage required until Phase 6 |

**Metrics dashboard fields:**

- `contracts.fixtures.total`
- `contracts.fixtures.sharedBySurface`
- `contracts.surfaceDriftFailures`
- `contracts.legacyTypeImportsRemaining`

**Rollback/compatibility notes:**

- Keep adapter-level compatibility re-exports during migration.
- Do not remove old local types until all consumers use the new package.
- Each surface migration should be independently revertible.

**Exit gate:** Runtime, rpc-client compatibility exports, and source-owned golden fixtures share contracts and no high-risk `src` contract drift remains.

## Phase 2: Token And Context Reduction Plan

**Goal:** Reduce prompt/context size while improving relevance and preserving output quality.

**Owned areas:**

- `src/resources/extensions/gsd/prompts/**`.
- `src/resources/extensions/gsd/auto-prompts.ts`.
- `src/resources/extensions/gsd/auto-context.ts` and adjacent source context builders if present.
- Skill filtering and skill catalog rendering under `src`.
- GSD prompt builders and `UnitContextManifest` under `src`.
- Context budget enforcement under `src`.
- Prompt telemetry and golden prompt fixtures.

**Implementation plan:**

1. Split prompt material into three lanes:
   - stable system kernel
   - unit-specific instructions
   - volatile retrieved context
2. Wire existing `skillFilter` through session and GSD unit context so only relevant skills render by default.
3. Keep explicitly requested skills visible even if automatic filtering would omit them.
4. Make `UnitContextManifest` the source of truth for artifacts, tools, skills, memory, and budgets.
5. Enforce budgets in `composeUnitContext`:
   - required artifacts missing should fail loudly
   - optional artifacts missing should produce compact omission notes
   - oversized sections should be scored and trimmed before full truncation
6. Replace repeated large prompt prose with concise unit instructions plus enforced gates/policies.
7. Refactor validate-milestone and reactive-execute prompt assembly to share common context once and include per-unit deltas.
8. Add prompt quality/eval fixtures that compare:
   - prompt size
   - required instruction retention
   - tool policy retention
   - output acceptance criteria retention

**Acceptance criteria:**

- Representative golden prompts are at least 40% smaller than Phase 0 baselines.
- Required instructions and tool policies remain present.
- Skill catalog rendering is filtered by default and observable in telemetry.
- Provider-specific assumptions are replaced by capability-driven metadata where touched.

**Test matrix:**

| Area | Required tests |
| --- | --- |
| Skill filtering | default filtered catalog, explicit skill retention, empty catalog behavior |
| Context composer | required vs optional artifacts, budget enforcement, omitted section notes |
| Prompt fixtures | discuss, plan-slice, execute-task, complete-slice, validate-milestone, reactive-execute |
| Provider neutrality | token estimates and context behavior use capability metadata, not provider identity shortcuts |
| Quality evals | fixed task fixtures retain acceptance criteria and verification expectations |

**Metrics dashboard fields:**

- `prompt.reductionPercent`
- `prompt.renderedSkillCount`
- `prompt.filteredSkillCount`
- `prompt.requiredSectionsPresent`
- `prompt.omittedSectionCount`
- `context.requiredArtifactFailures`
- `context.optionalArtifactOmissions`
- `eval.outputPassRate`
- `eval.verificationPassRate`

**Rollback/compatibility notes:**

- Keep old prompt builders reachable behind a temporary compatibility flag until golden fixtures are stable.
- If output quality drops, rollback per unit type rather than reverting all context work.

**Exit gate:** 40% prompt-size reduction target is met on representative fixtures with no loss of required sections or acceptance criteria.

**Closeout status (2026-05-04):** Phase 2 source-owned golden fixtures now enforce the exit gate in `src/tests/prompt-golden-fixtures.test.ts`. Representative fixture total dropped from 45,247 chars at the SRC-first Phase 2 start baseline to 16,548 chars, a 63.4% reduction, while retaining required markers for `gsd_plan_slice`, `gsd_task_complete`, `gsd_slice_complete`, verification evidence, blocker handling, and summary sections. The regression gate runs through `npm run baseline:refactor:gate`.

## Phase 3: Build/Test Speed Plan

**Goal:** Speed up `src` iteration while preserving the full `npm run verify:pr` preflight.

**Owned areas:**

- Test compile script and `dist-test` lifecycle.
- Source-scoped local verification scripts.
- Build/test timing metrics.

**Implementation plan:**

1. Make `test:compile` stale-aware and reusable within one command sequence.
2. Avoid recompiling before both unit and package tests when sources have not changed.
3. Add a scoped changed-test command for `src` local iteration.
4. Preserve `npm run verify:pr` as the full gate.
5. Emit file count, byte count, cache hit, and wall-time metrics for source build/test steps.

**Acceptance criteria:**

- Warm local verification loop is at least 25% faster than Phase 0 baseline.
- Full `npm run verify:pr` still runs build core, extension typecheck, and unit tests.
- Stale detection is tested and does not skip required builds/tests.

**Test matrix:**

| Area | Required tests |
| --- | --- |
| Compile cache | cold run compiles, warm run reuses, source change invalidates |
| Scoped tests | changed-source inputs select expected test set |
| Full preflight | `verify:pr` command remains full and unchanged in intent |

**Metrics dashboard fields:**

- `testCompile.coldWallMs`
- `testCompile.warmWallMs`
- `testCompile.cacheHit`
- `testCompile.fileCount`
- `testCompile.bytesCopied`
- `verify.changedWallMs`
- `verify.fullWallMs`
- `webFingerprint.inputCount`
- `webFingerprint.cacheHit`

**Rollback/compatibility notes:**

- Keep a force-clean flag that ignores caches.
- If stale detection fails, default to recompiling rather than skipping.

**Exit gate:** Warm local loop improves by at least 25% and full preflight behavior is preserved.

**Closeout status (2026-05-04):** Phase 3 source-owned tooling now has stale-aware `test:compile`, a focused `npm run test:changed:src` loop, and baseline dashboard fields for test compile cache state, compile file/byte counts, warm/cold command timings, and changed/full verification timings. Measured cold compile was 6146ms; measured warm compile was 417ms, a 93.2% reduction. The focused changed-src path also supports `--base` and `--since`, uses the shared TypeScript test loader, and ran `src/tests/refactor-baseline.test.ts` successfully through the warm compile path. The full `npm run verify:pr` command remains the preserved preflight gate; Phase 3 verification in this stack used targeted tests plus `npm run baseline:refactor:gate`.

## Phase 4: Workflow Kernel Plan

**Goal:** Extract auto-mode into a pure workflow kernel with explicit adapters and a thin facade.

**Owned areas:**

- Auto-mode dispatch and loop boundaries.
- Workflow state/event/action contracts.
- Adapter interfaces for side effects.
- Recovery and worktree behavior tests.

**Implementation plan:**

1. Define pure kernel input/output:
   - input: derived workflow state, event, policy, runtime preferences
   - output: dispatch action, blocked reason, retry decision, closeout intent
2. Keep `auto.ts` as the public facade for start/stop/pause/resume.
3. Move side effects behind adapters:
   - DB repository adapter
   - git/worktree adapter
   - UI/notification adapter
   - model selection adapter
   - prompt/context adapter
   - MCP/remote questions adapter
   - recovery adapter
4. Convert existing auto loop behavior incrementally through the kernel without changing command behavior.
5. Add explainability output for "why this unit ran" and "why this unit is blocked."

**Acceptance criteria:**

- Kernel can be tested without filesystem, git, model provider, or UI side effects.
- `auto.ts` remains a compatibility facade.
- Existing auto pause/resume/recovery/worktree/milestone behavior is preserved.

**Test matrix:**

| Area | Required tests |
| --- | --- |
| Kernel pure tests | state/event/policy produces expected actions |
| Auto facade | start, stop, pause, resume retain behavior |
| Recovery | recoverable failure, retry limit, stuck detection |
| Worktree | isolation, cleanup, merge readiness |
| Milestone | transition, completion guard, validation gate |
| Explainability | blocked/runnable reason codes visible |

**Metrics dashboard fields:**

- `workflow.kernelActionCount`
- `workflow.blockedReason`
- `workflow.retryDecision`
- `workflow.adapterCallCount`
- `workflow.recoveryCount`
- `workflow.unitWallMs`

**Rollback/compatibility notes:**

- Route facade back to legacy loop per unit type if a kernel regression appears.
- Keep side-effect adapters thin and reversible.

**Exit gate:** Auto pause/resume/recovery/worktree/milestone tests pass through the facade and pure kernel tests cover dispatch decisions.

**Closeout status (2026-05-04):** Phase 4 SRC-first workflow-kernel extraction is closed. `auto/loop.ts` remains the compatibility facade, while repeated loop responsibilities now sit behind small source-owned adapters for phase/journal/turn reporting, session-lock validation, worker heartbeat, memory pressure, dispatch claims, dispatch ledger settling, sidecar queue dequeue, sidecar/custom-engine iteration data, unit dispatch, request timestamp resolution, iteration completion cleanup, custom verify retry persistence, custom-engine dispatch outcomes, verify outcomes, reconcile handling, and reconcile outcomes. Pure kernel decisions are covered in `src/resources/extensions/gsd/tests/workflow-kernel.test.ts`; side-effect adapters have direct `node:test` coverage; facade behavior is covered by `src/resources/extensions/gsd/tests/auto-loop.test.ts` and `src/resources/extensions/gsd/tests/custom-engine-loop-integration.test.ts`. Latest gate used targeted adapter tests, custom-engine integration, `auto-loop`, `npm run test:changed:src -- --since HEAD~1`, and `npm run baseline:refactor:gate`.

## Phase 5: DB Split Plan

**Goal:** Preserve the single-writer invariant while splitting the DB implementation into maintainable modules.

**Owned areas:**

- DB connection/provider loading.
- Schema and migrations.
- Domain repositories.
- Projection/manifest writers.
- Compatibility readers.
- State derivation hot paths.

**Implementation plan:**

1. Keep one public write facade and the existing single-writer invariant.
2. Split internals into:
   - connection/provider loading
   - schema definition
   - migrations
   - transaction helper
   - milestone repository
   - slice repository
   - task repository
   - journal/audit repository
   - memory repository
   - projection repository
   - compatibility readers
3. Move functions in behavior-preserving groups.
4. Add lightweight query variants where callers need only ids/status.
5. Benchmark JSON dependency lookup against indexed dependency tables before changing query strategy.
6. Keep old export names as compatibility exports until callers migrate.

**Acceptance criteria:**

- Single-writer guardrail remains enforced.
- Migration fixtures pass.
- State derivation output remains behaviorally identical.
- Hot-path query changes are backed by benchmark or query-plan evidence.

**Test matrix:**

| Area | Required tests |
| --- | --- |
| Single writer | write bypass detection, transaction wrapper behavior |
| Migrations | old schema fixtures migrate to current schema |
| Repositories | milestone/slice/task/journal/memory CRUD behavior |
| State derivation | DB authoritative state, legacy fallback behavior where still supported |
| Query speed | dependency lookup and state derive benchmark fixtures |

**Metrics dashboard fields:**

- `db.deriveStateWallMs`
- `db.queryCount`
- `db.writeCount`
- `db.migrationWallMs`
- `db.singleWriterViolationCount`
- `db.dependencyLookupWallMs`

**Rollback/compatibility notes:**

- Maintain facade exports until every caller is migrated.
- If repository split causes drift, revert the affected repository move without reverting schema work.

**Exit gate:** Migration, state derivation, dependency lookup, and single-writer tests pass with compatibility exports intact.

**Closeout status (2026-05-04):** Phase 5 SRC-first DB split is closed for this merge stack. `gsd-db.ts` remains the compatibility facade while provider loading, adapter wrapping, workspace connection caching, open-status state, transaction running, schema metadata/version helpers, runtime KV schema, coordination schema, verification-evidence schema, memory FTS schema, base schema DDL, migration backup/control/DDL steps, and row mappers for decision/requirement, task/slice, milestone/artifact, lightweight query, and verification evidence now live in focused source-owned modules. Verification coverage includes DB provider/adapter/cache/open-state/transaction/schema/migration helper tests, `gsd-db.test.ts`, `derive-state-db.test.ts`, `derive-state-db-disk-reconcile.test.ts`, and single-writer guardrail coverage.

## Phase 6: App Surface Plan - Deferred

**Goal:** Deferred. Make app surfaces thin adapters over shared contracts only after the `src` runtime is smaller and more stable.

**Owned areas:**

- Web workspace store and bridge service.
- MCP server adapters.
- VS Code client.
- Studio runtime boundary.
- Shared contract imports.

**Implementation plan:**

1. Do not start this phase until Phases 2, 4, and 5 have passed their exit gates.
2. Split web workspace store into store slices while preserving the public hook behavior.
3. Make web routes and bridge services delegate to shared contracts and services instead of importing deep runtime internals.
4. Move MCP workflow tool registration toward the shared workflow registry metadata.
5. Move VS Code DTO parsing to `@gsd-build/contracts`.
6. Decide Studio role:
   - either wire it to the same contracts
   - or mark it explicitly as prototype/non-runtime until wired
7. Add app contract tests for authenticated web flows and adapter payloads.

**Acceptance criteria:**

- Web public hook behavior remains compatible.
- Web, MCP, VS Code, and Studio no longer need divergent DTO definitions for migrated surfaces.
- Authenticated web file/session flows pass.

**Test matrix:**

| Area | Required tests |
| --- | --- |
| Web store | public selectors/actions preserve behavior |
| Web auth | file/session/live-state routes use authenticated client path |
| MCP | workflow registry aliases and blocker payloads match contracts |
| VS Code | parser handles canonical fixtures |
| Studio | contract status is tested or explicitly documented as prototype |

**Metrics dashboard fields:**

- `app.contractConsumersMigrated`
- `app.legacyDtoCount`
- `app.webStoreSliceCount`
- `app.bridgeDeepImportCount`
- `app.authFlowFailures`

**Rollback/compatibility notes:**

- Keep old web hook exports stable.
- Migrate one surface at a time so contract regressions are isolated.

**Exit gate:** App contract tests and authenticated web flows pass.

**Deferred status (2026-05-04):** Phase 6 remains intentionally out of scope for this SRC-first merge. A few low-risk app-surface contract import slices landed where they supported shared DTO convergence, but broad web, VS Code, Studio, and app-store restructuring is not required to merge the source-runtime refactor. Treat remaining Phase 6 work as a follow-up roadmap item with its own PR stack and app-specific verification gate.

## Phase 7: Process Consolidation Plan

**Goal:** Consolidate shipping paths and align docs/process with the actual DB-backed and CI-backed system.

**Owned areas:**

- `/gsd ship`.
- `git.auto_pr`.
- GitHub sync PR generation.
- PR evidence generation.
- CI/release docs.
- DB-vs-markdown workflow docs.

**Implementation plan:**

1. Create one PR evidence generator used by:
   - `/gsd ship`
   - `git.auto_pr`
   - GitHub sync
2. Evidence generator must include:
   - TL;DR
   - linked issue when required
   - change type checklist
   - tests run
   - AI-assisted disclosure
   - rollback notes for behavior changes
3. Align workflow docs:
   - DB is authoritative for auto-mode state
   - markdown is projection/manual/legacy unless explicitly stated
   - actual CI/release workflows are documented as they exist
4. Add one recommended task path per task size:
   - hotfix
   - bugfix
   - small feature
   - large feature
   - architecture change

**Acceptance criteria:**

- All PR creation paths use the same evidence generator.
- Generated PR bodies satisfy the repo template and contribution policy.
- Docs no longer conflict on DB state authority or release workflow behavior.

**Test matrix:**

| Area | Required tests |
| --- | --- |
| PR evidence | generated body includes required sections for each change type |
| Ship paths | `/gsd ship`, `git.auto_pr`, GitHub sync call shared generator |
| Docs examples | command examples match actual supported flows |
| Process classification | task size maps to expected planning/execution path |

**Metrics dashboard fields:**

- `process.prGeneratorConsumers`
- `process.prBodiesMissingIssue`
- `process.prBodiesMissingTests`
- `process.docsConflictCount`
- `process.shipPathCount`

**Rollback/compatibility notes:**

- Keep command names stable.
- If generator rollout breaks one path, route only that path back temporarily while preserving shared tests.

**Exit gate:** Generated PR bodies satisfy template, linked issue, tests, and AI disclosure requirements.

**Closeout status (2026-05-04):** Phase 7 SRC-first process consolidation is closed. `/gsd ship`, `git.auto_pr`, and GitHub sync PR body generation now use the shared `buildPrEvidence` generator with TL;DR, linked issue, tests run, change type, rollback/compatibility, and AI disclosure sections. GitHub sync slice PRs and swarm lane PRs reuse the same generator through their existing formatter surface. Process routing now has one canonical recommended path for hotfix, bugfix, small feature, large feature, and architecture-change work, surfaced in `/gsd start` and `/gsd templates` help. Baseline reporting now emits the Phase 7 dashboard fields: `process.prGeneratorConsumers`, `process.prBodiesMissingIssue`, `process.prBodiesMissingTests`, `process.docsConflictCount`, and `process.shipPathCount`; the current scan reports three shared PR evidence consumers, zero missing issue/test evidence flags, and zero DB-vs-markdown docs conflicts. Verification used targeted PR evidence, GitHub sync formatter, process path, workflow template, changed-src, and `npm run baseline:refactor:gate` tests.

## Phase 8: Legacy Cleanup Plan

**Goal:** Remove compatibility layers only after telemetry and tests prove they are safe.

**Owned areas:**

- Legacy markdown state fallback.
- Legacy workflow engines/templates.
- UOK parity/fallback wrappers.
- Duplicate MCP aliases.
- Legacy component formats.
- Provider-biased defaults or docs where touched.

**Implementation plan:**

1. Add telemetry counters before deletion:
   - markdown fallback used
   - legacy workflow engine selected
   - UOK fallback/parity path used
   - MCP alias used
   - legacy component format loaded
   - provider-specific default selected without explicit user preference
2. Add runtime diagnostics for deprecated paths.
3. Publish migration notes for each removed path.
4. Delete only one legacy category per PR.
5. Keep deletions independently reversible.

**Implemented so far:**

- Added runtime counters and baseline dashboard fields for all six Phase 8 deprecated paths.
- Wired `legacy.markdownFallbackUsed` to explicit markdown state derivation fallback.
- Wired `legacy.workflowEngineUsed` to legacy workflow template dispatch paths.
- Wired `legacy.uokFallbackUsed` to non-kernel UOK execution paths.
- Wired `legacy.mcpAliasUsed` to execution through backward-compatible MCP tool aliases.
- Wired `legacy.componentFormatUsed` to successful legacy `SKILL.md` and agent `.md` component loads.
- Wired `legacy.providerDefaultUsed` to canonical provider-default model fallback selection.
- Added one-warning-per-counter diagnostics through the shared legacy telemetry layer.
- Added opt-in snapshot persistence via `GSD_LEGACY_TELEMETRY_FILE` so representative runs can produce deletion-gate evidence without hardcoded local paths.
- Added explicit zero-use snapshot flushing so representative runs can produce deletion-gate evidence even when no legacy counters increment.
- Added `npm run legacy:cleanup:gate -- --file <snapshot>` to block deletion PRs when any Phase 8 counter is missing or nonzero.
- Added `npm run legacy:cleanup:evidence -- --file <snapshot>` to run representative checks, write a zero-use snapshot if no legacy path is touched, and then apply the cleanup gate.

**Migration notes before deletion:**

- Markdown state fallback: migrate projects to DB-authoritative state and keep markdown as projection/manual reference only. Do not remove fallback until `legacy.markdownFallbackUsed` stays at zero on representative runs.
- Legacy workflow engines/templates: move active workflows to `auto-milestone` templates and keep legacy aliases documented until `legacy.workflowEngineUsed` is zero.
- UOK fallback/parity wrappers: resolve blockers that require `legacy_fallback` or `GSD_UOK_FORCE_LEGACY`; removal is allowed only after kernel path telemetry shows no fallback use.
- MCP tool aliases: update agents and prompts to canonical `gsd_*` tool names before alias removal. Alias wrappers now count actual alias execution.
- Legacy component formats: migrate installed skills and agents to `component.yaml` plus `SKILL.md` or `AGENT.md` entry files before removing legacy loaders.
- Provider defaults: configure explicit provider-aware model preferences or available model registries before removing canonical provider-default fallbacks.

**Acceptance criteria:**

- No legacy path is removed before telemetry exists.
- Removal PRs include tests proving supported replacements.
- Migration notes exist for affected users or extension authors.

**Test matrix:**

| Area | Required tests |
| --- | --- |
| Telemetry | counters increment for each legacy path |
| Diagnostics | deprecated path warning appears once and is actionable |
| Replacement | modern path covers the supported behavior |
| Deletion | old fixtures fail with migration guidance where appropriate |

**Metrics dashboard fields:**

- `legacy.markdownFallbackUsed`
- `legacy.workflowEngineUsed`
- `legacy.uokFallbackUsed`
- `legacy.mcpAliasUsed`
- `legacy.componentFormatUsed`
- `legacy.providerDefaultUsed`

**Rollback/compatibility notes:**

- Every deletion PR must identify the exact commit that can restore the path.
- Prefer feature-flagged disablement before hard deletion for high-risk paths.

**Exit gate:** Each deletion is small, tested, telemetry-backed, documented, and independently reversible.

**Closeout status (2026-05-04):** Phase 8 telemetry readiness is closed. No legacy path has been deleted in this stack. Deletion work is intentionally blocked until representative runs pass `npm run legacy:cleanup:evidence -- --file <snapshot>`, or an equivalent manually collected zero-use snapshot passes `npm run legacy:cleanup:gate -- --file <snapshot>`.

## File Ownership Map

| Phase | Primary owners | Must not overlap with |
| --- | --- | --- |
| 0 | metrics scripts, test fixtures, docs for baseline command | implementation refactors in auto/DB/web |
| 1A | `packages/contracts`, source-owned RPC/headless/auto fixtures | prompt builder rewrites that change payload shape |
| 2 | `src/resources/extensions/gsd/prompts/**`, `src/resources/extensions/gsd/*context*`, skill filtering, GSD prompt/context compiler | workflow kernel side-effect moves |
| 3 | source build/test scripts and `dist-test` lifecycle | test fixture schema moves from Phase 1A without coordination |
| 4 | auto-mode kernel/facade/adapters | DB module moves and prompt compiler churn in same files |
| 5 | DB internals and repositories | workflow kernel changes that depend on DB private functions |
| 6 | deferred web store, bridge, MCP, VS Code, Studio adapters | active `src` phases |
| 7 | ship/PR generator, GitHub sync, docs | app adapter DTO migration |
| 8 | one legacy category per deletion PR | any active behavior-changing phase |

## Long-Running Program Dashboard

The implementation program should maintain a dashboard with these groups:

| Group | Fields |
| --- | --- |
| Speed | build wall time, unit test wall time, package test wall time, scoped verify wall time, startup wall time |
| Tokens | system chars, skill chars, unit instruction chars, volatile context chars, total prompt chars, input tokens, output tokens, cache read/write |
| Quality | golden prompt required-section pass rate, output eval pass rate, verification pass rate, retry count |
| Contracts | fixture pass rate by surface, legacy DTO count, incompatible payload failures |
| Workflow | kernel action count, blocked reason counts, recovery count, unit wall time |
| DB | derive state p95, query count, write count, migration wall time, single-writer violations |
| Process | PR generator consumers, missing issue/test evidence count, docs conflict count |
| Legacy | usage counters for each deprecated path |

## Implementation Order

1. Phase 0: Baseline and Safety
2. Phase 1A: SRC Contracts Closure
3. Phase 2: Token and Context Reduction
4. Phase 4: Workflow Kernel
5. Phase 5: DB Split
6. Phase 3: SRC Build/Test Speed where it accelerates active source work
7. Phase 7: Process Consolidation
8. Phase 8: Legacy Cleanup
9. Phase 6: App Surface, deferred until source runtime simplification lands

Parallel work is allowed only inside a phase when file ownership is disjoint and the phase lead owns integration.

## Done Definition For The Whole Program

- Prompt/context size is materially lower and tracked by repeatable metrics.
- Shared contracts are used where they protect `src` runtime behavior; app-surface convergence is deferred and tracked separately.
- Warm local verification is materially faster while full `verify:pr` remains intact.
- Auto-mode has a pure testable workflow kernel and explicit side-effect adapters.
- DB internals are split without weakening the single-writer invariant.
- App-surface convergence is either completed in a dedicated Phase 6 stack or explicitly deferred from the SRC-first merge.
- Shipping paths produce one consistent PR evidence format.
- Legacy paths are removed only after telemetry, tests, and migration notes.



================================================
FILE: docs/dev/ADR-001-branchless-worktree-architecture.md
================================================
# ADR-001: Branchless Worktree Architecture

**Status:** Superseded
**Date:** 2026-03-15
**Superseded:** 2026-05-08 by the ADR-016 trio (worktree lifecycle split, fail-closed safety, phase-2 design) and ADR-017 (drift-driven state reconciliation)
**Deciders:** Lex Christopherson
**Advisors:** Claude Opus 4.6, Gemini 2.5 Pro, GPT-5.4 (Codex)

> **Superseded — read this first.** The DB-authoritative runtime model took the place of this ADR's primary motivation (`.gsd/` merge conflicts, planning-artifact visibility): the project-root database is canonical, markdown files are projections, and cross-branch state clobbering is no longer the failure mode this ADR was trying to fix. The remaining worktree concerns (lifecycle ownership, fail-closed source-writing, drift repair) were consolidated by **ADR-014** (deep Auto Orchestration), **ADR-015** (runtime invariant modules), the **ADR-016 trio** (worktree lifecycle/projection split, fail-closed safety), and **ADR-017** (drift-driven reconciliation). Slice branches inside milestone worktrees were retained. The historical analysis below is preserved for context; the decisions in §Decision were not adopted.

## Context

GSD uses git for isolation during autonomous coding sessions. The current architecture (shipped in M003, v2.13.0) creates a **worktree per milestone** with **slice branches inside each worktree**. Each slice (`S01`, `S02`, ...) gets its own branch (`gsd/M001/S01`) within the worktree, which merges back to the milestone branch (`milestone/M001`) via `--no-ff` when the slice completes. The milestone branch squash-merges to `main` when the milestone completes.

This architecture replaced a previous "branch-per-slice" model that had severe `.gsd/` merge conflicts. M003 solved the merge conflicts but retained slice branches inside worktrees, inheriting complexity that has produced persistent, user-facing failures.

### Problems

**1. Planning artifact invisibility (loop detection failures)**

When `research-slice` or `plan-slice` dispatches, the agent writes artifacts (e.g., `S02-RESEARCH.md`) on a slice branch. After the agent completes, `handleAgentEnd` switches back to the milestone branch for the next dispatch. The artifact is on the slice branch, not the milestone branch. `verifyExpectedArtifact()` checks the milestone branch, can't find the file, increments the loop counter, retries, same result. After 3 retries → hard stop. After 6 lifetime dispatches → permanent stop. This burns budget and blocks progress.

Documented in the auto-stop architecture doc as "The Branch-Switching Problem."

**2. `.gsd/` state clobbering across branches**

`.gsd/` is gitignored (line 52 of `.gitignore`: `.gsd/`). Planning artifacts (roadmaps, plans, summaries, decisions, requirements) live in `.gsd/milestones/` but are invisible to git. When multiple branches or worktrees operate from the same repo, they share a single `.gsd/` directory on disk. Branch A's M001 roadmap overwrites Branch B's M001 roadmap. GSD reads corrupted state, shows wrong milestone as complete, or enters infinite dispatch loops.

The codebase has a contradictory workaround: `smartStage()` (git-service.ts:304-352) force-adds `GSD_DURABLE_PATHS` (milestones/, DECISIONS.md, PROJECT.md, REQUIREMENTS.md, QUEUE.md) despite the `.gitignore`. This means `.gsd/milestones/` IS partially tracked on some branches but the gitignore claims otherwise. The code fights the configuration.

**3. Merge/conflict code complexity**

The current slice branch model requires:
- `mergeSliceToMilestone()` — 98 lines, `--no-ff` merge with `withMergeHeal` wrapper
- `mergeSliceToMain()` — 189 lines, squash-merge with conflict detection/categorization/auto-resolution
- `git-self-heal.ts` — 198 lines, 3 recovery functions for merge failures
- `fix-merge` dispatch unit — dedicated LLM session to resolve conflicts the auto-resolver can't handle
- `smartStage()` — 49 lines of runtime exclusion during staging
- Conflict categorization — 80 lines classifying `.gsd/` vs runtime vs code conflicts

Total: **~582 lines** of merge/branch/conflict code across 3 files, plus the `fix-merge` prompt template and dispatch logic. This code exists solely because of slice branches.

**4. Dual isolation modes**

Branch-mode (`git-service.ts:mergeSliceToMain`) and worktree-mode (`auto-worktree.ts:mergeSliceToMilestone`) have parallel implementations with different merge strategies, different conflict handling, and different branch naming. Both paths must be maintained and tested. 11 test files exercise merge/branch/worktree logic.

**5. Bug history**

- v2.11.1: URGENT fix for parse cache staleness causing repeated unit dispatch (directly caused by branch switching invalidation timing)
- v2.13.1: Windows hotfix for multi-line commit messages in `mergeSliceToMilestone`
- 15+ separate bug fixes for `.gsd/` merge conflicts in the pre-M003 era
- Persistent user complaints about loop detection failures and state corruption

## Decision

**Eliminate slice branches entirely.** All work within a milestone worktree commits sequentially on a single branch (`milestone/<MID>`). No branch creation, no branch switching, no slice merges, no conflict resolution within a worktree.

Track `.gsd/` planning artifacts in git. Gitignore only runtime/ephemeral state.

### The Architecture

```
main ──────────────────────────────────────────── main
  │                                                 ↑
  └─ worktree (milestone/M001)                      │
       │                                            │
       commit: feat(M001): context + roadmap        │
       commit: feat(M001/S01): research             │
       commit: feat(M001/S01): plan                 │
       commit: feat(M001/S01/T01): impl             │
       commit: feat(M001/S01/T02): impl             │
       commit: feat(M001/S01): summary + UAT        │
       commit: feat(M001/S02): research             │
       commit: ...                                  │
       commit: feat(M001): milestone complete       │
       │                                            │
       └──────────── squash merge ──────────────────┘
```

### Git Primitives Used

| Primitive | Purpose |
|-----------|---------|
| **Worktrees** | One per active milestone. Filesystem isolation. |
| **Commits** | Granular sequential history of every action. |
| **Squash merge** | Clean single commit on `main` per milestone. |
| **Branches** | Only `main` and `milestone/<MID>`. Nothing else. |

### Git Primitives NOT Used

| Primitive | Why Not |
|-----------|---------|
| Slice branches | Slices are sequential. Branches add complexity with no rollback benefit. |
| `--no-ff` merges | No branches to merge within a worktree. |
| Branch switching | Never happens. All work on one branch. |
| Conflict resolution | No merges within a worktree means no conflicts within a worktree. |

### `.gsd/` Tracking Model

**Tracked in git (travels with the branch):**
```
.gsd/milestones/         — roadmaps, plans, summaries, research, contexts, task plans/summaries
.gsd/PROJECT.md          — project overview
.gsd/DECISIONS.md        — architectural decision register
.gsd/REQUIREMENTS.md     — requirements register
.gsd/QUEUE.md            — work queue
```

**Gitignored (ephemeral, runtime, infrastructure):**
```
.gsd/runtime/            — dispatch records, timeout tracking
.gsd/activity/           — JSONL session dumps
.gsd/worktrees/          — git worktree working directories
.gsd/auto.lock           — crash detection sentinel
.gsd/metrics.json        — token/cost accumulator
.gsd/completed-units.json — dispatch idempotency tracker
.gsd/STATE.md            — rendered state projection
.gsd/gsd.db              — authoritative runtime database (local, gitignored)
.gsd/DISCUSSION-MANIFEST.json — discussion phase tracking
.gsd/milestones/**/*-CONTINUE.md — interrupted-work markers
.gsd/milestones/**/continue.md   — legacy continue markers
```

### `.gitignore` Update

Replace the current blanket `.gsd/` ignore with explicit runtime-only ignores:

```gitignore
# ── GSD: Runtime / Ephemeral ─────────────────────────────────
.gsd/auto.lock
.gsd/completed-units.json
.gsd/STATE.md
.gsd/metrics.json
.gsd/gsd.db
.gsd/activity/
.gsd/runtime/
.gsd/worktrees/
.gsd/DISCUSSION-MANIFEST.json
.gsd/milestones/**/*-CONTINUE.md
.gsd/milestones/**/continue.md
```

Planning artifacts (milestones/, PROJECT.md, DECISIONS.md, REQUIREMENTS.md, QUEUE.md) are NOT in `.gitignore` and are tracked normally.

## Consequences

### Code Deletion

| File | Lines Deleted | What's Removed |
|------|--------------|----------------|
| `auto-worktree.ts` | ~246 | `mergeSliceToMilestone()`, `shouldUseWorktreeIsolation()`, `getMergeToMainMode()`, slice merge guards |
| `git-service.ts` | ~250 | `mergeSliceToMain()`, conflict resolution, runtime stripping post-merge, `ensureSliceBranch()`, `switchToMain()` |
| `git-self-heal.ts` | ~86 | `abortAndReset()`, `withMergeHeal()` (merge-specific recovery) |
| `auto.ts` | ~150 | Merge dispatch guards, `fix-merge` dispatch path, branch-mode routing |
| `worktree.ts` | ~40 | `getSliceBranchName()`, `ensureSliceBranch()`, `mergeSliceToMain()` delegates |
| **Test files** | ~11 files | `auto-worktree-merge.test.ts`, `auto-worktree-milestone-merge.test.ts`, merge-related test cases |
| **Total** | **~770+ lines** | |

### What `mergeMilestoneToMain()` Becomes

The function simplifies dramatically:
1. Auto-commit any dirty state in worktree
2. `chdir` back to main repo root
3. `git checkout main`
4. `git merge --squash milestone/<MID>`
5. `git commit` with milestone summary
6. Remove worktree + delete branch

No conflict categorization. No runtime file stripping. No `.gsd/` special handling. Planning artifacts merge cleanly because they're in `.gsd/milestones/M001/` which doesn't exist on `main` until this merge.

### What `smartStage()` Becomes

The force-add of `GSD_DURABLE_PATHS` is no longer needed — planning artifacts are not gitignored, so `git add -A` picks them up naturally. The function reduces to:

1. `git add -A`
2. `git reset HEAD -- <runtime paths>` (unstage runtime files)

The `_runtimeFilesCleanedUp` one-time migration logic can also be removed.

### What Happens to `handleAgentEnd()`

After any unit completes:
1. Invalidate caches
2. `autoCommitCurrentBranch()` — commits on the one and only branch
3. `verifyExpectedArtifact()` — file is always on the current branch (no branch switching)
4. Persist completion key

The "Path A fix" (lines 937-953) becomes the only path. No branch mismatch possible.

### What Happens to `fix-merge`

The `fix-merge` dispatch unit type is eliminated. Within a worktree, there are no merges that can conflict. The only merge is milestone→main (squash), and if that conflicts (rare, parallel milestone edge case), it's handled as a one-time resolution at milestone completion — not a dispatch loop.

### Backwards Compatibility

The `shouldUseWorktreeIsolation()` three-tier preference resolution is replaced by a single behavior: worktree isolation is always used. The `git.isolation: "branch"` preference is deprecated.

Projects with existing `gsd/M001/S01` slice branches can still be read by state derivation, but new work never creates slice branches.

### Risks

**1. Parallel milestone code conflicts at squash-merge time**

If two milestones modify the same source file, the second squash-merge to `main` will conflict. Mitigation: `git fetch origin main && git rebase main` before squash-merge. This is standard practice and rare in single-user workflows.

**2. Loss of per-slice git history after squash**

Squash merge collapses all commits into one on `main`. Mitigations:
- Commit messages tag slices (`feat(M001/S01/T01):`) — filterable with `git log --grep`
- The milestone branch can be preserved (not deleted) if history is needed
- Alternative: `merge --no-ff` instead of `--squash` to keep history on `main`

**3. SQLite DB desync after `git reset`**

If tracked markdown rolls back via `git reset --hard`, the gitignored `gsd.db` does not. Current GSD treats the database as authoritative during runtime and does not silently import markdown projections. Operators should use explicit recovery/import commands when markdown is the intended source after database loss or corruption.

**4. Disk space with multiple worktrees**

Each worktree duplicates the working directory (including `node_modules`). Mitigation: single active milestone at a time (single-user workflow), immediate cleanup after completion.

## Alternatives Considered

### A. Keep slice branches, fix visibility with immediate mini-merges

After `research-slice` or `plan-slice`, immediately merge the slice branch back to the milestone branch. This fixes the loop detection bug but retains all merge complexity.

**Rejected:** Adds another merge path instead of removing the root cause. Still requires conflict resolution, self-healing, branch switching.

### B. Keep `.gsd/` gitignored, bootstrap from git history for manual worktrees

When GSD detects an empty `.gsd/` in a worktree, reconstruct state from the branch's git history using `git show <commit>:.gsd/...`.

**Rejected:** Recovery logic, not architecture. Doesn't fix the fundamental problem of branch-agnostic state. Fails when git history has been rewritten.

### C. Branch-scoped `.gsd/` directories (`.gsd/branches/<branch-name>/milestones/...`)

Each branch writes to a namespaced subdirectory within `.gsd/`.

**Rejected:** Adds complexity instead of removing it. Requires renaming/moving on branch creation, doesn't work with standard git tools (`git checkout` doesn't rename directories).

## Validation

This architecture was stress-tested by three independent models:

**Gemini 2.5 Pro** identified 6 attack vectors. None broke the core model. Recommendations: pre-flight rebase before squash-merge (adopted), heartbeat locks (already exists), DB rebuild on startup (adopted via M001/S02 importers).

**GPT-5.4 (Codex)** read the full codebase and confirmed the model is sound. Identified that `smartStage()` already force-adds durable paths (validating the tracked-artifact approach) and that `resolveMainWorktreeRoot` in PR #487 is architecturally wrong (adopted — PR to be closed).

**Codebase analysis** confirmed `.gsd/milestones/` is already partially tracked on `main` despite the `.gitignore`, that `GSD_DURABLE_PATHS` exists as a code-level acknowledgment that planning artifacts should be tracked, and that the README already documents the correct runtime-only gitignore pattern.

### Codex (GPT-5.4) Dissent — "No Slice Branches Is a Redesign"

Codex read the full codebase and raised 4 concerns. Each is addressed:

**Concern 1: "Crash after slice done but before integration — today the runtime detects orphaned slice branches and merges them."**

Rebuttal: In the branchless model, there is no integration step to crash between. Slice work is committed directly on the milestone branch. On restart, `deriveState()` reads the branch state as-is. The orphaned-branch recovery path exists solely because of slice branches — removing branches removes the failure mode it recovers from.

**Concern 2: "Concurrent edits to shared root docs (PROJECT.md, DECISIONS.md) from two terminals."**

Rebuttal: Valid edge case. If `/gsd queue` edits `DECISIONS.md` on `main` while auto-mode edits it in a worktree, there's a content conflict at squash-merge time. This is a standard git content conflict — no different from two developers editing the same file. Handled by normal merge resolution. Not caused by or solved by slice branches.

**Concern 3: "Slice→milestone merges provide continuous integration. Removing them pushes conflict discovery to the end."**

Rebuttal: In a single-user sequential workflow, there is nothing to integrate against within a worktree. Each slice builds on the previous one. The only conflict source is `main` diverging (e.g., another milestone merging first), which slice→milestone merges don't catch anyway — they merge within the worktree, not against `main`. Pre-flight rebase before squash-merge catches this more directly.

**Concern 4: "Replace slice branches with another explicit slice-boundary primitive. Don't just delete them."**

Response: Accepted in spirit. Commits with conventional tags (`feat(M001/S01):`, `feat(M001/S01/T01):`) serve as the slice boundary primitive. `git log --grep="M001/S01"` isolates a slice's history. `git revert` targets specific commits. Git tags (`gsd/M001/S01-complete`) can mark slice completion if needed. The boundary primitive is commit metadata, not branches.

## Action Items

1. Close PR #487 (`resolveMainWorktreeRoot`) — contradicts this architecture
2. Implement as a GSD milestone with phases:
   - Update `.gitignore` and force-add existing planning artifacts
   - Remove slice branch creation/switching/merging code
   - Simplify `mergeMilestoneToMain()` and `smartStage()`
   - Remove `fix-merge` dispatch unit
   - Remove branch-mode isolation (`git.isolation: "branch"`)
   - Update/delete 11 test files
   - Update README suggested gitignore
   - Migration path for existing projects with slice branches



================================================
FILE: docs/dev/ADR-002-external-state-directory.md
================================================
# ADR-002: External State Directory

**Status:** Not Adopted
**Date:** 2026-03-16 (proposed window)
**Closed:** 2026-05-10
**Related:** ADR-001 (branchless worktree, superseded), ADR-016 trio, ADR-017 (drift-driven reconciliation)

## Context

ADR-001 and ADR-003 reference an "external state directory" ADR-002 that was never written as a standalone document. The idea, as carried in those ADRs, was to move `.gsd/` runtime state out of the working tree so that branch checkouts, slice merges, and worktree teardown could not clobber in-flight planning artifacts.

## Decision

**Not adopted.** GSD instead moved to a **DB-authoritative runtime model**: the project-root database is canonical, and `.gsd/**` markdown files are rendered projections of that database. With the database living outside the per-worktree working set and projections regenerated on demand, the cross-branch state-clobbering problem an external state directory was meant to solve no longer presents the same shape.

The worktree-level concerns this ADR would have touched were addressed by:

- **ADR-016 trio** — split worktree handling into Worktree Lifecycle and State Projection modules, with fail-closed safety for source-writing units.
- **ADR-017** — drift-driven State Reconciliation module, which detects and repairs the specific state classes (sketch flags, merge state, roadmap divergence, completion timestamps, stale workers) that an external state directory would have tried to keep out of the working tree entirely.

## Consequences

- Cross-references to "ADR-002" from ADR-001 and ADR-003 should be read as "the work that became ADR-016/017."
- No external state directory exists or is planned. `.gsd/` artifacts remain inside the working tree as projections.



================================================
FILE: docs/dev/ADR-004-capability-aware-model-routing.md
================================================
# ADR-004: Capability-Aware Model Routing

**Status:** Implemented (Phase 2)
**Date:** 2026-03-26
**Revised:** 2026-04-03
**Deciders:** Jeremy McSpadden
**Related:** ADR-003 (pipeline simplification), [Issue #2655](https://github.com/gsd-build/gsd-2/issues/2655), `docs/dynamic-model-routing.md`

## Context

GSD already supports dynamic model routing in auto-mode, but the current router is fundamentally **complexity-tier and cost based**, not **task-capability based**.

Today the selection pipeline is:

```
unit dispatch
  → classifyUnitComplexity(unitType, unitId, basePath, budgetPct)
      → UNIT_TYPE_TIERS default mapping
      → analyzeTaskComplexity() / analyzePlanComplexity()  [metadata heuristics]
      → getAdaptiveTierAdjustment()                        [routing history]
      → applyBudgetPressure()                              [budget ceiling]
  → resolveModelForComplexity(classification, phaseConfig, routingConfig, availableModelIds)
      → downgrade-only: never upgrades beyond user's configured model
      → MODEL_CAPABILITY_TIER lookup → cheapest available in tier
      → fallback chain assembly
  → resolveModelId() → pi.setModel()
  → before_provider_request hook (payload mutation only)
```

This architecture works when all models inside a tier are effectively interchangeable. That assumption no longer holds.

Users increasingly configure heterogeneous provider pools through `models.json`, scoped provider setup, and `/scoped-models`. In practice:

- Claude-class models often perform best on greenfield implementation and architecture work
- Codex-class models often perform best on debugging, refactoring, and root-cause analysis
- Gemini-class models often perform best on long-context synthesis and research-heavy tasks
- Fast small models are often best for cheap validation, triage, and lightweight hooks

The current router cannot express those differences. If Claude and Codex are both available at the same tier, GSD either:

- treats them as equivalent and picks the cheaper one, or
- requires the user to hardcode specific phase models manually

That produces three structural problems:

### 1. Wrong optimization target

The router optimizes primarily for **task difficulty vs model cost**. The real problem is **task requirements vs model strengths**, subject to cost constraints.

### 2. Poor behavior with heterogeneous pools

Different users have different subscriptions and provider access. A fixed mapping like "research always uses Gemini" does not generalize when the user only has Claude + Codex, or only local models.

### 3. Capability knowledge is trapped in user intuition

Experienced users know which models are better at coding, debugging, research, long-context work, or instruction following. GSD has no representation for that knowledge, so it cannot route intelligently on the user's behalf.

The system already has several building blocks that make a richer router feasible:

- unit types already encode the kind of work being dispatched
- `complexity-classifier.ts` already extracts rich `TaskMetadata` (file counts, dependency counts, tags, complexity keywords, code block counts)
- `auto-dispatch.ts` and prompt builders provide stable task categories
- `ctx.modelRegistry.getAvailable()` exposes the current model pool
- `models.json` already supports user overrides and cost data per model
- budget ceilings, routing history, and retry escalation already exist
- the `model_select` hook fires on model changes and could be extended for pre-selection interception

## Decision

**Extend dynamic routing from a one-dimensional tier system to a two-dimensional system that combines complexity classification ("how hard") with capability scoring ("what kind"), while preserving downgrade-only semantics, budget controls, and user overrideability.**

### Design Principles

1. **Downgrade-only invariant is preserved.** The user's configured model for a phase is always the ceiling. Capability scoring ranks models within the eligible set — it never promotes above the user's configured model.

2. **Complexity classification remains.** The existing `classifyUnitComplexity()` pipeline (unit type defaults, task plan analysis, adaptive learning, budget pressure) continues to determine tier eligibility. Capability scoring selects among tier-eligible models.

3. **Cost is a constraint, not a score dimension.** Budget pressure constrains which models are eligible. Capability profiles describe what models are good at, not what they cost.

4. **Requirement vectors are dynamic, not static.** Task requirements are computed from `(unitType, TaskMetadata)`, not from unit type alone.

### The Revised Routing Pipeline

```
unit dispatch
  → classifyUnitComplexity(unitType, unitId, basePath, budgetPct)
      [unchanged — determines tier eligibility and budget filtering]
  → resolveModelForComplexity(classification, phaseConfig, routingConfig, availableModelIds)
      → STEP 1: filter to tier-eligible models (downgrade-only from user ceiling)
      → STEP 2: if capability routing enabled AND >1 eligible model:
          → computeTaskRequirements(unitType, taskMetadata)
          → scoreEligibleModels(eligible, taskRequirements)
          → select highest-scoring model (deterministic tie-break by cost, then ID)
      → STEP 3: assemble fallback chain
  → resolveModelId() → pi.setModel()
```

### Model Capability Profiles

Each model gains an optional capability profile:

```ts
interface ModelCapabilities {
  coding: number;       // greenfield implementation, code generation
  debugging: number;    // root-cause analysis, error diagnosis, refactoring
  research: number;     // information synthesis, investigation, exploration
  reasoning: number;    // multi-step logic, planning, architecture
  speed: number;        // response latency (inverse of thinking time)
  longContext: number;  // effective use of large input windows
  instruction: number;  // instruction following, structured output adherence
}
```

Scores are normalized `0–100`. Seven dimensions. No `costEfficiency` dimension — cost is handled separately by budget pressure and tier economics.

Models without a capability profile are treated as having uniform scores across all dimensions (score 50 in each), which makes capability scoring a no-op for those models and falls back to the existing cheapest-in-tier behavior.

### Dynamic Task Requirement Vectors

Requirement vectors are computed as a function of `(unitType, TaskMetadata)`, not looked up from a static table. This preserves the nuance that `classifyUnitComplexity` already captures.

```ts
function computeTaskRequirements(
  unitType: string,
  metadata?: TaskMetadata,
): Partial<Record<keyof ModelCapabilities, number>> {
  // Base vector from unit type
  const base = BASE_REQUIREMENTS[unitType] ?? { reasoning: 0.5 };

  // Refine based on task metadata (only for execute-task)
  if (unitType === "execute-task" && metadata) {
    // Docs/config/rename tasks → boost instruction, reduce coding
    if (metadata.tags?.some(t => /^(docs?|readme|comment|config|typo|rename)$/i.test(t))) {
      return { ...base, instruction: 0.9, coding: 0.3, speed: 0.7 };
    }
    // Debugging keywords → boost debugging and reasoning
    if (metadata.complexityKeywords?.some(k => k === "concurrency" || k === "compatibility")) {
      return { ...base, debugging: 0.9, reasoning: 0.8 };
    }
    // Migration/architecture → boost reasoning and coding
    if (metadata.complexityKeywords?.some(k => k === "migration" || k === "architecture")) {
      return { ...base, reasoning: 0.9, coding: 0.8 };
    }
    // Many files or high estimated lines → boost coding
    if ((metadata.fileCount ?? 0) >= 6 || (metadata.estimatedLines ?? 0) >= 500) {
      return { ...base, coding: 0.9, reasoning: 0.7 };
    }
  }

  return base;
}
```

Base requirement vectors by unit type:

```ts
const BASE_REQUIREMENTS: Record<string, Partial<Record<keyof ModelCapabilities, number>>> = {
  "execute-task":        { coding: 0.9, instruction: 0.7, speed: 0.3 },
  "research-milestone":  { research: 0.9, longContext: 0.7, reasoning: 0.5 },
  "research-slice":      { research: 0.9, longContext: 0.7, reasoning: 0.5 },
  "plan-milestone":      { reasoning: 0.9, coding: 0.5 },
  "plan-slice":          { reasoning: 0.9, coding: 0.5 },
  "replan-slice":        { reasoning: 0.9, debugging: 0.6, coding: 0.5 },
  "reassess-roadmap":    { reasoning: 0.9, research: 0.5 },
  "complete-slice":      { instruction: 0.8, speed: 0.7 },
  "run-uat":             { instruction: 0.7, speed: 0.8 },
  "discuss-milestone":   { reasoning: 0.6, instruction: 0.7 },
  "complete-milestone":  { instruction: 0.8, reasoning: 0.5 },
};
```

### Scoring Function

```ts
function scoreModel(
  model: ModelCapabilities,
  requirements: Partial<Record<keyof ModelCapabilities, number>>,
): number {
  let weightedSum = 0;
  let weightSum = 0;
  for (const [dim, weight] of Object.entries(requirements)) {
    const capability = model[dim as keyof ModelCapabilities] ?? 50;
    weightedSum += weight * capability;
    weightSum += weight;
  }
  return weightSum > 0 ? weightedSum / weightSum : 50;
}
```

This produces a **weighted average** in the range `0–100`, where each dimension's contribution is proportional to its requirement weight. The output is directly comparable across models regardless of how many dimensions the requirement vector has.

**Tie-breaking:** When two models score within 2 points of each other, prefer the cheaper model (by `MODEL_COST_PER_1K_INPUT`). If cost is also equal, break ties by lexicographic model ID for determinism.

### Configuration Model

Built-in capability profiles ship as a data table alongside `MODEL_CAPABILITY_TIER` and `MODEL_COST_PER_1K_INPUT` in `model-router.ts`:

```ts
const MODEL_CAPABILITY_PROFILES: Record<string, ModelCapabilities> = {
  "claude-opus-4-6":     { coding: 95, debugging: 90, research: 85, reasoning: 95, speed: 30, longContext: 80, instruction: 90 },
  "claude-sonnet-4-6":   { coding: 85, debugging: 80, research: 75, reasoning: 80, speed: 60, longContext: 75, instruction: 85 },
  "claude-haiku-4-5":    { coding: 60, debugging: 50, research: 45, reasoning: 50, speed: 95, longContext: 50, instruction: 75 },
  "gpt-4o":              { coding: 80, debugging: 75, research: 70, reasoning: 75, speed: 65, longContext: 70, instruction: 80 },
  "gpt-4o-mini":         { coding: 55, debugging: 45, research: 40, reasoning: 45, speed: 90, longContext: 45, instruction: 70 },
  "gemini-2.5-pro":      { coding: 75, debugging: 70, research: 85, reasoning: 75, speed: 55, longContext: 90, instruction: 75 },
  "gemini-2.0-flash":    { coding: 50, debugging: 40, research: 50, reasoning: 40, speed: 95, longContext: 60, instruction: 65 },
  "deepseek-chat":       { coding: 75, debugging: 65, research: 55, reasoning: 70, speed: 70, longContext: 55, instruction: 65 },
  "o3":                  { coding: 80, debugging: 85, research: 80, reasoning: 92, speed: 25, longContext: 70, instruction: 85 },
};
```

Users can override capability profiles in `models.json` per provider:

```json
{
  "providers": {
    "anthropic": {
      "modelOverrides": {
        "claude-sonnet-4-6": {
          "capabilities": {
            "debugging": 90,
            "research": 85
          }
        }
      }
    }
  }
}
```

Partial overrides are deep-merged with built-in defaults. This uses the same `modelOverrides` path that already supports `contextWindow`, `cost`, and `compat` overrides.

### Profile Versioning

Built-in capability profiles are maintained alongside the existing `MODEL_CAPABILITY_TIER` and `MODEL_COST_PER_1K_INPUT` tables in `model-router.ts`. When the `@gsd/pi-ai` model catalog is updated with new models, the capability profile table must be updated in the same PR. A linting rule should flag any model present in `MODEL_CAPABILITY_TIER` but missing from `MODEL_CAPABILITY_PROFILES`.

Profiles are versioned implicitly by GSD release. The existing `models.json` `modelOverrides` mechanism allows users to correct stale defaults immediately without waiting for a GSD update.

### Extension-First Rollout

Capability-aware routing should be prototypable as an extension before moving to core. The current hook surface is **insufficient** for this:

- `before_provider_request` fires after model selection, at the API payload level — too late to swap model choice.
- `model_select` fires reactively when a model changes, not before selection — it cannot influence the choice.

**Required hook addition:** A `before_model_select` hook that fires within `selectAndApplyModel()` after tier classification but before `resolveModelForComplexity()`. This hook would receive:

```ts
interface BeforeModelSelectEvent {
  unitType: string;
  unitId: string;
  classification: ClassificationResult;
  taskMetadata: TaskMetadata;
  eligibleModels: string[];     // tier-filtered available models
  phaseConfig: ResolvedModelConfig;
}
```

Return value: `{ modelId: string } | undefined` (override selection, or undefined to use default).

This hook enables an extension to implement capability scoring externally, test it against real workloads, and validate behavior before the logic moves into `model-router.ts`.

**Rollout sequence:**

1. **Phase 1:** Add `before_model_select` hook and `TaskMetadata` to `ClassificationResult`. Ship built-in capability profile data table. No core routing changes.
2. **Phase 2:** Implement capability scoring as an extension that hooks `before_model_select`. Gather user feedback through routing history.
3. **Phase 3:** If behavior proves stable, move scoring into `resolveModelForComplexity()` in core. Extension hook remains for custom routing strategies.

### Observability

Every routing decision must be inspectable. The existing `RoutingDecision` interface is extended:

```ts
interface RoutingDecision {
  modelId: string;
  fallbacks: string[];
  tier: ComplexityTier;
  wasDowngraded: boolean;
  reason: string;
  // New fields:
  capabilityScores?: Record<string, number>;    // model ID → score
  taskRequirements?: Partial<Record<string, number>>;  // dimension → weight
  selectionMethod: "tier-only" | "capability-scored";
}
```

When verbose mode is on, the routing notification includes the top-scoring models and why the winner was selected:

```
Dynamic routing [S]: claude-sonnet-4-6 (scored 82.3 — coding:0.9×85, debugging:0.6×80)
  runner-up: gpt-4o (scored 78.1)
```

## Consequences

### Positive

#### 1. Better model-task fit

Routing decisions become based on the kind of work being done, not only how expensive or complex the work appears. A debugging task routes to the strongest debugger in the pool; a research task routes to the best synthesizer.

#### 2. Works across arbitrary model pools

The router no longer depends on a hardcoded vendor assumption. If a user has only Claude + Codex, it can still route intelligently between them. If the user adds Gemini or local models later, the same scoring system continues to work.

#### 3. Preserves all existing invariants

- **Downgrade-only semantics:** capability scoring never upgrades beyond the user's configured phase model.
- **Budget pressure:** unchanged — constrains tier eligibility before scoring runs.
- **Retry escalation:** unchanged — escalates tier, then scoring picks the best model in the new tier.
- **Fallback chains:** assembled the same way, with capability-scored winner as primary.

#### 4. Creates a testable, versionable contract for routing behavior

Capability profiles and task vectors are explicit data structures. Routing decisions are inspectable in verbose mode. The scoring function is a pure function suitable for deterministic unit tests.

#### 5. Opens the door to adaptive learning

Existing routing history (`routing-history.ts`) can later refine capability scores per task type. When a model consistently fails at a particular task shape, its effective score for that dimension decreases. This is a natural extension of the existing `getAdaptiveTierAdjustment()` mechanism.

#### 6. Graceful degradation

Models without capability profiles get uniform scores, producing the same cheapest-in-tier behavior as today. Zero behavior change for users who don't configure heterogeneous pools.

### Negative

#### 1. More metadata to maintain

Built-in model profiles will drift as model families evolve. Mitigation: profiles live in a single data table, versioned with GSD releases, with a lint rule for completeness.

#### 2. Scoring can create false precision

A `0–100` capability scale looks exact but is still heuristic. Mitigation: document profiles as "relative rankings, not benchmarks." The 2-point tie-breaking threshold prevents insignificant score differences from overriding cost optimization.

#### 3. More routing complexity

The current tier router is simple to explain and debug. Multi-dimensional scoring is more powerful but harder to reason about. Mitigation: verbose observability output shows scores and reasons. The `selectionMethod` field in routing decisions makes it clear whether capability scoring was active.

#### 4. Stronger test requirements

The router will need coverage for:

- profile loading and override merge rules (partial deep-merge from `modelOverrides`)
- `computeTaskRequirements()` with various unit types and metadata combinations
- scoring function correctness (weighted average, tie-breaking)
- interaction with tier eligibility filtering
- budget pressure applied before scoring, not conflicting with it
- fallback behavior when no scored model is eligible
- graceful degradation when no profiles exist (uniform scores)
- `before_model_select` hook contract (extension path)

#### 5. New hook surface to maintain

The `before_model_select` hook adds a new extension API contract that must be maintained across releases. Mitigation: the hook is narrowly scoped — one event type, optional return.

### Neutral / Migration

#### 1. Tier-based routing does not disappear

Complexity tiers remain as:

- the primary "how hard is this" signal that determines tier eligibility
- the fallback behavior for models without capability profiles
- the escalation path on retries (light → standard → heavy)

Capability scoring adds the "what kind of work" signal on top. The two systems are layered, not competing.

#### 2. Existing preferences continue to work

`dynamic_routing.tier_models` still works — it pins a specific model per tier, bypassing capability scoring for that tier. Per-phase model overrides (`models.planning`, `models.execution`, etc.) continue to set the ceiling. No existing configuration breaks.

#### 3. Documentation update required

`docs/dynamic-model-routing.md` must be updated to explain:

- what capability profiles are and how to override them
- how scoring interacts with tier routing
- how to read verbose routing output
- how to use `before_model_select` for custom routing extensions

## Risks

### 1. Hardcoded vendor stereotypes become stale

If the default profiles are not reviewed regularly, GSD will encode outdated assumptions about which models are "best" at which tasks.

**Mitigation:** Keep defaults in a single data table (not scattered conditionals). Lint for completeness against the model catalog. User overrides via `modelOverrides` provide immediate escape hatch. Document profiles as heuristic rankings, not benchmarks.

### 2. Budget logic and capability logic may conflict in user perception

The highest-scoring model may not be selected because budget pressure constrained the eligible tier. This could look inconsistent if the user doesn't understand the pipeline order.

**Mitigation:** Pipeline order is explicit and enforced in code:
1. Complexity classification determines tier
2. Budget pressure may downgrade tier
3. Tier-eligible models are filtered (downgrade-only from user ceiling)
4. Capability scoring ranks the eligible set
5. Cost tie-breaks within scoring threshold

Verbose output shows each step. The user sees "budget pressure: 85%" in the reason string when downgrade occurs.

### 3. Task-type classification may be too coarse initially

A unit type like `execute-task` contains many sub-shapes. The initial base vector plus metadata refinement may not distinguish all meaningful cases.

**Mitigation:** The `computeTaskRequirements()` function is designed for iterative refinement. The existing `TaskMetadata` already captures tags, complexity keywords, file counts, dependency counts, and code block counts. New metadata signals can be added to the existing `extractTaskMetadata()` without changing the scoring function. Routing history provides signal on where refinement is needed.

### 4. Unknown and custom models may score poorly by default

Users often bring custom provider IDs, local models, or vendor aliases that will not exist in the built-in profile table.

**Mitigation:** Unknown models receive uniform scores (50 across all dimensions), making capability scoring a no-op — they compete on cost within their tier, same as today. Users can add capability profiles via `modelOverrides` in `models.json` for models they know well.

### 5. Extension hook adds API surface

The `before_model_select` hook creates a contract that extensions may depend on.

**Mitigation:** The hook has a narrow, well-defined interface. It is additive (existing hooks unchanged). The return type is simple (`{ modelId } | undefined`). Breaking changes would be handled through the same extension API versioning as other hooks.

## Alternatives Considered

### A. Keep pure complexity-tier routing

Rejected because it optimizes cost within a tier but still treats meaningfully different models as interchangeable. The existing `MODEL_CAPABILITY_TIER` table already proves this is a recognized gap — it just stops at three buckets.

### B. Hardcode task → model mappings

Rejected because it breaks as soon as the user does not have the expected model. This is appropriate for a closed product with a fixed fleet, not for GSD's user-configured provider model.

### C. Route only by user-specified per-phase models

Rejected because it pushes all routing intelligence onto the user and does not adapt to retries, task subtype, or provider heterogeneity.

### D. Use capability-aware routing only as an extension, never in core

Not rejected as a starting point, but insufficient as the long-term architecture. Extension prototyping is the recommended first phase. However, coherent preferences, diagnostics, testing, and profile versioning will likely require core integration if the model proves valuable.

### E. Add `costEfficiency` as a capability dimension

Rejected because it conflates two concerns. If cost appears in both the scoring function and the budget constraint, the router has two competing cost signals that produce confusing behavior (e.g., a cheap model wins on `costEfficiency` score but then gets filtered out by budget pressure, or vice versa). Cost constrains eligibility; capability determines ranking.

### F. Use static requirement vectors per unit type (no metadata refinement)

Rejected because the existing `classifyUnitComplexity()` already proves that unit type alone is too coarse. A `execute-task` for docs vs. a `execute-task` for migration are categorically different. The metadata signals (tags, complexity keywords, file counts) that the classifier already extracts should inform requirement vectors.

## Appendix: Current Architecture Reference

For implementors, the current routing pipeline files:

| File | Role |
|------|------|
| `auto-dispatch.ts` | Rule table that determines unit type + prompt |
| `auto-model-selection.ts` | Orchestrates model selection for each dispatch |
| `complexity-classifier.ts` | Tier classification with task metadata analysis |
| `model-router.ts` | Tier → model resolution with downgrade-only semantics |
| `routing-history.ts` | Adaptive learning from success/failure patterns |
| `preferences-models.ts` | Per-phase model config resolution and fallbacks |
| `register-hooks.ts` | Hook registration including `before_provider_request` |

The capability scoring additions would primarily touch `model-router.ts` (profiles, scoring function) and `auto-model-selection.ts` (passing metadata to the router, new hook point).



================================================
FILE: docs/dev/ADR-005-multi-model-provider-tool-strategy.md
================================================
# ADR-005: Multi-Model, Multi-Provider, and Tool Strategy

**Status:** Accepted
**Date:** 2026-03-27
**Deciders:** Jeremy McSpadden
**Related:** ADR-004 (capability-aware model routing), ADR-003 (pipeline simplification), [Issue #2790](https://github.com/gsd-build/gsd-2/issues/2790)

## Context

PR #2755 lands capability-aware model routing (ADR-004), extending the router from a one-dimensional complexity-tier system to a two-dimensional system that scores models across 7 capability dimensions. GSD can now intelligently pick the best model for a task from a heterogeneous pool.

But model selection is only one piece of the multi-model puzzle. The system faces structural gaps as users configure diverse provider pools:

1. **Tool compatibility is assumed, not verified** — Every registered tool is sent to every model regardless of provider capabilities.
2. **No tool-aware model routing** — ADR-004 scores 7 capability dimensions but none encode whether a model can actually use the tools a task requires.
3. **Provider failover loses context fidelity** — Cross-provider switches silently degrade conversation quality (thinking blocks dropped, tool IDs remapped).
4. **Tool availability is static across a session** — The same tools are presented regardless of the selected model's capabilities.
5. **No provider capability registry** — Provider quirks are scattered across `*-shared.ts` files.

## Decision

Introduce a provider capability registry and tool compatibility layer that integrates with ADR-004's capability-aware model router.

### Design Principles

1. **Layered on ADR-004, not replacing it.** Capability scoring remains primary. This adds tool compatibility as a hard constraint.
2. **Hard constraints filter; soft scores rank.** Tool support is binary — it filters the eligible set before scoring.
3. **Provider knowledge is declarative, not scattered.** Provider capabilities move to an explicit registry.
4. **Tool sets adapt to model capabilities.** Active tool set adjusts when the router selects a different model.
5. **Graceful degradation preserved.** Unknown providers get full tool access — same as today.

### Implementation Phases

1. **Phase 1:** Provider Capabilities Registry (`packages/pi-ai/src/providers/provider-capabilities.ts`)
2. **Phase 2:** Tool Compatibility Metadata (extend `ToolDefinition` with `compatibility` field)
3. **Phase 3:** Tool-compatibility filter in routing pipeline + `ProviderSwitchReport` in `transform-messages.ts`
4. **Phase 3b:** Export `ProviderSwitchObserver` / `setProviderSwitchObserver` from `@gsd/pi-ai` and install a GSD observer that surfaces non-empty provider-switch reports as audit events, notifications, and in-memory stats.
5. **Phase 4:** `adjustToolSet` extension hook

### Provider Switch Visibility

`packages/pi-ai/src/providers/transform-messages.ts` produces a `ProviderSwitchReport` whenever replaying conversation history into a different provider requires context transformations. The report includes:

- `fromApi` / `toApi`
- `thinkingBlocksDropped`
- `thinkingBlocksDowngraded`
- `toolCallIdsRemapped`
- `syntheticToolResultsInserted`
- `thoughtSignaturesDropped`

`@gsd/pi-ai` exports `ProviderSwitchReport`, `ProviderSwitchObserver`, and `setProviderSwitchObserver(observer)`. The observer is single-subscriber by design; pass `undefined` to clear it. Observers receive only non-empty reports and are invoked synchronously after verbose stderr logging, with observer errors swallowed so telemetry cannot break model streaming.

GSD installs its observer during bootstrap. Each non-empty report is surfaced in three places:

- A UOK audit event when auto-mode trace context is active: `category: "model-policy"`, `type: "provider-switch"`, with the report fields in `payload`.
- A warning notification via the notification store, so provider-switch context loss is visible without `GSD_VERBOSE=1`.
- Process-local rollup stats from `getProviderSwitchStats()` in `src/resources/extensions/gsd/provider-switch-observer.ts`, including totals, per-trace buckets, the last report, and timestamps.

## Consequences

### Positive
- Eliminates silent tool failures when routing to incompatible providers
- Makes cross-provider routing safe by default
- Provider knowledge becomes queryable (registry vs scattered code)
- Cross-provider context loss becomes visible via `ProviderSwitchReport`

### Negative
- More metadata to maintain (provider capabilities, tool compatibility)
- Tool filtering adds a pipeline step (sub-millisecond, O(models × tools))
- Risk of over-filtering (mitigated: opt-in per tool, permissive defaults)

### Neutral
- Existing behavior unchanged without metadata
- ADR-004 scoring is unmodified
- Provider implementations simplify over time as registry replaces scattered workarounds

## Appendix: Architecture Reference

| File | Role |
|------|------|
| `packages/pi-ai/src/providers/register-builtins.ts` | Provider registration |
| `packages/pi-ai/src/providers/*-shared.ts` | Provider-specific handling |
| `packages/pi-ai/src/providers/transform-messages.ts` | Cross-provider normalization |
| `packages/pi-ai/src/types.ts` | Core types |
| `packages/pi-coding-agent/src/core/extensions/types.ts` | ToolDefinition, ExtensionAPI |
| `src/resources/extensions/gsd/model-router.ts` | Capability scoring (ADR-004) |
| `src/resources/extensions/gsd/auto-model-selection.ts` | Model selection orchestration |



================================================
FILE: docs/dev/ADR-006-extension-modularization.md
================================================
# ADR-006: Extension Modularization & Install Infrastructure

**Status:** Proposed (canonical text on GitHub)
**Date:** 2026-03-28
**Deciders:** Jeremy McSpadden
**Canonical location:** [Issue #2995](https://github.com/gsd-build/gsd-2/issues/2995)

## Pointer

This ADR's full text lives on the GitHub issue tracker so the discussion thread, label state (`accepted`, `priority: high`, `area: gsd-extension`), and milestone progress stay in one place. This file exists only so the local ADR sequence has no gap and cross-references from ADR-007 resolve to a real path.

## One-line summary

Modularize GSD2 across 7 milestones (v1.3–v1.9): split the 177K-LOC core `gsd` extension, introduce an `npm`-package-based extension install/uninstall/update surface, lazy-load provider SDKs, and ship a `~450 MB` default install instead of `~842 MB`.

See [Issue #2995](https://github.com/gsd-build/gsd-2/issues/2995) for the full Context / Decision / Milestone breakdown / Success Metrics / Risks / Consequences.



================================================
FILE: docs/dev/ADR-007-model-catalog-split.md
================================================
[Binary file]


================================================
FILE: docs/dev/ADR-008-gsd-tools-over-mcp-for-provider-parity.md
================================================
# ADR-008: Expose GSD Workflow Tools Over MCP for Provider Parity

**Status:** Accepted (implemented)
**Date:** 2026-04-09
**Implemented:** 2026-05 (all six phases + validation parity test)
**Deciders:** Jeremy McSpadden
**Related:** ADR-004 (capability-aware model routing), ADR-007 (model catalog split and provider API encapsulation), `src/resources/extensions/gsd/bootstrap/db-tools.ts`, `src/resources/extensions/claude-code-cli/stream-adapter.ts`, `packages/mcp-server/src/server.ts`

## Implementation status

| Phase | Status | Evidence |
|---|---|---|
| 1. Extract shared handlers | ✅ | `src/resources/extensions/gsd/tools/workflow-tool-executors.ts` exports 11 transport-neutral executors used by both native (`bootstrap/db-tools.ts`) and MCP (`packages/mcp-server/src/workflow-tools.ts`) registrations |
| 2. Workflow-tool MCP surface | ✅ | `packages/mcp-server/src/workflow-tools.ts` exposes the canonical set; aliases handled via `logAliasUsage` |
| 3. Port safety enforcement | ✅ | `enforceWorkflowWriteGate(toolName, projectDir, milestoneId)` runs at the head of every MCP handler |
| 4. Attach MCP to Claude Code | ✅ | `src/resources/extensions/claude-code-cli/stream-adapter.ts:1318` calls `buildWorkflowMcpServers(sdkCwd)` and passes `mcpServers` to the Anthropic Agent SDK session |
| 5. Provider capability gating | ✅ | `getWorkflowTransportSupportError(...)` in `src/resources/extensions/gsd/workflow-mcp.ts` fires pre-dispatch from `auto/phases.ts`, `guided-flow.ts`, and `auto-direct-dispatch.ts`. Fails early with an actionable error when the active provider can access neither native tools nor an MCP workflow surface. 27 tests in `workflow-mcp.test.ts` |
| 6. Prompts and docs transport-neutral | ✅ | Prompts under `src/resources/extensions/gsd/prompts/` reference the "DB-backed canonical write path" without prescribing transport; no manual-summary-fallback language remains anywhere |

Validation criterion #3 ("MCP-invoked workflow tools produce the same DB updates, rendered artifacts, and state transitions as native tool calls") is locked in by `packages/mcp-server/src/workflow-tools-parity.test.ts` (added 2026-05-10 via PR #5760).

## Context

GSD currently has two different tool surfaces:

1. **In-process extension tools** registered directly into the runtime via `pi.registerTool(...)`.
2. **An external MCP server** that exposes session orchestration and read-only project inspection.

This split is now creating a real provider compatibility problem.

### What exists today

The core GSD workflow tools are internal extension tools. Examples include:

- `gsd_summary_save`
- `gsd_plan_milestone`
- `gsd_plan_slice`
- `gsd_plan_task`
- `gsd_task_complete` / `gsd_complete_task`
- `gsd_slice_complete`
- `gsd_complete_milestone`
- `gsd_validate_milestone`
- `gsd_replan_slice`
- `gsd_reassess_roadmap`

These are registered in `src/resources/extensions/gsd/bootstrap/db-tools.ts` and related bootstrap files. GSD prompts assume these tools are available during discuss, plan, and execute flows.

Separately, `packages/mcp-server/src/server.ts` exposes a different tool surface:

- session control: `gsd_execute`, `gsd_status`, `gsd_result`, `gsd_cancel`, `gsd_query`, `gsd_resolve_blocker`
- read-only inspection: `gsd_progress`, `gsd_roadmap`, `gsd_history`, `gsd_doctor`, `gsd_captures`, `gsd_knowledge`

That MCP server is useful, but it is **not** a transport for the internal workflow/mutation tools.

### The current failure mode

The Claude Code CLI provider uses the Anthropic Agent SDK through `src/resources/extensions/claude-code-cli/stream-adapter.ts`. That adapter starts a Claude SDK session, but it does not forward the internal GSD tool registry into the SDK session, nor does it attach a GSD MCP server for those tools.

As a result:

- prompts tell the model to call tools like `gsd_complete_task`
- the tools exist in GSD
- but Claude Code sessions do not actually receive those tools

This produces a contract mismatch: the model is required to use tools that are unavailable in that provider path.

### Why this matters

This is not a one-off Claude Code bug. It reveals a deeper architectural issue:

- GSD’s core workflow contract is transport-specific
- prompt authors assume “internal extension tool availability”
- provider integrations do not all share the same execution surface

If GSD wants provider parity, its workflow tools need a transport-neutral exposure model.

## Decision

**Expose the GSD workflow tool contract over MCP as a first-class transport, and make MCP the compatibility layer for providers that cannot directly access the in-process GSD tool registry.**

This means:

1. GSD will keep its existing in-process tool registration for native runtime use.
2. GSD will add an MCP execution surface for the same workflow tools.
3. Both surfaces must call the same underlying business logic.
4. Provider integrations such as Claude Code will use the MCP surface when they cannot access native in-process tools directly.

The decision is explicitly **not** to replace the native tool system with MCP everywhere. MCP is the parity and portability layer, not the only runtime path.

## Decision Details

### 1. One handler layer, multiple transports

GSD tool behavior must not be implemented twice.

The transport-neutral business logic for workflow tools should be shared by:

- native extension tool registration (`pi.registerTool(...)`)
- MCP server tool registration

The MCP server should wrap the same handlers used by `db-tools.ts`, `query-tools.ts`, and related modules. This avoids logic drift and keeps validation, DB writes, file rendering, and recovery behavior consistent.

### 2. Add a workflow-tool MCP surface

GSD will expose the workflow tools required for discuss, planning, execution, and completion over MCP.

Initial minimum set:

- `gsd_summary_save`
- `gsd_decision_save`
- `gsd_plan_milestone`
- `gsd_plan_slice`
- `gsd_plan_task`
- `gsd_task_complete`
- `gsd_slice_complete`
- `gsd_complete_milestone`
- `gsd_validate_milestone`
- `gsd_replan_slice`
- `gsd_reassess_roadmap`
- `gsd_save_gate_result`
- selected read/query tools such as `gsd_milestone_status`

Aliases should be treated conservatively. MCP should prefer canonical names unless compatibility requires exposing aliases.

### 3. Preserve safety semantics

The current GSD safety model includes write gates, discussion gates, queue-mode restrictions, and state integrity guarantees.

Those guarantees must continue to apply when tools are invoked over MCP. In particular:

- MCP must not create a path that bypasses write gating
- MCP mutations must preserve the same DB/file/state invariants as native tools
- provider-specific fallback behavior must not allow manual summary writing in place of canonical completion tools

### 4. Make provider capability checks explicit

Before dispatching a workflow that requires GSD workflow tools, GSD should check whether the selected provider/session can access the required tool surface.

If a provider cannot access either:

- native in-process GSD tools, or
- the GSD MCP workflow tool surface

then GSD must fail early with a clear compatibility error rather than allowing execution to continue in a degraded, state-breaking mode.

### 5. Keep the existing session/read MCP server

The existing MCP server in `packages/mcp-server` remains valid. It serves a different purpose:

- remote session orchestration
- status/result polling
- filesystem-backed project inspection

The new workflow-tool MCP surface is complementary, not a replacement.

## Alternatives Considered

### Alternative A: Reroute away from Claude Code whenever tool-backed execution is needed

This would fix the immediate failure for multi-provider users, but it does not solve provider parity. It also fails completely for users who only have Claude Code configured.

**Rejected** because it treats the symptom, not the architectural gap.

### Alternative B: Hard-fail Claude Code and require another provider

This is a valid short-term guardrail and may still be used before MCP support is complete.

**Rejected as the long-term architecture** because it permanently excludes a supported provider from first-class GSD execution.

### Alternative C: Inject the internal GSD tool registry directly into the Claude Agent SDK without MCP

This would tightly couple GSD’s internal extension runtime to a provider-specific integration path. It would not generalize well to other providers or external tool clients.

**Rejected** because it creates a provider-specific bridge instead of a transport-neutral contract.

### Alternative D: Replace native GSD tools entirely with MCP

This would simplify the conceptual model, but it would force all runtimes through an external protocol boundary even when the native in-process path is faster and already works well.

**Rejected** because MCP is needed for portability, not because the native tool system is flawed.

## Consequences

### Positive

1. **Provider parity improves.** Providers that can consume MCP tools can participate in full GSD workflow execution.
2. **The workflow contract becomes transport-neutral.** Prompts can rely on capabilities rather than a specific runtime implementation detail.
3. **One compatibility story for external clients.** Claude Code, Cursor, and other MCP-capable clients can use the same workflow tool surface.
4. **Better long-term architecture.** Internal tools and external transports converge on shared handlers instead of diverging implementations.

### Negative

1. **Larger surface area to secure and test.** Mutation tools over MCP are higher risk than read-only inspection tools.
2. **Migration complexity.** Tool registration, gating, and handler extraction must be refactored carefully.
3. **Two transport paths must remain aligned.** Native and MCP invocation semantics must stay behaviorally identical.

### Neutral / Tradeoff

The system will now support:

- native in-process tool execution when available
- MCP-backed tool execution when native access is unavailable

That is more complex than a single-path system, but it is the cost of provider portability without sacrificing native runtime quality.

## Migration Plan

### Phase 1: Extract shared handlers

Refactor workflow tools so MCP and native registration can call the same transport-neutral functions.

Priority targets:

- `gsd_summary_save`
- `gsd_task_complete`
- `gsd_plan_milestone`
- `gsd_plan_slice`
- `gsd_plan_task`

### Phase 2: Stand up the workflow-tool MCP server

Add a new MCP surface for workflow tool execution. This may extend the existing MCP package or live as a sibling package, but it must be clearly separated from the current session/read API.

### Phase 3: Port safety enforcement

Move or centralize write gates and related policy checks so MCP mutations cannot bypass the existing safety model.

### Phase 4: Attach MCP workflow tools to Claude Code sessions

Update the Claude Code provider integration to pass a GSD-managed `mcpServers` configuration into the Claude Agent SDK session when required.

### Phase 5: Add provider capability gating

Before tool-dependent flows begin, verify that the active provider can access the required GSD workflow tools via either native registration or MCP.

### Phase 6: Update prompts and docs

Prompt contracts should remain strict about using canonical GSD completion/planning tools, but documentation and runtime messaging must no longer assume that only native in-process tool registration satisfies that contract.

## Validation

Success is defined by all of the following:

1. A Claude Code-backed execution session can complete a task using canonical GSD workflow tools without manual summary writing.
2. Native provider behavior remains unchanged.
3. MCP-invoked workflow tools produce the same DB updates, rendered artifacts, and state transitions as native tool calls.
4. Write-gate and discussion-gate protections still hold under MCP invocation.
5. When required capabilities are unavailable, GSD fails early with a precise compatibility error.

## Scope Notes

This ADR establishes the architectural direction. It does **not** require full MCP exposure of every historical alias or every auxiliary tool in the first implementation.

The first implementation should prioritize the minimum workflow tool set needed to make discuss/plan/execute/complete flows work safely for MCP-capable providers.



================================================
FILE: docs/dev/ADR-008-IMPLEMENTATION-PLAN.md
================================================
# ADR-008 Implementation Plan

**Related ADR:** [ADR-008-gsd-tools-over-mcp-for-provider-parity.md](/Users/jeremymcspadden/Github/gsd-2/docs/ADR-008-gsd-tools-over-mcp-for-provider-parity.md)
**Status:** Draft
**Date:** 2026-04-09

## Objective

Implement the ADR-008 decision by exposing the core GSD workflow tool contract over MCP, then wiring MCP-backed access into provider paths that cannot use the native in-process GSD tool registry directly.

The first usable outcome is:

- a Claude Code-backed execution session can complete a task using canonical GSD tools
- no manual summary-writing fallback is needed
- native provider behavior remains unchanged

## Non-Goals

- Replacing native in-process GSD tools with MCP
- Exporting every historical alias in the first rollout
- Reworking the entire session-oriented MCP server before proving the workflow-tool surface
- Supporting every provider path before Claude Code is working end-to-end

## Constraints

- Native and MCP tool paths must share business logic
- MCP must not bypass write-gate or discussion-gate protections
- Canonical GSD state transitions must remain DB-backed
- Provider capability mismatches must fail early, not degrade silently

## Workstreams

### 1. Shared Handler Extraction

Goal: separate business logic from transport registration.

Targets:

- `src/resources/extensions/gsd/bootstrap/db-tools.ts`
- `src/resources/extensions/gsd/bootstrap/query-tools.ts`
- `src/resources/extensions/gsd/tools/complete-task.ts`
- sibling modules used by planning/summary/validation tools

Deliverables:

- transport-neutral handler entrypoints for the minimum workflow tool set
- thin native registration wrappers that call those handlers
- thin MCP registration wrappers that call those handlers

Exit criteria:

- native tool behavior is unchanged
- no workflow tool logic is duplicated in MCP server code

### 2. Workflow-Tool MCP Surface

Goal: add an MCP server surface for real GSD workflow tools, distinct from the current session/read API.

Preferred first-cut tool set:

- `gsd_summary_save`
- `gsd_decision_save`
- `gsd_plan_milestone`
- `gsd_plan_slice`
- `gsd_plan_task`
- `gsd_task_complete`
- `gsd_slice_complete`
- `gsd_complete_milestone`
- `gsd_validate_milestone`
- `gsd_replan_slice`
- `gsd_reassess_roadmap`
- `gsd_save_gate_result`
- `gsd_milestone_status`

Likely files:

- `packages/mcp-server/src/server.ts` or a new sibling server package
- `packages/mcp-server/src/...` supporting modules
- shared tool-definition metadata if needed

Decisions to make during implementation:

- extend existing MCP package vs create `packages/mcp-gsd-tools-server`
- canonical names only vs selected alias export
- single combined server vs separate “session” and “workflow” server modes

Exit criteria:

- MCP tool discovery shows the minimum tool set
- each MCP tool invokes the shared handlers successfully in isolation

### 3. Safety and Policy Parity

Goal: ensure MCP mutations enforce the same rules as native tool calls.

Targets:

- `src/resources/extensions/gsd/bootstrap/write-gate.ts`
- any current tool-call gating hooks tied to native runtime only
- MCP wrapper layer before shared handler invocation

Required protections:

- discussion gate blocking
- queue-mode restrictions
- write-path restrictions
- canonical DB/file rendering order

Exit criteria:

- MCP cannot be used to bypass native write restrictions
- blocked native scenarios remain blocked over MCP

### 4. Claude Code Provider Integration

Goal: attach the GSD workflow-tool MCP surface to Claude Code sessions.

Targets:

- `src/resources/extensions/claude-code-cli/stream-adapter.ts`
- `src/resources/extensions/claude-code-cli/index.ts`

Expected work:

- build a GSD-managed `mcpServers` config for the Claude SDK session
- attach the workflow MCP server only when the session requires GSD tools
- keep current Claude Code streaming behavior intact

Exit criteria:

- Claude Code session can discover the GSD workflow MCP tools
- task execution path can call `gsd_task_complete` successfully

### 5. Capability Detection and Failure Path

Goal: refuse to start tool-dependent workflows when required capabilities are unavailable.

Targets:

- GSD dispatch / auto-mode preflight
- provider selection and routing checks
- user-facing compatibility errors

Required behavior:

- if native GSD tools are available, proceed
- else if GSD workflow MCP tools are available, proceed
- else fail fast with a precise message

Exit criteria:

- no execution prompt is sent that requires unavailable tools
- users with only unsupported capability combinations get a hard error, not a fake fallback

### 6. Prompt and Documentation Alignment

Goal: keep the workflow contract strict while removing transport assumptions from docs and runtime messaging.

Targets:

- `src/resources/extensions/gsd/prompts/execute-task.md`
- related planning/discuss prompts that reference tool availability
- provider and MCP docs

Rules:

- prompts should keep requiring canonical GSD completion/planning tools
- prompts should not imply “native in-process tool only”
- docs should explain native vs MCP-backed fulfillment paths

Exit criteria:

- prompt contract matches runtime reality
- no provider is told to use a tool surface it cannot access

## Phase Plan

## Phase 1: Spike and Handler Extraction

Scope:

- extract shared logic for `gsd_summary_save`, `gsd_task_complete`, and `gsd_milestone_status`
- prove native wrappers still work

Why first:

- these tools are enough to test end-to-end completion semantics without migrating the full catalog

Verification:

- existing native tests still pass
- new unit tests cover shared handler entrypoints directly

## Phase 2: Minimal Workflow MCP Server

Scope:

- expose the three extracted tools over MCP
- ensure discovery schemas are clean and canonical

Verification:

- MCP discovery returns all three tools
- direct MCP calls succeed against a fixture project

## Phase 3: Claude Code End-to-End Proof

Scope:

- wire the minimal workflow MCP server into the Claude SDK session
- run a single execution path that ends with task completion

Verification:

- Claude Code can call `gsd_task_complete`
- summary file, DB state, and plan checkbox update correctly

## Phase 4: Expand to Full Minimum Workflow Set

Scope:

- add planning, slice completion, milestone completion, roadmap reassessment, and gate result tools

Verification:

- discuss/plan/execute/complete lifecycle works over MCP for the supported flow set

## Phase 5: Capability Gating and UX Hardening

Scope:

- add preflight capability checks
- add clear error messaging for unsupported setups

Verification:

- unsupported provider/session combinations fail before execution starts

## Phase 6: Prompt and Doc Cleanup

Scope:

- align prompts and docs with the new transport-neutral contract

Verification:

- prompt references are accurate
- docs describe the supported architecture and limitations

## File-Level Starting Map

High-probability files for the first implementation:

- `src/resources/extensions/gsd/bootstrap/db-tools.ts`
- `src/resources/extensions/gsd/bootstrap/query-tools.ts`
- `src/resources/extensions/gsd/bootstrap/write-gate.ts`
- `src/resources/extensions/gsd/tools/complete-task.ts`
- `src/resources/extensions/claude-code-cli/stream-adapter.ts`
- `src/resources/extensions/claude-code-cli/index.ts`
- `packages/mcp-server/src/server.ts`
- `packages/mcp-server/src/session-manager.ts`
- `packages/mcp-server/README.md`
- `src/resources/extensions/gsd/prompts/execute-task.md`

## Testing Strategy

### Unit

- shared handlers
- MCP wrapper adapters
- gating / capability-check helpers

### Integration

- direct MCP tool invocation against fixture projects
- native tool invocation regression coverage
- Claude Code provider path with MCP attached

### End-to-End

- plan or execute a small fixture task and complete it through canonical GSD tools
- confirm DB row, rendered summary, and plan state stay in sync

## Risks

### Risk 1: Logic Drift

If native and MCP wrappers each evolve their own behavior, parity will collapse quickly.

Mitigation:

- shared handler extraction before broad MCP exposure

### Risk 2: Safety Regression

If MCP becomes a side door around native gating, the architecture is worse than before.

Mitigation:

- centralize or reuse gating checks before shared handler invocation

### Risk 3: Overly Broad First Rollout

Exporting every tool and alias immediately increases scope and test burden.

Mitigation:

- ship a minimal workflow tool set first

### Risk 4: Claude SDK Session Wiring Complexity

Attaching MCP servers dynamically may expose edge cases around cwd, permissions, or subprocess lifecycle.

Mitigation:

- prove a narrow spike with 2-3 tools before expanding

## Exit Criteria for ADR-008

ADR-008 is considered implemented when:

1. Claude Code-backed execution can use canonical GSD workflow tools over MCP.
2. Native provider behavior remains intact.
3. Shared handlers back both native and MCP invocation.
4. Gating and state integrity protections apply equally to MCP mutations.
5. Capability checks prevent prompts from requiring unavailable tools.

## Recommended Next Task

Start with a narrow spike:

1. Extract shared handlers for `gsd_summary_save`, `gsd_task_complete`, and `gsd_milestone_status`.
2. Expose those tools through a minimal workflow MCP server.
3. Attach that MCP server to Claude Code sessions.
4. Prove end-to-end task completion on a fixture project.



================================================
FILE: docs/dev/ADR-009-IMPLEMENTATION-PLAN.md
================================================
# ADR-009 Implementation Plan

**Related ADR:** [ADR-009-orchestration-kernel-refactor.md](/Users/jeremymcspadden/Github/gsd-2/docs/dev/ADR-009-orchestration-kernel-refactor.md)  
**Status:** Completed (closure baseline established; emergency fallback retained)  
**Date:** 2026-04-14  
**Target Window:** 8-10 waves (incremental, no big-bang rewrite)

## Objective

Implement ADR-009 by migrating GSD orchestration internals to a Unified Orchestration Kernel (UOK) with six control planes:

1. Plan
2. Execution
3. Model
4. Gate
5. GitOps
6. Audit

without breaking existing CLI/web/MCP workflows.

The first production-safe outcome is:

- existing auto-mode behavior remains stable
- new kernel contracts exist behind feature flags
- every turn is traceable with deterministic gate outcomes

## Non-Goals

- Rewriting user-facing command surfaces
- Replacing all legacy modules in a single PR
- Introducing new provider auth flows that bypass existing compliance boundaries
- Forcing `burn-max` behavior as default

## Constraints

- Maintain current runtime compatibility and defaults
- Preserve existing state-on-disk and DB-backed transition model
- Keep provider-agnostic behavior while enforcing provider-specific policy constraints
- All migration steps must be reversible behind flags
- High-risk changes require parity tests against existing behavior

## Program Structure

Implementation is organized into parallel workstreams and executed in waves.

### Workstream A: Kernel Contracts and Orchestrator Spine

Goal: define typed contracts and a new orchestration spine without changing behavior.

Primary targets:

- `src/resources/extensions/gsd/auto.ts`
- `src/resources/extensions/gsd/auto/loop.ts`
- `src/resources/extensions/gsd/auto/types.ts`
- `src/resources/extensions/gsd/auto/session.ts`

Deliverables:

- `TurnContract` and `TurnResult` types
- `GateResult` envelope
- kernel entrypoint that wraps current dispatch loop via adapter

### Workstream B: Gate Plane

Goal: normalize all checks into a unified gate runner.

Primary targets:

- `src/resources/extensions/gsd/verification-gate.ts`
- `src/resources/extensions/gsd/auto-verification.ts`
- `src/resources/extensions/gsd/pre-execution-checks.ts`
- `src/resources/extensions/gsd/post-execution-checks.ts`
- `src/resources/extensions/gsd/milestone-validation-gates.ts`

Deliverables:

- unified gate registry and execution API
- deterministic failure classes and retry policies
- explicit terminal status persistence

### Workstream C: Model Plane + Policy Engine

Goal: enable any-model-any-phase through requirement-based selection plus policy filtering.

Primary targets:

- `src/resources/extensions/gsd/model-router.ts`
- `src/resources/extensions/gsd/auto-model-selection.ts`
- `src/resources/extensions/gsd/preferences-models.ts`
- `src/resources/extensions/gsd/model-cost-table.ts`
- `src/resources/extensions/gsd/custom-execution-policy.ts`

Deliverables:

- requirement vector builder for units
- policy filter before capability scoring
- new `burn-max` profile
- policy decision audit events

### Workstream D: Execution Graph (Agents/Subagents/Parallel/Teams)

Goal: move to one DAG scheduler contract.

Primary targets:

- `src/resources/extensions/gsd/reactive-graph.ts`
- `src/resources/extensions/gsd/slice-parallel-orchestrator.ts`
- `src/resources/extensions/gsd/parallel-orchestrator.ts`
- `src/resources/extensions/gsd/graph.ts`
- `src/resources/extensions/gsd/unit-runtime.ts`

Deliverables:

- typed node kinds (`unit`, `hook`, `subagent`, `team-worker`, `verification`, `reprocess`)
- shared dependency/conflict resolver
- scheduler adapter for current parallel and reactive paths

### Workstream E: GitOps Transaction Layer

Goal: guarantee git action and metadata record per turn.

Primary targets:

- `src/resources/extensions/gsd/git-service.ts`
- `src/resources/extensions/gsd/auto-post-unit.ts`
- `src/resources/extensions/gsd/auto-unit-closeout.ts`
- `src/resources/extensions/gsd/auto-worktree.ts`

Deliverables:

- `turn-start -> stage -> checkpoint -> publish -> record` transaction API
- configurable turn action mode (`commit|snapshot|status-only`)
- closeout gate integration for git failures

### Workstream F: Unified Audit Plane

Goal: unify journal/activity/metrics into a causal event model.

Primary targets:

- `src/resources/extensions/gsd/journal.ts`
- `src/resources/extensions/gsd/activity-log.ts`
- `src/resources/extensions/gsd/metrics.ts`
- `src/resources/extensions/gsd/workflow-logger.ts`
- `src/resources/extensions/gsd/gsd-db.ts`

Deliverables:

- common `AuditEventEnvelope`
- trace/turn IDs on all events
- append-only JSONL raw log + DB projection index

### Workstream G: Plan Plane v2

Goal: formal multi-round clarify/research/draft/compile flow.

Primary targets:

- `src/resources/extensions/gsd/guided-flow.ts`
- `src/resources/extensions/gsd/preparation.ts`
- `src/resources/extensions/gsd/auto/phases.ts`
- `src/resources/extensions/gsd/auto-prompts.ts`
- prompt templates under `src/resources/extensions/gsd/prompts/`

Deliverables:

- bounded multi-round question loop
- plan compile step producing executable unit graph
- plan gate fail-closed behavior

## Wave Plan (Execution Order)

## Wave 0: Baseline and Flag Scaffolding

Purpose: establish safe rollout controls and baseline telemetry.

Tasks:

- Add feature flags:
  - `uok.enabled`
  - `uok.gates.enabled`
  - `uok.model_policy.enabled`
  - `uok.execution_graph.enabled`
  - `uok.gitops.enabled`
  - `uok.audit_unified.enabled`
  - `uok.plan_v2.enabled`
- Add no-op kernel wrapper around current auto loop
- Add baseline metrics for parity comparison

Exit criteria:

- zero behavior change with all flags off
- parity telemetry collected for existing loop

Verification:

- `npm run typecheck:extensions`
- `npm run test:unit`

## Wave 1: Contract Extraction

Purpose: create stable internal API boundaries.

Tasks:

- Introduce:
  - `TurnContract`
  - `UnitExecutionContext`
  - `GateResult`
  - `FailureClass`
  - `TurnCloseoutRecord`
- Adapter layer from legacy auto loop into contracts
- Add contract fixtures and serialization tests

Exit criteria:

- current auto dispatch runs through adapter path without behavior change
- all turn outcomes represented in structured result type

Verification:

- targeted tests in `src/resources/extensions/gsd/tests/*auto*`
- `npm run test:unit`

## Wave 2: Gate Plane Unification

Purpose: centralize pre/in/post checks and retries.

Tasks:

- Build `gate-runner` and gate registry
- Port existing checks into registered gates:
  - policy/input/execution/artifact/verification/closeout
- Implement deterministic retry matrix by failure class

Exit criteria:

- every unit passes through gate runner
- explicit gate result persisted for pass/fail/retry/manual-attention

Verification:

- extend `verification-gate.test.ts`
- extend `validation-gate-patterns.test.ts`
- add integration tests for retry escalation

## Wave 3: Model Plane + Policy Filter

Purpose: enable requirement-based selection constrained by policy.

Tasks:

- Add requirement extraction from unit metadata
- Insert policy filter before model scoring
- Add `burn-max` token profile wiring
- Emit model policy allow/deny events

Exit criteria:

- units can select any eligible model across phases
- policy-denied routes fail before dispatch
- fallback chains remain deterministic

Verification:

- extend `model-cost-table.test.ts`
- extend model routing tests (`interactive-routing-bypass`, `tool-compatibility`, related router suites)
- add policy denial regression tests

## Wave 4: Execution Graph Scheduler

Purpose: unify hooks/subagents/parallel/team work under one scheduler contract.

Tasks:

- Introduce graph scheduler facade
- Map reactive execution nodes to shared node model
- Map slice/milestone parallel orchestrators onto scheduler
- Add file IO conflict lock integration

Exit criteria:

- same task set can execute in deterministic single-worker or parallel graph mode
- no deadlock under known reactive/parallel fixtures

Verification:

- `slice-parallel-orchestrator.test.ts`
- `slice-parallel-conflict.test.ts`
- `sidecar-queue.test.ts`
- integration: `src/resources/extensions/gsd/tests/integration/*.test.ts`

## Wave 5: GitOps Transactions Per Turn

Purpose: enforce turn-level git actions and closeout discipline.

Tasks:

- Implement turn transaction API
- Wire turn transactions into auto closeout path
- Add configurable `turn_action` and `turn_push` semantics
- Persist git transaction metadata into audit stream

Exit criteria:

- each turn has a git transaction record
- blocked git states surface as closeout gate failures

Verification:

- `git-service` integration tests
- worktree-related integration suites
- closeout and merge regression suites

## Wave 6: Unified Audit Plane

Purpose: converge logging/metrics/journal into one causal model.

Tasks:

- Define `AuditEventEnvelope` schema
- Add `traceId`, `turnId`, `causedBy` to event emitters
- Write projection pipeline into DB index tables
- Maintain append-only raw JSONL logs

Exit criteria:

- action-level traceability across model/tool/git/gate/test events
- legacy readers remain functional through compatibility projection

Verification:

- `workflow-logger*.test.ts`
- `workflow-events.test.ts`
- `journal` and `metrics` regression tests

## Wave 7: Plan Plane v2

Purpose: deliver full multi-round planning and compile-to-unit graph.

Tasks:

- Implement bounded clarify rounds
- Add explicit research synthesis stage
- Add plan compile stage with dependency graph output
- Add plan gate with fail-closed checks

Exit criteria:

- full roadmap and unit graph produced before execution begins (when enabled)
- invalid plans cannot proceed to execution

Verification:

- prompt and plan parsing tests
- planning tool tests (`plan-milestone`, `plan-slice`, `plan-task`)
- discuss/guided flow regression tests

## Wave 8: Legacy Branch Retirement + Default Flip

Purpose: reduce maintenance burden and enable UOK as default.

Tasks:

- remove superseded code paths in `auto.ts`, `auto-phases`, and legacy closeout paths
- keep legacy fallback behind emergency flag for one release window
- update docs and preferences reference

Exit criteria:

- UOK default in stable channel
- no critical parity regressions in one full release cycle

Verification:

- full `npm test`
- smoke + integration suites
- targeted manual UAT for CLI/web/headless

## Testing and Validation Matrix

### 1. Unit

- contract serialization
- gate runner behavior by failure class
- model policy filter decisions
- git transaction state machine
- event envelope schema validation

### 2. Integration

- auto dispatch across plan/execute/complete/reassess/uat
- worktree/branch/none isolation behaviors
- parallel and reactive execution parity
- policy-denied dispatch fast-fail

### 3. End-to-End

- greenfield milestone from discuss -> plan -> execute -> complete -> merge
- failure reprocessing (test failure, tool failure, model failure)
- full audit trace reconstruction by `traceId`
- provider compliance scenarios (allowed vs denied paths)

### 4. Parity Harness

- replay selected historical workflows against legacy and UOK paths
- compare:
  - state transitions
  - produced artifacts
  - gate decisions
  - commit outcomes

## Rollout Strategy

### Stages

1. Internal dogfood with flags on
2. Beta cohort opt-in via project preference
3. General availability with flags default-on
4. Legacy fallback removed after stability window

### Safety Controls

- runtime kill-switch for each plane
- release-note explicit migration warnings
- auto-rollback trigger on critical regressions (gates, git integrity, state corruption)

## Data and Schema Changes

Expected schema additions:

- audit projection tables in `gsd.db`
- gate result persistence tables
- turn transaction metadata

Rules:

- additive migrations only until Wave 8
- keep backwards-compatible readers during migration window

## Dependencies

1. Stable contract definitions before gate/model/scheduler rewires
2. Gate plane before gitops hard enforcement
3. Model policy engine before enabling any-model-any-phase by default
4. Audit envelope before legacy logger removal
5. Plan v2 before enforcing front-loaded planning defaults

## Risk Register

### Risk 1: Hidden Coupling in Auto Loop

Impact: migration bugs due to implicit side effects.  
Mitigation: adapter-first extraction and parity harness before path switch.

### Risk 2: Parallel Deadlocks

Impact: blocked runs or inconsistent state.  
Mitigation: graph-level deadlock checks, IO lock tests, staged rollout behind flags.

### Risk 3: Git Noise / Team Workflow Friction

Impact: commit churn and review overhead.  
Mitigation: milestone squash defaults and configurable turn transaction modes.

### Risk 4: Policy Drift Across Providers

Impact: compliance regressions.  
Mitigation: provider policy registry tests and release checklist gates.

### Risk 5: Telemetry Volume Growth

Impact: storage/perf pressure in long-running projects.  
Mitigation: append-only raw + indexed projection + retention policies.

## Definition of Done (ADR-009)

ADR-009 closure matrix (updated 2026-04-16):

| DoD Criterion | Status | Evidence |
| --- | --- | --- |
| 1. UOK path is default and stable. | Complete | `resolveUokFlags()` defaults UOK planes to enabled; kernel path dispatch by default in `runAutoLoopWithUok()` and `startAuto()`. (`src/resources/extensions/gsd/uok/flags.ts`, `src/resources/extensions/gsd/uok/kernel.ts`, `src/resources/extensions/gsd/auto.ts`) |
| 2. All units execute through unified gate runner. | Complete | Shared phased loop path remains authoritative in `auto/loop.ts` (`runGuards`, `runFinalize`, phase observer telemetry). |
| 3. Model selection supports any eligible model in any phase with policy enforcement. | Complete | ADR-009 model plane behavior is active in runtime selection/policy integration. (`src/resources/extensions/gsd/auto-model-selection.ts`, `src/resources/extensions/gsd/custom-execution-policy.ts`) |
| 4. Hooks/agents/subagents/parallel/team execution runs through one scheduler contract. | Complete | Dispatch kind mapping + scheduler facade for `unit|hook|subagent|team-worker|verification|reprocess` in `runUnitPhaseViaContract()`. (`src/resources/extensions/gsd/auto/loop.ts`) |
| 5. Turn-level git transaction record exists for every executed turn. | Complete | Turn observer emits git transaction stages + closeout records via UOK gitops adapter. (`src/resources/extensions/gsd/uok/loop-adapter.ts`, `src/resources/extensions/gsd/uok/gitops.ts`) |
| 6. Unified audit events provide causal traceability across orchestration, model, tool, git, and test actions. | Complete | Unified audit envelope + emission path wired through UOK observer/kernel entry. (`src/resources/extensions/gsd/uok/audit.ts`, `src/resources/extensions/gsd/uok/kernel.ts`) |
| 7. Plan v2 can produce a complete unit graph with fail-closed plan gate. | Complete | Plan v2 control plane remains default-on with bounded flow + gate coverage in current planner pipeline. (`src/resources/extensions/gsd/uok/flags.ts`, planning/gate suites) |
| 8. `burn-max` profile is available and policy-safe. | Complete | Profile and policy bounds are enforced via preferences + model policy filtering. (`src/resources/extensions/gsd/auto-model-selection.ts`, `src/resources/extensions/gsd/preferences-models.ts`) |
| 9. Legacy orchestration branches are retired or behind emergency-only fallback. | Complete | Legacy path remains only under explicit emergency controls (`uok.legacy_fallback.enabled`, `GSD_UOK_FORCE_LEGACY`, `GSD_UOK_LEGACY_FALLBACK`). (`src/resources/extensions/gsd/uok/flags.ts`) |
| 10. CLI/web/headless behavior remains user-compatible. | Complete | Existing startup/dispatch surfaces preserved; integration coverage remains green in CI. (`src/resources/extensions/gsd/auto.ts`, `src/tests/integration/*.test.ts`) |

Targeted closure/parity tests:

- `src/resources/extensions/gsd/tests/uok-flags.test.ts`
- `src/resources/extensions/gsd/tests/uok-execution-graph.test.ts`
- `src/resources/extensions/gsd/tests/uok-kernel-path.test.ts`

## Recommended Immediate Next Tasks (Week 1)

1. Add Wave 0 feature flags and default-off wiring.
2. Introduce contract types and adapter shell (Wave 1 scaffolding).
3. Add parity telemetry capture for legacy loop baseline.
4. Land initial tests for contract serialization and turn result envelopes.



================================================
FILE: docs/dev/ADR-009-orchestration-kernel-refactor.md
================================================
# ADR-009: Unified Orchestration Kernel Refactor

**Status:** Accepted (implemented; emergency legacy fallback retained)
**Date:** 2026-04-14
**Deciders:** Jeremy McSpadden, GSD Core Team
**Related:** ADR-001 (worktree architecture), ADR-003 (pipeline simplification), ADR-004 (capability-aware routing), ADR-005 (multi-provider strategy), ADR-008 (tools over MCP)

## Context

GSD already ships many advanced features:

- dynamic model routing and multi-provider support
- hooks (`pre_dispatch_hooks`, `post_unit_hooks`)
- subagents and parallel execution
- worktree/branch isolation and automated git flows
- per-unit metrics and cost ledgers
- activity logs and structured journal events
- verification retries and failure recovery

The current limitation is not missing capability. The limitation is **distribution of control logic across large, mixed-concern modules**, especially in auto-mode and related orchestration files. This raises change risk, creates duplicated policy paths, and slows the introduction of stronger guarantees.

The target requirements for the next architecture are:

1. User can use any available model during any phase.
2. First-class hooks, agents, sub-agents, team execution, and parallel workflows.
3. Git actions on every turn with deterministic, auditable behavior.
4. Logging of every action with causal traceability.
5. Long upfront planning via multi-round questioning and research.
6. Plan slicing and controlled dispatch through strict gate validation.
7. Deterministic failure reprocessing loops.
8. Automatic testing during build and gate transitions.
9. Explicit token usage controls including a high-burn mode.
10. Enforced compliance with provider/model terms of service.

## Decision

Refactor GSD into a **Unified Orchestration Kernel (UOK)** with explicit control planes, typed contracts, and an incremental strangler migration. This is a staged architectural replacement of orchestration internals, not a rewrite of user-facing CLI/web/MCP surfaces.

### Core Architectural Model

The orchestrator is split into six control planes:

1. **Plan Plane**
2. **Execution Plane**
3. **Model Plane**
4. **Gate Plane**
5. **GitOps Plane**
6. **Audit Plane**

Each dispatched unit (turn) executes through a single deterministic pipeline:

```text
Discover/Clarify/Research -> Plan Compile -> Model Select -> Execute -> Validate -> Git Transaction -> Persist Audit -> Next Unit
```

## Detailed Design

### 1) Plan Plane: Multi-Round Front-Loaded Planning

Add a formal planning lifecycle:

1. `discover`: codebase and state scan
2. `clarify`: multi-round user questions (bounded rounds, explicit stop condition)
3. `research`: internal and external synthesis
4. `draft-plan`: produce full roadmap and milestones
5. `compile`: slice into executable units with IO boundaries
6. `plan-gate`: reject/repair invalid plans before execution starts

Required outputs:

- `ROADMAP.md` (complete)
- per-milestone slice graph
- per-task executable unit specs
- requirement trace matrix (requirement -> unit(s) -> verification)
- plan risk register

Plan gate fails closed if:

- missing acceptance criteria
- missing verification strategy
- cyclic task dependencies
- unowned artifacts
- missing rollback/recovery semantics for risky units

### 2) Execution Plane: Agents, Sub-Agents, Teams, Parallel

Unify all execution into a typed DAG scheduler.

Node kinds:

- `unit` (single execution task)
- `hook`
- `subagent`
- `team-worker`
- `verification`
- `reprocess`

Edges express:

- hard dependencies
- resource conflicts (file-level IO locks)
- ordering constraints (gate-before-merge, test-before-closeout)

Execution modes:

- single-worker deterministic mode
- multi-worker parallel mode
- team mode (shared repo, unique milestone IDs, gated merge)

This removes ad-hoc parallel behavior and makes sub-agent and team paths first-class scheduler decisions.

### 3) Model Plane: Any Model in Any Phase

Replace rigid phase->model assumptions with **requirement-based eligibility**.

Selection pipeline:

1. gather phase/unit requirements (capabilities, context size, latency profile)
2. gather eligible models from configured providers
3. apply hard policy filters (provider auth, TOS, tool compatibility, org rules)
4. apply soft scoring (capability vectors, budget profile, historical outcomes)
5. choose primary + fallback chain

Rules:

- Any model can run any phase if it passes policy and capability constraints.
- User pins remain hard ceilings only when configured explicitly.
- Unknown models are allowed with conservative default capability scores.

Add model intent profiles:

- `economy` (lowest cost)
- `balanced`
- `quality`
- `burn-max` (highest compute/token burn within policy and budget limits)

### 4) Gate Plane: Controlled Dispatch and Reprocessing

All units pass explicit gates:

1. `policy-gate` (provider/tool/TOS/security checks)
2. `input-gate` (unit contract completeness, artifact readiness)
3. `execution-gate` (runtime guardrails, timeout strategy, tool allowlist)
4. `artifact-gate` (expected outputs and format validation)
5. `verification-gate` (lint/test/typecheck/security checks)
6. `closeout-gate` (state transition safety + git transaction outcome)

Gate outcomes:

- `pass`
- `retryable-fail`
- `hard-fail`
- `manual-attention`

Failure reprocessing matrix (deterministic):

- code failure -> targeted fix prompt + bounded retry
- test failure -> impacted test fix loop
- tool failure -> alternate tool/provider fallback
- model failure -> fallback model chain
- policy failure -> immediate hard stop and explicit reason

Retry policy:

- bounded attempts per gate
- escalating strategy per attempt
- terminal state persisted with full evidence

### 5) GitOps Plane: Git Action Every Turn

Every dispatched unit is wrapped in a git transaction:

1. `turn-start`: capture branch/worktree status and dirty-state snapshot
2. `turn-exec`: run unit
3. `turn-stage`: stage relevant changes
4. `turn-checkpoint`: commit checkpoint or structured no-op record
5. `turn-publish`: optional push per policy
6. `turn-record`: write commit metadata into audit ledger

Defaults:

- checkpoint commit each turn in milestone branch/worktree
- squash on milestone merge to keep main history clean

Configurable strictness:

- `git.turn_action: commit|snapshot|status-only`
- `git.turn_push: never|milestone|always`

If a repo state blocks commit (e.g., conflicts), turn fails at closeout gate with explicit diagnostics.

### 6) Audit Plane: Log Every Action

Promote current activity/journal into a single causal event model.

Event classes:

- orchestrator (`dispatch`, `gate-result`, `state-transition`)
- model (`selection`, `fallback`, `provider-switch`)
- tool (`call`, `result`, `error`)
- git (`status`, `stage`, `commit`, `merge`, `push`)
- test (`command`, `result`, `retry`)
- policy (`allow`, `deny`, `warning`)
- cost (`tokens`, `cost`, `cache-hit`, `budget-pressure`)

Every event includes:

- `eventId`
- `traceId` (session)
- `turnId` (unit)
- `causedBy` reference
- timestamp
- durable payload

Storage:

- append-only JSONL + indexed SQLite projection for queryability
- no destructive rewrites of source audit logs

## Compliance and TOS Enforcement

Introduce a provider policy engine as a hard dependency of the policy gate.

Provider policy definition includes:

- allowed auth modes
- prohibited token exchange paths
- tool/protocol constraints
- subscription vs API usage boundaries
- model-specific restrictions

Enforcement rules:

- deny disallowed auth/routing before dispatch
- deny model selection if provider constraints are not met
- emit policy evidence events on every allow/deny decision

This formalizes current compliance work (notably Anthropic/Claude Code boundaries) into a reusable engine rather than scattered checks.

## Automatic Testing Strategy

Testing becomes mandatory at three levels:

1. **Per-turn**: impacted tests + lint/typecheck subset
2. **Per-slice closeout**: full slice verification profile
3. **Per-milestone closeout**: full suite (or policy-defined release profile)

Verification commands become declarative policies by unit type, not ad-hoc shell lists only.

## Token Strategy and Burn-Max Mode

Existing token optimization modes remain, plus explicit high-burn profile.

`burn-max` behavior:

- maximize context inclusion
- prefer high-capability models
- enable deeper critique/review passes
- increase planning/research depth

Hard limits still apply:

- budget ceiling and enforcement rules
- provider rate limits
- TOS/policy constraints

The system must never bypass provider restrictions to increase usage.

## Migration Plan (Strangler Refactor)

No big-bang rewrite. Migrate in waves with compatibility adapters.

### Wave 0: Contracts and Telemetry Baseline

- define turn contract and gate result schemas
- add trace IDs/turn IDs to current paths
- keep behavior unchanged

### Wave 1: Gate Plane Extraction

- extract gate runner from auto loop
- route existing checks through unified gate API

### Wave 2: Model Plane Unification

- requirement-based model selection
- policy filter insertion before scoring
- preserve existing model config semantics

### Wave 3: Scheduler and Execution Graph

- introduce DAG scheduler
- map existing subagent/parallel features to graph nodes
- enable graph mode behind flag

### Wave 4: GitOps Transaction Layer

- enforce turn-level git actions
- add deterministic checkpoint behavior

### Wave 5: Audit Plane Consolidation

- unify journal/activity/metrics events under common envelope
- add query projection

### Wave 6: Plan Plane v2

- multi-round clarify/research planner
- compiled unit graph + plan gate

### Wave 7: Legacy Path Retirement

- remove obsolete branches in `auto.ts` and related modules
- keep CLI/API compatibility

## Module Extraction Targets

Primary decomposition targets:

- `auto.ts` -> orchestrator kernel + adapters
- `auto-prompts.ts` -> plan compiler + prompt renderers
- `state.ts` -> state query service + immutable state views
- `gsd-db.ts` -> data access layer + event projection store
- `auto-post-unit.ts` / `auto-verification.ts` -> closeout gate services

## Acceptance Criteria

The refactor is accepted when all conditions are true:

1. Any configured model can be selected in any phase when policy permits.
2. Hooks, agents, sub-agents, teams, and parallel all execute under one scheduler contract.
3. Every turn produces at least one git action record and auditable turn closeout.
4. Every dispatch and action is traceable by `traceId` and `turnId`.
5. Multi-round planning produces a full executable unit graph before execution.
6. Gate outcomes are explicit, deterministic, and persisted.
7. Failure reprocessing uses typed failure classes, not generic retries.
8. Automatic tests run per policy on every turn/slice/milestone gate.
9. Token usage is tracked at turn granularity with burn-max profile support.
10. Policy engine blocks TOS-violating routes and records evidence.

## Closure Update (2026-04-16)

ADR-009 closure is complete, with emergency fallback retained as a release safety valve.

- Kernel-native dispatch is the default UOK runtime path.
- Unit/hook/subagent/team-worker/verification/reprocess dispatch now routes through the scheduler contract facade.
- Legacy execution remains available only through explicit emergency fallback controls.
- CI parity coverage includes explicit kernel-vs-legacy path assertions.

Evidence is tracked in the implementation plan matrix:
- [ADR-009-IMPLEMENTATION-PLAN.md](/Users/jeremymcspadden/Github/gsd-2/docs/dev/ADR-009-IMPLEMENTATION-PLAN.md)

## Consequences

### Positive

- Stronger reliability through fail-closed gates
- Faster feature delivery by isolating orchestration concerns
- Clear compliance and audit posture
- Better debuggabilit