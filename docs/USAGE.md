# Kira — Guia de Uso

> Documentação completa de todas as funcionalidades disponíveis no harness Kira.

## Sumário

- [Início Rápido](#início-rápido)
- [Modos de Execução](#modos-de-execução)
- [Tiers de LLM](#tiers-de-llm)
- [Interface TUI](#interface-tui)
- [Keybindings](#keybindings)
- [Slash Commands](#slash-commands)
- [Overlays](#overlays)
- [Sidebar](#sidebar)
- [Layers View](#layers-view)
- [HITL — Aprovação de Ferramentas](#hitl--aprovação-de-ferramentas)
- [Memória Semântica](#memória-semântica)
  - [Decay](#decay-decaimento-exponencial)
  - [Consolidação](#consolidação)
  - [Session Handoff](#session-handoff)
  - [API de Maturation](#api-de-maturation)
  - [Configuração](#configuração-1)
  - [Prompts Editáveis](#prompts-editáveis)
  - [Degradação Graciosa](#degradação-graciosa)
- [Fila de Mensagens](#fila-de-mensagens)
- [Pipeline SDD](#pipeline-sdd)
- [Waves — Execução Paralela](#waves--execução-paralela)
- [Agentes](#agentes)
- [Lessons](#lessons)
- [Conexão e Resiliência](#conexão-e-resiliência)
- [CLI Não-Interativo](#cli-não-interativo)
- [Configuração](#configuração)
- [Eventos SSE](#eventos-sse)

---

## Início Rápido

```bash
# Terminal 1: daemon
cargo run -p kira-core

# Terminal 2: TUI
cargo run -p kira-cli

# Ou modo não-interativo:
cargo run -p kira-cli -- chat "explique o código em src/main.rs"
```

Requisitos: `rustc 1.85+`, terminal mínimo 80×24.

---

## Modos de Execução

| Modo | Comportamento | Toggle |
|------|--------------|--------|
| **YOLO** | Executa automaticamente sem pedir confirmação (exceto HIGH/CRITICAL) | `Ctrl+M` ou `/mode yolo` |
| **PLAN** | Requer aprovação antes de executar ações | `Ctrl+M` ou `/mode plan` |

O modo atual é exibido no header da TUI. O toggle é uma involução (duplo toggle retorna ao original).

---

## Tiers de LLM

| Tier | Uso | Comando |
|------|-----|---------|
| **heavy** | Tarefas complexas, raciocínio profundo | `/tier heavy` |
| **standard** | Uso geral (padrão) | `/tier standard` |
| **light** | Tarefas simples, baixo custo | `/tier light` |

Cada tier mapeia para candidatos de providers configurados em `tokens.toml`. O roteamento é transparente — fallback automático entre providers.

---

## Interface TUI

A TUI é organizada em camadas de renderização:

```
┌─────────────────────────────────────────────────────┐
│ Header (modo, tier, status, modelo, custo)          │
├──────────┬──────────────────────────────────────────┤
│ Sidebar  │ Chat Panel (mensagens, streaming)        │
│ (42 col) │                                          │
│          │                                          │
│          ├──────────────────────────────────────────┤
│          │ Input Area (3-10 linhas)                 │
├──────────┴──────────────────────────────────────────┤
│ Footer (workspace, git, LSP, MCP, hints)            │
└─────────────────────────────────────────────────────┘
```

Camadas de renderização (z-order):
1. **Base**: header, sidebar, chat, input, footer
2. **Layers View**: pipeline visualization (entre base e overlays)
3. **Overlays**: command palette, session picker, task browser, help, approval
4. **HUD**: toasts e which-key panel (sempre no topo)

---

## Keybindings

### Globais (funcionam em qualquer contexto)

| Tecla | Ação |
|-------|------|
| `Ctrl+C` | Sair do Kira |
| `Ctrl+P` | Abrir Command Palette |
| `Ctrl+M` | Toggle modo Plan/Yolo |
| `Ctrl+L` | Toggle Layers View |
| `Tab` | Toggle sidebar |
| `?` | Abrir overlay de Help |

### Chat Panel

| Tecla | Ação |
|-------|------|
| `j` / `↓` | Scroll para baixo |
| `k` / `↑` | Scroll para cima |
| `g g` | Ir para o topo |
| `G` (Shift+G) | Ir para o final |
| `PageUp` | Página acima |
| `PageDown` | Página abaixo |

### Input Area

| Tecla | Ação |
|-------|------|
| `Enter` | Enviar mensagem ou comando |
| `Shift+Enter` | Nova linha |
| `↑` / `↓` | Navegar histórico |
| `Backspace` | Apagar caractere |

### Layers View (quando visível)

| Tecla | Ação |
|-------|------|
| `1` | Drill-down: Perception |
| `2` | Drill-down: Reasoning |
| `3` | Drill-down: Security |
| `4` | Drill-down: Execution |
| `Escape` | Fechar detail panel / dismiss layers |

> Nota: teclas 1-4 só ativam drill-down quando o input está vazio. Se estiver digitando, os números vão para o input normalmente.

### Overlays

| Tecla | Ação |
|-------|------|
| `Escape` | Fechar overlay |
| `↑` / `↓` | Navegar lista |
| `Enter` | Selecionar item |
| Texto | Filtrar (command palette) |

### Approval Modal (HITL)

| Tecla | Ação |
|-------|------|
| `y` | Aprovar tool call |
| `n` | Rejeitar tool call |
| `f` | Entrar em modo feedback (texto + Enter para rejeitar com feedback) |
| `Escape` | Cancelar modo feedback |

---

## Slash Commands

| Comando | Tipo | Descrição |
|---------|------|-----------|
| `/help` | Local | Listar todos os comandos |
| `/quit` | Local | Sair do Kira |
| `/mode [yolo\|plan]` | Local | Alterar modo de execução |
| `/tier [heavy\|standard\|light]` | Local | Alterar tier de LLM |
| `/layers` | Local | Toggle Layers View |
| `/agents` | Local | Listar agentes ativos |
| `/queue` | Remoto | Listar fila de mensagens |
| `/queue cancel <id>` | Remoto | Cancelar item da fila |
| `/memory search <query>` | Remoto | Buscar fragmentos de memória |
| `/memory <run_id>` | Remoto | Ver contexto de memória de uma execução |
| `/cancel <prefix>` | Remoto | Cancelar agente por prefixo de ID |
| `/lessons` | Remoto | Listar lessons aprendidas |
| `/waves` | Local | Ver plano de waves |
| `/workflow <name>` | Local | Iniciar workflow |
| `/workflows` | Local | Listar workflows |
| `/worktrees` | Local | Listar worktrees |
| `/plan` / `/plans` | Local | Ver planos |
| `/approve` | Local | Aprovar plano pendente |

Comandos **locais** executam imediatamente. Comandos **remotos** fazem chamadas HTTP ao Core e retornam resultados via canal assíncrono.

---

## Overlays

### Command Palette (`Ctrl+P`)

Busca fuzzy por comandos e ações. Suporta:
- Todos os slash commands
- Toggle Mode (Plan/Yolo)
- Toggle Sidebar
- Open Help
- Open Session Picker
- Open Task Browser
- Toggle Layers View

Scoring: exact match (100) > starts with (80) > contains (50). Máximo 15 resultados.

### Session Picker

Navegar e trocar entre sessões ativas.

### Task Browser

Visualizar tarefas do pipeline SDD organizadas por waves.

### Help (`?`)

Referência completa de keybindings organizada por contexto (Global, Chat Panel, Input Area, Sidebar, Overlays, Layers View).

---

## Sidebar

A sidebar (42 colunas) exibe informações em tempo real:

| Seção | Conteúdo |
|-------|----------|
| **Session** | Título da sessão ativa |
| **Workspace** | Caminho do workspace + branch git |
| **Budget** | Tokens usados, % uso, custo USD, limite |
| **Providers** | Status de conexão (●verde/●vermelho/●amarelo) |
| **SDD Pipeline** | Fase atual: SPECIFY → DESIGN → TASKS → EXECUTE |
| **Waves** | Progresso por wave (✓ complete, ◑ in progress, ○ pending) |
| **Agents** | Até 6 agentes ativos (ID, tipo, status, custo) |
| **Files** | Até 8 arquivos modificados (+insertions/-deletions) |
| **Version** | Versão do kira-cli |
| **Layers** | Pipeline visualization (quando placement = SidebarSection) |

A sidebar aparece automaticamente em terminais largos (>120 cols) ou via `Tab`.

---

## Layers View

Visualização em tempo real do pipeline de processamento, mapeando os 7 pilares internos em 4 camadas conceituais:

```
┌──────────────────────────────┐
│ ⠋ Perception              0s │  ← P7 Cognition (memory/rag/context)
└──────────────────────────────┘
              ▼ 3
┌──────────────────────────────┐
│ · Reasoning               0s │  ← P1 SDD, P2 Coordination, P3 Verification
└──────────────────────────────┘
              ▼ 1
┌──────────────────────────────┐
│ · Security                0s │  ← P4 Gates, P5 Guardrails
└──────────────────────────────┘
              ▼ 0
┌──────────────────────────────┐
│ · Execution               0s │  ← P2+spawn/wave, P6+tool/execute
└──────────────────────────────┘
```

### Indicadores de Status

| Status | Indicador | Cor |
|--------|-----------|-----|
| Active | Spinner braille (⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏) | Verde bold |
| Idle | Ponto (·) | Dim |
| Blocked | Pausa (⏸) | Amarelo |
| Errored | Cruz (✗) | Vermelho bold |

### Cores por Camada

| Camada | Cor |
|--------|-----|
| Perception | Cyan |
| Reasoning | Magenta |
| Security | Amarelo |
| Execution | Verde |

### Flow Arrows

- **Normal**: ▼ verde com contagem de throughput
- **Stale** (>5s sem flow): ▼ dim/cinza
- **Bottleneck** (>10s zero throughput com layer ativa): ▼ fundo amarelo

### Bottleneck Detection

Quando uma camada fica ativa por mais tempo que o threshold configurado, a borda fica amarela e a duração é destacada em bold amarelo.

### Detail Panel (drill-down)

Pressione 1-4 para expandir o painel de detalhes de uma camada:

- **Perception**: Memory fragments count, files read, RAG query
- **Reasoning**: SDD phase, intent, active model
- **Security**: Guardrail reason, risk level, HITL status
- **Execution**: Tool name, agent ID, elapsed time

Mostra os últimos 20 eventos (timestamp, phase, message).

### Modos de Renderização

| Modo | Condição | Layout |
|------|----------|--------|
| Full | width≥30, height≥12 | 4 boxes + arrows + detail |
| Vertical Compact | width<30, height≥12 | Coluna P/R/S/E com ícones |
| Horizontal Compact | height<12 | Linha P→R→S→E com ícones |

### Placement

| Condição | Posição |
|----------|---------|
| Sidebar visível + terminal >160 cols | Seção na sidebar (max 12 rows) |
| Caso contrário | Floating panel (36 cols, 60% height, top-right) |

---

## HITL — Aprovação de Ferramentas

Quando um agente tenta executar uma ferramenta classificada como HIGH ou CRITICAL risk, um modal de aprovação aparece:

```
┌─ Approval Required ─────────────────────┐
│                                          │
│  Tool: bash                              │
│  Risk: ██ CRITICAL                       │
│  Agent: abcdef12                         │
│                                          │
│  Args: rm -rf /tmp/workspace/...         │
│                                          │
│  ⏱ 108s remaining                        │
│                                          │
│  [y] approve  [n] reject  [f] feedback   │
└──────────────────────────────────────────┘
```

- Borda **vermelha** para CRITICAL, **amarela** para HIGH
- Countdown timer com auto-reject ao expirar (padrão: 120s)
- Fila de aprovações: se múltiplas chegam, são enfileiradas (FIFO)
- Modo feedback: `f` → digitar texto → `Enter` para rejeitar com feedback

---

## Memória Semântica

O sistema de memória cross-thread permite que agentes compartilhem conhecimento entre sessões. Fragmentos são classificados em **tiers de maturação** que compactam conhecimento progressivamente:

| Tier | Descrição | Criação |
|------|-----------|---------|
| **Working** | Fragmentos brutos extraídos de cada execução | Automática (extractor) |
| **Episodic** | Resumos de sessão consolidados | LLM ao fim da sessão |
| **Semantic** | Conhecimento cross-sessão (clusters similares) | LLM periódico (6h) |
| **Procedural** | Regras e padrões estáveis (imune a decay) | Promoção por acesso |

### Comandos

```bash
# Buscar fragmentos por similaridade semântica (ranking ajustado por decay)
/memory search "como configurar o router"

# Ver contexto de memória de uma execução específica
/memory abc123-run-id
```

### Decay (Decaimento Exponencial)

Fragmentos não acessados perdem prioridade no ranking de retrieval:

```
effective_score = base_score × decay_factor^(dias_desde_último_acesso)
```

- **Padrão**: `decay_factor = 0.95` (perde ~5% por dia sem acesso)
- **Floor**: score nunca cai abaixo de `0.01`
- **Procedural**: fragmentos procedurais são **isentos** de decay
- **Reforço**: acessar um fragmento reseta o timer de decay

### Consolidação

Fragmentos progridem automaticamente pelos tiers:

1. **Working → Episodic**: ao fim de cada sessão, se ≥3 fragmentos working, o LLM gera um resumo episódico
2. **Episodic → Semantic**: a cada 6h, clusters de ≥5 episódicos similares (coseno ≥0.75) são consolidados
3. **Semantic → Procedural**: fragmentos semânticos com ≥10 acessos e ≥7 dias de idade são promovidos (sem LLM)

Fragmentos working consolidados são retidos por 24h e depois removidos.

### Session Handoff

Continuidade automática entre sessões:

1. **Geração**: ao fim da sessão, o LLM gera um resumo estruturado (contexto, perguntas abertas, próximos passos, decisões)
2. **Injeção**: ao iniciar nova sessão, o handoff anterior é injetado automaticamente no contexto
3. **Debounce**: reconexões SSE dentro de 30s não re-injetam nem regeneram handoff

### API de Maturation

```bash
# Status da consolidação (state, contagem por tier, decay_factor)
curl http://127.0.0.1:7777/v1/memory/maturation/status

# Trigger consolidação imediata (202 Accepted / 409 Conflict)
curl -X POST http://127.0.0.1:7777/v1/memory/maturation/consolidate

# Ver handoff pendente (200 / 404)
curl http://127.0.0.1:7777/v1/memory/handoff

# Limpar handoff (204 No Content)
curl -X DELETE http://127.0.0.1:7777/v1/memory/handoff

# Metadata de um fragmento (tier, access_count, effective_score)
curl http://127.0.0.1:7777/v1/memory/{fragment_id}/metadata
```

### Configuração

```toml
[memory.maturation]
enabled = true                        # false = desliga consolidação/handoff, decay continua
decay_factor = 0.95                   # fator por dia (0.5–0.999)
decay_floor = 0.01                    # score mínimo (0.001–0.1)
min_fragments_for_consolidation = 3   # mínimo para consolidar sessão (2–50)
semantic_similarity_threshold = 0.75  # limiar de clustering (0.5–1.0)
min_episodic_for_semantic = 5         # mínimo no cluster (3–20)
semantic_check_interval_hours = 6     # intervalo do timer (1–168)
procedural_access_threshold = 10      # acessos para promoção (5–100)
procedural_min_age_days = 7           # idade mínima para promoção (1–90)
max_handoff_chars = 2000              # tamanho máximo do handoff (500–10000)
max_promotions_per_cycle = 50         # máximo promoções por ciclo (1–500)
```

**Hot-reload**: `kill -HUP <pid>` recarrega a configuração sem reiniciar. Operações em andamento terminam com a config antiga.

### Prompts Editáveis

Os prompts de consolidação e handoff são arquivos em disco, editáveis pelo operador:

- `core/agents/prompts/memory_consolidator.md` — como o LLM deve consolidar fragmentos
- `core/agents/prompts/session_handoff.md` — como o LLM deve gerar o resumo de sessão

Edições tomam efeito imediatamente (sem restart).

### Degradação Graciosa

- LLM indisponível → consolidação/handoff são pulados, decay continua funcionando
- Falha em qualquer operação de maturation **nunca** bloqueia o request path
- Fragmentos permanecem inalterados em caso de falha (atomicidade)
- Circuit breaker respeitado — sem bypass de mecanismos de resiliência

---

## Fila de Mensagens

Buffer de resiliência para mensagens durante desconexão:

```bash
# Listar fila
/queue

# Cancelar item
/queue cancel <id>
```

- Máximo 64 mensagens enfileiradas localmente durante desconexão
- Flush automático ao reconectar (FIFO, com retry+backoff)
- Indicador de profundidade da fila via evento `QueueDepthChanged`

---

## Pipeline SDD

Spec-Driven Execution em 4 fases:

```
SPECIFY → DESIGN → TASKS → EXECUTE
```

| Fase | Descrição |
|------|-----------|
| **SPECIFY** | Definir requisitos e especificação |
| **DESIGN** | Projetar a solução técnica |
| **TASKS** | Decompor em tarefas executáveis |
| **EXECUTE** | Executar tarefas via agentes |

O progresso é visível na sidebar (SDD Pipeline section) e no Layers View (Reasoning layer).

---

## Waves — Execução Paralela

Waves são grupos de tarefas que executam em paralelo:

```
Wave 1: [task-a, task-b, task-c]  ← paralelo
         ↓ barreira de sincronização
Wave 2: [task-d, task-e]          ← paralelo
         ↓ barreira de sincronização
Wave 3: [task-f]                  ← sequencial
```

- Tarefas sem dependências não-resolvidas ficam na mesma wave
- Wave N+1 só contém tarefas cujas dependências estão em waves ≤ N
- Dentro de uma wave: um agente por tarefa (star topology)
- Políticas de falha: `continue`, `abort`, `retry`

Visualização: `/waves` ou sidebar (Waves section).

---

## Agentes

Sub-agentes executam tarefas isoladamente:

| Tipo | Tier | Acesso FS | Limites |
|------|------|-----------|---------|
| `writer` | Standard | `src/`, `public/` | 50 iters, $1, 1h |
| `deployer` | Light | `scripts/`, `dist/` | 50 iters, $1, 1h |
| `judge` | Judge (≠ writer) | read-only | — |

```bash
# Listar agentes ativos
/agents

# Cancelar agente por prefixo
/cancel abc123

# Spawn via CLI
kira agent spawn --kind writer --task "implementar feature X"
```

Coordenação via star topology (Core orquestra). Comunicação via FS mailbox em `.kira/mailbox/<agent_id>/`.

---

## Lessons

Regras compactas geradas a partir de falhas:

```bash
# Listar todas as lessons
/lessons

# Via CLI
kira lessons list --format table
kira lessons list --format json
```

Lessons são injetadas automaticamente em retries (`AgentMistake` → retry com lesson injection).

---

## Conexão e Resiliência

### Health Checker

- Verifica `GET /v1/health` a cada 30s
- Emite toast apenas em transições (connected↔disconnected)
- Timeout de health check: 5s

### SSE Listener

- Conecta em `GET /v1/events/stream` após primeiro `CoreConnected`
- Reconexão com backoff exponencial (1s → 60s)
- Detecção de conexão stale (90s sem eventos → reconecta)

### Resiliência de Mensagens

- Mensagens enfileiradas localmente durante desconexão (max 64)
- Flush automático ao reconectar
- Input lock previne submissões duplicadas
- Texto restaurado no input em caso de falha

### Stall Detection (3 sinais)

1. Mesma tool chamada 3x consecutivas com mesmos args
2. Hash de arquivo inalterado após iteração
3. 90 segundos sem progresso

### Failure Classification

| Tipo | Ação |
|------|------|
| `AgentMistake` | Retry com lesson injection |
| `InfraTransient` | Retry com fallback chain |
| `Stall` | Abort + notificação |

---

## CLI Não-Interativo

```bash
# Enviar mensagem e receber resposta
kira chat "explique este código"

# Spawn de agente
kira agent spawn --kind writer --task "refatorar módulo X"

# Status de agente
kira agent status <agent-id>

# Listar lessons
kira lessons list --format table
```

### Argumentos Globais

| Argumento | Env Var | Padrão | Descrição |
|-----------|---------|--------|-----------|
| `--core-url` | `KIRA_CORE_URL` | `http://127.0.0.1:7777` | URL do daemon |
| `--tier` | — | `standard` | Tier de LLM |
| `--mode` | — | `yolo` | Modo de execução |

---

## Configuração

Toda configuração vive em `tokens.toml` (raiz do workspace):

### Providers

```toml
[providers.deepseek]
url = "https://api.deepseek.com/v1"
api_key_env = "DEEPSEEK_API_KEY"
enabled = true

[providers.ollama]
url = "http://127.0.0.1:11434/v1"
enabled = true

[providers.openrouter]
url = "https://openrouter.ai/api/v1"
api_key_env = "OPENROUTER_API_KEY"
enabled = false
```

### Tiers

```toml
[tiers.heavy]
candidates = ["deepseek:deepseek-chat"]

[tiers.standard]
candidates = ["deepseek:deepseek-chat", "ollama:qwen2.5:7b"]

[tiers.light]
candidates = ["ollama:qwen2.5:1.5b"]

[tiers.judge]
candidates = ["deepseek:deepseek-chat"]
```

### Waves

```toml
[waves]
max_parallel_agents = 3
failure_policy = "continue"  # continue | abort | retry
retry_max_attempts = 2
```

### UI

```toml
[ui]
toast_duration_secs = 4
theme = "dark"
scroll_lines_per_key = 3
sidebar_width = 42
sidebar_auto_threshold = 120
input_min_height = 3
input_max_height = 10
which_key_delay_ms = 500
max_output_lines = 10000
task_refresh_interval_secs = 2

[ui.layers]
visible_on_start = false
perception_threshold_secs = 10
reasoning_threshold_secs = 30
security_threshold_secs = 5
execution_threshold_secs = 60
```

### Timeouts

```toml
[time]
approval_timeout_secs = 120
health_timeout_secs = 5
request_timeout_secs = 30
health_interval_secs = 30
```

### Budget

```toml
[budget]
monthly_hard_stop_usd = 50.0
warn_pct = 80.0
```

---

## Eventos SSE

O Core emite eventos em tempo real via Server-Sent Events:

| Evento | Dados | Efeito na TUI |
|--------|-------|---------------|
| `BudgetWarn` | `{ pct: u8 }` | Toast warning + Layers (Security) |
| `BudgetStop` | — | Budget stop + output error |
| `GuardrailTriggered` | `{ level, reason, agent_id }` | Toast + Layers (Security) |
| `MemoryInjected` | `{ fragment_count, agent_id }` | Toast info + Layers (Perception) |
| `QueueDepthChanged` | `{ depth }` | Atualiza queue_depth + Layers (Reasoning) |
| `Heartbeat` | `{ agent_id }` | Mantém conexão viva |
| `AwaitingApproval` | `{ agent_id, tool_name, risk_level, args, timeout }` | Modal de aprovação |
| `HandoffInjected` | `{ content }` | Handoff injetado no início da sessão |
| `ConsolidationStarted` | `{ source_tier, target_tier, fragment_count }` | Layers (Perception) |
| `ConsolidationCompleted` | `{ source_tier, target_tier, fragment_id }` | Layers (Perception) |
| `ConsolidationFailed` | `{ source_tier, target_tier, error }` | Toast warning |
| `HandoffGenerated` | `{ session_id, summary_length_chars }` | Layers (Perception) |
| `HandoffFailed` | `{ session_id, error }` | Toast warning |
| `MemoryPromotedToProcedural` | `{ fragment_id, content_preview }` | Toast info |

Todos os eventos de maturation usam `Pillar::P7Cognition` e são persistidos no audit JSONL com prefixo `memory_maturation_`.

---

## Segurança

### Guardrails (3 Níveis)

1. **Input**: Detecção de injection, redação de PII, blocklist
2. **Tool**: Classificação de risco (LOW/HIGH/CRITICAL), HITL para HIGH+
3. **Output**: LLM-as-Judge (judge_model ≠ writer_model)

### Classificação de Risco

| Ferramenta | Risco |
|------------|-------|
| `file_read` | LOW |
| `bash` | HIGH |
| `bash` + `rm` | CRITICAL |

### Sandbox

| Fase | Isolamento |
|------|-----------|
| v0 (atual) | Git Worktree + Path Guard |
| v0.5 | Docker container por agente |
| v1.0 | MicroVM (Firecracker) por tarefa |

---

## Glossário Rápido

| Termo | Significado |
|-------|-------------|
| Core | Daemon Rust em `127.0.0.1:7777` |
| Harness | O sistema Kira como um todo |
| Worktree | Git worktree isolado por execução |
| Pilot | State machine que decide o próximo intent |
| Gate | Check determinístico no cascade de 6 níveis |
| Lesson | Regra compacta gerada de uma falha |
| HITL | Human-in-the-loop (aprovação manual) |
| Bundle Hash | SHA-256 de config+prompts+workflows |
| Memory Tier | Nível de maturação — working, episodic, semantic, procedural |
| Consolidator | Subsistema que consolida fragmentos em tiers superiores via LLM |
| Handoff | Resumo estruturado de sessão para continuidade |
| Decay | Decaimento exponencial no score de retrieval por inatividade |
| Effective Score | Score ajustado por decay: `base × factor^dias` |
