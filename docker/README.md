# Observability Stack (Docker)

This stack provides Grafana, Prometheus, Tempo, a Pushgateway, and an OTLP collector
for local Subcog telemetry.

## Run

```bash
docker compose -f docker/docker-compose.observability.yml up --build
```

## Configure Subcog

Set these values in `~/.config/subcog/config.toml` (or project config):

```toml
[observability.metrics]
enabled = true
# Pushgateway accepts metrics from CLI and hooks (no HTTP listener needed)
[observability.metrics.push_gateway]
endpoint = "http://localhost:9091/metrics/job/subcog"

[observability.tracing]
enabled = true
# OTLP gRPC endpoint from the otel-collector
[observability.tracing.otlp]
endpoint = "http://localhost:4317"
protocol = "grpc"
```

If you run `subcog serve` locally, Prometheus scrapes the host at
`host.docker.internal:9090` by default. Update `docker/prometheus/prometheus.yml`
if your host or port differs.

## Grafana

- URL: http://localhost:3000
- Login: admin / admin

Dashboards are auto-provisioned from `docker/dashboards`.
