# Relatório — O que vale trazer do Tech Leads Club para o KIRA (agent-skills · harness-toolkit)

- **Data**: 12/Nov/2026
- **Fontes**: `tech-leads-club/agent-skills` @ `e74dc8a` (7.157★, MIT no código +
  **CC-BY-4.0** nos SKILL.md) e `tech-leads-club/harness-toolkit` @ `d293c7f`
  (310★, **Elastic License 2.0**) — análise feita sobre clone raso dos dois
  (dump gitingest gerado dos dois; veja `docs/dumps/`).
- **Licenças, lidas no arquivo**: agent-skills = MIT para código, CC-BY-4.0 para
  conteúdo de skill (**copia com atribuição**); harness-toolkit = ELv2
  (**conceito sim, código não** — extração em
  [`docs/dumps/harness-toolkit-extracao.md`](dumps/harness-toolkit-extracao.md);
  o dump cru deles fica local, fora do repo MIT).
- **ADR**: [ADR-0027](adr/ADR-0027-techleads-piso-guards-licoes-custodia.md)
- **Benchmark**: [docs/benchmark-kira-vs-techleads.md](benchmark-kira-vs-techleads.md)
- **Veredito**: 1 tese comprovada + 8 melhorias (fila PR44–PR51) + 2 bônus a
  confirmar; nenhum contrato do mandato quebrado; nenhuma dependência nova;
  nada depende de daemon, MCP, embeddings ou modelo caro.

---

## O pulo do gato (TL;DR)

Os dois repos são a tese do KIRA dita em outro nível da escada, com um detalhe
que falta aqui:

1. **agent-skills é a biblioteca de prompts com cadeia de custódia.** 94 skills
   versionadas (semver + autor no frontmatter), scan anti-injection no CI,
   lockfile + hash imutável e curadoria humana — a resposta deles ao Snyk 2026
   Agent Threat Report (13,4% dos skills de marketplace com falha crítica). O
   KIRA tem prompts versados no repo (`src/llm/prompts.rs`) mas **não tem o
   ritual**: nenhum scan de prompt, nenhum hash por versão no store.
2. **harness-toolkit é enforcement em tiers com veredito tipado.** Floor (8
   regras que nenhuma config alcança) → always-on (3) → rails (25, cada uma com
   trade-off declarado no catálogo). O KIRA tem `guard::path` + jail + budget,
   mas **sem o piso declarado** e **sem taxonomia de veredito**
   (`deny/ask/block-stop/follow-up/context/record`).
3. **E o detalhe que falta nos dois lados do oceano:** a lição do
   harness-toolkit que a wiki do KIRA ainda não tem — **uma página de
   conhecimento tem que provar que ainda é verdade** (refs que precisam resolver:
   "a lição que nomeia arquivo renomeado é pior que ausente") **e medir se
   ajudou** (`helped n/m` gradado pelo próximo run do gate). Isso conecta
   direto no rascunho de cache de raciocínio + economia medida (PR35 da onda TE).

O KIRA ganha deles em uma coisa que nenhum dos dois tem: **determinismo
contratual** (mesma entrada = mesma saída byte a byte) e **gate de acerto
medido contra gold do dono** (ADR-0018) — o benchmark doc mostra a tabela.
O que eles têm e o KIRA não: breadth de conhecimento empacotado (agent-skills)
e enforcement estruturado por evento com observabilidade (harness-toolkit).

---

## A jornada — o que cada um faz

### agent-skills (`packages/`: cli · marketplace · mcp · skills-catalog; `libs/core`)

- **94 SKILL.md em 14 categorias**, 819 arquivos, 6,6 MB no catálogo; os
  SKILL.md somam 1,33 MB. Frontmatter obrigatório: `name`, `description` (com
  triggers "Use when…" e anti-triggers "Do NOT use…"), `license: CC-BY-4.0`,
  `metadata.author`, `metadata.version`. Dump completo comitável:
  [`docs/dumps/agent-skills/INDEX.md`](dumps/agent-skills/INDEX.md).
- **Threat model próprio** (`SECURITY.md`): payload malicioso ⇒ 100% texto
  auditável; roubo de credencial ⇒ análise estática no CI (skill com chamada de
  rede ou acesso a segredo não publica); supply chain ⇒ **lockfile + content
  hashing** (atualização só explícita); prompt injection ⇒ **curadoria humana
  de cada prompt**.
- **CLI defense-in-depth** (installer): sanitização de nome (separadores, NUL,
  chars proibidos do Windows, pontos de liderança, cap de 255) → isolamento
  filesystem (`resolve()` dos dois lados antes do `startsWith`) → **symlink
  guard** (`lstat` não `stat`, validação do alvo final, detecção de loop ELOOP,
  junctions no Windows). É a `guard::path` do KIRA com três camadas a mais.
- Skills diretamente comparáveis com o que o KIRA já faz:
  - **tlc-spec-driven** (v4.1.0): fases Specify→Design→Tasks→Execute com
    **auto-sizing por complexidade**; requisitos em **notação EARS**; **gates
    determinísticos em Python antes da revisão humana** ("enforced by code, not
    memory"); aprovações gravadas **nas palavras do usuário**; **author ≠
    verifier** (Verificador independente sempre roda); blast radius declarado —
    aprovar spec/tasks autoriza **implementação e commit locais apenas**,
    `git push`/deploy exigem go-ahead explícito para aquela ação; `.specs/` com
    STATE.md (log de decisões AD-NNN) e **LESSONS.md auto-melhorável**.
  - **the-judge** (v1.4.0): revisão de PR "evidence-first" — **evidência ou
    silêncio** (`file:line` confirmado lendo o arquivo; claim externo exige URL
    oficial buscada na hora), **"training data é rumor; o changelog é fonte"**,
    **budget de ruído** (máx 5 nits inline; overflow vira contagem no resumo),
    **nunca comentar o que o tooling do próprio repo pega** (a escada
    CLI-first aplicada a review), veredito ponderado por severidade
    (🔴→REQUEST_CHANGES etc.), **a primeira rodada é a rodada inteira** (sem
    ping-pong), todo comentário passa por gate determinístico (`review_gate.py`)
    antes de postar.
  - **not-your-babysitter**: modo operador sênior — "evidence or stop",
    "fake nothing" (nenhum teste morto para ficar verde), três posturas
    (`paired/solo/heads-down` — a verificação idêntica, muda só o surfacing),
    **três únicas paradas**: ação irreversível, dead-end real, ambiguidade que
    muda o resultado.
  - **the-fool / the-jury**: protocolo de painel 3–5 subagentes com **round
    cego** (opinião independente antes de deliberar), **dissidente obrigatório**,
    cap de 5 (mais jurados = ruído correlacionado, não independência),
    "**consenso não é prova**", dissent preservado e **sempre há veredito**.
  - **harness-eval / spec-driven-eval**: eles avaliam os próprios skills com
    **Track A determinística (~0 token, sempre roda)** e tracks B/C com juízes
    LLM + traps plantadas — **opt-in, com custo anunciado ANTES** e discordância
    ⇒ hold. E `skills/harness-init/evals/trigger_evals.json` testa o *quando
    dispara*.
  - **create-adr**: MADR por padrão, "ADR = decisão já feita, imutável,
    superseded nunca editado; RFC = antes da decisão".

### harness-toolkit (resumo — a extração completa com citações está em `docs/dumps/`)

- Hooks do Cursor/Claude Code respondidos por um runtime global
  (`~/.tlc/harness/`) + política por projeto (`.tlc/harness/config.json`)
  — "global does the work, project only configures"; **nada mais roda** além
  das 8+3+25 linhas documentadas: "se uma mensagem na sua tela não veio de uma
  das trinta e seis linhas abaixo, não é o harness".
- Portas e adaptadores com **fronteiras mecanicamente checadas** + scan que
  reprova nome de vendor dentro de `core/`/`contracts/` (AD-004/010);
  `biome --error-on-warnings` + check-suppressions que exige `why:/hazard:/
  invariant:` na justificativa de qualquer supressão (AD-051).
- **139 documentos de decisão** (`ad-001…ad-146`, lacunas incluídas) — o
  changelog deles É a lista de decisões (AD-031).
- Observabilidade em três planos (signal/debug/audit) com provider por registro,
  rollup por sessão com retention, `--json` que não muda bytes nem exit codes,
  lock de gate com veredito reusado por hash de inputs e **defer** em vez de
  falha (AD-073/024).
- Lessons com tiers, refs de staleness, janela de validade e **efetividade
  gradada pelo próximo run do gate** — detalhado na extração.

---

## O que vale trazer (concreto, por módulo)

| # | Ideia | De onde | Onde cai no KIRA | Prioridade |
|---|---|---|---|---|
| P1 | **Piso de guardas em tiers + veredito tipado** (`deny/ask/block-stop/follow-up/context/record`); as 8 regras do floor deles viram o piso declarado do KIRA (escopo do repo, segredo, rewrite de história, destruição improvável, máquina, rede→shell, superfície de política, wiring), com `rule=<nome>` em toda recusa | harness-toolkit | `src/guard/` (veredito tipado + piso), `docs/kira-w5-loop.md` | **Alta** |
| P2 | **Engenharia de fadiga de aprovação**: asks por classe de risco (sai da máquina / sobrescreve caminho existente / muda permissão — `chmod` pergunta porque não aparece em diff); **append nunca pergunta**; **deadline de pergunta** ("pergunta atrasada é pior que decisão" — assume e declara em 1 linha); valor inválido de modo é **recusado, não absorvido** | harness-toolkit (AD-026) + not-your-babysitter | `src/guard/`, UX de `awaiting_approval`, spec W5 | **Alta** |
| P3 | **Lições que provam que ainda valem**: refs `caminho:símbolo` no episódio/página da wiki — ref que não resolve ⇒ página sai do pack com nota (a página que cita arquivo renomeado é pior que ausente); `valid_from`/`valid_to`; **efetividade `helped n/m`** gradada pelo próximo triage do mesmo sintoma — a medida que falta ao rascunho de cache de raciocínio (PR35 da onda TE) | harness-toolkit (AD-036/037/039/040) | `src/wiki/`, `src/context/`, integra com o cache de raciocínio (PR35) | **Alta — o pulo da rodada** |
| P4 | **Cadeia de custódia de prompt**: scan anti-injection no CI para `src/llm/prompts.rs` + fixtures (padrão de exfiltração/fetch reprova), hash por versão de prompt gravado na sessão do store | agent-skills (SECURITY.md) | CI + `src/store/` (hash da sessão) | Média |
| P5 | **Symlink/junction no jail**: `lstat` em vez de `stat` (checar antes de seguir), validação do alvo final de symlink/junction contra o root canônico, detecção de loop — junction é o caso do dono (OneDrive!) | agent-skills (installer) | `src/guard/jail.rs` (follow-up do PR27) | Média |
| P6 | **Evidence-first judge para um futuro `kira judge`** (review de PR do ADO): evidência ou silêncio; externo exige fonte oficial na hora; budget de ruído (≤5 nits); nunca comentar o que fmt/clippy/test já pegam; veredito ponderado por severidade; primeira rodada = rodada inteira | the-judge (CC-BY-4.0 — regras reimplementadas, citação obrigatória) | doc-first, depois `src/` | Média |
| P7 | **Ship claim + empty-diff anti-ship no loop**: "done" sem prova não existe — claim com diff não-vazio + evidência de teste; stop sem prova vira follow-up | harness-toolkit | `docs/kira-w5-loop.md` (spec), código depois | Média |
| P8 | **Eval com tracks**: A determinística (~0 token, sempre) é o `--eval` do M2; track qualitativa opt-in (juízes LLM com traps, custo anunciado ANTES, discordância ⇒ hold) — **report-only, nunca o gate** ("sem LLM como juiz" do ADR-0018 intocado) | harness-eval | `kira triage --eval` (extensão) | Baixa — a confirmar |
| P9 | **`kira debate` (bônus; the-fool/the-jury)**: painel 3–5 com round cego, dissidente obrigatório, veredito único + dissent preservado, cap de 5 ("consenso não é prova") — para decisões do DONO, nunca em gate, nunca no ciclo; Ollama default, `--llm` caro só explícito | the-fool/the-jury (CC-BY-4.0 — protocolo reimplementado) | `src/llm/` (novo comando) | Baixa — bônus |
| P10 | **Doctor de config redeclarada** (quando config ganhar herança global→projeto): nomear chaves que o projeto redeclara em vez de herdar, com o arquivo onde pararam de herdar | harness-toolkit (concepts) | `src/doctor.rs` (futuro) | Baixa |

## O que NÃO vale

**Do agent-skills**
- **Marketplace/registry multi-agente, CLI instalador, MCP server, curadoria de
  comunidade** — KIRA não é registry de skills; é um agente por projeto. MCP
  rejeitado no mandato.
- **94 skills de domínio alheio** (GTM/deploy/cloud/design: `ai-seo`,
  `vercel-deploy`, `aws-advisor`, `figma`…) — fora do domínio de sustentação
  ADO. O valor é o *formato* (frontmatter, triggers/anti-triggers, semver,
  autor) e as skills de processo (judge/ADR/spec), não o catálogo inteiro.
- **Copiar SKILL.md inteiro para dentro de `src/llm/prompts.rs`** — os prompts
  do KIRA são const Rust revisada em PR; skill deles inspira, não substitui
  (e CC-BY-4.0 exige atribuição — se um dia inspirar texto literal, o comentário
  do const cita autor + licença).

**Do harness-toolkit (ELv2)**
- **Código, catálogo, docs e textos deles** — a extração é paráfrase com citação
  curta; nada entra no repo MIT. O dump cru fica local.
- **Hooks de editor como runtime** — o KIRA é CLI de comando único com loop
  próprio; os eventos do editor são o mundo do harness deles. O que vem é o
  *design de veredito*, não o transporte.
- **Portar TS para Rust** — Sísifo, mesmo veredicto da rodada anterior.
- **Pricing catalogs em USD / cost alert** — o KIRA mede chamadas (budget com
  número), não dinheiro; nota se um dia precisar.

**De ambos**
- **Subagente como mecânica de primeira classe no ciclo** — custo de token sob
  budget de 50/dia; painel de debate só sob comando explícito do dono (P9).
- **`the-jury` como mecanismo de gate** — "sem LLM como juiz" (ADR-0018) manda:
  painel não mede o M2.

---

## O plano — Onda TL (tech-leads), PR44–PR51

Regra vigente: **um item = um PR**, gates padrão (`fmt --check` ·
`clippy -D warnings` · `test` · `build` · CI 3 SOs). Nenhuma dependência nova.
A numeração segue depois da fila do relatório anterior (PR34–PR43, token
economy — ainda proposta); se o dono aprovar ordem diferente, renumera — o
ledger é o `docs/roadmap-prs.md`, não este relatório.

| PR | Escopo | Arquivos | Aceite específico | Status |
|---|---|---|---|---|
| **PR44** | Piso de guardas em tiers + veredito tipado; recusa com `rule=<nome>`; as 8 regras do piso como const testada | `src/guard/` (novo módulo de veredito), `docs/kira-w5-loop.md` | Tabela floor/always-on/rails no doc; teste por regra do piso; recusa não toca disco (padrão PR3) | proposto |
| **PR45** | Fadiga de aprovação: classes de risco dos asks, append nunca pergunta, deadline de pergunta, modo inválido recusado | `src/guard/`, spec W5 | Teste por classe; `>>` não gera pergunta; valor inválido de modo ⇒ erro acionável com o nome da palavra | proposto |
| **PR46** | Lições com refs de staleness + janela de validade + efetividade `helped n/m` | `src/wiki/`, `src/context/` | Ref renomeada ⇒ página fora do pack com nota (não erro); efetividade aparece no `kira history`; integra com o cache de raciocínio (PR35 da onda TE) | proposto |
| **PR47** | Symlink/junction no jail (`lstat`, alvo final, loop) | `src/guard/jail.rs` | Junction fora do root recusada (teste Windows); loop detectado; sem `unsafe` | proposto |
| **PR48** | Custódia de prompt: scan anti-injection no CI + hash por versão no store | CI workflow, `src/store/` | Prompt com padrão de exfiltração reprova CI; sessão grava `(prompt_version, hash)` | proposto |
| **PR49** | Ship claim + empty-diff anti-ship na spec W5 | `docs/kira-w5-loop.md` | Spec com watchdogs numerados; stop sem prova = follow-up, nunca silêncio | proposto (doc) |
| **PR50** | Track qualitativa opt-in no `--eval` (juízes LLM + traps, custo antes, discordância ⇒ hold) | `src/triage.rs` | Report-only; `--eval` determinístico byte-idêntico sem a flag; custo impresso antes de rodar | **a confirmar** |
| **PR51** | `kira debate` (the-fool/the-jury, Ollama default) | `src/llm/` | Nunca no ciclo nem no gate; budget conta chamada; sem backend caro implícito | **a confirmar** |

**Nota de escopo**: PR44/45/49 dependem da onda W5 existir (spec hoje); os três
primeiros itens doc-first para o dono aprovar o desenho antes de código — igual
fez o harness-toolkit: **139 ADRs antes de confiar no runtime**.

## Ordem de execução sugerida

1. **PR46** — o pulo (lições com refs + efetividade) e o que fecha a medida do cache de raciocínio (PR35).
2. **PR44 → PR45** — piso e fadiga de aprovação, doc-first, antes do loop
   crescer para cima deles.
3. **PR47 → PR48** — endurecimento barato (junction, custódia de prompt).
4. **PR49** — spec W5.
5. **PR50/PR51** — só com aval do dono (bônus).

## Pendências do dono

1. **Aprovar o ADR-0027** — os PRs acima viram fila no `docs/roadmap-prs.md`
   (ledger do Orchestrator) só depois do aval.
2. **O dump cru do harness-toolkit fica FORA do repo (ELv2)** — se o dono
   quiser comitar mesmo assim, precisa de pasta própria com a licença ELv2
   colada e header de atribuição; a recomendação (professor paciente) é não.
3. **Rotular o `tests/fixtures/triage_gold/`** (continua vazio) — pré-requisito
   do gate M2 e da efetividade `helped n/m` do PR46; sem gold, a métrica nova
   nasce sem número.

## Permanece rejeitado (desta rodada)

Código/texto ELv2 no repo · registry/marketplace/CLI instalador/MCP · hooks de
editor como runtime · portar TS para Rust · subagente no ciclo · painel de
jurados em gate · pricing em USD · skills de domínio alheio (GTM/cloud/design).

---

## Apêndice — onde está a evidência

| Claim | Evidência (caminho) |
|---|---|
| 94 skills, 14 categorias, frontmatter/semver | `docs/dumps/agent-skills/INDEX.md` |
| Threat model + CLI defense-in-depth | `docs/dumps/agent-skills/repo-docs.md` (SECURITY.md) |
| Requisitos EARS, author ≠ verifier, blast radius | `docs/dumps/agent-skills/development.md` (tlc-spec-driven) |
| Evidence-first, noise budget, veredito por severidade | `docs/dumps/agent-skills/quality.md` (the-judge) |
| Round cego, dissidente, "consenso não é prova" | `docs/dumps/agent-skills/decision-making.md` |
| 8 floor rules + tiers + 25 rails | `docs/dumps/harness-toolkit-extracao.md` (paráfrase; fonte `capabilities/catalog.json` v15 no repo ELv2) |
| Lessons com refs/staleness/efetividade | idem (docs `lessons.md`, AD-036/037/039/040) |
| 139 decisões, fronteiras checadas mecanicamente | idem (docs `architecture.md`, `decisions/index.md`) |
