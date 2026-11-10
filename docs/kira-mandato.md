# MANDATO KIRA v3 — para a Delta · posto pelo Orchestrator · decisão do Sérgio

> 10/Nov/2026 · Supre a v2. Integre o veredito da juíza independente
> (`claude-haiku-5-5`: **BLOQUEADO, fidelidade 5/10** — os 3 bloqueios são reais
> e viram patches abaixo) e as decisões novas do dono desta manhã.
> Fonte de contexto para agentes externos: **o repo `main` passa a ser a fonte**
> — este arquivo viverá em `docs/kira-mandato.md`.

## 0) Patches da juíza — em vigor (nenhum negociável sem dado novo)

| Bloqueio | Correção v3 |
|---|---|
| **Intenção DP (Devin) perdida**: nenhum PR cria o círculo de aprovação humana | O ciclo ganha estado **`awaiting_approval`** no SQLite + comando **`kira approve <id>`**: toda escrita no mundo (Azure, repo do projeto, blog) fica parada até aprovação humana — sem ela não existe budget, `--apply` e nem modo agendado que passe |
| **Gate M2 sem método de medição** (autonomia ficaria bloqueada para sempre) | PR novo **W2-bis (`kira triage --eval <dir>`)** com `tests/fixtures/triage_gold/` — 20 bugs fechados (input JSON + causa raiz esperada, **rotulados pelo dono**); saída = % de acerto. Esse PR vem **antes do PR15** e é o que desbloqueia W5 |
| **Blog é inflação** | Blog **sai do PR18** e vai para ROADMAP como "questão a confirmar com o dono" |

Ajustes de menor edit (aplicados):
- Budget **com número**: default `max_llm_calls_per_day = 50` em `config.toml`.
- "Costly" **quem marca**: o dono/operador via `kira note costly=true ...` (ou campo no item).
- **PII sai do ROADMAP** e vira **pré-requisito do PR12** (logs carregam e-mail/CPF/token).
- **Agendamento sai do PR14 e entra no PR16** (o modo agendado precisa existir antes de ser registrado no Task Scheduler — senão falha em silêncio).
- **PR11**: se a policy bloquear o Copilot, o caminho é **ollama-only registrado no ROADMAP**, e BYOK vira **PR11b** (o plano v2 §9 já pedia BYOK desde o M0).
- **T(agento): PR13** = `commands.toml` (verbetes win/linux/mac) + `toml` como **dependência declarada e justificada** (=~ já é crates.io 0.9 no plano v2 §3); um `tests/fixtures/commands/` por OS com golden test.
- **"PR inócuo em loop" ganha teste**: dois ciclos seguidos com saída-estado idêntica no store = critério do PR15.

## 1) Decisões novas do dono (entram na arquitetura hoje)

- **KIRA é uma criaturinha em pixel art** — mascote colorido e fofo. Isso é **ARTE, não PR da Delta**: sprites entram como assets num momento posterior (W6); o binário os consume (`kira --about` mostra o sprite no boot interativo). Detalhe: pixel art autêntico não nasce de monkey-code — desenhamos/geramos manualmente e versionamos.
- **TUI**: portamos o TUI da branch `dev` **como CONCEITO/DESIGN, nunca o código Bun/Ink/TS de lá**. A implementação é **Ratatui como modo interativo rico do MESMO binário**, com 3 leis: (a) **NÃO substitui o modo texto** (loop/agendado continua consumível por log; erros em texto copiável); (b) mesmo contrato (read-only, render de estados do store); (c) 1 PR por campo do painel, CI 3 SOs (Ratatui é CI-safe). **Fica no ROADMAP até o gate M2 passar** — a juíza é clara: sem precisão medida, pintar a nave é enfeite de avião sem motor.
- Branch `dev`: continua sendo lida por `git show` (conceito/UX apenas).

## 2) Ordem final das ondas (v3)

§ base → **W1 (PR1-4)** → **W2 (PR5-8)** → **W2-bis (gate de medição M2)** →
**W3 (PR9-10)** → **W4 (PR11-14, com patches da seção 0)** → **GATE M2** →
**W5 (PR15-18, sem blog)** → **W6 (TUI Ratatui + mascote pixel art)**.

W4 já nasce com as leis da seção 1 e os patches da 0 (PII pré-requisito do PR12,
agendamento transferido para o PR16, BYOK registrado no PR11, budget com número,
`toml` dep justificada).

## 3) Fonte de contexto para agentes externos (nova regra do dono)

- `docs/kira-mandato.md` = ESTE arquivo v3 (mandato vigente · fonte de verdada)
- `docs/kira-judge-veredicto.md` = veredicto da juíza (origem dos patches)
- Prompts de juíza apontam para os caminhos **do repo**, não para `/opt/data/planos`
- O plano v2 e o fluxo dos comandos ficam em `planos/` e são lidos no repo quando copiados/acontecem mudanças de versão

## 4) Pré-requisitos de acesso (a dono, antes dos PRs 11-14)

Da juíza §5 — o dono precisa ajustar: (a) policy do Copilot CLI na Avanade;
(b) duas chaves Datadog (API key + APP key) e site regional; (c) número real
de budget; (d) **rotular os 20 bugs** no `triage_gold` (só ele tem a causa raiz
verdadeira); (e) decidir o `Set-ExecutionPolicy` para o `install.ps1`.
Sem (d), o M2 nunca é medido — e toda a autonomia é ficção.

## 5) Permanece REJEITADO (v3, atualizado)

- Daemon Axum/SSE/serviço separado (o loop é modo do binário) — inclusive o TUI NÃO é daemon
- Outro DB além do SQLite `bundled`; embeddings/semantic search (ROADMAP post-M2)
- Framework de orquestração/LangChain; agentes em fan-out; tiers/budget circuit
- MCP como dependência
- **Escrita autônoma antes do gate do M2 + `--apply` explícito** — nem com o mascot sorrendo

## 6) Gates por PR (inalterados)

`cargo fmt --check` · `cargo clippy --all-targets -- -D warnings` · `cargo test` ·
`cargo build` · Orchestrator selo/push/CI 3 SOs.
