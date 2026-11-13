# Relatório — O que vale trazer do compozy para o KIRA (prompts e conceito)

> **Data**: 13/Nov/2026 · **Fonte**: `sergingroisman/compozy` (repo próprio do dono,
> Go) — dump gitingest em `C:\Users\Serginho\Downloads\sergingroisman-compozy-8a5edab282632443.txt`
> (lido nesta rodada). **Licença**: repo do dono — não é barreira para reuso.
> **Veredito**: pouco a trazer de *novo* — a engenharia de prompt do compozy já é
> refletida no KIRA (regras numeradas + seções delimitadas + prompt versionado); o
> valor fica em **2 referências marginais** (reasoning-effort como dado, contrato de
> execução para o futuro `fix --apply`) + **1 validação** de decisão já tomada.

---

## O que é o compozy

Framework de **composição de agentes reutilizáveis**: extensões que são subprocessos
falando JSON-RPC 2.0 sobre stdin/stdout, com modelo de capacidades
(`network.egress`/`subprocess.spawn` — advisory), descoberta em três escopos
(bundled/user/workspace) e **trust-and-enablement** (extensões descobertas ficam
desabilitadas até o operador habilitar; bundled nasce ligado). Um runtime global + um
catálogo de skills (`cy-*`: create-prd → create-tasks → execute-task → fix-reviews →
final-verify) e um pacote `internal/core/prompt/` que **monta prompts por seções
XML nomeadas** com um contrato de execução numerado.

## A engenharia de prompt dele (o que o usuário pediu para olhar)

| Peça | O que é | Onde cai no KIRA |
|---|---|---|
| `<critical>` / `<batch_scope>` / `<batch_issue_files>` / `<execution_contract>` | prompt montado por seções XML nomeadas + contrato numerado (1. leia tudo; 2. triage `valid`/`invalid` com raciocínio; 3. implemente; 6. verifique com `cy-final-verify`; 7. **um commit local, nunca push**) | triage v2 já é delimitado (`<<<CONTEÚDO NÃO CONFIÁVEL>>>`) com 6 regras numeradas — **já refletido** |
| `claude-reasoning-{low,medium,high,xhigh}.txt` | 4 tiers de esforço de raciocínio como **dado** (texto embutido) | `KIRA_FOUNDRY_BODY_MODE=auto\|classic\|reasoning` (já existe) + `token_profile` (PR40, proposto) — **mapeável, não novo** |
| `prompt.pre_build`/`post_build`/`pre_system` | hooks que transformam o prompt em runtime | **rejeita** — quebra o determinismo (prompt é const Rust versionada em PR) |
| `required_skills` (nomeia a skill que manda no fluxo) | fonte de verdade nomeada no prompt | `TRIAGE_PROMPT_VERSION` + `prompt_id()` já fazem isso — **já refletido** |
| `NormalizeForPrompt` / `SafeFileName` | caminho relativo + sanitização | slug validation + caminhos POSIX relativos (ADR-0038) — **já tem** |

## O que vale trazer (concreto)

| # | Ideia | Onde cai | Prioridade |
|---|---|---|---|
| C1 | **Reasoning-effort como dado no `token_profile`** — o perfil `quality` pré-para o system prompt com a instrução de "pensar profundo" (a família `claude-reasoning-*.txt`), para Foundry classic e modelos maiores. Não afeta o Ollama 4B de hoje | `token_profile` (PR40 da onda TE) + `src/llm/prompts.rs` | Baixa — referência |
| C2 | **Contrato de execução numerado para o futuro `fix --apply`** — o padrão "passos numerados + frontmatter de status (`pending`→`valid`/`invalid`→`resolved`) + 'um commit local, nunca push'" é o template do prompt de escrita quando o `fix --apply` (PR27/jail) ganhar prompt | doc-first, `src/guard/` + spec W5 | Baixa — futuro |
| C3 | **Validação do trust-and-enablement** — "descoberto ≠ habilitado; o operador liga" é exatamente o `.kira/trust.json` (ADR-0038: `ingest` roda sem trust, `digest` exige). Nada novo; reforça a decisão já tomada | — | — |

## O que NÃO vale

- **Runtime de extensão (JSON-RPC/subprocess/MCP)** — o mandato rejeita MCP como
  dependência; o KIRA é binário único, não host de plugins.
- **Catálogo de skills** (`react`, `tailwind`, `tanstack`, `shadcn`, `drawio`…) — domínio
  de frontend/ferramentas alheio à sustentação ADO. O valor é o *formato*, não o catálogo
  (mesma conclusão do relatório do agent-skills).
- **Hooks de prompt em runtime** (`pre_build`/`post_build`) — viola o determinismo
  contratual (mesma entrada ⇒ mesmos bytes); prompt do KIRA é const revisada em PR.
- **`cy-*` workflow completo** (PRD→tasks→execute→review) — o KIRA já tem o próprio
  pipeline (ingest→digest→publish); o workflow-memory deles equivale à wiki/store (ADR-0013/0021).

## Conclusão

O compozy **valida** o desenho do KIRA (prompt versionado, trust, caminhos relativos) e
oferece **duas referências de prompt** (reasoning-effort como dado; contrato de execução
para escrita) — ambas de baixa prioridade e nenhuma bloqueante. Não há lacuna de prompt
no KIRA que o compozy feche hoje: a triage já é delimitada e numerada, e o
`KIRA_FOUNDRY_BODY_MODE` já é o "reasoning-effort" na nossa forma.
