# ADR-0038 — Ingestão, digestão e emissão: o caminho do repositório até a wiki

**Status:** aceito (decisões do dono, 12–13/Nov/2026). Itens marcados *(provisório)* são
recomendação do agente sujeita a veto — o executor seguinte não deve tratá-los como lei.

## Contexto

O KIRA "cru" entrega `setup → scan → triage --file → pr draft → dashboard`: percebe, julga,
escreve e mostra. O que ele **não** faz é conhecer o projeto no momento da instalação — o
resumo do setup ainda manda o dev rodar `scan`, e nenhum artefato de projeto nasce ali.

Foram estudados três produtos antes desta decisão:

- **gitingest** — ingere um repositório com *denylist* + limites e despeja o conteúdo inteiro
  num documento. Os números dele (`MAX_FILE_SIZE = 10 MB`, `MAX_FILES = 10_000`,
  `MAX_DIRECTORY_DEPTH = 20`, `MAX_TOTAL_SIZE_BYTES = 500 MB` rotulado no fonte como *"Maximum
  size of output file"*) valem para **um dump**, não para um índice.
- **repomix** — pack com orçamento de token; mesma natureza de saída.
- **caveman** — comprime o que já entrou no contexto (TOON, logs, HTML, axtree, pixels). É
  camada **complementar**: o KIRA produz contexto curado, o caveman economiza o que sobe ao
  modelo.

Medição que ancorou a decisão: o próprio repositório do KIRA tem 181 arquivos indexáveis,
3,5 MB e 75 k linhas, com caminhada completa em **14 ms**; o `target/` ocupa 4,5 GB. Ou seja,
orçamento em *bytes do diretório* mede a coisa errada, e custo de caminhada não é risco.

## Decisão

1. **Três verbos, três contratos.** `kira ingest` (fatos, determinístico, sem LLM, segundos),
   `kira digest` (julgamento com IA, cacheado, retomável), `kira publish` (publica o `.md` no
   wiki, atrás de gate). Só o `ingest` entra no `kira setup`.
2. **Domínio `post`.** A unidade de trabalho **é** o artigo: tabela `posts`, view `outbox`
   para os pendentes, fase `phase` (verbos técnicos: `claim → generate → deliver`), lease
   `draft_ttl`, falha `failed`, loop `turn` (ADR-0039 aposenta a lore de correio).
   Identificadores em inglês; a lore (carta, selo, carteiro) vive na doc e na UI. `letters`
   foi descartado: dois nomes para o mesmo objeto é o começo de um deles divergir.
3. **Ingestão completa em qualquer stack:** *denylist* de padrões (não allowlist de extensões),
   detecção de binário por sniff, teto por arquivo **com nota** (nunca skip silencioso) e
   detector de stack por marcador (`go.mod`, `pubspec.yaml`, `*.sln`, `build.gradle.kts`,
   `app.json`, `Package.swift`, `terraform/`). Substitui a allowlist de 34 extensões e os 14
   diretórios fixos; `packages/` deixa de ser pulado e a raiz homônima (`build`, `bin`) deixa
   de zerar o índice.
4. **Idioma é dado, não código.** `.kira/**`, prompts e contratos internos **sempre em inglês**.
   O que o humano lê — posts, cards do board, título e corpo de PR — usa `output.lang`,
   escolhido no `kira setup` (flag `--lang <bcp47>` no não-interativo), com override por
   projeto. `lang` entra no `cache_key` junto de `input_hash`, `prompt_version` e `model`.
5. **Fila é tabela, não broker.** Uma tabela no `store.db` (SQLite `bundled`, que o Kira já
   embarca) com claim por `UPDATE … RETURNING` e lease. Sem Redis/Postgres: fura o local-first
   (ADR-0030) e não roda nativo no Windows. `read_at` marca "não lido"; o sino ao vivo é
   `tokio::broadcast`, nunca a fonte da verdade.
6. **LLM local primeiro.** `ollama` é o backend do `digest`; Foundry é capacidade opcional;
   `copilot` nunca é implícito. O `ingest` não importa `llm::*` (a allowlist do teste de
   política já reprova).
7. **Orçamento explícito.** 1 M tokens por run de `digest` (é leitura inicial de projeto
   grande) **e** teto por post (30 k), com parada dura e nota — sem teto por post um único
   artigo guloso consome o run.
8. **Trust do repositório.** O `ingest` é read-only e roda sempre; o `digest` — que envia
   código ao modelo — só roda com o repositório aceito pelo dev (`.kira/trust.json` com hash
   do caminho). Sem trust, nada é enviado.
9. **Determinismo onde há determinismo.** `facts.md` e `inventory.json` são byte-idênticos
   para o mesmo estado: sem timestamp, caminhos relativos POSIX, tudo ordenado, redação na
   escrita (ADR-0021), `digest::render` puro (sem I/O). O que passa por LLM fica em
   `.kira/drafts/` e é identificado por `model` + `prompt_version` + `input_hash`.

## Consequências

- O trabalho se divide em 5 blocos (M1–M5) em `docs/BUILD-PROGRAM.md`, um PR por bloco.
- `SOURCE_EXTENSIONS`/`SKIPPED_DIRS` viram implementação de `context::ignore`; o teste que
  prova a política é o mesmo que roda no índice e no grep.
- A wiki deixa de ter "seed só se ausente": cada post carrega `source_hash`, então
  desatualização fica **detectável** e vira fila (`doctor` lista os posts velhos).
- A fila W5 (`loop`/`sentinel`) **não** é tocada: continua atrás do gate M2/B1.

## Alternativas rejeitadas

| Alternativa | Por que não |
|---|---|
| Allowlist de extensões (estado anterior) | não cobre stack desconhecida — 3 dos 11 repositórios de referência usavam `.mts`, `.xml` ou `.txt` de manifesto |
| `MAX_TOTAL_SIZE_BYTES = 500 MB` do gitingest | é teto de *dump*; o Kira não despeja conteúdo — mediria a coisa errada e estouraria em qualquer repo com `target/` |
| Broker externo (Redis, Postgres, AMQP) | quebra local-first/ADR-0030, exige serviço no Windows e não resolve nada que uma tabela não resolva |
| `apalis` + `apalis-sqlite` | puxa `sqlx 0.9` → `libsqlite3-sys < 0.38`, incompatível com o `^0.39` do `rusqlite` que o Kira compila (`links = "sqlite3"` ⇒ dois pacotes linkando a mesma lib nativa) |
| Implementar gitwildmatch completo agora | risco alto, ganho baixo antes de existir digest; o subset entra quando um repositório real mostrar necessidade (crate `ignore` fica registrado como alternativa) |
| Embeddings em vez de FTS5 | ADR-0021 já decidiu bm25; nada neste ADR muda isso |
| Traduzir post em runtime | custa uma chamada extra e desvia do texto aprovado; template por idioma é mais barato e determinístico |
