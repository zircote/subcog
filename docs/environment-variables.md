# Environment Variables Reference

This document lists all environment variables supported by Subcog, organized by category.

## Configuration Loading Order

1. **Default values** - Hard-coded sensible defaults
2. **Config file** - `~/.config/subcog/config.toml`
3. **Environment variables** - Override config file settings

Environment variables always take precedence over config file values.

## Core Configuration

These variables can also be set in the `[subcog]` section of `config.toml`.

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SUBCOG_REPO_PATH` | path | `.` | Path to the git repository |
| `SUBCOG_DATA_DIR` | path | `~/.local/share/subcog` | Path to the data directory |
| `SUBCOG_MAX_RESULTS` | integer | `10` | Maximum number of search results |
| `SUBCOG_DEFAULT_SEARCH_MODE` | string | `hybrid` | Default search mode: `hybrid`, `text`, or `vector` |

## LLM Provider Configuration

Configure the LLM provider for features like auto-capture analysis, enrichment, and consolidation.

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SUBCOG_LLM_PROVIDER` | string | `anthropic` | Provider: `anthropic`, `openai`, `ollama`, `lmstudio` |
| `SUBCOG_LLM_MODEL` | string | varies | Model name (provider-specific) |
| `SUBCOG_LLM_API_KEY` | string | - | API key (supports `${VAR}` expansion in config) |
| `SUBCOG_LLM_BASE_URL` | string | - | Base URL for self-hosted providers |
| `SUBCOG_LLM_TIMEOUT_MS` | integer | `30000` | Request timeout in milliseconds |
| `SUBCOG_LLM_CONNECT_TIMEOUT_MS` | integer | `5000` | Connection timeout in milliseconds |

### Resilience Settings

Configure retry and circuit breaker behavior for LLM calls.

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SUBCOG_LLM_MAX_RETRIES` | integer | `3` | Maximum retry attempts |
| `SUBCOG_LLM_RETRY_BACKOFF_MS` | integer | `1000` | Base backoff between retries |
| `SUBCOG_LLM_BREAKER_FAILURE_THRESHOLD` | integer | `5` | Failures before opening circuit |
| `SUBCOG_LLM_BREAKER_RESET_MS` | integer | `30000` | Time before half-open state |
| `SUBCOG_LLM_BREAKER_HALF_OPEN_MAX_CALLS` | integer | `3` | Trial calls in half-open |
| `SUBCOG_LLM_LATENCY_SLO_MS` | integer | `5000` | Latency budget (SLO) |
| `SUBCOG_LLM_ERROR_BUDGET_RATIO` | float | `0.01` | Error budget threshold (1%) |
| `SUBCOG_LLM_ERROR_BUDGET_WINDOW_SECS` | integer | `3600` | Error budget window (1 hour) |

## Search Intent Detection

Control automatic memory surfacing based on detected user intent.

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SUBCOG_SEARCH_INTENT_ENABLED` | boolean | `true` | Enable intent detection |
| `SUBCOG_SEARCH_INTENT_USE_LLM` | boolean | `true` | Use LLM for classification |
| `SUBCOG_SEARCH_INTENT_LLM_TIMEOUT_MS` | integer | `200` | LLM classification timeout |
| `SUBCOG_SEARCH_INTENT_MIN_CONFIDENCE` | float | `0.5` | Minimum confidence threshold (0.0-1.0) |

### Intent-Based Memory Injection

Based on detected intent confidence:

| Confidence | Memory Count | Behavior |
|------------|--------------|----------|
| ≥ 0.8 (high) | 15 memories | Full context injection |
| ≥ 0.5 (medium) | 10 memories | Standard injection |
| < 0.5 (low) | 5 memories | Minimal injection |

## Deduplication Service

Configure duplicate detection for the pre-compact hook auto-capture.

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SUBCOG_DEDUP_ENABLED` | boolean | `true` | Enable deduplication |
| `SUBCOG_DEDUP_THRESHOLD_DEFAULT` | float | `0.90` | Default similarity threshold |
| `SUBCOG_DEDUP_TIME_WINDOW_SECS` | integer | `300` | Recent capture window (5 min) |
| `SUBCOG_DEDUP_CACHE_CAPACITY` | integer | `1000` | LRU cache size |
| `SUBCOG_DEDUP_MIN_SEMANTIC_LENGTH` | integer | `50` | Min length for semantic check |

### Per-Namespace Similarity Thresholds

Higher thresholds require closer matches to be considered duplicates.

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SUBCOG_DEDUP_THRESHOLD_DECISIONS` | float | `0.92` | Decisions namespace threshold |
| `SUBCOG_DEDUP_THRESHOLD_PATTERNS` | float | `0.90` | Patterns namespace threshold |
| `SUBCOG_DEDUP_THRESHOLD_LEARNINGS` | float | `0.88` | Learnings namespace threshold |
| `SUBCOG_DEDUP_THRESHOLD_BLOCKERS` | float | `0.90` | Blockers namespace threshold |
| `SUBCOG_DEDUP_THRESHOLD_TECHDEBT` | float | `0.90` | Tech-debt namespace threshold |
| `SUBCOG_DEDUP_THRESHOLD_CONTEXT` | float | `0.90` | Context namespace threshold |

## Prompt Customization

Customize LLM prompts for different operations.

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SUBCOG_PROMPT_IDENTITY_ADDENDUM` | string | - | Additional identity context |
| `SUBCOG_PROMPT_ADDITIONAL_GUIDANCE` | string | - | Global additional guidance |

## Observability

### Logging

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SUBCOG_LOG_FORMAT` | string | `pretty` | Log format: `json` or `pretty` |
| `SUBCOG_LOG_LEVEL` | string | `info` | Log level: `trace`, `debug`, `info`, `warn`, `error` |
| `SUBCOG_LOG_FILTER` | string | - | Full filter override (e.g., `subcog=debug,hyper=info`) |
| `SUBCOG_LOG_FILE` | path | - | Log file path (logs to stderr if not set) |

### Tracing (OpenTelemetry)

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SUBCOG_TRACING_ENABLED` | boolean | `false` | Enable distributed tracing |
| `SUBCOG_TRACING_SAMPLE_RATIO` | float | `1.0` | Trace sampling ratio |
| `SUBCOG_TRACING_SERVICE_NAME` | string | `subcog` | Service name in traces |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | string | - | OTLP collector endpoint |
| `OTEL_EXPORTER_OTLP_PROTOCOL` | string | `grpc` | Protocol: `grpc` or `http` |

### Metrics (Prometheus)

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SUBCOG_METRICS_ENABLED` | boolean | `false` | Enable Prometheus metrics |
| `SUBCOG_METRICS_PORT` | integer | `9090` | Prometheus exporter port |

### Metrics Push Gateway

For short-lived processes (e.g., hooks), push metrics to a gateway.

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SUBCOG_METRICS_PUSH_ENDPOINT` | string | - | Push gateway endpoint |
| `SUBCOG_METRICS_PUSH_USERNAME` | string | - | Basic auth username |
| `SUBCOG_METRICS_PUSH_PASSWORD` | string | - | Basic auth password |
| `SUBCOG_METRICS_PUSH_HTTP_POST` | boolean | `false` | Use POST instead of PUT |

## Feature Flags

Enable or disable optional features via config file only (not environment variables).

```toml
[features]
secrets_filter = true    # Detect and filter secrets
pii_filter = false       # Detect and filter PII
multi_domain = false     # Multi-domain support
audit_log = false        # SOC2/GDPR audit logging
llm_features = true      # LLM-powered features
auto_capture = true      # Auto-capture during hooks
consolidation = false    # Memory consolidation
```

## Storage Backend Configuration

Configure storage backends via config file. SQLite is the default and recommended backend.

```toml
[storage]
# Persistence layer (SQLite is default)
persistence = "sqlite"  # sqlite, postgresql, filesystem

# Index layer
index = "sqlite"  # sqlite, postgresql, redis

# Vector layer
vector = "usearch"  # usearch, pgvector, redis

# Data directory
data_dir = "~/.local/share/subcog"
```

### PostgreSQL Configuration

For high-performance deployments:

```toml
[postgresql]
host = "localhost"
port = 5432
database = "subcog"
user = "subcog"
password = "${SUBCOG_PG_PASSWORD}"
ssl_mode = "prefer"
```

Or via connection URL:

```bash
export DATABASE_URL="postgres://subcog:pass@localhost:5432/subcog"
```

## API Key Configuration Examples

### Anthropic (Claude)

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
```

Or in config.toml:
```toml
[llm]
provider = "anthropic"
api_key = "${ANTHROPIC_API_KEY}"
model = "claude-sonnet-4-20250514"
```

### OpenAI

```bash
export OPENAI_API_KEY="sk-..."
```

Or in config.toml:
```toml
[llm]
provider = "openai"
api_key = "${OPENAI_API_KEY}"
model = "gpt-4o"
```

### Ollama (Local)

```toml
[llm]
provider = "ollama"
base_url = "http://localhost:11434"
model = "llama3.2"
```

### LM Studio (Local)

```toml
[llm]
provider = "lmstudio"
base_url = "http://localhost:1234"
model = "local-model"
```

## Complete Config File Example

```toml
# ~/.config/subcog/config.toml

repo_path = "."
data_dir = "~/.local/share/subcog"
max_results = 10
default_search_mode = "hybrid"

[storage]
persistence = "sqlite"
index = "sqlite"
vector = "usearch"
data_dir = "~/.local/share/subcog"

[features]
secrets_filter = true
pii_filter = false
multi_domain = false
audit_log = false
llm_features = true
auto_capture = true
consolidation = false

[llm]
provider = "anthropic"
api_key = "${ANTHROPIC_API_KEY}"
model = "claude-sonnet-4-20250514"
timeout_ms = 30000
max_retries = 3
breaker_failure_threshold = 5
breaker_reset_ms = 30000

[search_intent]
enabled = true
use_llm = true
llm_timeout_ms = 200
min_confidence = 0.5
base_count = 5
max_count = 15
max_tokens = 4000

[search_intent.weights.troubleshoot]
blockers = 2.0
learnings = 1.5
tech-debt = 1.2

[search_intent.weights.howto]
patterns = 2.0
learnings = 1.5

[observability.logging]
format = "json"
level = "info"

[observability.tracing]
enabled = false
sample_ratio = 0.1

[observability.metrics]
enabled = false
port = 9090

[prompt]
identity_addendum = "You are assisting with the Acme Corp codebase."
additional_guidance = "Always consider security implications."
```
