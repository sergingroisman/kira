# ADR-0030 — Local-first: integrações são capacidades, não pré-requisitos

**Status**: proposto (12/Nov/2026) · **Substitui na prática parte do fluxo do ADR-0025** (o setup
continua ensinando, mas deixa de pedir PAT no primeiro contato) · **Registro**: linha no
`docs/adr/README.md`

## Contexto

O `setup` atual pede o PAT do Azure DevOps logo na primeira execução. Isso nasceu de uma premissa
errada: a de que o **board é a fonte da verdade**. Para o dev que instala o KIRA na máquina dele, a
fonte da verdade é o **repositório local** — e a maior parte do valor do produto (scan, wiki,
contexto, draft de PR, dashboard) não precisa de credencial nenhuma.

Consequências observadas do fluxo invertido:
- pede credencial corporativa antes de provar qualquer valor (barreira de adoção em empresa);
- falha em ambiente onde o board não existe (repo pessoal, projeto open source, máquina offline);
- mistura "não tenho PAT" com "o KIRA não funciona", quando são coisas independentes;
- viola o princípio do menor privilégio: passo de leitura local pedindo token de leitura remota.

## Decisão

**1. O KIRA funciona completo sem nenhuma integração externa.** Toda jornada local roda com zero
credencial: `scan`, `codex` (wiki), `lens` (context), `scribe` (pr draft), `watch` (dashboard) e
`judge --file` (julgamento com backend de IA local).

**2. Integração é uma capacidade (`Capability`) que se conecta quando faz sentido.**
Capacidades iniciais: `board.azure`, `scm.github`, `scm.gitlab`. Cada uma tem estado
(`desconectada | conectada | inválida`) e é registrada **por projeto** no store.

**3. O setup pergunta primeiro o que é local.** Nome do dev → projeto → IA (com a opção explícita
"nenhuma por agora") → resumo do que funciona e do que está desativado. **PAT não é pedido no setup.**

**4. Credencial só nasce em `kira connect <provider>`.** É lá que mora a aula (o texto do ADR-0025),
a validação imediata (200/401/403/sem rede) e a gravação no keyring. A aula não foi descartada: foi
movida para o momento em que o dev entende por que precisa dela.

**5. Comando que exige capacidade falha com mensagem de capacidade, nunca com 401/404 cru.**
Formato: `esta jornada precisa de <capacidade> — rode: kira connect <provider>` (exit code próprio,
sem stack trace, sem tentativa de rede).

**6. Nada é conectado implicitamente.** Sem fallback silencioso, sem tentar o board porque a env
existe: se a capacidade não está conectada, o comando de board não executa.

## Matriz de capacidades (contrato de teste)

| Jornada | Capacidade | Backend de IA | Comportamento sem capacidade |
|---|---|---|---|
| `scan` | — | — | roda |
| `codex` (wiki) | — | — | roda |
| `lens` (context) | — | — | roda |
| `scribe` (pr draft) | — | — | roda (markdown local) |
| `watch` (dashboard) | — | — | roda |
| `judge --file` | — | obrigatório | sem IA: `esta jornada precisa de backend de IA — rode: kira setup --backend ollama` |
| `judge <id>` | `board.azure` | obrigatório | `esta jornada precisa de board.azure — rode: kira connect azure` |
| `read` (inspect) | `board.azure` | — | idem |
| publicar PR / abrir issue | `scm.github` ou `scm.gitlab` | — | `esta jornada precisa de scm.github — rode: kira connect github` |
| `verdict` (`--eval`) | — | obrigatório | sem IA: erro acionável (nunca medição parcial) |
| `sentinel` (W5) | — | obrigatório | bloqueado pelo gate M2 (ADR-0018) |

## Alternativas rejeitadas

- **Manter PAT no setup com a opção "pular"** (estado atual): o dev ainda é recebido por uma pergunta
  de credencial corporativa; a mensagem que passa é "isto é uma ferramenta da empresa", quando o
  produto é uma ferramenta do dev.
- **Wizard único com todas as integrações no início**: quem não tem board (a maioria dos repos
  pessoais) trava no primeiro passo, e o KIRA vira "ferramenta que não instala".
- **Detectar credencial por variável de ambiente e conectar sozinho**: conveniente e perigoso —
  transforma presença de segredo em consentimento. O dev precisa dizer que quer conectar.
- **Guardar credencial em arquivo de projeto**: viola o contrato de não versionar segredo; keyring do
  OS é o lugar (como já decidido no ADR-0025).

## Consequências

Boas:
- instalação e primeiro valor em **menos de um minuto**, sem falar com a TI;
- KIRA utilizável em repo pessoal, open source, máquina offline e CI sem segredo;
- fronteira de dados explícita: nada sai da máquina até uma capacidade ser conectada;
- argumento de adoção: "instala, roda, e só conecta o board quando quiser".

Ruins (assumidas):
- **schema novo** (`connect_<provider>` por projeto) e migração de `user_version`;
- duas mensagens de erro novas que precisam ser mantidas alinhadas com a matriz (vira teste de contrato);
- o dev que já tinha PAT no keyring não é conectado automaticamente — precisa de um `connect` (fricção
  única, proposital).
- `doctor` ganha uma seção de capacidades (o que está conectado, o que está desativado).

## Implementação (para o Kombai)

1. `src/capabilities.rs` (novo): enum `Capability { BoardAzure, ScmGithub, ScmGitlab }`, estado por
   projeto no store, leitura/escrita, mensagem de erro padronizada.
2. `src/main.rs`: subcomando `kira connect <provider>`; remover o passo de PAT do `setup`; ligar o
   gating em `triage <id>`, `inspect` e nos comandos de publicação.
3. `src/error.rs`: variante `CapabilityMissing { capability, command }` com a mensagem e exit code.
4. `setup`: novo fluxo local-first com a opção "nenhuma IA por agora".
5. Testes: matriz acima como tabela de casos (cada linha = teste que roda o binário e confere a
   mensagem), mais `connect` com sucesso/falha e `doctor` reportando capacidades.
