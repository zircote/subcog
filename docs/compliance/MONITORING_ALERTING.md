# Monitoring and Alerting (COMP-H11)

## Purpose
Define monitoring and alerting requirements.

## Metrics Collection

### Application Metrics
| Metric | Type | Alert Threshold |
|--------|------|-----------------|
| `subcog_capture_total` | Counter | N/A |
| `subcog_recall_total` | Counter | N/A |
| `subcog_recall_latency_ms` | Histogram | p99 > 500ms |
| `subcog_error_total` | Counter | > 10/min |

### Infrastructure Metrics
- CPU, memory, disk usage
- Database connections
- Network I/O

## Observability Stack

### Tracing
```yaml
observability:
  tracing:
    enabled: true
    otlp:
      endpoint: "http://collector:4317"
```

### Metrics
```yaml
observability:
  metrics:
    enabled: true
    port: 9090
```

### Logging
```yaml
observability:
  logging:
    format: json
    level: info
```

## Alert Rules

| Alert | Condition | Severity | Action |
|-------|-----------|----------|--------|
| HighErrorRate | errors > 10/min | High | Page on-call |
| SlowSearch | p99 > 500ms | Medium | Investigate |
| DiskFull | usage > 90% | Critical | Expand storage |
| AuthFailures | > 100/min | High | Check for attack |

## Dashboards
- Service health overview
- Error breakdown by type
- Latency percentiles
- Resource utilization
