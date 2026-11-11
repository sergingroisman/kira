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
3. **ADRs** em `docs/adr/` (formato dos que você já criou; a série está em
   ADR-0016, então os seus novos são **0017, 0018 e 0019**):
   - ADR-0017: autonomia por loop/agendado no mesmo binário (pilar do dono;
     máquina de estados §7 do plano v2; `awaiting_approval` + `kira approve`);
   - ADR-0018: gate do M2 = `kira triage --eval` + `tests/fixtures/triage_gold/`
     (20 bugs rotulados pelo dono) — autonomia de escrita só depois disso;
   - ADR-0019: backend Copilot via spawn do CLI `copilot -p` como "LLM mais
     cara" sob demanda (Ollama default 100% local); policy da Avanade é
     pré-requisito — registrado o caminho ollama-only caso bloqueada.
   - Cada ADR: contexto (intenção do dono, citada), decisão, consequências.

Referência de verdade: `docs/kira-mandato.md` (v3) e
`docs/kira-judge-veredicto.md`. Não mude código além dos itens listados.

---

## ⚠️ Estado em 11/Nov (pós-janela estourada) — leia antes de retomar

- **O PR-F1 já está FEITO e publicado como PR #26 (draft)**: o Orchestrator implementou
  a mesma correção que você tinha validada em `w2-fix/pr-f1-pnpm-workspace` (members
  dobrados na stack da raiz; e2e com shim npm que falha provando que npm não roda).
  **Abandone seu worktree local** (`git reset --hard origin/main` no clone do Zed) e
  parta de `origin/main`. Não refaça o F1.
- **Seu teste local na máquina do dono achou um bug extra (PR-F4)**: em Windows pt-BR,
  `missing_audit_tool_is_actionable` falha porque o cmd localizado emite
  "não é reconhecido" em OEM/mojibake e a classificação texto-based não casa.
  Confirmado em `origin/main` limpo — não é regressão de ninguém.
- **Próxima tarefa sua: PR-F1b** (`fix(scan)`, parser pnpm real). Captura obrigatória:
  `cd teishoku && pnpm audit --json > pnpm-audit-real.json` (a máquina do dono tem
  pnpm real; a captura de 11/Nov mostrou `{actions, advisories, muted, metadata}` —
  39 advisories, `findings[].paths` vazio em todas). Fixture:
  `tests/fixtures/scan/scan_pnpm_audit_real.json` (mantenha `advisories` + `metadata`
  verbatim; remova só o top-level `actions`, irrelevante ao parser). Parse: um
  Finding por advisory (module_name, severity, title, findings[0].version,
  findings[0].paths[0] com fallback `pnpm-lock.yaml`). Não suporte o formato
  fantasma array-de-eventos (nunca existiu no pnpm real — era formato do yarn).

## PR-F4 — detecção de ferramenta SEM depender de texto localizado (novo)

- Sintoma (máquina do dono, Windows pt-BR): `kira scan` num projeto Node sem npm
  instalado imprime `a ferramenta npm audit falhou (exit 1) — ... (dica: 'npm' n�o �
  reconhecido ...)` em vez do acionável "instale o npm". O e2e
  `missing_audit_tool_is_actionable` falha localmente por isso (CI en-US passa).
- Causa: `is_missing_command_text` casa marcadores de texto que o cmd localizado
  emite em OEM (mojibake) — parsing de mensagem localizada é frágil por design.
- Correção prescrita: **resolver o binário antes de executar** — busca `which`-style
  em Rust puro (PATH × PATHEXT no Windows: `.exe/.cmd/.bat`; Linux/mac: exec bit),
  respeitando o override `KIRA_SCAN_*_BIN`. Ferramenta não resolvida ⇒ erro
  "instale o X" (sem spawn, sem parsing de texto). `.cmd/.bat` resolvido ⇒ wrapper
  `cmd /C` como hoje. Remover os marcadores CMD_MISSING_MARKERS e o caminho
  texto-based; testes: shim faltando ⇒ mensagem de instalação; shim presente que
  falha ⇒ ToolFailure compacto; e2e `missing_audit_tool` verde em qualquer locale.
- Dono do PR: Delta (é ela quem roda em Windows pt-BR para validar de verdade).
