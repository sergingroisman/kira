# ADR-0042 — Embeddings locais (Gemma 2) e forecast estatístico

**Status:** Aceito (F5)
**Data:** 2026-11-17

## Contexto

O dono pediu a avaliação de modelos locais pequenos para a parte PREDITIVA do
KIRA (predição de bugs, cards, ADRs; sustentação constante e controlada). De
cinco candidatos avaliados, dois valiam o esforço: **embeddings** (busca
semântica de bugs/ADRs, classificação few-shot, RAG) e **forecasting de séries
temporais** (velocity, recorrência, capacidade). Os outros três (OCR, STT,
TTS) não ajudam predição e ficaram fora.

Duas verdades de catálogo pesaram na decisão:

1. **Timestral (mistralai/Timestral-209M) é real e forte** (SOTA zero-shot em
   GIFT-Eval/TIME/fev-bench), mas é um encoder patch-based numérico com cabeça
   de quantis — **não é um LLM de tokens**. O Ollama (llama.cpp/GGUF) só roda
   arquiteturas de token; nem os GGUFs não-oficiais do modelo declaram
   compatibilidade com llama.cpp/Ollama. Uma tag `ollama pull timestral:209m`
   era instrução mentirosa (e existia no primeiro rascunho deste PR — removida).
2. O caminho nativo real do Timestral é **runtime próprio em C++/Rust**
   (`timestral.cpp`; `rust-model-inference`, GGUF F32/Q4_K de ~120 MB) — ambos
   ainda "interim" (qualidade/latência em avaliação). Vale como alvo futuro
   NATIVO (o KIRA é Rust), não como sidecar Python/PyTorch.

## Decisão

**D1 — Embeddings via `/api/embed` do Ollama (Gemma 2).** `src/embeddings.rs`
usa `POST /api/embed` (endpoint nativo de embeddings, Ollama ≥ 0.3) com as tags
oficiais `gemma2:2b`/`gemma2:7b`. A DIMENSÃO do vetor é derivada da resposta,
nunca fixada no fonte. Índice em memória com busca por cosseno; a persistência
dos vetores no store é PR seguinte (bump de schema próprio, disciplina de
`PRAGMA user_version`). Regra OC-1 reutilizada de `llm/ollama.rs` (`pub(crate)`):
URL não-local exige chave; localhost nunca anexa header. O módulo entra na
allowlist do ADR-0016 (`ALLOWED_EXACT` + `MUST_JUSTIFY`) com `// LLM porque:`
no call site — o vetor de sentido SÓ o modelo produz.

**D2 — Forecast é ESTATÍSTICA pura (sem LLM).** `src/forecast.rs`: regressão
linear com banda p10/p90 a partir do desvio dos residuais contra a LINHA
(banda constante, z(0.10) ≈ 1,2816 — aproximação documentada, não modelo
probabilístico completo), média móvel e detecção de anomalias por z-score
(só com ≥ 10 pontos). Sem timestamps legíveis, os timestamps do forecast
ficam VAZIOS — nunca inventados. Erros são acionáveis (série < 2 pontos,
horizonte fora de 1..=365). O módulo NÃO entra na allowlist: é a camada 3 do
ADR-0016 de propósito (cálculo determinístico, zero custo, zero rede).
Datas futuras por aritmética civil própria (days/civil_from_days de Hinnant) —
sem dependência nova em `Cargo.toml`.

**D3 — Sem escolha de modelo de forecast no `setup`.** O passo de forecast do
setup é informação fixa ("engine estatística — sem LLM; Timestral é runtime
nativo futuro"), não um menu de uma opção falsa. `--forecast-model` não existe.

**D4 — Timestral fica bloqueado para integração** até o runtime nativo
(Rust/C++) passar seus gates de qualidade/latência — e a integração seguiria o
padrão de runtime externo do `rea` (jail, teto), nunca PyTorch embutido.
Reabrir a decisão é item novo de ADR, não patch deste.

## Consequências

- Positivo: busca semântica e classificação few-shot desbloqueadas com custo
  zero por chamada (local); forecast honesto e determinístico desde o dia 1;
  nenhuma dependência nova.
- Negativo: embeddings de modelo generativo (Gemma 2) são inferiores a
  embedders dedicados (`nomic-embed-text`, `bge-m3`) — aceitável para v1;
  trocar o modelo é config, não código (a dimensão vem da resposta).
- Neutro: o índice em memória re-embute os documentos a cada sessão — a
  persistência no store (com cache por hash de texto) é o que resolve, no PR
  seguinte.
