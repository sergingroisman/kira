# docker/ — a casa do organismo (container = agendador, não daemon)

Desenho selado na auditoria (§6) e no plano (§5): o KIRA **não** vira serviço
(Axum/SSE rejeitados pelo mandato); o **cron do container** é que dá o
batimento, chamando o metabolismo que já existe no binário.

## Peças

| Arquivo | Papel |
|---|---|
| `Dockerfile` | multi-stage: `rust:bookworm` compila o binário único → `debian:bookworm-slim` + `cron`/`curl` |
| `entrypoint.sh` | escreve `/etc/cron.d/kira-tick` com `KIRA_TICK_MINUTES`, dá o 1º tick na subida e `exec cron -f` |
| `kira-tick.sh` | 1 batimento: `ingest` sempre → `digest` **só com** `.kira/trust.json` **e** Ollama respondendo → `publish`; falha vira linha de log, nunca silêncio |
| `compose.yml` | `kira` + `ollama` sidecar (modelos num volume); `KIRA_OLLAMA_URL=http://ollama:11434` |

## Rodando

```bash
docker compose -f docker/compose.yml up -d --build
docker compose -f docker/compose.yml exec ollama ollama pull qwen3.5:4b
docker compose -f docker/compose.yml exec kira kira trust --path /repo   # aceite humano
docker compose -f docker/compose.yml logs -f kira                        # o pulso
```

Sem o `trust`, o tick ingere e para com nota — **nada de código saindo da
máquina (ou do container) sem aceite**. Sem Ollama, nota e segue. Nenhum tick
usa `--apply`: escrita no mundo continua atrás de `kira approve` (ADR-0017).

## O que isto NÃO é

- **Não é o coração**: o turno agendado aqui é o *pulso externo*; o grafo de
  estados (`kira brain`, PR15+) continua sendo o coração desenhado em
  `docs/kira-brain.md`. Container sem o PR15 = organismo com marcapasso mas sem
  bulbo — roda o metabolismo, não decide ciclos.
- **Não é multithread**: paralelismo de verdade mora no binário (scan paralelo,
  P1); o container só agenda.
- **Não é deploy**: é a casa local do dono (local-first, ADR-0030). Nada aqui
  sobe para nuvem.

## Verificação pendente (honestidade)

Esta máquina não tem Docker instalado: os arquivos acima foram revisados contra
o código real (`KIRA_OLLAMA_URL` em `src/llm/ollama.rs:32`, trust gate em
`src/trust.rs`, flags `--pause`/`--path` em `main.rs`), mas o aceite do plano
§5 ("compose varre fixture offline e produz facts + draft") roda na primeira
máquina com Docker — virará PR com essa evidência.
