# ADR-0043 — `kira start`: um comando, zero interação

**Status:** Aceito (START-01)
**Data:** 2026-11-17

## Contexto

O objetivo do MVP (decisão 3 do dono, 17/Nov/2026) é `kira start` colapsar a
cadeia manual — setup → trust → ingest → agenda → primeiro tick — num comando
só, sem UMA pergunta. Antes disso, o dev precisava rodar cinco comandos na
ordem certa, e o "rodar enquanto eu durmo" ficava atrás dessa barreira.

O padrão de referência é o onboarding-com-estado do openclaude
(`projectOnboardingState.ts`, apêndice 10.1 do plano): estado retomável, cada
etapa idempotente.

## Decisão

**D1 — Orquestração em `main.rs`, nada reimplementado.** `Command::Start`
chama as peças reais: `setup::run` com `--yes` (fonte única de efeitos do
setup), `trust::accept`, `schedule_add` (o mesmo `kira schedule add`) e
`kira::r#loop::run` com `once` (o mesmo `kira loop --once`). Mesmo princípio
do ciclo: quem orquestra não duplica lógica das fases.

**D2 — O estado de retomada é o PRÓPRIO MUNDO.** Nada de tabela de
"onboarding" no store: config.toml existe?, `.kira/trust.json` com hash
vigente?, linha da fase na tabela `schedule`? Cada passo DERIVA o estado do
mundo antes de agir — re-rodar `kira start` nunca duplica agendamento nem
refaz pergunta. É o mesmo princípio do "próximo tick é derivado, nunca
gravado" do scheduler (ADR no cabeçalho de `src/scheduler/mod.rs`): uma
segunda fonte de verdade discorda do mundo cedo ou tarde.

**D3 — Agenda default: scan 30m, digest 2h (plano item 11.1).** O publish
fica SEM linha: fase sem cadência declarada roda no default do ciclo
(`SchedulePlan::is_due` — declarar é o que restringe), e publicar a cada tick
é barato (só publica rascunho novo; igual não reescreve). Dedupe por AÇÃO:
uma linha existente da fase — mesmo desligada — é escolha do dono; o start
não a sobrescreve nem duplica.

**D4 — `--dry-run` é consulta pura.** O plano é derivado do mundo e impresso;
NADA executa. A leitura da agenda usa `Store::open_readonly_at` (a mesma
abertura do dashboard): mostrar o plano nunca cria `store.db` como efeito
colateral, e o dry-run não grava sessão (mesma regra de history/note/profile).

**D5 — Falha de passo para o start, o que veio antes fica.** Cada falha é
acionável e lembra que o estado é retomável ("rode `kira start` de novo:
nada será duplicado"). O resumo final segue o formato do setup (`pronto:` /
`funciona agora:` / `próximo passo: kira daemon run`) — nunca "configuração
concluída".

## Consequências

- Positivo: a jornada do MVP ("rode `kira start` uma vez e de manhã veja a
  doc viva") fica num comando; nenhuma dependência nova; zero-toque é
  testável por e2e de pasta nova (sem UMA pergunta).
- Negativo: o primeiro tick sem Ollama conta falhas de post (`digest_failed`)
  em vez de gerar doc — o ciclo continua, e o resumo diz onde está.
- Neutro: sem Ollama o setup escolhe "nenhuma por agora" e imprime o pulado;
  o `kira start` não liga IA implicitamente (o caro nunca é default).
