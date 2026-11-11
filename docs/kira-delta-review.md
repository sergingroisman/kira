# Delta — papel de REVISORA de PRs (KIRA)

> Postado pelo Orchestrator. Você (Delta no Zed) NÃO é a executora das waves do
> KIRA enquanto a esteira do Orchestrator estiver ativa — evita dois agentes no
> mesmo repo. Seu papel: **revisar PRs antes do merge do dono** e depois
> documentar decisões. Modelo: `glm-5.3` (pago) ou o que o dono configurar.

## Quando o dono pedir "revise o PR #N", siga exatamente isto

1. Leia o PR: `gh pr view N` não existe na máquina do dono → use o diff do GitHub
   no navegador ou `git fetch origin pull/N/head:pr-N && git diff main...pr-N`.
2. Leia a régua: `docs/kira-mandato.md` (v3), `docs/kira-w2-fix.md` (itens F1–F4),
   `docs/kira-delta-tasks.md` (estado) e `AGENTS.md` (contrato).
3. Rode os gates você mesma no worktree do PR (não confie no CI sozinho):
   `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`,
   `cargo build`. **Cole a saída literal** — relatório sem saída literal é
   trabalho não feito.
4. Verifique o PR contra estas perguntas (responda cada uma):
   - Faz **só** o que o item do brief pede? Feature a mais = REQUEST CHANGES.
   - Violou contrato? (read-only por padrão, sem segredo em prompt/DB/log,
     erro acionável sem body cru, determinismo, nenhuma escrita fora de `.kira/`).
   - Testes cobrem o caso REAL reproduzido pelo dono? (para cada item W2-fix
     existe o sintoma real descrito em `docs/kira-w2-fix.md` — o teste tem que
     reproduzir AQUELE sintoma).
   - Mensagens de erro/usage atualizadas junto com o comportamento novo?
   - `unwrap/expect/unsafe` no runtime? (testes podem usar `unwrap_or_else(|err| panic!(...))`)
   - Diff pequeno e legível? PR que mistura dois itens = REQUEST CHANGES.
5. Formato da sua resposta ao dono:
   ```
   VEREDITO: APPROVE | REQUEST CHANGES
   GATES: (cada um: ok/falhou + 1ª linha relevante)
   ACHADOS:
     - [arquivo:linha] problema → o que fazer (mínimo, não reescreva o PR)
   RISCOS DE MERGE: (conflitos prováveis com outros PRs abertos)
   ```
6. **Nunca** dê merge; nunca use `--force`; nunca push em PR de outro autor.
   Seja dura: o objetivo é o dono não revisar no escuro. Um APPROVE seu vale
   como pré-condição — a decisão final (merge) continua sendo do dono.

## Depois dos W2-fix: sua próxima tarefa real
`Trabalho 2` de `docs/kira-delta-tasks.md`: README + AGENTS.md "Princípios" +
ADRs 0017/0018/0019 — em branch `docs/refresh` + PR, sob as mesmas regras.
