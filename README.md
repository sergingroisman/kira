# KIRA

KIRA — Knowledge-Informed Remediation Agent (*Keep It Running, Always*).

A Rust CLI for assisted maintenance of repositories hosted on Azure DevOps.
In M0 it does exactly one thing, read-only: fetch a work item and print a
readable summary in the terminal.

## Status

M1.0 — pluggable board (`BoardProvider`, Azure DevOps MVP), local triage, and
`kira setup` (PAT in the OS keyring). Read-only: no writes to Azure DevOps.

## Install

```bash
cargo install --path .
```

## Try it locally (Windows, no Azure account)

Triage runs 100% locally against an Ollama model: no PAT, no Azure account, no
cloud. You only need a work-item JSON file — the repo ships with one at
`tests\fixtures\work_item_4821.json`.

1. Install Rust via rustup: download and run `rustup-init.exe` from
   [rustup.rs](https://rustup.rs) (default options are fine).

2. Install Ollama:

   ```powershell
   winget install Ollama.Ollama
   ```

   or download it from [ollama.com/download](https://ollama.com/download). It
   runs as a Windows service automatically.

3. Pull the default model (~3.3 GB):

   ```powershell
   ollama pull qwen3.5:4b
   ```

4. Build KIRA from the repository folder:

   ```powershell
   cargo install --path .
   ```

   Or run straight from source without installing:

   ```powershell
   cargo run -- triage --file tests\fixtures\work_item_4821.json
   ```

5. Triage the sample work item:

   ```powershell
   kira triage --file tests\fixtures\work_item_4821.json
   ```

   Expected output (the model text varies):

   ```text
   Causa raiz provável:
   O token de sessão tem TTL curto (5 minutos) e não é renovado no refresh — a sessão cai mesmo com token válido.

   Arquivos prováveis:
   src/auth/session.rs
   src/auth/token.rs

   Confiança:
   média
   ```

The live path (`kira triage <work-item-id>` and `kira inspect <id>`) still
requires the Azure DevOps PAT — see [Setup](#setup).

### Local LLM variables

| Variable | Description | Default |
| --- | --- | --- |
| `KIRA_OLLAMA_URL` | Base URL of the local Ollama server | `http://localhost:11434` |
| `KIRA_MODEL` | Model tag used by `kira triage` | `qwen3.5:4b` |
| `KIRA_NUM_CTX` | Context window for the model | `4096` |

The `--model` flag overrides `KIRA_MODEL`:

```bash
kira triage 4821 --model llama3.2:3b
```

### Troubleshooting

| Symptom | Fix |
| --- | --- |
| `Ollama não está rodando` (connection refused) | The Ollama service is stopped — open Ollama from the Start menu (on Windows it runs as a service) or run `ollama serve`, then retry |
| Slow answers / split GPU | Run `ollama ps` and check the model shows `100% GPU`; if it shows a CPU split, reduce the context with `KIRA_NUM_CTX=2048` and retry |
| Not enough VRAM | Use a smaller model: `ollama pull llama3.2:3b` (2.0 GB) or `ollama pull phi4-mini` (2.5 GB), then run with `--model llama3.2:3b` |
| `HTTP 404 do Ollama` | The model was not pulled — run `ollama pull <model>` and check `ollama list` |

On a 6 GB RTX 2060, `qwen3.5:4b` (~3.3 GB, Q4_K_M) fits entirely in GPU
memory and the default `num_ctx` of 4096 is conservative.

## Setup

KIRA talks to Azure DevOps with a Personal Access Token (PAT). Where your PAT
lives:

| Where | How | Best for |
| --- | --- | --- |
| OS keyring (Windows Credential Manager / secret service) | `kira setup` | Your machine — interactive use |
| `AZURE_DEVOPS_PAT` env var | exported by hand | CI / headless machines |

### Option A — `kira setup` (recommended, interactive)

```bash
kira setup
```

The command walks you through four questions:

1. **organization** and **project** (you can also pass them with
   `--org <name> --project <name>`, or have them in `AZURE_DEVOPS_ORG` /
   `AZURE_DEVOPS_PROJECT` or `config.toml` — anything already known is not
   asked again);
2. **"Você já tem um PAT pronto? [S/n]"** — with `Enter = S`;
   - **S**: paste the token straight away, with no steps and no browser;
   - **N**: KIRA prints the token page
     (`https://dev.azure.com/{your-org}/_usersSettings/tokens`) and the
     **Work Items (Read)** + **Project and Team (Read)** scopes, then asks
     **"Abrir no navegador agora? [S/n]"** — the browser only opens after your
     confirmation and never blocks the terminal (`(se não abrir, a URL está
     acima)`); prefer the shortest expiration that fits your workflow (e.g. 90
     days) and rotate it when it expires;
3. you paste the PAT (one line, visible input);
4. KIRA **validates before saving**: it lists your projects and checks the
   project name. Only when that succeeds does it store the PAT in the OS
   keyring (service `kira`) and save org/project to
   `~/.config/kira/config.toml` (mode 0600). If validation fails (401), nothing
   is saved and the command tells you what to review.

   ```text
   setup concluído — rode kira triage <id>
   ```

Piped stdin skips the *"já tem um PAT?"* question and keeps the old fast path;
`--no-browser` skips the steps and the browser open entirely, going straight to
the paste prompt; `--pat` skips every token prompt.

To remove the saved PAT from the keyring:

```bash
kira setup --reset
```

#### Already have a token?

In the interactive flow just answer **`S`** to "Você já tem um PAT pronto?" and
paste it when asked — or skip every prompt from the CLI. Read the token from
a secure file or the environment instead of typing it bare (PowerShell):

```powershell
$pat = Get-Content "$env:USERPROFILE\.secrets\ado-pat.txt"   # ou $env:ADO_PAT
kira setup --org minha-org --project MeuProjeto --pat $pat
```

or, passing it directly (--pat skips every prompt and never echoes the token):

```powershell
kira setup --org minha-org --project MeuProjeto --pat <your-token>
```

Use `--no-browser` to skip the instruction steps and the browser open but
still paste the token when asked:

```bash
kira setup --no-browser
```

If the token is already exported as `AZURE_DEVOPS_PAT`, no setup step is
needed at all: the env var wins over the keyring and `config.toml` at runtime.

### Option B — manual (env vars, for CI / headless)

Create a PAT with the **Work Items (Read)** scope at
`https://dev.azure.com/{your-organization}/_usersSettings/tokens`, then export
the three variables.

   bash:

   ```bash
   export AZURE_DEVOPS_PAT=your-pat
   export AZURE_DEVOPS_ORG=your-org
   export AZURE_DEVOPS_PROJECT=your-project
   ```

   PowerShell:

   ```powershell
   $env:AZURE_DEVOPS_PAT = "your-pat"
   $env:AZURE_DEVOPS_ORG = "your-org"
   $env:AZURE_DEVOPS_PROJECT = "your-project"
   ```

The env var wins over the keyring and over `config.toml`, so Option B also
works on a machine configured with Option A (handy for one-shot CI runs).

### Verify the configuration

```bash
kira doctor
```

It shows where each setting came from (env, keyring or `config.toml`), tests
the connection and checks the project name. Then inspect a work item:

```bash
kira inspect 4821
```

## Usage

```bash
kira inspect <work-item-id> [--project <name>]
```

| Variable | Description |
| --- | --- |
| `AZURE_DEVOPS_PAT` | Personal Access Token with **Work Items (Read)** permission (env var or saved by `kira setup` in the OS keyring) |
| `AZURE_DEVOPS_ORG` | Azure DevOps organization name (env var or `config.toml`) |
| `AZURE_DEVOPS_PROJECT` | Default project; overridden by `--project <name>` (env var or `config.toml`) |
| `KIRA_BOARD` | Board provider to use; `azure` is the only one in this phase |

Example:

```bash
export AZURE_DEVOPS_PAT=xxx
export AZURE_DEVOPS_ORG=my-org
export AZURE_DEVOPS_PROJECT=my-project
kira inspect 4821
```

Output:

```text
#4821  Bug  Active  [Backend]
Título: Sessão expirada com token válido
Responsável: —
Descrição: first 400 chars of the text, HTML removed, line breaks preserved
```

## Scanning (determinístico)

`kira scan` runs the **standard tools of each stack** of the project, normalizes
them into findings and writes a deterministic file. **No AI in this step** —
KIRA does not analyze; it orchestrates and normalizes (grounded triage comes
later).

### Requirements

| Stack | Tool | Where it comes from |
| --- | --- | --- |
| .NET | `dotnet list package --vulnerable --include-transitive --format json` | .NET SDK 8+ — [dotnet.com/download](https://dotnet.com/download) |
| Node/TS (pnpm) | `pnpm audit --json` | `npm install -g pnpm` |
| Node/TS (npm) | `npm audit --json` | [nodejs.org/download](https://nodejs.org/download) |

Note: `kira scan` runs on Windows too — see "Windows invocation" below.

### Windows invocation

Windows spawns differ from Unix: the standard library only looks for `.exe`,
so the Node tools spawn wrapped in `cmd /C` (which resolves `npm.cmd`/
`pnpm.cmd` through PATHEXT):

- `npm audit --json` → `cmd /C npm audit --json`
- `pnpm audit --json` → `cmd /C pnpm audit --json`
- `dotnet list package ...` → spawned directly (`dotnet.exe` is a real exe).

### Binary overrides (`KIRA_SCAN_*_BIN`)

Any scan tool's binary can be replaced without PATH shadowing (`dotnet`
shadowing is unreliable anyway). The value is the FULL path of an
executable/shim:

| Env var | Replaces |
| --- | --- |
| `KIRA_SCAN_DOTNET_BIN` | `dotnet list package` |
| `KIRA_SCAN_NPM_BIN` | `npm audit --json` |
| `KIRA_SCAN_PNPM_BIN` | `pnpm audit --json` |

Uses: CI runners without the tool installed, custom packaging, offline tests
(the e2e suite points its fake shims exactly through these). An
`.cmd`/`.bat` value on Windows spawns via `cmd /C`; anything else (and all
values on Linux/macOS) spawns directly. No override set => the default
PATH lookup above.

```sh
# CI runner without a .NET SDK: point to the packaged binary
KIRA_SCAN_DOTNET_BIN=/usr/local/packaged/bin/dotnet kira scan
```

Windows examples:

```powershell
kira scan                    # uses the project root (detected like the store)
kira scan --path C:\repos\api
```

### What it detects

In the folder given (`--path <dir>`, up to 2 levels deep) KIRA detects:

- **.NET** — any `*.csproj`, `*.sln` or `global.json`;
- **Node/TS** — `package.json` (pnpm when there is `pnpm-lock.yaml`, npm with
  `package-lock.json` or no lock at all).

No marker found => actionable error ("nenhum stack detectado ...").

### Output

- `<root>\.kira\findings.json` — findings normalized as
  `{ tool, rule, package, version, severity, path, message, fingerprint }`.
  `tool` is `dotnet` (rules `vulnerable`/`outdated` — packages with
  vulnerabilities or deprecated) or `audit` (npm/pnpm user, rule
  `vulnerable`). **`.kira/` é local — nunca comite este diretório**: adicione
  `.kira/` ao `.gitignore` do projeto (o `.gitignore` da KIRA não precisa
  mudar).
- the stdout prints the summary: total findings, count by severity with count
  per tool, and the file path.

### Determinism (M1 gate)

The `fingerprint` is FNV-1a 64-bit (hand-implemented, shared with the project
store's project id) over `tool|rule|package|version|message` (trimmed), and
the list is stored **ascending by fingerprint**. Two runs over the same state
produce the same fingerprints in the same order — the M1 reproducibility gate
of the plan v2 (§5/§8). The `.kira/README.md` note explains that the folder
is local.

### Troubleshooting

The scan does not install anything and never echoes the raw stdio of the tool:
errors are separate and actionable.

- **Tool missing** (`spawn` NotFound): "o scan .NET exige o SDK do .NET …" or
  "o scan Node/TS via npm exige o npm — instale em https://nodejs.org/download …".
  Install the tool and run again.
- **Tool present but failed** (exit code ≠ 0 on an audit whose JSON did not
  parse, or a failed `dotnet list package`): `a ferramenta <tool> falhou
  (exit <N>) — confira versão/instalação e rode kira scan de novo (dica: …)` —
  the hint is at most the FIRST line of the tool stderr (≤200 chars), never
  the raw body. Check the tool version/installation (e.g. `dotnet --info`,
  `npm --version`) and run `kira scan` again.
- Note: `npm/pnpm audit` exits 1 exactly when vulnerabilities are FOUND —
  that is the normal, healthy case (KIRA parses the JSON); only a run whose
  output cannot be parsed is reported as a failure.

## Local store

Every KIRA run leaves a persistent trail in a single local SQLite database —
no server, no account, project-scoped and offline.

### Where the DB lives

| Platform | Path |
| --- | --- |
| Windows | `%USERPROFILE%\.config\kira\store.db` |
| Linux / macOS | `~/.config/kira/store.db` |

The same folder already holds `config.toml`. Override the directory with the
`KIRA_STORE_DIR` env var (handy for tests and portable runs):

```bash
KIRA_STORE_DIR=/tmp/kira kira history
```

### Schema (summary)

| Table | Columns | Written by |
| --- | --- | --- |
| `projects` | `id`, `root_path`, `name`, `first_seen_at` | automatic (first run in a repo root) |
| `sessions` | `id`, `project_id`, `command`, `args`, `model`, `started_at`, `duration_ms`, `ok`, `summary` | automatic (`inspect`/`doctor`/`triage`/`scan`) |
| `facts` | `project_id`, `key`, `value`, `updated_at` | `kira note` |

The project id is a FNV-1a 64-bit hash (implemented by hand, no extra
dependency) of the canonical project root: the first ancestor of the current
directory that contains `.git`, falling back to the nearest ancestor with
`Cargo.toml`/`package.json`, falling back to the current directory itself.
Timestamps are RFC 3339 UTC; commands display local time via SQLite's
`datetime(x, 'localtime')`.

### Commands

```bash
kira history [--last N]        # recent sessions of the current project (N = 10 by default)
kira note "text of the fact"   # add a profile fact (append-only: note-1, note-2, ...)
kira note --remove 2           # remove the fact with that number
kira profile                   # project root/name/first-seen + all stored facts
```

`inspect`, `triage` (both the live and the `--file` path), `doctor` and
`scan` record
their session automatically — including failed runs (✗, with the error text as
summary, first 2000 chars, capped).

### Fail-open

A broken store never breaks the main command. If the trail cannot be written
(disk full, bad `KIRA_STORE_DIR`, corrupt DB...), KIRA prints one warning on
stderr and continues with the same output and exit code:

```text
aviso: trilha não foi gravada (não foi possível criar o diretório do store em '...': ...)
```

### Privacy

- Never stored: the PAT, any token/credential or env var contents.
- Never stored: the full work-item description — only the truncated command
  output (at most 2000 characters, and the PAT is masked by `doctor`).
- `kira note` stores exactly what you type.

### Clean-up

Delete the single file to wipe every session, fact and project profile:

```powershell
Remove-Item "$env:USERPROFILE\.config\kira\store.db"
```

## Development

```bash
cargo build
cargo test                                   # unit tests only, no network
cargo test -- --ignored                      # real integration test (needs the env vars above)
cargo clippy --all-targets -- -D warnings
cargo fmt
```

See [AGENTS.md](AGENTS.md) for the repository contract.

## License

MIT — see [LICENSE](LICENSE).
