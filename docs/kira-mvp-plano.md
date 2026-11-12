# MVP do KIRA — plano de execução (para o Kombai)

Base: **ADR-0030 (local-first, capacidades)** e `docs/kira-jornadas.md`.
Regra de ouro: **um PR por bloco**, merge humano, gates completos em cada um
(`fmt --check` · `clippy -D warnings` · `test` · `build` · CI 3 SOs).

## Definição de pronto do MVP

Um dev instala o KIRA, roda `kira setup` **sem informar nenhuma credencial**, aponta para o repositório
e consegue: `scan` → `judge --file` (com Ollama local) → `scribe` (markdown do PR) → `watch` (ver tudo),
com `codex` guardando o que aprendeu entre execuções. Nenhuma chamada de rede obrigatória.

---

## MVP-1 — Capacidades e gating (a fundação)

**Branch**: `mvp/1-capacidades`
**Escopo**
- `src/capabilities.rs` (novo): `Capability { BoardAzure, ScmGithub, ScmGitlab }`,
  estado por projeto (`conectada | desconectada | inválida`) persistido no store.
- Migração de `user_version` para a tabela de capacidades (nunca recriar tabela existente).
- `Error::CapabilityMissing { capability, command }` com mensagem **exata**:
  `esta jornada precisa de board.azure — rode: kira connect azure`
- Gating em `triage <id>`, `inspect` e nos caminhos de publicação (que hoje não existem: só preparar).
- Sem rede quando a capacidade falta (nem tentar).

**Aceite (testes)**
- A matriz de capacidades vira tabela de casos: cada linha roda o binário e confere mensagem + exit code.
- Comando local (`scan`, `wiki`, `context`, `pr draft`, `dashboard`) continua funcionando com zero env.
- Nenhuma tentativa de rede é registrada quando a capacidade está ausente.

**Não fazer**: não mexer no `setup` ainda; não renomear comandos.

## MVP-2 — `setup` local-first

**Branch**: `mvp/2-setup-local`
**Escopo**
- Remover o passo de PAT do `setup` (a aula migra para `connect`, no MVP-3).
- Fluxo: nome do dev → projeto (raiz detectada) → backend de IA com a opção **4) nenhuma por agora**
  (default quando Ollama ausente) → resumo.
- Resumo com três blocos: `funciona agora` · `desativado` · `próximo passo: kira scan`.
- `--yes` (não-interativo) nunca bloqueia: escolhe "nenhuma IA" e imprime o pulado.

**Aceite**
- `kira setup` em TTY simulado **não pergunta PAT** (assert: string `PAT` não aparece antes de `connect`).
- `kira setup --yes` sem Ollama termina com exit 0 e resumo correto.
- `doctor` continua funcionando e agora diz: `capacidades: nenhuma conectada (modo local)`.

**Não fazer**: não quebrar `--org/--project/--pat` (continuam existindo como atalho para quem já tem).

## MVP-3 — `kira connect azure` (a aula no lugar certo)

**Branch**: `mvp/3-connect-azure`
**Escopo**
- Novo subcomando `connect <provider>` com `azure` implementado.
- Reaproveitar de `setup.rs`: aula do PAT (texto do ADR-0025), menu 1/2/3, validação 200/401/403/sem rede,
  entrada sem eco, gravação no keyring.
- Sucesso ⇒ marca `board.azure` conectada **para o projeto atual**; falha ⇒ mensagem da causa, nada gravado.
- `connect github`/`gitlab`: aceitar o argumento e responder "ainda não implementado" com exit code
  próprio (não fingir que existe).

**Aceite**
- Servidor falso: 200/401/403/sem rede ⇒ as 4 mensagens exatas + estado da capacidade correto.
- `triage <id>` depois de conectar passa a tentar o board (e falha por rede, não por capacidade).

## MVP-4 — Capacidades visíveis

**Branch**: `mvp/4-capacidades-visiveis`
**Escopo**
- `doctor`: seção `capacidades:` com estado por provider e o comando de ativação.
- `dashboard`: um campo no cabeçalho (`capacidades: local` ou `board.azure ✓`) — **uma linha**, com o
  helper `one_line` já existente.
- Ajuda (`?`) explica o que muda quando cada capacidade conecta.

**Aceite**
- `--snapshot` mostra o campo; TestBackend confirma altura constante.
- Nenhuma tecla escreve (hash do store igual antes/depois).

## MVP-5 — Prova do ciclo cru (sem rede)

**Branch**: `mvp/5-smoke-local`
**Escopo**
- e2e `tests/mvp_local_e2e.rs`: fixture de repo pequeno + `KIRA_SECRET_STORE=off` + `KIRA_CONFIG` em tempdir.
- Roda, sem rede e sem credencial: `setup --yes` → `scan` → `judge --file` (backend falso) → `scribe` →
  `watch --snapshot`, conferindo os artefatos de cada etapa.

**Aceite**
- O teste falha se qualquer etapa tocar a rede (guardar por ausência de servidor + timeout curto).
- Exit 0 no fim; `findings.json` e o `.md` do PR com conteúdo esperado.

---

## Fora do MVP (registrado para não voltar como dúvida)

| Item | Por que espera |
|---|---|
| `verdict` (`--eval`) como gate | precisa dos 20 bugs rotulados pelo dono |
| `sentinel` (W5) | bloqueado pelo gate M2 |
| Publicar PR/issue em `scm.*` | capacidade nova, depois de o ciclo local estar estável |
| Rename dos comandos (ADR-0029) | cosmético e transversal; melhor depois do MVP-2 para não rebasear tudo |
| Mascote / `ratatui-image` | pós-MVP (decisão do dono) |

## Ordem e paralelismo

```
MVP-1 → MVP-2 → MVP-3 → MVP-4 → MVP-5
```

MVP-1 e MVP-2 podem ir em paralelo **se** o MVP-2 não tocar em `capabilities.rs`; caso contrário,
sequencial. Depois do MVP-2, o KIRA já é instalável por qualquer dev sem credencial — que é o objetivo
do MVP. MVP-3/4/5 fecham a experiência.
