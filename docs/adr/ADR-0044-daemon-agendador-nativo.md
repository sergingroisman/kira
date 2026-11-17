# ADR-0044 — Daemon no agendador nativo do SO (schtasks/launchd)

**Status:** Aceito (DAEMON-02)
**Data:** 2026-11-17

## Contexto

O DAEMON-01 entregou `kira daemon run` (lock por projeto, ticks agendados até
Ctrl+C) — mas o daemon só vive enquanto o terminal vive. O objetivo do MVP é
"rode `kira start` uma vez e de manhã veja a doc viva": o daemon precisa
acordar sozinho no login/boot, sem terminal aberto e sem virar serviço.

## Decisão

**D1 — Agendador nativo via CLI do SO, sem dependência nova.** Windows:
`schtasks /Create /TN <tarefa> /TR <linha> /SC ONLOGON /F`; macOS: plist
`RunAtLoad` em `~/Library/LaunchAgents` + `launchctl load`. A tarefa NÃO é
serviço do KIRA — é um convite para o SO spawnar o MESMO binário
(`current_exe`) com `daemon run --path <raiz>`; o lock por projeto
(DAEMON-01) e a cadência da tabela `schedule` continuam a fonte de verdade.
Instalar de novo SOBRESCREVE (nunca duplica).

**D2 — Nome determinístico por projeto.** `kira-<nome saneado>-<8 hex do
project_id>`: a mesma raiz canônica gera sempre a mesma tarefa (o uninstall
acha exatamente o que o install criou).

**D3 — Nada de segredo na linha.** O daemon é local; a tarefa carrega só o
caminho do binário e a raiz. O PAT do Azure nunca entra em `/TR` nem no
plist (o ciclo não fala com o board sem o dono conectado explicitamente).

**D4 — Resolver antes de spawnar, decidir por exit code.** Mesmo contrato
do scan (binresolve): `KIRA_SCHTASKS_BIN` / `KIRA_LAUNCHCTL_BIN` como
override (envs novas, justificadas no PR — os e2e apontam shims e NUNCA
tocam o agendador real), depois PATH; texto localizado do OS nunca é
parseado — decisão por exit code, dica = primeira linha de stderr.

**D5 — Uninstall idempotente; Linux fora do MVP.** Tarefa ausente é "nada a
remover" (query por exit code no schtasks; plist no disco é a fonte no
launchd). Linux responde erro acionável apontando `kira daemon run` — o
MVP cobre os SOs do dono (RTX 2060 Windows + M1 macOS).

**D6 — Validação real fica no hardware do dono (bloqueio B5).** Aqui a prova
é a linha gerada (função pura testada), o plist (conteúdo testado) e os e2e
com shim; o comportamento de Task Scheduler/launchd reais é o bloqueio B5
do plano (Sérgio, RTX 2060 + M1).

## Consequências

- Positivo: "rodar enquanto eu durmo" sobrevive ao fechamento do terminal;
  zero dependência nova; o filho grava trilha como sempre (é o mesmo binário).
- Negativo: o Windows sem `/SC ONLOGON` em conta de usuário pode pedir
  elevação em algumas configurações — erro acionável com dica; sem watchdog
  de renascimento (se o processo morrer, volta só no próximo login — o
  `kira daemon status` mostra o lock órfão e `--force` recupera).
- Neutro: daemon install NÃO liga agenda sozinho — sem `schedule` o daemon
  recusa (DAEMON-01), e o `kira start` é quem cria a agenda default.
