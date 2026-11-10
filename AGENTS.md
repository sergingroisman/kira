# KIRA — contrato do repositório
CLI em Rust para manutenção assistida de repositórios hospedados no Azure DevOps.
Nome: KIRA = Knowledge-Informed Remediation Agent.
## Stack fixa
- Rust 2024, cargo, binário único `kira`
- `azure_devops_rust_api` 0.39 (API gerada, assíncrona) — todo uso isolado em `src/ado/`
- `clap` 4 para CLI, `tokio` apenas onde a API gerada exigir
- `serde`/`serde_json` para serialização
## Comandos
- build: `cargo build`
- test: `cargo test`
- lint: `cargo clippy --all-targets -- -D warnings`
- fmt: `cargo fmt`
## Regras duras
- Nunca `unwrap()`/`expect()` em entrada de usuário, rede ou parse. `thiserror` + `?`.
- Segredo (PAT) só de env `AZURE_DEVOPS_PAT`. Nunca em `Debug`, `log`, mensagem de erro ou teste.
- Operações de leitura por padrão. Escrita no Azure exige flag explícita (`--apply`).
- Erro precisa dizer o que fazer: "defina AZURE_DEVOPS_PAT" > "credencial ausente".
- Sem `unsafe`. Sem dependência nova sem justificar no PR.
## IA
- Backend LLM plugável via trait `LlmBackend` (`src/llm/`), previsto para copilot-sdk | byok-http | ollama. Nesta fase só existe `OllamaBackend`, sempre localhost (default `http://localhost:11434`).
- Nunca enviar segredos ao LLM: PAT, env e credenciais não entram no prompt — o prompt só contém campos do work item.
- `kira triage` é somente leitura: o prompt do sistema nunca pede para escrever/editar código e o comando nunca altera o Azure DevOps.
