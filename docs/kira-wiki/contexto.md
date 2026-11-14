# Contexto

O KIRA (*Knowledge-Informed Remediation Agent*) é um binário único em Rust,
local-first, que vive dentro do seu projeto: percebe o repositório, lembra do que
aprendeu, julga itens de trabalho com evidência local e publica o que sabe em uma
wiki em markdown. Sem serviço, sem daemon, sem conta — um executável e um
SQLite.

## O problema que ele resolve

Um dev julga itens de trabalho o dia inteiro: é bug? é degradação? qual a causa
raiz? que arquivo tocar? Esse julgamento hoje depende de memória — a sua — que
não está no repositório. O KIRA cria essa memória **a partir do que está no
disco**: fatos varridos do código, achados das ferramentas padrão da stack,
episódios de bugs passados, decisões registradas. O julgamento passa a ter
evidência local citada como `caminho:linha`, não opinião de modelo.

Três princípios fecham o contrato:

- **Read-only por padrão.** Nenhum comando escreve no Azure DevOps nem no seu
  código. Escrita no mundo só com `--apply` explícito e, para autonomia, só
  depois de `awaiting_approval` + `kira approve` humano. O único disco que o
  KIRA toca é o dele (`~/.config/kira/`) e a pasta local do projeto (`.kira/`).
- **Erro acionável.** Toda falha diz o que fazer. Nunca body cru de HTTP ou
  stderr em output.
- **Determinismo antes de inteligência.** Mesma entrada = mesma saída byte a
  byte em tudo que não passa por LLM. O LLM é o último recurso (ver
  [[padroes]]).

## O metabolismo: ingest → digest → publish

O ciclo do projeto tem três verbos, três contratos (ADR-0038):

1. **`kira ingest`** — fatos, sem LLM: uma caminhada com denylist (nada de
   allowlist de extensão — stack nova entra de graça), detector de stack,
   índice FTS5 incremental e `.kira/facts.md` determinístico. Segundos,
   byte-idêntico para o mesmo estado.
2. **`kira digest`** — julgamento com IA local (Ollama): enfileira o que falta
   em `posts`, gera rascunhos em `.kira/drafts/` com cache por
   `(input_hash, prompt_version, model, lang)` e orçamento explícito.
   **Exige o aceite do repositório (`kira trust`)** — é ele que manda código ao
   modelo.
3. **`kira publish`** — publica os rascunhos em `.kira/wiki/` atrás de gate por
   hash (`source_hash`/`published_hash`). Nunca sobrescreve página editada à
   mão sem `--force`.

`ingest` é read-only e roda sempre; `digest` e `publish` são as portas onde o
consentimento e o gate vivem. A escada completa, do prompt ao sistema, está em
[[engenharia-harness-toolkit]]; as decisões de engenharia do binário, em
[[engenharia-kira]].

## A escada: prompt → skill → loop → graph → harness → system design

A autonomia do KIRA é uma escada, e cada degrau tem custo declarado:

| Degrau | O que é | Estado no KIRA |
|---|---|---|
| prompt | uma pergunta com contexto empacotado | `kira triage`, packer retrieval-first |
| skill | instrução reutilizável versionada | `kira skills` (onda SK, SK-1) |
| loop | o ciclo que repete sozinho | `kira loop` (PR15) |
| graph | grafo de estados do orquestrador interno | `brain` (desenhado, `src/brain/`) |
| harness | guarda com piso + veredito tipado | proposta (PR44–51) |
| system design | o binário inteiro | esta wiki |

## Onde o KIRA está hoje

O que o código entrega, onda por onda (PRs reais no git):

- **W1 segurança/erros** — `redact`, `guard/path`, taxonomia de erro (PR1–4).
- **W2 contexto/eval** — packer v1, prompt versionado, `LlmBackend` (PR5–8).
- **W2.5 wiki** — `.kira/wiki/` + FTS5 + episódios + promote/forget (PR28–32).
- **W3 flywheel** — `scan` determinístico + `pr draft` (PR9–10).
- **W4** — Copilot spawn, install verificado (PR11).
- **W6 dashboard** — Ratatui read-only; leitura por padrão, ação só confirmada
  (ADR-0033).
- **Onda M (onboarding)** — `ingest` (#84), posts+trust (#85), `digest` (#86),
  `publish` (#87) fechados; a caixa de correio (M4) está em aberto.
- **Renames da nomenclatura canônica** (#89–#95) — o vocabulário de correio
  foi aposentado de código: verbos e nomes passaram a ser os do glossário
  selado ([[glossario]]).
- **P0 observabilidade** — contador `llm_calls` (schema v9): o budget de
  50 chamadas de LLM por dia virou número medido, não promessa.
- **`kira loop` (PR15)** — tick = `ingest → digest (só se trusted) → publish`,
  tudo local, com e2e provando o segundo tick byte-idêntico.
- **Onda SK (SK-1, #107)** — catálogo de skills: bundled embutido no binário +
  `.kira/skills/` do usuário, com FTS5 derivado (ver
  [[engenharia-agent-skills]]).
- **Onda HF (ADR-0040, #103)** — Hugging Face é CATÁLOGO de GGUF consumido via
  Ollama (`kira model list`/`kira model pull`), nunca runtime de inferência.

O que falta para pulsar sozinho, na ordem da fila: `kira approve` + caixa de
correio no dashboard (PR18/M4), política de quota/stale do budget (PR17),
gold rotulado pelo dono (20 bugs — sem ≥70% medido no `kira triage --eval`,
nada de "autônomo"), cache de raciocínio e piso de guardas (ondas TE/TL).

## As três memórias

O KIRA separa memória em três lugares com contratos diferentes (mapa §6):

| Memória | Onde | Contrato |
|---|---|---|
| Estado e trilha | `store.db` (SQLite bundled) | sessões, fatos, fila, índices; fail-open (a trilha nunca trava o comando) |
| Conhecimento | `.kira/wiki/` (markdown) | fonte de verdade; índice FTS5 derivado e descartável |
| Segredo | keyring do OS | PAT e chave Foundry; nunca em arquivo, log ou prompt |

Regra transversal: **markdown é a fonte de verdade** — wiki, skills e facts
são arquivos que você lê e edita; o índice é reconstruído quando você quiser
(`kira wiki reindex`, `kira skills reindex`).

## A imunidade (as travas do metabolismo)

O ciclo é poderoso e, por isso, cercado — cinco travas que o código prova
([[padroes]] traz o teste de cada uma):

1. **redact na escrita do índice** — o store nunca guarda segredo, nem em
   dado derivado;
2. **prompt delimitado** — o work item é dado, não instrução (grounding do
   prompt de triage; guardrails contra injeção invisível, ADR-0024);
3. **jail de caminho** — toda escrita confinada à raiz, id inválido recusado
   antes de I/O;
4. **trust gate** — código não vai ao modelo sem o aceite do repositório;
5. **read-only default + TUI com confirmação** — navegar nunca executa; ação
   só confirmada, com hash byte-idêntico do store provado no e2e.

E a invariante-mestra que amarra tudo: **mesma entrada = mesma saída byte a
byte em tudo que não passa por LLM**; o que passa por LLM é identificado por
`(model, prompt_version, input_hash)` e orçado.

## O que o KIRA não é

- **Não é daemon**: `kira loop` é modo do binário; agendador externo chama o
  executável ([[decisoes]], decisão 5).
- **Não é framework de orquestração**: sem tool-calling, sem LangGraph, sem
  índice vetorial — CLI real → parser/SQL → cálculo em Rust → LLM (ADR-0016).
- **Não é registry de conhecimento alheio**: as skills dele carregam
  conhecimento do projeto ([[engenharia-agent-skills]]).
- **Não é produto que promete**: o que não está no binário está na fila do
  roadmap, não no README como pronto.
- **Não é escritor**: leitura é livre; escrita no mundo exige `--apply` e,
  no ciclo autônomo, `awaiting_approval` + `kira approve` humano. Merge é
  sempre humano.

## Como navegar nesta wiki

- [[getting-started]] — a jornada do zero, comando por comando.
- [[engenharia-kira]] — decisões do binário: store, erros, guard, budget.
- [[engenharia-agent-skills]] — o formato de skill e o catálogo.
- [[engenharia-harness-toolkit]] — a escada de autonomia aplicada ao KIRA.
- [[decisoes]] — ADRs 0011–0040 e as cinco decisões que moldam o binário.
- [[padroes]] — invariantes que o código prova (e o teste que prova cada um).
- [[glossario]] — nomes canônicos e termos de RAG/medição.
