# ADR-0033 — Dashboard: leitura por padrão, execução só com confirmação explícita

> Renumerado em 12/Nov/2026: era `ADR-0029` na árvore local até a PR #64 reservar `0029`
> (vocabulário de comandos) e `0030` (local-first). Nenhum merge existiu com o número velho.

- **Status**: implementado — aguarda aceite do dono (substitui parcialmente a cláusula
  "read-only PROVADO" do AGENTS.md)
- **Data**: 12/Nov/2026 · **Decisor**: dono (Sérgio)
- **Relaciona**: ADR-0014 (casca statui) · ADR-0016 (CLI-first, LLM-last) · ADR-0013 (wiki) · AGENTS.md

## Contexto

O `kira dashboard` nasceu read-only: nenhuma tecla escrevia, e o e2e provava isso comparando o hash
do `store.db` antes/depois (`tests/dashboard_e2e.rs`). Com o W5 (loop/scheduler) e o uso diário, o
dono pediu que o dashboard **execute** `scan`, `triage` e `pr draft` **por trás dos painéis** — sem
expor uma linha de shell ao usuário — mantendo o CLI disponível por fora, porque o board precisa
estar plugado antes de um triage ao vivo.

As duas coisas não cabem na mesma frase: executar `kira scan` grava `.kira/findings.json`, e `kira
triage` grava episódio na wiki e chama o backend de IA. O contrato anterior passaria a mentir.

## Decisão

1. **Navegar nunca executa.** `Tab`, `1–4`, `j/k`, `g/G`, `i`, `Enter` de detalhe, `Esc`, `r`, `/`,
   `?`, resize e redraw não podem spawnar processo, tocar rede ou gravar arquivo. O renderer recebe
   `&Snapshot` e `&ExecState`; não conhece I/O.
2. **Só uma tecla de ação explícita, seguida de confirmação, spawna.** A ação é semântica
   (`Action::Scan`, `Triage`, `PrDraft`, `Approve`) e vira `ActionPlan` com **argv allowlist**,
   `std::env::current_exe()`, `stdin(null)`, `stdout/stderr` em pipe. Nunca shell, nunca `cmd /C`.
3. **Todo efeito é declarado antes de rodar**, na folha de confirmação: arquivo gravado, trilha,
   rede, LLM. Escrita no mundo continua exigindo `--apply` no CLI; o dashboard **não** aprova escrita
   autônoma — `awaiting_approval` continua exigindo `kira approve` humano.
4. **`kira dashboard --read-only`** desliga as teclas de ação e restaura a prova antiga: hash do
   `store.db` byte-idêntico. É o modo do e2e, do CI e de quem só quer olhar.
5. **O subprocesso é o mesmo binário.** `src/tui` continua fora da allowlist de `tests/llm_allowlist.rs`
   — quem importa `llm::*` é o `kira triage` filho, não a TUI.
6. **Saída capturada é dado, não decoração**: linhas passam por sanitização (ANSI/caracteres de
   controle) e por `crate::redact` antes de aparecer; o log não é gravado no DB.

## Consequências

- Positivas: o dashboard vira a superfície de operação do KIRA (pedido da persona §7), sem perder a
  possibilidade de provar read-only por hash em CI; o CLI continua sendo a fonte de verdade e o
  caminho para quando o board não está plugado.
- Negativas: o e2e de hash passa a rodar em `--read-only`/`--snapshot`; a suíte ganha testes de
  allowlist de argv, sanitização de log e "nenhuma tecla de navegação inicia ação".
- Riscos: subprocesso travando a TUI (thread + canal + poll de 150 ms), saída com escape quebrando a
  tela (`sanitize`), PAT vazando no log (`redact`).

## Testes que fixam a decisão

- `navigation_never_returns_an_execution_outcome` (unit, `src/tui/app.rs`).
- `plan_uses_allowlisted_argv_and_never_a_shell` (unit, `src/tui/action` — PR de execução).
- `--read-only` mantém o hash do `store.db` (e2e, `tests/dashboard_e2e.rs`).
- `sanitize_log_line_strips_ansi_and_control` (unit, `src/tui/action` — PR de execução).
