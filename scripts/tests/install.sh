#!/usr/bin/env bash
# Testes de shell do instalador (HF-4). Rodam no CI em job `install-check`
# (ubuntu apenas) e local via `bash scripts/tests/install.sh`.
#
# Por que file:// em vez de servidor HTTP de verdade: os testes provam o
# CONTRATO do instalador (SHA256SUMS fail-closed, Ollama ausente = instrução
# + exit 0, `--with-model` com consentimento e disco medidos antes de
# qualquer spawn) — não provam a rede. `curl -fsSL` sobre file:// dá o mesmo
# caminho de download/verificação do script, hermético e sem porta.
#
# Isolamento: nada aqui toca fora de $WORK. O "ollama" dos testes é um shim
# que registra o argv em arquivo — presença do registro = spawn aconteceu,
# e é isso que os asserts de "recusa ANTES de qualquer spawn" vigiam.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
INSTALL="$ROOT/scripts/install.sh"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

passed=0
failed=0

ok() { printf 'PASS  %s\n' "$1"; passed=$((passed + 1)); }
bad() { printf 'FAIL  %s\n' "$1"; failed=$((failed + 1)); }

# Binário falso do KIRA: --version prova execução; doctor responde conforme
# KIRA_FIXTURE_DOCTOR_EXIT (default 0) — usado para provar aviso ≠ erro.
make_fixture() {
    local dir="$1" name
    mkdir -p "$dir/pkg"
    cat > "$dir/pkg/kira" <<'EOF'
#!/usr/bin/env bash
case "$1" in
    --version) echo "kira 0.0.0-fixture" ;;
    doctor)
        echo "aviso: pendencias de instalacao nova (fixture)"
        exit "${KIRA_FIXTURE_DOCTOR_EXIT:-0}"
        ;;
esac
EOF
    chmod +x "$dir/pkg/kira"
    # Dois alvos: o nome do artifact depende do uname do runner (x86_64 no
    # ubuntu-latest; aarch64 cobre runner ARM sem custo de manutenção).
    for name in kira-x86_64-unknown-linux-gnu.tar.gz kira-aarch64-unknown-linux-gnu.tar.gz; do
        tar -C "$dir/pkg" -czf "$dir/$name" kira
    done
    (cd "$dir" && sha256sum kira-*.tar.gz > SHA256SUMS)
}

# Shim do ollama: registra o argv (spawn aconteceu = registro existe) e sai 0.
make_ollama_shim() {
    local dir="$1"
    mkdir -p "$dir"
    cat > "$dir/ollama" <<EOF
#!/usr/bin/env bash
printf '%s\n' "\$*" >> "$1/argv.log"
EOF
    chmod +x "$dir/ollama"
}

assert_log_absent() {
    [ ! -f "$WORK/ollama/argv.log" ]
}

# ---------------------------------------------------------------- T1: aceito
# Install sem Ollama: sai 0, binário instalado e instrução manual impressa —
# nunca instala o Ollama por conta própria.
name="install sem ollama sai 0 com instrução"
make_fixture "$WORK/fix"
make_ollama_shim "$WORK/ollama"
if KIRA_BASE_URL="file://$WORK/fix" KIRA_INSTALL_DIR="$WORK/bin1" \
    KIRA_MODEL_OLLAMA_BIN="$WORK/nao-existe/ollama" \
    bash "$INSTALL" >"$WORK/t1.out" 2>&1; then
    if [ -x "$WORK/bin1/kira" ] && grep -q "ollama.com" "$WORK/t1.out" \
        && grep -q "kira setup" "$WORK/t1.out"; then
        ok "$name"
    else
        bad "$name (binário ausente ou instrução faltando: $(cat "$WORK/t1.out"))"
    fi
else
    bad "$name (exit != 0: $(cat "$WORK/t1.out"))"
fi

# ------------------------------------------------------------- T2: disco nulo
# --with-model com disco INMedido (df falha no destino): recusa acionável
# ANTES de qualquer spawn — mesmo com --yes (disco é físico, não consentimento).
name="--with-model sem disco ⇒ recusa antes de qualquer spawn"
KIRA_BASE_URL="file://$WORK/fix" KIRA_INSTALL_DIR="$WORK/bin2" \
    KIRA_MODEL_OLLAMA_BIN="$WORK/ollama/ollama" OLLAMA_MODELS="$WORK/sem-disco" \
    KIRA_MIN_FREE_MB=1 \
    bash "$INSTALL" --with-model hf.co/org/model:Q4_K_M --yes \
    >"$WORK/t2.out" 2>&1 && rc=0 || rc=$?
if [ "$rc" -ne 0 ] && grep -q "NADA foi baixado" "$WORK/t2.out" \
    && assert_log_absent; then
    ok "$name"
else
    bad "$name (rc=$rc, spawn indevido ou nota faltando: $(cat "$WORK/t2.out"))"
fi

# ---------------------------------------------------------- T3: piso de disco
# Disco MEDIDO mas abaixo do piso (KIRA_MIN_FREE_MB enorme): recusa acionável
# ANTES de qualquer spawn — o aceite literal do HF-4 ("sem disco ⇒ recusa").
name="--with-model disco abaixo do piso ⇒ recusa antes de qualquer spawn"
mkdir -p "$WORK/models"
KIRA_BASE_URL="file://$WORK/fix" KIRA_INSTALL_DIR="$WORK/bin3" \
    KIRA_MODEL_OLLAMA_BIN="$WORK/ollama/ollama" OLLAMA_MODELS="$WORK/models" \
    KIRA_MIN_FREE_MB=999999999 \
    bash "$INSTALL" --with-model qwen3.5:4b --yes \
    >"$WORK/t3.out" 2>&1 && rc=0 || rc=$?
if [ "$rc" -ne 0 ] && grep -q "espaço livre insuficiente" "$WORK/t3.out" \
    && grep -q "KIRA_MIN_FREE_MB" "$WORK/t3.out" && assert_log_absent; then
    ok "$name"
else
    bad "$name (rc=$rc, spawn indevido ou nota faltando: $(cat "$WORK/t3.out"))"
fi

# ------------------------------------------------------------ T4: consentimento
# --with-model sem --yes e sem TTY (stdin de /dev/null): consentimento é
# estado — recusa acionável, nenhum spawn, NADA baixado.
name="--with-model sem consentimento ⇒ recusa, nenhum spawn"
KIRA_BASE_URL="file://$WORK/fix" KIRA_INSTALL_DIR="$WORK/bin4" \
    KIRA_MODEL_OLLAMA_BIN="$WORK/ollama/ollama" OLLAMA_MODELS="$WORK/models" \
    KIRA_MIN_FREE_MB=1 \
    bash "$INSTALL" --with-model qwen3.5:4b </dev/null \
    >"$WORK/t4.out" 2>&1 && rc=0 || rc=$?
if [ "$rc" -ne 0 ] && grep -q "confirme com --yes" "$WORK/t4.out" \
    && assert_log_absent; then
    ok "$name"
else
    bad "$name (rc=$rc, spawn indevido ou nota faltando: $(cat "$WORK/t4.out"))"
fi

# -------------------------------------------------------------- T5: caminho feliz
# --with-model + --yes + disco ok + piso 1 MB: pull spawna uma vez com o argv
# certo (registry do shim prova ordem resolve→disco→consentimento→pull).
name="--with-model --yes com disco ok ⇒ pull spawna com o repo certo"
KIRA_BASE_URL="file://$WORK/fix" KIRA_INSTALL_DIR="$WORK/bin5" \
    KIRA_MODEL_OLLAMA_BIN="$WORK/ollama/ollama" OLLAMA_MODELS="$WORK/models" \
    KIRA_MIN_FREE_MB=1 \
    bash "$INSTALL" --with-model qwen3.5:4b --yes \
    >"$WORK/t5.out" 2>&1 && rc=0 || rc=$?
if [ "$rc" -eq 0 ] && [ -f "$WORK/ollama/argv.log" ] \
    && [ "$(cat "$WORK/ollama/argv.log")" = "pull qwen3.5:4b" ]; then
    ok "$name"
else
    bad "$name (rc=$rc, argv: $(cat "$WORK/ollama/argv.log" 2>/dev/null || echo vazio))"
fi

# --------------------------------------------------------------- T6: aviso ≠ erro
# doctor que falha (estado normal pós-instalação: sem PAT/config) NÃO derruba
# o install — smoke é diagnóstico, nunca decisão de falha.
name="doctor falhando não derruba o install (aviso ≠ erro)"
if KIRA_BASE_URL="file://$WORK/fix" KIRA_INSTALL_DIR="$WORK/bin6" \
    KIRA_MODEL_OLLAMA_BIN="$WORK/nao-existe/ollama" KIRA_FIXTURE_DOCTOR_EXIT=7 \
    bash "$INSTALL" >"$WORK/t6.out" 2>&1; then
    ok "$name"
else
    bad "$name (install saiu != 0 por causa do doctor: $(cat "$WORK/t6.out"))"
fi

# ------------------------------------------------------------------ T7: argumento
# Argumento desconhecido = erro acionável com uso, não comportamento surpresa.
name="argumento desconhecido ⇒ erro acionável com uso"
KIRA_BASE_URL="file://$WORK/fix" bash "$INSTALL" --totem \
    >"$WORK/t7.out" 2>&1 && rc=0 || rc=$?
if [ "$rc" -ne 0 ] && grep -q "argumento desconhecido" "$WORK/t7.out" \
    && grep -q "uso: install.sh" "$WORK/t7.out"; then
    ok "$name"
else
    bad "$name (rc=$rc: $(cat "$WORK/t7.out"))"
fi

printf '\n%d passou, %d falhou\n' "$passed" "$failed"
[ "$failed" -eq 0 ]
