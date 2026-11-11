# ADR-0020 — Backend Foundry BYOK via HTTP OpenAI-compatible; Ollama continua default

- **Status**: aceito (PR11b)
- **Decisores**: Sérgio (dono) · Delta (autoria)

## Contexto

O caminho "caro" do ADR-0019 era o spawn do CLI do Copilot (`copilot -p`), mas
o CLI está **bloqueado por SAML** na máquina do dono — o caminho HTTP é o que
destrava. O dono tem acesso ao Microsoft Foundry pelo benefício Visual Studio,
com recurso próprio (`sdossantosjunior-5474-resource`) e chave de API. O PR11
(padronizado em flag `--llm`/`KIRA_LLM`) ainda não estava na main quando este
PR partiu, então `LlmKind` e `from_flag_and_env` nasceram aqui, sem quebrar o
que já existe (Ollama segue sendo o único default).

O deployment em uso é `gpt-5` (família GPT-5): modelos GPT-5 e o-series
**rejeitam** `temperature` (só aceitam o default do servidor) e **rejeitam**
`max_tokens` (exigem `max_completion_tokens`); mandar os campos antigos devolve
400 `unsupported_parameter`.

## Decisão

Novo `LlmBackend`: `src/llm/foundry.rs` (`FoundryBackend`), via `reqwest`
(dependência que já existe — nenhuma adicionada). Regras:

1. **O backend caro é explícito, nunca implícito**: só roda com
   `--llm foundry` ou `KIRA_LLM=foundry`. Ollama é e continua o default;
   nenhum fallback silencioso em nenhuma direção.
2. **Duas superfícies, escolhidas pelo formato do endpoint**:
   - **v1** (endpoint termina em `/v1`): `POST {endpoint}/chat/completions`,
     sem `/openai/deployments/...` e sem `?api-version`; o deployment vai no
     campo `model` do corpo. Exemplo real do dono:
     `https://sdossantosjunior-5474-resource.openai.azure.com/openai/v1`
     + deployment `gpt-5`.
   - **legada**: qualquer outro endpoint vira
     `{endpoint}/openai/deployments/{deployment}/chat/completions?api-version=
     {KIRA_FOUNDRY_API_VERSION}` (default `2024-10-21`, sobreponível).
   - Endpoint de projeto (`*/api/projects/*`) é recusado de cara com erro
     acionável: ele não serve chat completions.
   - `KIRA_FOUNDRY_URL` definida => usada VERBATIM (escape para gateways
     OpenAI-compatible e para os testes offline).
3. **Auth**: header `api-key: <chave>` por default (padrão Azure);
   `KIRA_FOUNDRY_AUTH=bearer` troca para `Authorization: Bearer` (gateways).
   Chave ausente/vazia => erro acionável SEM chamada de rede.
4. **Corpo por família de modelo** (`KIRA_FOUNDRY_BODY_MODE=auto|classic|
   reasoning`, default `auto`): `auto` detecta pelo prefixo do
   deployment/modelo (case-insensitive) — `gpt-5`, `o1`, `o3`, `o4` são
   *reasoning* (sem `temperature`, com `max_completion_tokens`); o resto é
   *classic* (`temperature: 0.2` + `max_tokens`). `classic`/`reasoning`
   forçam o formato. `max_tokens`/`max_completion_tokens` vêm de
   `KIRA_FOUNDRY_MAX_TOKENS` (default 1024, parse validado).
5. **Erros acionáveis, sem chave**: `FoundryAuth` (401/403 — como gerar nova
   chave; Entra ID fica para o roadmap), `FoundryDeployment` (404 — onde
   conferir), `FoundryApiVersion` (400/404 citando `api-version` — dizer para
   definir a env), `FoundryBodyMode` (400 com `unsupported_parameter`/
   `temperature`/`max_tokens` — sugerir `KIRA_FOUNDRY_BODY_MODE`),
   `FoundryRateLimited` (429 — a guardrails v2 traz `retry-after`, que é
   incluído na mensagem), `FoundryFailed` (outros status/shape: 1ª linha de
   `error.message`, ≤200 chars, mascarada por `redact`, nunca body cru) e
   timeout por `KIRA_FOUNDRY_TIMEOUT` (default 120s). A chave nunca aparece
   em log, erro, `Debug` ou trilha; headers nunca são ecoados.

## Fronteira de dados

**Dados de cliente/Avanade NUNCA entram neste recurso de learning do
Microsoft Foundry.** Para trabalho de cliente: Ollama local ou um endpoint
explicitamente aprovado pelo cliente. Este recurso é para estudo/roadmap do
dono — o prompt do triage é redigido por `redact` e o KIRA não manda segredo
para lugar nenhum, mas a regra de fronteira é de escopo, não de técnica.

## Consequências

- **Positivas**: destrava o caminho caro sem depender do CLI bloqueado por
  SAML; duas superfícies cobrem o recurso atual do dono (v1) e os formatos
  antigos/gateways; testes 100% offline com servidor HTTP falso.
- **Negativas**: gestão de chave é do operador (env hoje; keyring depois);
  a fronteira de dados depende de disciplina do operador — a env é opt-in
  por execução, nunca default.
- **Risco de má interpretação**: `KIRA_LLM=foundry` numa sessão de cliente é
  decisão explícita do operador, e o default do binário segue sendo Ollama.

## Alternativas

- **Spawn do CLI Copilot (ADR-0019)** — segue válido como plano B quando a
  policy liberar o SAML; o PR11 (spawn) pode reusar o `LlmKind` criado aqui.
- **SDK do Copilot em preview** — rejeitado por ora: mesma razão do ADR-0019.
- **Enviar dado de cliente ao recurso de learning** — rejeitado: viola a
  fronteira de dados acima e o contrato do repositório.
