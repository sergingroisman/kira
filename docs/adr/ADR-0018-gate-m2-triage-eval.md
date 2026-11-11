# ADR-0018 — Gate do M2 é medido: `kira triage --eval` + `tests/fixtures/triage_gold/` (20 bugs, ≥70%)

- **Status**: aceito (decisão do dono, registrada no mandato v3)
- **Decisores**: Sérgio (dono) · Delta (autoria)

## Contexto

A intenção do dono é que o KIRA ganhe autonomia de escrita — mas a juíza
independente bloqueou a autonomia por falta de método: sem medição da
qualidade do triage, "o M2 passou" é opinião, e autonomia sobre opinião é
inflação. A correção registrada no mandato v3: o gate do M2 precisa virar
**número**, e o número tem que vir de dados que só o dono tem — a causa raiz
verdadeira dos bugs que ele fechou.

## Decisão

O gate do M2 é o comando **`kira triage --eval <dir>`** rodando sobre
**`tests/fixtures/triage_gold/`**: **20 bugs fechados**, cada um como input
JSON do work item + causa raiz esperada, **rotulados pelo dono** (só ele tem a
verdade da causa raiz). O comando compara a causa raiz prevista pelo triage
com o rótulo e imprime o percentual de acerto.

- O M2 só é aprovado com **≥70%** medido; não bateu, **pare e conserte o
  context packer** (ADR-0015) — sem isso não se pinta a nave.
- Autonomia de escrita (`kira loop`/agendado promovidos a "autônomo",
  ADR-0017) **só depois** deste gate; antes disso a autonomia permanece
  read-only.
- Os rótulos são do dono, não da Delta: a Delta implementa a medição, o dono
  rotula (`docs/kira-mandato.md` §4, pré-requisito (d)).

## Consequências

- **Positivas**: critério objetivo e reproduzível para ligar a autonomia;
  regressões de prompt/contexto aparecem como queda de número, não como
  surpresa do dono; o `--eval` vira comando de CI para o próprio gate.
- **Negativas**: bloqueia o dono em uma tarefa que ninguém pode fazer por ele
  (rotular 20 bugs); e 20 casos é amostra pequena — aceito, é o MVP do gate.
- **Custo de manutenção**: cada mudança de prompt/packer roda o `--eval` de
  novo; o número fica no ledger do PR.

## Alternativas

- **Avaliação manual/opinião do dono por amostra** — rejeitada: não é
  reproduzível nem comparável entre PRs.
- **Benchmark público (SWE-bench etc.)** — rejeitado: mede o modelo, não o
  produto (prompt + packer + grounding) neste domínio do dono.
- **LLM-judge avaliando o triage** — rejeitado por ora: julgar com LLM um
  produto que usa LLM não substitui rótulo humano de causa raiz; pode voltar
  como reforço pós-M2, nunca como gate.
