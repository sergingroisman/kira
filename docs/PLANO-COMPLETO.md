# KIRA — plano completo de construção (v1, 10/Nov/2026)

> Documento para leitura do dono. Supre `docs/BUILD-PROGRAM.md` como visão geral;
> o BUILD-PROGRAM continua sendo o **estado diário** da execução.
> Fontes: `docs/kira-mandato.md` (v3), `docs/kira-judge-veredicto.md`,
> `docs/roadmap-prs.md` (ledger PR1–PR32), `docs/adr/ADR-0011` e `ADR-0013`.

## 1. O produto que estamos construindo

Um **agente de manutenção de repositório** que mora no terminal e no dashboard,
roda **localmente** (Ollama) por padrão, e evolui em três camadas:

1. **Olho** — `kira inspect` + `kira scan`: lê o work item do Azure DevOps e acha
   vulnerabilidade/dependência velha com fingerprint determinístico. *(pronto)*
2. **Cabeça** — `kira triage` + **wiki** + context packer: propõe causa raiz
   usando o conhecimento acumulado do projeto, e **mede** o próprio acerto.
3. **Mão** — `kira loop` + `kira schedule` + `kira fix --apply`: executa em ciclo,
   agenda tarefas, e só escreve no mundo com aprovação humana explícita.

O diferencial não é "AI que arruma bug". É **memória local auditável + autonomia
que não escreve sem permissão**. O que o `ai-memory` faz para memória de agente, o
KIRA faz para manutenção de repositório — em um binário, sem servidor.

## 2. Arquitetura alvo (módulos)

```
src/
├── main.rs            CLI (clap) — todos os comandos
├── board/             BoardProvider (Azure MVP; jira/trello depois)
├── ado/               única camada que fala com a API gerada
├── llm/               LlmBackend: ollama (default) · copilot (opt-in) · byok
├── scan/              .NET + npm/pnpm → findings.json determinístico   [PRONTO]
├── triage.rs          item + contexto + wiki → causa raiz + confiança
├── wiki/              markdown como fonte + FTS5 derivado               [NOVO]
├── context/           context packer (item + findings + wiki, com teto)  [NOVO]
├── guard/             path (deny-list) + jail (confinamento + rlimits)   [NOVO]
├── redact.rs          fronteira de privacidade (PAT/CPF/e-mail/token)    [NOVO]
├── store/             SQLite bundled: sessões, notas, estados, wiki_fts   [PRONTO]
├── scheduler/         agendamentos (cron interno, sem daemon)            [NOVO]
├── loop.rs            ciclo de vida (estados + budget + aprovação)       [NOVO]
├── tui/               Ratatui: dashboard estilo jogo                     [NOVO]
└── doctor.rs          diagnóstico: keyring, Ollama, VRAM, store, wiki
```

Regra estrutural: **nada de lógica fora do módulo dono**. TUI lê o store, não
chama Azure. `loop` não fala com LLM direto, fala com `triage`.

## 3. A wiki (o pedido novo) — ADR-0013

- **Onde**: `<projeto>/.kira/wiki/*.md` — local (ADR-0039; a decisão original de wiki commitada foi revertida no W2-FIX). Páginas:
  `contexto.md`, `decisoes.md`, `padroes.md`, `glossario.md`, `bugs/<item-id>.md`.
- **Escrita ≠ autonomia**: comando invocado por você grava direto em `.kira/wiki/`;
  o ciclo autônomo grava só em `.kira/wiki-staging/` e depende de `kira wiki promote`
  (humano). Autonomia nunca escreve no seu repo, mesmo com a wiki versionada.
- **Verdade x índice**: markdown manda; `wiki_page` + `wiki_fts` no `store.db` são
  **derivados e descartáveis** (`kira wiki reindex` reconstrói; apagar o índice
  nunca perde conhecimento).
- **Fronteira de privacidade**: nada entra sem `redact` (PR2). Body cru de HTTP
  nunca entra.
- **Captura silenciosa**: pós-`triage` grava episódio (item, achados, causa raiz
  prevista, confiança, arquivos). Episódio > 30 dias é colapsado em resumo.
- **Consumo**: FTS5 sobre termos do item + arquivos dos findings → top-k páginas
  com caminho e idade no prompt. **Sem embedding** até o gate M2.
- **Comandos**: `kira wiki list|show|search|add|forget|reindex|refresh|export`.
- **Igual ao DeltaDB no espírito**: o "porquê" fica escrito e consultável —
  `kira wiki search "por que aquele retry existe"`. Diferente do DeltaDB em
  escopo: aqui é conhecimento do projeto, não o histórico de cada mensagem.
- **Não-objetivos**: servidor, MCP, web UI, multi-usuário, vetor, auto-rewrite
  da wiki por LLM sem revisão.

## 4. Fases, PRs e aceite

Cada linha = 1 PR = 1 branch = 4 gates + CI 3 SOs antes do merge.

### F0 — W1 segurança/erros (base de tudo) · 4 PRs
| PR | Entrega | Aceite |
|---|---|---|
| PR1 ✅ | README+AGENTS ao mandato v3 | docs coerentes; gates verdes |
| PR2 | `redact.rs` (PAT/token/CPF/e-mail) | prompt e log provadamente sem segredo |
| PR3 | `guard/path.rs` deny-list | erro acionável, não toca disco, traversal testado |
| PR4 | taxonomia de erro única | mensagem diz o que fazer; zero `unwrap` em rede/parse/input |

### F1 — contexto (o motor) · 5 + 5 PRs
| PR | Entrega |
|---|---|
| PR28–32 | **wiki** (store md, FTS5, captura episódica, retrieval, export) |
| PR5 | context packer v1 consumindo findings + wiki (teto de tokens) |
| PR6 | prompt de triage v1 + fixtures entrada/saída |
| PR7 | `prompts.rs` público + teste de regressão de texto |
| PR8 | `LlmBackend` explícito (ollama / copilot / byok) |
| W2-bis | `kira triage --eval <dir>` + `triage_gold` (schema + % de acerto) |

### F2 — agente vivo · 5 PRs
| PR | Entrega |
|---|---|
| PR18 | estado `awaiting_approval` + `kira approve <id>` *(guarda da autonomia)* |
| PR15 | `kira loop` (modo do binário, sem daemon) + teste de 2 ciclos idênticos |
| PR16 | `kira schedule add|list|rm` + execução sob o loop (sem serviço) |
| PR17 | budget `max_llm_calls_per_day=50` + stale-ok + 429 → 5 min |
| PR19 | `kira audit --weekly` |

### F3 — hardware e provedores · 3 PRs
| PR | Entrega |
|---|---|
| PR20 | perfil de hardware: detecção de VRAM/backend, escolha de modelo, `doctor` |
| PR11 | Copilot SDK atrás de flag/env (fallback spawn de CLI; bloqueio Avanade) |
| PR27 | jail do `fix --apply`: confinamento + rlimits (ADR-0011, opção B) |

### F4 — dashboard Ratatui estilo jogo · 6 PRs
| PR | Entrega |
|---|---|
| PR21 | shell do TUI: render 30 fps, layout "sala de controle", teclas, tema |
| PR22 | painel **bugs encontrados** (findings + triage, severidade, animação) |
| PR23 | painel **arquivos criados/alterados** (store, timeline do ciclo) |
| PR24 | painel **agendamentos e quota** (próximo tick, chamadas do dia) |
| PR25 | `kira --about` com mascote pixel art (asset versionado) |
| PR26 | criação de agendamento pela TUI (form → store) |

### F5 — medição (bloqueada pelo dono) · 1 gate
`kira triage --eval` sobre os **20 bugs rotulados** ≥ 70%. Sem isso, nada de
"autônomo" no README — o número é o release gate.

## 5. Hardware — perfis suportados

| Perfil | Modelo | Contexto | Notas |
|---|---|---|---|
| **Windows + RTX 2060 6 GB** (alvo 1) | ≤4B q4_K_M (ex. `qwen3.5:4b`) | 4096 | VRAM sobra ~2 GB; não usar 7B q4 (estoura) |
| **MacBook M1 8 GB** (alvo 2) | mesmo modelo, Metal | 4096 | unificada; fechar apps pesados |
| CPU only | mesmo modelo | 2048 | lento; `doctor` avisa |
| Copilot (futuro) | remoto | 4096 | opt-in por flag/env; policy Avanade pendente |

`kira doctor` passa a reportar: backend (Metal/CUDA/CPU), modelo efetivo, VRAM
detectada, idade do último dado bom do Ollama e se o perfil bate com o hardware.

## 6. Autonomia — como "vivo" mas seguro

- **Loop** = modo do binário (`kira loop --interval 5m`), sem daemon/serviço.
- **Agendamento** = tabela no SQLite + o loop verifica o relógio. Só roda com o
  KIRA vivo (sem Task Scheduler no MVP). Criar por CLI **e** pela TUI (PR26).
- **Estados**: `queued → packing → planning → awaiting_approval → applying →
  verifying → gated → reported`, com `discarded` por falha/budget.
- **Escrita no mundo**: só em `applying`, só com `--apply`, só depois de
  `kira approve <id>`. Autonomia nunca aprova a si mesma.
- **Budget**: **50 chamadas LLM/dia** (não é R$ — é contagem de chamada). Com
  Ollama local o custo é **zero**; o teto existe para o loop não girar sem fim e
  para limitar custo quando o Copilot entrar. Teto em dinheiro
  (`max_cost_per_day_brl`) só faz sentido com provedor pago e entra junto com o PR11.
- **Falha de provedor**: 429 → 5 min de silêncio; último resultado bom fica
  visível e marcado `stale`.

## 7. O dashboard (estilo jogo) — o que aparece

Sala de controle com quatro painéis e um mascote que reage ao estado:

```
┌─ KIRA ──────────────────────────────── perfil: RTX2060 · qwen3.5:4b ─┐
│  ▄▀▀▄   BUGS ENCONTRADOS          │  CICLO ATUAL                      │
│  █▄▄█   crítico   3  ▲            │  #4821 · packing  ████░░░░ 52%    │
│  ▀██▀   alto      7               │  próximo: planning                │
│         médio    12               │  orçamento: 18/50 chamadas        │
├───────────────────────────────────┼───────────────────────────────────┤
│  ARQUIVOS TOCADOS PELO KIRA       │  AGENDAMENTOS                     │
│  + .kira/wiki/bugs/4821.md  12:04 │  scan diário     03:00  ok        │
│  ~ src/scan/node.rs         12:07 │  triage 529511   seg 09:00  ⏸     │
│  + .kira/findings.json      12:09 │  audit semanal   dom 20:00 ⏸      │
└───────────────────────────────────┴───────────────────────────────────┘
  [a]provar  [s]can  [t]riage  [w]iki  [g]end  [q]uit     mascote: idle
```

Leis: (a) não substitui o modo texto; (b) read-only sobre o store; (c) 1 PR por
painel; (d) erro nunca em popup — vai para o rodapé em texto copiável.

## 8. Ordem de execução e por quê

```
F0 (segurança) → F1 (wiki + contexto + eval) → F2 (loop/scheduler/approve)
→ F3 (hardware/Copilot/jail) → F4 (dashboard) → F5 (medição do dono)
```

Motivo: a wiki e o packer são o que fazem o triage acertar; o loop e o dashboard
sem contexto seriam "enfeite de avião sem motor" (a crítica da juíza). O
dashboard vem depois porque ele **mostra** estado — e sem estado útil não há o que
mostrar. A autonomia vem antes do dashboard porque é ela que gera o estado.

## 9. O que eu faço sozinho e o que não consigo

| Consigo | Não consigo |
|---|---|
| Implementar e testar todo PR no sandbox Linux | Rodar na sua RTX 2060 / M1 (não tenho essas máquinas) |
| Rodar os 4 gates + CI 3 SOs (ubuntu/windows/macos) | Rotular os 20 bugs (só você tem a causa raiz verdadeira) |
| Mergear em main com tudo verde | Confirmar policy do Copilot na Avanade |
| Publicar progresso no Slack a cada ciclo | Decidir budget real (uso 50 até você dizer) |

## 10. Cadência automática (pausada até seu OK)

- Cron `kira-autobuild` (id `ed5f03635662`), **a cada 2 h**, um PR por ciclo,
  reportando no seu DM do Slack. **Está pausado agora**, por seu pedido.
- Protocolo de cada ciclo: fetch → primeiro item aberto → branch → implementa →
  4 gates → push → PR → espera CI 3 SOs → merge verde → marca no BUILD-PROGRAM.
- Freio de segurança: gate vermelho 2× = item marcado como bloqueado e para.

## 11. Checklist de aprovação (o que preciso ouvir de você)

1. **Ordem das fases** F0→F5 como está? (ou quer dashboard antes do loop?)
2. **Wiki em `.kira/wiki/` local** (não comitada) — ou você quer que ela nasça
   em `docs/wiki/` commitada no repo do projeto?
3. **Default 4B q4 / ctx 4096** serve para a 2060 e o M1?
4. **Schedule só com o KIRA vivo** (sem serviço do Windows no MVP) — aceita?
5. **Budget 50/dia** como default?
6. Liberar o cron de 2 h (ou prefere 3 h / manual por enquanto)?

Responda só os números que quiser mudar; o resto eu executo como está.
