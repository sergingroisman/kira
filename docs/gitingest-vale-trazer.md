# Relatório — O que vale trazer do gitingest para o KIRA

- **Data**: 11/Nov/2026
- **Fonte**: `coderamp-labs/gitingest` (Python, MIT) — análise feita sobre o dump
  completo do repo (README, `src/gitingest/`, `src/server/`, `tests/`)
- **ADR**: [ADR-0023](adr/ADR-0023-gitingest-reaproveitamento.md)
- **Veredito**: 5 melhorias + 1 opcional; P1 corrige bug latente comprovado.
- **Status final da onda M (13/Nov/2026)**: P1 ✅ (M7) · P2 cortado · P3/P4 absorvidos pelo M1 ·
  P5 ✅ (M7) · P6 parcial (coberto em substância pelo ADR-0021).

## Por que o gitingest é rápido e preciso (a jornada, em 4 passos)

Não há mágica nem LLM — é o mesmo princípio do KIRA (determinismo antes de
inteligência):

1. **Podas cedo**: caminhada streaming que aplica ignore patterns *antes* de
   abrir qualquer arquivo (nunca indexa `node_modules`/`target/` para depois
   filtrar).
2. **Guardrails numéricos**: orçamento triplo que corta a caminhada —
   `MAX_DIRECTORY_DEPTH=20`, `MAX_FILES=10_000`, `MAX_TOTAL_SIZE_BYTES=500MB`,
   `max_file_size=10MB` por arquivo. Estourou → pula com log de warning,
   **nunca erro** (é o "orçamento estourado vira nota" do KIRA).
3. **Heurística binário/texto barata**: lê só 1 KB de chunk e tenta decodificar;
   binário → placeholder `[Binary file]` e o conteúdo não é ingerido.
4. **Matching por spec real, não heurística**: `pathspec` com sintaxe
   `gitwildmatch` do git (negação `!` e `.gitignore` aninhado). A precisão vem
   da gramática do git, não de regex inventada.

## O que vale trazer (concreto, por módulo)

| # | Ideia do gitingest | Onde cai no KIRA | Prioridade |
|---|---|---|---|
| P1 | **BOM-sniff antes de parsear JSON de ferramenta** (`EF BB BF`/`FF FE`/`FE FF`) | `src/scan/node.rs`, `src/scan/dotnet.rs` | **Bug real** — a captura `pnpm-audit-real.json` (redirect PowerShell) saiu UTF-16 LE com BOM; o `serde_json` quebraria. **Implementado 13/Nov/2026 (M7)**: `src/scan/decode.rs` cobre os 3 BOMs nos dois call sites, com fixture UTF-16 LE real |
| P2 | ~~Sniff de binário (1 KB + decodificação)~~ | `src/context/repo.rs` | **Cortado (11/Nov)** — `read_text_file` já faz probe de NUL + `from_utf8` no arquivo inteiro; melhoria fantasma |
| P3 | **Tabela `DEFAULT_IGNORE_PATTERNS`** (~150 padrões curados por linguagem) | `src/context/ignore.rs` (denylist única dos três caminhos) | **Absorvido pelo M1 (13/Nov)**: a tabela vive em `DEFAULT_IGNORE_PATTERNS` com atribuição ao gitingest no header, e o teste de política é um só para índice, grep e inventário |
| P4 | **Orçamentos da caminhada**: profundidade máx. + total de bytes | `src/ingest/inventory.rs` (`Budget`: depth 20, teto de arquivos, cap por arquivo **com nota**) | **Absorvido pelo M1 (13/Nov)**: orçamento explícito com nota em todo corte (nunca skip silencioso); teto de *bytes totais* rejeitado de propósito — ver ADR-0038 |
| P5 | **Honrar `.gitignore`** (subconjunto gitwildmatch em Rust puro, com negação e `.gitignore` aninhado) | `src/context/gitignore.rs` aplicado nos três caminhos (índice, grep, inventário) | Média — a fonte da precisão do gitingest. **Implementado 13/Nov/2026 (M7)**: `GitIgnoreStack` com cascata já na assinatura (onda IG-1 pluga `.kiraignore`/`.ignore` sem retrabalho); subset documentado (sem classes/escapes; sensível a caixa em todo SO); teste de política única prova os três caminhos com as mesmas regras |
| P6 | **Estimativa de tokens no cabeçalho do pack** | `src/context/mod.rs` — `context_summary`/`--json` já imprimem `X/Y chars (~Z tokens, 3 chars/token)`; conservadorismo maior que o chars/4 do gitingest | **Parcial — coberto em substância pelo ADR-0021** (estimativa determinística no resumo e no JSON; sem tiktoken, que é rede) |

## O que NÃO vale

- **Clone remoto / sparse-checkout / `ls-remote`** — problema do gitingest
  (URL arbitrária); o KIRA é local-first e ADO-based.
- **Parser de slug/URL (`KNOWN_GIT_HOSTS`)** — idem.
- **tiktoken** — rede no runtime, contra CLI-first.
- **Servidor (FastAPI/S3/Sentry/PostHog)** — fora de escopo; "dado de cliente
  nunca vai para recurso de estudo" manda o contrário.
- **Digest format literal** (`==== FILE: ... ====`) — o KIRA já tem ancoragem
  `caminho:linha` + redação, que é a mesma regra em estado melhor.

## Validação de design de fora

O formato do digest do gitingest ancora **todo** blob com `caminho` — é
exatamente a regra do AGENTS.md ("blob anônimo no prompt é proibido"). Duas
comunidades chegaram no mesmo contrato por dor real.

## Ordem de execução sugerida

1. ~~**P1**~~ — **implementado em 13/Nov/2026 (M7)**: `src/scan/decode.rs` + fixture `scan_pnpm_audit_utf16le.json` (a captura real reencodada UTF-16 LE com BOM).
2. ~~**P3 → P4 → P5**~~ — P3/P4 foram absorvidos pelo M1 (denylist única +
   orçamentos `Budget`); **P5 implementado 13/Nov/2026 (M7)**:
   `src/context/gitignore.rs`, negação e aninhamento honrados nos três caminhos.
3. ~~**P2**~~ — cortado em 11/Nov/2026: `read_text_file` (probe de NUL +
   `from_utf8` no arquivo inteiro) já cobre o caso; melhoria fantasma.
