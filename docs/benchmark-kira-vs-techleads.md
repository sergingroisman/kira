# Benchmark — KIRA vs. Tech Leads Club (agent-skills · harness-toolkit)

- **Data**: 12/Nov/2026
- **Pergunta**: o que os dois toolkits do Tech Leads Club fazem que o KIRA não
  faz — e o que o KIRA faz que eles não fazem?
- **Método**: dois planos.
  1. **Escala verificável** — números contados no clone (script do dump conta
     bytes/arquivos; proveniência é o commit SHA).
  2. **Rubrica do mandato** — 10 dimensões extraídas dos contratos do KIRA
     (`AGENTS.md`, `docs/kira-mandato.md`, `docs/kira-persona.md`), nota 0–3
     por repo. As notas são **conceituais** (código/docs lidos), não runtime:
     o harness deles precisa de hooks do Cursor/Claude Code e o KIRA de work
     item do ADO — nenhum dos três roda headless no mesmo teste. Cada nota tem
     justificativa com evidência.
- **Fontes**: `agent-skills @ e74dc8a` (dump em `docs/dumps/agent-skills/`) ·
  `harness-toolkit @ d293c7f` (extração em `docs/dumps/harness-toolkit-extracao.md`,
  dump cru local — ELv2) · `kira @ 48cc75a` (main, 11/Nov/2026).
- **Cuidado honesto**: a rubrica é o **mandato do KIRA** usado como régua — mede
  aderência aos contratos do KIRA, NÃO "melhor produto geral". Os dois são
  produtos maduros com comunidade (7.157★/310★); o KIRA é um projeto novo.

## Escala verificável

| | agent-skills | harness-toolkit | KIRA |
|---|---|---|---|
| estrelas / criado | 7.157★ / jan/2026 | 310★ / ago/2026 | — |
| licença | MIT (código) + CC-BY-4.0 (skills) | **Elastic License 2.0** | MIT |
| linguagem | TypeScript (Nx, monorepo) | TypeScript (zero deps runtime, SLSA v1) | Rust, binário único, SQLite bundled |
| escala medida | 819 arquivos de skill (6,62 MB); 94 SKILL.md (1,33 MB); 14 categorias | 624 arquivos no dump local (4,2 MB); `src/` flat-by-aggregate | 46 arquivos Rust (~21,4k linhas); 14 arquivos de teste |
| conhecimento empacotado | 94 skills versionadas (semver + autor no frontmatter) | 8 floor rules + 3 always-on + 25 rails (catálogo v15, todo rail declara trade-off) | prompts const (`src/llm/prompts.rs`) + wiki FTS5 + episódios de bug |
| registro de decisões | — (CHANGELOG por semantic-release) | **139 ADs numerados** (`ad-001…146`) | 12 ADRs (`docs/adr/`) |
| eval próprio | `harness-eval` (tracks A/B/C), `spec-driven-eval`, `trigger_evals.json` | `obs report`/`doctor` (execução, não acerto) | `kira triage --eval` (gate M2, ADR-0018) — **gold ainda vazio** |
| transporte | npm + instalação em 20+ agentes | hooks do Cursor/Claude Code | CLI/loop próprio (sem hooks de editor) |

## Rubrica (0–3 por dimensão)

| # | Dimensão | agent-skills | harness-toolkit | KIRA |
|---|---|---|---|---|
| 1 | Determinismo contratual (mesma entrada = mesma saída) | 1 — CLI determinístico, mas skills são prose sem contrato de saída | 2 — gates determinísticos por evento, acoplados ao editor | **3** — findings.json byte a byte (gate M1); pack determinístico |
| 2 | Grounding/evidência no output | 2 — the-judge exige `file:line`/URL oficial na hora | 2 — gate sessions + audit trail por hook | 2 — `caminho:linha` + `redact`; os 3 estados (observado/inferido/desconhecido) ainda são rascunho (relatório token-economy) |
| 3 | Guardas de escrita em tiers | 2 — CLI defense-in-depth (sanitização→isolamento→symlink) | **3** — floor/always-on/rails + veredito tipado; o piso nenhuma config alcança | 2 — deny-list (PR3) + jail (PR27); sem piso declarado nem veredito tipado |
| 4 | Economia de token com número | 1 — progressive disclosure (refs sob demanda), sem número publicado | 2 — `budgetContinue` + rollup de tokens/USD por sessão | 2 — budget 50/dia com número; **ledger de economia ainda rascunho** (relatório token-economy) |
| 5 | Conhecimento com prazo de validade | 2 — skills versionadas, mas nada prova que ainda valem | **3** — lessons: refs que precisam resolver + janela + efetividade `helped n/m` | 2 — wiki FTS5 + colapso de episódios >30 dias; sem refs, sem efetividade |
| 6 | Gate de acerto medido contra gold | 2 — tracks B/C com traps plantadas, sem gold determinístico | 1 — mede execução/observabilidade, não acerto de predição | 2 — `--eval` determinístico desenhado (ADR-0018), **gold ainda vazio** (seed: 3) |
| 7 | Cadeia de custódia de prompt | **3** — Snyk Agent Scan + lockfile + hash imutável + curadoria humana | 2 — zero deps runtime + SLSA provenance no pacote | 2 — prompts versados em PR; sem scan anti-injection nem hash por versão |
| 8 | Licença e portabilidade | **3** — MIT + CC-BY-4.0 com fronteiras escritas | 1 — ELv2 restritiva; dump não pode entrar no repo MIT | **3** — MIT |
| 9 | Registro de decisões | 1 — sem ADR público | **3** — 139 ADs com índice e cross-link | 2 — 12 ADRs, um por decisão relevante |
| 10 | Foco do escopo | 2 — 94 skills (GTM/cloud/design dispersos) | **3** — harness estreito e completo | **3** — sustentação ADO estreita |
| | **Total (30)** | **19** | **22** | **23** |

## Leitura do placar

**Onde o KIRA ganha** (dimensão 1 e o desenho da 6): determinismo contratual e
um gate de acerto medido contra gold do dono — nenhum dos dois tem isso. O
agent-skills avalia skills com juízes LLM e traps; o harness mede execução, não
predição. O contrato do M1 (byte a byte) é a única invariante dos três que
traduz direto em reproducibilidade.

**Onde o KIRA perde** (dimensões 3, 5, 9 — e 7): enforcement em tiers com
veredito tipado (harness), conhecimento que prova que ainda vale (harness) e
registro de decisões na escala deles (139 ADs). São exatamente os P1–P3 do
[relatório](agent-skills-harness-vale-trazer.md); adotados, as dimensões 3 e 5
sobe para 3 sem tocar os 3s existentes.

**Onde ninguém está a salvo**: gold do triage vazio (KIRA), scan anti-injection
de prompt ausente (harness/KIRA), e o agent-skills é o único dos três com
cadeia de custódia de prompt completa — é o P4 da fila.

## O que o benchmark NÃO mede

- Não rodou nenhum dos três em workload comum (impossível headless: hooks de
  editor, npm/registry e ADO/PROJ real). A rubrica é conceitual e a escala é
  contada — nada de número de estrela como qualidade.
- Não mede LLM sob os toolkits: agent-skills e harness-toolkit dependem do
  agente hospedeiro (Claude Code/Cursor) para o modelo; o KIRA tem backend
  plugável com budget. Comparar "qualidade de modelo" aqui seria comparar
  terceiros.
- Não mede a wiki/e2e do KIRA em produção (o projeto ainda não tem o ciclo
  autônomo em uso real) — por isso a nota 2 nas dimensões onde o desenho já
  existe e a medição não.
