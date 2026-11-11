# W2-FIX — correções da primeira rodada real (bug report do dono)

> Dados reais de uso em 11/Nov/2026 na teishoku (monorepo pnpm) + bug 529511.
> 3 PRs de correção, nesta ordem, um por vez, gates colados (fmt/clippy/test/build).
> NADA de feature nova nestes PRs — só corrigir o que o uso real quebrou.
> Baseline: `origin/main` @ `4387cc2` (W2 feita; CI verde). Orchestrator sela/push.

## PR-F1 — scanner: workspace pnpm mal-classificado (real, reproduzido)

Sintoma real: `kira scan` na raiz do teishoku detectou "pnpm, npm, npm":
raiz = pnpm (correto: package.json + pnpm-lock.yaml), mas `apps/api` e
`packages/shared` foram marcados como stacks **npm** porque têm `package.json`
sem lockfile — e são MEMBROS do workspace pnpm, não projetos npm.

Correção:
1. Na detecção (`src/scan/mod.rs`): um `package.json` sem lockfile NÃO é stack
   npm própria quando o ancestor detectado tem `pnpm-lock.yaml` **ou**
   `package.json` raiz com campo `workspaces` **ou** existe `pnpm-workspace.yaml`.
   Nesses casos: marcar como membro do workspace pnpm (nova variante `StackKind::PnpmWorkspace` ou stack com `members`), NÃO invocar `npm audit` neles.
2. `pnpm audit --json` roda UMA vez na raiz do workspace (cobre os members).
3. Testes: fixture do teishoku real (3 scopes => 1 stack pnpm com 2 members);
   fixture de monorepo npm (workspaces + package-lock => npm válido).
4. Nota: fixture sintéticas anteriores foram reconstruídas sobre formato
   documentado — se o parser divergir do `pnpm audit --json` REAL, o dono
   fornecerá a saída real; não invente formato novo sem ela.

## PR-F2 — triage: resposta fora do formato degrada em silêncio (real, reproduzido)

> **FEITO** — PR #27, merge `6968704`. `format_triage` retorna `Result`;
> resposta sem nenhum dos 3 cabeçalhos => `Error::LlmFormat` (exit 1), e2e com
> Ollama falso provando stdout vazio + nenhuma página de wiki.

Sintoma real: `kira triage 529511` imprimiu as 3 seções TODAS com `—`
(sem erro, exit 0) e a wiki registrou o episódio vazio como válido.

Correção:
1. Quando o texto do modelo não casa com o formato das 3 seções
   (`Causa raiz provável:` / `Arquivos prováveis:` / `Confiança:`), o triage
   NÃO pode degradar em placeholders silenciosos: erro acionável
   `Error::LlmFormat` ("resposta do modelo fora do formato das 3 seções —
   rode de novo, troque o modelo com --model, ou confira `ollama ps`"),
   exit 1. Placeholder `—` só se aplica a SEÇÕES vazias de uma resposta
   parseada com sucesso (ex.: seção vazia é dado, ausência de formato é erro).
2. Se o parser reconhecer padrões decorados (markdown, headers), continua
   tolerando — já coberto por testes — mas 0 de 3 seções = erro.
3. Teste novo: resposta sem as 3 seções => erro acionável; resposta com 2
   seções válidas + 1 ausente => placeholder só na ausente.

## PR-F3 — wiki: episódio vazio gravado + path fora de .kira/ (real, reproduzido)

Sintoma real: `docs/wiki/bugs/529511.md` criado DENTRO do projeto do cliente
com episódio "Causa raiz prevista: —" (vazio) + path exibido com `\\?\` cru.

Correção:
1. Wiki só grava episódio quando o triage parseou COM SUCESSO (0 seções `—`
   a menos de seção parcialmente vazia); caso contrário: episode `failed`,
   gravado apenas no store (trail), nunca no arquivo da wiki.
2. Destino da wiki muda de `docs/wiki/` para **`.kira/wiki/`** (o .kira/ é
   local do KIRA e já tem contrato); gravação em pasta do projeto do cliente
   é escrita fora do escopo e viola o contrato read-only.
3. Exibição do path usa `display_path` (o `\\?\` cru é bug de display igual
   ao que já foi corrigido no `kira profile`).
4. Testes: episódio falho não cria arquivo; episódio válido cria em
   `.kira/wiki/`; display sem `\\?\`.

## Pendências do dono que a Delta NÃO resolve (não invente)

- `pnpm audit --json` real da teishoku (o dono fornecerá) — se o parser atual
  divergir do formato REAL, isso vira PR-F1b separado.
- `ollama ps` + nova tentativa do `kira triage 529511` — se com o modelo
  carregado o "—" persistir, o PR-F2 cobre; se o modelo nem carregar, é
  outro diagnóstico.
- Os 20 bugs rotulados do `triage_gold` (pré-requisito do gate M2).

## Regra de ouro

Nenhuma feature nova. Nada de docs/wiki em pasta do cliente. Orchestrator
sela cada PR. CI 3 SOs tem que seguir verde.
