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
