# Decisões

Decisão no KIRA é **ADR**: um arquivo em `docs/adr/NNNN-titulo-em-kebab.md`,
versionado no repo, commitado com o código; número é imutável; ordem de
numeração é ordem de impacto; alternativa rejeitada e consequência declarada
são parte do formato. Esta página aponta para a fonte (a pasta `docs/adr/` é a
verdade, a wiki é espelho de leitura) e extrai as cinco decisões que moldam o
binário. Os invariantes que o código prova por cima delas estão em
[[padroes]]; os nomes canônicos, em [[glossario]].

## Os ADRs (índice `docs/adr/README.md`, ordem de impacto)

| Nº | Decisão (uma linha) |
|---|---|
| 0011 | Sandbox do `fix`: por que o agente não monta ambiente sozinho |
| 0013 | Wiki local em `.kira/wiki/` — markdown é fonte de verdade, índice derivado |
| 0014 | Dashboard com casca statui (TUI) |
| 0015 | Contexto: retrieval antes do modelo — pack ancorado em `caminho:linha` |
| 0016 | CLI-first, LLM-last: escada obrigatória e allowlist de módulos |
| 0017 | Autonomia por loop + `awaiting_approval` |
| 0018 | Gate M2 medido por `kira triage --eval`, sem LLM como juiz |
| 0019 | Copilot por spawn (`copilot -p`); Ollama é o default local |
| 0020 | Foundry BYOK: fronteira de dados e auth |
| 0021 | Índice de retrieval FTS5 (bm25), não embeddings — redigido na escrita |
| 0022 | Organização por domínio agora, camadas quando houver código compartilhado |
| 0023 | Gitingest: o que vale trazer (BOM/UTF-16, ignorados, orçamentos) |
| 0024 | Guardrails v2: injeção invisível (zero-width/bidi) + bloco delimitado no prompt |
| 0025 | `kira setup` ensina: aula do PAT, menu com pular, validação na hora |
| 0026 | Probe suite de segurança no `cargo test` |
| 0027 | Piso de guardas com veredito tipado, lições que provam que valem e custódia de prompt |
| 0028 | Cache de raciocínio e economia medida (ledger, cache-wiki, barra de qualidade) |
| 0029 | Vocabulário dos comandos (reservado, aguarda decisão) |
| 0030 | Local-first: integrações são capacidades, não pré-requisitos |
| 0033 | Dashboard: leitura por padrão, execução só com confirmação explícita |
| 0034 | `kira setup` com wizard de tela cheia sobre o mesmo motor de texto |
| 0035 | Foundry configurável pelo setup (formulário inline, chave no keyring) |
| 0036 | Camada visual do TUI: nativo do ratatui primeiro, dependência depois |
| 0037 | Resumo em cartões: chip vem da trilha, spinner só com filho vivo |
| 0038 | Ingestão, digestão e emissão: três verbos, três contratos |
| 0039 | Nomenclatura canônica: um nome por conceito |
| 0040 | Hugging Face é CATÁLOGO de GGUF via Ollama, nunca runtime de inferência |

(ADRs 0012 e 0031/0032 não constam no índice vigente do repo.)

## As cinco decisões que moldam o binário

### 1. Hugging Face é catálogo de GGUF, nunca runtime (ADR-0040)

Perguntado se vale acoplar Hugging Face em Rust, a resposta foi "onde isso
mora na arquitetura?". Candle, `llama-cpp-2` e `hf-hub` executariam GGUF
in-process — e trariam dois runtimes para o mesmo trabalho, binário gigante e
build de ML. O Ollama **já resolve** o acesso ao Hub (`ollama pull
hf.co/<repo>:<quant>`). Então: o KIRA orquestra (`kira model list` parseia
`ollama list`; `kira model pull` é o único caminho para GGUF do Hub — nenhum
código KIRA fala com a API da HF), quem executa o modelo é o Ollama, sempre.
Freios ao redor do download: consentimento explícito (sem `--yes` e sem TTY,
o pull nem começa), disco checado antes, piso de quantização **Q4_K_M** (Q2_K
proibido por política), gate de admissão por modelo (proveniência + SHA-256 +
template known-good + Ollama ≥ 0.17.1 + probes no GGUF quantizado). O budget
compartilhado garante que especialista novo não escapa do `llm_calls`.

### 2. Três verbos, três contratos: ingest → digest → publish (ADR-0038)

`kira ingest` (fatos, determinístico, sem LLM, segundos) · `kira digest`
(julgamento com IA local, cacheado, retomável) · `kira publish` (publica
atrás de gate por hash). Só o `ingest` entra no `kira setup`. A unidade de
trabalho é o **post** (tabela `posts`, view `outbox`), com fases
`claim → generate → deliver` e lease `draft_ttl`; a fila é **tabela, não
broker** (SQLite bundled, claim por `UPDATE … RETURNING` — Redis/Postgres
fura o local-first e não roda nativo no Windows). Ingestão completa em
qualquer stack por *denylist* + detector de stack (a allowlist de 34 extensões
não cobria `.mts`/`.xml`/manifesto `.txt` de 3 dos 11 repositórios de
referência). Medição que ancorou: o próprio repo do KIRA tem 181 arquivos
indexáveis e caminhada completa em 14 ms — orçamento em bytes de diretório
mede a coisa errada (o `target/` tem 4,5 GB).

### 3. Estaleiro não é componente: Hermes, Delta e Kombai ficam de fora do binário

`Hermes` (orquestra os ciclos de build) e `Delta` (escreve código) são agentes
de build; Kombai é a ferramenta de design do dono. Todos **constroem** o KIRA
junto com o dono — nenhum participa da execução. Por isso não aparecem em
nenhum nível do C4 nem em fluxo de runtime: os diagramas descrevem só o
binário rodando e os sistemas com que ele fala (dev, repo, Azure DevOps,
Ollama, keyring, ferramentas da stack). Os designs do frontend futuro vivem
como prints no README — ferramenta de design não é componente. O orquestrador
**interno** (runtime, quando existir como grafo) é o `brain` — outro conceito,
nome próprio outro ([[glossario]]).

### 4. Dose de thread: budget de chamadas, não dinheiro (e medido)

O orçamento do KIRA é **50 chamadas de LLM por dia** (default,
`max_llm_calls_per_day`), não R$ — Ollama local custa zero. O que deu dente à
decisão foi a auditoria: budget sem contador é promessa, então o P0 entregou
o `llm_calls` (schema v9), gravando backend/model/comando/tokens por chamada,
checado antes de cada chamada e de cada transição. Complemento de dose: o
digest tem orçamento duplo (1 M tokens/run + 30 k/post) e o cache por
`(input_hash, prompt_version, model, lang)` garante que repetir não paga
duas vezes. A onda TE (ADR-0028) completa a dose: **medir antes de otimizar**
— ledger de economia, cache de raciocínio (triage de bug conhecido pode
responder sem LLM, hit ≥ 0,50 de similaridade), barra de qualidade de
"corte ≥ 20%" para qualquer estratégia nova de packer. Sem número, a economia
é fé.

### 5. Container = agendador; o KIRA nunca é daemon

Não há serviço residente: `kira loop` é modo do binário que roda ticks em
primeiro plano e sai (`--once` para CI); agendamento externo (docker,
cron do SO) chama o binário — container é agendador, não runner residente.
Estado do ciclo vive no SQLite (`posts`, sessões), não em memória: um tick
retomável, idempotente por construção (ingest determinístico, cache do
digest, `published_hash` do publish). Por isso a fila é tabela com lease
`draft_ttl` e não um broker: quem agenda de fora só precisa conseguir rodar
um executável.

## As regras do índice de ADRs

De `docs/adr/README.md` — o processo que mantém o livro honesto:

1. **Um ADR = um arquivo** versionado no repo, commitado com o código; o
   histórico é o `git log` do arquivo e a discussão é o PR (padrão Nygard).
   Wiki é espelho de leitura, nunca fonte.
2. **Número é imutável** depois de mergeado: não se renumera, não se reusa,
   não se apaga — ADR que perdeu validade vira
   `Status: substituído por ADR-NNNN`.
3. **Ordem de numeração = ordem de impacto**: risco de segurança e decisões
   que travam outras (fronteira de dados, autonomia) vêm antes de
   conveniência. Por isso o índice não é cronológico.
4. **Reserva no PR**: a linha entra na tabela com `status: proposto` no mesmo
   PR que cria o arquivo — duas sessões paralelas nunca escolhem o mesmo
   número.
5. **Status**: `proposto` → `aceito` (só humano) → `substituído`. `rejeitado`
   é válido e fica registrado — histórico de tentativa é conhecimento.
6. **Teto prático**: até 4 ADRs abertos por onda; mais que isso é decisão
   grande demais escondida em documento.

O que **não** é ADR: plano de onda (vai em `docs/roadmap-prs.md`), relatório
de engenharia reversa e proposta de produto. ADR é decisão com alternativa
rejeitada e consequência declarada.

## As decisões do dono que dirigem a fila

Registradas no `docs/BUILD-PROGRAM.md` (10/Nov, revisadas 12–13/Nov — onde
conflitam, vencem as de 10/Nov):

- **Ordem das fases** F0→F5 como está; dashboard depois do loop.
- **Wiki é local** em `.kira/wiki/`; ciclo autônomo só grava em
  `.kira/wiki-staging/` e depende de `kira wiki promote`.
- **Modelo local**: ≤4B quantizado Q4_K_M, `KIRA_NUM_CTX=4096`
  (Windows + RTX 2060 6 GB e MacBook M1 8 GB são o perfil mínimo que o
  código assume; sem GPU cai para CPU e o doctor avisa que o triage é lento).
- **Agendamento só com o KIRA vivo** — sem serviço do Windows no MVP.
- **Budget = 50 chamadas LLM/dia, não R$** — teto em dinheiro só com
  provedor pago.
- **Cadência do ciclo de build: a cada 3 h, um PR por ciclo** — e um item
  por ciclo, nunca dois.
- **Ollama é o backend do `digest`, sempre primeiro**; Foundry é capacidade;
  Copilot nunca implícito.
- **Trust do repositório no onboarding**: `ingest` roda read-only sem trust;
  `digest` exige o aceite.
- **Fila em SQL, sem broker**; `tokio::broadcast` é só sino, nunca fonte da
  verdade.
- **1 M tokens por run** de `digest` + teto de 30 k por post.
- **Multi-stack desde o MVP**: denylist + detector de stack, não allowlist.
- **Idioma é dado, não código**: `.kira/**`, prompts e contratos internos
  sempre em inglês; o que o humano lê usa `output.lang`, escolhido no setup,
  com override por projeto — e `lang` entra no `cache_key`.
- **Domínio `post`**: a unidade de trabalho é o `post`; a lore de correio
  foi aposentada ([[glossario]]).

Bloqueios que esperam o dono (formato `🔴 KIRA · APROVAÇÃO [kira-N]`;
silêncio não é aprovação): os 20 bugs rotulados (trava o gate M2 inteiro),
número real do budget (default 50 até ele dizer), validação em hardware real
(RTX 2060/M1), policy do Copilot CLI na Avanade.

## Como uma decisão muda

Não é editar ADR: é ADR novo que supersede o anterior (exemplo real — o
ADR-0030 mudou na prática parte do fluxo do ADR-0025: o setup continua
ensinando, mas deixa de pedir PAT no primeiro contato). Um veto do dono vira
ADR que supersede — nunca edição silenciosa. Esta página é espelho: quando a
pasta `docs/adr/` mudar, atualize aqui sem mudar o número de nada.

## O que fecha o conjunto

Essas cinco decisões não estão sozinhas — cada uma é sustentada por
invariantes que o código prova ([[padroes]]: determinismo byte a byte, redact
antes do disco, `author ≠ verifier`, fail-open/fail-closed no lugar certo,
rendezvous por `UPDATE … RETURNING`) e nomeadas em um vocabulário só
([[glossario]]). Quando uma decisão mudar de valor, o caminho não é editar
esta página: é um ADR novo que supersede o anterior — número é imutável.
