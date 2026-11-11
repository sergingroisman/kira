# Delta — dois trabalhos, nesta ordem (postado pelo Orchestrator)

Você é a Delta no Zed. Seu papel NESTE ciclo: (1) corrigir 3 bugs reais
encontrados no primeiro uso real, e (2) documentar as decisões do projeto.
Trabalhe em **branch + PR por item** — merge é do Sérgio, não seu.
Gates por PR: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test`, `cargo build`. O Orchestrator sela/pusha após sua PR estar aberta
(e o CI 3 SOs decidir). Nada de feature nova fora destes itens.

## Trabalho 1 — W2-FIX (3 PRs: `fix(scan)`, `fix(triage)`, `fix(wiki)`)

Siga `docs/kira-w2-fix.md` (brief completo, com o comportamento real reproduzido
pelo dono): F1 = workspace pnpm mal-classificado; F2 = triage degrada "—" em
silêncio; F3 = wiki grava episódio vazio fora de `.kira/` + `\\?\` no display.
Captura de `pnpm audit --json` REAL: rode na teishoku e anexe a saída ao PR-F1
(você é quem tem o pnpm real; se o parser divergir do formato real, abra o
PR-F1b seguindo a saída capturada).

## Trabalho 2 — documentar as decisões (1 PR: `docs/refresh`)

Atualize `README.md` e `AGENTS.md` para o estado atual e registre ADRs:

1. **README**: status real = M1-p1 + W2 (scan determinístico, triage com
   context packer, store SQLite, setup wizard com keyring, doctor, history/
   note/profile). Remova o "In M0 it does exactly one thing". Seções:
   o que faz, instalação (Windows first), setup, comandos, store, requisitos
   (Rust + Ollama para o triage; dotnet/pnpm para o scan).
2. **AGENTS.md**: acrescente a seção "Princípios" (edit-the-harness: edite o
   prompt, não o output; determinismo antes de inteligência; erro acionável
   nunca body cru; read-only por padrão, `--apply` explícito).
3. **ADRs** em `docs/adr/` (formato dos que você já criou):
   - ADR-0021: autonomia por loop/agendado no mesmo binário (pilar do dono;
     máquina de estados §7 do plano v2; `awaiting_approval` + `kira approve`);
   - ADR-0022: gate do M2 = `kira triage --eval` + `tests/fixtures/triage_gold/`
     (20 bugs rotulados pelo dono) — autonomia de escrita só depois disso;
   - ADR-0023: backend Copilot via spawn do CLI `copilot -p` como "LLM mais
     cara" sob demanda (Ollama default 100% local); policy da Avanade é
     pré-requisito — registrado o caminho ollama-only caso bloqueada.
   - Cada ADR: contexto (intenção do dono, citada), decisão, consequências.

Referência de verdade: `docs/kira-mandato.md` (v3) e
`docs/kira-judge-veredicto.md`. Não mude código além dos itens listados.
