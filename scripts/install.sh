#!/usr/bin/env bash
# KIRA — instalador com verificação de integridade (SHA-256).
#
# Por que este script existe em vez de `curl … | bash`:
# o pipe cego executa o que chegar, sem chance de conferir. Aqui o binário é
# baixado, o hash é comparado com o SHA256SUMS publicado e SÓ ENTÃO o arquivo
# roda. Se qualquer byte mudou no caminho (proxy, CDN, download truncado), a
# instalação para antes de tocar no sistema.
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

die() { printf 'erro: %s\n' "$*" >&2; exit 1; }
say() { printf '%s\n' "$*"; }
note() { printf '  %s\n' "$*"; }

have() { command -v "$1" >/dev/null 2>&1; }

# --- 1. Requisitos: falhar cedo e dizendo o que falta -------------------------
have curl || die "curl não encontrado — instale o curl e tente de novo"
have tar || die "tar não encontrado — instale o tar e tente de novo"

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
