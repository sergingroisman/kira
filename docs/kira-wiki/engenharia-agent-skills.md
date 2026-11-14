# Engenharia — agent skills

O segundo degrau da escada ([[engenharia-harness-toolkit]]): a instrução
reutilizável, versionada e custodiada. O KIRA não é registry de conhecimento
alheio — as skills dele carregam **conhecimento do projeto**, com determinismo
e ciclo próprio. O formato, emprestado do estudo do registry de 94 skills
(`docs/mapa-completo-c4.md` §7, ADR-0027), entra na onda SK (SK-1, PR #107).

## O formato de skill

Uma skill é um markdown com frontmatter obrigatório — fonte de verdade em
markdown, como a wiki. O parser aceita e valida:

| Campo | Papel |
|---|---|
| `name` | identidade. É por ele que a skill do usuário substitui a bundled |
| `description` | carrega **trigger** ("Use when…") e **anti-trigger** ("Do NOT use…") — é o que o índice casa |
| `version` | semver — cadeia de custódia junto do autor |
| `author` | autoria |
| `license` | licença do conteúdo |

`version` + `author` dão a cadeia de custódia (mesmo espírito do
`TRIAGE_PROMPT_VERSION` do prompt de triage: prompt é código, tem dono e hash).
O corpo segue o formato dos SK.md estudados: instruções curtas, referências por
tópico, um ponto por lição. A validação do `add` reprova frontmatter
incompleto — skill sem autor/versão é anônima, e anônimo não entra.

## Onde as skills moram

Duas origens, mesma tabela de verdade:

1. **Bundled** — embutido no binário via `include_str!` (`skills/*.md` no repo
   do KIRA): zero instalação, determinístico, o catálogo acompanha o
   executável. No índice, os bundled vivem sob o `project_id` reservado
   `__bundled__` (nunca colide com id de projeto, que é hash FNV-1a).
2. **Usuário** — `<projeto>/.kira/skills/*.md`: local, nunca commitado (o
   `.gitignore` do `.kira/` já nasce com o diretório).

**Usuário vence bundled pelo `name`**: a listagem marca a origem
(`bundled`/`user`) e um `name` repetido do lado do usuário substitui o bundled.
Estender sem editar o binário; sobrescrever é explícito e visível.

## O índice: FTS5 derivado, com degradação para LIKE

Como a wiki (ADR-0013) e o repositório (ADR-0021), o índice de skills é
**derivado e descartável** — reconstruí-lo nunca perde conhecimento, porque a
fonte de verdade são os markdowns. No store: tabelas `skill`/`skill_fts`
(schema v10).

A busca (`kira skills search`) consulta o FTS5 com ranking bm25 e âncora de
custódia no hit: `skill:<nome>@<versão>`. **Fail-open honesto**: sem a tabela
FTS5 (ou sem suporte), a busca degrada para `LIKE` — mesmo truque da wiki
(frase entre aspas). Falha de leitura de uma skill vira nota, nunca erro do
comando: uma skill quebrada não cobra a inteira catálogo.

## Redact no parse

Regra da casa: **todo corpo e descrição passam por `crate::redact` antes de
indexar e antes de gravar**. Segredo não chega ao disco nem ao prompt — nem
quando quem escreve é o próprio dono colando um trecho de log na skill. Igual
ao índice de retrieval e à wiki: redação na escrita, não na leitura.

## O que já está no catálogo

Os bundled entregues na SK-1 (`skills/*.md` do repo, embutidos no binário):
`drawio`, `react`, `shadcn`, `tailwind`, `tanstack` — instruções de processo,
não catálogo de domínio alheio. O `kira skills list` mostra origem e versão
de cada um; o índice os agrupa sob `project_id = "__bundled__"`.

O que a fila reserva (ordem do plano SK): skills de **conhecimento do
projeto** geradas pelo próprio ciclo (kind de post novo), skills de processo
dos estudos (rtk/caveman: compressão no packer), e o degrau de uso — skill
entrando no pack do triage/loop como instrução versionada. Nada disso está
no binário hoje; [[contexto]] mantém a fila honesta.

## A CLI: `kira skills`

| Comando | O que faz |
|---|---|
| `kira skills list` | lista bundled + usuário, com origem e versão |
| `kira skills show <nome>` | mostra a skill inteira |
| `kira skills add <arquivo>` | valida o frontmatter (`name/description/license/author/version` semver), **redige o corpo** e grava em `.kira/skills/` |
| `kira skills search <termo>` | consulta o índice FTS5 derivado com âncora `skill:<nome>@<versão>` |
| `kira skills reindex` | reconstrói o índice a partir dos markdowns |

Sem LLM, sem rede: todo o catálogo roda offline. E nada de skill entra em
nenhum julgamento ainda — o degrau "skill na execução do triage/loop" é o
próximo passo da escada, não a SK-1.

## Trigger evals no aceite

Do relatório do registry (mapa §7): o que a onda SK leva além do formato é o
**aceite por trigger evals** — uma skill nova só entra se os triggers e
anti-triggers forem avaliados (a skill dispara quando deve e não dispara quando
não deve). É a mesma disciplina do prompt versionado com fixtures dourados
(P6 do ADR-0016 aplicado a instrução): sem medição, "Use when…" é opinião.
A cadeia completa emprestada do estudo, com destino por PR na fila do
ADR-0027:

- formato de skill (frontmatter, triggers, semver, autor) → SK-1 (entregue);
- custódia de prompt (PR48): scan anti-injection no CI para
  `src/llm/prompts.rs` + hash `(prompt_version, hash)` gravado na sessão;
- symlink guard no jail (PR47): `lstat`, alvo final, ELOOP — do instalador
  deles;
- trigger evals (aceite da SK).

## Por que markdown, e não JSON ou banco

Mesma resposta da wiki (ADR-0013): **fonte de verdade em markdown**. A skill
é texto que o dev escreve, lê, versiona no editor e coloca no repositório
quando quer; JSON de skill é outra coisa que ninguém edita. O índice
(`skill_fts`) existe para buscar, não para ser a verdade — por isso é
derivado, descartável e reconstruído com `reindex` sem custo. E por isso
`add` e `reindex` aceitam qualquer arquivo que valide: o formato não tem
runtime, tem conteúdo.

## Para onde a escada segue com skills

O que está desenhado para além da SK-1 (fonte: mapa de desejos do dono,
mapa §10):

- **RAG com os textos do dono** — um diretório `knowledge/` + o `skill_fts`
  existente servindo de índice de leitura;
- **skills/prompts com compressão** — o formato SK carrega a instrução; a
  compressão do conteúdo (estudo do caveman) entra no packer, economizando o
  que sobe ao modelo;
- **kind `getting-started`** — a narrativa de primeira jornada como kind de
  post do ciclo, publicada como as demais.

Nenhum desses itens está no binário hoje — [[contexto]] mantém a fila
honesta, e a regra da casa vale: nada é prometido como pronto se não está no
código.

## O teste que fixa o formato

`tests/skills_e2e.rs` spawna o binário real e fixa o contrato no limite do
CLI: a listagem mostra o catálogo bundled, o `add` valida, **redige** e lista
como `user`, frontmatter inválido é recusado, a busca é ancorada e
determinística, `show` devolve o corpo e nome desconhecido dá erro acionável.
Sem rede, sem LLM: puro catálogo + índice derivado em `KIRA_STORE_DIR`
isolado. Como todo e2e da casa, não há caminho de produto que os testes não
exercitem — o formato não é doc, é gate.

## Engenharia-nota do registry estudado (o que rendeu este formato)

Para contexto de onde vem cada peça (mapa §7):

- **Cadeia de custódia por SHA-256 único compartilhado** — registry →
  lockfile → verificação MCP: mesma ideia do `TRIAGE_SYSTEM_PROMPT_FNV1A64`
  do prompt de triage e do par `version + author` da skill;
- **Instalador defense-in-depth com symlink guard** (`lstat`, ELOOP,
  junctions) — vira PR47 no jail;
- **Validação editorial de skill** (triggers/anti-triggers, semver, autor,
  ≤500 linhas) — a parte que entrou no formato de frontmatter e nos trigger
  evals;
- **Bloqueio de supply chain (Snyk Agent Scan)** e marketplace SSG —
  reconhecidos como engenharia boa, fora do escopo do KIRA.

## O que o KIRA deliberadamente não copia

O registry deles empacota *conhecimento de domínio alheio* com custódia de
supply chain (SHA-256 único compartilhado, Snyk Agent Scan bloqueante,
marketplace, MCP com progressive disclosure). O KIRA gera *conhecimento do
projeto* — skills de cloud/design/GTM de terceiros ficam fora do domínio de
sustentação do KIRA (ADR-0027, rejeitado). O valor emprestado é o formato e a
disciplina, não o catálogo nem o transporte.
