# Engenharia — harness toolkit e a escada de autonomia

Os dois repositórios estudados antes do formato de skill ([[engenharia-agent-skills]])
têm governança madura; o KIRA tem **ação**. Este documento separa o que é
degrau (empresta) do que é outro prédio (não copia) e aplica a escada
`prompt → skill → loop → graph → harness → system design` ao que o binário
faz hoje.

## A escada, degrau por degrau

| Degrau | Pergunta que responde | No KIRA |
|---|---|---|
| prompt | "responda isto" | `kira triage` com context pack retrieval-first (ADR-0015) |
| skill | "responda sempre assim" | `kira skills` (SK-1) |
| loop | "repita sem que eu peça" | `kira loop` (PR15) |
| graph | "vá além do loop com estados" | `brain` (orquestrador interno, desenhado — `kira-brain.md`) |
| harness | "não faça o que não pode" | piso de guardas proposto (PR44–51, ADR-0027) |
| system design | "tudo isso junto é um produto" | o binário e esta wiki |

## O turno do binário: `kira loop`

`kira loop` (PR15, PR #106) é o terceiro degrau em execução: **um tick =
`ingest → digest (só se trusted) → publish`**, tudo local. O que o design
fixa:

- **Modo do binário, sem daemon.** O loop roda ticks em primeiro plano e sai.
  Não há serviço residente: o "agendamento" da casa é o KIRA vivo (sem serviço
  do Windows no MVP). `--once` roda UM tick (é a forma de CI e teste);
  `--ticks` roda N; `--skip-digest` faz um ciclo 100% determinístico.
- **Estado no SQLite, não em memória.** A fila `posts` é tabela (onda M2):
  claim por lease, `draft_ttl`, estado `failed` retomável. Um tick que morre
  retoma; nada de fila em processo.
- **Anti-PR-inócuo (D2).** Dois ticks sem mudança ⇒ estado idêntico: o segundo
  tick não reescreve `facts.md` (ingest determinístico), não gera post novo
  (cache por `cache_key`) e não republica (`published_hash`). A detecção é
  `TickReport::changed()` — função pura (facts mudaram / post novo /
  publicação nova; **cache e failed não contam**). O e2e
  (`tests/loop_e2e.rs`) prova: o 2º tick deixa facts, drafts, wiki e a fila
  byte-idênticos. A trilha de sessões cresce de propósito — é auditoria, não
  estado do ciclo.
- **Sem trust, o digest é pulado com nota acionável** e o ciclo segue com
  exit 0 — o ingest roda sempre (read-only), o que manda código ao modelo é
  que recusa.
- **Wiring puro.** O módulo constrói o backend e entrega ao `post::run`, que é
  quem fala com o modelo (com o `// LLM porque:` no call site). O loop em si
  não faz nenhuma pergunta de julgamento — continua fora da allowlist real de
  julgamento; a porção de driver roda até o ponto fixo ("estável: nada a
  fazer").

## Próximo degrau: graph e o Brain

O orquestrador **interno** é o `brain` (runtime, `src/brain/`): grafo de
estados onde cada nó pode ter um **turno** (o loop que repete) e cada turno
fases (`claim`, `generate`, `deliver`). Hierarquia canônica:
`graph → turn → phase` ([[glossario]]). O fluxo desenhado (mapa §5) estende o
tick: nó ingest → nó digest → nó publish → nó triage → nó `fix --apply`,
com o verificador provando `author ≠ verifier` e a rota de falha voltando ao
fix — e `awaiting_approval` travando a escrita no mundo até o humano passar
`kira approve`.

## A guarda: `awaiting_approval` e `kira approve`

**`awaiting_approval` é estado do store, não convenção.** Antes de qualquer
escrita no mundo (Azure DevOps, repo do projeto), o ciclo grava o estado e
para. Só `kira approve <id>` — humano — transiciona para `applying`.
Orçamento estourado **não** é aprovação; `--apply` isolado **não** é
aprovação. Este é o PR18, pré-requisito de qualquer escrita autônoma, e
mantém-se depois que o graph existir: leitura é livre, escrita nunca.

## O estudo que originou este degrau

O harness-toolkit estuda governança de agentes via hooks do Cursor/Claude
Code: runtime global Bun/Node com zero deps, 18 eventos, 2 providers,
decisões tipadas. O que o relatório do mapa considera engenharia madura
deles — e o que cada peça rende no KIRA:

| Peça deles | O que é | Onde entra no KIRA |
|---|---|---|
| Floor de 8 regras avaliado antes de qualquer política | nenhuma config alcança | PR44 (piso de guardas) |
| Veredito tipado (`deny \| ask \| block-stop \| follow-up \| context \| record`) | decisão com nome | PR44 (spec W5) |
| Observation mode | o novo guarda observa antes de bloquear | aceite do PR44 |
| Lessons com `path:symbol` e efetividade `helped n/m` | lição que se aposenta sozinha | PR46 |
| `degrade` por capability do host | degradação honesta | filosofia fail-open da casa |
| Catálogo de 25 rails com trade-off declarado e **docs gerados do catálogo** (drift reprova) | docs atrelados a código | checker CI |
| Observabilidade em 3 planos JSONL com recusas atribuídas por regra + attestation hash-chained | `why` e `rule=<nome>` por decisão | PR44 + trilha de sessão |
| Gate próprio de 20 passos; 139 ADs como changelog | disciplina de decisão | ADRs da casa |

**Licença importa**: o código deles é Elastic License 2.0 — **conceito sim,
código/texto não** (extração em paráfrase; o dump cru fica local). O
agent-skills é MIT/CC-BY-4.0 (código MIT, conteúdo dos SK.md CC-BY-4.0 —
copiar com atribuição). Copiar código de projeto não-MIT no repo MIT é
anti-padrão recusado ([[padroes]]).

## As peças do ADR-0027, uma por PR

- **P2 (PR45) — engenharia de fadiga de aprovação.** Perguntar tem custo.
  Classes de risco dos pedidos: sai da máquina / pode sobrescrever caminho
  existente / muda quem alcança (`chmod` pergunta — é a única mudança que não
  aparece em diff). **Append nunca pergunta**: perguntar sobre não-destrutivo
  treina o dono a limpar o prompt sem ler — e aí a pergunta que importava
  passa batida. Valor inválido de modo é **recusado, não absorvido**.
- **P6 (PR49) — ship claim + empty-diff anti-ship.** "De done sem prova não
  existe": claim de item concluído exige diff não-vazio (e a evidência do
  teste); stop sem prova vira `follow-up`, nunca silêncio.
- **P7 (PR50) — track qualitativa opt-in no `--eval`.** Track A
  (determinística, o gate M2) sempre roda; track B/C com juízes LLM + traps
  é **opt-in com custo anunciado antes de rodar** e discordância ⇒ hold.
  **Report-only**: o gate continua sem LLM como juiz — sem a flag, o
  `--eval` é byte-idêntico ao de hoje.
- **P8 (PR51) — `kira debate`** (the-fool/the-jury, bônus, a confirmar).
  Painel 3–5 com round cego, dissidente obrigatório, cap de 5 ("mais jurados
  = ruído correlacionado"), dissent preservado e sempre um veredito — para
  decisões do **dono**. Nunca no ciclo, nunca em gate; Ollama default;
  backend caro só sob `--llm` explícito; chamadas contam no budget.

## A fila PR44–PR51 em uma linha

| PR | Entrega | Fonte |
|---|---|---|
| 44 | piso de guardas em tiers + veredito tipado | harness-toolkit |
| 45 | fadiga de aprovação (append nunca pergunta) | harness-toolkit |
| 46 | lições com refs, janela e efetividade | harness-toolkit |
| 47 | symlink/junction no jail | agent-skills |
| 48 | cadeia de custódia de prompt (scan CI + hash na sessão) | agent-skills |
| 49 | ship claim + empty-diff anti-ship | harness-toolkit |
| 50 | track qualitativa opt-in no `--eval` | harness-eval |
| 51 | `kira debate` (bônus, a confirmar) | the-fool/the-jury |

## O que o harness-toolkit empresta (e o que não)

O harness-toolkit estuda governança de agentes via hooks do Cursor/Claude
Code: runtime global, 18 eventos, veredito tipado, observabilidade em JSONL
atribuída por regra, lessons com efetividade medida, catálogo de "rails" com
trade-off declarado. **Diferencial declarado no mapa**: eles *reagem* ao
editor com enforcement; o KIRA *age* por turno próprio com memória longa.

O KIRA leva deles (fila PR44–51 do ADR-0027):

- **Piso + veredito tipado (PR44)** — o floor que nenhuma configuração alcança
  (nenhuma config alcança leitura de segredo para transcript, `push --force`,
  destruição fora do escopo) acima de rails configuráveis; recusa imprime
  `rule=<nome>`.
- **Observation mode (aceite do PR44)** — o novo modo entra observando antes
  de bloquear.
- **Lessons com refs e efetividade (PR46)** — página da wiki ganha `refs`
  (`caminho:símbolo`) e janela de validade; ref que não resolve ⇒ a página sai
  do pack com nota (lição que cita arquivo renomeado é pior que ausente);
  efetividade medida: `helped n/m`, `neutral`, `unproven` — taxa `null` sobre
  zero gradas, nunca `0` ("medido e nunca ajudou" leria errado).
- **`why` por decisão gravado na sessão** — toda decisão da guarda deixa o
  motivo na trilha.
- **Docs atrelados a código** — checker que reprova drift entre catálogo e
  docs.

E fica de fora, por decisão: hooks de editor como runtime (o KIRA é CLI/loop
próprio), portar o TS deles para Rust (Sísifo), marketplace/registry multi
agente, painel de jurados dentro do ciclo ou do gate (o gate M2 não tem LLM
como juiz, ADR-0018; painel só sob comando explícito do dono, PR50/51).

## Medição antes de autonomia

A promoção do loop a "autônomo" é travada por um **número**, não por
entusiasmo: `kira triage --eval` com os 20 bugs rotulados pelo dono
(`triage_gold`), limiar ≥ 70%, comparação determinística sem LLM como juiz
(ADR-0018). Sem o número, o degrau "graph" não é escalado. É a mesma ordem da
onda TE (ADR-0028): **o medidor precede o pulo — sem número, a economia é fé**.
