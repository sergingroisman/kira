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
- Sem `unsafe` no código de runtime. Exceção única e test-only: testes que mexem em variáveis de ambiente (edition 2024 torna `env::set_var`/`remove_var` unsafe). Sem dependência nova sem justificar no PR.
## Boards plugáveis
- Todo acesso a work item passa pela trait `BoardProvider` (`src/board/`): `work_item(id)` e `project_names()`. A fábrica é `board::for_board(<nome>, ...)`; nome vem de `KIRA_BOARD` (default `azure`), board desconhecido => erro acionável.
- O Azure DevOps é o MVP: `azure::AdoBoard` apenas delega para `src/ado` — nada de lógica de API fora de `src/ado/`. Jira/Trello virarão novos módulos em `src/board/`.
## IA
- Backend LLM plugável via trait `LlmBackend` (`src/llm/`), previsto para copilot-sdk | byok-http | ollama. Nesta fase só existe `OllamaBackend`, sempre localhost (default `http://localhost:11434`).
- Prompts são versados no repo, em `src/llm/prompts.rs` (constantes públicas, revisadas em PR) — sem framework de orquestração.
- Nunca enviar segredos ao LLM: PAT, env e credenciais não entram no prompt — o prompt só contém campos do work item.
- `kira triage` é somente leitura: o prompt do sistema nunca pede para escrever/editar código e o comando nunca altera o Azure DevOps.
- Segredos no keyring: `src/credentials.rs` concentra `CredStore` (`OsKeyring` no runtime, `InMemory` só em testes). Testes NUNCA tocam o keyring do OS.
## Store
- Trilha local por projeto em SQLite (rusqlite, feature `bundled` — binário sem instalação externa); tudo em `src/store/`; DB único central `~/.config/kira/store.db` (Windows: `%USERPROFILE%\.config\kira\store.db`), modo WAL, migração idempotente via `PRAGMA user_version` com `CREATE TABLE IF NOT EXISTS`.
- FAIL-OPEN OBRIGATÓRIO: erro do store nunca quebra o comando principal — só `aviso: trilha não foi gravada (<causa>)` no stderr (emitido só no `main.rs`, em lugar nenhum outro) e o código de saída/saída do comando permanece.
- NUNCA gravar segredos no DB: PAT, tokens, env. NUNCA gravar a description completa do work item — summary = primeiros 2000 chars do output do comando (o `doctor` já maska o PAT).
- Comandos `inspect`, `triage` (vivo e `--file`) e `doctor` gravam sessão automaticamente; `history`/`note`/`profile` fazem consulta e NÃO gravam sessão.
- Timestamps no DB em RFC 3339 UTC; exibição local usa SQLite `datetime(x, 'localtime')` (sem dependência de fuso próprio).
- Sobrescrever o diretório do DB com a env `KIRA_STORE_DIR` (testes e execuções portáveis) — envs novas só com justificativa no PR.
