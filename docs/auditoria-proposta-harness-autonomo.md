# Auditoria — KIRA vs. a proposta do dono (harness local seguro, full autônomo, com observabilidade)

- **Data**: 13/Nov/2026 · **Base**: `kira @ 4a30051` (main)
- **Método**: código lido módulo a módulo + greps de promessa-vs-código + gates
  rodados (`fmt` ✅ · `clippy -D warnings` ✅ · `cargo test --lib` 605/0 ✅).
  Cada achado cita `arquivo:linha`. Nada aqui é opinião sem evidência.
- **Proposta auditada** (palavras do dono): *"harness local seguro full autônomo
  com observabilidade do projeto que o KIRA for inicializado"* + apoiar o dev a
  decidir + wiki com Getting Started/storytelling + skills e prompts
  especializados (caveman/rtk embutidos) + multithread na dose certa + docker
  local + RAG alimentado por textos do dono + TUI/dashboard admin futuros
  (designs prontos do dono; a ferramenta de design é estaleiro, não componente).

---

## 1. Veredito em um parágrafo

O KIRA **é** um harness local seguro hoje: binário único, local-first, trust
gate, jail de caminho, redação de segredo na escrita, budget desenhado e fila de
julgamentos retomável — isso existe, é testado e passa nos gates. Ele **não é**
full autônomo ainda: falta o cérebro (`src/brain/`, desenhado em
`kira-brain.md`), o turno agendado, a aprovação humana como estado do store e —
o achado mais sério desta auditoria — **o contador de chamadas de LLM não
existe**, então o budget "com número" do mandato hoje é contrato de doc, não
código. Observabilidade existe na forma *store-driven* (sessões, history,
doctor, dashboard, briefing), boa para local-first, mas sem atribuição por
decisão e sem rollup. Autonomia sem medição é ficção: o gold do triage continua
vazio e o budget sem contador — são os dois números que faltam para a proposta
virar verdade.

---

## 2. Pilar por pilar da proposta

| # | Pilar da proposta | Status | Evidência | O que falta |
|---|---|---|---|---|
| 1 | **Harness local** (roda na máquina, sem nuvem obrigatória) | ✅ | binário único + SQLite bundled (`Cargo.toml`); `ingest`/`scan`/`publish` sem rede; Ollama localhost | — |
| 2 | **Seguro** | ✅ com lacunas | `redact.rs` (redação na escrita do índice), `guard/path.rs`, `trust.rs`, prompt delimitado (ADR-0024), `security_probe` e2e | piso de guardas com veredito tipado (PR44), symlink/junction `lstat` (PR47), custódia de prompt com scan+hash (PR48) |
| 3 | **Full autônomo** | ❌ | metabolismo vivo (`post/mod.rs` turn com `draft_ttl`); mas sem `src/brain/`, sem `loop/`, sem scheduler, sem `awaiting_approval` no store | ondas IG→SK→TE→TL→W5 (`kira-brain.md` §7); PR15–19 |
| 4 | **Observabilidade do projeto** | ⚠️ | `sessions` no store + `kira history` + `doctor` + dashboard + `.kira/briefing.md` | contador de chamadas LLM (ver F2), atribuição por regra/decisão, rollup/attestation, `kira why`-style |
| 5 | **Apoiar o dev a decidir** | ✅– | `triage` + `--eval` (gate medido), `context --explain`, `pr draft` com evidência, posts na wiki | storytelling/Getting Started na wiki (visão de produto, sem PR) |
| 6 | **RAG alimentado pelo dono** | ⚠️ | 4 fontes hoje (`repo_fts`, `wiki_fts`, findings, facts) sobre FTS5/bm25, redigidas na escrita | textos do dono: hoje só via páginas da wiki/notes; onda SK adiciona `knowledge/` + `skill_fts` |
| 7 | **Multithread na dose certa** | ❌ (e está certo por enquanto) | nenhum `rayon`/`thread::`/`join!` em `src/` — spawns sequenciais (`scan/mod.rs:265-282`); async só para HTTP/ADO | plano medido na §5 |
| 8 | **Docker local** | ❌ | nada no repo (`grep -i docker` só em dumps de terceiros) | desenho na §6 |
| 9 | **TUI + dashboard admin (frontend futuro do dono)** | ⚠️ | TUI read-only real (`tui/app.rs`, `decide()` pura, hash e2e); prints do design no README | dashboard *admin* (escrita confirmada) e o frontend futuro |

---

## 3. Consistência código ↔ docs (achados da revisão)

| # | Achado | Evidência | Severidade | Ação |
|---|---|---|---|---|
| F1 | **Allowlist de LLM desatualizada no AGENTS.md**: o contrato diz "só `triage.rs`, `loop/`, `wiki/`"; o teste permite `triage.rs`, `eval.rs`, `setup.rs` + prefixos `llm/`, `loop/`, `post/` (e `wiki/` ainda *não* entrou) | `AGENTS.md` (seção IA) vs `tests/llm_allowlist.rs:28-34` | média (doc mente sobre o código) | corrigir o AGENTS.md para a lista real do teste |
| F2 | **Budget sem contador**: mandato/AGENTS prometem `max_llm_calls_per_day = 50` checado antes de transições; `src/config.rs` só tem `KIRA_NUM_CTX`; não há tabela/coluna de chamadas | `src/config.rs:1-33`; `src/tui/widgets/gauge.rs:12-13` admite honestamente | **alta** (autonomia depende disso) | PR novo: tabela `llm_calls` + cheque no `digest`/`triage` + linha no doctor/dashboard |
| F3 | **Jornadas promete parse que não existe**: "parse tolerante (BOM/UTF-16)" | `docs/kira-jornadas.md:94`; M7 aberto no `BUILD-PROGRAM.md:194-198` | média (promessa de produto) | M7 ou remover a promessa (M8 manda remover) |
| F4 | Nomenclatura: `kira-brain.md` ainda usa `postage_until` (canônico: `draft_ttl`) | `docs/kira-brain.md:67` | baixa | edição de 1 linha |
| F5 | Docs históricos usam `Orchestrator` (canônico: `Hermes`) — aceitável como registro; mas docs *vivos* (BUILD-PROGRAM, campo Decisores dos ADRs) ainda não migraram | `docs/nomenclatura.md:57`; `BUILD-PROGRAM.md`, `kira-mandato.md:1` | baixa | varredura nos docs vivos |
| F6 | README **atualizado** pós-rename (`kira publish`, `.kira/facts.md`) | `README.md:28,31` | — | nenhum |
| F7 | `docs/kira-motor.md` (12/Nov) descreve módulos pré-rename (`emit/`) | `docs/kira-motor.md` §2 | baixa | refresh no M8 |
| F8 | **Posts publicados e páginas customizadas ficam fora do FTS5 da wiki**: `list()`/`reindex()` só leem as 4 páginas fixas + `bugs/*`, então o que o `publish` grava em `.kira/wiki/<slug>.md` não é recuperável pelo packer (RAG) nem por `kira wiki search` | `src/wiki/mod.rs:172-195`; reproduzido ao vivo: reindex contou 0 páginas com 3 páginas de topo presentes | **alta para a tese "evoluir com RAG"** | PR: `list()`/`reindex()` varrem `*.md` de topo (exceto `index.md`), mantendo guard de id; aceite: post publicado aparece no `wiki search` e no pack |

**O que está certo e vale elogiar com lastro**: o código *admite* as próprias
lacunas em comentário (`gauge.rs:12-13`), o CI documenta o próprio custo
(`ci.yml:3-13`), e nenhum doc de produto promete o `loop` como pronto — a
desonestidade aqui é zero; o risco é só doc histórico lido como vivo.

---

## 4. "Estamos cobrindo tudo?" — matriz de cobertura da lista de desejos

| Desejo do dono | Cobre hoje? | Como / onde | Se não, o que falta |
|---|---|---|---|
| Varredura do repo sem IA | ✅ | `ingest` (denylist, FTS5, facts determinísticos) | — |
| Relatórios constantes de melhoria | ✅– | posts `intro`/`c4-container` + `briefing.md` + wiki | mais *kinds* de post (auditoria, tendência) |
| Evoluir com RAG | ⚠️ | FTS5/bm25 + índice incremental + pack orçado | SK: `skill_fts`/`knowledge_fts`; PR46 validade/efetividade |
| Skills especializados com código embutido (caveman, rtk) | ❌ | caveman estudado como camada complementar (ADR-0038 §Contexto); rtk estudado em `docs/rtk-gsd-rea-openclaude-vale-trazer.md` | SK: formato de skill (frontmatter trigger/anti-trigger) + `MAX_SKILLS` selecionado por FTS5 antes do prompt |
| Getting Started / storytelling na wiki | ❌ | wiki existe (episódios, decisões, posts) | kind de post `getting-started` + tom narrativo (persona §6) |
| Decisão com IA local (Ollama) | ✅ | `triage`, `digest` (Ollama default; caro só explícito) | — |
| Multithread | ❌ | sequencial por escolha implícita | §5 |
| Docker | ❌ | — | §6 |
| Observabilidade | ⚠️ | sessões/history/doctor/dashboard | F2 + atribuição |
| Segurança de escrita | ✅– | trust + jail + read-only default + TUI confirmada | PR44/45/47 |

---

## 5. Multithread na dose certa (pragmático, medido, sem dependência nova)

**Estado real**: zero paralelismo de cálculo. Os únicos `spawn` são processos
filhos sequenciais (`scan/mod.rs:265-282` roda uma ferramenta por vez;
`llm/copilot.rs:203`; `tui/action/runner.rs:73`). Async (`tokio`) existe só
onde a API gerada do ADO exige.

**Regra da dose** (persona: evoluir devagar; mandato: sem dependência nova sem
justificativa): thread onde o gargalo é **esperar**, não calcular; medir antes e
depois; `std::thread::scope` (Rust 2024) — **nada de rayon** por enquanto.

| Candidato | Gargalo | Ganho esperado | Risco | Veredito |
|---|---|---|---|---|
| **scan: dotnet + npm/pnpm em paralelo** | I/O de processo (segundos) | ~2× no wall-time do scan | baixo (processos independentes; stderr já é colhido por ferramenta) | **1º a fazer** (medição: antes/depois no `sessions.duration_ms`) |
| **ingest: redact+hash por arquivo em paralelo, escrita FTS5 serial** | CPU (regex) + I/O | médio em repos grandes; SQLite WAL = 1 writer, então paraleliza o *prep* e serializa o *write* (batch por transação) | médio (ordem determinística precisa sobreviver: ordenar resultados antes de gravar) | 2º, só se repo real mostrar necessidade (hoje: 14 ms no próprio repo) |
| **digest: N posts em paralelo (N=2–4)** | espera do Ollama | alto em fila cheia | alto sem contador de chamadas (F2!) — budget estouraria em paralelo | **depois do F2**; lease já protege duplicação |
| context packer / wiki FTS | µs–ms | nenhum | — | não tocar |

---

## 6. Docker local (desenho, antes de código)

- **Imagem**: multi-stage (`rust:slim` build → `debian:slim` + binário + `ollama`
  **não** dentro: Ollama é sidecar ou serviço do host via
  `host.docker.internal:11434` — GPU e modelo ficam fora da imagem).
- **Volumes**: `~/.config/kira` (store.db, config, knowledge) e o repo montado
  read-write só em `.kira/` (o resto read-only) — o jail vira também mount.
- **Batimento**: `cron`/`s6` do container chamando `kira loop`/`kira brain`
  (PR15+) — **o container é o agendador, não um daemon** (mandato rejeita
  daemon Axum/SSE, não rejeita container).
- **Pré-requisito**: PR15 existir; sem turno, o container roda um organismo sem
  coração. Trust gate continua valendo *dentro* do container (`.kira/trust.json`).

---

## 7. RAG — o conteúdo hoje e o caminho dos textos do dono

| Fonte (hoje) | Conteúdo | Índice | Quem alimenta |
|---|---|---|---|
| `repo_fts` | linhas de código **redigidas na escrita** | FTS5 bm25 | `kira ingest` |
| `wiki_fts` | páginas da wiki do projeto (episódios, decisões, posts publicados) | FTS5 bm25 | `kira wiki` / `publish` / **o dono, hoje, pelos seus textos** |
| findings/brief | resumo do `scan` | entra direto no pack | `kira scan` |
| facts | `.kira/facts.md` do ingest | entra direto no pack | `kira ingest` |

**Hoje**, o jeito de alimentar o RAG com seus textos é escrevê-los como páginas
da wiki (`kira wiki decision/contexto` ou posts promovidos) — markdown é a fonte
de verdade e o FTS5 é derivado. **Depois da onda SK** (`skills-rag-plano.md`):
`~/.config/kira/knowledge/` (notas de livros/artigos, `knowledge_fts`) e skills
bundled + `.kira/skills/` (`skill_fts`), com prioridade
`brief → findings → skills → knowledge → wiki → repo`. Embeddings semânticos
ficam no **SK-4 (backlog)**, atrás da mesma interface de busca — a troca não
vaza para o packer. Caveman entra como camada de **compressão do que já foi
recuperado** (complementar, não substituto — ADR-0038), e rtk como referência de
engenharia de skill/prompt (`docs/rtk-gsd-rea-openclaude-vale-trazer.md`).

---

## 8. Fila pragmática (um PR por item, na ordem que destrava)

| # | Item | Por que nesta posição | Aceite |
|---|---|---|---|
| P0 | ✅ **implementado (13/Nov, `feat/p0-llm-call-counter`)**: tabela `llm_calls` (schema v9) + ledger dentro do `triage_item` (cobre id/`--file`/`--eval`) + teto checado antes de cada claim no `digest` + linha `chamadas LLM hoje: N/50` no doctor; teto por `KIRA_MAX_LLM_CALLS_PER_DAY` > `max_llm_calls_per_day` (config) > 50. E2e: 2ª chamada com teto 1 falha acionável; digest para antes do claim com nota. **Falta só o dashboard** (`chamadas hoje: N/50` no gauge — PR17 segue para política 429/stale) | F2: autonomia e observabilidade dependem dele; é pequeno | 51ª chamada do dia recusa com mensagem acionável; dashboard mostra `chamadas hoje: N/50` |
| P1 | **scan paralelo** (dotnet + audit) | 1º multithread, ganho medido, risco baixo | wall-time do scan cai; findings byte-idênticos (ordem por fingerprint) |
| P2 | **PR15 `kira loop`** (turno do binário) | coração mínimo sobre a fila que já existe | 2 ciclos seguidos = estado idêntico no store |
| P3 | **PR16-18** (agenda, budget no ciclo, approve) | autonomia com guarda | escrita sem approve é impossível por construção |
| P4 | **Docker** (§6) | casa do organismo | `docker run` varre repo de fixture offline |
| P5 | **SK** (skills + knowledge + Getting Started/storytelling) | RAG do dono + decisão apoiada | textos do dono recuperáveis no pack com âncora |
| P6 | **TL** (piso + veredito + custódia de prompt, PR44-48) | pele grossa antes de escrever no mundo | recusa nomeia `rule=`; prompt com hash na sessão |
| P7 | **gold do triage** (dono) | o número que promove autonomia | 20 bugs rotulados; `--eval` ≥70% |
