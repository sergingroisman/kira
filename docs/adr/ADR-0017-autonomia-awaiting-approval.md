# ADR-0017 — Autonomia por loop/agendado no mesmo binário, com `awaiting_approval` + `kira approve`

- **Status**: aceito (decisão do dono, registrada no mandato v3)
- **Decisores**: Sérgio (dono) · Delta (autoria)

## Contexto

A intenção do dono é explícita: o KIRA é um "agente vivo" — o ciclo roda
sozinho, em loop ou agendado, e evolui de assistente de leitura para executor.
A juíza independente (veredito no mandato v3) bloqueou a primeira forma desse
ciclo por um motivo real: **nenhum PR criava o círculo de aprovação humana**.
Sem esse círculo, um modo agendado poderia escrever no mundo sem ninguém
olhando — e "Devin-lite" sem freio verificável é risco, não produto.

## Decisão

O ciclo autônomo é um **modo do mesmo binário `kira`** — `kira loop` e o modo
agendado (registrado opt-in no Task Scheduler/cron) — sem daemon, sem serviço
separado. A autonomia é travada por três travas, nesta ordem:

1. **`awaiting_approval` é estado, não convenção.** Antes de qualquer escrita
   no mundo (Azure DevOps, repo do projeto, blog), o ciclo grava o estado
   `awaiting_approval` no store SQLite e para. Nenhuma transição de escrita
   acontece por conta própria.
2. **Só o humano aprova**: `kira approve <id>` é a única transição que leva o
   ciclo de `awaiting_approval` para `applying`. Budget estourado **não** é
   aprovação; `--apply` isolado **não** é aprovação.
3. **Budget com número**: `max_llm_calls_per_day = 50` em `config.toml`,
   checado ANTES de cada transição de estado — quota é proteção de custo, não
   atalho de consentimento.

Pré-requisitos de ativação (medidos, não opinados): gate do M2 ≥70% via
`kira triage --eval` (ADR-0018) e `--apply` explícito em qualquer escrita.
O modo agendado só é registrado no agendador **depois** de `kira loop` existir
e ser testado (senão o agendamento falha em silêncio).

## Consequências

- **Positivas**: o dono pode deixar o KIRA rodando diário porque toda escrita
  tem fila visível (`kira history`/store) e portão humano; auditoria trivial —
  cada transição é uma linha no SQLite com carimbo de tempo.
- **Negativas**: o ciclo não é "hands-off" — o dono tem que aprovar; e o estado
  a mais na máquina de estados custa cobertura de teste (transições
  `awaiting_approval` → `applying`/`rejected` precisam de e2e).
- **Saída canônica continua texto**: loop/agendado imprime texto copiável por
  log; a TUI (ADR-0014) nunca substitui isso.

## Alternativas

- **Daemon/serviço separado (Axum/SSE)** — rejeitado no mandato v3: processo
  extra, portas abertas, e o TUI também vira modo do binário.
- **Escrita autônoma direta (Devin completo)** — rejeitada: sem o círculo de
  aprovação a autonomia é ficção perigosa; veredito da juíza.
- **Orquestrador externo (LangChain/framework)** — rejeitado: dependência de
  framework não entra neste repo MIT; o binário único é contrato.
