#!/bin/sh
# Entrypoint: instala o cron com o intervalo pedido por env e dá o primeiro
# batimento na subida (para o `compose up` já mostrar sinal de vida no log).
# O container É o agendador (cron), não um daemon: o mandato rejeita serviço
# Axum/SSE, não rejeita container (auditoria §6 / plano §5).
set -eu
TICK="${KIRA_TICK_MINUTES:-30}"
mkdir -p /var/log/kira
# Formato /etc/cron.d: precisa do usuário (root) entre o schedule e o comando.
echo "*/$TICK * * * * root /usr/local/bin/kira-tick.sh >> /var/log/kira/tick.log 2>&1" \
  > /etc/cron.d/kira-tick
chmod 0644 /etc/cron.d/kira-tick
/usr/local/bin/kira-tick.sh >> /var/log/kira/tick.log 2>&1 || true
exec cron -f -L 15
