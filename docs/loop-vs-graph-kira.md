# Estudo Comparativo — Loop Engineering vs. Graph Engineering & Pós-Spec-Driven no KIRA

> **Data**: 13/Nov/2026 · **Fontes analisadas (transcrições completas)**:
> 1. **Vídeo 1 (`IMLwvK08JVc`)** — *You Can Learn AI Agent Graph Engineering In 21 Min | Loop, Harness, System Design, Waku Agent* (Sean / Waku Agent)
> 2. **Vídeo 2 (`yKLedmyUDMA`)** — *App do Zero com IA: Harness, Sub Agents, Spec-Driven, Novas Skills e Workflows* (Waldemar Neto / Tech Leads Club)

---

## 1. Visão Geral: por que esses dois vídeos se completam

Os dois vídeos atacam o mesmo problema — **como sair de agentes imprevisíveis que queimam tokens para sistemas de engenharia confiáveis** — por dois ângulos complementares:

- **O Vídeo 1 (`IMLwvK08JVc`) olha para a topologia de execução (Loop vs. Graph):** quando deixar o modelo decidir o próximo passo em loop aberto e quando travar o fluxo num grafo determinístico (DAG) com etapas paralelas e estado explícito.
- **O Vídeo 2 (`yKLedmyUDMA`) olha para a disciplina de engenharia e contexto (Harness, Skills, Sub-agents e Spec-Driven → Spec-Lean):** como estruturar `AGENTS.md`, por que **Skills aposentam agentes fixos**, como usar sub-agents para **não poluir a janela de contexto**, onde o Spec-Driven clássico ajuda (modelos menores) ou atrapalha (babysitting em modelos fortes), e por que **o agente que implementa nunca deve validar o próprio trabalho (`author ≠ verifier`)**.

---

## 2. O que cada vídeo ensina (evidência extraída das transcrições)

### 2.1 Vídeo 1 (`IMLwvK08JVc`): A escada `Skill → Loop → Graph` no Waku Agent

1. **Skill (procedimento estrito):**
   - É uma instrução empacotada para o modelo seguir um procedimento padrão (SOP) sem que o humano precise repetir o contexto a cada prompt.
2. **Loop Engineering (descoberta passo a passo):**
   - Você dá um **objetivo aberto** + uma **caixa de ferramentas** (CLIs, APIs, MCPs) e diz: *"descubra o que fazer a seguir até atingir o objetivo"*.
   - **Como executa:** serial e reativo (`LLM → escolhe tool 1 → lê resultado → escolhe tool 2 → ... → resposta`).
   - **Onde brilha:** trabalho exploratório sem receita fixa (ex.: *deep research*, investigar um bug inédito).
   - **Onde quebra:** contexto cresce e fica ruidoso a cada turno; dependências ficam só na memória transitória do modelo; se uma etapa falha, o loop entra em *retry* infinito queimando tokens; o modelo pode reescrever o próprio plano no meio do caminho, destruindo a trilha de auditoria.
3. **Graph Engineering (topologia explícita com loops contidos):**
   - Quando um processo se repete e você **já sabe o formato do caminho** (SOP corporativo, triage, coleta de contexto), deixar o LLM decidir a ordem das ferramentas é desperdício e risco.
   - **Como executa:** nós (*nodes*: chamadas de ferramenta, chamadas de LLM, routers ou sub-loops) e arestas (*edges*: transições condicionais ou paralelas) pré-definidos pelo engenheiro.
   - **Os 2 grafos demonstrados no vídeo (Waku Agent):**
     - **Grafo `gather` (coleta paralela):** em vez de um loop chamar GitHub → depois Web → depois Calendário → depois Memória em série, o grafo dispara as **4 coletas em paralelo** e entrega tudo junto para um único nó de síntese.
     - **Grafo `triage`:** classifica na entrada se precisa de ação pesada enquanto já consulta o estado básico em paralelo, roteando para um loop apenas se necessário.
   - **A síntese do vídeo 1:**
     > *"Graph não substitui Loop — eles coexistem. Loop é quando o modelo decide um passo por vez. Graph é quando você conhece o formato e quer que as peças se movam juntas. Escreva a Skill primeiro; quando o workflow parar de mudar, consolide num Graph — mantendo Loops apenas dentro dos nós exploratórios."*

---

### 2.2 Vídeo 2 (`yKLedmyUDMA`): A evolução prática em 5 níveis (Waldemar Neto / TLC)

Construindo um ERP do zero, o vídeo percorre 5 níveis reais e mostra onde cada um falha:

1. **Nível 0 — Prompt cru:** pede a feature direto. Falha porque ninguém sabe o que será feito nem como provar que funcionou.
2. **Nível 1 — Plan Mode + Sub-agents como isolamento de contexto:**
   - **Sub-agent = thread descartável para poupar contexto:** abrir sub-agents para varrer API e frontend em paralelo e devolver **apenas o resumo** para a janela principal. A janela principal fica limpa para decidir.
3. **Nível 2 — Os dois sentidos de Harness:**
   - **Harness estrito (a ferramenta):** Cursor, Claude Code, OpenCode, KIRA — o motor que controla loops e tokens (um harness ruim gasta muito mais token para o mesmo modelo).
   - **Harness do desenvolvedor (o ambiente):**
     - **`AGENTS.md` minimalista:** colocar **só o que o modelo não deduz lendo o código** (ex.: como é o DI e a estratégia de testes por camada). *"O próprio código bem estruturado é o melhor harness que existe."*
     - **Sensores de feedback real:** obrigar validação real no fim (testes unitários em domínio, integração em rotas/banco, Playwright/E2E quando tocar em tela).
4. **Nível 3 — Por que Skills tornaram Agentes customizados quase obsoletos:**
   - Um agente fixo tem um papel só. **Skills são composáveis**: você instancia um executor genérico e injeta 2 ou 3 skills conforme a tarefa.
   - **Atenção ao orçamento de contexto das Skills:** o `name` e a `description` do frontmatter de **todas** as skills ficam carregados para o roteador decidir qual ativar — se você tiver dezenas de skills com descrições longas, elas comem a janela antes da tarefa começar.
   - **Entrevista 1 pergunta por vez (*one question at a time*) com opção recomendada:** na fase de *discover*, nunca despejar 10 perguntas juntas (porque a resposta da 2ª depende da 1ª).
5. **Nível 4 → 5 — De `Spec-Driven` pesado para `TLC Spec-Lean` + Verificador Independente:**
   - **O limite do Spec-Driven clássico (Spec + Design + 20 arquivos de Tasks):** funciona como "rodinha de bicicleta" essencial para **modelos menores/baratos** (que precisam de passo a passo curto), mas vira *babysitting* burocrático que engessa modelos fortes.
   - **O próximo passo (`Spec-Lean`):** 1 documento enxuto direto ao ponto (Spec `Given/When/Then` + regras de domínio + decisões técnicas) + **Checklist de verificação rodada por um Sub-agent independente**.
   - **A prova ao vivo no vídeo:** o agente implementador declarou *"terminei, tudo pronto"*. O sub-agent verificador rodou os 39 itens do checklist contra o código e os testes: **38 passaram, 1 falhou**.
   - **Lei de ouro do Vídeo 2:**
     > *"O agente que implementa nunca deveria validar o próprio trabalho."* (`author ≠ verifier`)

---

## 3. Comparação Direta: Loop vs. Graph vs. Spec-Lean Harness

```mermaid
flowchart TB
    subgraph LOOP["1. Loop Engineering (Exploratório)"]
        direction LR
        L_Goal["Objetivo + Tools"] --> L_LLM["LLM decide próximo passo"]
        L_LLM --> L_Tool["Chama 1 Tool"]
        L_Tool --> L_Obs["Observa output (contexto cresce)"]
        L_Obs -->|ainda não terminou| L_LLM
        L_Obs -->|acha que terminou| L_Done["Resposta"]
    end

    subgraph GRAPH["2. Graph Engineering (Topologia Determinística + Loops nos Nós)"]
        direction LR
        G_In["Entrada"] --> G_Par1["Nó 1: Scan/CLI"]
        G_In --> G_Par2["Nó 2: FTS5 Wiki/Repo"]
        G_In --> G_Par3["Nó 3: Skills/Knowledge"]
        G_Par1 & G_Par2 & G_Par3 --> G_Pack["Context Packer (Determinístico)"]
        G_Pack --> G_NodeLoop["Nó LLM (Loop contido / 1-shot)"]
        G_NodeLoop --> G_Gate{"Gate / Verifier\n(author != verifier)"}
        G_Gate -->|falhou (retry capado)| G_NodeLoop
        G_Gate -->|passou| G_Approve["awaiting_approval / Emit"]
    end
```

| Dimensão | Loop Puro | Graph Engineering (Vídeo 1) | Harness + Spec-Lean (Vídeo 2) | Como entra no KIRA |
|---|---|---|---|---|
| **Quem decide o fluxo?** | O LLM a cada turno | O grafo (arestas determinísticas) | A skill de workflow + gates | **Grafo determinístico em Rust** (ADR-0016); LLM só nos nós de julgamento |
| **Coleta de contexto** | Serial (1 tool call por turno, caro) | **Paralela** (grafo `gather` antes do LLM) | Sub-agents isolados devolvem só resumo | **Context Packer** (`scan` + `repo_fts` + `wiki_fts` + `skill_fts` + `knowledge_fts`) antes do modelo |
| **Poluição de contexto** | Alta (histórico acumula lixo) | Baixa (cada nó recebe só seu estado) | Baixa (`AGENTS.md` enxuto + sub-agents) | Janela fixa (`KIRA_NUM_CTX=4096`) montada do zero a cada nó |
| **Validação de saída** | O próprio loop diz "pronto" | Nó avaliador / aresta condicional | **Verificador separado** + sensores (testes/Playwright) | Gates Rust (`fmt/clippy/test`) + nó `verifier` separado do `planner` |
| **Adequação ao modelo** | Exige modelo caro (Opus/Sonnet/GPT-5) | Funciona com **modelo 4B local** porque o grafo pensa a estrutura | **Adaptativo**: Spec detalhada p/ modelo barato; Spec-Lean p/ modelo forte | **Perfil duplo**: modo guiado para `qwen3.5:4b` (local) e modo `spec-lean` para Foundry/Copilot |

---

## 4. O que isso muda e valida no caminho do KIRA

### 4.1 Três teses do KIRA que saem 100% confirmadas

1. **O KIRA já nasceu Graph-First sem usar a buzzword (ADR-0016 + ADR-0015):**
   - O vídeo 1 mostra que chamar tools uma por uma num loop de LLM é lento e caro; o grafo `gather` busca tudo antes e sintetiza uma vez.
   - O `kira triage` e o `kira ingest → digest → publish` fazem **exatamente o grafo `gather`**: o Rust busca work item, findings, wiki e código de forma determinística e entrega empacotado para 1 chamada de LLM. Isso explica por que o KIRA consegue rodar num `qwen3.5:4b` em 6 GB de VRAM onde um loop estilo Claude Code/OpenHands travaria no 3º turno.
2. **Skills > Agentes fixos (valida a Onda SK que acabamos de planejar):**
   - Em vez de criar 20 subcomandos de agentes fixos, o catálogo em `src/skills/` (`react`, `tailwind`, `tanstack`, `shadcn`, `drawio`...) injeta a especialidade como dado no nó de execução.
3. **`awaiting_approval` e gates determinísticos (ADR-0017):**
   - A prova do vídeo 2 (*"38 passaram, 1 falhou quando o agente disse que tinha terminado"*) é a justificativa empírica para o KIRA nunca aceitar "pronto" verbal do LLM sem gate determinístico.

---

### 4.2 Cinco ajustes cruciais que precisamos trazer dos vídeos para o KIRA

#### Ajuste 1 — `kira loop` (W5) deve ser implementado como um **State Graph determinístico** (e não um agent loop aberto)
- Na spec W5 (`docs/kira-w5-loop.md`), os estados já existem:
  `queued → packing → planning → awaiting_approval → applying → verifying → gated → reported`.
- **A regra vinda do Vídeo 1:** esses estados são **nós de um grafo em Rust**. O LLM **não** controla a transição de estados e **não** escolhe ferramentas em loop livre. O LLM só é chamado dentro do nó `planning` (para propor) e, se aprovado, no nó `applying`, com **orçamento estrito de retries** na aresta `verifying → applying`.

#### Ajuste 2 — Separar `Author` de `Verifier` no ciclo e no `digest`/`fix` (Vídeo 2)
- Nunca usar o mesmo prompt/chamada que gerou o código ou post para responder *"está tudo certo?"*.
- No KIRA:
  1. **Camada 1 (custo zero):** sensores determinísticos (compilador, `clippy`, testes, checagem de referências `caminho:linha` reais no disco).
  2. **Camada 2 (quando houver LLM):** chamada de verificação independente (baseada em checklist `Given/When/Then` extraída do item) que só recebe o **contrato + diff**, sem o histórico de racionalização do autor.

#### Ajuste 3 — Índice de Skills em 2 níveis (*Progressive Disclosure*) para proteger os 4096 tokens (Vídeo 2)
- O Waldemar alerta no Vídeo 2: carregar dezenas de skills no prompt consome a janela inteira.
- No nosso `docs/skills-rag-plano.md` (Onda SK):
  - **Nunca** listar todas as skills no system prompt.
  - O **roteador é determinístico (FTS5 + detector de stack `src/stack.rs`)**: se `stack::detect` achou `react`/`vite` e o work item cita `tabela`, o FTS5 seleciona no máximo `MAX_SKILLS=2` ou `3` (`react`, `tanstack`, `tailwind`) e **só elas** entram na janela do `qwen3.5:4b`.

#### Ajuste 4 — Modo Adaptativo por Capacidade do Modelo: `Guided Spec` (4B local) vs. `Spec-Lean` (Foundry/Copilot) (Vídeo 2)
- O Vídeo 2 mostra que **modelos menores precisam de Spec-Driven passo a passo**, enquanto **modelos grandes rendem mais com Spec-Lean** (plano único direto + checklist de verificação).
- O KIRA tem exatamente os dois mundos (`Ollama qwen3.5:4b` por padrão e `Foundry`/`Copilot` opt-in):
  - Quando `LlmKind::Ollama` (4B): decompor em micro-etapas guiadas pelo grafo em Rust.
  - Quando `LlmKind::Foundry` / `Copilot`: usar o formato **Spec-Lean** (1 contrato `Given/When/Then` + decisões técnicas + checklist de verificação no fim), economizando chamadas do budget diário de 50.

#### Ajuste 5 — Protocolo *"One Question at a Time + Recomendação Default"* nos fluxos interativos (Vídeo 2)
- O KIRA já faz isso intuitivamente no wizard do `kira setup` (ADR-0034). Devemos cravar como regra para qualquer futuro comando interativo de especificação/refinamento: **uma pergunta por vez, sempre com a opção recomendada no topo**, e em modo não-interativo/autônomo assumir o recomendado e registrar a suposição em 1 linha (conecta com o deadline de pergunta da onda TL / PR45).

---

## 5. Arquitetura C4 (Nível 3) do KIRA unindo Graph + Loop + Skills + Verifier

```mermaid
flowchart TB
    subgraph Entrada["1. Entrada & Roteamento Determinístico (Zero LLM)"]
        WI["Work Item / Tarefa"] --> Stack["stack::detect\n(Rust, Node, React, .NET...)"]
        WI --> KW["context::keywords()"]
    end

    subgraph GatherGraph["2. Grafo 'Gather' em Paralelo (Vídeo 1 — Retrieval-First)"]
        KW --> RepoFTS["repo_fts\n(código ancorado caminho:linha)"]
        KW --> WikiFTS["wiki_fts\n(episódios + arquitetura)"]
        Stack & KW --> SkillFTS["skill_fts (Onda SK)\n(top-k skills: react, tailwind...)"]
        KW --> KnowFTS["knowledge_fts (Onda SK)\n(livros e artigos do dono)"]
        WI --> Scan["scan::findings\n(vulnerabilidades/outdated)"]
    end

    subgraph Packer["3. Empacotamento com Orçamento"]
        RepoFTS & WikiFTS & SkillFTS & KnowFTS & Scan --> CP["Context Packer\n(Redact + Teto KIRA_NUM_CTX)"]
    end

    subgraph ExecutionGraph["4. Grafo de Execução Controlado (W5)"]
        CP --> PlanNode["Nó Planning (Loop contido)\nGuiado (4B) ou Spec-Lean (Foundry)"]
        PlanNode --> Approval{"Gate Humano\nawaiting_approval"}
        Approval -->|kira approve| ApplyNode["Nó Applying\n(1 passo / diff isolado)"]
        ApplyNode --> Sensors["Sensores Reais (CLI-First)\nfmt · clippy · test · refs"]
        Sensors -->|falhou (tentativa < max)| ApplyNode
        Sensors -->|verde| VerifierNode["Nó Verifier Independente\n(author != verifier · checklist)"]
        VerifierNode -->|reprovou item| ApplyNode
        VerifierNode -->|100% checklist| Done["Reported / PR Draft"]
    end
```

---

## 6. Sequência Recomendada das Ondas do KIRA ( consolidada pós-vídeos )

Combinando os 3 planos que desenhamos hoje (`ig-implementacao.md`, `skills-rag-plano.md` e este estudo):

1. **Onda IG (Ingestão v2 — repomix + gitingest):** garante que a matéria-prima do grafo `gather` seja perfeita (BOM UTF-16 `IG-0`, cascade `.kiraignore` `IG-1`, binários na árvore `IG-2`, detector de segredos `IG-3`).
2. **Onda SK (Catálogo de Skills + RAG de Livros/Artigos):** constrói a biblioteca de procedimentos (`src/skills/` com progressive disclosure via FTS5 + `stack::detect`) e a biblioteca pessoal (`src/knowledge/`). **Vem antes do loop (W5)**, exatamente como os dois vídeos recomendam (*"escreva as skills primeiro"*).
3. **Onda TE + TL (Economia de Tokens + Piso do Harness):** cache de raciocínio na wiki (PR35) + piso de 8 guardas (PR44) + validade das lições (PR46).
4. **Onda W5 (`kira loop` como State Graph + Verifier Independente):** implementa o ciclo autônomo não como um loop cego, mas como o **grafo de estados explícito** da Seção 5, com separação estrita `author ≠ verifier` e perfil adaptativo (`Guided` para Ollama 4B / `Spec-Lean` para Foundry/Copilot).
