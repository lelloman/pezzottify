#!/bin/sh
# Generate prometheus config with hostname label

HOSTNAME_LABEL="${ALERT_HOSTNAME:-unknown}"

# Read template and inject hostname into external_labels
sed "s/environment: 'production'/environment: 'production'\n    host: '$HOSTNAME_LABEL'/" \
    /etc/prometheus/prometheus.yml.template > /etc/prometheus/prometheus.yml

echo "Generated prometheus config with host=$HOSTNAME_LABEL:"
cat /etc/prometheus/prometheus.yml
echo "---"

# Start prometheus
exec /bin/prometheus \
    --config.file=/etc/prometheus/prometheus.yml \
    --storage.tsdb.path=/prometheus \
    --storage.tsdb.retention.time=30d \
    --web.enable-lifecycle \
    "$@"
