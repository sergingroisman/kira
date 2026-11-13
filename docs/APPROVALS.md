# APROVAÇÕES — como o dono autoriza (e como eu peço)

> Decisão do dono (10/Nov/2026): *"se precisar de mim me avisa no Slack que aprovo
> por lá"*. Este arquivo é o protocolo. Vale para o cron `kira-autobuild` e para
> qualquer sessão minha que trabalhe no KIRA.

## Regra de ouro

**Silêncio não é aprovação.** Nada que exija o dono avança por ausência de
resposta. Se ele não respondeu, o item fica `bloqueado` e eu sigo para o próximo
item independente.

## O que NUNCA precisa de aprovação (já autorizado em 10/Nov)

- Ler, `scan`, `triage`, `doctor`, rodar testes e gates.
- Criar branch, commitar, abrir PR.
- **Mergear PR com CI 3 SOs verde** (o dono delegou explicitamente).
- Escrever/atualizar `docs/` e `docs/adr/` do próprio repo do KIRA.
- Corrigir bug com teste que reproduz o defeito, desde que os gates passem.

## O que SEMPRE precisa de aprovação no Slack

| Categoria | Exemplo |
|---|---|
| Escrita fora do repo do KIRA | escrever no repo do cliente/projeto do usuário |
| Escrita no Azure DevOps | qualquer `--apply`, mudança de work item |
| Dependência nova em `Cargo.toml` | qualquer crate fora de `ratatui`/`crossterm` |
| Apagar ou reescrever história | `reset`, force-push em `main`, deletar branch de outro |
| Mudar decisão de ADR | qualquer coisa que contrarie `docs/adr/` |
| Gastar dinheiro | provedor pago, Copilot, API key |
| Rotular dado de verdade | os 20 bugs do `triage_gold` |
| Sobrescrever wiki existente | apagar página de `.kira/wiki/` |

## Formato do pedido (o que eu envio no Slack)

```
🔴 KIRA · APROVAÇÃO [kira-N]
O que: <uma frase, imperativa>
Por que: <o bloqueio, em uma linha>
Risco se aprovar: <o pior caso concreto>
Risco se recusar: <o que fica parado>
Responda: "aprovo kira-N"  ou  "recuso kira-N <motivo>"
```

- `N` é sequencial e único por pedido; aparece no PR/issue correspondente.
- Um pedido = uma pergunta. Nunca embutir duas decisões num mesmo `kira-N`.
- Sem resposta em **1 ciclo (3 h)**: o item fica `bloqueado` em
  `docs/BUILD-PROGRAM.md` e o ciclo seguinte pega outro item. Não reenvio o mesmo
  pedido mais de uma vez por dia.

## Formato da resposta (o que vale)

| Resposta do dono | Efeito |
|---|---|
| `aprovo kira-7` | autoriza **só** aquele pedido, e eu registro no PR |
| `recuso kira-7 <motivo>` | item marcado `bloqueado` com o motivo escrito |
| `aprovo kira-7 e kira-9` | autoriza os dois, nada além |
| `ok`, `pode`, 👍 sem ID | **não** autoriza — eu pergunto de novo com o ID |
| silêncio | não autoriza |

## Registro

- O pedido e a resposta ficam no histórico da conversa do Slack (é o log) **e** no
  PR/issue que o pedido destrava, em um comentário `Aprovado por @dono em <data>
  via Slack (kira-N)`.
- A lista de pendentes vive no `docs/BUILD-PROGRAM.md`, seção **Bloqueios**.
