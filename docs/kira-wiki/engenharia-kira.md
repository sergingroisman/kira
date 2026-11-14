# Engenharia do KIRA — o binário

Este documento descreve as decisões de engenharia que moldam o binário `kira`
único: onde os dados moram, como ele falha e como ele se protege. O público é
o dev que vai manter ou estender o código. Os invariantes que o código *prova*
(e os testes que os provam) estão em [[padroes]]; os nomes canônicos, em
[[glossario]]; a arquitetura de alto nível, em [[contexto]].

## Binário único, read-only-first

O KIRA é um executável Rust (Rust 2024, clap 4) que orquestra ferramentas —
não embala runtimes. A única dependência pesada é o
`azure_devops_rust_api` (API gerada, assíncrona), isolada por domínio em
`src/ado/` (ADR-0022: organização por domínio agora, camadas só quando houver
código compartilhado com evidência).

O default de todo comando é leitura. Escrita no Azure, git remoto ou repo do
usuário exige `--apply` explícito e, para o ciclo autônomo, `awaiting_approval`
+ `kira approve` humano. O binário toca apenas:

- `~/.config/kira/store.db` (Windows: `%USERPROFILE%\.config\kira\store.db`) —
  trilha e estado;
- `~/.config/kira/config.toml` (0600) — org, projeto, nome do dev;
- `<projeto>/.kira/` — artefatos locais do projeto (`.gitignore` próprio nasce
  com o diretório; `.kira/` é local, nunca comita);
- keyring do OS — PAT e chave Foundry (nunca em arquivo).

## Store: SQLite bundled, WAL, user_version

Um DB central por máquina (não por projeto), SQLite compilado junto ao binário
(feature `bundled` — `rusqlite`): instalação sem serviço externo, roda nativo
no Windows. Modo WAL; migração idempotente via `PRAGMA user_version` com
`CREATE TABLE IF NOT EXISTS` e guard `column_exists()` para `ALTER TABLE`.

O schema atual é **v10**. O que cada bump entregou:

| Versão | O que trouxe |
|---|---|
| v3 | índice de retrieval do repo (`repo_file`/`repo_fts` — ADR-0021) |
| v6 | tabela `posts` + view `outbox` (onda M2) |
| v7 | coluna `draft_ttl` (rename da lease) |
| v8 | estado `failed` da fila |
| v9 | `llm_calls` — contador de chamadas de LLM (P0 da auditoria) |
| v10 | `skill`/`skill_fts` — catálogo de skills (SK-1) |

Dois PRs reservando o mesmo passo de migração (`if current < 9` nos dois)
conflitam: o segundo a mergear renumera o seu passo (bump + guard + pin).
Regra da casa: quem entra depois, renumera.

Tabelas principais: `projects`, `sessions`, `facts`, `wiki_page`/`wiki_fts`,
`repo_file`/`repo_fts`, `capabilities` (estado por projeto de `board.azure`,
`scm.github`, `scm.gitlab` — ADR-0030), `posts` + `outbox`, `llm_calls`,
`skill`/`skill_fts`. Timestamps em RFC 3339 UTC; exibição local usa
`datetime(x, 'localtime')`. O id do projeto é FNV-1a 64 do root canônico
(ancestral com `.git`, senão `Cargo.toml`/`package.json`, senão diretório atual).

**Fail-open obrigatório do store**: erro do store nunca quebra o comando
principal — só `aviso: trilha não foi gravada (<causa>)` no stderr (emitido
apenas em `main.rs`) e a saída/exit code do comando intacta.

O contraponto é o `publish`: **store indisponível ali é fail-closed** — o
comando recusa (`PublishError::Store`), porque publicar sem trilha perde o
gate por hash. Degradar onde a degradação é honesta; recusar onde é seguro.

## Erros acionáveis

Taxonomia única em `src/error.rs` (`thiserror` + `?`; zero
`unwrap()`/`expect()` em entrada de usuário, rede ou parse — isso é gate de
CI em `tests/no_runtime_unwrap.rs`). Toda falha diz o que fazer:

- "defina `AZURE_DEVOPS_PAT` ou rode `kira setup`" > "credencial ausente";
- erro de API do Azure DevOps nunca ecoa body cru de HTTP;
- `--llm copilot` sem CLI: `instale o Copilot CLI + copilot auth login, ou use
  o default Ollama` — a ausência nunca vira fallback silencioso;
- erro de auth do Copilot lista os 3 passos de SAML (login, Grant na org, TI)
  nesta ordem.

## Redact: segredo nunca chega ao disco nem ao prompt

`src/redact.rs` mascara PAT/token/CPF/e-mail **antes** de log, erro, prompt,
índice e wiki. A regra é "redação na escrita": toda linha de código que entra
no índice de retrieval (ADR-0021), toda linha do `pr draft` (que pode ir para
PR público), todo corpo de skill gravado, toda saída capturada pelo dashboard
passam por `crate::redact` antes do disco. Linha de atribuição
(`token = ...`, `password = ...`) sai mascarada mesmo quando é código —
privacidade acima de utilidade. O store nunca grava segredo, nem em dado
derivado; PAT vive só em env ou keyring.

## Guard/path: jail de escrita

`src/guard/path.rs` confina toda escrita na raiz: deny-list + path traversal.
Id de página inválido (`..`, absoluto, maiúscula, slug fora de `[a-z0-9_-]`)
é recusado **antes de qualquer I/O**. Evoluções planejadas: jail mínimo do
`fix --apply` por canonicalização + rlimits em Unix (PR27, ADR-0011) e
cobertura de symlink/junction com `lstat` + validação do alvo final + ELOOP
(PR47). A proposta da onda TL (ADR-0027) adiciona o piso declarado:
regras que **nenhuma** configuração alcança — destruição fora do escopo do
repo, leitura de segredo para transcript, `push --force`, escrita na superfície
de política — acima de rails configuráveis, com veredito tipado
(`deny | ask | block-stop | follow-up | context | record`) e toda recusa
imprimindo `rule=<nome>` acionável.

## Fail-open vs fail-closed: quando cada um

| Situação | Conduta | Por quê |
|---|---|---|
| store indisponível em comando comum | fail-open (aviso, comando segue) | a trilha é auditoria, não o resultado |
| índice `repo_fts` ausente/vazio | fail-open: cai no grep determinístico, fonte marcada (`index`/`grep`) | derivado e descartável |
| FTS5 indisponível para skills/wiki | fail-open: busca por `LIKE` | índice é derivado |
| wiki/findings corrompidos no packer | fail-open: nota no pack | julgamento continua |
| `digest` sem `kira trust` | **fail-closed**: recusa | manda código ao modelo |
| store indisponível no `publish` | **fail-closed**: recusa | perder o gate por hash é pior que parar |
| pull de modelo sem consentimento | **fail-closed**: erro acionável | download é escrita no disco do dev |
| disco insuficiente no pull | **fail-closed** com o número | nunca download que estoura o disco |
| falha de quota (429) | provedor intocado por 5 min; último resultado bom fica `stale` | quota não apaga dado |
| item falho no `triage --eval` | **fail-closed**: aborta a medição | número parcial não é número |

## Determinismo: FNV-1a e byte-idêntico

Fingerprints são FNV-1a 64: do `findings.json` do `scan`
(sobre `tool|rule|package|version|message`, lista em ordem crescente), do id
de projeto (root canônico) e do prompt de triage
(`TRIAGE_SYSTEM_PROMPT_FNV1A64`, testado contra fixture byte a byte). Sem
timestamp em artefato determinístico; caminhos relativos POSIX, tudo ordenado.
Mesma árvore ⇒ mesmo `.kira/facts.md` byte a byte; mesmo estado ⇒ mesmo
context pack. O que passa por LLM é identificado por
`(model, prompt_version, input_hash)` (mais `lang` no `cache_key` do digest) e
orçado.

## Allowlist de LLM por comando

LLM é o último recurso (ADR-0016, CLI-first LLM-last): escada obrigatória de
baixo para cima — (1) CLI real (`dotnet`, `npm`/`pnpm`, `git`, `rg`, `gh`),
(2) parser puro/SQL/FTS5, (3) cálculo em Rust, (4) LLM só quando 1–3 não
respondem. Toda chamada exige pergunta de **julgamento** (não recuperação,
extração, contagem ou comparação) e o comentário `// LLM porque: <razão>` no
call site.

A política é provada por teste: `tests/llm_allowlist.rs` varre `src/` e reprova
qualquer módulo fora da lista importando `llm::*`. Importam: `src/triage.rs`,
`src/eval.rs`, `src/setup.rs` (probe do backend), `src/llm/`, `src/loop/` e
`src/post/`. `scan`, `store`, `board`, `ado`, `guard`, `format`, `tui`,
`doctor` nunca falam com modelo. Comando determinístico não consome budget.

Backends via trait `LlmBackend` (`src/llm/`), resolução `--llm` > `KIRA_LLM` >
default:

- **Ollama** (`OllamaBackend`) — localhost `http://localhost:11434`, modelo
  default `qwen3.5:4b` (~3,3 GB, Q4_K_M). O default, sempre.
- **Copilot** (`CopilotBackend`) — spawn do CLI `copilot -p` (ADR-0019),
  sem SDK, sem chave em arquivo; binário resolvido por `KIRA_COPILOT_BIN`/PATH.
- **Foundry** (`FoundryBackend`) — BYOK Microsoft Foundry via HTTP
  OpenAI-compatible (ADR-0020); fronteira de dados: dado de cliente/Avanade
  NUNCA entra no recurso de learning do dono — cliente usa Ollama local ou
  endpoint aprovado pelo cliente.

O caro nunca é default nem fallback silencioso em nenhuma direção.
Hugging Face entra como catálogo de GGUF via Ollama (`kira model list`/
`kira model pull`), nunca runtime (ADR-0040) — ver [[decisoes]].

## Budget de chamadas: `llm_calls` (v9, P0)

O budget mede **chamadas de LLM, não trabalho e não dinheiro**:
`max_llm_calls_per_day = 50` (default) em `config.toml`, checado antes de cada
chamada e de cada transição de estado. O contador `llm_calls` (schema v9) grava
`(backend, model, command, tokens_est, at)` por chamada — de qualquer backend,
de qualquer modelo: especialista novo não escapa do teto. Baixar modelo
(`kira model pull`) não consome o contador: pull é trabalho, não inferência.
O corte diário é por data local (zero-padded). Estourou: erro acionável
com o número do dia — retome amanhã ou ajuste o teto.
A política de quota/stale (429 → 5 min de silêncio) e o gauge do dashboard
ainda são PR17; a medição existe desde o P0.

## Orçamento do digest

Duas chaves, do ADR-0038: **1 M tokens por run** de `digest` (é leitura
inicial de projeto grande) e **teto de 30 k por post** — sem o segundo, um
post guloso come o run. Estourou: parada dura **com nota**, nunca com erro.
A fila (`posts`) retoma com `--resume`; `--pause` para depois de N posts.

## Credenciais: env > keyring, nunca arquivo pleno

- Segredo vive em env (`AZURE_DEVOPS_PAT`) **ou** no keyring do OS
  (service `kira`, gravado por `kira setup`/`kira connect`) — `src/credentials.rs`
  concentra `CredStore` (`OsKeyring` no runtime, `InMemory` só em testes; os
  testes nunca tocam o keyring real).
- Prioridade de configuração: flag > env > `$HOME/.config/kira/config.toml`
  (0600, escrito pelo setup). Org/projeto podem vir do arquivo; credencial,
  não.
- O valor nunca aparece em log, erro, `Debug` ou trilha; o `doctor` e o
  cabeçalho do dashboard mostram o PAT mascarado (ex.: `Aa2x****`).
- A trilha no store não grava segredo nem description completa de work item —
  o summary guarda no máximo os primeiros 2000 chars do output.

## Capacidades: conexão é estado, não pressuposição

ADR-0030: integração é uma `Capability` (`board.azure`, `scm.github`,
`scm.gitlab`) com estado por projeto no store (`desconectada | conectada |
inválida`). O comando que exige capacidade falha com mensagem de capacidade —
`esta jornada precisa de board.azure — rode: kira connect azure` — nunca com
401/404 cru, sem stack trace e sem tentativa de rede. **Nada é conectado
implicitamente**: presença de PAT na env não é consentimento. As jornadas
locais (`scan`, wiki, context, pr draft, dashboard) rodam com zero
credencial — a matriz de capacidades é contrato de teste
(`tests/capabilities_e2e.rs`).

## Ferramentas resolvidas no PATH, nunca classificadas por texto

O binário spawna ferramentas externas (`dotnet`, `npm`/`pnpm`, `copilot`,
`ollama`) sempre pelo caminho resolvido `which`-style (`src/binresolve.rs`):
Windows é `PATH × PATHEXT` (case-insensitive como o NTFS), Unix é bit de
execução. Ausente ⇒ "instale o X" **sem spawn** — nenhuma mensagem localizada
do OS é interpretada (o cmd é localizado e pode chegar em mojibake).
`.cmd`/`.bat` no Windows passam por wrapper `cmd /C` com `chcp 65001` (mantém
o stderr da ferramenta legível). Override por env (`KIRA_SCAN_*_BIN`,
`KIRA_COPILOT_BIN`, `KIRA_MODEL_OLLAMA_BIN`): valor presente é contrato do
usuário — inexistente cai no mesmo "instale o X". Ferramenta ausente e
ferramenta que falhou são erros distintos.

## O doctor como espelho da verdade

`kira doctor` reporta: de onde veio cada valor (env, keyring ou
`config.toml`), teste de conexão e do nome do projeto (PAT mascarado), a linha
`capacidades: nenhuma conectada (modo local)` ou a lista das conectadas, o
estado da fila do ciclo (ingestão, trust, posts desatualizados quando o repo
mudou — o `publish` por hash torna desatualização **detectável** e o doctor a
lista) e, com o contador v9, as chamadas de LLM do dia. Não ensina PAT: a
credencial é assunto de `kira connect`. Com 429 acumulado, mostra a idade do
último sucesso (o dado continua visível, marcado `stale`).

## Variáveis de ambiente que você vai encontrar primeiro

| Variável | Papel |
|---|---|
| `AZURE_DEVOPS_PAT` | PAT (env vence keyring e `config.toml`) |
| `AZURE_DEVOPS_ORG` / `AZURE_DEVOPS_PROJECT` | contexto do board |
| `KIRA_MODEL` | tag do modelo do triage (default `qwen3.5:4b`) |
| `KIRA_LLM` | backend (`ollama` \| `copilot` \| `foundry` — `--llm` vence) |
| `KIRA_OLLAMA_URL` | URL do Ollama (default `http://localhost:11434`) |
| `KIRA_NUM_CTX` | janela de contexto (default 4096) |
| `KIRA_STORE_DIR` | sobrescreve o diretório do store (testes/portáveis) |
| `KIRA_INDEX_MAX_FILES` | teto do índice de retrieval (default 5000) |
| `KIRA_MAX_LLM_CALLS_PER_DAY` | teto diário de chamadas de LLM |
| `KIRA_FOUNDRY_*` | endpoint, deployment, chave e tuning do backend BYOK |

Envs novas só entram com justificativa no PR — regra do AGENTS.md.

## Comandos do binário

`inspect`, `doctor`, `triage`, `setup`, `connect`, `scan`, `ingest`, `trust`,
`digest`, `publish`, `skills`, `model`, `context`, `dashboard`, `loop`,
`history`, `note`, `profile`, `wiki`, `pr draft`. Cada um com o contrato
detalhado no README do repo; a jornada de uso está em [[getting-started]].
