# ADR-0021 — Índice de retrieval do repositório: FTS5 incremental, redigido na escrita

- **Status**: aceito provisoriamente (11/Nov/2026) — W2-bis p1, **sujeito a veto
  do dono**. Um veto vira ADR que supersede este, não edição silenciosa.
- **Contexto**: viés alfabético do grep de 400 arquivos + ranking por "linha
  contém termo"; dono exige retrieval confiável e barato
- **Decisores**: Orchestrator (recomendação) · Sérgio (veto/aval)

## Contexto

O context packer (ADR-0015, PR5) faz um `walkdir` **ordenado por nome** e para
em `MAX_FILES_SCANNED = 400`. Em um repo grande, todo arquivo depois do 400º
em ordem alfabética **nunca é lido** — e não há erro nem nota: o corte é
silencioso. Além disso o ranking é "quantos termos a linha contém" e cada
triage re-walka a árvore inteira lendo de novo os mesmos arquivos.

Pesquisa prévia (Claude Code, Nov/2026): **busca local grep + FTS5 ganha de
embeddings** para este porte de projeto — recuperação de trecho exato com
`caminho:linha`, zero infraestrutura, zero rede, custo zero por consulta.
Diretiva do dono: **nada de vetor/embedding**.

## Decisão

**Índice de repositório em SQLite FTS5, incremental, redigido na escrita —
mesma extensão e mesma filosofia derivada-e-descartável da wiki (ADR-0013).**

1. **Store (schema v3)**: tabela `repo_file(project_id, path, mtime, size)` —
   identidade on-disk do que o indexador viu por último — e tabela virtual
   `repo_fts USING fts5(project_id UNINDEXED, path UNINDEXED, line UNINDEXED,
   text)`. Uma linha de FTS5 **por linha de código já redigida**, o que
   permite recuperar `(path, line, text)` diretamente do hit.
2. **Desenho rejeitado**: `content=''` (tabela contentless) não guarda a
   coluna indexada — `SELECT text` retornaria NULL; o `text` nunca seria
   recuperável. O `<path>\u{1}<line>\u{1}<texto>` num blob único não vale a
   complexidade de parse quando colunas UNINDEXED fazem o mesmo de graça.
3. **Indexador incremental** (`src/context/index.rs`): mesma allowlist do grep
   (`SKIPPED_DIRS`, dot-dirs, `MAX_FILE_BYTES=128KB`, probe de binário NUL);
   `KIRA_INDEX_MAX_FILES` (default **5000**, parse validado — valor inválido
   cai no default, nunca quebra) substitui o teto de 400. Só reprocessa
   arquivo cujo `(mtime, size)` mudou; arquivo que saiu do disco sai do
   índice; caminhos ordenados e POSIX — rodar 2× no mesmo estado é um no-op.
4. **REDAÇÃO NA ESCRITA**: toda linha passa por `crate::redact` (via
   `clean_line`, a mesma função do grep) **antes** de entrar no índice — o
   store nunca guarda segredo, nem em dado derivado (regra do AGENTS.md).
5. **Busca com bm25**: `repo_fts MATCH` + `ORDER BY rank`, um trecho
   `caminho:linha` igual ao do grep, com bônus para os arquivos citados em
   `findings.json` — **a saída do packer não muda de shape**.
6. **Fail-open**: índice ausente ou vazio ⇒ cai no grep atual, sem erro;
   falha de reindex vira nota no pack, nunca falha do `triage`.

## Consequências

- **Positivas**: fim do corte alfabético (5000 arquivos, teto ajustável por
  env); ranking bm25 em vez de contagem de termos; grep só roda como fallback;
  incrementalidade barata (um `stat` por arquivo); o `kira context --explain`
  mostra exatamente por que cada trecho entrou (fonte, rank, prioridade) e o
  `--json` dá recall mensurável sem tocar no LLM.
- **Negativas**: store cresce (~1 linha de FTS5 por linha de código fonte,
  redigida e capada a 200 chars); reindex roda a cada scan/ciclo (hoje: antes
  de cada pack de `triage`/`context` — mesmo walk que o grep já fazia, menos
  as leituras); índice pode ficar defasado se um ciclo externo editar o repo
  sem passar pelo KIRA — mitigado pelo reindex no início do pack.
- **Evolução prevista**: `kira context --eval` (como o `triage --eval`) medindo
  recall sobre `tests/fixtures/triage_gold/`; o `--json` já é a entrada disso.

## Rejeitado

- **Embeddings / vector-DB** (Chroma, Qdrant, sqlite-vec): rede ou binário
  novo, custo por consulta, e para recuperação de *linha exata* de código o
  FTS5 basta — conclusão da pesquisa com Claude Code/Akita e diretiva do
  dono. Revisitar apenas se a medição (`--eval`) mostrar recall < alvo.
- **Tabela contentless** (`content=''`): o `text` não seria recuperável.
- **`rg`/grep externo como "índice"**: spawn por consulta, sem incremental, e
  não resolve o corte silencioso — é o problema original.
- **Reindexar dentro do grep de fallback**: duplicaria o walk; o fallback é
  intencionalmente o caminho antigo, intocado.
