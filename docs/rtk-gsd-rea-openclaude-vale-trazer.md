# Relatório — O que vale trazer para o KIRA (RTK · GSD-2 · rea · openclaude)

- **Data**: 11/Nov/2026
- **Fontes**: 4 repositórios escolhidos a dedo pelo dono, analisados por README/docs/
  issues públicas (nenhum código copiado; licenças conferidas)
- **ADR proposto**: ADR-0026 (cache de raciocínio + economia medida) — draft na
  seção final, vira arquivo só com aceite do dono (ADR-0024/0025 já foram tomados
  por outras sessões — e há uma colisão de dois ADR-0025 em `docs/adr/`, para o
  Orchestrator resolver)
- **Dumps**: `docs/dumps/snapshot-{gsd-2,openclaude,rea}.md` (fornecidos pelo dono
  em 12/Nov; proveniência em `docs/dumps/snapshot-README.md`) — o dump do RTK
  segue faltando
- **Veredito**: 1 pulo do gato + 10 melhorias (fila PR34–PR43); nenhum contrato do
  mandato quebrado; nenhuma dependência nova; nada disso depende de daemon, MCP,
  embeddings ou modelo caro.

---

## O pulo do gato (TL;DR)

Os 4 repositórios atacam o mesmo problema por ângulos diferentes — **o token mais caro
é o que o modelo lê e não precisava ler**:

| Repo | O que corta | Mecanismo |
|---|---|---|
| RTK | a **saída** dos comandos | filtro por comando antes do modelo ler (56% média, até 90% nos mais ruidosos — números do site deles) |
| GSD-2 | a **conversa** | sessão nova por unidade, digest em disco (`gsd_exec`), routing heurístico sub-ms, pressão de budget |
| rea | a **repetição** | "nada é encontrado duas vezes": evidência bruta vai para arquivo, só o relato volta |
| openclaude | a **descoberta** | repo map estrutural pré-injetado no contexto (≤2048 tokens) |

O KIRA **já tem o esqueleto** disso tudo: context packer com teto (`src/context/`),
findings.json determinístico, wiki FTS5, CLI-first/LLM-last, budget com número. O que
falta é duas coisas:

1. **Medir a economia que o KIRA já produz.** O KIRA não tem um `rtk gain` próprio:
   ninguém sabe hoje quantos chars o pack economizou do teto, nem quantas chamadas de
   LLM o ciclo evitou. Sem número, otimização é opinião — o dono já impôs esse
   princípio para autonomia (gate M2); o mesmo vale para token.
2. **Fechar o ciclo de recorrência.** Em sustentação, *o bug de hoje é o bug de ontem*.
   O episódio da wiki que guarda a causa raiz do item anterior é um cache de
   raciocínio esperando para ser usado: **se a wiki já sabe a causa, o triage não
   precisa de LLM — zero token, resposta determinística, marcada como
   `cache-wiki`**. É o princípio retrieval-first levado ao fim (ADR-0015), medido pelo
   mesmo eval determinístico do M2 (similaridade ≥ 0.50, ADR-0018). GSD-2 economiza
   token roteando modelo; o KIRA pode economizar mais barato: *não perguntar*.

Esse é o pulo do gato: **o triage que acerta por memória não gasta token** — e no uso
real de sustentação, itens recorrentes são a maioria.

---

## A jornada — por que cada um é rápido

### RTK (Rust Token Killer) — `rtk-ai/rtk`, Rust, Apache-2.0 (motor BUSL-1.1)

- Binário único sem dependências; intercepta o comando do agente, roda
  `<rtk> <cmd>` e filtra a saída antes do contexto. Overhead < 25 ms.
- 4 estratégias, todas determinísticas: **filtrar** (só falhas de teste, contagem dos
  que passam), **agrupar** (erros por arquivo/regra), **truncar** com endereço,
  **deduplicar** (linha repetida colapsa com contagem).
- Três promessas que valem ouro para o KIRA: *nunca bloqueia* (filtro falhou ⇒ saída
  crua, exit code intacto), *nunca pior* (nunca imprime mais que o comando), *nada
  escondido* (o que foi cortado diz onde está a versão completa).
- `rtk gain`: medidor de economia com histórico de 30 dias.
- **Honestidade**: o próprio projeto avisa que os % são sobre *bytes de saída de
  comando*, não sobre a conta total — a economia dilui no contexto inteiro. Um
  cético no HN chamou o projeto de "ilusão de compressão"; a resposta honesta é que
  a métrica certa é a que o KIRA medir no próprio pack.

### GSD-2 — `gsd-build/gsd-2` → `open-gsd/gsd-pi`, TypeScript, MIT

- Saiu de "framework de prompt" para app que **controla a sessão** do agente: sessão
  nova por unidade (contexto limpo, sem lixo acumulado), contexto **pré-injetado** no
  dispatch (o modelo não gasta tool-call lendo arquivo).
- `gsd_exec`: comando ruidoso (scan/build/teste) escreve stdout completo em
  `.gsd/exec/` e só um **digest curto** entra na conversa; `gsd_exec_search` reusa
  runs anteriores em vez de repetir o comando caro.
- **Routing por complexidade**: classificação simples/complexo por heurística
  determinística, sub-milissegundo, sem LLM; aprende de resultados passados.
- **Pressão de budget**: 50/75/90% do teto degrada o trabalho progressivamente.
- Ledger de custo por unidade (token/valor por fase) com dashboard.

### rea (Reverse Engineer Anything) — `morluto/rea`, TypeScript, MIT

- MCP + CLI que dá ao agente ferramentas de análise (Ghidra/Hopper/IDA, JS/Electron,
  .NET/CIL, WASM). **Roda local**; devolve evidência, não adivinhação.
- O que faz funcionar: **evidência hashable** — `{ result, evidence_id, evidence }` com
  *observações* e *inferências* e *desconhecidos* em campos separados; o agente pode
  alucinar o resumo, mas os fatos que ele resume ficam presos e comparáveis.
- **Um motor por sessão, nunca troca silenciosa**: se o Ghidra falha, reporta a
  falha; não cai no Hopper e devolve pseudocódigo de motor diferente no meio da
  cadeia — exatamente a lei do KIRA para backend caro ("nunca fallback silencioso").
- Thread da comunidade sobre token: agentes repetem a mesma exploração; a cura é
  **consulta estreita** (uma função, um xref — nunca o inventário inteiro), **bruto
  vai para arquivo em disco**, e "nada é encontrado duas vezes".

### openclaude — `Gitlawb/openclaude`, Node/Ink, MIT

- Claude Code desacoplado do provedor: um CLI, vários backends (inclui Ollama e
  Foundry — mesma direção do ADR-0019/0020 do KIRA, mas como app de agente).
- UX que interessa: **repo map** (mapa estrutural do repo ranqueado por importância,
  auto-injetado com teto de 2048 tokens), **perfis de provider salvos** (`/provider`),
  **`/cost`** (sidebar com custo/token/sessão ao vivo), streaming com progresso de
  ferramenta, `/doctor`.
- Sessões em background com log (`--bg`, `-f`) — implantação via daemon/gRPC
  (rejeitada no mandato do KIRA; o loop é modo do binário).

---

## O que vale trazer (concreto, por módulo)

| # | Ideia | De onde | Onde cai no KIRA | Prioridade |
|---|---|---|---|---|
| P1 | **Ledger de economia** + estimativa de tokens no header do pack (`chars/CHARS_PER_TOKEN`, determinístico) — o `rtk gain` do KIRA: pack usado vs teto, fonte (`index`/`grep`), chamadas de LLM por sessão | RTK (`rtk gain`) + gitingest P6 | `src/context/mod.rs` (header), `src/store/` (tabela derivada, schema atual sem quebra), `kira history` | **Alta** |
| P2 | **Cache de raciocínio**: antes de chamar o backend, o triage consulta `wiki_fts` por episódios do mesmo sintoma; similaridade de tokens ≥ 0.50 (a MESMA métrica do eval) ⇒ prediz a causa do episódio anterior, fonte marcada `cache-wiki`, sem nenhuma chamada de LLM; `--no-cache` pula; acerto é medido pelo `--eval` existente | rea ("nada duas vezes") + GSD (routing) | `src/triage.rs` + `src/wiki/` | **Alta — o pulo** |
| P3 | **Evidência em 3 estados**: saída do triage e episódio da wiki separam *observado* (evidência `caminho:linha`) / *inferido* (modelo) / *desconhecido* (seção `## Desconhecidos` no episódio) — confiança auditável e melhora o grounding do M2 | rea | `src/llm/prompts.rs` (seções), `src/wiki/episode.rs` | **Alta** |
| P4 | **Digest bruto com endereço**: o JSON cru da ferramenta de scan vai para `.kira/raw/<tool>-<data>.json` (cap + rotação), stdout mantém o resumo; findings.json continua byte-idêntico (determinismo intacto) — debugging de BOM/UTF-16 fica possível | RTK ("nada escondido") + GSD (`gsd_exec`) | `src/scan/node.rs`, `src/scan/dotnet.rs` | Média |
| P5 | **4 estratégias do RTK no packer**: dedup de linhas consecutivas com contagem + agrupamento por arquivo nos trechos do repo (truncamento ancorado e filtro já existem) | RTK | `src/context/repo.rs` | Média |
| P6 | **Repo map no brief**: mapa estrutural determinístico ≤ ~200 tokens no pack (árvore de topo com contagens + arquivos mais citados por findings/wiki — frequência de citação já é a regra do packer; nada de PageRank) | openclaude | `src/context/` (brief) | Média |
| P7 | **Perfil de token** (`token_profile: budget|balanced|quality` em `config.toml` — o parser de chave simples já existe em `src/credentials.rs`): coordenada `KIRA_NUM_CTX` e caps do packer por perfil | GSD-2 | `src/config.rs`, `src/context/mod.rs` | Média |
| P8 | **Pressão de budget 50/75/90%** de `max_llm_calls_per_day`: degradação progressiva (50%: sem nice-to-have; 75%: só triage crítico; 90%: só observe/record) — entra na spec do W5 | GSD-2 | `docs/kira-w5-loop.md` (doc) | Média |
| P9 | **`kira rea <alvo>`**: wrapper read-only do CLI do rea resolvido no PATH (mesmo padrão `binresolve` do scan); `kira setup` pergunta "engenharia reversa? rea/nenhum" (como Datadog no PR12); saída bruta → `.kira/rea/<slug>.md` com evidência ancorada; só digest (teto) entra em prompt; ausente ⇒ erro acionável "instale o rea"; **nunca** MCP, **nunca** no ciclo automático, **nunca** spawn no boot do KIRA | rea | `src/binresolve.rs` + `src/rea/` (novo), `src/setup.rs` | Média |
| P10 | **Painel de budget/economia no dashboard** (1 PR por campo, padrão W6): budget do dia, chamadas usadas, pack médio, economia estimada — alimentado pelo P1 | openclaude (`/cost`) | `src/tui/widgets/` | Baixa (pós-P1) |

## O que NÃO vale

**Do RTK**
- **Código.** Licença mista (hook/filtros Apache-2.0; motor BUSL-1.1) e o KIRA é MIT —
  regra do dono manda: conceito sim, código não. As 4 estratégias são reimplementadas.
- Hook de reescrita de comandos de agentes de terceiros / proxy de tráfego de API —
  KIRA não é harness de agente; quem produz output é o próprio KIRA, então o filtro
  mora no **packer**, não num hook externo.
- Tabela de 100+ filtros de comandos alheios — o KIRA só precisa comprimir o que
  *ele* gera (scan, pack, episódio).
- "Route para modelo mais barato" como poupança primária — dado de cliente nunca vai
  para modelo caro; o default local manda (ADR-0019/0020).

**Do GSD-2**
- Runtime TS/Pi SDK, sessões de agente, crash forensics de harness — o KIRA é CLI de
  comando único, não agente conversacional.
- Estado em markdown `.gsd/STATE.md` — o store SQLite manda.
- Skills discovery, Docker sandbox, web interface, perguntas por Slack/Discord,
  worktree automático — fora do mandato (merge é sempre humano).

**Do rea**
- Servidor MCP — rejeitado no mandato.
- Portar o TS para Rust — Sísifo: o valor está nos motores (Ghidra/IDA), não no
  plumbing. O KIRA **orquestra** o CLI externo, igual faz com `dotnet`/`pnpm`.
- Análise binária como feature core — sustentação é o foco; rea é ferramenta opcional
  sob demanda.
- Cache próprio por bytes do alvo — o CLI do rea já cacheia; KIRA só aponta o caminho.

**Do openclaude**
- Código Node/Ink — Rust/Ratatui manda (mesma regra da branch `dev`).
- Daemon/background com gRPC — rejeitado (loop é modo do binário).
- VS Code extension, OAuth multi-provider — o caro nunca é implícito.
- Pixel-art companion — o mascote do KIRA é **arte manual do dono** (W6), não asset
  de terceiro.

---

## O plano — Onda TE (token economy), PR34–PR43

Regra vigente: **um item = um PR**, gates padrão (`fmt --check` · `clippy -D warnings` ·
`test` · `build` · CI 3 SOs). Nenhuma dependência nova; nada acima de milissegundos
determinísticos; spawn só sob comando explícito.

| PR | Escopo | Arquivos | Aceite específico | Quando |
|---|---|---|---|---|
| **PR34** | Ledger de economia: pack chars vs teto, fonte, nº de chamadas LLM por sessão; estimativa `chars/CHARS_PER_TOKEN` no header do pack e no `--json` | `src/context/mod.rs`, `src/store/` | Duas execuções no mesmo estado ⇒ ledger idêntico; dashboard/history mostram o número sem inventar (ausente ⇒ `—` + causa) | **agora** |
| **PR35** | Cache de raciocínio (o pulo): wiki-hit ≥ 0.50 ⇒ causa predita sem LLM, fonte `cache-wiki` no output e no episódio; `--no-cache` | `src/triage.rs`, `src/wiki/` | `--eval` mede as duas rotas (cache on/off); item novo fora da wiki ⇒ comportamento idêntico ao de hoje (byte a byte no modo padrão) | **agora** |
| **PR36** | Evidência em 3 estados (observado/inferido/desconhecido) no output e no episódio | `src/llm/prompts.rs`, `src/wiki/episode.rs`, fixtures | Fixtures de prompt atualizadas byte a byte; hash do prompt recomputado por teste (padrão PR7) | **agora** |
| **PR37** | Digest bruto com endereço no scan (`.kira/raw/`, cap + rotação) | `src/scan/node.rs`, `src/scan/dotnet.rs` | findings.json continua byte-idêntico em duas execuções; raw capado; `.kira/` segue sem comitar — mecanismo a acertar no PR: hoje o `.gitignore` só cobre `/target`, o `.git/info/exclude` só cobre `.delta/`, e a nota `.kira/README.md` prevista no AGENTS.md não existe neste clone | **agora** (junto do item P1 do relatório do gitingest — o BOM-sniff) |
| **PR38** | Dedup + agrupamento nos trechos do pack | `src/context/repo.rs`, fixtures | Pack com linhas repetidas colapsa com contagem; âncora `caminho:linha` preservada | **agora** |
| **PR39** | Repo map determinístico no brief (≤ ~200 tokens) | `src/context/` | Mesmo estado ⇒ mesmo mapa; arquivos citados em findings primeiro (regra vigente) | **agora** |
| **PR40** | `token_profile` em `config.toml` coordenando teto e caps | `src/credentials.rs` (resolução), `src/context/mod.rs` | `budget`/`balanced`/`quality` ⇒ três packs dourados distintos; inválido ⇒ `balanced` com aviso | **agora** |
| **PR41** | `kira rea` wrapper + pergunta no `setup` | `src/rea/` (novo), `src/binresolve.rs`, `src/setup.rs` | rea ausente ⇒ erro acionável sem spawn; saída nunca entra em prompt sem passar por `redact` + teto; teste com shim (padrão do scan) | **agora** |
| **PR42** | Pressão de budget 50/75/90% na spec W5 | `docs/kira-w5-loop.md` | Doc consistente com mandato; watchdogs numerados | **agora** (doc) |
| **PR43** | Painel de economia no dashboard | `src/tui/widgets/` | 1 PR por campo; read-only provado (padrão `dashboard_e2e`) | **W6** |

**Nota de performance**: tudo é lookup FTS5 ou aritmética em Rust — o custo adicionado
por ciclo é microssegundos; o P9 é o único spawn e só sob comando explícito.

## Ordem de execução

1. **PR34** — medir primeiro (sem número, o resto é opinião).
2. **PR35** — o pulo do gato (o PR34 passa a mostrar a economia que ele produz).
3. **PR36 + PR37** — pequenos, disciplinam o que entra no prompt e no disco.
4. **PR38 + PR39 + PR40** — compressão do pack (RTK + openclaude + GSD).
5. **PR41 + PR42 + PR43** — rea wrapper, spec W5, painel.

## Pendências do dono

1. **Dump do RTK não chegou** — os dumps de gsd-2, openclaude e rea estão em
   `docs/dumps/snapshot-*.md` (12/Nov, com análise no apêndice); falta o do RTK,
   o 4º repo escolhido. Sem ele, a análise do RTK fica no que o site/README
   público mostram (e a licença mista Apache-2.0/BUSL-1.1 manda "só conceito" —
   dump serve para confirmar as 4 estratégias, não para copiar).
2. **Rotular os 20 bugs do `tests/fixtures/triage_gold/`** — pré-requisito (d) do
   mandato; é o que valida o PR35/PR36 **com número** no `--eval`.
3. **Aprovar o plano** — os PRs viram fila no `docs/roadmap-prs.md` (ledger do
   Orchestrator) e o ADR-0026 abaixo vira arquivo.

## Permanece rejeitado (desta rodada)

Daemon/background de qualquer espécie · MCP como dependência · portar TS para Rust
(rea/GSD/openclaude) · copiar código RTK (licença mista) · proxy de tráfego de API ·
routing para modelo caro implícito · estado em markdown · skills discovery · web UI ·
Slack/Discord · VS Code extension.

---

## ADR-0026 (draft) — Cache de raciocínio e economia medida

**Contexto.** Sustentação é recorrente: o mesmo bug volta. Hoje cada triage chama o
LLM mesmo quando a wiki já registrou a causa raiz do mesmo sintoma. Paralelamente, o
KIRA não mede quanto contexto economiza — otimização sem número é opinião (mesmo
princípio do gate M2).

**Decisão.** (1) O `kira triage` consulta `wiki_fts` ANTES do backend; episódio com
similaridade de tokens ≥ 0.50 (métrica do ADR-0018) prediz a causa sem LLM, fonte
marcada `cache-wiki`; `--no-cache` desliga. (2) O pack e as sessões medem economia
(chars vs teto, fonte, chamadas de LLM) e o número aparece em `history`/dashboard.
(3) Evidência em 3 estados (observado/inferido/desconhecido) vira contrato do triage
e do episódio.

**Consequências.** LLM-last deixa de ser só ordem de escada e vira **poda de chamada**:
o item recorrente custa zero token. O `--eval` passa a medir as duas rotas; a precisão
do cache é auditável porque a fonte fica no output. Risco controlado: a predição só é
exibida quando a similaridade passa do mesmo limiar do gold — falso positivo do cache
é um episódio parecido demais, que o dono corrige na wiki (a fonte de verdade).

**Alternativas rejeitadas.** Routing para modelo "mais barato" (dado de cliente, caro
implícito); embeddings de similaridade (rejeitado até pós-M2; FTS5 de tokens
normalizados resolve sem dependência); sumário de arquivo por modelo pequeno
(GSD-style) — o packer determinístico do KIRA já faz isso sem rede.

---

## Apêndice — o que os dumps confirmam (12/Nov)

Análise feita sobre `docs/dumps/snapshot-{gsd-2,openclaude,rea}.md` (dumps do
dono); nada copiado para o código. Citações curtas com endereço de linha no dump.

- **GSD-2** (`snapshot-gsd-2.md`): o budget pressure é aplicado **antes** do
  scoring de capacidade — a linha 8926 do dump fecha o desenho: "cost constrains
  eligibility; capability determines ranking"; a mistura dos dois sinais foi
  rejeitada por eles ("confusing behavior"). O downgrade fica legível ao usuário
  (linha 8886: "budget pressure: 85%" na reason string). Confirma o P8 e ensina
  o detalhe que faltava ao KIRA: **orçamento é restrição de elegibilidade, não
  peso no ranking** — filtra o que é elegível primeiro, só depois ranqueia.
- **openclaude** (`snapshot-openclaude.md`): é o fork do dono do Gitlawb/openclaude
  — além do CLI original (slash commands, `/provider`, streaming; linhas 2245 e
  2274 do dump), o fork adiciona os providers Python do próprio dono
  (`atomic_chat_provider.py`, `ollama_provider.py`, `smart_router.py`, com testes
  dedicados) e `cost-tracker.ts`/`costHook.ts` — **custo como hook do ciclo**, não
  relatório retrospectivo. Reforça o P1/P10: o ledger tem que existir no ponto
  onde o gasto acontece. Nota honesta: o repo map/PageRank do README público
  **não aparece** no dump do fork (versão anterior do upstream); o P6 fica com a
  referência dos docs públicos.
- **rea** (`snapshot-rea.md`): o AGENTS.md deles manda, na linha 8668:
  "distinguish observations, derivations, inferences, and unknowns; preserve
  Evidence provenance" — confirma o P3 literalmente (observado/inferido/
  desconhecido separados). Bônus transferível: o perfil de schema compacto das
  ferramentas MCP (`REA_MCP_INPUT_SCHEMA_PROFILE=compact`, default; linha 32355
  do dump) existe para **economizar tokens de definição de ferramenta** — para o
  KIRA, a mesma lição vale para as seções versionadas do prompt (o gate de hash
  byte a byte do PR7 já impede inchaço silencioso).
