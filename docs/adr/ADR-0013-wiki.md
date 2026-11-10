# ADR-0013 — Wiki do KIRA: markdown como fonte de verdade, SQLite como índice

- **Status**: proposto (aguardando leitura do dono)
- **Origem**: estudo do `akitaonrails/ai-memory` (`docs/ARCHITECTURE.md`)
- **Rejeitado de propósito**: servidor, MCP, web UI, embeddings

## Contexto

O `kira triage` hoje não tem memória. Cada execução recomeça do zero: não lembra
que já viu aquele repositório, não lembra qual foi a causa raiz aceita do bug
semelhante de duas semanas atrás, e não tem como melhorar entre execuções. O
`triage` ao vivo de 10/Nov saiu **descritivo** justamente porque faltava contexto
— e o gate M2 (≥70%) depende de contexto, não de prompt melhor.

O `ai-memory` resolve isso para agentes de código com uma tese simples e boa:
**a wiki em markdown é a fonte de verdade; o SQLite é índice derivado,
reconstruível**. Isso respeita três proibições vigentes do KIRA ao mesmo tempo:
sem vector store, sem segundo DB (usa o `store.db` que já existe), sem daemon.

## Decisão

O KIRA ganha uma wiki local por projeto, em markdown, indexada por FTS5.

### Onde os arquivos vivem

```
<projeto>/.kira/wiki/
├── contexto.md       # arquitetura detectada: stack, módulos, comandos de build/test
├── decisoes.md       # decision log append-only (o "porquê" — o DeltaDB do KIRA)
├── padroes.md        # convenções inferidas e confirmadas pelo dono
├── glossario.md      # termos do domínio do projeto
└── bugs/
    └── <item-id>.md  # causa raiz (prevista + aceita), arquivos, evidência
```

`.kira/` continua **local e não comitado** (regra vigente do AGENTS.md). Quem
quiser compartilhar usa `kira wiki export` (PR32), que gera um bundle `.md` para
o usuário comitar onde quiser — a escrita no repo do usuário nunca é automática.

### Toda escrita passa pela fronteira de privacidade

Nenhum texto entra na wiki sem passar por `redact` (W1-PR2): PAT, token em URL,
e-mail e CPF são mascarados **antes** de tocar o disco. Body cru de HTTP nunca
entra. A wiki é um destino de escrita como qualquer outro.

### Índice derivado

No mesmo `store.db`:

```sql
CREATE TABLE IF NOT EXISTS wiki_page(
  slug TEXT PRIMARY KEY, path TEXT NOT NULL, kind TEXT NOT NULL,
  updated_at TEXT NOT NULL, content_hash TEXT NOT NULL);
CREATE VIRTUAL TABLE IF NOT EXISTS wiki_fts USING fts5(
  slug UNINDEXED, kind UNINDEXED, title, body);
```

- `kira wiki reindex` reconstrói do zero a partir dos `.md` — **o índice pode
  sempre ser jogado fora**, então corrupção de índice nunca perde conhecimento.
- `--no-fts5` (build sem FTS5) cai para busca por substring: degradação, nunca
  erro fatal (respeita o fail-open do store).

### Captura automática (episódica)

Depois de cada `triage`, o KIRA anexa um episódio com item id, achados do scan,
causa raiz prevista, confiança e arquivos — **sem pedir nada ao usuário**. É o
"captura o trabalho em silêncio" do ai-memory, reduzido ao mínimo.

- Episódios com mais de 30 dias são **colapsados** em uma linha de resumo na
  página do projeto (o "decay" do ai-memory), mantendo o diff de conhecimento.
- `kira wiki forget <slug>` apaga página e entradas de índice (LGPD: dado do dono).

### Consumo: é o context packer que faltava

O retrieval (PR31) alimenta o PR5:

1. termos do work item + nomes de arquivo dos findings → consulta FTS5;
2. top-k páginas (k limitado por orçamento de tokens, `KIRA_NUM_CTX`);
3. trechos entram no prompt **com o caminho da página**, para o dono auditar de
   onde veio a conclusão.

Sem embeddings até o gate M2 passar. Se a precisão não subir com FTS5, o
problema é o packer, não a falta de vetor.

## Consequências

- **Positivas**: conhecimento acumula entre execuções; o "porquê" fica escrito;
  auditável (`grep`), editável à mão (Zed), sem servidor nem instalação; alinhado
  ao DeltaDB, que você já usa — a wiki é o mesmo conceito, local e versionável.
- **Negativas**: wiki mal cuidada vira lixo (`contexto.md` desatualizado engana o
  triage). Mitigação: página de contexto tem `atualizado_em` e o prompt mostra a
  idade; acima de 30 dias o KIRA pede `kira wiki refresh`.
- **Custo**: 5 PRs (PR28–PR32). Nenhum muda o contrato read-only.

## Não-objetivos (explícitos)

Servidor HTTP, MCP, web UI, multi-usuário, embeddings/vetor, auto-improve com LLM
reescrevendo a própria wiki sem revisão humana, e qualquer escrita automática em
repositório do usuário.
