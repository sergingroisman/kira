# ADR-0028 — Cache de raciocínio e economia medida

**Data**: 12/Nov/2026 · **Estado**: aceite (selo do dono) · **Onda**: TE
(PR34–PR43, [`docs/te-implementacao.md`](../te-implementacao.md))

## Contexto

O plano de análise dos 4 repositórios escolhidos pelo dono (RTK, GSD-2, rea,
openclaude — PR #50) apontou um único "pulo do gato" com retorno desproporcional:
**triage que repete bug conhecido não precisa de LLM**. Episódios de bugs já
gravados em `.kira/wiki/bugs/<item-id>.md` guardam causa raiz, arquivos e
confiança; um item novo que casa com um episódio anterior (similaridade de
tokens ≥ 0,50) pode herdar a conclusão com custo **zero** de token. Hoje
(`kira triage`, prompt v2) toda análise custa uma chamada ao backend — e a
economia de qualquer melhoria de prompt/packer não é medida em lugar nenhum.

O RTK ancora a prática de medir: `rtk gain` (summary/graph/history/daily/json)
e `rtk discover` (caça economia perdida), com estimador `bytes / 4` e barra de
qualidade "each filter must cut at least 20%". O KIRA mede com `chars / 3`
(PT-BR é mais denso — estimador mais conservador; `CHARS_PER_TOKEN=3` já é
código em `src/context/mod.rs`).

## Decisão

1. **Ledger de economia (PR34)** — toda sessão grava pack chars vs teto, fonte,
   chamadas de LLM e sinal de pack ocioso; header do pack e `--json` expõem a
   estimativa `chars/CHARS_PER_TOKEN`. Gate de determinismo: mesmo estado ⇒
   header byte-idêntico e campos de valor do `--json` idênticos (ids e
   timestamps do SQLite ficam fora do gate, por design).
2. **Cache de raciocínio (PR35)** — antes de chamar o LLM, `kira triage`
   busca candidatos no FTS5 da wiki (top-3 por `keywords()`), pontua com
   similaridade de tokens (interseção sobre `content_tokens`, mesma família de
   `eval::content_tokens`) e, com hit ≥ 0,50, **responde sem LLM** com a
   conclusão do episódio anterior, fonte `cache-wiki`, `llm_calls=0` no ledger.
   `--no-cache` desliga. **Hit não grava episódio novo** (selo do dono,
   12/Nov/2026): página de bug não é log de re-execução — o hit fica no output
   e no ledger.
3. **Barra de qualidade (PR38)** — toda estratégia nova de corte do packer só
   entra se o fixture mostrar corte ≥ 20% (padrão de filtro do RTK).
4. **Medição antes de autonomia** — o cache é rota *padrão* de triage, mas a
   promoção do ciclo autônomo continua presa ao gate M2: ≥ 70% no
   `kira triage --eval` com 20 bugs rotulados pelo dono
   (`tests/fixtures/triage_gold/`). O `--eval` roda **as duas rotas** e conta
   hits — o percentual do gate não muda com cache ligado.

## Alternativas consideradas e recusadas

- **Embeddings/índice vetorial** para casar episódio — contraria
  RETRIEVAL-FIRST (ADR-0015): sem dep nova, sem modelo local; FTS5 + cálculo
  em Rust resolve.
- **Tool-calling / framework de orquestração** — idem, fora da casa.
- **Estimador `bytes / 4`** (RTK) — PT-BR é mais denso que inglês médio;
  3 chars/token fica (conservador: subestima a economia, nunca a infla).
- **Gravar episódio a cada hit** — transformaria a página do bug em log de
  re-execução (recusado pelo dono); dedup já evita duplicata, mas o valor
  informativo de "acertei de novo" pertence ao ledger, não à wiki.
- **Ajustar `KIRA_NUM_CTX` por perfil** (PR40) — o perfil coordena caps de
  pack; o teto de janela continua sendo do ambiente (fonte única de verdade).

## Consequências

- `llm_allowlist` continua verde: a rota de hit não importa `llm::*` (hit não
  fala com modelo — CLI-first, LLM-last, ADR-0016).
- Fail-open por fonte: wiki/índice ausentes ⇒ rota LLM de hoje byte a byte +
  nota no stderr.
- Cache errado é rastreável (fonte `cache-wiki` no output e no ledger) e
  desligável por invocação (`--no-cache`).
- O medidor (PR34) precede o pulo (PR35) na onda: sem número, a economia é fé.
- `.kira/` nunca comita (PR37 fecha o mecanismo); snapshots dos dumps continuam
  locais (decisão do dono).
