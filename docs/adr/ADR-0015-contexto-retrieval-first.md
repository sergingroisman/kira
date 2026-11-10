# ADR-0015 — Contexto para o LLM: retrieval-first determinístico, sem LangGraph

- **Status**: aceito provisoriamente (10/Nov/2026) — recomendação do Orchestrator,
  **sujeita a veto do dono**. Um veto vira ADR que supersede este, não edição
  silenciosa.
- **Contexto**: dúvida técnica do dono sobre grep/leitura de arquivos e LangGraph
- **Decisores**: Orchestrator (recomendação) · Sérgio (veto/aval)

## Contexto

O `kira triage` hoje envia ao Ollama **apenas** os campos do work item
(`src/triage.rs:29`): ID, tipo, título, estado, área, responsável e descrição
limpa. Nenhum arquivo do repositório entra no prompt, e o `findings.json` do
`kira scan` não é enviado ao modelo.

Resultado observado em 10/Nov (`kira triage 529511` ao vivo, `qwen3.5:4b`): a
seção "Arquivos prováveis" saiu descritiva, porque o modelo **adivinha** a partir
do texto do bug. Não é defeito de prompt — é ausência de contexto ancorado.

Duas famílias de solução foram consideradas:

1. **Grafo de agentes com tools** (o modelo pede `grep`/`read_file` em loop,
   no estilo LangGraph);
2. **Retrieval-first**: o binário busca, o modelo só interpreta.

## Decisão

**Retrieval-first, em Rust, determinístico. Sem LangGraph. Sem tool-calling no MVP.**

O fluxo do context packer (PR5 + PR31):

1. o KIRA deriva termos do work item (título, descrição, nomes citados) e os
   nomes de arquivo presentes em `findings.json`;
2. busca determinística em duas fontes: **FTS5 da wiki** (`docs/wiki/`, ADR-0013) e
   **grep nos arquivos do stack detectado** (`walkdir` + `regex`, ambos já no
   `Cargo.lock` como transitivos);
3. monta trechos com **`caminho:linha`** — auditáveis, nunca blob anônimo;
4. poda por orçamento de tokens (`KIRA_NUM_CTX=4096`) com prioridade declarada;
5. faz **uma** chamada ao Ollama.

### Por que não LangGraph

- **Quebra o binário único**: LangGraph é Python/JS; exigiria sidecar.
- **Já está rejeitado** no mandato v3 §5 ("framework de orquestração/LangChain"),
  endossado pela juíza.
- **É redundante**: o "grafo" já existe e está no lugar certo — estados do ciclo no
  SQLite (`queued → packing → planning → awaiting_approval → applying → verifying
  → gated → reported`) com retomada por `kira resume`. Um framework externo
  adicionaria uma segunda fonte de estado, que é como se perde determinismo.

### Por que não tool-calling agora

Alvo de hardware do produto: **4B quantizado em 6 GB de VRAM** (RTX 2060) e M1 de
8 GB. Nesse envelope, tool-calling multi-turno é frágil: cada rodada reenvia o
contexto, o teto de 4096 estoura rápido, e erro de sintaxe de tool custa uma
chamada inteira. Isso destrói justamente a métrica que o gate M2 precisa medir.

Retrieval-first é testável sem modelo (as funções de seleção de trechos são puras)
e reprodutível — o mesmo requisito de determinismo do gate M1.

## Consequências

- **Positivas**: contexto ancorado em arquivo real; precisão mensurável; zero
  dependência nova pesada (`regex`/`walkdir` já estão no lock como transitivas);
  funciona offline; auditável (`caminho:linha` no prompt e no relatório).
- **Negativas**: o packer não "explora" — se a heurística de termos errar, o
  trecho certo não entra. Mitigação: o relatório do `triage` mostra o que foi
  enviado, e o `--eval` mede o efeito.
- **Evolução prevista**: tool-calling vira modo **experimental pós-M2**, com
  provedor remoto (Copilot), onde o contexto é maior e o modelo lida melhor com
  sintaxe de tool. Nunca substituindo o modo determinístico.

## Rejeitado

- LangGraph / LangChain / qualquer framework de orquestração (Python/JS, estado
  duplicado, já vetado no mandato).
- Tool-calling no MVP com modelo 4B local.
- Enviar o `findings.json` inteiro ou arquivos inteiros no prompt (estoura 4096 e
  piora a precisão por diluição).
- Busca vetorial/embeddings antes do gate M2 (ADR-0013 já fixa FTS5 primeiro).
