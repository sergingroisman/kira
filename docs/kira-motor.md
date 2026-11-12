# KIRA — o motor (setup → crons, cabo a rabo)

> Como o KIRA funciona por dentro, para quem precisa explicá-lo sem ler 20 arquivos.
> Módulos e tabelas conferidos no código em 12/Nov/2026.

## 1. Em 30 segundos

Um **binário Rust único** que tem três camadas de memória:

1. **`store.db`** (SQLite + FTS5) — fatos do projeto, sessões, findings, índice de busca.
2. **`.kira/wiki/`** (markdown) — conhecimento durável: episódios de bug, decisões.
3. **Config/keyring** — org, projeto, nome do dev, backend de IA, PAT (no keyring do OS).

Tudo que **lê** é determinístico e sem token. Só `triage` **gasta** token, e só quando você manda.

## 2. Mapa de módulos

| Módulo | Responsabilidade | Invariante |
|---|---|---|
| `main.rs` | clap: comandos, flags, dispatch, `session_context` | nenhum comando escreve no mundo sem `--apply` |
| `setup.rs` | wizard que ensina (PAT, IA, nome) | entrada de segredo nunca ecoada |
| `doctor.rs` | diagnóstico por item, com causa e comando de correção | falha vira instrução, não stack trace |
| `credentials.rs` | resolução PAT/config; `KIRA_SECRET_STORE=off` | env > keyring > config; nada de segredo em log |
| `ado/` | cliente Azure DevOps (work item, projeto) | 401/403/404 com causa provável |
| `scan/` | detecta stacks, resolve ferramenta, roda audit, gera findings | determinístico; ferramenta ausente = erro acionável |
| `binresolve.rs` | resolução "which-style" (PATHEXT no Windows) | nunca classifica erro por texto localizado |
| `store/` | SQLite, WAL, migrações por `user_version` | abrir o store não muda conteúdo |
| `wiki/` | páginas markdown + índice FTS5 + episódios | episódio vazio não é gravado |
| `context/` | monta o pack (findings + wiki + código) com orçamento | mesmo estado ⇒ mesmo pack |
| `llm/` | 3 backends + prompts versionados | caro nunca implícito, nunca fallback silencioso |
| `triage.rs` | prompt, chamada, parse em 3 seções, grava sessão/episódio | fail-closed |
| `eval.rs` | mede o gate M2 contra gold | sem LLM juiz; falha aborta |
| `prdraft.rs` | PR em markdown do que já se sabe | anti-invenção: falta de dado vira `—` |
| `redact.rs` | mascara segredo/PII **antes** de qualquer prompt | invisíveis removidos antes do masking |
| `guard/` | caminho hostil, slug, escrita contida | nada escapa do diretório |
| `tui/` | dashboard read-only | `decide()` é puro e testado; tecla não escreve |

## 3. Persistência — o que fica onde

`store.db` (em `%USERPROFILE%\.config\kira\` no Windows, `~/.config/kira/` no resto):

| Tabela | Guarda |
|---|---|
| `projects` | raiz do projeto, primeira vez visto |
| `sessions` | cada execução: comando, entrada, saída, custo, timestamp |
| `facts` | notas do projeto (`kira note`) |
| `wiki_page` | metadados das páginas (o markdown vive em `.kira/wiki`) |
| `wiki_fts` | índice FTS5 das páginas (bm25) |
| `repo_file` | arquivos do repo com mtime+tamanho |
| `repo_fts` | índice FTS5 do código (retrieval incremental) |

`.kira/`: `wiki/` (conhecimento) e `reports/` (relatórios). **Nada disso vai para o git por padrão.**

## 4. Precedência de configuração

```
flag de CLI  >  variável de ambiente  >  keyring/config do projeto  >  default
```

Exemplos: `--llm` > `KIRA_LLM` > config; `--org` > `AZURE_DEVOPS_ORG` > config; `--pat` >
`AZURE_DEVOPS_PAT` > keyring. É essa ordem que permite testar sem tocar na máquina
(`KIRA_SECRET_STORE=off` desliga o keyring, `KIRA_CONFIG` aponta para arquivo de teste).

## 5. Segurança — as quatro travas

1. **Redação antes do prompt** (`redact.rs`): tokens, e-mails, CPF → `[REDACTED]`; caracteres
   invisíveis/bidi são removidos **antes** do masking (senão o regex não casa e o segredo vaza).
2. **Prompt delimitado** (`prompts.rs`, v2): conteúdo do repo/work item vai entre
   `<<<CONTEÚDO NÃO CONFIÁVEL` e `FIM DO CONTEÚDO NÃO CONFIÁVEL>>>`, com regra anti-instrução no system.
3. **Caminho hostil** (`guard/path.rs`): `../`, NUL, absoluto, `.git/` são recusados antes de I/O.
4. **Segredo nunca ecoado**: `doctor`/`setup` mostram só máscara (`Aa2x****`).

## 6. Orçamento e determinismo

- `CHARS_PER_TOKEN = 3` (PT-BR é mais denso que inglês; subestimar é melhor que inflar).
- O pack tem teto por fonte (código, wiki, findings) e `--explain` mostra **por que** cada item entrou.
- **Finding** tem fingerprint estável: mesmo problema ⇒ mesmo fingerprint entre scans.
- `pr draft` e `eval` são byte-determinísticos: mesma entrada, mesma saída (provado por hash nos testes).

## 7. Backends de IA (`LlmKind`)

| Backend | Custo | Quando |
|---|---|---|
| `ollama` | 0 (local) | **default**; `qwen3.5:4b` cabe em 6 GB |
| `copilot` | assinatura | spawn do CLI, sob demanda |
| `foundry` | por token (BYOK) | HTTP, superfície v1/legacy, body por família de modelo |

Sem backend disponível ⇒ **erro acionável e nenhum artefato**, nunca resposta vazia fingindo sucesso.

## 8. O dashboard (TUI)

`decide(estado, tecla) → Outcome` é **função pura**; o loop só executa. Read-only provado por hash do
store nos testes. 4 painéis: cabeçalho (identidade/status), achados, sessões, wiki.

## 9. Os crons

| Job | Cadência | O que faz | Trava |
|---|---|---|---|
| `kira-autobuild` | 30 min | pega **um** item da fila do BUILD PROGRAM, implementa em branch, roda gates, abre PR | responde `ocioso:` e não faz nada se: árvore suja, rebase em curso, branch ≠ `main` ou **qualquer PR aberto** |

Estado atual: **pausado** (motivo registrado) — para não colidir com sessões paralelas (OpenCode/Kombai).
Nota honesta: entrega em `slack:D0BBXPLB7A6`, um canal que aparenta estar morto; o valor real do job é
o PR que ele abre, não a mensagem.

## 10. Determinístico vs. modelo — onde mora o custo

| Etapa | Usa LLM? | Consequência |
|---|---|---|
| `scan`, `wiki`, `context`, `pr draft`, `eval`, `dashboard` | **não** | custo zero, resultado reprodutível |
| `triage` (ollama/copilot/foundry) | **sim** | único ponto que gasta token; sempre explícito |

Sete das onze jornadas não gastam token: é a razão de o KIRA poder ficar ligado.

## 11. Matriz jornada × módulo

| Jornada | Módulos que toca |
|---|---|
| `setup` | setup · credentials · doctor · llm (probe) |
| `scan` | scan · binresolve · store · guard |
| `wiki` | wiki · store (fts) · guard · redact |
| `context` | context · repo · store (fts) · redact |
| `triage` | triage · context · llm · prompts · redact · store · wiki |
| `pr draft` | prdraft · store · wiki · redact |
| `eval` | eval · triage · llm |
| `dashboard` | tui · store · wiki |
| `inspect` | ado · credentials |
| `loop` (W5) | todos os acima, sob budget e `awaiting_approval` |
