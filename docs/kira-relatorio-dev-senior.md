# KIRA — relatório para dev sênior: o que este código é, e como se compara aos anexos

- **Data**: 13/Nov/2026
- **Fontes**: `kira @ 4a30051` (main, 13/Nov/2026 15:35 -03 — inclui a onda de
  nomenclatura canônica, ADR-0039, e o doc-pai [`kira-brain.md`](kira-brain.md)) ·
  snapshots anexados `agent-skills @ e74dc8a` (7.157★, MIT + CC-BY-4.0) e
  `harness-toolkit @ d293c7f` (310★, Elastic License 2.0), lidos diretamente dos
  clones locais. Nomes usados neste relatório seguem o glossário canônico
  ([`docs/nomenclatura.md`](nomenclatura.md)): `publish`, `facts.md`, `drafts/`,
  `draft_ttl`, `failed`, `turn`/`phase`.
- **Método**: código lido e números contados nos três repositórios (82 arquivos Rust
  contados um a um; gates `fmt`/`clippy` rodados hoje; CI lida no workflow). Análises
  anteriores do dono continuam valendo:
  [`benchmark-kira-vs-techleads.md`](benchmark-kira-vs-techleads.md) (rubrica do
  mandato) e [`agent-skills-harness-vale-trazer.md`](agent-skills-harness-vale-trazer.md)
  (o que copiar). Este relatório **não repete** a rubrica — responde outra pergunta:
  *"o que eu estou construindo, afinal?"* — e compara os três pelo ângulo que só o
  KIRA tem: **autonomia**.

---

## 1. TL;DR

Você está construindo um **organismo de conhecimento por projeto**: um binário Rust
único, 100% local-first, que percebe o repositório (scan/ingest), guarda o que
aprendeu em memórias estruturadas (SQLite + wiki em markdown), julga com modelo
**local por padrão**, escreve relatórios/artigos em uma fila com `draft_ttl` e publica
quando o dono autoriza — tudo com determinismo contratual (mesma entrada = mesmo
arquivo, byte a byte) e um sistema imune (redação de segredo, jail de caminho,
trust gate, budget).

**O corpo já tem quase todos os órgãos; o coração não.** O metabolismo completo
(`ingest → trust → digest → publish`) existe e é testado (onda M, PRs #84–#87
mergear, com a renomeação canônica da onda ADR-0039 em #89–#95). O que falta
para "rodar sozinho" é o **Brain** (`src/brain/`, desenhado em
[`kira-brain.md`](kira-brain.md)): o grafo de estados que amarra
`ingest → digest → publish → triage → fix` — mais a casa (imagem Docker) e o
combustível da medição (gold do triage, que continua vazio). Sem o Brain, o
organismo hoje é um **motor de estação**: excelente sob comando, mas não pulsa
sozinho.

**Vs. os anexos**: os dois são ferramentas **dirigidas por humano** — o
agent-skills é um registry de 94 skills que o dev invoca; o harness-toolkit é
enforcement que reage a hooks do Cursor/Claude Code. Nenhum dos dois tem ciclo
próprio, fila com `draft_ttl`, cache de raciocínio por hash, trust gate ou wiki
com detecção de staleness. O KIRA é o único dos três desenhado para **viver sem
humano no laço** (com guarda de aprovação para toda escrita). Em contrapartida,
eles são mais maduros em conhecimento empacotado (94 skills), enforcement em
tiers com veredito tipado e registro de decisões (139 ADs) — é exatamente onde a
fila de adoção (PR44–PR51) mira. Na escada do próprio KIRA
([`kira-brain.md`](kira-brain.md)): **agent-skills é o degrau "skill"** (com a
melhor custódia de prompt dos três) e **harness-toolkit é o degrau "harness"**;
o KIRA está construindo a **escada inteira como módulos do binário**.

---

## 2. A tese em uma frase

> "Um dev/arquiteto companheiro, vivo, que olha o projeto continuamente e evolui
> devagar — um 'mini Sérgio' rodando na máquina do dev, inclusive enquanto ele
> dorme." (`docs/kira-persona.md` §2)

Cada palavra dessa frase tem um módulo responsável. O mapa completo está na §3–4.

---

## 3. Resumo do código (o que o binário tem hoje, verificado)

### 3.1 Números

| | KIRA | agent-skills | harness-toolkit |
|---|---|---|---|
| linguagem | Rust 2024, binário único | TypeScript (Node ≥24, Nx monorepo) | TypeScript (Bun/Node ≥24, zero deps runtime) |
| código (não-teste) | **82 arquivos, ~36,2k linhas** | ~221 arquivos, ~17,8k LOC + 94 skills | ~165 arquivos, ~19,3k LOC |
| testes | 21 arquivos e2e + unit (~9,3k linhas) — **605 unit tests, 0 falhas** (13/Nov) — todo e2e roda o **binário real** | Jest 30 + fast-check + e2e MCP | Node runner, 180 arquivos, ~27,2k LOC |
| deps runtime | rusqlite bundled, keyring, reqwest, clap, ratatui, walkdir, regex | runtime pesado (React 19 no TUI, Next no marketplace) | **zero** |
| CI | unit nos 3 SOs sempre + e2e Linux sempre + Windows/macOS no push main | CI + Snyk Agent Scan bloqueante | 4 plataformas + SLSA v1 |
| licença | MIT | MIT + CC-BY-4.0 (skills) | **Elastic License 2.0** (não copiável) |

### 3.2 Arquitetura por domínio (módulo × papel × invariante)

| Domínio | Módulo | Papel | Invariante que o código prova |
|---|---|---|---|
| **Perceber** | `scan/` (dotnet, node) + `binresolve` | roda as ferramentas padrão da stack e normaliza findings | determinístico: fingerprint FNV-1a, ordem estável; ferramenta ausente = "instale o X", sem spawn |
| | `ingest/` (mod, inventory, digest) + `context/ignore` + `stack` | caminhada completa com denylist + sniff binário + detector de stack por marcador | mesmo estado ⇒ `facts.md` byte-idêntico; nada some calado (cap vira nota) |
| **Lembrar** | `store/` (mod, posts) | SQLite bundled, WAL, migração idempotente por `user_version` | 9 tabelas: `projects, sessions, facts, wiki_page, wiki_fts, repo_file, repo_fts, capabilities, posts` (+ view `outbox`) |
| | `wiki/` (mod, episode) | markdown local + índice FTS5 **derivado e descartável** + episódios de bug | episódio vazio não grava; repetido não duplica; >30 dias colapsam |
| | `context/` (mod, repo, index) | context packer retrieval-first: bm25 do `repo_fts`, orçamento fixo | mesmo estado ⇒ mesmo pack; FAIL-OPEN: índice ausente ⇒ grep determinístico |
| **Julgar** | `llm/` (ollama, copilot, foundry) + `prompts` | 3 backends plugáveis; prompts como `const` versado no repo | caro nunca implícito, nunca fallback silencioso; versão do prompt + hash FNV fazem parte do contrato |
| | `triage.rs` + `eval.rs` | predição de causa raiz + **gate M2 medido** contra gold do dono | sem LLM como juiz; similaridade de tokens ≥ 0,50; exit code é o gate |
| **Agir** | `post/` (mod, plan, prompts) = o **turn** | fila de posts: fases `claim → generate → deliver`, cada fase com verbo técnico; claim com `draft_ttl` de 120 s | `cache_key = (input_hash, prompt_version, model, lang)` — retomar não refaz, trocar idioma regenera; `attempts ≥ 3` vira `failed`, nunca loop infinito |
| | `publish/` | publica rascunho na wiki com front-matter e índice derivado | idempotente; `source_hash` vs `published_hash` detecta staleness; post editado à mão é recusado sem `--force` |
| | `prdraft.rs` | rascunho de PR com evidência literal do store/findings/wiki | anti-invenção: dado ausente vira `—`, nunca é inventado |
| **Proteger** | `redact.rs` | mascara PAT/token/e-mail/CPF **antes** de qualquer prompt, log ou índice | caracteres invisíveis/bidi removidos antes do regex (senão o segredo vaza) |
| | `guard/path.rs` | jail de caminho: `../`, NUL, absoluto, `.git/` recusados antes de I/O | nada escapa do root |
| | `credentials.rs` | PAT só em env ou keyring do OS | `KIRA_SECRET_STORE=off` para teste; nunca em log/`Debug`/config |
| | `trust.rs` | aceite do repositório (`.kira/trust.json` com hash do caminho) | `ingest` roda sempre; `digest` — que envia código ao modelo — **recusa sem trust** |
| **Mostrar** | `tui/` (dashboard Ratatui) | 4 painéis read-only; execução de ação só após tecla + confirmação | `decide()` pura e testada; read-only provado por hash byte-idêntico do store no e2e |
| | `doctor.rs`, `capabilities.rs` | diagnóstico por item com comando de correção; capacidades conectadas por projeto | falha vira instrução, não stack trace |

Dois testes de **política** merecem destaque para um dev sênior: `llm_allowlist`
(reprova o build se qualquer módulo fora de `triage`/`loop`/`wiki` importar
`llm::*` — a arquitetura "LLM é o último recurso" é checada pelo compilador) e
`no_runtime_unwrap` (proibição de `unwrap`/`expect` em runtime, mecanicamente).

### 3.3 O metabolismo — ADR-0038 (o caminho do repositório até a wiki)

```
repo ──kira ingest──▶ fatos ──kira trust──▶ aceite ──kira digest──▶ fila de posts
                      (sem LLM,             (hash do path)          (turn: claim/generate/deliver)
                       determinístico,                              │
                       ~14 ms no próprio repo)                      ▼
                                                            .kira/drafts/<slug>.md
                                                                   │
repo muda ──▶ ingest de novo ──▶ source_hash difere ──▶ ──kira publish──▶ .kira/wiki/<slug>.md
                                                            (gate por hash; staleness detectável;
                                                             idempotente; nunca sobrescreve editado)
```

O artefato determinístico do `ingest` é o `.kira/facts.md` (antes `digest.md` —
renomeado na ADR-0039; os drafts vivem em `.kira/drafts/`). Pontos finos que um
dev sênior vai apreciar:

- **Fila é tabela, não broker** (ADR-0038, decisão 5): claim por `UPDATE … RETURNING`
  com `draft_ttl` de 120 s — dois claims não pegam a mesma linha; processo morto
  devolve o post sozinho; `attempts ≥ 3` vira `failed` (não retry infinito). Sem
  Redis/Postgres de propósito: fura o local-first e não roda nativo no Windows.
- **Orçamento em dois níveis**: 30 k tokens por post (senão um artigo guloso come o
  run) e 1 M por run, com parada dura **e nota** — nunca erro silencioso.
- **Idioma é dado**: `lang` entra no `cache_key`; o prompt é sempre inglês, o
  artefato segue `output.lang` do dono.
- **Ingestão por denylist** (não allowlist de extensão): stack nova entra de graça
  — Go, Flutter, Swift, Terraform detectados por marcador de manifesto.

### 3.4 As memórias

| Onde | O que vive | Propriedade |
|---|---|---|
| `~/.config/kira/store.db` | sessões, fatos, posts, capacidades, índices FTS5 | derivável/reconstruível menos sessões e fatos |
| `<repo>/.kira/wiki/` | conhecimento durável em markdown (episódios, decisões, posts publicados) | **fonte de verdade**; índice é descartável |
| `<repo>/.kira/findings.json`, `facts.md`, `ingest/*`, `briefing.md`, `trust.json` | artefatos determinísticos | byte-idênticos no mesmo estado; sem timestamp |
| keyring do OS | PAT / chave do Foundry | nunca em arquivo, log ou prompt |

Regra de ouro do store: **fail-open obrigatório** — erro de trilha nunca quebra o
comando; vira `aviso:` no stderr e o comando continua.

### 3.5 Os reflexos de segurança (a "pele")

1. Redação **na escrita** do índice (toda linha de código passa por `redact` antes
   de entrar no `repo_fts` — o store nunca guarda segredo nem em dado derivado).
2. Prompt delimitado: conteúdo do repo/work item entre marcadores de não-confiável
   com regra anti-instrução no system (`prompts.rs` v2, ADR-0024).
3. Trust gate: o comando que envia código ao modelo existe, mas só roda com aceite.
4. Budget com número (`max_llm_calls_per_day`), checado antes de transição.
5. TUI read-only por padrão, ação confirmada por tecla, hash do store no e2e.

### 3.6 Qualidade (rodado hoje, nesta máquina)

| Gate | Resultado (13/Nov/2026) |
|---|---|
| `cargo fmt --check` | ✅ OK |
| `cargo clippy --all-targets -- -D warnings` | ✅ OK (exit 0) |
| `cargo test --lib` | ✅ **605 passed, 0 failed** |
| `cargo test` (e2e) | ver CI — e2e sempre no Linux, Windows/macOS no push de main |

Os 21 arquivos de e2e cobrem os caminhos maus de propósito: `security_probe`
(injeção de prompt no work item, PAT invisível, caminhos hostis), `ado_live_errors`
(200/401/403/sem rede com mensagens exatas), `scan_e2e` (ferramenta ausente,
ferramenta que falha, duas execuções byte-idênticas), `dashboard_e2e` (hash do
store), `eval_e2e` (o gate medido). O CI é honesto sobre custo: comentário no
`ci.yml` explica por que Windows só roda e2e completo no push de main (27–31 min
de linkação de 19 binários de teste).

---

## 4. O "ser vivo" — órgão por órgão (o que pulsa e o que não)

| Órgão | Status | Evidência |
|---|---|---|
| **Metabolismo** (ingere → digere → publica) | ✅ vivo | M1–M5 mergear (#84–#87); `post/mod.rs`, `publish/mod.rs` |
| **Memória** (curto prazo = store; longo prazo = wiki) | ✅ | schema v8; wiki com FTS5 + episódios |
| **Sistema nervoso** (dashboard) | ✅ | `tui/app.rs` (2.259 linhas), read-only provado |
| **Sistema imune** (redact, guard, trust, budget) | ✅ | `security_probe` e2e |
| **Reflexos** (scan, prdraft, triage sob comando) | ✅ | e2e de cada caminho |
| **Coração — pulsa sozinho** (`kira loop`, ciclos com estado no SQLite) | ❌ PR15, não implementado | F2 no `BUILD-PROGRAM.md` |
| **Ritmo cardíaco** (agendamento opt-in) | ❌ PR16 | idem |
| **Apetite controlado no ciclo** (budget checado antes de cada transição) | ❌ PR17 | config existe; enforcement no ciclo não |
| **Juíza da escrita** (`awaiting_approval` + `kira approve`) | ❌ PR18 | ADR-0017 desenhado; sem código |
| **Exames de rotina** (`kira audit --weekly`) | ❌ PR19 | idem |
| **Escada/cérebro** (Brain: `src/brain/`, grafo `ingest→digest→publish→triage→fix`) | ⏳ desenhado, sem código | [`docs/kira-brain.md`](kira-brain.md) §4–5 (leis do Brain) |
| **Aprender de fora** (skills bundled + knowledge de livros/artigos) | ⏳ planejado (onda SK) | [`docs/skills-rag-plano.md`](skills-rag-plano.md) |
| **A casa** (Docker local: binário + Ollama + volume `.kira/`) | ❌ nada no repo | decisão a tomar — ver §7 |
| **Balanço** (gold do triage rotulado) | ❌ bloqueado pelo dono | `tests/fixtures/triage_gold/` vazio |

**Leitura honesta**: a tese "roda sozinho gerando análises e posts" está **~70%
pronta** — o que falta é menos código do que parece (a fila já é retomável,
pausável e idempotente; o loop é um agendador de ciclos *em cima* dela), mas é o
pedaço que transforma ferramenta em organismo. E o bloqueio real não é código: é
o gold do M2, que só o dono rotula. Sem número medido, autonomia é ficção — a
juíza independente do mandato já disse isso e o `BUILD-PROGRAM.md` repete.

---

## 5. Os anexos — o que cada um é (resumo para comparação)

### 5.1 `agent-skills` (Tech Leads Club) — a biblioteca com cadeia de custódia

Registry/CLI/MCP/marketplace de **94 skills** (14 categorias) instaláveis em **19
agentes** (Cursor, Claude Code, Copilot, Windsurf, Cline, Codex, Gemini CLI,
OpenCode…). Arquitetura hexagonal (`libs/core` com ports/adapters), Nx monorepo,
Next.js no site.

- **O que impressiona**: a **cadeia de custódia de prompt** — `generate-registry`
  grava SHA-256 por skill; o MCP recomputa o hash agregado e recusa divergência;
  lockfile do CLI com hash por skill, escrita atômica e audit log JSONL append-only;
  **Snyk Agent Scan bloqueante** no publish, com cache incremental e allowlist com
  validade; threat model próprio no `SECURITY.md`; instalação defense-in-depth
  (sanitização → contenção de path → symlink `lstat`/junction/loop).
- **O que NÃO é**: não é agente. Não varre repositório, não aprende, não roda
  sozinho — é conteúdo curado que um humano instala no agente dele. `gtm` tem 18
  skills de marketing dentro de um registry de código (o breadth cobra o pedágio).

### 5.2 `harness-toolkit` (Tech Leads Club) — enforcement em tiers, atado ao editor

Governança de agentes **via hooks do Cursor/Claude Code**: o editor pergunta, o
harness responde com decisão (`allow/deny/ask/context/abstain…`) e registra o
porquê. Ports-and-adapters com fronteiras checadas por script; 18 eventos; 2
provedores; 11 handlers; **8 regras floor (inconfiguráveis) + 3 always-on + 25
rails** com trade-off declarado; **139 ADs**; zero deps runtime; gate próprio de
20 passos; 69.034 registros de hook reais analisados para calibrar features.

- **O que impressiona**: piso que nenhuma config alcança (avaliado **antes** de
  carregar política); lessons com `path:symbol` que **sai de circulação quando o
  símbolo deixa de resolver**, janela de validade e efetividade `helped n/m`
  gradada pelo próximo run do gate; observabilidade em 3 planos JSONL +
  atestação encadeada por hash; **fail-open honesto** (`{}` = "carry on" se o
  runtime quebrar) com `hook-trace.jsonl` para diagnóstico.
- **O que NÃO é**: não é autônomo e não tem memória própria de longo prazo —
  existe **durante a sessão do editor**, dirigido pelo humano no laço. ELv2:
  conceito sim, código não.

---

## 6. A comparação — organismo vs. ferramenta

A diferença não é de qualidade, é de **natureza**. Os dois anexos vivem no tempo
do humano: o skill roda quando chamado, o hook roda quando o editor dispara. O
KIRA foi desenhado para viver no tempo do **projeto**: varre quando o repo muda,
enfileira o que aprendeu, publica com gate e espera o dono ler o briefing — o
ciclo existe sem ninguém na sala. Na terminologia da escada do próprio KIRA
([`kira-brain.md`](kira-brain.md): `prompt → skill → loop → graph → harness →
system design`), **cada anexo é um degrau maduro e isolado; o KIRA é a escada
inteira sendo construída como módulos de um binário só** — agent-skills domina o
degrau "skill" (com custódia de prompt de ponta), o harness-toolkit domina o
"harness", e o degrau "system design" (o Brain) é exatamente onde nenhum dos
três chegou ainda.

| Dimensão | agent-skills | harness-toolkit | KIRA |
|---|---|---|---|
| Dirigido por humano? | **sim** (instalação/invocação) | **sim** (hooks de sessão) | **não por desenho** (trust + budget + approve guardam a autonomia) |
| Ciclo próprio (fila, ttl, retomada, pausa) | — | — | ✅ posts com `draft_ttl` 120 s, `--pause/--resume/--budget` |
| Cache de raciocínio por hash (não refaz, não serve de outro idioma) | — | — | ✅ `cache_key=(input_hash, prompt_version, model, lang)` |
| Conhecimento que detecta a própria velhice | semver, mas nada prova validade | ✅ lessons `path:symbol` + `helped n/m` | ⚠️ `source_hash` detecta repo mudado (doctor lista post velho); refs/efetividade = PR46 |
| Determinismo contratual | CLI determinístico; skills são prose | gates por evento | ✅ byte a byte (gate M1), pack e digest idem |
| Gate de acerto vs. rótulo humano | tracks B/C com juízes LLM | mede execução, não predição | ✅ desenhado (ADR-0018) — **gold vazio** |
| Custódia de prompt | ✅ **3/3** (scan, lockfile, curadoria) | 2/3 (zero deps, SLSA) | 2/3 (prompts versados; sem scan/hash por versão) |
| Enforcement em tiers com veredito tipado | — | ✅ floor/always-on/rails + `Decision` | ⚠️ deny-list + jail; piso e veredito tipado = PR44 |
| Breadth de conhecimento | ✅ 94 skills | 1 skill (init) | prompts const + wiki viva |
| Registro de decisões | CHANGELOG gerado | ✅ 139 ADs | 25 ADRs |
| Roda sem rede | cache depois da 1ª instalação | ✅ | ✅ (apenas `digest`/`triage` usam localhost) |
| Custo por execução | n/a (não usa modelo) | tokens do agente hospedeiro | ✅ orçado em tokens, **LLM local por default** |
| Peso na máquina do dev | Node ≥24 + React no TUI | Bun/Node ≥24 | binário único, SQLite bundled |
| Licença | MIT + CC-BY-4.0 | ELv2 | MIT |

### O que nenhum dos anexos tem (e o KIRA tem hoje)

1. **Fila de trabalho com `draft_ttl`** — o KIRA enfileira julgamentos e sobrevive a
   crash/restart sem refazer nem perder (o vencido volta sozinho à fila;
   `attempts ≥ 3` vira `failed`, nunca retry infinito).
2. **Trust gate para envio de código ao modelo** — o agent-skills escaneia skills
   de terceiros; o KIRA escaneia o **próprio repo antes de enviar** e pede aceite.
3. **Wiki com gate de publicação** — post editado por humano jamais é
   sobrescrito sem `--force`; staleness é detectável e vira fila no `doctor`.
4. **Budget com número + LLM caro nunca implícito** — os anexos não controlam
   custo de modelo (não chamam modelo; o agente hospedeiro chama).

### O que eles têm e o KIRA não (a fila para "ser melhor")

1. **Breadth** (agent-skills): 94 skills de domínio empacotado — o KIRA resolve
   com profundidade no domínio dele (sustentação ADO), mas o **formato**
   (frontmatter com triggers/anti-triggers, semver, autor) vale adoção.
2. **Piso + veredito tipado** (harness-toolkit): o KIRA tem guardas, mas sem o
   piso declarado nem a taxonomia `deny/ask/block-stop/follow-up/context/record`
   — é o **P1 da fila (PR44)**, alto valor e barato.
3. **Lições que provam que ainda valem** (harness-toolkit AD-036/037/039/040):
   refs `caminho:símbolo` que saem do pack quando não resolvem + efetividade
   `helped n/m` — o **P3 (PR46)**, o pulo da rodada: conecta o "RAG" do KIRA ao
   tempo (hoje o índice sabe o que existe; não sabe o que **ainda vale**).
4. **Cadeia de custódia de prompt** (agent-skills SECURITY.md): scan
   anti-injection no CI + hash por versão gravado na sessão — o **P4 (PR48)**.
5. **Maturidade/communidade** (7.157★/310★ vs. projeto novo) — só o tempo resolve;
   o que resolve hoje é o gold rotulado, que dá número ao discurso.

---

## 7. Para ser melhor que eles — a fila honesta (prioridade de dono)

A ordem consolidada pelo dono em [`kira-brain.md`](kira-brain.md) §7 amarra as
ondas novas numa escada de baixo para cima — **cada degrau só é confiável
porque o de baixo já é determinístico**:

```
1. IG  (graph/gather)   → ingestão perfeita (BOM UTF-16, .kiraignore, segredos)
2. SK  (skill)          → catálogo de skills + knowledge (RAG pessoal)
3. TE  (loop)           → cache de raciocínio + economia medida
4. TL  (harness)        → piso de guardas + veredito tipado + custódia de prompt
5. W5  (system design)  → o Brain: grafo de estados + verifier independente
```

Em linguagem de organismo, essa ordem é: **digestão melhor → instintos →
metabolismo eficiente → pele mais grossa → coração**. O coração vem por último
de propósito: um Brain em cima de fundação instável é agente que alucina com
perseverança. 👍

**O coração (W5, o que transforma o metabolismo em organismo):**

1. **PR15 `kira loop`** — modo do binário (sem daemon), ciclos com estado no
   SQLite, teste de 2 ciclos idênticos = nada a fazer (anti-PR-inócuo).
2. **PR16 agendamento opt-in** → **PR17 budget no ciclo** → **PR18
   `awaiting_approval` + `kira approve`** → **PR19 audit semanal** — e o
   `kira brain --apply` do grafo em [`kira-brain.md`](kira-brain.md) §4 (leis:
   plano imutável, `author ≠ verifier`, LLM só nas folhas).
3. **Docker local** — decisão a tomar, e vale registrar o desenho antes: imagem
   com o binário + Ollama + volume para `.kira/` e o `store.db`; o KIRA continua
   sem daemon próprio (o container é o agendador, não um serviço Axum — o
   mandato rejeitou daemon, não rejeitou container). Pré-requisito: PR15 existir,
   senão o container executa um organismo sem batimento.
4. **M4/M6–M8** (caixa de posts no dashboard, wizard com ingestão, BOM/UTF-16
   — este último já coberto pela onda IG — e docs da onda M) — fecha a jornada
   do primeiro uso.

**Depois/paralelo, o que vale trazer dos anexos (fila TL, PR44–PR51, já
proposta em [`agent-skills-harness-vale-trazer.md`](agent-skills-harness-vale-trazer.md)):**
PR46 (lições com refs + efetividade) → PR44/45 (piso de guardas + fadiga de
aprovação) → PR47/48 (junction no jail + custódia de prompt) → PR49 (ship
claim) → PR50/51 (bônus com aval do dono).

**O bloqueio que nenhum PR resolve (dono):** rotular os 20 bugs do
`tests/fixtures/triage_gold/` — sem isso o gate M2 não mede, e autonomia não é
promovida. É uma tarde do dono, e destrava a onda inteira.

**RAG — o que é hoje e para onde vai:** o "RAG" do KIRA já é **retrieval-first
sem embeddings**: packer que injeta no prompt trechos ancorados de 6 fontes
priorizadas (`brief → findings → wiki → repo` hoje; a onda **SK** acrescenta
`skills` e `knowledge` — notas de livros/artigos do dono — com prioridade
`brief → findings → skills → knowledge → wiki → repo`), tudo sobre **FTS5 +
bm25**, índice derivado e descartável, redação na escrita, fail-open para grep
(ADR-0021/ADR-0038/[`skills-rag-plano.md`](skills-rag-plano.md)). A decisão do
dono já está selada: **FTS5/bm25 agora, embeddings semânticos depois (SK-4,
backlog)** — e a troca será local, atrás da mesma interface de busca, sem
dependência nova e sem MCP. O próximo degrau real de "evoluir com RAG" não é
vetor: é o PR46 — dar ao índice **noção de validade** (refs que precisam
resolver) e ao conhecimento **noção de efetividade** (`helped n/m`). Aí o
organismo não só lembra: sabe **quando o que lembrou parou de valer**.

---

## 8. Apêndice — onde está a evidência

| Claim | Evidência |
|---|---|
| 82 arquivos Rust / 36.215 linhas em `src/`; 21 testes / 9.291 linhas | contados nesta máquina, 13/Nov/2026 (pós-renome #89–#95) |
| `fmt` OK · `clippy -D warnings` OK · **605 unit tests, 0 falhas** | rodados hoje nesta máquina |
| metabolismo M1–M5 mergear + nomenclatura canônica | `git log`: #84 (ingest), #85 (posts/trust), #86 (digest), #87 (publish), renames #89–#95 |
| schema v8 (9 tabelas + view `outbox`) | `src/store/mod.rs:38` (`SCHEMA_VERSION = 8`) |
| fila com `draft_ttl` (120 s) e budget (30 k/1 M) | `src/store/posts.rs:26-70`, `src/post/mod.rs:28-37` |
| gate de publicação por hash | `src/publish/mod.rs` + `tests/publish_e2e.rs` |
| CI 3 SOs (unit sempre; e2e Linux sempre, desktop no main) | `.github/workflows/ci.yml:1-33` |
| Brain/loop não implementados (desenhados) | `src/` sem `brain/` nem `loop/`; desenho em `docs/kira-brain.md` §4; F2 (PR15-19) todos `[ ]` em `docs/BUILD-PROGRAM.md:139-148` |
| gold do triage vazio | `tests/fixtures/triage_gold/` sem pastas |
| Docker ausente no repo | `grep -i docker` só em dumps de terceiros |
| RAG: 6 fontes priorizadas, FTS5/bm25, embeddings = SK-4 backlog | `docs/skills-rag-plano.md` §1–2; `src/context/mod.rs` (packer) |
| Nomes canônicos (publish, facts.md, drafts/, draft_ttl, failed) | `docs/nomenclatura.md` (selado) + ADR-0039 |
| agent-skills: 94 skills/14 categorias, 19 agentes, hash SHA-256, Snyk bloqueante | sub-relatório de exploração do snapshot `e74dc8a` (registry.service, integrity.ts, scan-skills.ts, SECURITY.md) |
| harness-toolkit: 8 floor + 3 always-on + 25 rails, 139 ADs, lessons com refs | sub-relatório de exploração do snapshot `d293c7f` (floor.catalog.ts, catalog.json v15, docs/concepts.md, decisions/index) |
| Rubrica 0–3 anterior (KIRA 23/30) e fila P1–P10 | `docs/benchmark-kira-vs-techleads.md` e `docs/agent-skills-harness-vale-trazer.md` |
