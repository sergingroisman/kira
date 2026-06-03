# Workshop: Dev Assistido por IA — Harness Engineering na Prática

> **Apresentado por:** Felipe Rodrigues — Staff Engineer, BHub.ai  
> **Data:** Maio 2026  
> **Fonte:** PDF original — `WORKSHOP · DEV ASSISTIDO POR IA (1).pdf`  
> **Contexto:** Este workshop é a base arquitetural do **Spektor** — e por consequência, do **Kira** (harness agentic do Sergin).

---

## Sumário

1. [O Problema](#1--o-problema-agentes-falham-e-você-não-percebe)
2. [Fundamentos: Agente = Modelo + Harness](#2--fundamentos-agente--modelo--harness)
3. [A Régua: Onde Você Está?](#3--a-régua-onde-você-está-nesse-espectro)
4. [Como Funciona: Harness em Volta dos CLIs](#4--como-funciona-harness-em-volta-dos-clis)
5. [O Contexto: 3 Coisas que Mudaram](#5--o-contexto-três-coisas-mudaram-no-último-ano)
6. [Os 7 Pilares](#6--os-7-pilares)
   - [P1 — SDD (Spec-Driven Development)](#p1--sdd-spec-driven-development)
   - [P2 — Pilot (Maestro + Instrumentos)](#p2--pilot-um-maestro-vários-instrumentos)
   - [P3 — Judge (Verificação)](#p3--judge-quem-confere-o-trabalho-do-agente)
   - [P4 — Completion Gates (Cascata)](#p4--completion-gates-barato-primeiro-caro-só-quando-precisa)
   - [P5 — Guardrails](#p5--guardrails-os-limites-que-o-agente-não-pode-cruzar)
   - [P6 — Resiliência](#p6--resiliência-modelos-quebram-provedores-quebram)
   - [P7 — Observabilidade + Logs](#p7--observabilidade--logs-caixa-preta-não-escala)
7. [Bônus: Sandbox](#bônus--sandbox-onde-o-código-gerado-pelo-agente-roda)
8. [Comparação: Spektor vs Mercado](#comparação-spektor-vs-mercado)
9. [Fechamento](#fechamento)

---

## 1 · O Problema: Agentes Falham. E Você Não Percebe.

Agentes de IA no desenvolvimento de software falham de 4 maneiras silenciosas:

| # | Falha | Sintoma |
|---|-------|---------|
| **01** | **Vibe-coding** | "Resolve aí" — sem método, sem critério de aceitação, sem auditoria. |
| **02** | **Falham silenciosamente** | Apagam testes sem querer. "Commita" tarefas quebradas. Ninguém percebe até produção. |
| **03** | **Queimam dinheiro** | Retries cegos, loops infinitos, tasks desnecessárias. R$ 200 numa tarefa trivial. |
| **04** | **Não dá pra explicar** | Quando falha, ninguém sabe o que aconteceu. Caixa-preta total. |

> **Tese central:** o problema não é o modelo. É a **falta de um harness** — a disciplina, as regras, os limites.

---

## 2 · Fundamentos: Agente = Modelo + Harness

```
┌──────────────────────────────────────────────────────┐
│                    AGENTE                             │
│  ┌──────────────────┐   ┌──────────────────────────┐ │
│  │     MODELO        │   │        HARNESS           │ │
│  │  (O cérebro)      │   │  (O corpo, os reflexos,  │ │
│  │  É commodity —    │   │   a sala de cirurgia)     │ │
│  │  troca de Claude   │   │  Decide QUEM recebe COMO, │ │
│  │  pra GPT, pra     │   │  valida o output.         │ │
│  │  Gemini — e o     │   │                           │ │
│  │  problema acaba.  │   │  **Não é commodity.**     │ │
│  └──────────────────┘   └──────────────────────────┘ │
└──────────────────────────────────────────────────────┘
```

- **Modelo** = o cérebro. É commodity — você troca de Claude pra GPT, pra Gemini e o diferencial acaba.
- **Harness** = o corpo, os reflexos, a sala de cirurgia. Decide qual modelo recebe cada tarefa, com que prompt, e o que fazer com o output.
- **Consequência:** se o modelo é commodity, a vantagem competitiva está no harness.

---

## 3 · A Régua: Onde Você Está Nesse Espectro?

```
Outside ────→ In ────→ On ────→ Flywheel
the loop     the loop  the loop  (Self-improving)

"Resolve aí"  Micromanage  Harness     O harness
— e torce     — vira o     Engineering  aprende
pro melhor    próprio      — edita o    com cada
              gargalo      harness,     erro
              (RISCO)      não o output
```

> *"The difference between 'in the loop' and 'on the loop' shows up when you don't like the result. People on the loop change the harness that produced the artifact."*  
> — **Kief Morris, ThoughtWorks**

Quem está **on the loop** edita o harness (prompts, gates, guardrails, thresholds). Quem está **in the loop** edita o output (micromanage, revisa cada linha). Quem está **outside the loop** faz vibe coding.

---

## 4 · Como Funciona: Harness em Volta dos CLIs

```
┌────────────────────────────────────────────────────┐
│                   HARNESS                           │
│                                                     │
│   SDD  ──  Gates  ──  Judge  ──  Observabilidade   │
│                                                     │
│            CHAMADA via subprocesso                  │
│         ┌──────┐  ┌──────────┐  ┌───────┐          │
│         │Cursor│  │Claude Code│  │Codex  │  ...     │
│         └──────┘  └──────────┘  └───────┘          │
└────────────────────────────────────────────────────┘
```

**Por que subprocesso?**
- Cada chamada tem seu próprio processo — pode abortar, retry, trocar de modelo.
- Hoje é Cursor, amanhã Claude Code, depois Codex. Ou os três juntos, se quiser.
- O harness (SDD, gates, juiz, logs) fica no orquestrador, independente do modelo.

---

## 5 · O Contexto: Três Coisas Mudaram no Último Ano

| # | Mudança | Impacto |
|---|---------|---------|
| **01** | **Mesmo modelo, harness diferente → resultado diferente** | No Terminal-Bench, ChatGPT-4o viu sua taxa de acerto saltar só pela troca do scaffold ao redor do modelo. |
| **02** | **Fronteira ainda falha sem disciplina** | Abr/2025: Opus 4.5 perde tarefas. Mas o harness obriga feature_list, diário de progresso, commits atômicos. O modelo é forte; o que falta é harness. |
| **03** | **OWASP elevou a risco de segurança** | Top 10 for Agents (2026): as vulnerabilidades mais disparadas são risco de segurança — não só de qualidade de código. |

> **Tese:** para dois agentes competirem, o diferencial não está no modelo. Está no harness.

---

## 6 · Os 7 Pilares

### P1 — SDD (Spec-Driven Development)

**Problema:** Agente completa tasks sem spec. Sem spec, não há critério de aceitação. E o pior: o juiz que vai avaliar depois não tem gabarito — porque o critério era a spec que não foi escrita.

**O pipeline de 4 fases:**

```
┌──────────┐   ┌─────────┐   ┌─────────┐   ┌─────────┐
│ SPECIFY  │ → │ DESIGN  │ → │ TASKS   │ → │ EXECUTE │
│ spec.md  │   │ design  │   │ tasks   │   │ per-task│
│          │   │ .md     │   │ .json   │   │ agents  │
└──────────┘   └─────────┘   └─────────┘   └─────────┘
      ↑              ↑             ↑
      └── A spec é o gabarito do design ──┘
      └── O design é o gabarito das tasks ──┘
      └── As tasks, com spec+design, viram o artefato de execução ──┘
```

**Auto-sizing:** tarefa trivial (bug fix de 3 linhas, 1 arquivo) pula fases. Senão, burocracia.

**Como o Spektor faz:** cada fase escreve seu artefato via um agente Writer com prompt específico. Fase avança só se gate passa. Falha = halt + artefatos persistidos para inspeção.

---

### P2 — Pilot (Um Maestro, Vários Instrumentos)

**Problema:** único chamada de modelo, perde contexto entre iterações, comemora vitória cedo, esquece a parte fácil e só entrega metade.

**O padrão:**

```
┌───────┐     ┌──────────┐     ┌──────────┐
│ PILOT │ ──→ │ EXECUTOR │ ──→ │ VERIFIER │
└───────┘     └──────────┘     └──────────┘
  decide         roda a          confere,
 qual task       task            passa pelas
   vem           com o           gates,
                 modelo          reporta
```

**Como o Spektor faz:** Pilot é uma máquina de estados que decide qual intenção emitir. Executor roda a intenção (spawn de agente, chamada LLM, etc.). Verifier confere o resultado contra os gates. O loop se repete até Done ou Halt.

---

### P3 — Judge (Quem Confere o Trabalho do Agente?)

**Problema:** agente faz logout, deleta o login antigo, a feature de logout "funciona", e os testes continuam passando — porque ninguém testava o código deletado. Sistema quebrado segue.

**O conceito:** **Humans-as-Judge** — um modelo separado do executor, que olha o resultado com os critérios de aceitação e decide se passa.

- **No planejamento:** quem aprova o plano não é quem executa.
- **Na execução:** quem julga não é quem escreveu. Jamais auto-aprova.

> *"judge_model ≠ writer_model" é obrigatório.* Sem isso, o agente aprova o próprio viés.

**Como o Spektor faz:** Judge é um agente separado, Heavy-tier, read-only. Só escreve `eval_runs`. Usa prompt determinístico (temp=0.0, json_object). Não participa da escrita — só avalia.

---

### P4 — Completion Gates (Barato Primeiro, Caro Só Quando Precisa)

**Problema:** "pronto" não pode ser o agente dizendo que está pronto. Precisa ser o sistema provando que está pronto.

**A cascata (5 níveis, do mais barato ao mais caro):**

```
L1 — Structural Validation     (grátis)
  ↓
L2 — File Integrity            (grátis)
  ↓
L3 — Change Sufficiency        (grátis)
  ↓
L4 — Embedding Faithfulness    (~$0.00007)
  ↓
L5 — Judge LLM                 (~$0.10)
  ↓
TAREFA ACEITA (ou rejeitada)
```

**Short-circuit no primeiro Fail.** 

**Como o Spektor faz:** L1-L3 são heurísticas determinísticas (lint, brace balance, diff ratio). L4 usa cosine similarity entre embedding da spec e do output — se ≥ 0.80, confiança alta, pula L5. Só casos duvidosos (0.55–0.80) escalam pro juiz caro. L4 economiza $0.10 por gate run na maioria dos casos.

---

### P5 — Guardrails (Os Limites que o Agente Não Pode Cruzar)

**Problema:** Não são sugestões. São portas trancadas.

| Guardrail | O que faz |
|-----------|-----------|
| **File Integrity** | Arquivos que o agente NÃO pode reescrever livremente: spec, testes que validam a spec, arquivos de configuração. Se o agente apaga teste que falha — bloqueado. Clássico. |
| **Change Sufficiency** | O diff tem que ser proporcional à tarefa. Tarefa estimada em 50 linhas com diff de 2 linhas? Suspeito. Diff de 500 linhas pra um bug fix? Scope creep. |
| **Cost Cap** | Budget explícito. Esgotou: para. Sem loop infinito queimando R$ 200 numa tarefa trivial. |

> Cada guardrail vem com **defaults seguros** (opt-out, não opt-in). Você NUNCA deve lembrar de habilitar. Afinal: "arquivos importantes e você esquecer a trava de sobrescrita — acidente Markdown" (Anthropic, 2025).

**Como o Spektor faz:** Ownership registry com claims glob-based (Exclusive/Shared). File integrity = `touched_paths ⊆ owned_paths`. Change sufficiency = `actual_loc` dentro de `[0.3×, 3.0×]` do `estimated_loc`. Cost cap = budget hard-stop no router.

---

### P6 — Resiliência (Modelos Quebram. Provedores Quebram. Tarefas Travam.)

**Os 4 mecanismos:**

| Mecanismo | Descrição |
|-----------|-----------|
| **Stall detection** | Agente parado sem progresso → detecta e sinaliza. 3 sinais: mesmo tool+args repetido 3x, output hash inalterado 3x, 90s sem progresso no wallclock. |
| **Fallback chain** | Cadeia de modelos: primário falhou → fallback 1 → fallback 2. Do mais forte pro mais barato como último recurso. |
| **Escalation** | Cadeia esgotou → parar tudo OU pular-e-seguir (depende da severidade). |
| **Failure classification** | Agente falhou — mas por quê? Tratamento difere: AgentMistake (retry), InfraTransient (retry c/ backoff), InfraPermanent (escalar), Stall (cancelar). |

**Caso real:** time popular tinha um agente que só disparava em horário de pico. Quando travava (sem timeout), ficava 15 minutos parado em silêncio. Sem porta trancada. Stall detection teria disparado fallback em segundos, não em minutos.

---

### P7 — Observabilidade + Logs (Caixa-Preta Não Escala em Produção)

**Problema:**
- Se deu certo, o agente disse que deu certo. → *Não dá pra provar.*
- Se deu errado, o agente disse que deu errado. → *Não dá pra refutar.*
- Custo? → *Não dá pra atribuir.*
- Decisão? → *Não dá pra rastrear.*
- **Você não tem produto. Você tem fé.**

**Duas camadas no Spektor:**

| Camada | Descrição |
|--------|-----------|
| **1. Events** | Cada execução emite eventos (gate passou, gate falhou, tool chamada, stall detectado). Exportável pra Grafana/OTel. |
| **2. Hard Links (Bundle)** | Versão imutável do harness: hash SHA-256 da config (`tokens.toml`), dos workflows, dos prompts, das lessons. Ex: `bundle_12a3f, bc91e...`. |

> "Ficou mais lento desde terça?" — com bundle hash, você compara duas versões do harness e sabe exatamente o que mudou.

---

## Bônus · Sandbox: Onde o Código Gerado pelo Agente Roda?

**Por que importa:** código gerado por IA precisa rodar em sandbox real. Senão: Docker comprometido → código que apaga arquivos do host, exfiltra dados ou escala privilégio.

**Espectro de isolamento:**

```
NENHUM ── DOCKER ── SECCOMP ── gVisor ── Firecracker ── microVMs
(isolamento)                                                       (isolamento)
  ← pior                                          estado da arte →
```

**Posição honesta do Spektor:** hoje isola por subprocesso e por fronteira de plugin. O ideal é microVM (Firecracker), mas o custo operacional ainda é alto. O caminho: Docker → SECCOMP (syscalls filtradas) → gVisor → Firecracker.

---

## Comparação: Spektor vs Mercado

### Comparação 01 — Filosofia

| Ferramenta | Foco Primário | Modelo | Disponibilidade |
|------------|---------------|--------|-----------------|
| **Spektor** | SDD para software dev | Agnostico, local-first | Closed · US$ 500/mês |
| **Devin** | Engenheiro autônomo de software | Multi-modelo | Closed · cloud |
| **OpenCode** | Assistente multi-camada | Multi-modelo | Open Source |
| **Hermes** | Agente melhorável | Multi-modelo | Open Source |

> **Devin** é o concorrente direto no espaço de dev — fechado, cloud, sem espírito open.  
> **Spektor** é o nicho: engenharia de harness em volta de SDD para dev de software.

### Comparação 02 — Tabela Completa (Pilares)

| Pilar | Spektor | Devin | OpenCode | Hermes |
|-------|---------|-------|----------|--------|
| **1 · SDD** | ✅ Core, 4 fases | ❌ Sem método | ❌ | ❌ |
| **2 · Pilot + Orchestrator** | ✅ Maestro + instrumentos | ✅ Multi-agent | ~ | ~ |
| **3 · Verification** | ✅ Calibrável | ~ | ❌ | ~ (Feedback layer) |
| **4 · Gates** | ✅ Cascata 5 níveis | ~ | Testes | ❌ |
| **5 · Guardrails** | ✅ File + Chain + Cost | ✅ Sandbox | ~ | ~ (Context layer) |
| **6 · Resiliência** | ✅ Multi-trigger + class | ✅ Recovery | ~ (Retry básico) | ~ (Retry básico) |
| **7 · Observabilidade** | ✅ Events + Logs | ~ Dashboard remoto | ~ Dashboard local | ~ Trajectories |
| **Sandbox** | ✅ Configurável até microVM | ✅ | ❌ | ❌ |

---

## Fechamento

> ### "O MODELO É COMMODITY.  
> ### O HARNESS É A VANTAGEM COMPETITIVA."

**O que levar deste workshop:**

1. **Não edite o output. Edite o harness.** Quando o agente erra, mude os prompts, as gates, os guardrails — não o código gerado.
2. **SDD é o gabarito.** Sem spec escrita antes, não há o que julgar depois.
3. **Gates são portas, não sugestões.** L1-L3 são grátis. Use-as sempre.
4. **Judge não pode ser o mesmo modelo que escreveu.** `judge_model ≠ writer_model`.
5. **Observabilidade não é luxo.** Se você não sabe por que deu errado, não sabe o que consertar.

---

> **Felipe Rodrigues** — Staff Engineer, BHub.ai  
> Criador do **Spektor**  
> [linkedin.com/in/felipfr](https://linkedin.com/in/felipfr)
