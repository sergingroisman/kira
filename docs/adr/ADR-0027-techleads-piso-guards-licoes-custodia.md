# ADR-0027 — Emprestado do Tech Leads Club: piso de guardas com veredito, lições que provam que valem e custódia de prompt

- **Status**: proposto (12/Nov/2026) — Hermes (recomendação) · Sérgio (veto/aval).
  Um veto vira ADR que supersede este, não edição silenciosa.
- **Fonte**: análise de `tech-leads-club/agent-skills` @ `e74dc8a` (MIT no código,
  **CC-BY-4.0** no conteúdo dos SKILL.md — copiar com atribuição) e
  `tech-leads-club/harness-toolkit` @ `d293c7f` (**Elastic License 2.0** — não-MIT:
  **conceito sim, código/texto deles não**; extração em paráfrase em
  `docs/dumps/harness-toolkit-extracao.md`, dump cru fica local). Relatório
  completo: `docs/agent-skills-harness-vale-trazer.md`; placar:
  `docs/benchmark-kira-vs-techleads.md`.
- **Princípio que os dois validam**: o mesmo do ADR-0016 dito de outro jeito —
  *constrangimento determinístico antes do modelo*: o harness deles decide
  `allow/ask/deny` ANTES de qualquer política e os gates deles rodam "enforced by
  code, not memory". É a escada do CLI-first aplicada a guarda e a lição.

## Contexto

O KIRA já tem as peças de segurança (`guard::path` do PR3, jail do PR27, budget
com número, `awaiting_approval` do store) e de conhecimento (wiki FTS5,
episódios). A engenharia reversa dos dois repos (dump gitingest de ambos;
94 SKILL.md comitáveis com atribuição; 624 arquivos do harness extraídos em
paráfrase por causa da ELv2) achou três lacunas estruturais que nenhum item
planejado cobre: (1) o KIRA não declara um **piso** — o que NENHUMA configuração
pode alcançar; (2) a wiki não prova que uma página **ainda vale** nem mede se
**ajudou**; (3) prompts versados no repo não passam por **scan nem hash** no CI.
As três têm modelo pronto para emprestar, com licença limpa para o primeiro caso
e paráfrase para o segundo.

## Decisão

**Oito melhorias emprestadas, uma por PR (fila PR44–PR51 do relatório), duas
pendentes de aval do dono.** Nenhuma dependência nova; nada acima de
milissegundos determinísticos; spawn só sob comando explícito.

### P1 (PR44): Piso de guardas em tiers + veredito tipado — `guard`

Taxonomia do harness-toolkit, reimplementada: **floor** (regras que nenhuma
config alcança: destruição fora do escopo do repo, leitura de segredo para o
transcript, `push --force`, destruição de alvo improvável, máquina, rede→shell,
escrita na superfície de política, silenciamento do wiring) → **always-on** →
**rails** configuráveis. Veredito tipado: `deny | ask | block-stop | follow-up |
context | record`; toda recusa imprime `rule=<nome>` acionável. Entra em
`src/guard/` e na spec do W5.

### P2 (PR45): Engenharia de fadiga de aprovação — `guard`/W5

Perguntar tem custo. Classes de risco dos asks: **sai da máquina** / **pode
sobrescrever caminho existente** / **muda quem alcança** (`chmod` pergunta — é a
única mudança que não aparece em diff). **Append nunca pergunta** — perguntar
sobre não-destrutivo treina o dono a limpar o prompt sem ler, e aí a pergunta
que importava passa batida. **Deadline de pergunta**: ambiguidade de objetivo se
resolve no primeiro ato; em andamento, assume a leitura razoável e declara a
suposição em uma linha. Valor inválido de modo é **recusado, não absorvido**.

### P3 (PR46): Lições com refs, janela e efetividade — `wiki`/`context` (o pulo)

O episódio/página da wiki ganha `refs` (`caminho` ou `caminho:símbolo`) e
janela `valid_from`/`valid_to`. **Ref que não resolve ⇒ a página sai do pack com
nota** — a lição que cita arquivo renomeado é pior que ausente ("manda o próximo
turno procurar o que não existe mais, com autoridade de lição"). Arquivo
ilegível NÃO é stale (não é evidência de que parou de valer); página sem refs
nunca é stale. E a **efetividade é medida**: quando o cache de raciocínio
(PR35 da onda TE) usa uma lição no triage do sintoma S, o próximo triage do MESMO
sintoma gradua — passou ⇒ `helped`, falhou ⇒ `neutral`, nunca usada ⇒
`unproven` (única leitura que vira aviso no `doctor`). A taxa é `null` sobre
zero gradas, nunca `0` — zero leria como "medido e nunca ajudou". Promoção de
lição entre projetos **não é automática, por design** — igual ao staging do
KIRA (PR32); nada cruza por si.

### P4 (PR48): Cadeia de custódia de prompt — CI/store

Do agent-skills (MIT/CC-BY): (1) **scan anti-injection no CI** para
`src/llm/prompts.rs` + fixtures — padrão de exfiltração (fetch de rede, leitura
de env/segredo) num prompt reprova; (2) **hash por versão de prompt gravado na
sessão do store** — `(prompt_version, hash)` na trilha; (3) se um prompt um dia
inspirar texto literal de SKILL.md, o comentário do const cita autor + licença
CC-BY-4.0.

### P5 (PR47): Symlink/junction no jail — `guard`

Do installer do agent-skills: checar com `lstat` (não seguir antes de validar),
validar o **alvo final** de symlink/junction contra o root canônico, detectar
loop (ELOOP), cap de nome (255). Junction é o caso real do dono (workspace em
OneDrive). Follow-up direto do PR27.

### P6 (PR49): Ship claim + empty-diff anti-ship — spec W5

"Do done sem prova não existe": claim de item concluído exige **diff não-vazio**
(e a evidência do teste); stop sem prova vira `follow-up`, nunca silêncio. Doc
primeiro, watchdogs numerados, código depois.

### P7 (PR50 — a confirmar): Track qualitativa opt-in no `--eval`

Do `harness-eval` deles: Track A determinística (o `--eval` do M2) sempre roda;
track B/C com juízes LLM + traps plantadas é **opt-in com custo anunciado antes
de rodar** e discordância ⇒ hold. **Report-only**: o gate M2 continua sem LLM
como juiz (ADR-0018 intocado) — sem a flag, o `--eval` é byte-idêntico ao de
hoje.

### P8 (PR51 — a confirmar): `kira debate` (the-fool/the-jury, bônus)

Painel 3–5 com round cego, dissidente obrigatório, cap de 5 ("mais jurados =
ruído correlacionado"), dissent preservado e sempre um veredito — para decisões
do DONO. Nunca no ciclo, nunca em gate; Ollama default; backend caro só sob
`--llm` explícito (caro nunca é implícito); chamadas contam no budget.

## Consequências

- **Positivas**: o KIRA ganha o piso declarado e a taxonomia de veredito que a
  onda W5 precisa para crescer sem ambiguidade; a wiki passa a se autodepurar
  (stale sai do pack sozinho) e a medir o próprio valor (`helped n/m`), fechando
  o ciclo de medição do cache de raciocínio (PR35 da onda TE); prompts ganham CI que reprova exfiltração e
  trilha com hash; jail cobre junction (o caso OneDrive do dono).
- **Negativas**: 4–8 PRs a mais na fila (um por item); PR46 toca `src/wiki/` e
  `src/context/` juntos — entrar depois que a fila do ADR-0023 assentar os
  mesmos arquivos, para não rebasear; a rubrica do benchmark é conceitual —
  endossá-la como "placar" sem rodar workload comum seria vaidade, o que este
  ADR não faz.
- **Não muda**: read-only por padrão; `--apply` + `awaiting_approval` + `kira
  approve`; merge humano; determinismo M1; sem LLM como juiz do gate (ADR-0018);
  budget 50/dia; o caro nunca implícito; segredo nunca em prompt/DB/log.

## Rejeitado

- **Código/texto do harness-toolkit no repo (ELv2)** — licença restritiva; a
  extração é paráfrase com citação curta e o dump cru fica local. Se o dono
  insistir em comitar, pasta própria com a ELv2 colada e header de atribuição —
  e a recomendação continua sendo não.
- **Marketplace/registry multi-agente, CLI instalador, MCP server** — KIRA não é
  registry; MCP rejeitado no mandato.
- **Hooks de editor (Cursor/Claude Code) como runtime** — o KIRA é CLI/loop
  próprio; o que se empresta é o design de veredito, não o transporte.
- **Portar TS para Rust** — Sísifo (mesma decisão da rodada anterior).
- **Subagente/painel de jurados dentro do ciclo ou do gate** — custo de token
  sob budget e "sem LLM como juiz" (ADR-0018) manda; painel só sob comando
  explícito do dono (P8).
- **Pricing em USD / cost alert** — o KIRA mede chamadas, não dinheiro.
- **Catálogo de 94 skills de domínio alheio (GTM/cloud/design)** — fora do
  domínio de sustentação ADO; o valor é o formato (frontmatter, triggers,
  semver, autor) e as skills de processo.
