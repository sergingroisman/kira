# ADR-0023 — Melhorias emprestadas do gitingest: sniff de BOM/binário, tabela de ignorados e orçamentos da caminhada

- **Status**: proposto (11/Nov/2026) — Hermes (recomendação) · Sérgio (veto/aval).
  Um veto vira ADR que supersede este, não edição silenciosa.
- **Fonte**: análise do repo `coderamp-labs/gitingest` (Python, **MIT**) — licença
  compatível com este repo (MIT). O `ai-jail` continua proibido (GPL); conceito
  sim, código GPL não.
- **Princípio que o gitingest valida**: recuperação de estado local precisa é
  pipeline determinístico com podas cedo e guardrails numéricos — zero LLM na
  caminhada. É a mesma escada do ADR-0016 aplicada a retrieval.

## Contexto

O índice de retrieval (ADR-0021) e o context packer (ADR-0015) já cobrem a
essência do que o gitingest faz: caminhada ordenada, teto de arquivos, redação
na escrita, FTS5/bm25, fail-open. A análise de reaproveitamento olhou as peças
que faltam e encontrou 5 melhorias concretaes + 1 opcional, das quais **uma
corrige um bug latente comprovado no fluxo real do próprio KIRA** (captura
`pnpm-audit-real.json` em UTF-16 LE com BOM, gerada por redirect do PowerShell).

## Decisão

**Quatro melhorias emprestadas do gitingest + 1 opcional, uma por PR,
agrupadas por domínio.** O P2 do relatório original foi cortado após releitura
do código (ver seção cortada abaixo).

### P1 (bug real): BOM-sniff antes de parsear JSON de ferramenta — `scan`

Ferramentas spawnadas no Windows podem devolver stdout com BOM
(UTF-8/UTF-16 LE) quando o usuário redireciona via PowerShell; o `serde_json`
quebra. O scan deve: (1) sniffar os primeiros bytes
(`EF BB BF` / `FF FE` / `FE FF`), decodificar antes do parse; (2) se a
decodificação falhar, erro acionável ("saída da ferramenta veio com BOM não
decodificável — rode `<tool>` direto no terminal e confirme o encoding"),
nunca body cru. Fixture em `tests/fixtures/scan/` com a captura real
`pnpm-audit-real.json` (UTF-16 LE com BOM, hoje untracked na raiz) como
caso de teste canônico.

### ~~P2: Sniff de binário na indexação (chunk de 1 KB) — `context`~~ (CORTADO, 11/Nov/2026)

Releitura do código real (`src/context/repo.rs::read_text_file`): o KIRA **já
faz** probe de NUL + `String::from_utf8` sobre o arquivo inteiro (cap 128 KB) —
binário sem NUL no probe mas com UTF-8 inválido já é rejeitado hoje. O único
escape é binário com UTF-8 válido (raríssimo), e o ganho de eficiência do
gitingest (não ler o arquivo todo) é irrelevante sob o cap de 128 KB.
Melhoria fantasma; sai da fila. Fica registrado como lição: checar o código
atual ANTES de mandar o item para execução.

### P3: Tabela de padrões ignorados curada — `context`

O gitingest mantém `DEFAULT_IGNORE_PATTERNS` (~150 padrões por linguagem:
lock files, `*.pdb`, `*.nupkg`, `vendor/`, `.gradle/`, caches de build, bins,
mídia). A caminhada do KIRA cobre dot-dirs/`target/`/`node_modules`; a tabela
cobre o resto que só aparece em repos reais de cliente. Copiar **a tabela
(literal, MIT, com atribuição no comentário)** — não o código Python — e
unificar com a allowlist existente do grep + índice em uma única fonte.

### P4: Orçamentos da caminhada (profundidade + total de bytes) — `context`

`KIRA_INDEX_MAX_FILES` só limita contagem de arquivos. O gitingest usa três:
`MAX_DIRECTORY_DEPTH` (20), `MAX_TOTAL_SIZE_BYTES` (500 MB) e teto por arquivo
(KIRA já tem `MAX_FILE_BYTES=128KB`). Faltam profundidade máxima e tamanho
acumulado — como **constantes fixas** primeiro (envs novas só com justificativa
no PR, regra do AGENTS.md); estourar é **nota no pack, nunca erro** (mesma
filosofia do `KIRA_NUM_CTX`).

### P5: Honrar `.gitignore` na caminhada — `context`

Arquivo que o dev ignorou deliberadamente não deveria entrar no retrieval.
Implementar em **Rust puro um subconjunto gitwildmatch** (glob + negação `!` +
`.gitignore` aninhado por diretório — a mesma gramática do git, não regex
inventada), sem dependência nova. É a fonte da "precisão" do gitingest.
Corte conservador: o que não casa o subconjunto casa `*` literal.

### P6 (opcional): Estimativa de tokens no cabeçalho do pack

`Estimated tokens: 15.2k` no brief ajuda a calibrar `KIRA_NUM_CTX`. No KIRA é
**chars/4 determinístico** (tiktoken do gitingest baixa BPE da rede — contra o
CLI-first). Nota no pack, nunca erro.

## Consequências

- **Positivas**: fim do bug latente de BOM (P1 é comprovado no fluxo real);
  allowlist unificada cobre repos de cliente reais; caminhada com teto triplo
  idempotente e previsível; retrieval respeita a intenção do dev
  (`.gitignore`).
- **Negativas**: 4 PRs a mais na fila (um por item, regra do repo); P3/P5 tocam
  `src/context/index.rs` e `src/context/repo.rs` simultaneamente — ordenar
  P3 → P4 → P5 para não rebasear; tabela copiada de outro repo precisa de
  atribuição explícita no comentário do arquivo.
- **Não muda**: shape do pack, saída do `triage`, gate do M1/M2, redação na
  escrita, fail-open por fonte — tudo preservado por construção.

## Rejeitado

- **Clone remoto / sparse-checkout / `ls-remote` / parser de slug-URL**
  (`KNOWN_GIT_HOSTS`): problema do gitingest (URL arbitrária), não do KIRA —
  que é local-first e ADO-based.
- **tiktoken como dependência**: rede no runtime; chars/4 basta para nota.
- **Stack de servidor (FastAPI/S3/Sentry/PostHog)**: fora de escopo e contra
  "dado de cliente nunca vai para recurso de estudo".
- **Dependência Python/Rust nova de gitignore** (`pathspec` e afins): subconjunto
  em Rust puro cobre o caso real; sem dep sem justificar no PR.
- **Copiar o digest format literal** (`==== FILE: ... ====`): o KIRA já tem
  ancoragem `caminho:linha` com redação — é a mesma regra, em melhor estado.
