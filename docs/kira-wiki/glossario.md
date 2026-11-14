# Glossário

Espelho do glossário canônico selado em `docs/nomenclatura.md` (a fonte de
verdade — este espelho é leitura) + os termos de RAG/medição que a wiki usa.
Regra-mestra: **um nome por conceito, e o nome diz o que a coisa é**. Código,
comandos e identificadores sempre em inglês; docs, fala e UI em PT-BR.

## Conceito → nome canônico → aposentados

| Conceito | Canônico | Aposentados |
|---|---|---|
| lê os fatos do repo (sem LLM) | `ingest` | — |
| gera os posts (LLM) | `digest` | — |
| publica os posts na wiki | `publish` | `emit` |
| escada de arquitetura | `prompt → skill → loop → graph → harness → system design` | — |
| orquestrador interno (runtime) | `brain` (`src/brain/`) | `master` |
| agente de build externo | `Hermes` | `Orchestrator` |
| codificador (build) | `Delta` | — |
| ferramenta de design do dono | Kombai (estaleiro, não componente) | — |
| grafo de estados do Brain | `graph` | — |
| loop que repete (o ciclo da fila) | `turn` | `Postmaster` |
| uma ação dentro do turno (`claim → generate → deliver`) | `phase` | `Courier`, `step` |
| tempo de vida do draft no claim | `draft_ttl` | `Postage`, `lease`, `postage_until` |
| post que falhou | `failed` | `DeadLetter`, `dead-letter` |
| unidade de trabalho | `post` | `artigo`, `carta`, `letter` |
| artefato do `ingest` | `.kira/facts.md` | `.kira/digest.md` |
| rascunhos do `digest` | `.kira/drafts/` | `.kira/digest/` |
| conhecimento publicado | `.kira/wiki/` + `.kira/wiki-staging/` | `docs/wiki/` |

**Hierarquia do loop**: `graph → turn → phase` — o grafo tem nós; cada nó
pode ter um turno (o loop que repete, como turno de jogo de carta); cada
turno tem fases (verbos técnicos: `claim`, `generate`, `deliver`).

**Estaleiro não é sistema**: `Hermes`/`Delta` (agentes de build) e Kombai
(design) **constroem** o KIRA com o dono — nunca aparecem em diagramas C4,
atores de sistema ou fluxos de runtime ([[decisoes]], decisão 3).

**Estado da fila**: `sealed` (na fila) → `picked` (em claim, com `draft_ttl`)
→ `delivered` (draft gravado, aguardando publish; `read_at` marca "lido")
ou `failed`. Renames de código são PR isolado com gate verde e nota de
migração — renomear comando/coluna/estado quebra compatibilidade.

## RAG e contexto

- **packer (context packer)** — monta o prompt: item + findings + trechos do
  repositório + páginas da wiki, dentro do orçamento de tokens
  (`KIRA_NUM_CTX`, default 4096). Ordem fixa de prioridade: brief do scan →
  achados individuais → páginas da wiki → trechos do repo. Orçamento
  estourado vira nota, nunca erro.
- **retrieval-first** — o KIRA busca, o modelo só interpreta. Todo trecho sai
  ancorado (`caminho:linha` no repo; caminho + idade em dias na wiki) — blob
  anônimo no prompt é proibido (ADR-0015).
- **FTS5/bm25** — busca full-text do SQLite com ranking bm25. Índices
  (`wiki_fts`, `repo_fts`, `skill_fts`) são derivados e descartáveis: a fonte
  de verdade é o markdown (wiki/skills) ou os arquivos do disco (repo).
- **`repo_fts`** — índice do código: uma linha de FTS5 por linha de código
  **já redigida** (`redact` na escrita), incremental por `(mtime, size)`,
  teto `KIRA_INDEX_MAX_FILES` (default 5000).
- **fail-open (retrieval)** — índice ausente ⇒ cai no grep determinístico e
  marca a fonte (`index`/`grep`) na saída.
- **`context --explain`** — mostra por que cada trecho entrou no pack (rank
  bm25, prioridade de finding, fonte).
- **cache-wiki (cache de raciocínio)** — rota do triage (onda TE, ADR-0028):
  episódio de bug anterior com similaridade de tokens ≥ 0,50 responde **sem
  LLM** (`llm_calls=0`), fonte marcada, desligável com `--no-cache`.

## Medição e orçamento

- **gold (`triage_gold`)** — rótulos verdadeiros do dono: 20 bugs fechados em
  pastas com `item.json` + `gold.json` (causa raiz, arquivos, confiança).
  O gate M2 (`kira triage --eval`) compara contra o gold, determinístico,
  limiar ≥ 70% — sem LLM como juiz. Sem o gold, nada de "autônomo".
- **`llm_calls`** — contador de chamadas de LLM (schema v9): backend, model,
  command, tokens estimados, data. Base do budget de 50/dia (default
  `max_llm_calls_per_day`). Pull de modelo e comando determinístico não
  consomem.
- **ledger de economia (onda TE)** — pack chars vs teto, fonte, chamadas de
  LLM e sinal de pack ocioso, por sessão. Estimador de tokens:
  **3 chars/token** (`CHARS_PER_TOKEN`, PT-BR é mais denso — conservador;
  subestima a economia, nunca a infla).
- **barra de qualidade** — filtro da casa (do RTK): toda estratégia nova de
  corte do packer só entra se o fixture mostrar corte ≥ 20%.

## Modelos e instalação

- **GGUF / quant** — formato de modelo quantizado do Ollama/Hugging Face.
  Piso da casa: **Q4_K_M** — Q2_K proibido por política (degradação abaixo do
  aceitável para triage). Modelo default: `qwen3.5:4b` (~3,3 GB, Q4_K_M).
- **catálogo HF** — Hugging Face entra como catálogo de GGUF consumido via
  Ollama (`kira model list`/`kira model pull`), nunca runtime de inferência
  (ADR-0040).
- **gate de admissão de modelo** — proveniência conhecida + SHA-256 conferido
  + template de chat known-good + Ollama ≥ 0.17.1 + probes de segurança no
  GGUF quantizado (não no modelo de papel) + budget compartilhado.
- **SHA256SUMS / install fail-closed** — instalador para antes de gravar
  qualquer arquivo se o hash não bater; pin por `KIRA_PINNED_SHA256` exige
  autoria, não só transporte.

## Nomes de artefato e mecanismo

- **`facts.md`** — artefato do `ingest`: árvore + perfil de stack + até 12
  arquivos-chave com `path` e linhas redigidas. Determinístico (sem
  timestamp, POSIX, ordenado); arquivo/binário acima do cap aparece como
  nota, nunca somem calados.
- **`findings.json`** — achados do `scan`: `tool, rule, package, version,
  severity, path, message, fingerprint`. `.kira/` é local, nunca comita.
- **`outbox`** — view dos posts pendentes na fila do digest (a tabela é
  `posts`).
- **`source_hash` / `published_hash`** — o par que faz o `publish` honesto:
  o post carrega o hash do estado do repo de onde nasceu; a página publicada
  carrega o hash do que foi gravado. Repo mudou ⇒ post desatualizado é
  detectável (o `doctor` lista); página editada à mão ⇒ hash não bate ⇒
  publish recusa.
- **`inventory.json` / `tree.txt`** — artefatos da caminhada em
  `.kira/ingest/` (iguais para o mesmo estado).
- **staging / promote** — `.kira/wiki-staging/` é o destino do ciclo
  autônomo; `kira wiki promote` é a promoção humana para `.kira/wiki/`.
  Nada cruza por si (mesma regra da promoção de lições entre projetos,
  ADR-0027).
- **episódio** — página de bug em `.kira/wiki/bugs/<item-id>.md` gravada pelo
  `kira triage`: causa raiz prevista, arquivos, confiança, brief do scan.
  Episódio vazio não grava; repetido não duplica; >30 dias colapsa em
  `## Histórico colapsado`. `kira wiki forget` apaga (LGPD: dado do dono).
- **trust (`trust.json`)** — aceite do repositório, com hash do caminho
  canônico. O gate que separa "ler" de "mandar código ao modelo".
- **`briefing`** — `.kira/briefing.md`, escrito a cada `digest`: estado da
  fila e o próximo passo. Honestidade em arquivo.
- **redact** — máscara de PAT/token/CPF/e-mail **antes** de log, erro, prompt,
  índice e wiki. Redação na escrita, não na leitura.
- **jail** — confinamento de escrita: `guard/path` (deny-list + traversal, id
  de página recusado antes de I/O), canonicalização + rlimits no Unix (PR27),
  symlink/junction com `lstat` e validação do alvo final (PR47).
- **fingerprint** — FNV-1a 64: do achado do scan
  (`tool|rule|package|version|message`), do id de projeto (root canônico) e
  do prompt de triage (`TRIAGE_SYSTEM_PROMPT_FNV1A64`). A marca do
  determinismo.
- **keyring** — cofre de credencial do OS (service `kira`): PAT e chave
  Foundry. Nunca arquivo pleno, log, `Debug` ou prompt de LLM.
- **capabilities** — integração como estado por projeto
  (`desconectada | conectada | inválida`); comando sem capacidade falha com
  `esta jornada precisa de <capacidade> — rode: kira connect <provider>`
  (ADR-0030). Presença de credencial na env não é consentimento.
- **stale** — último resultado bom mantido visível depois de falha de quota
  (429 intocado por 5 min); o `doctor` mostra a idade do último sucesso.

## Ciclo e fila

- **tick** — uma passada do `kira loop`: `ingest → digest (só se trusted) →
  publish`. `--once` roda um tick. Ponto fixo: "estável: nada a fazer"
  (`TickReport::changed()` pura — facts mudaram / post novo / publicação
  nova; cache e failed não contam).
- **egresso** — resultado que sai do ciclo para o mundo (board, PR, wiki
  promovida). O egresso **nunca é direto**: sai por gate
  (`awaiting_approval` + `kira approve`, ou `kira wiki promote` do staging) —
  leitura é livre, escrita nunca.

## Como usar o glossário

Código, comandos, identificadores e colunas de DB: **sempre inglês**. Docs,
fala ao usuário e UI: **PT-BR**. A ideia de **turno** (jogo de carta) vive na
doc/UI; os verbos das fases são técnicos (`claim`, `generate`, `deliver`).
A fonte de verdade deste espelho é `docs/nomenclatura.md` — se este arquivo
divergir, ele está errado.
