# KIRA

KIRA — Knowledge-Informed Remediation Agent (*Keep It Running, Always*).

CLI em Rust para manutenção assistida de repositórios hospedados no Azure DevOps.
**Read-only por padrão; escrita só com `--apply` no futuro** — e só depois de
aprovação humana explícita. Hoje nenhum comando escreve no Azure DevOps nem no
seu código: o único disco que o KIRA toca é o dele (`~/.config/kira/`) e a
pasta local do projeto (`.kira/`).

## Estado real do binário (M1 + ondas W2/W2-fix)

O que existe de verdade, verificado em `src/main.rs`:

| Comando | O que faz |
| --- | --- |
| `kira inspect <id> [--project <nome>]` | Resumo legível de um work item do Azure DevOps (requer PAT). |
| `kira doctor` | Diagnóstico da configuração da máquina: de onde veio cada valor (env, keyring ou `config.toml`), testa a conexão e o nome do projeto (PAT sai mascarado). |
| `kira triage <id>` | Triage **vivo**: busca o work item no Azure e pede causa raiz ao **Ollama local** (requer PAT + Ollama). Grava um episódio em `.kira/wiki/bugs/`. |
| `kira triage --file <json>` | Triage **offline** do mesmo JSON de work item — sem PAT, sem Azure, 100% local. |
| `kira triage <id\|--file> --model <tag>` | Troca o modelo do Ollama para esta execução. |
| `kira triage <id\|--file> --llm copilot` | Usa o **Copilot CLI** como LLM caro desta execução (spawn de `copilot -p`; exige `copilot auth login`) — ver [Backend LLM (ADR-0019)](#backend-llm-adr-0019). |
| `kira triage <id\|--file> --llm foundry` | Usa o **Microsoft Foundry** (BYOK, HTTP OpenAI-compatible) no lugar do Ollama para esta execução — ver [Backend foundry](#backend-foundry-byok-microsoft-foundry) abaixo. |
| `kira triage --eval <dir>` | **Gate M2 medido (ADR-0018)**: compara o triage com os rótulos do dono (`item.json` + `gold.json` por pasta) de forma **determinística, sem LLM juiz**; exit 0 = ≥70% (`PASSOU`), 1 = <70% ou erro. `--json` para automação; `--seed` copia 3 fixtures sintéticas num diretório vazio. Ver [seção do gate](#kira-triage---eval--o-gate-m2-medido-adr-0018). |
| `kira setup [--org --project --name --pat --no-browser --reset]` | Wizard: org, projeto, nome do dev (`--name`) e PAT validados **antes** de salvar; PAT vai para o keyring do OS (service `kira`). `--reset` remove o PAT. |
| `kira scan [--path <dir>]` | Scan **determinístico** das ferramentas padrão da stack (`dotnet list package --vulnerable --include-transitive --format json`; `pnpm audit --json` / `npm audit --json`). Sem IA. Grava `<root>/.kira/findings.json` com fingerprint FNV-1a e ordem estável. |
| `kira history [--last N]` | Sessões recentes do projeto atual (N = 10 por padrão). |
| `kira note "<texto>"` / `kira note --remove N` | Fato do perfil do projeto (append-only, numerado). |
| `kira profile` | Root, nome, primeira vez visto e todos os fatos do projeto. |
| `kira wiki list\|show\|decision\|reindex\|search\|forget\|promote\|export` | Wiki local do projeto (seção Wiki abaixo). |
| `kira context --file <json> [--explain] [--json]` | Mostra o **context pack** que iria ao prompt do `kira triage` — sem LLM, sem rede: fonte do repositório (`index` FTS5 ou `grep`), seções da wiki, cada trecho como `path:linha — trecho` e o resumo `contexto: N trechos — X/Y chars (~Z tokens, 3 chars/token)`. `--explain` mostra também por que cada item entrou (rank bm25, prioridade de finding); `--json` devolve a estrutura serializável. O id ao vivo (`kira context <id>`) é recusado: este comando não toca na rede. |

O que **não existe ainda** vai no [Roadmap](#roadmap) — nada é prometido como
pronto se não está no binário.

## Requisitos

| Componente | Para quê | De onde |
| --- | --- | --- |
| Rust (via rustup) | build do binário | [rustup.rs](https://rustup.rs) |
| Ollama | **default do `kira triage`** (LLM local, 100% gratuito) | `winget install Ollama.Ollama` ou [ollama.com/download](https://ollama.com/download) |
| Copilot CLI | **opcional**: backend caro do triage (`--llm copilot`) | [github.com/github/copilot-cli](https://github.com/github/copilot-cli) + `copilot auth login` |
| .NET SDK 8+ | `kira scan` em projetos .NET | [dotnet.microsoft.com/download](https://dotnet.microsoft.com/download) |
| npm ou pnpm | `kira scan` em projetos Node/TS | [nodejs.org/download](https://nodejs.org/download) / `npm install -g pnpm` |

Modelo default do triage: `qwen3.5:4b` (~3,3 GB, Q4_K_M):

```powershell
ollama pull qwen3.5:4b
```

Trocável por env (`KIRA_MODEL`) ou por execução (`--model`). O triage roda
100% na sua máquina: nada vai para nuvem. Para testar sem PAT:

```powershell
kira triage --file tests\fixtures\work_item_4821.json
```

## `kira triage --eval` — o gate M2 medido (ADR-0018)

O gate do M2 é um **número**, não opinião: `kira triage --eval <dir>` mede o
acerto do triage contra rótulos verdadeiros do dono (os 20 bugs do
`triage_gold`). Cada item do diretório é uma **pasta** com 2 arquivos:

```
<pasta-do-bug>/
  item.json  → JSON do work item do Azure DevOps (o mesmo do `kira triage --file`)
  gold.json  → rótulo do dono:
    {
      "causa_raiz": "TTL do token de sessão expira antes do refresh",
      "arquivos": ["src/Api/Auth/TokenService.cs"],
      "confianca": "alta"          // alta|media|baixa (opcional)
    }
```

O triage roda **offline** (`--file`) contra o backend escolhido
(`--llm ollama` por default; `--llm copilot|foundry` respeitando as regras
dos ADR-0019/0020). A comparação com o gold é **determinística e auditável —
sem LLM como juiz** (um LLM julgando um produto de LLM não é gate):

- **causa raiz**: similaridade de tokens de conteúdo (minúsculas, sem acento,
  sem stopwords PT, palavras ≥ 4 caracteres; `score = |interseção| / |gold|`).
  Acerto = **score ≥ 0.50**.
- **arquivos**: pelo menos **1 basename** do gold presente na previsão
  (comparação sem caixa).

Saída: tabela por item (`id | causa % | arquivos ok? | veredito`) e a linha
final `GATE M2: <acertos>/<total> = <pct>% (limiar 70%) — PASSOU|NÃO PASSOU`.
`--json` devolve o mesmo relatório em JSON (com os textos previstos e gold)
para automação.

- **Exit code**: `0` = ≥ 70% (PASSOU) · `1` = < 70% (NÃO PASSOU) ou erro — o
  gate é o exit code, pronto para CI.
- Qualquer item que falhe **aborta a medição**: número parcial não é número.
- `--seed`: copia 3 fixtures **sintéticas** (`seed-sintetico-01..03`,
  embutidas no binário) num diretório **vazio** e roda a medição sobre elas —
  para testar o formato antes de rotular os 20 reais. As seeds se declaram
  sintéticas no próprio `gold.json` (`nota`) e nunca substituem rótulo do dono.
- Diretório vazio sem `--seed` ⇒ erro acionável com o schema acima.

```bash
kira triage --eval tests/fixtures/triage_gold --llm ollama
kira triage --eval ./gold --llm copilot --json      # automação
kira triage --eval ./gold --seed                    # primeira vez, 3 fixtures sintéticas
```

## Instalação

```bash
cargo install --path .     # ou: cargo run -- <comando> direto do repo
```

## Setup (PAT do Azure DevOps)

O PAT precisa dos escopos **Work Items (Read)** + **Project and Team (Read)**.
Ele vive no keyring do OS (salvo por `kira setup`) ou na env
`AZURE_DEVOPS_PAT` (CI/headless — vence keyring e `config.toml`).

```bash
kira setup          # interativo: org, projeto, pergunta "Você já tem um PAT pronto? [S/n]"
kira setup --org minha-org --project MeuProjeto --pat <token>   # sem perguntas
kira setup --name Sergin   # como o KIRA te chama (vai para config.toml, nunca para um LLM)
kira setup --reset  # remove o PAT do keyring
```

Fluxo: a resposta **S** cola o token direto; **N** imprime a página do token e
os escopos e só abre o navegador **com confirmação** (nunca bloqueia o
terminal). O KIRA valida a credencial (lista projetos, confere o nome) antes
de salvar — 401 salva nada. Org/projeto ficam em
`~/.config/kira/config.toml` (0600).

Verificação:

```bash
kira doctor
kira inspect 4821
```

## Variáveis de ambiente

| Variável | Descrição | Default |
| --- | --- | --- |
| `AZURE_DEVOPS_PAT` | PAT com Work Items (Read) (env vence keyring e `config.toml`) | — |
| `AZURE_DEVOPS_ORG` | Organização do Azure DevOps | — |
| `AZURE_DEVOPS_PROJECT` | Projeto padrão (o `--project` vence) | — |
| `KIRA_SCAN_DOTNET_BIN` | Caminho completo que substitui o `dotnet` do scan | PATH |
| `KIRA_SCAN_NPM_BIN` | Caminho completo que substitui o `npm` do scan | PATH |
| `KIRA_SCAN_PNPM_BIN` | Caminho completo que substitui o `pnpm` do scan | PATH |
| `KIRA_OLLAMA_URL` | URL do servidor Ollama | `http://localhost:11434` |
| `KIRA_LLM` | Backend do triage: `ollama` (default) \| `copilot` \| `foundry` — o `--llm` vence | `ollama` |
| `KIRA_COPILOT_BIN` | Caminho completo que substitui o `copilot` do backend caro (padrão dos testes offline) | PATH |
| `KIRA_COPILOT_TIMEOUT` | Limite por chamada do CLI do Copilot, em segundos | `180` |
| `KIRA_MODEL` | Tag do modelo do triage | `qwen3.5:4b` |
| `KIRA_NUM_CTX` | Janela de contexto do modelo | `4096` |
| `KIRA_STORE_DIR` | Sobrescreve o diretório do store (testes/execuções portáveis) | `~/.config/kira` |
| `KIRA_INDEX_MAX_FILES` | Teto de arquivos do índice de retrieval do repositório (ADR-0021, substitui o corte de 400 do grep). Valor inválido/zero cai no default. | `5000` |
| `KIRA_BOARD` | Provedor de board (`azure` é o único nesta fase) | `azure` |
| `KIRA_CONFIG` | Caminho alternativo do `config.toml` | `~/.config/kira/config.toml` |
| `KIRA_FOUNDRY_ENDPOINT` | Endpoint do recurso Foundry (a superfície v1 termina em `/openai/v1`) | — |
| `KIRA_FOUNDRY_DEPLOYMENT` | Nome do deployment/modelo (`gpt-5` etc.) | — |
| `KIRA_FOUNDRY_API_KEY` | Chave do recurso (nunca vai para log/erro) | — |
| `KIRA_FOUNDRY_URL` | URL verbatim da requisição (gateways OpenAI-compatible/testes) | — |
| `KIRA_FOUNDRY_API_VERSION` | `api-version` da superfície legada (a v1 ignora) | `2024-10-21` |
| `KIRA_FOUNDRY_AUTH` | Modo de auth: `api-key` (Azure) \| `bearer` (gateways) | `api-key` |
| `KIRA_FOUNDRY_BODY_MODE` | Formato do corpo: `auto` \| `classic` \| `reasoning` | `auto` |
| `KIRA_FOUNDRY_MAX_TOKENS` | `max_tokens`/`max_completion_tokens` da resposta | `1024` |
| `KIRA_FOUNDRY_TIMEOUT` | Timeout da chamada, em segundos | `120` |

## Backend foundry (BYOK Microsoft Foundry)

O Ollama local é o default e nunca sai de cena; o Foundry é o caminho "caro"
**opt-in**: `kira triage <id|--file> --llm foundry` (ou `KIRA_LLM=foundry`).
Sem as envs, o comando falha com erro acionável — sem fallback silencioso.

**Dados de cliente/Avanade NUNCA entram neste recurso de learning do
Microsoft Foundry.** Trabalho de cliente: Ollama local ou endpoint
aprovado pelo cliente. Este recurso é para estudo/roadmap do dono.

**Duas superfícies**, escolhidas pelo formato de `KIRA_FOUNDRY_ENDPOINT`:

- **v1** (endpoint termina em `/v1`): `POST {endpoint}/chat/completions`, sem
  `/openai/deployments/...` e sem `?api-version`; o deployment vai no campo
  `model` do corpo. Exemplo real do dono:
  `KIRA_FOUNDRY_ENDPOINT=https://sdossantosjunior-5474-resource.openai.azure.com/openai/v1`
  + `KIRA_FOUNDRY_DEPLOYMENT=gpt-5`. Endpoint de projeto
  (`*.services.ai.azure.com/api/projects/...`) não serve chat completions —
  o KIRA recusa com erro acionável apontando o `/openai/v1`.
- **legada**: qualquer outro endpoint vira
  `{endpoint}/openai/deployments/{deployment}/chat/completions?api-version=
  {KIRA_FOUNDRY_API_VERSION}` (default `2024-10-21`).

**Auth**: `api-key: <chave>` por default (padrão Azure);
`KIRA_FOUNDRY_AUTH=bearer` troca para `Authorization: Bearer` (gateways
OpenAI-compatible). `KIRA_FOUNDRY_URL`, se definida, é usada verbatim.

**Corpo por família de modelo** (`KIRA_FOUNDRY_BODY_MODE`, default `auto`):
GPT-5 e o-series (`gpt-5*`, `o1*`, `o3*`, `o4*`) rejeitam `temperature` e
exigem `max_completion_tokens` — o modo `auto` detecta pelo nome do
deployment e envia só `max_completion_tokens`; o resto recebe
`temperature: 0.2` + `max_tokens`. `classic` e `reasoning` forçam o formato.
Se o servidor devolver 400 `unsupported_parameter`, o erro sugere
`KIRA_FOUNDRY_BODY_MODE`.

**Erros**: 401/403 ⇒ `FoundryAuth` (como gerar nova chave); 404 ⇒
`FoundryDeployment` (onde conferir endpoint/deployment); 400/404 citando
`api-version` ⇒ `FoundryApiVersion`; 429 ⇒ `FoundryRateLimited` com o
`retry-after` do servidor (a guardrails v2 cobra cota por período); demais
status/shape ⇒ `FoundryFailed` (1ª linha de `error.message`, ≤200 chars,
mascarada — nunca body cru). Timeout por `KIRA_FOUNDRY_TIMEOUT` (default
120s). A chave nunca aparece em log, erro ou trilha.

## Backend LLM (ADR-0019)

O **Ollama local é o default** — sempre: `kira triage` roda em
`http://localhost:11434` com `qwen3.5:4b`, 100% na sua máquina, sem custo.
O backend HTTP BYOK (Foundry) tem a sua própria documentação em
[Backend foundry (BYOK Microsoft Foundry)](#backend-foundry-byok-microsoft-foundry)
(ADR-0020).

O backend **caro** (Copilot) só existe sob escolha explícita — nunca é
implícito e nunca cai em fallback silencioso para o Ollama:

```powershell
kira triage 4821 --llm copilot        # flag vence
KIRA_LLM=copilot kira triage 4821    # equivalente por env
```

Como funciona: o KIRA compõe `system + prompt` (o prompt já vem redigido
pela camada de `redact` — PAT/env nunca vão ao modelo) num único texto e
**spawn do CLI**: `copilot -p "<texto>"`. Nenhum SDK, nenhuma chave em
arquivo. O binário é resolvido de `KIRA_COPILOT_BIN` ou do PATH; o limite
por chamada é `KIRA_COPILOT_TIMEOUT` (default 180s).

### Troubleshooting de auth (SAML/SSO) — os 3 passos

Quando o CLI nega o acesso (comum com a conta GitHub corporativa), a
mensagem de erro lista exatamente estes passos, nesta ordem:

1. **Rode `copilot auth login`** e refaça o login com a conta da org
   Avanade.
2. **Autorize o app do Copilot para a organização** em
   [github.com/settings/connections/applications](https://github.com/settings/connections/applications)
   — botão **Grant**/**Authorize** na org Avanade.
3. **Se o admin bloquear apps OAuth**, peça a aprovação à TI. Enquanto
   isso, use `--llm ollama` (o default local continua funcionando).

> **Pré-requisito de policy (Avanade):** o uso do Copilot CLI pelo KIRA é
> decisão corporativa, não técnica. Confirme com a policy da empresa antes
> de operar este backend em código corporativo (ADR-0019).

Se o binário não existe (`--llm copilot` sem CLI instalado), o comando
falha com `instale o Copilot CLI` — a ausência nunca se torna Ollama sem
que você perceba.

## Store local

Toda execução deixa trilha em um único SQLite — sem servidor, sem conta,
offline.

| Plataforma | Caminho |
| --- | --- |
| Windows | `%USERPROFILE%\.config\kira\store.db` |
| Linux / macOS | `~/.config/kira/store.db` |

O mesmo diretório guarda `config.toml`. Tabelas: `projects`, `sessions`,
`facts` (e índice derivado da wiki, `wiki_page`/`wiki_fts`). O id do projeto é
FNV-1a 64 do root canônico (primeiro ancestral com `.git`, senão o mais
próximo com `Cargo.toml`/`package.json`, senão o diretório atual).

- **Fail-open**: store quebrado nunca quebra o comando — só um
  `aviso: trilha não foi gravada (<causa>)` no stderr, com a mesma saída e exit
  code.
- **Privacidade**: PAT/tokens/env nunca são gravados; o summary guarda no
  máximo os primeiros 2000 chars do output (o `doctor` mascara o PAT);
  `kira note` grava exatamente o que você digitou.
- Apagar `store.db` limpa toda a trilha.

## Dashboard — `kira dashboard` (W6-PR4, read-only)

O dashboard é a mesma TUI do binário (ratatui/crossterm, ADR-0014) mostrando o
estado vivo do projeto atual — nesta PR, o **painel de cabeçalho** (nome do
projeto, root, id FNV, `dev:` da chave `dev_name` do `config.toml`, backend LLM
default (`ollama`), PAT mascarado (`Aa2x****`), idade do último `kira scan` pela
trilha), o **painel de achados** (`.kira/findings.json`): tabela ordenada
critical→low com contagem por severidade no cabeçalho do bloco
(`critical N · high N · medium N · low N`), colunas SEVERIDADE / FERRAMENTA /
LOCAL (`path:linha`) / RESUMO (1ª linha da mensagem) e agregação por ferramenta
— o **painel de sessões** (trilha do store): as últimas 50 sessões do
projeto (`#id`), com quando (idade relativa — `há 3 min` — calculada por
função pura sobre o `now` capturado uma vez no snapshot, sem relógio por
frame), comando, modelo, duração (`412ms`, `1m5s`, `1h05m`) e a 1ª linha do
summary; sessão que falhou fica destacada, sucesso fica neutro. Store legível
sem sessões mostra `sessões: — (store sem sessões)` — e o **painel de wiki**
(W6-PR4, abaixo). `Enter` abre o detalhe — do achado ou da sessão selecionada
(painel lateral com o summary INTEIRO, ≤2000 chars já gravados pela store); no
painel de wiki `Enter` abre a página em tela cheia; `Esc` volta em todos.

### Painel de wiki (W6-PR4) — "ler um blog dentro do terminal"

O coração do MVP para o dono: as páginas da wiki (ADR-0013) listadas por
`tipo · título · atualização relativa` (idade por função pura sobre o `now` do
snapshot), lidas do índice `wiki_page` da store em modo `SQLITE_OPEN_READ_ONLY`
com o markdown REAL de cada arquivo em `.kira/wiki/` (limite de 256 KiB por
página — a fonte de verdade é o disco, o índice é derivado).

- **Markdown rendido por renderizador próprio** (`src/tui/markdown.rs`), SEM
  dependência nova: títulos com marcador por nível (`#`→`▌`, `##`→`■`), listas
  `-`/`*`→`•`, bloco de código e citação com barra `│` (sem cor de sintaxe),
  code span desligado, negrito/itálico desligando os marcadores, links
  `[txt](url)`→`txt (url)`, régua `---`→`────`. Nada de HTML executado (tags
  viram texto), quebra de linha na largura real, determinístico.
- **Busca incremental `/`**: filtra por título E conteúdo, case-insensitive e
  sem acento-sensível (digitar `sessao` acha `Sessão`) — função pura
  (`filter_pages`) unit-testada; o rodapé vira a caixa de texto com cursor e
  `Esc` cancela.
- **`Enter`** abre a página selecionada em tela cheia com scroll vertical
  (`j`/`k`, `g`/`G`, `PgUp`/`PgDn`); **`Esc`** volta à lista.
- Índice ausente/vazio ⇒ lista vazia com a causa honesta:
  `wiki: — (wiki vazia — rode kira wiki reindex)`.

- **Read-only provado**: o dashboard não escreve em nada — a store abre em modo
  `SQLITE_OPEN_READ_ONLY`, não toca rede nem LLM, e NÃO grava sessão na trilha.
  O e2e (`tests/dashboard_e2e.rs`) prova: hash do `store.db` antes/depois de
  rodar com e sem `--snapshot` é byte-idêntico — inclusive com o painel de
  wiki lendo `.kira/wiki/`.
- **Honestidade**: dado ausente aparece como `—` com a causa
  (`achados: — (nunca rodou kira scan)`), nunca inventado ou estimado.
- **Interativo só em TTY**: `kira dashboard` com stdout de terminal abre a TUI;
  sem TTY (pipe/CI) imprime o mesmo conteúdo em texto e sai 0.
- **`--snapshot`**: sempre a forma de texto, para o CI e para redirecionamento —
  nunca desenha na tela (inclui a lista de páginas e o markdown rendido).
- **Teclas** (setas E vim; nenhuma tecla escreve): `q` sair ·
  `Tab`/`Shift+Tab` ciclar painel (cabeçalho → achados → sessões → wiki) ·
  `j`/`k` ou `↑`/`↓` mover · `g`/`G` topo/fim · `Enter` detalhe (achado ou
  sessão) ou página da wiki · `Esc` volta · `/` busca na wiki ·
  `r` reler o disco (sem rede, sem escrita) · `?` ajuda.
- **Terminal pequeno**: abaixo de 80×24 o layout degrada para lista única com
  aviso claro.
- **Cores**: `NO_COLOR` (ou `TERM=dumb`) ⇒ paleta mono com a mesma informação.

Atribuição: a casca é inspirada no statui (MIT, Mohamed Badry) — nenhum trecho
foi copiado nesta PR; ver `third_party/statui/NOTICE.md`.

## PR draft — `kira pr draft` (W4, local)

Gera o **rascunho de PR em markdown** a partir do que o KIRA já sabe — SEM
rede, SEM LLM, SEM push e SEM abrir PR. Fontes: último `kira triage ITEM_ID`
na trilha do store (causa raiz, arquivos, confiança), achados do
`.kira/findings.json` cujo path aparece entre os arquivos citados pelo triage,
até 3 páginas de wiki relacionadas por termo comum e, opcionalmente, o item do
board via `--file caminho.json` (JSON salvo localmente).

```text
kira pr draft 4821                       # rascunho no stdout
kira pr draft 4821 --file item.json      # usa o item do board do JSON local (título/contexto)
kira pr draft 4821 --out draft.md        # escreve o arquivo (caminho explícito; nunca escolhe sozinho)
kira pr draft 4821 --json                # estrutura parseável (item_id, seções, markdown)
```

Exemplo de saída (com uma seção sem dado — o placeholder `—` sempre vem com a
causa, nunca dado inventado):

```markdown
# Sessão expirada com token válido

## Contexto
Item #4821 do tipo Bug no estado Active.

## Causa raiz provável
TTL do token muito curto.

Confiança: média

## Mudança proposta
A partir dos arquivos citados pelo último triage (nenhum arquivo novo é sugerido):
- revisar `src/auth/session.rs` (citado pelo triage)

## Evidências
src/auth/session.rs — achado audit/high: TTL fixo sem configurar
.kira/wiki/bugs/4821-sessao-expirada.md — trecho: "TTL do token muito curto causa relogin frequente..."

## Riscos
— (não há achado de scan relacionado aos arquivos citados; rode kira scan antes)

## Checklist
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -D warnings`, `cargo test` e `cargo build` verdes
- [ ] revisão humana antes do merge — o merge é sempre humano
- [ ] diff sem segredos (nenhum PAT/token/e-mail no diff)
```

Regras do comando (testadas em `src/prdraft.rs` e `tests/prdraft_e2e.rs`):

- **Anti-invenção**: evidência só entra se veio de dado real (sessão, finding
  ou wiki). Seção sem dado sai como `— (não há dado; rode kira triage <id>
  antes)` — sem arquivo, linha, causa ou métrica inventados. `path:linha` só
  quando o achado traz linha.
- **Redação obrigatória**: o texto inteiro passa por `crate::redact` antes de
  sair — o rascunho pode ir para PR público.
- **Determinismo**: template fixo, sem `now()` — a data vem do dado gravado;
  mesma entrada = mesma saída byte a byte.
- Item sem triage sai com `—` + causa e **exit 0** (falta de dado não é erro).
- O comando é read-only: não grava sessão na trilha, não toca rede nem LLM.

## Wiki — `.kira/wiki/`

O KIRA mantém conhecimento reutilizável (não log de conversa) em markdown
local, dentro do projeto: `<root>/.kira/wiki/{contexto,decisoes,padroes,glossario}.md`
e `<root>/.kira/wiki/bugs/<slug>.md` (ADR-0013).

- `kira triage` anexa um **episódio** em `bugs/` (causa raiz prevista, arquivos,
  confiança, brief do scan) sem perguntar nada. Episódio vazio não grava;
  repetido não duplica; com mais de 30 dias colapsa em `## Histórico colapsado`.
- `kira wiki reindex` reconstrói o índice FTS5 (derivado e descartável — o
  markdown é a fonte de verdade); `kira wiki search` consulta esse índice.
- `kira wiki forget <página>` apaga a página e o índice (LGPD — dado do dono).
- `kira wiki promote` promove páginas do staging; o ciclo autônomo (futuro)
  grava em `.kira/wiki-staging/`, nunca direto na wiki.
- `kira wiki export` imprime/salva a wiki inteira como um bundle `.md`.
- Toda escrita passa por `redact` (segredo não vai para o disco) e pela guarda
  de caminho (confinamento na raiz).

## Retrieval do repositório — índice FTS5 (ADR-0021)

Além da wiki, o `kira triage`/`kira context` busca no **código do repositório**
pelo índice FTS5 do store (`repo_file` + `repo_fts`, schema v3): uma linha de
índice por linha de código **já redigida** (`redact` na escrita — o store nunca
guarda segredo), ranking bm25, bônus para os arquivos citados em
`findings.json` e teto de **5000 arquivos** (`KIRA_INDEX_MAX_FILES`) no lugar
do corte alfabético de 400 do grep antigo.

- **Incremental**: só reprocessa arquivo cujo `(mtime, size)` mudou; arquivo
  que saiu do disco sai do índice; a cada pack (`kira triage`/`kira context`)
  o índice é refrescado antes da busca.
- **Fail-open**: índice ausente/vazio ⇒ o packer cai no grep determinístico de
  sempre e marca a fonte (`index`/`grep`) no pack.
- O índice é **derivado e descartável** — a fonte de verdade são os arquivos;
  `kira context --explain` mostra o pack que iria ao prompt e `--json` dá a
  estrutura para medir recall.

## Windows

A ferramenta é **resolvida no PATH antes de qualquer spawn** (which-style):

- **Procura**: cada diretório do `PATH` × `PATHEXT` (`.COM/.EXE/.BAT/.CMD`,
  comparação case-insensitive como no NTFS) no Windows; arquivo com bit de
  execução no Linux/macOS. O caminho devolvido é o **nome real no disco**.
- **Ausente ⇒ erro acionável sem spawn**: `o scan Node/TS via npm exige o npm —
  instale em https://nodejs.org/download e rode de novo`. Nenhuma mensagem
  localizada do sistema operacional é interpretada — funciona em qualquer
  locale/codepage (o caso clássico era `'npm' não é reconhecido` em pt-BR virar
  "ferramenta falhou").
- **Presente**: `.cmd`/`.bat` spawnam via wrapper `cmd /C` (o `chcp 65001` no
  wrapper mantém o stderr da ferramenta legível); `.exe` e binários Unix
  spawnam direto, pelo caminho completo.
- **Override `KIRA_SCAN_*_BIN`**: o arquivo apontado é verificado; inexistente
  cai no mesmo "instale o X". `.cmd`/`.bat` rodam via `cmd /C`.
- Ferramenta ausente × ferramenta que falhou são erros **distintos** — "instale
  o X" ou `a ferramenta <tool> falhou (exit <N>)` com no máximo a primeira
  linha do stderr (≤200 chars) como dica; nunca body cru. `npm/pnpm audit` sai
  com exit 1 exatamente quando há vulnerabilidades — isso é caso normal (o JSON
  é parseado).

## Scanning (determinístico)

`kira scan` orquestra e normaliza; **não analisa, não usa IA**. Detecção de
stack no diretório (`--path`, até 2 níveis): `.NET` = `*.csproj`, `*.sln` ou
`global.json`; Node/TS = `package.json` (pnpm com `pnpm-lock.yaml`, npm com
`package-lock.json` ou sem lock). Nenhum marcador ⇒ erro acionável.

Saída: `<root>/.kira/findings.json` —
`{ tool, rule, package, version, severity, path, message, fingerprint }`.
`fingerprint` = FNV-1a 64 sobre `tool|rule|package|version|message` (trimado),
lista ordenada crescente por fingerprint: duas execuções no mesmo estado
produzem o mesmo arquivo byte a byte (gate de reprodutibilidade do M1).
**`.kira/` é local — nunca comite** (adicione ao `.gitignore` do projeto).

## Desenvolvimento

```bash
cargo build
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test                                  # unit + e2e com shims, sem rede
cargo test -- --ignored                     # integração real (precisa das envs do Azure)
```

O CI (GitHub Actions) roda `fmt --check` + `clippy` + `test` em
ubuntu/macos/windows. O contrato do repositório está em
[AGENTS.md](AGENTS.md); as decisões de arquitetura em `docs/adr/`.

## Roadmap

O que ainda **não** existe no binário (fonte de verdade:
[`docs/roadmap-prs.md`](docs/roadmap-prs.md)):

- **`kira triage --eval <dir>`** + `tests/fixtures/triage_gold/` — 20 bugs
  fechados rotulados pelo dono; gate do M2 é **≥70% medido** (W2-bis).
- **Autonomia: `kira loop` / modo agendado** com estado `awaiting_approval` no
  store e `kira approve <id>` (W5) — escrita só depois do gate M2 e do
  `--apply` explícito.
- **TUI Ratatui** (W6) e `kira fix --apply` (M3) — só depois do M2.

## License

MIT — see [LICENSE](LICENSE).
