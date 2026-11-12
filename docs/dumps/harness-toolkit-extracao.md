# Extração — tech-leads-club/harness-toolkit para o KIRA

- **Data**: 12/Nov/2026
- **Fonte**: https://github.com/tech-leads-club/harness-toolkit @ commit `d293c7f`
- **Licença**: **Elastic License 2.0 (ELv2)** — não-MIT. Regra vigente do repo
  ("conceito sim, código não", mesma aplicada ao RTK/BUSL): **nada do código ou
  dos textos deles é copiado para cá**. Este arquivo é uma extração de análise
  escrita do zero, com citações curtas atribuídas. O dump cru (624 arquivos,
  4,2 MB) foi gerado **apenas localmente** (`%TEMP%/opencode/dumps/
  gitingest-harness-toolkit.md`) para a engenharia reversa; não entrou no PR.
  Quem quiser o dump: rode `git clone --depth 1` e gere com o gitingest de
  sempre — o repo é público.
- **Companheiro**: `docs/agent-skills-harness-vale-trazer.md` (análise e plano) e
  [ADR-0027](../adr/ADR-0027-techleads-piso-guards-licoes-custodia.md).

## O que é

Hooks nativos do Cursor e do Claude Code. O editor dispara o evento; o harness
responde com um **veredito** — `allow`, `ask`, `deny` ou texto injetado no turno
(`context`) — e grava o que decidiu e por quê. Estrutura em três tiers, e o tier
define se você pode desligar:

| Tier | O que é | Configurável | Roda |
|---|---|---|---|
| **Floor** (8 regras) | o piso — avaliado ANTES de ler qualquer política | nunca | toda tool call, shell e leitura |
| **Always on** (3 checks) | sempre ligados | nunca | depois do piso, em todo evento de ação |
| **Rails** (25 capacidades) | o que você escolhe; cada uma com trade-off declarado no catálogo | individualmente | conforme o catálogo |

O catálogo (`capabilities/catalog.json`, versão 15) é a única fonte de metadados
de capacidade (AD-017). Cada rail carrega: `id`, `configPath`, `title`,
`summary`, `benefit`, **`tradeOff`**, `defaultOn`, `fires` (evento do editor),
`verdict` (o que devolve) e `inspect` (como conferir). **Toda capacidade declara
o próprio custo** — a rail `grind` (lint/teste em loop até passar) diz no
catálogo que um suite de 4 minutos com `maxLoops` 5 pode gastar 20 minutos só de
teste.

## O piso (as 8 regras que nenhuma configuração alcança)

Nomeadas no README, avaliadas antes do arquivo de política, toda negação
imprime `rule=<nome>`:

1. **outside-project-destruction** — comando destrutivo com alvo que resolve
   FORA do repositório e fora do temp do OS.
2. **unprovable-destruction** — verbo destrutivo cujo alvo é variável,
   substituição de comando ou algo construído em runtime (a porta não sabe o que
   apagaria). Só passa caminho literal resolvível.
3. **secret-access** — leitura que copiaria credencial para o transcript
   (`.env`, `~/.ssh`, `~/.aws`, `*.pem`, instance metadata service). Nota fina:
   `grep` procurando um endereço literal **não** bloqueia — grep não fala com a
   rede.
4. **history-rewrite** — `git push --force` bloqueado; `--force-with-lease`
   liberado (ele mesmo recusa se o remote andou).
5. **machine-control** — `shutdown`, `reboot`, `halt`, `poweroff`.
6. **unprovable-execution** — programa buscado da rede e dado a um shell
   (pipe, substituição de processo, `-c`/`eval`).
7. **policy-surface-write** — qualquer rota de escrita do agente sobre a
   política e o estado do próprio harness (redirect de shell, heredoc, write
   tool) + subcomandos mutantes de `tlc harness` chamados de dentro da sessão.
8. **wiring-tamper** — escrita no arquivo de wiring do provider (que registra os
   hooks): sobrescrever é silenciar o harness inteiro para aquele editor.

A linha operador/agente é **estrutural, não convencional** (AD-022): os dois
tiers de config (runtime home e projeto) são recusados ao agente no write-tool e
no shell redirect — mudança de política é coisa de operador, num terminal fora
da sessão do agente.

## Os vereditos e as 25 rails

Vereditos existentes: `deny`, `ask`, `block-stop`, `follow-up`,
`context` (texto injetado no turno), `record` (grava, não decide). A lista
completa com `defaultOn` (v15):

| rail | veredito | dispara em | default |
|---|---|---|---|
| grind | follow-up | stop | off |
| shipGate | block-stop | stop | off |
| emptyDiffAntiShip | block-stop | stop | off |
| comments (gate de comentários adicionados) | block-stop | stop | off |
| duplication (código duplicado neste turno) | block-stop | stop | off |
| supplyChain | block-stop | stop | off |
| subagents | deny | tool.before, subagent.start | off |
| blockParentFast | deny | tool.before, subagent.start | off |
| shellStall | deny | shell.before | off |
| catastrophicShell | ask | shell.before | **on** |
| lessons | context | session.start, stop, session.end | off |
| budgetContinue | follow-up | stop | off |
| gapFeedback | follow-up | stop | **on** |
| failureClassification | record | stop | **on** |
| progressiveHandoff | context | session.start | **on** |
| progressiveContext | follow-up | stop | **on** |
| autopilot | follow-up | stop | **on** |
| idleTurnGate | block-stop | stop | off |
| docsGate | block-stop | stop | off |
| globalSpool | record | tool.after, tool.failure | off |
| untrustedContent | context | tool.after | off |
| planGate | block-stop | response.after, stop | off |
| observe | record | stop, session.end | off |
| operatorRules | deny | tool.before, stop | off |
| secretRedaction | context | tool.after | **on** |

Dois detalhes de design que valem ouro: o event kind é provider-agnóstico e as
capacidades são **dados, não flags** (AD-009); e o arquivo de gate por sessão
(`harness.gate.v1`) guarda exit code, comando, arquivos, `outputTail` e
`findings` — um arquivo por sessão, então sessões concorrentes não se
atravessam.

## Postura (paired/solo/focus) e fadiga de aprovação

- A postura muda **só o surfacing** — a verificação é idêntica nas três
  (AD-025). Postura nunca liga ou desliga uma capacidade ("uma preferência de
  surfacing não sobrescreve silenciosamente uma capacidade que tem o próprio
  switch e o próprio trade-off").
- Taxonomia dos asks, cada classe nomeia um risco diferente: comando que **sai
  da máquina**; que **pode sobrescrever ou remover** caminho existente; que
  **muda quem alcança** o caminho (`chmod`/`chown` pergunta — é a única mudança
  que não aparece em nenhum diff).
- **Append não pergunta** — sob `>>` o conteúdo sobrevive; perguntar sobre ele
  "treinaria você a limpar o prompt sem ler", e aí a ação que importava passa
  batida. (Engenharia de fadiga de alerta explícita.)
- **Deadline de pergunta** (AD-026): "uma pergunta atrasada é mensuravelmente
  pior que uma decisão" — objetivo ambíguo se resolve no primeiro ato; em
  andamento, assume a leitura mais razoável e declara a suposição em uma linha.
  `focus` admite exatamente UMA pergunta inicial (objetivo ilegível).
- Valor inválido de postura é **recusado, não absorvido** — `status`/`doctor`
  nomeiam a palavra rejeitada e a postura que ficou no lugar.

## Lessons — conhecimento com prazo de validade e nota de efetividade

O sistema de lições do harness é o comparativo mais próximo da wiki do KIRA:

- **Três tiers** (`core` dentro do runtime, `global` por máquina, `project` por
  repo) com `projectBoost` — **o conhecimento local vale mais** que o carregado
  de fora; id duplicado, o tier mais próximo vence.
- **Nada cruza entre produtos por si** (AD-040): promoção automática de lição
  não existe "por design" — lição minerada de uma falha de gate cita o runner e
  o layout do repo de origem, e nenhum threshold sabe se vale no outro. Só o
  operador promove.
- **Staleness por refs** (AD-036): a lição pode declarar `refs` como
  `caminho` ou `caminho:símbolo`. Ref que não resolve ⇒ a lição **deixa de ser
  injetada** — "uma lição que nomeia um arquivo renomeado é pior que ausente,
  porque manda o próximo turno procurar o que não existe mais, com autoridade
  de lição". `unreadable` NÃO é stale (não é evidência de que parou de valer) e
  lição sem refs nunca é stale (a maioria é de conduta).
- **Janela de validade** (AD-037): `validFrom`/`validTo` ISO; bound
  não-parseável **retrai a lição** (tratar declaração quebrada como sem
  declaração injetaria exatamente o que o autor queria limitar).
- **Efetividade medida** (AD-039): quando a lição é injetada num retry do gate
  G, id+G vão para o handoff; **o próximo run do gate G gradua** — passou ⇒
  `helped`, falhou ⇒ `neutral`. Leituras: `helped n/m`, `neutral 0/m`,
  `unproven` (única que vira warning no `doctor`), `session-only`,
  `not-injected`. Taxa é `null` sobre zero injeções gradas — zero leria como
  "medido e nunca ajudou".
- Render em `[gate/status/tier]`: quem lê `[test/active/global]` sabe que o
  conselho foi escrito sobre outro repositório.

## Observabilidade e gates

- Três planos com provider por registro: **signal** (`obs.jsonl`, on), **debug**
  (off), **audit** (um registro por invocação de hook — negação nunca fica
  fora de auditoria; `shell.end` sobe de debug para signal quando a permissão
  não foi plain allow).
- `tlc harness obs report`: rollup markdown de tokens, USD estimado, tools,
  subagentes e gates; retention 14 dias; `globalSpool` espelha tudo por repo
  num arquivo único da máquina — best-effort (runtime home não escrevível
  degrada para projeto, nunca falha o hook).
- Todo comando de leitura aceita `--json`: sem a flag, saída byte-idêntica à
  de sempre; com a flag, exatamente um JSON e nenhum prose — exit code não muda.
- Lock de gate no state dir do projeto: sessão vizinha não trava o turno;
  veredito gravado cujo hash de inputs casa é **reusado**; expirou ⇒ o gate
  **defere** (turno termina, handoff registra `skipped`, e o dono do lock é
  nomeado — AD-073). Lock antigo (>30 min), ilegível ou com processo morto é
  retomado (pid só significa algo na máquina que o escreveu — AD-024).

## Arquitetura e disciplina

- Ports and adapters: `contracts/` (vocabulário compartilhado) ← `core/` (lógica
  agnóstica, flat-by-aggregate) e `providers/` (um ACL por editor);
  `entrypoints/` compõe. **`core` nunca importa `providers`, `providers` nunca
  importa `core` — e as fronteiras são checadas mecanicamente**
  (`tools/dev/check-boundaries.ts`), inclusive um scan que reprova nome de
  vendor (`cursor`, `claude`, `anthropic`, …) em qualquer lugar sob
  `src/core/` ou `src/contracts/` (AD-004, AD-010).
- `biome check --error-on-warnings` — warn não muda exit code, então três
  warnings consertáveis sobreviveram a vários runs verdes; e
  `tools/check-suppressions.ts` reprova `biome-ignore`/`@ts-ignore` cuja
  justificativa não abre com `why:`/`hazard:`/`invariant:` seguido de frase real
  (AD-051) — código mais rígido cria pressão para silenciar em vez de consertar.
- **139 documentos de decisão** (`docs/decisions/ad-001…ad-146`, com lacunas),
  cada um com título-frase e status no índice. O índice substituiu a seção
  `## Decisions` do STATE.md; o changelog do projeto É a sequência de decisões
  (AD-031).
- Zero dependência runtime (badge do README), SLSA v1 provenance no npm.
- O skill `harness-init` deles tem `evals/trigger_evals.json` — eles testam
  **quando o skill dispara**, não só o que ele faz.
