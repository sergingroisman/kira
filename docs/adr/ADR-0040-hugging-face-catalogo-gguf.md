# ADR-0040 — Hugging Face é CATÁLOGO de GGUF, não runtime de inferência

**Status**: proposto (13/Nov/2026) · **Responde**: pedido do dono "Rust com Hugging
Face faz sentido?" · **Registro**: linha no `docs/adr/README.md` · **Itens**: HF-1..HF-5 do
[`docs/plano-finalizacao-kira.md`](../plano-finalizacao-kira.md)

## Contexto

O dono perguntou se vale acoplar Hugging Face ao KIRA em Rust. O ecossistema existe:
[`candle`](https://github.com/huggingface/candle) (ML da HF em Rust), `hf-hub`,
`tokenizers` e `llama-cpp-2` rodam GGUF **in-process** — é tecnicamente possível. A
pergunta certa não é "dá para fazer?", é "onde isso mora na arquitetura?".

Três fatos fixam o contexto:

1. **Binário único e leve é decisão selada** (ADR-0030): o KIRA orquestra
   ferramentas, não embala runtimes. O `azure_devops_rust_api` já é a única
   dependência pesada, e ela é justificada por domínio (ADR-0022).
2. **O `LlmBackend` já é plugável** (`src/llm/`, Ollama/Copilot/Foundry —
   ADR-0019/0020): trocar de motor não é problema em aberto.
3. **O Ollama já resolve o acesso ao Hub**: `ollama pull hf.co/<repo>:<quant>`
   baixa GGUF do Hugging Face sem que o KIRA escreva uma linha de HTTP contra a
   HF. O que falta não é runtime — é **catálogo, consentimento e gate de
   admissão**.

Sem ADR, o risco concreto: um PR futuro "só para testar" traz `candle` no
`Cargo.toml`, o binário dobra de tamanho, o build passa a exigir toolchain de ML
e dois runtimes fazem o mesmo trabalho — exatamente o que o ADR-0030 proíbe.

## Decisão

**O Hugging Face entra no KIRA como CATÁLOGO de modelos GGUF consumido via
Ollama, nunca como runtime de inferência.** O KIRA orquestra o Ollama
(`ollama list`, `ollama pull`) e documenta/garante os freios ao redor do
download; quem executa o modelo é o Ollama, sempre.

O que isso significa por camada:

- **Catálogo e listagem (HF-2)**: `kira model list` parseia `ollama list`
  (resolvido no PATH, ADR-0016 — sem rede fora do localhost do Ollama);
  `kira model pull hf.co/<repo>:<quant>` é o único caminho para trazer GGUF do
  Hub. Nenhum código KIRA fala com a API do Hugging Face.
- **Consentimento antes de baixar (HF-2)**: `pull` sem `--yes` e sem TTY = erro
  acionável; em TTY, prompt interativo mostrando **tamanho em GB** e o que vai
  acontecer. Download de modelo é escrita no mundo do dev — vale o mesmo
  espírito do `--apply`: leitura é livre, escrita nunca é implícita.
- **Disco checado antes (HF-2)**: tamanho estimado em GB × espaço livre no
  destino; insuficiente ⇒ recusa fail-closed com o número, nunca download que
  estoura o disco no meio.
- **Instalador com freio (HF-4)**: `scripts/install.sh` / `install.ps1` nunca
  `curl | sh`; Ollama ausente ⇒ instrução manual e saída limpa; SHA256SUMS
  verificado antes de tocar o sistema (mesma conduta do `release.yml`);
  `--with-model <repo>` é opt-in explícito com consentimento + disco checado.
- **Gate de admissão de modelo (HF-5)**: nenhum modelo entra no catálogo de
  especialistas sem passar: proveniência conhecida + SHA-256 conferido +
  template de chat known-good + **Ollama ≥ 0.17.1** (suporte a GGUF do Hub) +
  probes de segurança rodando no GGUF quantizado (não no modelo de papel) +
  budget compartilhado (`llm_calls` conta chamada de qualquer modelo, P0).
  **Quantização: piso Q4_K_M — Q2_K proibido por política** (degradação de
  segurança/qualidade abaixo do aceitável para triage).
- **Guardrail de saída (HF-3)**: a camada cara (trust, redact, jail) já existe;
  o que entra é guardrail **determinístico** de saída no `digest` (puro Rust,
  zero dependência): front-matter obrigatório, teto de tamanho, scan de segredo
  no corpo via `redact`, recusa com nota — nada silencioso.

## Alternativas rejeitadas

- **`candle` in-process** — ML da HF em Rust puro, mas: dois runtimes para o
  mesmo trabalho (Ollama já executa GGUF), binário gigante, build pesado,
  atendimento de e2e próprio. Viola ADR-0030 (binário único e leve) sem
  responder problema nenhum que o Ollama não responda.
- **`llama-cpp-2` / FFI de llama.cpp** — mesmo custo do candle + FFI insegura
  na prática (C++ no build, sem `unsafe` por política da casa no runtime).
- **`hf-hub` + `tokenizers` direto** — resolve download/tokenização, mas
  **não executa** o modelo; completaria o trio de dependências de ML para
  então ainda precisar de runtime. Não é o número que falta, é o papel.
- **Baixar GGUF por conta do KIRA (HTTP próprio contra a HF)** — duplica o que
  `ollama pull` já faz, perde verificação/caching do Ollama e abre superfície
  nova de rede. O KIRA orquestra; não reexecuta.
- **Cloud da HF (Inference API) como backend** — dado de cliente fora da
  máquina viola o local-first (ADR-0030); o caminho de nuvem da casa é Ollama
  Cloud (fase 2 da stack, escolha explícita do dono) e Foundry BYOK (ADR-0020).

## Consequências

**Positivas**:
- Binário único preservado; zero dependência nova de ML; build continua
  `cargo build` sem toolchain especial.
- Acesso a milhares de modelos do Hub pelo caminho que o Ollama já mantém —
  proveniência, verificação e cache ficam com quem é bom nisso.
- Acompanha o mercado de GGUF quantizado sem custo de manutenção no KIRA.

**Negativas / limites**:
- O KIRA depende da qualidade do Ollama para pull/execução — sem Ollama
  instalado, o catálogo HF não funciona (o `install` falha fechado com
  instrução manual; `kira doctor` continua mostrando o estado).
- Modelos que o Ollama não consegue servir ficam fora do alcance — o catálogo
  é auditável, não exaustivo.
- O gate de admissão (HF-5) é trabalho por modelo: cada especialista novo
  passa por proveniência + SHA-256 + probes. É o preço da admissão
  consciente; sem ele, `model pull` arbitrário vira superfície de risco.

**Neutras**:
- Ollama Cloud (fase 2 da stack) usa o mesmo catálogo de tags; `model pull`
  continuará orquestrando um comando, o destino muda.
- O budget compartilhado já está garantido pelo P0: chamada de LLM de
  qualquer modelo entra no `llm_calls` (schema v9) — especialista novo não
  escapa do teto de 50/dia.
