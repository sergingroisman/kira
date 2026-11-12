# KIRA — as jornadas (mapa completo, do binário real)

> Levantado em 12/Nov/2026 a partir do `kira --help` e de cada subcomando (não de memória).
> Fonte viva: `src/main.rs` (clap). Toda jornada aqui é **read-only por padrão** — escrita no mundo
> exige `--apply` + `approval` humano.

## 0. Mapa geral (quem alimenta quem)

```mermaid
flowchart LR
  ADO[Azure DevOps<br/>work item] -->|inspect / triage ID| KIRA
  FS[Repositório<br/>package.json, lockfiles] -->|scan| KIRA[KIRA]
  KIRA --> STORE[(store.db<br/>SQLite + FTS5)]
  KIRA --> KIRADIR[.kira/<br/>wiki · reports]
  STORE --> CTX[context pack<br/>budget por fonte]
  KIRADIR --> CTX
  CTX --> LLM[Ollama · Copilot · Foundry]
  LLM --> TRI[triage]
  TRI --> PRD[pr draft .md]
  STORE --> DASH[dashboard<br/>4 painéis]
  KIRADIR --> DASH
  GOLD[gold set 20 bugs] --> EVAL[triage --eval]
  TRI --> EVAL
  EVAL -->|>=70%| LOOP[kira loop<br/>W5 — bloqueado]
```

**Artefatos**: `store.db` (sessões, findings, episódios, índice FTS5) e `.kira/` (wiki em markdown,
relatórios). Nada disso vai para o git por padrão (`PR37` fechará o mecanismo do `.gitignore`).

---

## J0 — Onboarding: virar um dev com KIRA configurado

```mermaid
flowchart TD
  A[kira setup] --> B{TTY?}
  B -->|sim| C[Menu PAT<br/>1 colar · 2 env · 3 pular]
  B -->|não / --yes| D[usa AZURE_DEVOPS_PAT<br/>senão pula]
  C --> E[Valida PAT agora<br/>200/401/403/sem rede]
  D --> F[nome do dev]
  E --> F
  F --> G[Backend de IA<br/>ollama · copilot · foundry]
  G --> H[Resumo: pronto / pulado / próximo comando]
  H --> I[kira doctor]
  I --> J{status por item}
  J -->|falta algo| K[repete a explicação curta<br/>e aponta o comando]
```

- **Ensina antes de pedir** (ADR-0025): PAT é explicado como "senha descartável de leitura".
- **Nunca bloqueia**: todo passo tem "pular", e o resumo diz o que foi pulado e como retomar.
- **Falhas**: 401 (token expirado/copiado errado), 403 (falta escopo `Work Items → Read`), sem rede
  ⇒ salva e manda validar depois com `doctor`.

## J1 — Percepção: o que o projeto tem de errado

```mermaid
flowchart TD
  A[kira scan --path DIR] --> B[detecta stacks<br/>node · dotnet]
  B --> C{workspace pnpm?}
  C -->|sim| D[ROOT: lê members do pnpm-workspace.yaml<br/>não roda npm]
  C -->|não| E[cada package.json]
  B --> F[resolve a ferramenta no PATH<br/>PATHEXT no Windows]
  F --> G[roda audit / list package]
  G --> H[parse tolerante<br/>BOM/UTF-16 + formato real]
  H --> I[Findings com fingerprint]
  I --> J[finding.json<br/>determinístico]
  I --> K[(store)]
```

- **Determinístico**: mesma entrada ⇒ mesmo JSON (sem LLM nesta fase).
- **Falhas**: ferramenta ausente ⇒ erro acionável (não silêncio); formato inesperado ⇒ erro de parse,
  nunca "0 achados" mentiroso.

## J2 — Memória do projeto (identidade e trilha)

```mermaid
flowchart LR
  A[kira profile] --> B[root, primeira vez,<br/>todas as notas]
  C[kira note TEXTO] --> D[(config do projeto)]
  E[kira note --remove N] --> D
  F[kira history --last N] --> G[(sessões no store)]
```

## J3 — Conhecimento: a wiki que aprende

```mermaid
flowchart TD
  A[kira wiki list] --> B[páginas em .kira/wiki]
  C[kira wiki show PAGINA] --> D[render no terminal]
  E[kira wiki decision TEXTO] --> F[append em decisoes.md<br/>redigido antes de escrever]
  G[kira wiki reindex] --> H[índice FTS5 derivado]
  H --> I[kira wiki search TERMO<br/>bm25]
  J[kira wiki forget PAGINA] --> K[apaga do disco E do índice<br/>LGPD]
  L[kira wiki promote] --> M[páginas de ciclo não assistido<br/>=> wiki oficial]
```

- A wiki é **conhecimento**, não log de chat: episódio vazio não é gravado.
- `forget` apaga de verdade (disco + índice) — direito ao esquecimento.

## J4 — Contexto: o que chegaria ao modelo

```mermaid
flowchart TD
  A[kira context ID ou --file] --> B[busca: findings + wiki FTS5 + trechos de código]
  B --> C[orçamento por fonte<br/>CHARS_PER_TOKEN=3]
  C --> D[context pack]
  D --> E[--explain: por que cada item entrou<br/>fonte, rank bm25, prioridade]
  D --> F[--json: recall evaluation]
```

- **Zero token, zero rede**: é o "ver antes de pagar". Fail-open se índice ausente ⇒ grep + aviso.

## J5 — Julgamento: o triage

```mermaid
flowchart TD
  A[kira triage ID / --file] --> B[context pack]
  B --> C{--llm}
  C -->|ollama| D[local, barato]
  C -->|copilot| E[spawn CLI<br/>custa assinatura]
  C -->|foundry| F[BYOK HTTP<br/>custa por token]
  D --> G[resposta em 3 seções<br/>causa raiz · evidências · próximo passo]
  E --> G
  F --> G
  G --> H[(sessão no store)]
  G --> I[episódio na wiki]
```

- **Caro nunca implícito**: sem fallback silencioso entre backends (ADR-0019/0020).
- **Fail-closed**: backend indisponível ⇒ erro acionável, nunca resposta vazia fingindo sucesso.
- **Isolamento**: segredo e PII são redigidos antes de qualquer prompt.

## J6 — Escrita: o PR em markdown

```mermaid
flowchart LR
  A[kira pr draft ID] --> B[lê triage + findings + wiki do store]
  B --> C{sem dado?}
  C -->|sim| D[seção com — e a causa<br/>nada inventado]
  C -->|não| E[Contexto · Causa raiz<br/>Mudança · Evidências · Riscos · Checklist]
  E --> F[stdout / --out arquivo / --json]
```

- **Sem rede, sem LLM**: só reorganiza o que já foi apurado, com evidência citada (`path:linha`).
- Não abre PR, não faz push: **merge e abertura são humanos**.

## J7 — Medição: o gate M2

```mermaid
flowchart TD
  A[gold set: 20 pastas<br/>item.json + gold.json] --> B[kira triage --eval DIR]
  B --> C[triage offline por item]
  C --> D[compara causa: tokens normalizados ≥0.50]
  C --> E[compara arquivo: basename]
  D --> F[tabela por item + linha do gate]
  E --> F
  F --> G{>=70%?}
  G -->|sim| H[exit 0 — PASSOU<br/>autonomia liberável]
  G -->|não| I[exit 1 — NÃO PASSOU]
```

- **Sem LLM juiz**: métrica determinística e auditável. Item que falha aborta a medição (nunca parcial).
- `--seed` cria 3 fixtures sintéticas para testar o instrumento antes de rotular os 20.

## J8 — Vigilância: o dashboard

```mermaid
flowchart TD
  A[kira dashboard] --> B{TTY?}
  B -->|não| C[--snapshot: texto e sai<br/>mesmo conteúdo, exit 0]
  B -->|sim| D[TUI ratatui]
  D --> E[4 painéis: cabeçalho · achados · sessões · wiki]
  E --> F[Tab cicla · j/k move só o foco]
  E --> G[Enter abre detalhe/página · Esc volta]
  E --> H[q ou Ctrl+C saem]
  E --> I[? mostra causa COMPLETA das pendências]
```

- **Read-only provado por hash do store** nos testes: uma tecla não escreve nada.

## J9 — Inspeção viva (Azure DevOps)

```mermaid
flowchart LR
  A[kira inspect ID] --> B[ADO API com PAT]
  B --> C[resumo legível do work item]
  B --> D[401/403/404 acionáveis]
```

## J10 — Autonomia (desenhada, bloqueada)

```mermaid
stateDiagram-v2
  [*] --> idle
  idle --> observe: tick
  observe --> decide
  decide --> idle: nada mudou (0 token)
  decide --> leitura: item/achado novo
  leitura --> record: LLM (budget)
  record --> idle
  record --> awaiting_approval: quer escrever
  awaiting_approval --> applying: kira approve (humano)
```

- Spec completa: `docs/kira-w5-loop.md`. **Bloqueada pelo gate M2** (J7 ≥70%). Sem ele, o loop fica
  em **modo sugestão**: produz `pr draft` e relatório, nunca abre PR.
- Watchdogs: budget/dia verificado antes de cada transição, backoff após 3 ciclos sem novidade,
  `loop.lock` contra instância dupla, `kira loop stop` como kill switch.

---

## Resumo: 11 jornadas, 4 tipos

| Tipo | Jornadas | Característica |
|---|---|---|
| **Configuração** | J0, J9 | fala com o mundo (ADO), ensina e valida |
| **Percepção** | J1, J2, J3, J4 | 0 token, determinística |
| **Julgamento e escrita** | J5, J6 | consome token; caro é explícito |
| **Controle** | J7, J8, J10 | mede, vigia, e só age com aprovação |
