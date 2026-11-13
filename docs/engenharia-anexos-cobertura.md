# Engenharia dos anexos e cobertura — o que usamos, o que não usamos, o que cobrimos

- **Data**: 13/Nov/2026 · **Fontes**: snapshots locais `agent-skills @ e74dc8a`
  (MIT + CC-BY-4.0) e `harness-toolkit @ d293c7f` (Elastic License 2.0 — conceito
  sim, código não), lidos arquivo a arquivo; cruzados com o código do KIRA.
- **Pergunta respondida**: *"tem algo que não estamos utilizando? estamos
  cobrindo tudo?"* — cada mecanismo dos dois repos recebe um status no KIRA:
  **usando** · **planejado** (onda/PR) · **rejeitado** (com motivo) · **coberto de
  outra forma**.

---

## 1. agent-skills — engenharia por dentro

Monorepo Nx (npm workspaces) com 5 pacotes + plugin local de generator:

| Componente | Engenharia que vale nota |
|---|---|
| `packages/skills-catalog` | 94 `SKILL.md` em 14 categorias; `generate-registry` emite `skills-registry.json` (116 KB) com **SHA-256 por skill**; `scan-skills` roda **Snyk Agent Scan** bloqueante no publish, com cache incremental por hash e allowlist com validade |
| `libs/core` | hexagonal de verdade: ports (`env/fs/http/logger/shell/package-resolver`) + adapters node + services (`agents` com 19 agentes e diretórios por agente, `installer`, `lockfile`, `registry` com CDN pinado + cache 24 h, `audit-log` JSONL append-only); API pública sob teste (`api-surface.spec.ts`) |
| `packages/cli` | Commander + **Ink/React 19** no TUI interativo (wizard de install/browse/remove), jotai para estado; subcomandos não-interativos para CI |
| `packages/mcp` | servidor stdio com **progressive disclosure** nas instruções (`search → read → fetch só o referenciado`), staging idempotente por hash em `~/.cache`, integridade recomputada sobre paths ordenados (`integrity.ts`) |
| `packages/marketplace` | Next.js 16 SSG; dados gerados do frontmatter; deploy próprio |
| `tools/validate-skills.ts` | ~20 checagens editoriais: trigger "Use when…", anti-trigger "Do NOT use…", `metadata.version/author`, corpo ≤500 linhas, "skills são para agentes, não humanos" |
| Instalador | defense-in-depth: sanitização de nome → contenção por `resolve()` dos dois lados → **symlink guard com `lstat`**, alvo final, loop ELOOP, junctions Windows |

**Cadeia de custódia** (o diferencial real): um único `computeSkillHash`
compartilhado por registry, lockfile do CLI e verificação do MCP — o hash viaja
com o conteúdo do authoring ao runtime.

### Cobertura no KIRA

| Mecanismo deles | Status no KIRA | Onde / por quê |
|---|---|---|
| Hash por versão de prompt + scan anti-injection no CI | **planejado** PR48 | `src/llm/prompts.rs` já tem versão+FNV; falta o scan e o hash na sessão |
| Frontmatter de skill (trigger/anti-trigger/semver/autor) | **planejado** onda SK | `skills-rag-plano.md`; formato reimplementado, catálogo próprio |
| Validação editorial de prompt (≤500 linhas, triggers obrigatórios) | **coberto de outra forma** | fixtures de prompt como contrato (`triage_prompt_fixtures`) + revisão em PR |
| Lockfile + audit log de instalação | **rejeitado** | KIRA não instala conteúdo de terceiros; não há supply chain de skill |
| MCP server / marketplace / 19 agentes | **rejeitado** | veto do mandato (MCP); KIRA é um agente por projeto, não registry |
| Symlink/junction guard (`lstat`, ELOOP) | **planejado** PR47 | `guard/path.rs` hoje usa contenção sem `lstat` |
| Progressive disclosure no prompt | **usando** | packer com orçamento e prioridade fixa (ADR-0015) — mesma ideia, sem MCP |
| Snyk/scan de conteúdo de terceiros | **rejeitado** | não há conteúdo de terceiros; o scan do KIRA é de *dep do projeto* (`kira scan`) |
| Trigger evals (testar *quando* o skill dispara) | **não usando — deveria** | ver §3 |

## 2. harness-toolkit — engenharia por dentro

Runtime global (`~/.tlc/harness`) respondendo hooks do Cursor/Claude Code;
TypeScript estrito executado direto por Bun (~1 ms/hook) ou Node ≥24
(`dist/*.mjs`, ~27 ms); **zero dependências de runtime**.

| Componente | Engenharia que vale nota |
|---|---|
| `bin/tlc-exec.mjs` | launcher único de todos os hooks; resolve runtime home, decide Bun vs Node; **fail-open** (`{}` = carry on) com `hook-trace.jsonl` para diagnóstico |
| `src/contracts` | vocabulário puro: 18 `HarnessEventKind`, `Decision` tipado (`allow/deny/ask/context/continue/rewriteInput/rewriteOutput/abstain`) |
| `src/core` | ~27 agregados flat: `floor` (8 regras avaliadas **antes** de carregar política), `gate`, `lesson`, `observability`, `attest` (hash-chained), `ship`, `pricing`, `stagnation`, `secret-scan`… |
| `src/providers` | 2 adapters (Cursor, Claude) atrás de `ProviderPort`; `provider.degrade.ts` mapeia decisão que o host não expressa (`ask`→`deny`, hooks-less→ADVISORY) |
| `capabilities/catalog.json` | v15, 25 rails com `trade-off` declarado; **docs e tabelas do README gerados do catálogo** — drift reprova o gate |
| Observabilidade | 3 planos JSONL (`obs`/`debug`/`audit`) + `obs report` (rollup por sessão: tokens, USD, gates, recusas **atribuídas por regra**) + attestation encadeada por hash |
| Lessons | 3 tiers; `--ref path:symbol` **aposenta a lição quando o símbolo some**; `--until`; efetividade `helped n/m` gradada pelo próximo run do gate |
| Qualidade | gate de 20 passos (`tlc harness test`): biome, tsc, 2 suítes, knip com tetos, 9 checkers próprios (boundaries, complexity, suppressions com `why:/hazard:/invariant:`, wiring, docs-bundle, decisions…) |
| Decisões | **139 ADs** numerados; CHANGELOG e `docs/log.md` gerados deles |

### Cobertura no KIRA

| Mecanismo deles | Status no KIRA | Onde / por quê |
|---|---|---|
| Floor inconfigurável + veredito tipado | **planejado** PR44 | `guard/` hoje: deny-list + jail, sem piso declarado nem `rule=` na recusa |
| Lessons com refs que resolvem + efetividade | **planejado** PR46 | wiki tem idade/colapso; falta ref `path:symbol` e `helped n/m` |
| Observabilidade em planos + atribuição por regra | **coberto de outra forma** (parcial) | store-driven: `sessions` + history + doctor + dashboard; falta atribuição e o contador (auditoria F2) |
| Attestation hash-chained | **não usando — avaliar depois** | nosso equivalente barato: `source_hash`/`published_hash` no front-matter + fingerprint FNV |
| Docs gerados do catálogo (drift reprova build) | **não usando — deveria** | ver §3 |
| Observation mode (medir rail antes de enforcing) | **não usando — deveria** | aplicável aos guards novos: modo nota antes de modo recusa |
| Fail-open de hook com trace | **usando (espírito)** | store fail-open (`aviso:` no stderr) + e2e; sem trace file próprio |
| Pricing em USD / catálogo de preços | **rejeitado** | mandato: KIRA mede chamadas, não dinheiro |
| Hooks de editor como transporte | **rejeitado** | KIRA é CLI com turno próprio; vem o *design de veredito*, não o transporte |
| Portar TS→Rust / copiar código ELv2 | **rejeitado** | licença + Sísifo |

## 3. O que **não estamos utilizando** e deveríamos (curto e acionável)

1. **Trigger evals** (agent-skills `trigger_evals.json`): testar *quando* um
   skill/prompt dispara, não só o que ele produz. Na onda SK, cada skill bundled
   deveria vir com 3–5 casos "deve casar / não deve casar" — barato e impede
   skill que casa com tudo. → entra no aceite da SK.
2. **Observation mode** (harness `docs/concepts.md`): todo guard novo nasce em
   modo *nota* por N execuções antes de virar recusa — engenharia de fadiga de
   aprovação sem apostas. → entra no aceite do PR44.
3. **Docs atrelados a código**: tabelas de guards/comandos geradas do código ou
   checadas no CI (eles reprovam drift). Nosso M8 é manual; um checker
   `docs-vs-código` (grep de nomes de comando no README/AGENTS vs `clap`) pega o
   F1/F3 desta auditoria automaticamente. → PR barato, sem dependência.
4. **`why` por decisão** (harness `tlc harness why`): nosso `sessions` guarda o
   quê, não o *porquê da recusa*; com o veredito tipado do PR44, gravar
   `rule=` na sessão fecha isso.

## 4. O que cobrimos e eles não (diferencial, resumido)

Turno com fila/lease/cache retomável · trust gate antes de enviar código ao
modelo · wiki com gate de publicação e staleness detectável · determinismo
contratual byte a byte · gate de acerto contra gold humano sem LLM juiz ·
binário único local-first com SQLite bundled · LLM caro nunca implícito ·
redação de segredo **na escrita do índice**. Nenhum dos dois anexos tem
qualquer um desses — porque nenhum dos dois é um organismo; são degraus.

## 5. Conclusão de cobertura

| Status | agent-skills | harness-toolkit |
|---|---|---|
| usando hoje | 1 (progressive disclosure) | 2 (fail-open, orçamento por chamada) |
| planejado com PR/onda | 4 (PR47, PR48, SK×2) | 3 (PR44, PR46, atribuição/contador) |
| deveria, sem PR ainda | 1 (trigger evals → SK) | 3 (observation mode, docs-vs-código, `why`) |
| rejeitado com motivo | 4 | 4 |
| coberto de outra forma | 2 | 2 |

**Nada relevante ficou de fora sem decisão registrada** — os rejeitados estão
justificados por mandato (MCP, daemon, pricing, ELv2) e os "deveria" viraram
aceite de PR/onda nesta auditoria.
