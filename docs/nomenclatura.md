# Nomenclatura do KIRA — glossário canônico (SELADO)

> **Data**: 13/Nov/2026 · **Status**: decidido pelo dono (revisado e ajustado) ·
> **Regra-mestra**: um nome por conceito, e o nome diz o que a coisa é. Sem metáfora de
> correio; a metáfora de jogo de carta (turno) fica **só na ideia de "turno"**, não nos verbos.

---

## 1. Glossário canônico (uma tabela só)

| Conceito | Nome canônico | Aliases aposentados |
|---|---|---|
| lê os fatos do repo (sem LLM) | **`ingest`** | — |
| gera os posts (LLM) | **`digest`** | — |
| publica os posts na wiki | **`publish`** | `emit` |
| escada de arquitetura | **`prompt → skill → loop → graph → harness → system design`** | — |
| orquestrador interno (runtime) | **`brain`** (`kira brain`, `src/brain/`) | `master` |
| agente de build (orquestra os ciclos) | **`Hermes`** | `Orchestrator` |
| codificador (build, futuro) | **`Delta`** | — |
| grafo de estados do Brain | **`graph`** | — |
| loop que repete (o ciclo da fila do digest) | **`turn`** | `Postmaster` |
| uma ação dentro do turno (claim → generate → deliver) | **`phase`** (verbos técnicos) | `Courier`, `step` |
| tempo de vida do draft no claim | **`draft_ttl`** | `Postage`, `lease`, `postage_until` |
| post que falhou | **`failed`** | `DeadLetter`, `dead-letter` |
| unidade de trabalho | **`post`** | `artigo`, `carta`, `letter` |
| artefato do `ingest` | **`.kira/facts.md`** | `.kira/digest.md` |
| rascunhos do `digest` | **`.kira/drafts/`** | `.kira/digest/` |
| conhecimento publicado | **`.kira/wiki/`** + `.kira/wiki-staging/` | `docs/wiki/` |

**Hierarquia do loop**: `graph → turn → phase`. O grafo tem nós; cada nó pode ter um
**turno** (o loop que repete, como turno de jogo de carta); cada turno tem **fases**
(verbos técnicos: `claim`, `generate`, `deliver`).

---

## 2. Mapa de rename (old → new)

### Código (renomeia identificador — PR por item)

| Old | New | Onde |
|---|---|---|
| comando `kira emit` | `kira publish` | `src/main.rs`, `src/emit/mod.rs` → `src/publish/` |
| `.kira/digest.md` | `.kira/facts.md` | `src/ingest/mod.rs`, `src/post/mod.rs`, `src/context/mod.rs`, `src/doctor.rs`, `src/setup.rs`, `src/main.rs` |
| `.kira/digest/` | `.kira/drafts/` | `src/post/mod.rs`, `src/emit/mod.rs`, `src/tui/state.rs`, `src/tui/widgets/posts.rs` |
| coluna `postage_until` | `draft_ttl` | `src/store/posts.rs`, migração |
| `PostState::Dead` | `PostState::Failed` | `src/store/posts.rs` |

### Doc/comentário (renomeia texto — barato)

| Old | New | Onde |
|---|---|---|
| "o `Courier`" (loop/worker) | "o **turno**" / "a **fase**" | `src/post/mod.rs` L1, meus docs |
| `Postmaster` | `turn` | ADR-0038, BUILD-PROGRAM |
| `Postage` | `draft_ttl` | ADR-0038 |
| `DeadLetter` | `failed` | ADR-0038 |
| `docs/wiki/` (stale) | `.kira/wiki/` | BUILD-PROGRAM, ADR-0013 (seções antigas), PLANO-COMPLETO, roadmap-prs |
| `Orchestrator` | `Hermes` | BUILD-PROGRAM, ADRs "Decisores" |
| `artigo` / `carta` | `post` | docs |

---

## 3. Como aplicar (ordem)

1. **Docs (agora)**: atualizar meus docs + o ADR-0038/BUILD-PROGRAM com os nomes canônicos.
2. **Código (PR por item)**: `emit`→`publish` · `.kira/digest.md`→`.kira/facts.md` ·
   `.kira/digest/`→`.kira/drafts/` · `postage_until`→`draft_ttl` · `Dead`→`Failed`.

---

## 4. Regras permanentes

- Código, comandos, identificadores, colunas de DB: **sempre inglês**.
- Docs, fala ao usuário, UI: **PT-BR**.
- A ideia de **turno** (jogo de carta) vive na doc/UI; os verbos das fases são técnicos
  (`claim`, `generate`, `deliver`).
- Renomear nome de comando/coluna/estado **quebra compatibilidade** (migração + flags) —
  por isso cada rename de código é um PR isolado, com gate verde e nota de migração.
- **Estaleiro não é sistema**: `Hermes`/`Delta` (agentes de build) e Kombai
  (ferramenta de design do dono) **constroem** o KIRA com o dono — nunca aparecem
  em diagramas C4, atores de sistema ou fluxos de runtime. C4 descreve só o
  binário em execução e os sistemas externos com que ele fala; menções em docs
  de processo (BUILD-PROGRAM, roadmap) ficam como registro de *quem constrói*.
