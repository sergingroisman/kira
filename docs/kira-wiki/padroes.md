# Padrões — invariantes que o código prova

Estes não são princípios aspiracionais: são contratos que um teste ou e2e
reprova se você quebrar. Cada invariante abaixo lista onde mora no código e
qual teste o prova. Os ADRs que os decidiram estão em [[decisoes]]; os
mecanismos do binário, em [[engenharia-kira]].

## 1. Mesma árvore ⇒ mesmo `facts.md` byte a byte

- **Onde**: `src/ingest/` — `render` puro (sem I/O), sem timestamp, caminhos
  relativos POSIX, tudo ordenado, redact na escrita.
- **Prova**: `tests/ingest_e2e.rs::two_ingests_produce_a_byte_identical_facts`
  — dois `ingest` seguidos produzem `facts.md` byte-idêntico.
- **Alcance**: idem para `inventory.json` e `findings.json` do `scan`
  (fingerprint FNV-1a 64 sobre `tool|rule|package|version|message`, lista em
  ordem crescente — `tests/scan_e2e.rs` prova duas execuções byte-idênticas).
- **Por que é invariante e não promessa**: a fileira do ADR-0038 é "sem
  timestamp, caminhos relativos POSIX, tudo ordenado" — o teste falha na hora
  que alguém coloca `now()` no render.

## 2. Erro acionável sempre

- **Onde**: taxonomia única em `src/error.rs`; mapa de erros por backend
  (Copilot: `CopilotMissing`/`CopilotAuth`/`CopilotFailed`/`CopilotTimeout`;
  Foundry: `FoundryAuth`/`FoundryDeployment`/`FoundryApiVersion`/
  `FoundryRateLimited`/`FoundryFailed`; capacidades: `CapabilityMissing`
  com `esta jornada precisa de <capacidade> — rode: kira connect <provider>`).
- **Prova**: `tests/no_runtime_unwrap.rs` reprova `unwrap()`/`expect()` em
  runtime; e2e por backend (`tests/triage_e2e.rs`, `tests/capabilities_e2e.rs`)
  confere a mensagem exata — inclusive as 4 mensagens da spec de validação
  (200/401/403/sem rede). Body cru de HTTP/stderr nunca ecoa (limite: 1ª
  linha, ≤200 chars, mascarada).

## 3. Redact antes do disco

- **Onde**: `crate::redact` é chamado na escrita de todo artefato: índice de
  retrieval (`src/context/index.rs`, via `clean_line`), wiki, skills
  (`kira skills add` redige antes de gravar), `pr draft` (o texto inteiro,
  pois o rascunho pode ir para PR público), log capturado pelo dashboard.
- **Prova**: `tests/security_probe.rs` (probe suite de segurança no
  `cargo test`, ADR-0026) e os testes do prompt de triage que provam que o
  prompt não leva segredo. No índice, a linha entra mascarada no store —
  nem em dado derivado há segredo.
- **A linha mais dura**: linha de atribuição (`token = ...`) sai mascarada
  mesmo quando é código — privacidade acima de utilidade.

## 4. `author ≠ verifier` — a política roda no binário real

- **Onde**: o gate M2 (`kira triage --eval`) compara a saída do triage com o
  gold do dono por regra determinística (similaridade de tokens ≥ 0,50 para
  causa raiz; ≥ 1 basename de arquivo), **sem LLM como juiz** — um LLM
  julgando produto de LLM não é gate.
- **Prova**: `tests/eval_e2e.rs` + o desenho do fluxo do Brain (mapa §5): o
  nó fix é o author, o verifier é outra função; qualquer item falho aborta a
  medição — número parcial não é número.
- **Corolário**: o gate é o exit code (`0` = ≥70%), pronto para CI.

## 5. Fail-open onde a degradação é honesta

- **FTS5 → LIKE**: sem tabela FTS5, a busca da wiki e das skills degrada para
  `LIKE` — `tests/wiki_e2e.rs` e `tests/skills_e2e.rs` cobrem os dois caminhos.
- **Índice → grep**: `repo_fts` ausente/vazio ⇒ o packer cai no grep
  determinístico e marca a fonte (`index`/`grep`) — o `kira context
  --explain` diz qual foi.
- **Store → aviso**: erro do store nunca quebra o comando principal — só
  `aviso: trilha não foi gravada (<causa>)` no stderr, emitido **só em
  `main.rs`**, com a mesma saída e exit code. Store quebrado não paga o
  resultado do comando.
- **Wiki/findings corrompidos no packer**: nota no pack, triage continua
  (fail-open por fonte). Página sem refs nunca é stale.

## 6. Fail-closed onde é seguro

- **Trust**: `digest` recusa rodar sem `.kira/trust.json` (é ele que manda
  código ao modelo) — a política e o e2e (`tests/trust_e2e.rs`) provam o
  gate; `ingest` read-only roda sempre.
- **Store indisponível no `publish`**: recusa (`PublishError::Store`) —
  publicar sem trilha perde o gate por hash.
- **Consentimento no pull de modelo**: sem `--yes` e sem TTY, o pull nem
  começa (erro acionável); disco insuficiente recusa com o número, nunca
  download que estoura o disco.
- **Instalação verificada**: hash divergente ⇒ parar antes de gravar
  qualquer arquivo.
- **Medição**: item falho no `triage --eval` aborta a medição inteira.
- **`awaiting_approval`**: escrita no mundo no ciclo só transiciona com
  `kira approve` humano — budget estourado e `--apply` isolado não são
  aprovação.

## 7. Rendezvous por `UPDATE … RETURNING`: claim concorrente não duplica

- **Onde**: `src/store/posts.rs` — o claim da fila é um único statement:
  `UPDATE posts SET state = 'picked', draft_ttl = ?2, attempts = attempts + 1
  WHERE ... state = 'sealed' OR (state = 'picked' AND draft_ttl vencida)`.
- **Prova**: aceite do M2 em `tests/digest_e2e.rs`/store — dois claims não
  pegam a mesma linha; lease vencida devolve o post à fila (estado volta a
  `sealed`); migração preserva dados.
- **Por que**: sem broker (ADR-0038), a fila vive no SQLite; o UPDATE
  atômico é o rendezvous — dois processos reclamando o mesmo post é o caso
  real do `kira loop` com agendamento externo.

## 8. Idempotência do ciclo

- **Ingest**: invariante 1 (byte a byte).
- **Digest**: cache por `(input_hash, prompt_version, model, lang)` — segundo
  run não repete post (`tests/digest_e2e.rs`; `--lang` diferente não
  reaproveita cache).
- **Publish**: gate por `source_hash`/`published_hash` — publicar duas vezes
  é idempotente; página editada à mão é recusada e preservada, `--force` é a
  única porta (`tests/publish_e2e.rs`).
- **Loop**: 2º tick sem mudança deixa facts, drafts, wiki e fila byte-idênticos
  (`tests/loop_e2e.rs` — o e2e do D2). `TickReport::changed()` é pura e
  **cache e failed não contam** como mudança.

## 9. Read-only provado por hash

- **Onde**: `kira dashboard --read-only`; store sempre em
  `SQLITE_OPEN_READ_ONLY` na TUI.
- **Prova**: `tests/dashboard_e2e.rs` compara o hash do `store.db` antes/depois
  — byte-idêntico. Navegação nunca spawna, escreve ou toca rede
  (`navigation_never_returns_an_execution_outcome`); a tecla de ação só executa
  com argv allowlist (`plan_uses_allowlisted_argv_and_never_a_shell`) e
  confirmação; saída do filho sanitizada e redactada
  (`sanitize_log_line_strips_ansi_and_control`).
- **Honestidade de tela**: dado ausente é `—` com a causa, nunca inventado —
  "nada de gauge de budget enquanto não houver contador de chamadas: dado
  inventado é proibido aqui como em qualquer tela".

## 10. LLM só onde a escada acabou

- **Onde**: `tests/llm_allowlist.rs` varre `src/` e reprova módulo fora da
  lista importando `llm::*`, e reprova chamada sem o comentário
  `// LLM porque:`.
- **Prova**: o teste é o gate — `scan`, `store`, `board`, `ado`, `guard`,
  `format`, `tui`, `doctor` nunca falam com modelo. Comando determinístico
  não consome budget; pull de modelo também não.

## 11. Prompts são código versionado

- **Onde**: `src/llm/prompts.rs` (constantes públicas, revisadas em PR);
  `TRIAGE_SYSTEM_PROMPT_FNV1A64` + `triage_prompt_id()` no stderr do
  `kira triage`.
- **Prova**: fixture dourada `tests/fixtures/prompts/triage-system.txt`
  comparada byte a byte e o hash recomputado por teste — mexer no texto sem
  atualizar os dois reprova (`tests/triage_prompt_fixtures.rs`). Resposta de
  modelo 4B sem os 3 cabeçalhos ⇒ `Error::LlmFormat` (exit 1, acionável) —
  nunca três `—` com exit 0.

## 12. Schema é contrato

- **Onde**: `src/store/mod.rs`, `SCHEMA_VERSION = 10`, migração idempotente
  via `PRAGMA user_version`.
- **Prova**: `tests/trust_e2e.rs` fixa a versão exata do schema — bump quebra
  o e2e de propósito. Cada bump de migração é um passo (`if current < N`)
  renumerado no merge quando dois PRs disputam o mesmo passo.

## 13. Context pack determinístico (gate do M1)

- **Onde**: `src/context/` — caminhada ordenada por nome, ranking fixo
  (arquivo citado em `findings.json` primeiro, depois termos casados, caminho
  e linha), orçamento `KIRA_NUM_CTX` com prioridade fixa.
- **Prova**: `tests/context_e2e.rs` roda o binário real e fixa o contrato no
  limite do CLI: o comando imprime o pack que iria ao prompt — sem modelo,
  sem rede — com a fonte marcada (`index`/`grep`), cada trecho ancorado
  `path:linha` e as vistas `--explain`/`--json`. O determinismo ("mesmo
  estado ⇒ mesmo pack") é o gate do M1 declarado no contrato do repo
  (AGENTS.md); o `--json` serializável é a entrada para medir recall sem
  tocar no LLM.
- **Ancoragem**: todo trecho sai com `caminho:linha` — blob anônimo no prompt
  é proibido (ADR-0015). Ordens de prioridade e "estourou vira nota" em
  [[glossario]].

## 14. `pr draft` sem invenção

- **Onde**: `src/prdraft.rs` — evidência só entra se veio de dado real
  (sessão, finding ou wiki); seção sem dado sai como
  `— (não há dado; rode kira triage <id> antes)` com **exit 0** (falta de dado
  não é erro).
- **Prova**: `tests/prdraft_e2e.rs` — anti-invenção, `path:linha` só quando o
  achado traz linha, texto inteiro redactado, template fixo sem `now()`
  (mesma entrada = mesma saída byte a byte), comando read-only (não grava
  sessão, não toca rede nem LLM).

## 15. Episódio de wiki: grava o que vale

- **Onde**: `src/wiki/` — o `kira triage` anexa episódio em
  `.kira/wiki/bugs/<item-id>.md` sem perguntar nada.
- **Prova**: `tests/wiki_e2e.rs` — episódio **vazio** (as 3 seções sem
  conteúdo) não grava (a falha fica só na trilha); episódio **repetido**
  não duplica; episódios com mais de 30 dias colapsam numa linha em
  `## Histórico colapsado`; `forget` apaga markdown + índice, página
  inexistente é exit 0 (LGPD: dado do dono); falha de limpar o índice vira
  aviso, nunca erro.
- **Ciclo autônomo**: usa `.kira/wiki-staging/` (`EpisodeTarget::Staging`),
  nunca a wiki do dev — promoção é `kira wiki promote`, humana.
- **Onda TE, proposta D1**: hit de cache não grava episódio novo — página de
  bug não é log de re-execução; o valor de "acertei de novo" pertence ao
  ledger, não à wiki.

## 16. Ferramenta é resolvida, não adivinhada

- **Onde**: `src/binresolve.rs` — PATH × PATHEXT no Windows, bit de execução
  no Unix; spawn sempre pelo caminho completo resolvido.
- **Prova**: `tests/scan_e2e.rs` cobre os caminhos "ferramenta ausente"
  (sem overrides e com PATH vazio), "presente que falha" (shim exit 1 apontado
  por `KIRA_SCAN_*_BIN` — sem shadowing de PATH) e "nenhum achado" com
  arquivo determinístico. Extração dos JSON dos audit é função pura testada
  com fixtures (nenhuma ferramenta real em teste).
- **A lição que originou**: classificar por texto de OS/cmd localizado é
  proibido — `'npm' não é reconhecido` em pt-BR virava "ferramenta falhou".

## 17. Toda escrita de página confinada

- **Onde**: `src/guard/path.rs` + `redact` — toda escrita na wiki, no índice
  e nos skills passa pelas duas portas.
- **Prova**: id inválido (`..`, absoluto, maiúscula, slug fora de
  `[a-z0-9_-]`) é recusado antes de qualquer I/O (`tests/wiki_e2e.rs`);
  o `trust.json` é o aceite, e o `.kira/` nasce com `.gitignore` próprio —
  `.kira/` é local, nunca comita (mecanismo fechado pelo PR37 da onda TE).

## Anti-padrões recusados (com o porquê registrado)

- **Broker externo (Redis/Postgres/AMQP)** — fura o local-first (ADR-0030),
  exige serviço no Windows e não resolve nada que uma tabela não resolva;
  `apalis` puxaria `sqlx` incompatível com o `rusqlite` compilado (`links =
  "sqlite3"` — dois pacotes linkando a mesma lib nativa).
- **Embeddings antes de bm25** — ADR-0021 decidiu FTS5 + bm25 por pesquisa:
  para recuperação de *linha exata* de código, `caminho:linha` com custo zero
  por consulta ganha; embeddings voltam só se a medição (`--eval`) mostrar
  recall abaixo do alvo. Diretiva do dono: nada de vetor.
- **Daemon/serviço residente** — estado no SQLite, tick idempotente,
  agendador externo chama o binário ([[decisoes]], decisão 5).
- **LLM como juiz do gate** — ADR-0018: comparação determinística contra gold
  do dono; painel de jurados só sob comando explícito, nunca no ciclo/gate.
- **Fallback silencioso do backend caro** — `--llm copilot` sem CLI falha
  com erro acionável; a ausência nunca vira Ollama sem que você perceba.
- **Dado inventado em tela** — `—` com a causa, sempre; gauge sem contador é
  dado fabricado.
- **Código GPL no repo MIT** (ex.: `ai-jail`) — conceito sim, código não.
