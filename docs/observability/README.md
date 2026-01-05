# Observability Configuration

## Logging

Environment variables:

- `SUBCOG_LOG_FORMAT` (`json` or `pretty`)
- `SUBCOG_LOG_LEVEL` (e.g., `info`, `debug`)
- `SUBCOG_LOG_FILTER` (advanced filters)
- `SUBCOG_LOG_FILE` (optional path)

## Tracing

Environment variables:

- `SUBCOG_TRACING_ENABLED`
- `SUBCOG_TRACE_SAMPLE_RATIO`
- `SUBCOG_TRACING_SAMPLER`
- `SUBCOG_OTLP_ENDPOINT`
- `SUBCOG_OTLP_PROTOCOL`
- `OTEL_SERVICE_NAME`
- `OTEL_RESOURCE_ATTRIBUTES`

## Metrics

Environment variables:

- `SUBCOG_METRICS_ENABLED`
- `SUBCOG_METRICS_PORT`
- `SUBCOG_METRICS_PUSH_GATEWAY_ENDPOINT`
- `SUBCOG_METRICS_PUSH_GATEWAY_USERNAME`
- `SUBCOG_METRICS_PUSH_GATEWAY_PASSWORD`
- `SUBCOG_METRICS_PUSH_GATEWAY_USE_POST`

## Troubleshooting

- Logging missing fields: confirm `SUBCOG_LOG_FORMAT=json` and `SUBCOG_LOG_LEVEL` are set.
- Tracing not exported: verify `SUBCOG_OTLP_ENDPOINT` and `SUBCOG_OTLP_PROTOCOL` values.
- Metrics empty: ensure `SUBCOG_METRICS_ENABLED=true` and check the metrics port.

## Deployment Checklist

- OTLP collector reachable from the service network.
- OTLP protocol matches collector (gRPC 4317 vs HTTP 4318).
- Log sink permissions and file paths validated.
- Metrics port exposed or push gateway configured.

## Quickstart

```bash
export SUBCOG_LOG_FORMAT=json
export SUBCOG_TRACING_ENABLED=true
export SUBCOG_OTLP_ENDPOINT=http://localhost:4318
export SUBCOG_METRICS_ENABLED=true
export SUBCOG_METRICS_PORT=9090

subcog status
```
