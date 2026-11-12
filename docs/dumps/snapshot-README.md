# Snapshots (dumps gitingest) — material de planejamento TE

- **Data**: 12/Nov/2026 · fornecidos pelo dono (Downloads) e comitados a pedido
  dele na PR `docs/te-plano-snapshots`.
- **O que são**: dumps gitingest (texto puro) de repositórios **públicos**,
  material de análise do plano de economia de token
  ([`docs/rtk-gsd-rea-openclaude-vale-trazer.md`](../rtk-gsd-rea-openclaude-vale-trazer.md)).
  Nada deles é código do KIRA; **nada foi copiado para o código** — leitura e
  análise apenas (mesma regra "conceito sim, código não" dos relatórios irmãos).
- **Nota de tamanho**: ~83 MB no total. O `snapshot-rea.md` (65,2 MB) passa do
  aviso de 50 MB do GitHub (o bloqueio duro é em 100 MB). Aceite consciente do
  dono; se clone/CI sofrerem, vale pedir a versão enxuta (só README/docs/AGENTS
  de cada repo — os dumps completos podem continuar locais).

| Arquivo | Fonte | Tamanho | Nota |
|---|---|---|---|
| `snapshot-gsd-2.md` | fork do dono de `gsd-build/gsd-2` (→ `open-gsd/gsd-pi`) | 0,5 MB | docs e user-docs pesados (routing/budget pressure) |
| `snapshot-openclaude.md` | fork do dono de `Gitlawb/openclaude` | 17,1 MB | inclui os providers Python do próprio dono (`atomic_chat_provider.py`, `ollama_provider.py`, `smart_router.py` + testes) e `cost-tracker.ts`/`costHook.ts` |
| `snapshot-rea.md` | `morluto/rea` (upstream) | 65,2 MB | dump completo do código (TS/MCP + CLI) |

- **Licenças dos fontes**: MIT (os três). Ainda assim, a regra do repo vale para
  qualquer reaproveitamento: conceito sim, código não, sem verificação de
  compatibilidade de licença caso a caso.
- **Falta o dump do RTK** (`rtk-ai/rtk`) — o 4º repo escolhido; até lá, a análise
  dele fica no que o site/README público mostram (licença mista Apache-2.0 /
  BUSL-1.1 reforça: só conceito).
