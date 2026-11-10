# ADR-0016 — CLI-first, LLM-last: nada de LLM onde um comando resolve

- **Status**: aceito (decisão explícita do dono, 10/Nov/2026)
- **Decisores**: Sérgio (dono) · Orchestrator (Hermes)

## Contexto

O KIRA nasceu como CLI de manutenção e cresceu para "agente vivo". Em toda essa
superfície existe uma tentação permanente: pedir ao modelo o que um comando já
responde. Isso custa tokens, queima a janela de 4096 do Ollama local, introduz
não-determinismo e transforma um fato verificável em opinião.

O dono fixou a regra: **tudo que puder ser feito por comando de linha, o KIRA faz
por comando de linha — a LLM é o último recurso.**

## Decisão

### A escada de decisão (obrigatória, em ordem)

| Camada | Ferramenta | Exemplos no KIRA |
|---|---|---|
| **1** | CLI real do ecossistema | `dotnet list package --vulnerable --format json` · `pnpm audit --json` · `git log/diff/show` · `rg` · `gh` |
| **2** | Parser puro / DSL | JSON do audit → `Finding` · TOML de config · **SQL/FTS5** no `store.db` |
| **3** | Cálculo em Rust | fingerprint FNV-1a · severidade · budget · contagem · comparação de diff |
| **4** | **LLM (só quando 1–3 não respondem)** | hipótese de causa raiz · síntese em linguagem natural · classificação genuinamente ambígua |

Regra de parada: **suba a escada de baixo para cima e pare na primeira camada que
resolve.** Nunca comece pela 4.

### Teste de admissão para qualquer chamada de LLM

Toda chamada ao `LlmBackend` precisa satisfazer as três condições:

1. **Pergunta de julgamento**: a tarefa é julgar/interpretar, não recuperar,
   extrair, contar, comparar ou formatar?
2. **Nenhuma camada inferior resolve**: existe um comando/parser que responde? Se
   existe, é proibido usar LLM.
3. **Justificativa no código**: comentário `// LLM porque: <razão>` acima do call
   site, e o módulo consta na allowlist.

### Allowlist de módulos que podem chamar o LLM

```
src/triage.rs        hipótese de causa raiz sobre contexto já montado
src/loop/            reavaliação de hipótese quando a verificação falha
src/wiki/            consolidação de episódios em página (pós-M2)
```

Qualquer outro módulo (`scan`, `store`, `board`, `ado`, `guard`, `format`, `tui`,
`doctor`, `scheduler`) **não** importa `llm::*`. Isso é verificado por teste
(PR33), não por disciplina.

### Consequências operacionais

- **Budget de 50 chamadas/dia mede LLM, não trabalho.** Comando de CLI não consome
  budget — o KIRA pode fazer mil verificações determinísticas sem gastar nada.
- **`kira doctor` mostra a camada**: cada passo do ciclo reporta se veio de CLI,
  parser, cálculo ou modelo. Se um passo que deveria ser CLI caiu no modelo, é bug.
- **Determinismo é testável**: as camadas 1–3 têm teste sem modelo; o gate M2 mede
  só o que sobra para a camada 4.

## Consequências

- **Positivas**: custo baixo, comportamento reprodutível, prompt curto, auditoria
  simples ("de onde veio essa conclusão?" → camada + comando), e o produto melhora
  quando o modelo é pequeno (4B local), porque ele recebe menos trabalho.
- **Negativas**: mais código de integração (parsers por ferramenta) e manutenção de
  formato de saída de terceiros (`dotnet`, `npm`, `git`). Aceito: o repo já provou
  que faz isso bem no `scan`, com fixtures por formato.
- **Risco de má interpretação**: alguém pode achar que "nunca usar LLM" — não é
  isso. A camada 4 existe e é a razão do produto; o que se proíbe é usá-la para o
  que a camada 1–3 já entrega.

## Rejeitado

- "Mandar o repo inteiro no prompt e deixar o modelo procurar" — caro, lento e
  não-determinístico (ver ADR-0015).
- Usar LLM para parsear saída de ferramenta que tem `--json` — o `scan` já faz com
  parser puro e fixtures.
- Usar LLM para decidir orçamento/agendamento/contagem — aritmética é camada 3.
