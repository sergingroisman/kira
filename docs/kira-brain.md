# KIRA Brain — a escada `prompt → skill → loop → graph → harness → system design` em Rust

> **Data**: 13/Nov/2026 · **Status**: visão consolidada (dono) · **É o doc-pai**: revisa e
> amarra os demais planos num desenho só. Tudo em Rust, sem framework de orquestração
> (sem LangGraph/LangChain/MCP — veto do mandato).

---

## 1. A ideia central

O KIRA é **um único binário Rust** que implementa a escada de engenharia de agente como
**módulos do próprio código** — não como dependências. O **Brain** é o orquestrador que
amarra **ingestão → digestão → emissão → triagem → fix** numa linha, com **loops internos**
contidos nos nós. Cada degrau da escada é um módulo com dono claro (ADR-0022):

```
prompt → skill → loop → graph → harness → system design
  (o que     (como      (quando    (por onde  (o que      (o Brain:
   dizer)     fazer)     repetir)   ir)        protege)     quem amarra tudo)
```

A regra que não muda (ADR-0016, CLI-first/LLM-last): **quanto mais à esquerda um degrau
resolve, melhor.** O KIRA só sobe a escada quando o degrau de baixo não responde — e o
orçamento (`max_llm_calls_per_day=50`) mede exatamente quantas vezes o LLM foi chamado.

---

## 2. A escada mapeada para Rust (evidência de código)

| Degrau | O que é | Módulo Rust | Estado |
|---|---|---|---|
| **prompt** | instrução versionada, delimitada, com grounding e `path:line` | `src/llm/prompts.rs` (triage **v2**, FNV-1a), `src/post/prompts.rs` (digest **m3-v1**, EN) | ✅ feito |
| **skill** | SOP recuperável por FTS5 (react, tailwind, tanstack, shadcn, drawio…) | `src/skills/` (bundled + `.kira/skills/`) | ⏳ **SK** (planejado) |
| **loop** | ciclo de controle com `draft_ttl`, budget, cache e retry | `src/post/mod.rs` (o **turn**, L182–269; fases `claim`/`generate`/`deliver`), `src/triage.rs` | ✅ feito (digestão) |
| **graph** | DAG determinístico CLI-first (coleta paralela antes do LLM) | `src/ingest/` → `src/post/` → `src/publish/`; `src/context/` (packer) | ✅ implícito (ADR-0016) |
| **harness** | guard, trust, redact, budget, approve | `src/guard/path.rs`, `src/trust.rs`, `src/redact.rs`, `src/store/posts.rs` (lease) | ✅ parcial; ⏳ **TL** (piso+veredito) |
| **system design** | o **Brain**: orquestra a escada de ponta a ponta | `src/brain/` (novo) | ⏳ planejado |

---

## 3. Os loops internos (já existem, comprovados no código)

A frase "ingestão e digestão terão loops internos" **já é verdade no código atual** — o que
falta é dar nome e amarrar no Brain.

### 3.1 Loop da ingestão (determinístico, zero LLM)
`src/ingest/mod.rs::run` → `inventory::walk` (caminhada ordenada por nome, com
`filter_entry` de denylist) → `context::reindex` (loop incremental por `(mtime,size)`).
É um **loop de grafo**, não de LLM: mesma árvore ⇒ mesma saída, byte a byte.

### 3.2 Turno da digestão (o `turn` — o loop interno central; fases `claim`/`generate`/`deliver`)
`src/post/mod.rs::run` (L182–269):

```
loop {
    se pause_after atingido → break;
    se budget esgotado       → break (nota);
    carta = store.post_claim(lease 120s)  // UPDATE … RETURNING, idempotente
    se fila vazia            → break;
    (system, prompt) = build_prompt(kind, lang, digest, evidence, teto);
    resposta = backend.complete(system, prompt);   // LLM porque: (comentário no call site)
    match resposta { Ok → deliver; vazio/falha → post_fail; backend quebrou → break; }
}
```

As propriedades que fazem esse loop ser **engenharia, não promessa**:
- **lease** (`postage_until`): processo morre ⇒ a carta volta sozinha à fila;
- **cache** (`source_hash` + `prompt_version` + `model` + `lang`): segundo run não refaz;
- **budget** (1 M/run + 30 k/post): parada dura com nota, nunca loop infinito;
- **retry capado**: backend quebrou ⇒ `break` (não tenta 3× por carta com o mesmo erro).

### 3.3 Loop da emissão (gate de idempotência)
`src/publish/mod.rs`: publica só o que o `digest` entregou, e **recusa** sobrescrever post
editado à mão (`published_hash` no front-matter). É o `verifier` determinístico do degrau
de emissão — `author ≠ verifier`.

---

## 4. O Brain (`src/brain/`) — o degrau "system design"

Hoje o dev roda `kira ingest`, depois `kira digest`, depois `kira publish` à mão (o `briefing.md`
até sugere o próximo comando). O **Brain** é o que falta: um orquestrador que roda essa
escada **como um grafo de estados em Rust**, não como um loop aberto de LLM.

```mermaid
flowchart TB
    B[kira brain --apply] --> ING[Nó ingest · grafo determinístico]
    ING --> DIG[Nó digest · turno (turn)]
    DIG -->|fila vazia| EMI[Nó publish · gate published_hash]
    EMI --> TRI[Nó triage · loop interno (1 LLM/achado)]
    TRI --> FIX[Nó fix --apply · 1 passo isolado]
    FIX --> VER{Nó verifier\nauthor != verifier}
    VER -->|falhou, tentativa < max| FIX
    VER -->|verde| REP[Nó report / pr draft]
```

**Leis do Brain (herdadas dos vídeos e do mandato):**

1. **Grafo em Rust, loop só nos nós de julgamento.** Os estados são nós; as transições
   são arestas determinísticas; o LLM só entra dentro de `digest`/`triage`/`fix` (as folhas
   exploratórias), nunca controlando a topologia.
2. **Imutabilidade de plano** — a ordem `ingest→digest→publish→triage→fix` é fixa; o modelo
   não reescreve o próprio plano no meio do run. Só o humano reordena (via flag/approve).
3. **`author ≠ verifier`** — o nó que gera (digest/fix) nunca é o nó que aprova; o
   `verifier` é deterministico (fmt/clippy/test/refs `path:line`) antes de qualquer LLM.
4. **`awaiting_approval` antes de escrever no mundo** (ADR-0017) — o Brain nunca passa
   de `fix` sem `kira approve <id>` + `--apply`.
5. **Perfil adaptativo** — `Guided` (passos curtos) no Ollama 4B local; `Spec-Lean`
   (1 contrato + checklist) no Foundry/Copilot.
6. **Skills antes do fix** — o catálogo (SK) é quem dá o SOP ao `fix`; sem skill casada,
   o Brain não inventa procedimento.

---

## 5. Revisão dos prompts (o degrau "prompt" hoje)

| Prompt | Estado | Veredito |
|---|---|---|
| `TRIAGE_SYSTEM_PROMPT` (v2) | 6 regras numeradas, anti-injeção (`<<<CONTEÚDO NÃO CONFIÁVEL>>>`), 3 seções fixas, read-only | ✅ sólido; é o "prompt engineering" do vídeo 1 aplicado |
| `post::prompts::SYSTEM` (m3-v1) | grounding, sem invenção, `path:line` obrigatório, `not available` em vez de chute, GFM só | ✅ sólido; "use ONLY the facts provided" = grafo-first |
| `intro` / `c4_container` | template com seções em ordem fixa + evidência ancorada | ✅ sólido |
| *(SK, planejado)* | frontmatter (name/description com trigger+anti-trigger/license/author/version) | ⏳ segue o formato do agent-skills, reimplementado |

**O que a escada pede de ajuste nos prompts** (já capturado nos planos):
- **Skill como dado, não como system prompt** — o `description` das skills **não** entra
  no system fixo; o FTS5 + `stack::detect` seleciona `MAX_SKILLS` antes de montar o prompt
  (senão a janela de 4096 estoura — lição do vídeo 2).
- **Checklist de verificação no nó `verifier`** — extraído do item (`Given/When/Then`),
  numa chamada limpa, sem o histórico de racionalização do autor (lição dos "38/39 critérios").

---

## 6. Reconciliando os documentos (índice de planos)

| Doc | Papel na escada | Onda |
|---|---|---|
| `docs/ingestao-repomix-gitingest-plano.md` | melhora o nó `graph`/`gather` da ingestão | **IG** |
| `docs/ig-implementacao.md` | itens executáveis (IG-0..IG-6) do degrau `graph` | **IG** |
| `docs/skills-rag-plano.md` | constrói o degrau **`skill`** + o `knowledge` (RAG) | **SK** |
| `docs/loop-vs-graph-kira.md` | teoria (vídeos) que justifica `graph` + `harness` | — |
| `docs/compozy-vale-trazer.md` | referência de prompt engineering (seções XML + contrato) | — |
| `docs/agent-skills-harness-vale-trazer.md` (existente) | degrau **`harness`** (piso + veredito + custódia) | **TL** |
| `docs/te-implementacao.md` (existente) | degrau **`loop`** (cache de raciocínio, economia) | **TE** |

---

## 7. Ordem de execução consolidada (a escada de baixo para cima)

```
1. IG  (graph/gather)     → ingestão perfeita (BOM UTF-16, .kiraignore, segredos)
2. SK  (skill)            → catálogo + knowledge (RAG) — ANTES do fix, como os vídeos mandam
3. TE  (loop)             → cache de raciocínio + economia medida
4. TL  (harness)          → piso de guardas + veredito + custódia de prompt
5. W5  (system design)    → o Brain: grafo de estados + verifier independente
```

**Regra de ouro que amarra tudo**: o Brain só existe porque cada degrau de baixo já é
determinístico. O `loop` só é seguro porque o `graph` já trouxe o contexto certo; o `graph`
só é preciso porque o `skill` já disse o procedimento; o `harness` só confia porque o
`prompt` já está versionado e redigido. **É uma escada, não um monte de features soltas.**
