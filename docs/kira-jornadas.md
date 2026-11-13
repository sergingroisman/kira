# KIRA — as jornadas (local-first, com capacidades)

> Levantado do binário e **reescrito** em 12/Nov/2026 após a decisão do ADR-0030: o KIRA é
> **local-first**; integrações são **capacidades** conectadas quando o dev quiser.

## Princípio

A fonte da verdade é o **repositório local do dev**. Board, GitHub, GitLab e qualquer serviço externo
são **capacidades opcionais**. Nenhuma jornada local pede credencial; nenhuma conexão acontece sem o
dev mandar (`kira connect`).

```mermaid
flowchart LR
  FS[Repositório do dev] --> CORE[KIRA local<br/>scan · codex · lens · scribe · watch · ingest]
  CORE --> STORE[(store.db + .kira/)]
  CORE -.->|opcional: IA local| OLL[Ollama]
  CORE -.->|capacidade board.azure| ADO[Azure DevOps]
  CORE -.->|capacidade scm.github| GH[GitHub]
  CORE -.->|capacidade scm.gitlab| GL[GitLab]
```

## Matriz de capacidades (é contrato de teste, não documentação)

| Jornada | Capacidade | IA | Sem capacidade |
|---|---|---|---|
| J1 `scan` | — | — | roda |
| J3 `codex` (wiki) | — | — | roda |
| J4 `lens` (context) | — | — | roda |
| J6 `scribe` (pr draft) | — | — | roda (markdown local) |
| J8 `watch` (dashboard) | — | — | roda |
| J5 `judge --file` | — | obrigatório | `precisa de backend de IA — kira setup --backend ollama` |
| J5 `judge <id>` | `board.azure` | obrigatório | `precisa de board.azure — kira connect azure` |
| J9 `read` (inspect) | `board.azure` | — | idem |
| Publicar PR / issue | `scm.github`/`scm.gitlab` | — | `precisa de scm.github — kira connect github` |
| J7 `verdict` (eval) | — | obrigatório | erro acionável, nunca medição parcial |
| J10 `sentinel` | — | obrigatório | bloqueado pelo gate M2 |
| JM1 `ingest` | — | — | roda (read-only, **sem** trust) |
| JM2 `trust` | — | — | roda (grava `.kira/trust.json`; é aceite, não capacidade) |
| JM3 `digest` | — | obrigatório (Ollama primeiro) | sem trust: **recusa** e diz o que fazer; sem IA: erro acionável |
| JM4 `emit` | — | — | roda (publica rascunhos; gate por hash, `--force` única porta) |

## MVP — o que é "KIRA cru"

**MVP = as 5 jornadas que rodam com zero credencial**, mais o julgamento local com `--file`:

| # | Jornada | Por que é MVP |
|---|---|---|
| J1 | `scan` | é a percepção: sem isso o KIRA não tem o que dizer |
| JM1/JM2 | `ingest` + `trust` | da onda M: conhecimento do projeto sem credencial; o `digest` que deles come (M3) precisa de IA, mas a matéria-prima é toda local |
| J4 | `lens` | mostra o que iria ao modelo; é o "ver antes de pagar" |
| J5 | `judge --file` | o julgamento local — o coração, provado offline |
| J6 | `scribe` | entrega o artefato que o dev usa (PR em markdown) |
| J8 | `watch` | é onde o dev vê o estado sem rodar comando |
| J3 | `codex` | memória: sem wiki, o KIRA não aprende entre execuções |

**Pós-MVP (capacidade)**: J9 `read` e `judge <id>` (board.azure), publicação em GitHub/GitLab,
J7 `verdict` (precisa do gold set do dono), J10 `sentinel` (gate M2).
**Pós-MVP (conveniência)**: J2 `profile`/`note`/`history` — funcionam hoje, mas não são o motivo de instalar.

---

## J0 — Onboarding (local-first, zero PAT, projeto conhecido no fim)

```mermaid
flowchart TD
  A[kira setup] --> B[nome do dev<br/>default do SO]
  B --> C[projeto: raiz detectada + confirmar]
  C --> J[ingestão: preview read-only<br/>Enter ingere · n pula · l idioma]
  J --> D[backend de IA<br/>1 ollama · 2 copilot · 3 foundry · 4 nenhuma por agora]
  D --> E[resumo honesto<br/>dev · ia · idioma · store · ingestão]
  E --> F[funciona agora: scan · codex · lens · scribe · watch]
  E --> G[desativado: judge de item do board · read · publicar PR]
  E --> H[próximo passo: kira connect azure<br/>ou kira triage &lt;id&gt;, com board conectada]
  H --> I[kira doctor]
```

- **Não pede PAT.** Credencial nasce em `kira connect <provider>`.
- **Ingestão no setup (M6):** o `ingest` roda no motor, sem scan e sem rede — o mesmo
  ` kira ingest` que o dev pode rodar depois; pular tem comando de retomada (`kira ingest`).
- **Idioma (ADR-0038 D4):** `--lang`/`l` escolhe o idioma do que o humano lê; `.kira/**` e
  prompts internos ficam sempre em inglês.
- "Nenhuma IA por agora" é opção explícita e o default quando Ollama não existe.
- O resumo separa **pronto** de **pulado** e sempre diz o comando de retomada.

## J-C — Conectar capacidade (novo)

```mermaid
flowchart TD
  A[kira connect azure] --> B[aula do PAT<br/>o que é, como criar, escopo Work Items Read]
  B --> C{menu: colar · usar env · cancelar}
  C --> D[valida agora: 200/401/403/sem rede]
  D -->|ok| E[grava no keyring + capacidade conectada neste projeto]
  D -->|falha| F[mensagem com a causa e o link de correção]
```

`kira connect github` / `kira connect gitlab` seguem o mesmo desenho (token com escopo mínimo:
ler issues, abrir PR; nunca push em branch protegida).

## JM — Do repositório à wiki (ingest · trust · digest · emit, ADR-0038)

```mermaid
flowchart LR
  I[kira ingest<br/>fatos, determinístico] --> S[(inventory.json<br/>digest.md · índice FTS5)]
  S --> T[kira trust<br/>aceite do repositório]
  T --> G[kira digest<br/>posts na fila SQL · Ollama]
  G --> O[(outbox: rascunhos<br/>.kira/digest/)]
  O --> E[kira emit<br/>gate por hash]
  E --> W[.kira/wiki/&lt;slug&gt;.md]
```

Quatro contratos, um verbo cada (ADR-0038 D1):

| Verbo | O que jura | O que **não** faz |
|---|---|---|
| `ingest` | uma caminhada, denylist + `.gitignore` honrado (M7-P5), sniff binário, teto com nota; `digest.md`/`inventory.json` byte-idênticos para o mesmo estado | não usa IA, não pede trust, não toca a rede (o scan é ferramenta, não aqui) |
| `trust` | `.kira/trust.json` com o hash do caminho canônico; mudar de lugar o repo pede aceite de novo | não é capacidade de board; `--revoke` desfaz |
| `digest` | exige trust; fila `posts` em SQL com lease; Ollama sempre primeiro; `lang` no `cache_key`; 1 M/run + 30 k/post com parada dura e nota | sem aceite: recusa com o comando; backend caro nunca implícito |
| `emit` | publica em `.kira/wiki/<slug>.md` com front-matter e `index.md` derivado; idempotente; post editado à mão (ou wiki escrita do zero) é **recusado** sem `--force`, com o caminho na mensagem | nunca sobrescreve em silêncio; `--force` é a única porta |

A caixa de correio do dashboard (M4) consome a fila em modo leitura; o `kira doctor`
reporta ingestão, trust, fila e posts desatualizados.

---

## J1 — Percepção: `scan`

Igual ao desenho anterior: stacks (node/dotnet), workspace pnpm dobrado no root, resolução de
ferramenta no PATH (PATHEXT no Windows), finding com fingerprint, saída determinística.
**Zero rede, zero credencial.** O parse tolerante (BOM/UTF-16) é **implementado** (M7-P1,
branch `fix/m7-bom-scan`): `src/scan/decode.rs` inspeciona o BOM do stdout cru antes de
parsear — UTF-8, UTF-16 LE e BE — porque redirect do PowerShell escreve UTF-16.

## J3 — Conhecimento: `codex` (wiki)

`list` · `show` · `decision` · `reindex` · `search` · `forget` · `promote`.
Conhecimento, não log; redação antes de escrever; `forget` apaga do disco **e** do índice.

## J4 — Contexto: `lens`

Monta o pack (findings + **perfil/árvore do `.kira/digest.md`** + wiki + trechos de código)
com a ordem de orçamento declarada e teto por fonte, `CHARS_PER_TOKEN=3`.
`--explain` mostra por que cada item entrou (inclusive o corte do digest, quando ele não
coube); `--json` para avaliação. **Zero token.**

## J5 — Julgamento: `judge`

Duas portas, ambas explícitas:
- `judge --file item.json` — **offline**, precisa só de backend de IA (é o caminho do MVP).
- `judge <id>` — precisa de `board.azure` conectado.

Caro nunca é default, nunca fallback silencioso; fail-closed.

## J6 — Escrita: `scribe` (pr draft)

Gera o markdown do PR do que já se sabe (Contexto, Causa raiz, Mudança, Evidências, Riscos, Checklist),
com `—` onde falta dado. **Não abre PR, não faz push.** Publicar é capacidade `scm.*`.

## J7 — Medição: `verdict` (`--eval`)

Compara contra gold.json por similaridade determinística (limiar 0,50) e basename de arquivo; aborta
em caso de falha; exit 0/1. Sem LLM juiz.

## J8 — Vigilância: `watch` (dashboard)

5 painéis (Resumo · Achados · Sessões · Wiki · **Posts**), read-only, provado por hash do
store. A caixa de correio (M4) lista a fila `posts` com o marcador `[não lido]` na célula e abre
o rascunho entregue em tela cheia com o renderizador nativo; a única escrita possível — o
carimbo `read_at` ao abrir — mora na store do próprio dashboard, é idempotente e some com
`--read-only`. O painel também mostra **o que está conectado / desativado** — é o que evita o
dev achar que "não funciona".

## J9 — Inspeção viva: `read` (inspect)

Capacidade `board.azure`. 401/403/404 com causa provável.

## J2 — Identidade e trilha

`profile` · `note` · `history`. Local, sem capacidade.

## J10 — Autonomia: `sentinel`

Spec em `docs/kira-w5-loop.md`, bloqueada pelo gate M2. Sem o número, fica em modo sugestão.

---

## Resumo por dependência

| Precisa de… | Jornadas |
|---|---|
| **nada** | J0, J1, J2, J3, J4, J6, J8, JM1 `ingest`, JM2 `trust`, JM4 `emit` |
| **backend de IA** | J5 (`--file`), J7, J10, JM3 `digest` (que também exige `trust`) |
| **board.azure** | J5 (`<id>`), J9 |
| **scm.github / scm.gitlab** | publicar PR/issue |