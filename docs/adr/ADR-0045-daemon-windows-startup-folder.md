# ADR-0045 — Daemon do Windows na pasta Inicializar (não no Task Scheduler)

**Status:** Aceito (DAEMON-02, substitui a parte Windows do ADR-0044)
**Data:** 2026-11-17

## Contexto

O ADR-0044 escolheu `schtasks` para o Windows. A validação no hardware do
dono (bloqueio B5) falhou com `os error 5` no SPAWN — e o diagnóstico
empírico (17/Nov/2026, RTX 2060) provou que NÃO é bug do KIRA:

- `probe` (binário Rust unsigned em temp) → `schtasks /?`: **ok**.
- `probe` → `schtasks /Create …`: **negado** (CreateProcess, os error 5).
- `kira` → `schtasks /Create …`: **negado** — inclusive em terminal ADMIN.
- `kira` → `where.exe /Create …` (outro .exe do System32): **ok** — o spawn
  do kira funciona; o alvo é que decide.
- `PowerShell` → `cmd /C 'schtasks /Create …'`: **negado** (a LINHA do cmd
  menciona schtasks); `cmd` com batch que chama schtasks: o schtasks roda,
  mas o **serviço** do Task Scheduler nega `ONLOGON` sem elevação.

Conclusão: o EDR/Defender do dono bloqueia criação de tarefa agendada via
`schtasks` (a LOLBin clássica de persistência) quando o pai não é aprovado
— por parente E por argumento. Enrolar o schtasks dentro de batch para
escapar da inspeção de linha é **tática de malware** — o KIRA não faz isso.

## Decisão

**D1 — Windows: pasta Inicializar (Startup) do USUÁRIO.** O install grava
`%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup\kira-<nome>-<8hex>.cmd`
com `start "kira daemon <task>" /MIN "<exe>" daemon run --path "<raiz>"`.
Semântica idêntica ao ONLOGON projetado (acorda no login), sem privilégio,
sem `schtasks`, sem spawn — install/uninstall são pura escrita/remoção de
arquivo (determinísticos, idempotentes, testáveis sem tocar o SO). O console
do daemon nasce MINIMIZADO: honestidade visual (persona §7) — o dono vê que
o KIRA está vivo. Complemento ASCII-only no batch (code page OEM não mangla
a linha de comando).

**D2 — macOS segue launchd (ADR-0044 D1).** O plist `RunAtLoad` + `launchctl
load` não tem equivalente de bloqueio no hardware do dono (M1).

**D3 — Sem watchdog no Windows.** Se o processo morrer, volta no próximo
login; `kira daemon status` mostra o lock órfão e `--force` recupera. Um
watchdog periodicamente respawnando o daemon é comportamento de serviço —
fica para um card futuro, COM o dono (COM API do Task Scheduler é o caminho
legítimo, sem LOLBin — depende de crate `windows`, fora do MVP).

**D4 — O EDR do dono não é inimigo.** A regra que bloqueou o schtasks é
correta contra a técnica que ela mira; o KIRA muda de mecanismo legítimo em
vez de aprender a contornar inspeção. Nenhum fluxo do KIRA esconde comando
do agendador em arquivo intermediário.

## Consequências

- Positivo: install/uninstall sem spawn, sem admin, sem atrito com EDR;
  e2e mais hermético (asserção de arquivo com `APPDATA` no sandbox); a prova
  no hardware real já feita (17/Nov: install → arquivo na Startup real →
  uninstall → removido).
- Negativo: sem renascimento automático no meio do dia (D3); o atalho roda
  no login de CADA usuário que logar (é por usuário de propósito).
- Neutro: quem preferir Task Scheduler pode criar a tarefa À MÃO (com o
  comando que o install imprime) — o KIRA não precisa de permissão especial
  para o ciclo em si.
