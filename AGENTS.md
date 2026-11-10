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
- Segredo (PAT) vive em env `AZURE_DEVOPS_PAT` OU no keyring do OS (salvo por `kira setup`, service `kira`). NUNCA em arquivo pleno, log, `Debug`, mensagem de erro ou teste.
- Configuração (org, projeto) tem prioridade flag > env > `$HOME/.config/kira/config.toml` (0600, escrito por `kira setup`).
- Operações de leitura por padrão. Escrita no Azure exige flag explícita (`--apply`).
- Erro precisa dizer o que fazer: "defina AZURE_DEVOPS_PAT ou rode `kira setup`" > "credencial ausente".
- Sem `unsafe`. Sem dependência nova sem justificar no PR.
## Boards plugáveis
- Todo acesso a work item passa pela trait `BoardProvider` (`src/board/`): `work_item(id)` e `project_names()`. A fábrica é `board::for_board(<nome>, ...)`; nome vem de `KIRA_BOARD` (default `azure`), board desconhecido => erro acionável.
- O Azure DevOps é o MVP: `azure::AdoBoard` apenas delega para `src/ado` — nada de lógica de API fora de `src/ado/`. Jira/Trello virarão novos módulos em `src/board/`.
## IA
- Backend LLM plugável via trait `LlmBackend` (`src/llm/`), previsto para copilot-sdk | byok-http | ollama. Nesta fase só existe `OllamaBackend`, sempre localhost (default `http://localhost:11434`).
- Prompts são versados no repo, em `src/llm/prompts.rs` (constantes públicas, revisadas em PR) — sem framework de orquestração.
- Nunca enviar segredos ao LLM: PAT, env e credenciais não entram no prompt — o prompt só contém campos do work item.
- `kira triage` é somente leitura: o prompt do sistema nunca pede para escrever/editar código e o comando nunca altera o Azure DevOps.
- Segredos no keyring: `src/credentials.rs` concentra `CredStore` (`OsKeyring` no runtime, `InMemory` só em testes). Testes NUNCA tocam o keyring do OS.
