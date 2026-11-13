# ADR-0039 — Nomenclatura canônica do KIRA (um nome por conceito)

- **Status**: aceito (13/Nov/2026) — decisões do dono. **Glossário canônico final em
  [`docs/nomenclatura.md`](../nomenclatura.md)** (este ADR é a auditoria/evidência; aquele
  é a fonte de verdade).
- **Motivo**: o dono apontou "nomes diferentes para a mesma coisa". A auditoria abaixo
  mapeia cada colisão com **evidência de código/doc** e propõe o nome canônico.

---

## 1. Colisões encontradas (com evidência)

### C1 — "Orchestrator" tem três significados (e "Delta" troca com ele)

| Nome | O que é de fato | Evidência |
|---|---|---|
| **Hermes** | o agente **externo** (DeepSeek) que planeja, sela e merga os ciclos | `BUILD-PROGRAM.md` L1/L37 "Hermes/Orchestrator" |
| **Orchestrator** | papel do Hermes (só que ora aparece como nome próprio) | `roadmap-prs.md` L16 "Delta escreve; Orchestrator sela" |
| **Delta** | o agente que escreve código | `roadmap-prs.md` L16, `kira-delta-tasks.md` |
| **Brain** | o orquestrador **dentro** do binário KIRA (runtime) | `docs/kira-brain.md` (novo) |

Além disso, o campo **"Decisores"** dos ADRs troca o autor entre **"Delta (autoria)"**
(ADR-0017/0018/0019/0020) e **"Orchestrator (recomendação)"** (ADR-0015/0021/0023/0024/0026/0027)
— sem critério visível.

**Canônico proposto**: `Hermes` (o orquestrador externo) · `Delta` (o codificador) ·
`Brain` (o orquestrador interno). **Aposentar "Orchestrator" como nome próprio** — é papel
do Hermes. No campo "Decisores", usar `Hermes` (ou `Delta`) de forma consistente.

### C2 — "digest" é substantivo (artefato do `ingest`) **e** verbo (comando)

| Uso | O que é | Evidência |
|---|---|---|
| `.kira/digest.md` | artefato determinístico do **`kira ingest`** (fatos/índice) | `src/ingest/mod.rs` L178, `src/context/mod.rs` L483 |
| `kira digest` | comando que gera os posts (drafts) | `src/post/mod.rs` L1, `src/main.rs` L1311 |
| `.kira/digest/` | diretório dos drafts gerados pelo comando | `src/emit/mod.rs` L3, `src/tui/state.rs` L63 |

Colisão real: `.kira/digest.md` (arquivo, do ingest) vs `.kira/digest/` (diretório, do
comando) — mesmo nome, dois produtores.

**Canônico proposto**: manter `kira ingest` → `.kira/digest.md` (o "digest" substantivo,
já é o termo da ADR-0038); renomear o diretório dos drafts `.kira/digest/` → **`.kira/drafts/`**
(são rascunhos até o `emit` publicar). *(renome de código — precisa de selo)*

### C3 — Wiki: `docs/wiki/` (stale) vs `.kira/wiki/` (canônico)

O código e o AGENTS.md usam `.kira/wiki/` + `.kira/wiki-staging/`:
- `src/wiki/mod.rs` L30/L33: `WIKI_DIR = ".kira/wiki"`, `STAGING_DIR = ".kira/wiki-staging"`;
- `AGENTS.md` L93: "fonte de verdade em markdown **local** `<projeto>/.kira/wiki/`".

Mas vários docs ainda dizem `docs/wiki/` (commitada) — resíduo de antes do W2-FIX:
- `BUILD-PROGRAM.md` L14/L84/L90, `ADR-0013` (seções antigas), `PLANO-COMPLETO.md` L50,
  `roadmap-prs.md` PR28.

**Canônico**: `.kira/wiki/` (local, fonte de verdade) + `.kira/wiki-staging/` (staging do
ciclo autônomo). **Corrigir as referências stale `docs/wiki/`** *(renome de doc — barato)*.

### C4 — "post" = "artigo" = "carta" (a unidade de trabalho)

- Código/identificadores: `post` (módulo `src/post/`), `posts` (tabela).
- Doc PT-BR: "artigo" (ADR-0038 L32 "a unidade de trabalho é o artigo").
- Lore/UI: "carta/selo/carteiro" (`outbox`, `Courier`, `Postage`, `DeadLetter`, `Postmaster`).

**Canônico (por design, ADR-0038)**: `post` (EN, código) · `artigo` (PT-BR, doc) ·
`carta` (lore, só na doc/UI). **Não é bug** — documentar para não "corrigir" de volta.

### C5 — "Courier" (worker) vs "Postmaster" (loop)

A ADR-0038 L33 diz: "worker `Courier`, lease `Postage`, falha `DeadLetter`, **loop `Postmaster`**".
Mas os comentários de código (`src/post/mod.rs` L1 "o `Courier` que gera os posts") e os
meus docs (`docs/kira-brain.md` L51 "loop da digestão (o Courier)") chamam o **loop** de
"Courier".

**Canônico**: `Courier` = o worker (claim→gera→deliver, uma carta); `Postmaster` = o loop
(a função `run()` que itera a fila). **Corrigir os comentários e meus docs** para o loop ser
"Postmaster", não "Courier".

### C6 — EN (código/comando) vs PT-BR (doc/fala) — por design

`ingest`/`digest`/`emit`/`triage`/`scan` (EN, comandos/módulos) vs
ingestão/digestão/emissão/triagem (PT-BR, docs). É o contrato do AGENTS.md. **Manter e
documentar** a fronteira, para ninguém "traduzir" identificador.

---

## 2. Proposta original (supersedida — o glossário final está em `docs/nomenclatura.md`)

> Tabela abaixo é a proposta **inicial** da auditoria. O dono ajustou na revisão
> (ex.: `Postmaster`→`turn`, `Courier`→`phase`, `emit`→`publish`, `.kira/digest.md`→`.kira/facts.md`,
> `Postage`→`draft_ttl`, `DeadLetter`→`failed`, "artigo/carta"→`post`). O mapa final de rename
> vive em `docs/nomenclatura.md`.

| Conceito | Nome canônico | Aliases a aposentar | Onde |
|---|---|---|---|
| orquestrador externo (build) | **Hermes** | Orchestrator | `BUILD-PROGRAM`, ADRs "Decisores" |
| codificador (build) | **Delta** | — | `roadmap-prs`, ADRs "Decisores" |
| orquestrador interno (runtime) | **Brain** | master | `src/brain/`, `kira brain` |
| artefato do `ingest` | `digest.md` (substantivo) | — | `.kira/digest.md` |
| drafts do `digest` | `drafts` | `.kira/digest/` | `.kira/drafts/` |
| conhecimento do projeto | `wiki` (`.kira/wiki/`) | `docs/wiki/` | `src/wiki/` |
| conhecimento autônomo (staging) | `wiki-staging` | — | `.kira/wiki-staging/` |
| unidade de trabalho | `post` / artigo / carta | letters | `src/post/`, tabela `posts` |
| worker (uma carta) | **Courier** | — | `src/post/mod.rs` |
| loop da fila | **Postmaster** | Courier (como loop) | `src/post/mod.rs::run` |
| lease da carta | **Postage** | — | `postage_until` |
| carta extraviada | **DeadLetter** | — | `PostState::Dead` |

---

## 3. Consequências e ordem de correção

1. **Doc (barato, agora):** corrigir `docs/wiki/` → `.kira/wiki/` em `BUILD-PROGRAM.md`,
   `ADR-0013` (seções antigas), `PLANO-COMPLETO.md`, `roadmap-prs.md`; alinhar "Decisores"
   dos ADRs para `Hermes`/`Delta` consistentes; corrigir "loop Courier" → "loop Postmaster"
   nos meus docs.
2. **Código (um PR por item):** renomear `.kira/digest/` → `.kira/drafts/` (C2); alinhar
   comentário `src/post/mod.rs` L1 "Courier" → "Postmaster" para o loop (C5).
3. **Nada a fazer:** C4 (post/artigo/carta) e C6 (EN/PT-BR) — são por design, só documentar.

---

## 4. Decisões do dono (13/Nov/2026) — ver glossário final

As decisões N1–N4 (propostas abaixo) foram **ajustadas e seladas** pelo dono na revisão.
O resultado completo está em [`docs/nomenclatura.md`](../nomenclatura.md). Resumo do que
mudou em relação às propostas originais:

| Proposta original | Decisão final do dono |
|---|---|
| N1: `.kira/digest/` → `.kira/drafts/` | ✅ mantida (`.kira/drafts/`) |
| N2: loop=`Postmaster`, worker=`Courier` | **refinada**: `turn` (o loop, como turno de jogo de carta) + `phase` (verbos técnicos `claim`/`generate`/`deliver`) |
| N3: aposentar "Orchestrator" → "Hermes" + "Brain" | ✅ mantida (com `brain`/`kira-brain`) |
| N4: corrigir `docs/wiki/` stale → `.kira/wiki/` | ✅ mantida |
| *(novo)* `emit` | → **`publish`** |
| *(novo)* `.kira/digest.md` (artefato do ingest) | → **`.kira/facts.md`** |
| *(novo)* `Postage`/`lease` | → **`draft_ttl`** |
| *(novo)* `DeadLetter` | → **`failed`** |
| *(novo)* unidade "artigo"/"carta" | → **`post`** (larga a lore) |
