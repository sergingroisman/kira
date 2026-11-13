# Plano de implementação — Onda IG (ingestão v2, repomix + gitingest)

> **Data**: 13/Nov/2026 · **Fonte**: [`docs/ingestao-repomix-gitingest-plano.md`](ingestao-repomix-gitingest-plano.md)
> (avaliação + C4, reancorada nos snapshots reais) · **Estado ancorado**: `main` com
> M1/M2/M3 fechados.
>
> Este documento transforma os itens IG-0…IG-6 do plano de avaliação em **planos de
> implementação executáveis**: escopo fechado, arquivos com âncoras de código reais,
> fixtures, testes de aceite (determinismo byte a byte) e decisões abertas com proposta
> default. As âncoras foram verificadas no disco em 13/Nov/2026 — o módulo e o ponto de
> inserção são o contrato; números de linha podem derivar quando a onda começar.

**Advertência honesta**: cada item aqui só vira código após `cargo fmt --check` →
`cargo clippy --all-targets -- -D warnings` → `cargo test` → `cargo build` → CI 3 SOs
(gates do `docs/roadmap-prs.md`). Merge é sempre humano. As decisões G1–G7 da seção 9
precisam do selo do dono **antes** da PR correspondente entrar na fila.

### Reconciliação com a onda M (estado real do repo, 13/Nov)

O `docs/BUILD-PROGRAM.md` **já escopou** parte desta onda no **M7**:

- **P1 BOM** — `src/scan/decode.rs` (UTF-8/UTF-16 LE/BE) nos call sites de `node.rs`/`dotnet.rs`.
  **Já está no working tree** (não commitado): `src/scan/decode.rs::decode_tool_output` usa
  `String::from_utf16_lossy` (lone surrogate ⇒ replacement char, nunca pânico).
- **P5 `.gitignore`** — `src/context/gitignore.rs`, subset gitwildmatch, aninhado, sem dependência.

**Consequência para este plano**: os itens abaixo são o **delta que o M7 não cobre**.
O IG-0 (BOM no sniff de binário do *inventário/índice/digest*) é um módulo diferente do
P1 do M7 (que é o decode de *stdout de ferramenta*) — um arquivo UTF-16 **do repo** continua
sendo classificado como binário e sumindo. O IG-1 reduz-se à **cascata de prioridade**
(`.kiraignore` > `.ignore`) por cima do `gitignore.rs` do M7. IG-2/3/5/6 não têm nenhum
correspondente no M7.

---

## 1. Estado da main que a onda toca (âncoras verificadas 13/Nov)

| Módulo | Âncora (verificada) | Consumido por |
|---|---|---|
| `src/context/ignore.rs` | `DEFAULT_IGNORE_PATTERNS` (L39); `is_ignored_dir` (L224); `is_ignored_file` (L240); `matches_pattern` (L251); `glob` (L262); `looks_binary` (L299, `sample.contains(&0)`) | IG-0, IG-1 |
| `src/context/repo.rs` | `read_text_file` (L332, probe NUL em L339 + `from_utf8`); `is_source_file` (L322); `is_skipped_dir` (L306); `MAX_FILE_BYTES=128KB` (L20) | IG-0, IG-1 |
| `src/ingest/inventory.rs` | `Budget {max_depth:20, max_files:5000}` (L26); `Inventory` (L52); `tree()` (L71); `walk()` (L100); `read_probe` (L225); contador `binary_skipped` (L108, nota L196) | IG-0, IG-1, IG-2, IG-6 |
| `src/ingest/mod.rs` | `run()` (L118); `read_for_digest` (L231, `from_utf8_lossy` + strip `\u{feff}`); `reindex` (L204) | IG-0, IG-3 |
| `src/ingest/digest.rs` | `MAX_KEY_FILES=12` (L16); `MAX_KEY_FILE_LINES=60` (L19); `MAX_DIGEST_CHARS=200k` (L22); `select_key_files` (L66); `render` (L98) | IG-2, IG-4, IG-5 |
| `src/redact.rs` | `Patterns` (L64, regexes `bearer/keyed/url_query/known_prefixes/email/cpf`); `MASKED` (L20); `redact()` | IG-3 |
| `tests/fixtures/scan/pnpm-audit-real.json` | captura UTF-16 LE (caso canônico do bug de encoding) | IG-0 |
| `tests/fixtures/security/fake_pat.txt` | PAT sintético | IG-3 |

**Fato que reancora o plano**: existem **três** pontos de detecção de binário, todos com
a mesma regra "NUL = binário" — `ignore::looks_binary` (inventário), `repo::read_text_file`
(grep + índice) e `inventory::read_probe` (delega para `looks_binary`). Um arquivo
UTF-16 LE **vira binário nos três** e some do inventário, do índice e do digest. O IG-0
unifica os três numa função só.

---

## 2. Regras da onda (não negociáveis)

1. **Gates por PR**: `cargo fmt --check` → `clippy --all-targets -- -D warnings` →
   `test` → `build` → CI 3 SOs.
2. **Branch naming**: `ig/pr<n>-<slug>` (ex.: `ig/pr1-bom-sniff`). Um PR por item.
3. **Determinismo**: mesmo estado ⇒ saída byte-idêntica onde o plano indica. O gate
   byte a byte vale para os artefatos derivados (`facts.md`, `inventory.json`,
   `secrets.json`, inventário), não para linhas de DB (que têm `id`/timestamps por design).
4. **Dependências novas**: **nenhuma**. Decodificação UTF-16 é `String::from_utf16` (std).
   Env nova: nenhuma. Qualquer acréscimo precisa de justificativa explícita no PR.
5. **Segredo e redação**: nada cru no prompt, DB, log ou disco — toda escrita derivada
   passa por `crate::redact`; o detector do IG-3 **relata, nunca ecoa**.
6. **`.kira/` é local**: nunca comitado; artefatos novos (`secrets.json`) moram lá.
7. **CLI-first, LLM-last**: nenhum item desta onda importa `llm::*` — o teste de
   allowlist (`tests/llm_allowlist.rs`) continua verde.
8. **Fail-open**: wiki/índice/store/findings que falham viram nota; o comando principal
   continua (padrão vigente do packer).

---

## 3. Ordem de execução e dependências

```
IG-0 (bom-sniff + decode) ──► IG-1 (cascade ignore) ──► IG-2 (binários na árvore)
                                                          │
                                                          └─► IG-3 (segredos)
IG-5 (tokens) ── independente ──► IG-6 (bytes) ──► IG-4 (git-aware, backlog)
```

- **IG-0 primeiro** porque corrige um bug comprovado e toca a função compartilhada
  (`ignore::looks_binary` + `repo::read_text_file`) que IG-1/IG-2 também mexem — resolver
  a base antes evita rebase.
- **IG-1 antes do IG-2** porque os dois tocam `inventory.rs::walk`; IG-2 reaproveita o
  campo novo (`binary_paths`) que o IG-1 deixa passar.
- **IG-5/IG-6** são independentes e baratos; podem entrar em qualquer ponto depois do IG-0.
- **IG-4** fica no backlog (decisão G4) — não entra na fila agora.

> **Sobreposição com o M7**: o M7 (P1 BOM no scan + P5 `.gitignore`) roda em paralelo.
> IG-1 depende do `src/context/gitignore.rs` do M7/P5 para o subset — se o M7 ainda não
> landou, IG-1 traz o subset junto (ver seção 5). IG-0 é independente do M7 (módulo
> diferente: inventário/índice/digest, não scan).

---

## 4. IG-0 — BOM-aware sniff + decode UTF-16 (bug real)

**Objetivo**: um arquivo de texto com BOM (UTF-8/UTF-16 LE/BE) deixa de ser classificado
como binário e passa a ser **decodificado de verdade**. Hoje os três pontos de sniff
(`ignore::looks_binary`, `repo::read_text_file`, `inventory::read_probe`) tratam NUL como
binário — texto UTF-16 sprinkles NUL e some. Mesma família do ADR-0023-P1, **no sniff do
repo** (o M7/P1 já resolveu o lado do stdout de ferramenta em `src/scan/decode.rs`; este
item é o lado *arquivo do repositório*: inventário, índice e digest).

**Escopo fechado**: UTF-8 BOM + UTF-16 LE/BE. **UTF-32 e GB18030 ficam de fora** (raros;
registrados na decisão G7). Sem dependência nova. A decodificação **espelha** o
`src/scan/decode.rs::decode_tool_output` já escrito: `String::from_utf16_lossy` (lone
surrogate ⇒ replacement char, nunca pânico). Se o `decode.rs` do scan landar antes, extrair
um helper compartilhado (ex.: `src/context/decode.rs`) para não haver dois decoders UTF-16.

**Arquivos**:

- `src/context/ignore.rs` — duas funções novas (públicas ao crate):
  - `pub(crate) fn has_text_bom(bytes: &[u8]) -> bool` — reconhece `EF BB BF` (UTF-8),
    `FF FE` (UTF-16 LE), `FE FF` (UTF-16 BE);
  - `pub(crate) fn decode_text(bytes: &[u8]) -> Option<String>` — função **única** que:
    (1) se há BOM UTF-16, decodifica via `String::from_utf16_lossy` (LE: pares diretos; BE:
    troca bytes); (2) se há BOM UTF-8, `from_utf8` do restante; (3) sem BOM, `from_utf8`
    se **não** contiver NUL; (4) senão `None` (binário). `looks_binary` (L299) vira
    `!has_text_bom(sample) && sample.contains(&0)`.
- `src/context/repo.rs` — `read_text_file` (L332) troca o `probe.contains(&0)` + `from_utf8`
  por `ignore::decode_text(&bytes)`; a regra de `MAX_FILE_BYTES` (L20) fica.
- `src/ingest/mod.rs` — `read_for_digest` (L231) troca `from_utf8_lossy` + strip manual
  por `ignore::decode_text` (redação por linha permanece em cima do texto decodificado).
- `src/ingest/inventory.rs` — `read_probe` (L225) já delega para `looks_binary`; sem
  mudança estrutural (ganha a correção de graça).

**Fixtures**: `tests/fixtures/scan/pnpm-audit-real.json` (UTF-16 LE, já existente) como
caso canônico + um `.rs` sintético UTF-16 BE para o teste de unidade.

**Testes de aceite**:

- `decode_text` de UTF-16 LE/BE devolve o texto esperado; PNG/ELF devolvem `None`.
- `looks_binary` de `[0xFF, 0xFE, b'a', 0, b'b', 0]` (UTF-16 LE) é `false`.
- `read_text_file` sobre um arquivo UTF-16 LE devolve o conteúdo (não `None`).
- `inventory::walk` num repo com arquivo UTF-16 LE ⇒ arquivo **entra** no inventário
  (regressão: antes saía como binário).
- `grep`/índice sobre arquivo UTF-16 LE ⇒ trecho ancorado `caminho:linha` aparece.
- Dois `ingest` ⇒ `facts.md` byte-idêntico (o BOM não muda o determinismo).

**Decisão G7**: escopo de encodings = UTF-8 + UTF-16 (LE/BE); UTF-32/GB18030 fora.

---

## 5. IG-1 — Cascade de ignore (`.gitignore`/`.ignore`/`.kiraignore`)

**Objetivo**: a caminhada honra a intenção do dev. Prioridade (a do repomix, `fileSearch.ts`):
`.kiraignore` > `.ignore` > `.gitignore` (aninhado, mais profundo vence) > `DEFAULT_IGNORE_PATTERNS`.
Negações `!` e gitwildmatch num **subconjunto Rust puro** — a gramática do gitingest/`pathspec`
(ADR-0023-P5), **não** a do repomix (que avisa e pula `!`).

**Divisão com o M7/P5**: o M7 já escopou o `src/context/gitignore.rs` (subset gitwildmatch,
aninhado, sem dependência). Este item **não** reimplementa o subset — ele é a **camada de
cascata** por cima: carrega o `gitignore.rs` do M7 e acrescenta a prioridade
`.kiraignore` > `.ignore` > `.gitignore` (deferred para os control files, como o
`fileSearch.ts` do repomix). Se o M7 landar primeiro, IG-1 vira um PR pequeno que só adiciona
a prioridade; se IG-1 entrar primeiro, ele traz o subset junto (e o M7/P5 vira só o aceite).

**Escopo fechado**: glob `*`/`?`/`**` + negação `!` + `.gitignore` aninhado por diretório.
Corte conservador (ADR-0023-P5): o que não casa o subconjunto casa `*` literal. Sem
dependência nova (o `glob` de `ignore.rs` L262 já resolve `*`/`?`; estender para `**`/`!`).

**Arquivos**:

- `src/context/gitignore.rs` (do M7/P5) — o subset gitwildmatch + negação + aninhado
  (fonte única do matcher; o IG-1 **reusa**, não reimplementa).
- `src/context/ignore.rs` — novo `load_ignore_rules(root) -> IgnoreRules` (lê
  `.kiraignore` + `.ignore` + `.gitignore`, monta a lista ordenada por prioridade + negações)
  e `IgnoreRules::is_ignored(rel)`. A regra do `glob` (L262) já cobre `*`/`?`; o `**` e a
  negação `!` vêm do `gitignore.rs` (M7).
- `src/ingest/inventory.rs` — `walk` (L100) usa `IgnoreRules` no `filter_entry` (diretório)
  e no filtro de arquivo (L139), além do `is_ignored_file` atual.
- `src/context/index.rs` — `reindex_limited` (L89) e `repo::grep` (L165) respeitam as
  mesmas regras: **teste de política única** (o mesmo teste que prova `ignore` prova
  índice + grep).

**Fixtures**: `tests/fixtures/context/ignore/` — repo com `.gitignore` excluindo
`*.secret.txt` + negação `!keep.txt` + `.gitignore` aninhado.

**Testes de aceite**:

- Arquivo em `.gitignore` ⇒ fora do inventário, do índice e do digest.
- Negação `!keep.txt` ressuscita o arquivo.
- `.gitignore` aninhado (mais profundo) vence o da raiz.
- Sem nenhum arquivo de ignore ⇒ comportamento byte-idêntico ao de hoje (gate M1).
- `.kiraignore` tem prioridade sobre `.gitignore` quando os dois existem.
- Dois `ingest` ⇒ `facts.md` e `inventory.json` byte-idênticos.

**Decisões**: **G1** (nome = `.kiraignore`), **G2** (honrar `.gitignore` por default, com
flag `--no-gitignore`).

---

## 6. IG-2 — Binários na árvore

**Objetivo**: binários deixam de ser só "N ignorados" e aparecem na árvore como
`caminho [binário]`, sem conteúdo (repomix: overview completo do repo).

**Arquivos**:

- `src/ingest/inventory.rs` — `Inventory` (L52) ganha `binary_paths: Vec<String>` (ordenada);
  `walk` (L100) troca o `binary_skipped += 1` (L157) por `binary_paths.push(path)` (mantém
  a nota de contagem como resumo); `tree()` (L71) intercala os caminhos binários com o
  sufixo ` [binário]`.
- `src/ingest/digest.rs` — `render` (L98) lista os binários na `## Árvore` (via `tree()`)
  e, se `binary_paths` não vazio, uma linha no `## Perfil` (`- binários na árvore: N`).

**Testes de aceite**:

- Repo com `logo.png` ⇒ árvore mostra `logo.png [binário]` e **nenhum** conteúdo do PNG.
- `inventory.json` idempotente (mesma árvore ⇒ mesmos bytes).
- Repo sem binários ⇒ `binary_paths` vazio e digest byte-idêntico ao de hoje.

---

## 7. IG-3 — Detector de segredos na ingestão

**Objetivo**: a ingestão **detecta** (além de mascarar) segredos prováveis — PAT, chave
privada, connection string, senha em atribuição — e grava `.kira/ingest/secrets.json`
com **caminho + nº de linhas afetadas**, nunca o valor. É o Secretlint do repomix no
contrato do KIRA: relatar sem ecoar.

**Escopo fechado**: conteúdo de arquivo ingerido (git diff/log ficam de fora — backlog).
Reusa as regexes de `src/redact.rs` — **sem** dependência nova, **sem** Secretlint.

**Arquivos**:

- `src/redact.rs` — expor `pub(crate) fn classify(line: &str) -> Option<SecretKind>` com
  `enum SecretKind { Token, Key, Email, Cpf }`: roda as mesmas regexes de `Patterns`
  (L64) e devolve o tipo na primeira casada, **sem** o valor. `redact()` continua mascarando.
- `src/ingest/mod.rs` — após `read_for_digest`, uma passada por linha chama `classify` e
  acumula `(path, contagem por kind)`; grava `.kira/ingest/secrets.json` (ordenado,
  determinístico, sem valores). `digest.rs::render` ganha seção `## Segredos prováveis`
  (contagem por caminho) quando houver.
- `src/ingest/mod.rs` — `ensure_local_marker` (L274) já cobre o `.kira/`; `secrets.json`
  nasce dentro dele.

**Fixtures**: `tests/fixtures/security/fake_pat.txt` (PAT sintético) + um `.rs` com chave
privada/connection string.

**Testes de aceite**:

- `secrets.json` lista `path` + contagem e **não** contém o segredo (assert sobre ausência
  do valor literal, padrão do `tests/context_e2e.rs`).
- `facts.md` não ecoa o valor (só `[REDACTED]`/contagem).
- Arquivo limpo ⇒ `secrets.json` ausente e digest byte-idêntico ao de hoje.
- Dois `ingest` ⇒ `secrets.json` byte-idêntico.
- `redact::classify` idempotente e não aloca o valor na saída.

**Decisão G3**: gravar `secrets.json` no disco (`.kira/` é local) em vez de só nota.

---

## 8. IG-5 — Estimativa de tokens · IG-6 — Orçamento total de bytes · IG-4 (backlog)

### IG-5 — Estimativa de tokens no digest

- **Arquivos**: `digest.rs::render` (L98) — uma linha no `## Perfil`:
  `- tokens (estimativa): ~N (chars/4)`. Determinístico, sem BPE.
- **Aceite**: digest byte-idêntico entre execuções; `~` presente quando há conteúdo.
- **Decisão G5**: `chars/4`.

### IG-6 — Orçamento total de bytes da caminhada

- **Arquivos**: `inventory.rs::Budget` (L26) ganha `max_total_bytes: u64` (const fixa);
  `walk` (L100) soma `meta.len()` e, estourado, para com a nota "árvore parcial" (sem
  erro). Mede **bytes de arquivo lido**, não do diretório (lição do ADR-0038).
- **Aceite**: repo sintético acima do teto ⇒ "árvore parcial" + contagem determinística.
- **Decisão G6**: teto **128 MB** (constante; env só com justificativa no PR).

### IG-4 — Ranking git-aware (backlog)

- **Arquivos**: `digest.rs::select_key_files` (L66) — tie-break por frequência de mudança
  (`git log --format=%H -- <path>`, últimos 100 commits), `--no-git` desliga; `ingest/mod.rs`
  coleta o contador via `binresolve` (mesmo contrato do `KIRA_SCAN_*_BIN`).
- **Aceite**: hot file promovido; sem git ⇒ ranking atual byte a byte.
- **Decisão G4**: fica no backlog até um repo real justificar.

---

## 9. Decisões abertas para o dono selar (resumo)

**PROPOSTAS (13/Nov/2026), SEM SELO.** Cada PR cita a proposta que consumiu.

| # | Item | Proposta default |
|---|---|---|
| G1 | IG-1: nome do arquivo extra | `.kiraignore` |
| G2 | IG-1: honrar `.gitignore` por default | Sim, com `--no-gitignore` |
| G3 | IG-3: gravar `secrets.json` no disco | Sim (`.kira/` é local) |
| G4 | IG-4 entra na fila agora? | Não — backlog |
| G5 | IG-5: estimador | `chars/4` |
| G6 | IG-6: teto de bytes | 128 MB (constante) |
| G7 | IG-0: encodings suportados | UTF-8 + UTF-16 (LE/BE); UTF-32/GB18030 fora |

---

## 10. Pendências do dono (inalteradas)

1. **Sellar G1–G7** (ou aceitar os defaults) antes de cada PR.
2. **Merge é sempre humano** — a onda entra PR a PR na ordem da seção 3.
3. As pendências das ondas anteriores (rótulos `triage_gold`, D1–D12 da onda TE) **não
   são tocadas** por esta onda — IG é independente de M4/M5 e do gate M2.

## 11. O que esta onda **não** faz (não-objetivos)

- Não copia a tabela `defaultIgnore.ts` do repomix (é inferior à do gitingest, que já temos).
- Não adota compressão por strip-comments/Tree-sitter, `input.processors`, BPE, token-budget
  hard-fail, clone remoto, MCP/extensão JSON-RPC (rejeitados na avaliação — ver plano).
- Não toca o `scan`, o `triage`, o loop W5 nem o dashboard.
