# MTP (Multi-Token Prediction) — Guia para Kira

> **MTP** prediz 2-4 tokens de uma vez usando o módulo nativo embutido no modelo.
> **2-4x mais rápido** que geração token-a-token, sem perder qualidade.
> Zero custo, zero API calls, 100% local.

---

## 1. O Que é MTP

| Técnica | Como funciona | Velocidade | Setup |
|---|---|---|---|
| **Token-a-token (padrão)** | Prediz 1 token por forward pass | 1x | Simples |
| **Speculative Decoding** | Draft model propõe, main model verifica | ~2x | Precisa de 2 modelos |
| **MTP (Multi-Token)** | Modelo prediz N tokens de uma vez (módulo nativo) | **2-4x** | 1 modelo, 1 flag |

**MTP ≠ Speculative Decoding**
- Speculative: draft model separado (ex: Llama-3.2-1B + Llama-3.1-70B)
- MTP: módulo treinado dentro do próprio modelo (ex: Qwen 2.5 tem MTP head nativo)

---

## 2. Modelos Compatíveis com MTP

Modelos que têm **MTP module nativo** no checkpoint:

| Modelo | MTP Size Máx | RAM (Mac M-series) | t/s sem MTP | t/s com MTP |
|---|---|---|---|---|
| **Qwen 2.5 7B** | 2 | ~5 GB | ~80 | ~200 |
| **Qwen 2.5 14B** | 2 | ~9 GB | ~45 | ~100 |
| **Qwen 2.5 32B** | 2 | ~18 GB | ~25 | ~70 |
| **Qwen 2.5 72B** | 2 | ~40 GB | ~12 | ~35 |
| **Llama 3.1 8B** | 2 | ~5 GB | ~100 | ~250 |
| **Llama 3.1 70B** | 2 | ~40 GB | ~15 | ~40 |
| **Qwen 3 4B** | 4 | ~3 GB | ~120 | ~400 |
| **Qwen 3 8B** | 4 | ~6 GB | ~90 | ~300 |

> **Nota**: MTP size = quantidade de tokens preditos de uma vez.
> Qwen 3 suporta até 4 tokens. Qwen 2.5 suporta até 2.
> Setar acima do suportado pelo modelo = fallback silencioso para MTP size 1.

---

## 3. Setup: Compilar llama.cpp com MTP

### 3.1 Clonar e compilar

```bash
cd ~/Workspace
git clone https://github.com/ggml-org/llama.cpp
cd llama.cpp
make -j$(sysctl -n hw.ncpu)
```

### 3.2 Verificar MTP support

```bash
./llama-cli --help | grep mtp
# Deve mostrar:
#   --mtp-mode MTP_MODE       Multi-token prediction mode. None (-1), Auto (0), Enable (1), Disable (2)
#   --mtp-size MTP_SIZE       Number of tokens to predict in MTP mode (default: 1)
```

### 3.3 Compilar com Metal (Mac)

```bash
GGML_METAL=1 make -j$(sysctl -n hw.ncpu)
```

---

## 4. Baixar Modelos GGUF com MTP

### 4.1 Via HuggingFace (recomendado)

```bash
# Qwen 2.5 7B Instruct — Q4_K_M (balanced)
cd ~/Workspace/llama.cpp/models
wget https://huggingface.co/Qwen/Qwen2.5-7B-Instruct-GGUF/resolve/main/qwen2.5-7b-instruct-q4_k_m.gguf

# Qwen 2.5 32B Instruct — Q4_K_M
wget https://huggingface.co/Qwen/Qwen2.5-32B-Instruct-GGUF/resolve/main/qwen2.5-32b-instruct-q4_k_m.gguf

# Qwen 3 8B (melhor MTP)
wget https://huggingface.co/Qwen/Qwen3-8B-GGUF/resolve/main/qwen3-8b-q4_k_m.gguf
```

### 4.2 Converter do formato original

Se o modelo não tiver GGUF pronto:

```bash
# Instalar dependências
pip install huggingface_hub gguf

# Baixar pesos
huggingface-cli download Qwen/Qwen2.5-7B-Instruct --local-dir ./Qwen2.5-7B-Instruct

# Converter
python convert-hf-to-gguf.py ./Qwen2.5-7B-Instruct --outfile qwen2.5-7b.gguf --outtype q4_k_m
```

### 4.3 Verificar se o modelo tem MTP

```bash
./llama-cli -m qwen2.5-7b.gguf --mtp-mode 0 --verbose-prompt -n 0 2>&1 | grep -i mtp
# Se tiver MTP module: "MTP module found, size: 2"
# Se não tiver: "No MTP module found in model"
```

---

## 5. Rodar Server com MTP

### 5.1 Comando base

```bash
cd ~/Workspace/llama.cpp

./llama-server \
  -m models/qwen2.5-7b-instruct-q4_k_m.gguf \
  --mtp-mode 1 \
  --mtp-size 2 \
  --port 8080 \
  --ctx-size 8192 \
  --threads $(sysctl -n hw.physicalcpu) \
  --threads-batch $(sysctl -n hw.physicalcpu)
```

### 5.2 Flags explicadas

| Flag | Valor | Descrição |
|---|---|---|
| `--mtp-mode` | `1` (enable) | `-1`=none, `0`=auto, `1`=enable, `2`=disable |
| `--mtp-size` | `2` | Tokens preditos de uma vez (max depende do modelo) |
| `--port` | `8080` | Porta HTTP (compatível com OpenAI API) |
| `--ctx-size` | `8192` | Context window (aumentar para 32768 se precisar) |
| `--threads` | auto | CPU threads (físicos, não lógicos) |
| `--gpu-layers` | `999` | Offload para GPU (Mac: Metal) |

### 5.3 Com GPU offload (Mac M-series)

```bash
./llama-server \
  -m models/qwen2.5-7b-instruct-q4_k_m.gguf \
  --mtp-mode 1 \
  --mtp-size 2 \
  --port 8080 \
  --ctx-size 8192 \
  --gpu-layers 999 \
  --threads $(sysctl -n hw.physicalcpu)
```

> **Nota**: No Mac, Metal é ativado automaticamente se compilado com `GGML_METAL=1`.

### 5.4 Verificar que está rodando

```bash
curl http://127.0.0.1:8080/v1/models
# Retorna: {"object":"list","data":[{"id":"qwen2.5-7b-instruct-q4_k_m","object":"model"}]}
```

---

## 6. Benchmark: Medir Performance

### 6.1 Benchmark simples

```bash
./llama-bench \
  -m models/qwen2.5-7b-instruct-q4_k_m.gguf \
  -p 512 \
  -n 128 \
  --mtp-mode 1 \
  --mtp-size 2
```

### 6.2 Comparar com/sem MTP

```bash
# Sem MTP
./llama-bench -m models/qwen2.5-7b-instruct-q4_k_m.gguf -p 512 -n 128 --mtp-mode 2
# Resultado esperado: ~80 t/s

# Com MTP
./llama-bench -m models/qwen2.5-7b-instruct-q4_k_m.gguf -p 512 -n 128 --mtp-mode 1 --mtp-size 2
# Resultado esperado: ~200 t/s
```

### 6.3 Benchmark completo (batch de configurações)

```bash
# Script para testar todas as combinações
for mtp_mode in 2 1; do
  for mtp_size in 1 2; do
    echo "=== MTP mode=$mtp_mode size=$mtp_size ==="
    ./llama-bench -m models/qwen2.5-7b-instruct-q4_k_m.gguf \
      -p 512 -n 128 \
      --mtp-mode $mtp_mode \
      --mtp-size $mtp_size \
      2>&1 | grep -E "t/s|pp|tg"
  done
done
```

---

## 7. Integrar com Kira

### 7.1 Configurar provider no tokens.toml

```toml
# tokens.toml — adicionar após [providers.ollama_local]

[providers.llama_cpp_local]
base_url = "http://127.0.0.1:8080/v1"
enabled  = true
priority = 3
```

### 7.2 Adicionar modelo ao tier Light

```toml
# tokens.toml

[models.light]
primary    = "deepseek-v4-flash"
fallback_1 = "qwen-flash"
fallback_2 = "qwen2.5-7b-instruct-q4_k_m"  # ← llama.cpp local
```

### 7.3 Rodar o fluxo completo

```bash
# Terminal 1: Core daemon
cd /Users/avanade/Workspace/kira/core
cargo run

# Terminal 2: llama.cpp server com MTP
cd ~/Workspace/llama.cpp
./llama-server -m models/qwen2.5-7b-instruct-q4_k_m.gguf \
  --mtp-mode 1 --mtp-size 2 --port 8080

# Terminal 3: REPL
cd /Users/avanade/Workspace/kira/agent
bun run src/cli/index.tsx

# No REPL:
/tier l
explore como funciona o router
```

### 7.4 Verificar que está usando MTP

Nos logs do llama.cpp server, procure por:
```
llm_load_tensors: using MTP module, size: 2
```

Se não aparecer, o modelo não tem MTP module ou a flag não foi passada corretamente.

---

## 8. Troubleshooting

### 8.1 "No MTP module found"

**Problema**: O modelo GGUF não foi convertido com MTP support.

**Solução**:
1. Verificar se o modelo original tem MTP head (Qwen 2.5/3 sim, Llama 3.1+ sim)
2. Reconverter com a versão mais recente do `convert-hf-to-gguf.py`:
   ```bash
   cd ~/Workspace/llama.cpp
   git pull  # precisa da versão mais recente
   python convert-hf-to-gguf.py ./Qwen2.5-7B-Instruct --outfile qwen2.5-7b.gguf --outtype q4_k_m
   ```

### 8.2 MTP size ignorado (fallback para 1)

**Problema**: O modelo suporta MTP size 2 mas você setou 4.

**Solução**: Usar o max suportado pelo modelo:
- Qwen 2.5: max 2
- Qwen 3: max 4
- Llama 3.1: max 2

### 8.3 Performance pior com MTP

**Problema**: t/s menor com MTP enabled.

**Causas possíveis**:
1. **Context window muito grande** — MTP overhead cresce com ctx-size. Reduzir para 8192.
2. **GPU offload incompleto** — verificar se `--gpu-layers` está setado.
3. **CPU bottleneck** — verificar se `--threads` usa CPUs físicos (não lógicos).
4. **Modelo quantizado demais** — Q2_K perde qualidade no MTP head. Usar Q4_K_M ou Q5_K_M.

### 8.4 "Metal backend not available"

**Problema**: No Mac, GPU não está sendo usada.

**Solução**:
```bash
cd ~/Workspace/llama.cpp
GGML_METAL=1 make clean && make -j$(sysctl -n hw.ncpu)
```

### 8.5 Server não responde na porta 8080

**Solução**:
```bash
# Verificar se a porta está livre
lsof -i :8080

# Testar conectividade
curl -v http://127.0.0.1:8080/v1/models

# Ver logs do server (deve mostrar "starting server" e "listening on 0.0.0.0:8080")
```

---

## 9. Integração com Explorer Agent (Futuro)

Quando o Explorer agent for implementado no Kira, o fluxo ideal será:

```
Explorer agent (Light tier)
    ↓
Roteador: fallback_2 = "qwen2.5-7b-instruct-q4_k_m" (llama.cpp local)
    ↓
llama.cpp server com MTP (--mtp-mode 1 --mtp-size 2)
    ↓
Tools: glob, grep, file_read, tavily_search
    ↓
Resultado: exploração rápida, zero custo, ~200 t/s
```

Isso dá ao Explorer:
- **Velocidade**: 2-4x mais rápido que API cloud
- **Custo**: zero (local)
- **Latência**: <10ms (local vs ~200ms API)
- **Privacidade**: código nunca sai da máquina
- **Independência de provider**: roda local quando faz sentido; tier `local` (Qwen via Ollama) fica como primary do `light` sem trancar o operador num único vendor.

---

## 10. Quick Reference

```bash
# Compilar
cd ~/Workspace/llama.cpp && GGML_METAL=1 make -j$(sysctl -n hw.ncpu)

# Baixar modelo
wget https://huggingface.co/Qwen/Qwen2.5-7B-Instruct-GGUF/resolve/main/qwen2.5-7b-instruct-q4_k_m.gguf

# Rodar server com MTP
./llama-server -m qwen2.5-7b-instruct-q4_k_m.gguf --mtp-mode 1 --mtp-size 2 --port 8080

# Benchmark
./llama-bench -m qwen2.5-7b-instruct-q4_k_m.gguf -p 512 -n 128 --mtp-mode 1 --mtp-size 2

# Verificar MTP
./llama-cli -m qwen2.5-7b-instruct-q4_k_m.gguf --mtp-mode 0 -n 0 2>&1 | grep -i mtp
```

---

## 11. Links Úteis

- [llama.cpp GitHub](https://github.com/ggml-org/llama.cpp)
- [llama.cpp server docs](https://github.com/ggml-org/llama.cpp/blob/master/examples/server/README.md)
- [Qwen 2.5 GGUF models](https://huggingface.co/collections/Qwen/qwen25-66e81a666513e518adb90d9e)
- [Qwen 3 GGUF models](https://huggingface.co/collections/Qwen/qwen3-67c30a3535e54a3d0f4f3f17)
- [MTP no llama.cpp (PR)](https://github.com/ggml-org/llama.cpp/pulls?q=mtp)
