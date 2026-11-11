# KIRA — contrato do repositório
CLI em Rust para manutenção assistida de repositórios hospedados no Azure DevOps.
Nome: KIRA = Knowledge-Informed Remediation Agent.

## Princípios
- Read-only por padrão. Escrita no mundo só com `--apply` explícito — e, para autonomia, só depois de `awaiting_approval` + `kira approve` (humano).
- Erro acionável: toda falha diz o que fazer; nunca body cru de HTTP/stderr no output.
- Segredo nunca vai para prompt, DB ou log — PAT/token vive só em env ou keyring.
- Determinismo antes de inteligência: mesma entrada = mesmo `findings.json` byte a byte (e mesmo context pack).
- LLM é o último recurso: CLI real → parser/SQL → cálculo em Rust; só então modelo (ADR-0016), com `// LLM porque:` no call site.
- Ollama local (localhost) é o default; provedor caro (Copilot) só sob flag explícita.
- Ferramenta resolvida no PATH antes do spawn (which-style, PATH × PATHEXT) — sem classificar por texto de OS/cmd localizado.
- Wiki local em `.kira/wiki/`: markdown é a fonte de verdade, índice FTS5 é derivado e descartável; ciclo autônomo só grava em `.kira/wiki-staging/`.
- Um item = um PR com gates (`cargo fmt --check`, `clippy -D warnings`, `test`, `build`, CI 3 SOs).

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
- Erro precisa dizer o que fazer: "defina AZURE_DEVOPS_PAT ou rode `kira setup`" > "credencial ausente". Falhas de API do ADO aparecem como acionáveis: nunca body cru de resposta HTTP em execução.
- Sem `unsafe` no código de runtime. Exceção única e test-only: testes que mexem em variáveis de ambiente (edition 2024 torna `env::set_var`/`remove_var` unsafe). Sem dependência nova sem justificar no PR.
## Setup
- `kira setup` pergunta "Você já tem um PAT pronto? [S/n]" SEMPRE no modo interativo (stdin é TTY — `stdin().is_terminal()`, injetado como `SetupOptions::interactive`); stdin pipado mantém o fast path antigo (passos + abrir + colar). Enter = 's'; respostas inválidas repetem no máximo 2x e então assumem 's', com aviso.
- 'n' na pergunta imprime os passos (URL `https://dev.azure.com/{org}/_usersSettings/tokens` + escopos Work Items Read + Project and Team Read) e confirma "Abrir no navegador agora? [S/n]" — o navegador SÓ abre com confirmação.
- Abertura de navegador NUNCA bloqueia: `Command::spawn` (handle descartado — nunca `.status()`/`.output()`); spawn falhou ⇒ só re-imprime a URL e continua.
- `--no-browser` pula os passos e nunca pergunta sobre navegador (vai direto ao "Cole o PAT"); `--pat` é zero-prompts de token e o valor nunca é ecoado (só máscara).

## Boards plugáveis
- Todo acesso a work item passa pela trait `BoardProvider` (`src/board/`): `work_item(id)` e `project_names()`. A fábrica é `board::for_board(<nome>, ...)`; nome vem de `KIRA_BOARD` (default `azure`), board desconhecido => erro acionável.
- O Azure DevOps é o MVP: `azure::AdoBoard` apenas delega para `src/ado` — nada de lógica de API fora de `src/ado/`. Jira/Trello virarão novos módulos em `src/board/`.
## IA
- Backend LLM plugável via trait `LlmBackend` (`src/llm/`), previsto para copilot-sdk | byok-http | ollama. Existem `OllamaBackend` (default, sempre localhost `http://localhost:11434`) e `CopilotBackend` (spawn do CLI `copilot -p`, ADR-0019).
- **O backend caro nunca é implícito**: só roda sob flag explícita (`--llm copilot`) ou env (`KIRA_LLM=copilot`); default permanece Ollama. Erro de auth do Copilot é acionável (login, Grant na org, TI) e **nunca vira fallback silencioso** para o Ollama — o usuário pediu o backend caro, a ausência falha.
- Prompts são versados no repo, em `src/llm/prompts.rs` (constantes públicas, revisadas em PR) — sem framework de orquestração.
- Nunca enviar segredos ao LLM: PAT, env e credenciais não entram no prompt. Além dos campos do work item, o prompt leva só o **context packer** (abaixo), já redigido por `redact`.
- `kira triage` é somente leitura: o prompt do sistema nunca pede para escrever/editar código e o comando nunca altera o Azure DevOps.
- Segredos no keyring: `src/credentials.rs` concentra `CredStore` (`OsKeyring` no runtime, `InMemory` só em testes). Testes NUNCA tocam o keyring do OS.
## Contexto (context packer, ADR-0015)
- RETRIEVAL-FIRST: o KIRA busca, o modelo só interpreta. Nada de tool-calling, embedding, LangGraph ou índice vetorial — `walkdir` + casamento de termos e FTS5 da wiki, tudo em `src/context/`.
- Todo trecho sai ancorado: repositório como `caminho:linha`, wiki como caminho + idade em dias. Blob anônimo no prompt é proibido.
- Ordem de prioridade do orçamento (`KIRA_NUM_CTX`), fixa: brief do `kira scan` → achados individuais → páginas da wiki → trechos do repositório. Orçamento estourado vira nota, nunca erro.
- DETERMINISMO: caminhada ordenada por nome, ranking = arquivo citado em `findings.json` primeiro, depois termos casados, caminho e linha. Mesmo estado ⇒ mesmo pack (gate do M1).
- Toda linha do repositório passa por `redact`; linha de atribuição (`token = ...`, `password = ...`) sai mascarada mesmo quando é código — privacidade acima de utilidade.
- Nunca greppar `.kira/wiki/` (já vem do FTS5), dot-dirs, `target/`, `node_modules/`, build output, nem o arquivo do work item passado em `--file`.
- **Índice de retrieval do repositório (ADR-0021): o índice (`repo_file`/`repo_fts`, schema v3) é DERIVADO E DESCARTÁVEL — a fonte de verdade são os arquivos do disco; reconstruí-lo nunca pode perder conhecimento.** REDAÇÃO NA ESCRITA: toda linha passa por `crate::redact` ANTES de entrar no índice — o store nunca guarda segredo, nem em dado derivado. Teto `KIRA_INDEX_MAX_FILES` (default 5000, parse validado) substitui o corte alfabético de 400. Busca = `repo_fts MATCH` com bm25; FAIL-OPEN: índice ausente/vazio ⇒ cai no grep determinístico, e o pack marca a fonte (`index`/`grep`). Falha de reindex é nota, nunca erro do comando.
- FAIL-OPEN por fonte: wiki, store, findings ou grep que falham só geram nota; `kira triage` continua e imprime `contexto: ...` no stderr dizendo o que foi enviado.
## Store
- Trilha local por projeto em SQLite (rusqlite, feature `bundled` — binário sem instalação externa); tudo em `src/store/`; DB único central `~/.config/kira/store.db` (Windows: `%USERPROFILE%\.config\kira\store.db`), modo WAL, migração idempotente via `PRAGMA user_version` com `CREATE TABLE IF NOT EXISTS`.
- FAIL-OPEN OBRIGATÓRIO: erro do store nunca quebra o comando principal — só `aviso: trilha não foi gravada (<causa>)` no stderr (emitido só no `main.rs`, em lugar nenhum outro) e o código de saída/saída do comando permanece.
- NUNCA gravar segredos no DB: PAT, tokens, env. NUNCA gravar a description completa do work item — summary = primeiros 2000 chars do output do comando (o `doctor` já maska o PAT).
- Comandos `inspect`, `triage` (vivo e `--file`) e `doctor` gravam sessão automaticamente; `history`/`note`/`profile` fazem consulta e NÃO gravam sessão.
- Timestamps no DB em RFC 3339 UTC; exibição local usa SQLite `datetime(x, 'localtime')` (sem dependência de fuso próprio).
- Sobrescrever o diretório do DB com a env `KIRA_STORE_DIR` (testes e execuções portáveis) — envs novas só com justificativa no PR.
- **Índice de retrieval do repositório (schema v3, ADR-0021):** `repo_file(project_id, path, mtime, size)` + `repo_fts` (FTS5, colunas UNINDEXED `project_id`/`path`/`line`, `text` indexado, tabela regular — NÃO contentless, o `text` precisa ser recuperável). Uma linha de `repo_fts` por linha de código já redigida; incremental por `(mtime, size)`; arquivo que saiu do disco sai do índice; determinístico (caminhos ordenados, POSIX). Falhar em criar o FTS5 degrada (aviso), nunca bloqueia — o packer cai para grep.
## Scan
- `kira scan` roda as ferramentas padrão da stack (`dotnet list package --vulnerable --include-transitive --format json`; `pnpm audit --json` / `npm audit --json`). KIRA NÃO analisa — orquestra, normaliza e mantém determinismo. NADA de IA nesta rodada (triage grounded entra depois).
- DETERMINISMO OBRIGATÓRIO (gate do M1): `fingerprint` = FNV-1a 64 sobre `tool|rule|package|version|message` (trimado); a lista gravada fica em ordem crescente por fingerprint — duas execuções no mesmo estado produzem o MESMO conjunto de fingerprints. `findings.json` não contém timestamp nem nada dependente de máquina.
- Findings gravados em `<root>/.kira/findings.json`; `.kira/` é LOCAL: nunca comitar (KIRA escreve a nota `.kira/README.md` de que o diretório é local). Resumo no stdout: achados por severidade com contagem por ferramenta + caminho do arquivo.
- `tool: dotnet` com regras `vulnerable` (advisory em `.vulnerabilities`) e `outdated` (pacote deprecado); `tool: audit` com `vulnerable` (advisory do npm/pnpm). `severity` grava-se exatamente como veio no JSON da ferramenta.
- Ferramenta é RESOLVIDA no PATH antes do spawn (`which`-style em Rust puro); ausência => "instale o X" SEM spawn; classificação por texto de OS é proibida (cmd é localizado e pode chegar em mojibake — nada de parse de mensagem do OS). Ordem: (1) override `KIRA_SCAN_<TOOL>_BIN` (valor presente e não vazio é contrato do usuário — o arquivo apontado spawna como veio; inexistente => o mesmo "instale o X"); (2) busca no PATH — Windows: cada dir do PATH × PATHEXT (default `.COM/.EXE/.BAT/.CMD`, lookup case-insensitive como o NTFS é); Linux/macOS: arquivo `<tool>` com bit de execução. O spawn usa sempre o caminho completo RESOLVIDO (nunca o nome nu); `.cmd`/`.bat` no Windows => wrapper `cmd /C chcp 65001 >nul 2>&1 & <caminho>` (o chcp fica: corrige o mojibake do OUTPUT de ferramenta real), resto spawn direto.
- Executou e saiu != 0 => `a ferramenta <tool> falhou (exit <N>) — confira versão/instalação e rode kira scan de novo` com no máximo a PRIMEIRA linha do stderr (trimada, ≤200 chars) como ` (dica: ...)` — ToolFailure compacto direto, sem classificação por texto; nunca parse nem eco de body cru — nem stderr cru de `dotnet`. Saída de `npm/pnpm audit` com exit-code errado: parse é tentado primeiro — o exit-code 1 do audit é a forma normal de dizer que há vulnerabilidades (só vira erro quando o JSON não parseia).
- Toda extração dos JSON é função pura de uma `String` (src/scan/), testada com fixtures em `tests/fixtures/scan/` (sem executar nenhuma ferramenta real em testes); testes de CLI cobrem os caminhos "ferramenta ausente" (sem overrides e com PATH vazio, os env limpos), "ferramenta presente que falha" (shim exit 1 apontado pela env `KIRA_SCAN_*_BIN` — SEM shadowing de PATH) e "nenhum achado" + arquivo determinístico (duas execuções byte-idênticas).
## CLI-first, LLM-last (regra do dono, ADR-0016)
- **Tudo que um comando de linha resolve, o KIRA faz por comando de linha.** A escada é obrigatória e de baixo para cima: (1) CLI real (`dotnet`, `pnpm`/`npm`, `git`, `rg`, `gh`) → (2) parser puro / SQL / FTS5 → (3) cálculo em Rust → (4) **LLM só quando 1–3 não respondem**.
- Toda chamada ao `LlmBackend` exige: pergunta de julgamento (não recuperação/extração/contagem/comparação), nenhuma camada inferior capaz de responder, e o comentário `// LLM porque: <razão>` no call site.
- Só `src/triage.rs`, `src/loop/` e `src/wiki/` podem importar `llm::*`. Teste de allowlist (PR33) reprova o resto — `scan`, `store`, `board`, `ado`, `guard`, `format`, `tui`, `doctor`, `scheduler` nunca falam com modelo.
- O budget de 50 chamadas/dia mede **LLM**, não trabalho: comando determinístico não consome budget.
## Wiki (`src/wiki/`, ADR-0013)
- Fonte de verdade em markdown **local** (W2-fix PR-F3): `<projeto>/.kira/wiki/{contexto,decisoes,padroes,glossario}.md` e `<projeto>/.kira/wiki/bugs/<slug>.md`. Índice (FTS5, PR29) é derivado e descartável.
- **Toda escrita passa por `redact` (segredos nunca vão para o disco) e por `guard::path` (confinamento na raiz).** Id de página inválido (`..`, absoluto, maiúscula, slug fora de `[a-z0-9_-]`) é recusado antes de qualquer I/O.
- **Ciclo autônomo nunca escreve em `.kira/wiki/`**: grava em `.kira/wiki-staging/` e depende de promoção humana (`kira wiki promote`, PR32).
- Páginas não são log de conversa: guardam conhecimento reutilizável (arquitetura, decisões, padrões, glossário, causa raiz de bug). Nada de transcrição.
- `kira triage` (comando humano) anexa um **episódio** em `.kira/wiki/bugs/<item-id>.md` com causa raiz prevista, arquivos, confiança e o brief do `kira scan` — sem perguntar nada. Episódio vazio (as 3 seções sem conteúdo) NÃO grava: a falha fica registrada só na trilha (`kira history`). Episódio repetido (mesmo conteúdo) não duplica; episódios com mais de 30 dias colapsam numa linha em `## Histórico colapsado`. O ciclo autônomo usa `.kira/wiki-staging/` (`EpisodeTarget::Staging`), nunca `.kira/wiki/`.
- `kira wiki forget <página>` apaga o markdown e as entradas do índice (LGPD: dado do dono); página inexistente não é erro. O índice é derivado — falha ao limpá-lo vira aviso, nunca erro do comando.

## Autonomia e aprovação
- **Leitura é livre, escrita nunca.** Todo comando é read-only por padrão; escrita no mundo exige flag explícita `--apply`. Modo agendado e `kira loop` NUNCA ganham escrita implícita.
- **`awaiting_approval` é estado do store, não convenção.** Antes de qualquer escrita no mundo (Azure DevOps, repo do projeto) o ciclo grava `awaiting_approval` e para. Só `kira approve <id>` (humano) transiciona para `applying`. Budget estourado ≠ aprovação; `--apply` isolado ≠ aprovação.
- **Budget tem número:** `max_llm_calls_per_day = 50` em `config.toml`, checado ANTES de cada transição de estado.
- **Medição antes de autonomia (previsto — W2-bis, ainda não implementado):** `kira triage --eval <dir>` compara a causa raiz prevista com o rótulo do dono em `tests/fixtures/triage_gold/` e imprime o percentual. Sem ≥70% medido, `kira loop` e o modo agendado não são promovidos a "autônomo".
- **Falha de quota não apaga dado:** provedor que responde 429 fica intocado por 5 minutos; o último resultado bom continua visível marcado como `stale` (`kira doctor` mostra a idade do último sucesso).
- **TUI é modo do binário, não substituto:** o dashboard (Ratatui) é read-only sobre o store; o modo texto do loop/agendado continua sendo a saída canônica e erros permanecem texto copiável.
- **Proibido copiar código de projeto GPL** (ex.: ai-jail) para este repo MIT. Conceito sim, código não.

