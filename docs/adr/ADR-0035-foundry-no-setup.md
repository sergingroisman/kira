# ADR-0035 — Foundry configurável pelo `kira setup` (formulário inline, chave no keyring)

> Renumerado em 12/Nov/2026: era `ADR-0031` na árvore local até a PR #64 reservar `0029`
> (vocabulário de comandos) e `0030` (local-first). Nenhum merge existiu com o número velho.

- **Status**: implementado — aguarda aceite do dono · **Data**: 12/Nov/2026
- **Relaciona**: ADR-0020 (Foundry BYOK) · ADR-0025 (setup ensina) · ADR-0034 (wizard de
  tela cheia) · AGENTS.md (segredo só em env ou keyring)

## Contexto

O wizard do ADR-0034 abria a etapa de IA com três opções, mas a opção 3 (Microsoft Foundry)
só funcionava quando `KIRA_FOUNDRY_ENDPOINT`, `KIRA_FOUNDRY_DEPLOYMENT` e `KIRA_FOUNDRY_API_KEY`
já estavam no ambiente. Sem elas, o wizard era obrigado a recusar a escolha ("escolha 1 ou 2"),
porque `setup::run` falharia logo depois pedindo as envs.

O resultado era o pior tipo de beco: a etapa existe, o backend é oferecido, e o dono precisa
sair do wizard, descobrir três nomes de variável e voltar. O ADR-0020 previa "keyring depois"
para a chave; o PAT já vive no keyring desde o ADR-0025, com `CredStore` especificado e
`InMemory` em testes — a infraestrutura para guardar a chave existe e é a mesma.

## Decisão

1. **Formulário inline na etapa IA.** Escolher `3` abre três campos no painel (endpoint,
   deployment, chave), com `Tab`/`↑`/`↓` entre eles, `Enter` validando e `Esc` fechando o
   formulário sem sair da etapa. A chave é renderizada como máscara de tamanho fixo: o valor
   nunca entra no buffer de desenho.
2. **Detecção antes de perguntar.** Endpoint e deployment vêm pré-preenchidos do ambiente
   quando existem; a chave nunca é pré-preenchida (segredo só vem do teclado). Se as três envs
   já existem, `3` avança direto e o formulário nem abre.
3. **Validação local antes do teste.** `foundry_problem` recusa endpoint vazio, sem `https://`,
   endpoint de projeto (`/api/projects/`, que não serve chat completions), deployment vazio ou
   com espaço e chave vazia — com a causa e o campo culpado. Nada disso toca a rede.
4. **Persistência é o que torna o formulário útil.** Depois que a chamada barata de teste passa:
   - endpoint e deployment (públicos, não são segredo) vão para `config.toml` como
     `foundry_endpoint` / `foundry_deployment`;
   - a chave vai para o keyring do OS, service `kira`, entrada `foundry-api-key`;
   - **chave recusada (401/403/404) não grava nada**, a mesma regra do PAT.
5. **Prioridade env > config > keyring.** `FoundryBackend::from_sources()` lê o endpoint e o
   deployment por `merge_lookup` (env vence o arquivo) e a chave de `KIRA_FOUNDRY_API_KEY` ou,
   na ausência dela, do keyring. Quem exporta não é surpreendido pelo que ficou salvo; quem
   configurou pelo wizard não precisa exportar nada.
6. **Só endpoint e deployment aceitam vir do arquivo.** `KIRA_FOUNDRY_URL`, `_API_KEY`,
   `_AUTH`, `_BODY_MODE`, `_MAX_TOKENS`, `_TIMEOUT` e `_API_VERSION` continuam ambiente-only:
   config persistente guarda o que identifica o recurso, não segredo nem ajuste de execução.
7. **O wizard MOSTRA o que já está gravado** (`SavedConfig`): a etapa Ambiente ganha a seção
   "já configurado" (org/projeto/dev, endpoint/deployment do Foundry, presença de PAT e de chave
   no keyring — nunca o valor). Org/projeto/dev e endpoint/deployment vêm pré-preenchidos;
   com os três valores resolvíveis o `3` avança sem abrir o formulário e diz a origem
   (`ambiente` ou `config.toml + keyring`). No modo texto, `--backend foundry` resolve pela
   mesma ordem (env > config > keyring) — o que o wizard gravou vale sem exportar nada.
8. **Simetria no `--reset`**: o reset apaga PAT e chave do Foundry do keyring e as chaves
   `foundry_endpoint`/`foundry_deployment` do `config.toml` (idempotente, ausência não é erro).
   Org, projeto e `dev_name` permanecem: são identidade do projeto, não credencial. Pelo mesmo
   motivo, um PAT já salvo é **validado e reaproveitado** (`PatResult::FromStore`) em vez de
   pedir para colar de novo o que já está guardado.

## Consequências

- Positivas: a etapa 3 do wizard deixa de ser um beco; a chave nunca aparece em arquivo, log,
  `Debug` ou tela; o `kira triage --llm foundry` funciona depois do wizard sem export manual;
  a regra de superfícies (v1 × legada) e os defaults continuam num lugar só (`FoundryConfig`).
- Negativas: mais um caminho de resolução para manter (env, arquivo e keyring) e uma entrada a
  mais no keyring para documentar/limpar (um `kira setup --reset` futuro pode removê-la).
- Riscos: máquina sem keyring (Linux CI/headless) grava só o par público — a saída diz
  `foundry parcial` e nomeia a env da chave; nunca finge que está tudo salvo.

## Testes que fixam a decisão

- `setup::wizard::tests::foundry_opens_the_inline_form_and_keeps_the_key_masked`
- `setup::wizard::tests::foundry_form_rejects_a_project_endpoint_and_empty_fields`
- `setup::wizard::tests::foundry_form_prefills_public_values_from_the_env_but_never_the_key`
- `setup::wizard::tests::esc_closes_only_the_foundry_form_not_the_step`
- `setup::wizard::tests::render_shows_the_foundry_form_and_never_the_key`
- `llm::foundry::tests::lookup_prefers_the_env_and_falls_back_to_the_saved_config`
- `setup_e2e::setup_foundry_form_is_tested_then_saved_to_config_and_keyring`
- `setup_e2e::setup_foundry_form_with_401_saves_nothing`
