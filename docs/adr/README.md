# ADRs do KIRA — índice e **registro de reserva de número**

> **Antes de escrever um ADR novo:** adicione sua linha nesta tabela **no mesmo PR** que
> cria o arquivo. Número reservado na tabela é seu; número não reservado pode ser tomado.
> Assim duas sessões paralelas nunca escolhem o mesmo `NNNN`.

## Regras

1. **Um ADR = um arquivo** `docs/adr/NNNN-titulo-em-kebab.md`, versionado no repo, commitado
   com o código. O histórico do ADR é o `git log` do arquivo, e a discussão é o PR (padrão
   Nygard). Wiki/Discussions/Notion são **espelho de leitura**, nunca a fonte.
2. **Número é imutável** depois de mergeado. Não se renumera, não se reusa, não se apaga:
   ADR que perdeu validade vira `Status: substituído por ADR-NNNN`.
3. **Ordem de numeração = ordem de impacto**, não de chegada: risco de segurança e
   decisões que travam outras (fronteira de dados, autonomia) vêm antes de conveniência.
4. **Reserva no PR**: a linha entra na tabela com `status: proposto` e o número do PR/branch.
   Sem linha na tabela, o PR não é mergeado.
5. **Status**: `proposto` → `aceito` (só humano) → `substituído`. `rejeitado` é válido e fica
   registrado com o motivo (histórico de tentativa é conhecimento).
6. **Teto prático**: até 4 ADRs abertos por onda. Mais que isso é sinal de decisão grande demais
   escondida em documento.

## Índice (ordem de impacto)

| Nº | Título | Status | Onde |
|---|---|---|---|
| 0011 | Sandbox do fix (por que o agente não monta ambiente sozinho) | aceito | `ADR-0011-sandbox-do-fix.md` |
| 0013 | Wiki local em `.kira/wiki` | aceito | `ADR-0013-wiki.md` |
| 0014 | Dashboard statui (TUI read-only) | aceito | `ADR-0014-dashboard-statui.md` |
| 0015 | Contexto: retrieval antes do modelo | aceito | `ADR-0015-contexto-retrieval-first.md` |
| 0016 | CLI-first, LLM-last (allowlist de chamadas) | aceito | `ADR-0016-cli-first-llm-last.md` |
| 0017 | Autonomia por loop + `awaiting_approval` | aceito | `ADR-0017-autonomia-awaiting-approval.md` |
| 0018 | Gate M2 medido por `kira triage --eval` | aceito | `ADR-0018-gate-m2-triage-eval.md` |
| 0019 | Copilot por spawn; Ollama como default local | aceito | `ADR-0019-copilot-spawn-ollama-default.md` |
| 0020 | Foundry BYOK: fronteira de dados e auth | aceito | `ADR-0020-foundry-byok.md` |
| 0021 | Índice de retrieval FTS5 (bm25), não embeddings | aceito | `ADR-0021-retrieval-index.md` |
| 0022 | Organização por domínio agora, camadas depois | aceito | `ADR-0022-organizacao-por-dominio.md` |
| 0023 | Gitingest: o que vale trazer (BOM/UTF-16, ignorados, orçamentos) | proposto | `ADR-0023-gitingest-reaproveitamento.md` |
| **0024** | **Guardrails v2: injeção invisível (zero-width/bidi) + bloco delimitado no prompt** | **proposto** — reservado | PR #72 (`w7/pr72-prompt-v2`, R2) · `ADR-0024-guardrails-invisiveis-prompt-v2.md` |
| **0025** | **`kira setup` ensina: aula do PAT, menu com pular, validação na hora** | **proposto** — reservado | PR #52 · `ADR-0025-setup-teaches.md` |
| **0026** | **Probe suite de segurança no `cargo test`** | **proposto** — reservado | PR #51 · `ADR-0026-probe-suite-seguranca.md` |
| **0027** | **Tech Leads Club: piso de guardas, lições e custódia de prompt** | **proposto** — reservado | PR #49 · `ADR-0027-techleads-piso-guards-licoes-custodia.md` |
| **0028** | **Reservado** — ferramentas de scan (ex.: `gitleaks`) / economia de token | **reservado** | a confirmar com o dono |

## O que **não** é ADR aqui

- Plano de onda (vai em `docs/roadmap-prs.md`), relatório de engenharia reversa
  (`docs/*-vale-trazer.md`) e proposta de produto. ADR é **decisão** com alternativa
  rejeitada e consequência declarada.

## Para sessões paralelas (OpenCode e afins)

```bash
git pull                      # traga o índice atualizado
grep -n 'reservado\|proposto' docs/adr/README.md   # veja o que já está tomado
# escolha o MENOR número livre, adicione sua linha + o arquivo no MESMO PR
```
