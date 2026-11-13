#!/bin/sh
# Um batimento do metabolismo: ingest (sempre) → digest (só com aceite E Ollama
# vivo) → publish. NUNCA escreve no mundo: nenhum --apply aqui — o container é
# o agendador, não o dono (ADR-0017: escrita exige approve humano).
# Falha vira linha de log com exit code, nunca silêncio: o cron continua.
set -u
PROJECT="${KIRA_PROJECT:-/repo}"
OLLAMA_URL="${KIRA_OLLAMA_URL:-http://localhost:11434}"
echo "== tick $(date -u +%FT%TZ) projeto=$PROJECT =="

kira ingest --path "$PROJECT" || echo "aviso: ingest falhou (exit $?)"

if [ ! -f "$PROJECT/.kira/trust.json" ]; then
  echo "nota: repo não aceito (kira trust --path $PROJECT) — digest pulado, nada sai da máquina"
elif curl -sf -o /dev/null "$OLLAMA_URL/api/tags"; then
  kira digest --path "$PROJECT" --pause "${KIRA_POSTS_PER_TICK:-1}" \
    || echo "aviso: digest falhou (exit $?)"
  kira publish --path "$PROJECT" || echo "aviso: publish falhou (exit $?)"
else
  echo "nota: Ollama não responde em $OLLAMA_URL — digest pulado neste tick"
fi
echo "== fim do tick =="
