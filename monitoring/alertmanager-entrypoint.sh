#!/bin/sh
# Generate alertmanager config with optional generic webhook

cat > /etc/alertmanager/alertmanager.yml << EOF
global:
  resolve_timeout: 5m

route:
  group_by: ['alertname', 'severity']
  group_wait: 10s
  group_interval: 10s
  repeat_interval: 12h
  receiver: 'default'
  routes:
    - match:
        severity: critical
      receiver: 'default'
      group_wait: 0s
      repeat_interval: 4h
    - match:
        severity: warning
      receiver: 'default'
      group_wait: 30s
      repeat_interval: 12h

receivers:
  - name: 'default'
    webhook_configs:
      - url: 'http://telegram-bot:8080'
        send_resolved: true
EOF

# Add generic webhook if URL is configured
if [ -n "$GENERIC_WEBHOOK_URL" ]; then
  # Ensure URL has http:// prefix
  WEBHOOK_URL="$GENERIC_WEBHOOK_URL"
  case "$WEBHOOK_URL" in
    http://*|https://*) ;;
    *) WEBHOOK_URL="http://$WEBHOOK_URL" ;;
  esac
  # Append to the webhook_configs list
  cat >> /etc/alertmanager/alertmanager.yml << EOF
      - url: '${WEBHOOK_URL}'
        send_resolved: true
EOF
  echo "Generic webhook enabled: $WEBHOOK_URL"
fi

# Add inhibit rules
cat >> /etc/alertmanager/alertmanager.yml << EOF

inhibit_rules:
  - source_match:
      severity: 'critical'
    target_match:
      severity: 'warning'
    equal: ['alertname']
EOF

echo "Generated alertmanager config:"
cat /etc/alertmanager/alertmanager.yml
echo "---"

# Start alertmanager
exec /bin/alertmanager --config.file=/etc/alertmanager/alertmanager.yml --storage.path=/alertmanager "$@"
