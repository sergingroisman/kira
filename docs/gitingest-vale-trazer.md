# Relatório — O que vale trazer do gitingest para o KIRA

- **Data**: 11/Nov/2026
- **Fonte**: `coderamp-labs/gitingest` (Python, MIT) — análise feita sobre o dump
  completo do repo (README, `src/gitingest/`, `src/server/`, `tests/`)
- **ADR**: [ADR-0023](adr/ADR-0023-gitingest-reaproveitamento.md)
- **Veredito**: 5 melhorias + 1 opcional; P1 corrige bug latente comprovado.

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
| P1 | **BOM-sniff antes de parsear JSON de ferramenta** (`EF BB BF`/`FF FE`/`FE FF`) | `src/scan/node.rs`, `src/scan/dotnet.rs` | **Bug real** — a captura `pnpm-audit-real.json` (redirect PowerShell) saiu UTF-16 LE com BOM; o `serde_json` quebraria |
| P2 | ~~Sniff de binário (1 KB + decodificação)~~ | `src/context/repo.rs` | **Cortado (11/Nov)** — `read_text_file` já faz probe de NUL + `from_utf8` no arquivo inteiro; melhoria fantasma |
| P3 | **Tabela `DEFAULT_IGNORE_PATTERNS`** (~150 padrões curados por linguagem) | `src/context/index.rs` + allowlist do grep | Alta — MIT, copiar tabela com atribuição, unificar numa única fonte |
| P4 | **Orçamentos da caminhada**: profundidade máx. + total de bytes | `src/context/index.rs` (constantes fixas; env só com justificativa) | Média |
| P5 | **Honrar `.gitignore`** (subconjunto gitwildmatch em Rust puro, com negação e `.gitignore` aninhado) | `src/context/index.rs`, `src/context/repo.rs` | Média — a fonte da precisão do gitingest |
| P6 | **Estimativa de tokens no cabeçalho do pack** | `src/context/mod.rs` — chars/4 determinístico (tiktoken baixa BPE da rede = contra o CLI-first) | Opcional |

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

1. **P1** (bug, pequeno, fixture pronta) — pode entrar na fila W2.
2. **P3 → P4 → P5** (todos tocam `src/context/`; nessa ordem para não
   rebasear) — fila depois do W2-FIX, um PR por item.
3. ~~**P2**~~ — cortado em 11/Nov/2026: `read_text_file` (probe de NUL +
   `from_utf8` no arquivo inteiro) já cobre o caso; melhoria fantasma.
