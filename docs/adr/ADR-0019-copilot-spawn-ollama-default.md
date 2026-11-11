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
