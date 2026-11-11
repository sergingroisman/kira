# ADR-0019 — LLM caro via Copilot spawn (`copilot -p`); Ollama local é o default

- **Status**: aceito (decisão do dono, registrada no mandato v3)
- **Decisores**: Sérgio (dono) · Delta (autoria)

## Contexto

A intenção do dono, citada no plano: o Ollama local é 100% gratuito e privado,
mas às vezes o triage precisa de mais força do que um modelo 4B — e aí o
"costly" entra em cena. A Avanade (empregador do dono) tem licença Copilot,
porém **policy** governa o que pode rodar: usar o CLI do Copilot é
pré-requisito de aprovação corporativa, não decisão técnica unilateral. Se a
policy bloquear, o produto não pode depender do caminho caro.

## Decisão

O backend "LLM caro" é o **spawn do CLI do Copilot** (`copilot -p "<prompt>"`)
como novo `LlmBackend` — subprocesso isolado, sem SDK na fase do MVP, sem
chave em arquivo. Regras:

1. **Ollama é o default do binário**, sempre localhost — o caminho caro nunca
   é implícito: só roda **sob flag explícita** (env/flag de ativação) definida
   pelo dono/operador, que marca o que é "costly" (`kira note costly=true ...`
   ou campo no item, mandato v3).
2. **Policy da Avanade é pré-requisito**: antes de qualquer PR que ligue o
   backend Copilot, o dono confirma que o uso é permitido. Sem essa
   confirmação, o caminho registrado é **ollama-only no roadmap**, e o BYOK
   HTTP vira PR11b (plano v2 §9).
3. **O mesmo contrato do ADR-0016 se aplica**: a chamada caro precisa passar
   no teste de admissão (pergunta de julgamento, nenhuma camada inferior
   resolve, `// LLM porque:` no call site) e respeitar o budget de
   50 chamadas/dia — chamada caro consome budget, Ollama também.
4. **Segredo nunca vai ao prompt**: o prompt leva só o context packer
   redigido por `redact`; o spawn do CLI não ecoa o prompt em log.

## Consequências

- **Positivas**: upgrade de qualidade sob demanda sem infraestrutura nova
  (o CLI já está na máquina do dono); fallback de custo zero sempre disponível;
  o trait `LlmBackend` já preparado no repositório torna o backend plugável.
- **Negativas**: spawn de CLI de terceiro é menos estável que HTTP (saída
  muda de versão para versão) — exige parser tolerante e fixture de saída;
  e a dependência de policy corporativa é risco externo, tratado pelo
  ollama-only do item 2.
- **Risco de má interpretação**: "caro" não significa "usado por padrão" — o
  default permanece Ollama; o caminho caro existe para os casos marcados.

## Alternativas

- **SDK do Copilot (pin preview)** — registrado como evolução no roadmap
  (PR11) com fallback spawn de CLI; na fase atual o spawn `copilot -p` é
  suficiente e evita dependência de SDK em preview.
- **BYOK HTTP (chave própria, API de terceiro)** — rejeitado por ora, vira
  PR11b: adiciona gestão de chave e transferência de código para nuvem sem
  ganho sobre o que a licença Copilot já dá ao dono.
- **Enviar dados a LLM de nuvem sem flag/approval** — rejeitado: viola o
  contrato read-only/seguro do repositório e a ADR-0016.

## Implementação (PR11)

- `src/llm/copilot.rs`: `CopilotBackend` compõe `system` + `\n\n` + `prompt`
  num único texto e spawna `copilot -p <texto> -s` (subprocesso isolado, sem
  HTTP/SDK). Timeout por `KIRA_COPILOT_TIMEOUT` (default 180s) com kill no
  fim do budget; stdout/stderr drenados em threads durante a espera.
- Resolução do binário: `KIRA_COPILOT_BIN` (arquivo existente) → senão
  busca `copilot` no PATH (helper `which`-style extraído para
  `src/binresolve.rs`, compartilhado com o scan sem mudar seu
  comportamento); `.cmd`/`.bat` no Windows passam pelo wrapper `cmd /C`.
  Não achou => `CopilotMissing`.
- Mapa de erros (`src/error.rs`):
  | Caso | Erro | Mensagem |
  | --- | --- | --- |
  | binário ausente | `CopilotMissing` | instale o Copilot CLI + `copilot auth login`, ou use o default Ollama (`--llm ollama`) |
  | exit != 0 com stderr de auth (saml, sso, 'not authorized', 401, 403, login — case-insensitive) | `CopilotAuth` | exatamente: (1) `copilot auth login`; (2) autorizar o app do Copilot para a organização em https://github.com/settings/connections/applications (botão 'Grant'/'Authorize' na org Avanade); (3) se o admin bloquear apps OAuth, pedir aprovação à TI; (4) enquanto isso `--llm ollama` |
  | exit != 0 outros | `CopilotFailed` | 1ª linha do stderr (≤200 chars, sem body cru) + testar `copilot -p teste` fora do KIRA |
  | timeout | `CopilotTimeout` | limite atingido + `KIRA_COPILOT_TIMEOUT=<segundos>` para aumentar |
  | exit 0 com saída vazia | `CopilotFailed` | "não devolveu texto" |
- Parser de saída tolerante: remove ANSI (CSI/OSC), descarta linhas de banner
  (heurística documentada: inicia com box-drawing/braille/check-mark, ou cita
  "copilot"/"using model") e devolve o texto limpo (linhas internas vazias
  preservadas — o triage tem seções). Vazio => `CopilotFailed`.
- Wiring: `kira triage ... --llm <ollama|copilot>` (default `ollama`) e env
  `KIRA_LLM` equivalente (a flag vence). Valor desconhecido => `UnknownLlm`.
  `--llm copilot` sem CLI => `CopilotMissing` — **nunca fallback silencioso
  para Ollama**.
- Segredo: o prompt já vem redigido pelo context packer (`redact`); o spawn
  não ecoa o prompt nem o stderr cru em log/erro.
- Testes offline (sem rede, sem login): shims falsos via `KIRA_COPILOT_BIN`
  (sucesso com ANSI+banner, SAML no stderr, falha comum, binário ausente,
  timeout curto) e e2e em `tests/triage_e2e.rs` com `KIRA_OLLAMA_URL` em
  porta morta — provando que o Ollama não é usado.
