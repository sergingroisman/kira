#!/usr/bin/env bash
# KIRA — instalador com verificação de integridade (SHA-256).
#
# Por que este script existe em vez de `curl … | bash`:
# o pipe cego executa o que chegar, sem chance de conferir. Aqui o binário é
# baixado, o hash é comparado com o SHA256SUMS publicado e SÓ ENTÃO o arquivo
# roda. Se qualquer byte mudou no caminho (proxy, CDN, download truncado), a
# instalação para antes de tocar no sistema.
#
#   NUNCA rode este script via `curl | sh`. Ele valida e orienta — o fluxo
#   seguro é baixar, conferir o SHA256SUMS e SÓ DEPOIS executar:
#     curl -fsSLO <base>/install.sh <base>/SHA256SUMS
#     sha256sum -c SHA256SUMS      # install.sh precisa estar listado
#     bash install.sh [--with-model <repo>] [--yes]
#
# Uso:
#   install.sh                      # só o binário do KIRA
#   install.sh --with-model <repo>  # + download do modelo (opt-in, nunca
#                                   #  silencioso — mesmo contrato do
#                                   #  `kira model pull`: consentimento
#                                   #  explícito `--yes` OU prompt em TTY,
#                                   #  destino + disco livre medidos ANTES
#                                   #  de qualquer download/spawn)
#   install.sh --yes                # atalho de consentimento (script/CI)
#
# Ollama NUNCA é instalado daqui: ausente ⇒ instrução manual e saída limpa
# (exit 0) — instalar dependência de IA silenciosamente é exatamente o que
# este script existe para não fazer. Piso de disco para `--with-model`:
# `KIRA_MIN_FREE_MB` (default 4096 MB — qwen3.5:4b Q4_K_M ≈ 3,3 GB + margem).
#
# Fronteira honesta do que isso prova: o SHA256SUMS vem do mesmo host do
# binário, então isso garante INTEGRIDADE DE TRANSPORTE, não autoria. Quem
# controla o host controla os dois arquivos. Para provar autoria, fixe o hash
# fora do canal (`KIRA_PINNED_SHA256`) ou use a assinatura da release.
#
# Sem sudo, sem admin: instala no diretório do usuário.
set -euo pipefail

REPO="sergingroisman/kira"
BASE_URL="${KIRA_BASE_URL:-https://github.com/${REPO}/releases/latest/download}"
INSTALL_DIR="${KIRA_INSTALL_DIR:-$HOME/.local/bin}"
PINNED_SHA256="${KIRA_PINNED_SHA256:-}"
MIN_FREE_MB="${KIRA_MIN_FREE_MB:-4096}"

die() { printf 'erro: %s\n' "$*" >&2; exit 1; }
say() { printf '%s\n' "$*"; }
note() { printf '  %s\n' "$*"; }

have() { command -v "$1" >/dev/null 2>&1; }

# --- 1. Requisitos: falhar cedo e dizendo o que falta -------------------------
have curl || die "curl não encontrado — instale o curl e tente de novo"
have tar || die "tar não encontrado — instale o tar e tente de novo"

# --- 1b. Argumentos: opt-in do modelo ----------------------------------------
# Sem `--with-model` NENHUM download de modelo acontece — o instalador só
# coloca o binário do KIRA no disco. O repo é o formato do Ollama
# (`hf.co/<org>/<repo>:Q4_K_M` ou tag local como `qwen3.5:4b`).
WITH_MODEL=""
ASSUME_YES=0
while [ $# -gt 0 ]; do
    case "$1" in
        --with-model)
            [ $# -ge 2 ] || die "--with-model exige o repo (ex.: hf.co/org/model:Q4_K_M)"
            WITH_MODEL="$2"
            shift 2
            ;;
        --yes) ASSUME_YES=1; shift ;;
        *) die "argumento desconhecido: $1 (uso: install.sh [--with-model <repo>] [--yes])" ;;
    esac
done

# --- 2. Plataforma ------------------------------------------------------------
uname_s="$(uname -s)"
uname_m="$(uname -m)"

case "$uname_s" in
    Linux) os="linux" ;;
    Darwin) os="darwin" ;;
    *) die "sistema não suportado: $uname_s (este script cobre Linux e macOS; no Windows use install.ps1)" ;;
esac

case "$os:$uname_m" in
    linux:x86_64) target="x86_64-unknown-linux-gnu" ;;
    linux:aarch64 | linux:arm64) target="aarch64-unknown-linux-gnu" ;;
    darwin:x86_64) target="x86_64-apple-darwin" ;;
    darwin:arm64 | darwin:aarch64) target="aarch64-apple-darwin" ;;
    *) die "arquitetura não suportada: $uname_m em $os" ;;
esac

artifact="kira-${target}.tar.gz"

# --- 3. Programa de hash (sha256sum não existe no macOS) ----------------------
if have sha256sum; then
    hash_of() { sha256sum "$1" | awk '{print $1}'; }
elif have shasum; then
    hash_of() { shasum -a 256 "$1" | awk '{print $1}'; }
else
    die "sem sha256sum nem shasum — não dá para verificar a integridade, e instalar sem verificar é o que este script existe para evitar"
fi

# --- 4. Download -------------------------------------------------------------
tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

say "KIRA — baixando $artifact"
curl -fsSL --retry 3 --retry-delay 2 -o "$tmpdir/$artifact" "$BASE_URL/$artifact" \
    || die "não consegui baixar $BASE_URL/$artifact (release inexistente? rede?)"
curl -fsSL --retry 3 --retry-delay 2 -o "$tmpdir/SHA256SUMS" "$BASE_URL/SHA256SUMS" \
    || die "não consegui baixar o SHA256SUMS — sem ele não há verificação"

# --- 5. Verificação ----------------------------------------------------------
expected="$(awk -v f="$artifact" '$2 == f || $2 == "*" f { print $1 }' "$tmpdir/SHA256SUMS")"
[ -n "$expected" ] || die "o SHA256SUMS publicado não lista $artifact — release incompleta, não instale"

if [ -n "$PINNED_SHA256" ]; then
    if [ "$PINNED_SHA256" != "$expected" ]; then
        die "o hash publicado ($expected) difere do que você fixou ($PINNED_SHA256) — pare e verifique a release"
    fi
    note "hash confere com o valor fixado fora do canal"
fi

actual="$(hash_of "$tmpdir/$artifact")"
if [ "$actual" != "$expected" ]; then
    die "VERIFICAÇÃO FALHOU — esperado $expected, obtido $actual. Nada foi instalado."
fi
note "sha256 ok: $actual"

# --- 6. Extração e instalação ------------------------------------------------
tar -xzf "$tmpdir/$artifact" -C "$tmpdir" || die "não consegui extrair o arquivo baixado"

binary="$(find "$tmpdir" -type f -name kira -perm -u+x | head -1)"
[ -n "$binary" ] || binary="$(find "$tmpdir" -type f -name kira | head -1)"
[ -n "$binary" ] || die "o pacote não contém o binário kira"

mkdir -p "$INSTALL_DIR" || die "não consegui criar $INSTALL_DIR"
install -m 0755 "$binary" "$INSTALL_DIR/kira" 2>/dev/null \
    || { cp "$binary" "$INSTALL_DIR/kira" && chmod 0755 "$INSTALL_DIR/kira"; } \
    || die "não consegui instalar em $INSTALL_DIR"

# O binário precisa rodar depois de instalado: se não roda, foi pacote errado.
"$INSTALL_DIR/kira" --version >/dev/null 2>&1 \
    || die "instalei $INSTALL_DIR/kira mas ele não executa — pacote incompatível com esta máquina"

say ""
say "KIRA instalado em $INSTALL_DIR/kira"
case ":$PATH:" in
    *":$INSTALL_DIR:"*) note "pronto para usar: kira setup" ;;
    *)
        note "atenção: $INSTALL_DIR não está no PATH. Adicione:"
        note "  export PATH=\"$INSTALL_DIR:\$PATH\""
        note "depois: kira setup"
        ;;
esac

# --- 7. Ollama: detectar e orientar, NUNCA instalar ---------------------------
# O Ollama é o backend de IA local (default eterno) e quem baixa/executa
# modelos (ADR-0040). Este script nunca o instala: ausente ⇒ instrução manual
# e saída limpa (exit 0) — instalar dependência de IA silenciosamente é
# exatamente o que este script existe para não fazer. A override
# `KIRA_MODEL_OLLAMA_BIN` espelha o contrato do `kira model pull` (HF-2).
ollama_bin=""
if [ -n "${KIRA_MODEL_OLLAMA_BIN:-}" ]; then
    # Override presente é contrato do usuário (mesma conduta do binresolve):
    # arquivo executável é usado como veio; inválido conta como ausente.
    if [ -x "$KIRA_MODEL_OLLAMA_BIN" ]; then
        ollama_bin="$KIRA_MODEL_OLLAMA_BIN"
    fi
elif have ollama; then
    ollama_bin="$(command -v ollama)"
fi

if [ -z "$ollama_bin" ]; then
    say ""
    say "Ollama não encontrado — o KIRA funciona, mas o backend de IA local não."
    note "instale manualmente: https://ollama.com (este script não instala nada silenciosamente)"
    note "depois rode: kira setup   (o kira doctor detecta o ollama)"
    if [ -n "$WITH_MODEL" ]; then
        note "--with-model $WITH_MODEL NÃO baixado: sem ollama não há pull — instale o Ollama e rode 'kira model pull $WITH_MODEL'"
    fi
else
    # --- 7b. --with-model: mesmo contrato do `kira model pull` (HF-2) ---------
    # Disco medido ANTES de qualquer consentimento/spawn; consentimento é
    # estado (`--yes` OU prompt em TTY); recusa em qualquer um = nada baixado.
    # Sem `--with-model`, NADA desta seção roda — nenhum download implícito.
    if [ -n "$WITH_MODEL" ]; then
        case "$MIN_FREE_MB" in
            '' | *[!0-9]*) die "KIRA_MIN_FREE_MB='$MIN_FREE_MB' não é um inteiro (MB) — fixe um valor numérico" ;;
        esac

        dest="${OLLAMA_MODELS:-$HOME/.ollama/models}"
        # df -k: coluna 4 (avail, blocos de 1K) da ÚLTIMA linha — números POSIX
        # sem separador de locale (mesma leitura do parse_df_k do model.rs).
        # df que falha ⇒ medição vazia, nunca zero fabricado.
        free_kb="$(df -k "$dest" 2>/dev/null | awk 'NF { last = $4 } END { print last }' || true)"
        case "$free_kb" in
            '' | *[!0-9]*) free_kb="" ;;
        esac

        # Disco é limite físico, não consentimento: sem medição ou abaixo do
        # piso, recusa ANTES de qualquer spawn — mesmo com --yes.
        if [ -z "$free_kb" ]; then
            die "não consegui medir o disco livre de $dest — NADA foi baixado; confira o destino (OLLAMA_MODELS?) e rode de novo"
        fi
        required_kb=$((MIN_FREE_MB * 1024))
        if [ "$free_kb" -lt "$required_kb" ]; then
            die "espaço livre insuficiente em $dest: $((free_kb / 1024)) MB livre, piso ${MIN_FREE_MB} MB (KIRA_MIN_FREE_MB) — NADA foi baixado; libere espaço ou rode 'ollama pull $WITH_MODEL' por sua conta"
        fi

        if [ "$ASSUME_YES" -ne 1 ]; then
            if [ ! -t 0 ]; then
                die "download de modelo é escrita no disco — confirme com --yes ou rode em um terminal interativo (a pergunta mostra destino e espaço livre)"
            fi
            printf 'baixar modelo do Hugging Face? %s\n' "$WITH_MODEL"
            printf '  destino: %s\n' "$dest"
            printf '  espaço livre: %s MB\n' "$((free_kb / 1024))"
            printf '  o Ollama mostra o tamanho durante o download\n'
            printf 'confirmar [y/N]: '
            # EOF/Enter vazio = recusa (mesmo contrato do connect: nunca baixa).
            read -r answer || answer=""
            case "$(printf '%s' "$answer" | tr '[:upper:]' '[:lower:]')" in
                y | yes | s | sim) ;; # consentido
                *) die "download cancelado — nada foi baixado" ;;
            esac
        fi

        if ! "$ollama_bin" pull "$WITH_MODEL"; then
            die "o 'ollama pull $WITH_MODEL' falhou — rode o pull direto para ver o erro completo e rode 'install.sh --with-model $WITH_MODEL' de novo"
        fi
        say "modelo baixado: $WITH_MODEL (destino $dest)"
    fi
fi

# --- 8. Smoke: `kira doctor` no fim — aviso ≠ erro -----------------------------
# Em instalação nova o doctor SEMPRE sinaliza pendências (PAT, config, ollama
# ausente): são estados esperados pós-instalação, não defeito do install. O
# binário já provou que executa (`--version` acima); o smoke é diagnóstico
# impresso para o dev, nunca decisão de falha do instalador.
say ""
say "smoke: kira doctor"
if "$INSTALL_DIR/kira" doctor; then
    note "doctor ok"
else
    note "doctor sinalizou pendências — esperado em instalação nova (PAT/config); o binário funciona e o install está completo"
fi

exit 0
