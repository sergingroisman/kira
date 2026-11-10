# KIRA

KIRA — Knowledge-Informed Remediation Agent (*Keep It Running, Always*).

A Rust CLI for assisted maintenance of repositories hosted on Azure DevOps.
In M0 it does exactly one thing, read-only: fetch a work item and print a
readable summary in the terminal.

## Status

M0 — one command. No AI, no Copilot, no MCP, no writes to Azure DevOps.

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

KIRA talks to Azure DevOps with a Personal Access Token (PAT) stored in
environment variables.

1. Create a PAT with the **Work Items (Read)** scope at
   `https://dev.azure.com/{your-organization}/_usersSettings/tokens`.
   Prefer the shortest expiration that fits your workflow (e.g. 90 days) and
   rotate it when it expires.

2. Export the three variables.

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

3. Verify the configuration:

   ```bash
   kira doctor
   ```

4. Then inspect a work item:

   ```bash
   kira inspect 4821
   ```

## Usage

```bash
kira inspect <work-item-id> [--project <name>]
```

| Variable | Description |
| --- | --- |
| `AZURE_DEVOPS_PAT` | Personal Access Token with **Work Items (Read)** permission |
| `AZURE_DEVOPS_ORG` | Azure DevOps organization name |
| `AZURE_DEVOPS_PROJECT` | Default project; overridden by `--project <name>` |

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
