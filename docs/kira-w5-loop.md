# W5 — `kira loop`: o ser vivo que fica ligado (especificação)

- **Status**: proposta; **bloqueado pelo gate M2** (20 bugs rotulados + ≥70% medido)
- **Base**: ADR-0017 (autonomia por loop/agendado + `awaiting_approval`), ADR-0016
  (CLI-first/LLM-last), ADR-0021 (retrieval FTS5), persona do dono

## O que "sempre ligado" significa aqui (e o que não significa)

Significa: um processo do **mesmo binário** roda enquanto o terminal/PC está ligado,
olhando o projeto em intervalos, sem gastar token à toa, e produzindo artefatos que o
dev lê. **Não** significa daemon, servidor, escrita autônoma ou agente que decide sozinho
o que é "melhoria".

## Estados do ciclo

```
idle ──tick──▶ observe ──▶ decide ──▶ (leitura) ─────────────▶ record ──▶ idle
                    │                     │
                    │                     └─(escrita)──▶ awaiting_approval ──humano──▶ applying
                    └─(nada mudou)──▶ idle (sem token, só log)
```

| Estado | O que faz | Custo |
|---|---|---|
| `observe` | `scan` (se passou o intervalo), relê git (`status`, último commit), lê `findings.json`, store e wiki | **0 token** |
| `decide` | regra pura: há item novo no board? há achado novo? há PR aberto? passou o intervalo de triage? | **0 token** |
| `leitura` | `triage` no item mais prioritário, `pr draft` do que já tem evidência | LLM só aqui, sob budget |
| `record` | grava sessão no store, atualiza wiki, escreve relatório em `.kira/reports/` | 0 token |
| `awaiting_approval` | qualquer escrita no mundo (board, git remoto) para e espera `kira approve <id>` | 0 token |

## Cadência (default, ajustável)

| Trabalho | Intervalo | Por quê |
|---|---|---|
| `scan` | 30 min | barato, pega dependência/segurança nova |
| leitura do git | 2 min | é o sinal mais barato de "aconteceu algo" |
| `triage` de item novo | 15 min, máx. 1 por ciclo | caro; só quando há item novo no board |
| consolidação da wiki (o "sono") | 1× por dia, ocioso | memória envelhece e consolida sem LLM |
| relatório diário | 1× por dia | é o que o dev lê no café |

## Watchdogs (o que impede virar zumbi)

1. **Budget**: `max_llm_calls_per_day = 50` (config), checado **antes** de cada transição
   de estado. Estourado ⇒ só `observe`/`record`, zero LLM, e um aviso no rodapé.
2. **Quota do provedor (429)**: provedor intocado por 5 min; último resultado bom marcado
   `stale`; se nem Ollama estiver de pé, o ciclo **cai para modo observação** e avisa
   (downgrade explícito, nunca silencioso).
3. **Custo por ciclo**: teto de 1 chamada cara por ciclo; 3 ciclos seguidos sem novidade
   ⇒ intervalo dobra (backoff), até 8× .
4. **Trava de instância**: um `loop.lock` no store; segundo processo sai com mensagem
   (dois loops no mesmo projeto corrompem a trilha).
5. **Kill switch**: `kira loop stop` grava `stop` no store; o ciclo lê antes de cada
   transição e encerra limpo.
6. **Sem escrita por acidente**: qualquer ação que escreva exige `--apply` **e** passa por
   `awaiting_approval`; `kira approve <id>` é do humano, com registro de quem aprovou.

## Integração com logs (Datadog, depois outros)

- `setup` pergunta a fonte de log (`datadog` | `nenhum`).
- No ciclo, `observe` puxa **só contagem e códigos de erro agregados** (nunca payload de
  cliente); PII/segredo passam por `redact` antes de qualquer prompt.
- Um log novo que vira episódio entra na wiki como `bugs/<slug>.md` com a evidência citada.
- Som/notificação do dashboard: **apenas** erro crítico de produção e bug crítico em PR.

## Interface com o humano

- **`kira loop` (TUI)**: status ao vivo (estado, próximo tick, budget do dia, último artefato).
- **`.kira/reports/AAAA-MM-DD.md`**: o relatório que o dev lê — o que mudou, o que foi
  triado, o que aguarda aprovação, o que está `stale`.
- **`kira inbox`** (futuro): lista o que o KIRA produziu e está esperando leitura.
- Cada artefato aponta a evidência (`path:linha`, página de wiki, item do board).

## Como provar que funciona (antes de soltar no PC de alguém)

1. **Teste de 2 ciclos seguidos com estado idêntico** (critério anti-"PR inócuo em loop"):
   o segundo ciclo não faz chamada de LLM nem grava artefato novo.
2. **Budget respeitado**: com `max_llm_calls_per_day = 2`, o terceiro ciclo não chama LLM
   e o relatório diz por quê.
3. **Trava de instância**: segundo processo sai com mensagem, sem corromper.
4. **Kill switch**: `stop` encerra em menos de 1 tick.
5. **Modo observação**: com Ollama fora, o ciclo continua, avisa o downgrade e não mede.
6. **Gate M2**: ≥70% em 20 bugs. Sem isso, o loop **não é promovido** a "autônomo" — fica
   em modo sugestão (produz `pr draft` e relatório, nunca abre PR).

## O que fica explicitamente fora do W5

- Abrir PR/merge sozinho (exige autorização por flag + revisão humana).
- Escrever no board.
- Instalar dependência.
- Qualquer escrita fora de `.kira/`.
