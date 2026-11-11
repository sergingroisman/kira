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
| `kira setup [--org --project --pat --no-browser --reset]` | Wizard: org, projeto e PAT validados **antes** de salvar; PAT vai para o keyring do OS (service `kira`). `--reset` remove o PAT. |
| `kira scan [--path <dir>]` | Scan **determinístico** das ferramentas padrão da stack (`dotnet list package --vulnerable --include-transitive --format json`; `pnpm audit --json` / `npm audit --json`). Sem IA. Grava `<root>/.kira/findings.json` com fingerprint FNV-1a e ordem estável. |
| `kira history [--last N]` | Sessões recentes do projeto atual (N = 10 por padrão). |
| `kira note "<texto>"` / `kira note --remove N` | Fato do perfil do projeto (append-only, numerado). |
| `kira profile` | Root, nome, primeira vez visto e todos os fatos do projeto. |
| `kira wiki list\|show\|decision\|reindex\|search\|forget\|promote\|export` | Wiki local do projeto (seção Wiki abaixo). |

O que **não existe ainda** vai no [Roadmap](#roadmap) — nada é prometido como
pronto se não está no binário.

## Requisitos

| Componente | Para quê | De onde |
| --- | --- | --- |
| Rust (via rustup) | build do binário | [rustup.rs](https://rustup.rs) |
| Ollama | **obrigatório para `kira triage`** (LLM local) | `winget install Ollama.Ollama` ou [ollama.com/download](https://ollama.com/download) |
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
| `KIRA_MODEL` | Tag do modelo do triage | `qwen3.5:4b` |
| `KIRA_NUM_CTX` | Janela de contexto do modelo | `4096` |
| `KIRA_STORE_DIR` | Sobrescreve o diretório do store (testes/execuções portáveis) | `~/.config/kira` |
| `KIRA_BOARD` | Provedor de board (`azure` é o único nesta fase) | `azure` |
| `KIRA_CONFIG` | Caminho alternativo do `config.toml` | `~/.config/kira/config.toml` |

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

## Windows

Spawns no Windows diferem do Unix: a std só resolve `.exe`, então:

- **npm/pnpm** spawnam via wrapper `cmd /C chcp 65001 >nul 2>&1 & <ferramenta>` —
  o `cmd /C` resolve `npm.cmd`/`pnpm.cmd` pelo PATHEXT, e o `chcp 65001` força
  UTF-8 para o stderr do cmd localizado não virar mojibake.
- **dotnet** é um `.exe` real e spawna direto.
- **Override `KIRA_SCAN_*_BIN`**: valor `.cmd`/`.bat` no Windows roda via
  `cmd /C`; qualquer outro (e todo valor em Linux/macOS) spawna direto.
- Ferramenta ausente × ferramenta que falhou são erros **acionáveis
  distintos** — "instale o X" ou `a ferramenta <tool> falhou (exit <N>)` com no
  máximo a primeira linha do stderr (≤200 chars) como dica; nunca body cru.
  `npm/pnpm audit` sai com exit 1 exatamente quando há vulnerabilidades — isso
  é caso normal (o JSON é parseado).

A resolução **which-style** do binário antes de qualquer spawn (PATH × PATHEXT,
sem classificação por texto de cmd localizado) está em andamento — veja
[Roadmap](#roadmap).

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

- **Resolução which-style da ferramenta antes do spawn** (PR-F4, em
  `w2-fix/pr-f4-tool-resolution`): PATH × PATHEXT em Rust puro, substituindo a
  classificação por marcadores de texto do cmd localizado.
- **Parser pnpm audit real** (PR-F1b): advisory-a-advisory com `paths`.
- **`kira triage --eval <dir>`** + `tests/fixtures/triage_gold/` — 20 bugs
  fechados rotulados pelo dono; gate do M2 é **≥70% medido** (W2-bis).
- **Autonomia: `kira loop` / modo agendado** com estado `awaiting_approval` no
  store e `kira approve <id>` (W5) — escrita só depois do gate M2 e do
  `--apply` explícito.
- **Backend Copilot** (`copilot -p`) como LLM caro sob flag explícita (PR11);
  Ollama segue como default 100% local.
- **TUI Ratatui** (W6) e `kira fix --apply` (M3) — só depois do M2.

## License

MIT — see [LICENSE](LICENSE).
