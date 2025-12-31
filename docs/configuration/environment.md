# Environment Variables

Complete reference of all environment variables supported by Subcog.

## Core Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBCOG_DOMAIN` | Domain scope (project, user, org) | `project` |
| `SUBCOG_LOG_LEVEL` | Log level (trace, debug, info, warn, error) | `info` |
| `SUBCOG_CONFIG_PATH` | Custom config file path | Auto-detected |
| `SUBCOG_GIT_DIR` | Git directory path | `.git` |
| `NO_COLOR` | Disable colored output | Unset |

## Storage Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBCOG_STORAGE_PERSISTENCE` | Persistence backend | `git_notes` |
| `SUBCOG_STORAGE_INDEX` | Index backend | `sqlite` |
| `SUBCOG_STORAGE_VECTOR` | Vector backend | `usearch` |
| `SUBCOG_STORAGE_SQLITE_PATH` | SQLite database path | `~/.subcog/index.db` |
| `SUBCOG_STORAGE_VECTOR_PATH` | Vector index path | `~/.subcog/vectors.usearch` |

## Feature Flags

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBCOG_FEATURES_SECRETS_FILTER` | Enable secrets detection | `true` |
| `SUBCOG_FEATURES_PII_FILTER` | Enable PII detection | `true` |
| `SUBCOG_FEATURES_MULTI_DOMAIN` | Enable multi-domain | `false` |
| `SUBCOG_FEATURES_AUDIT_LOG` | Enable audit logging | `false` |
| `SUBCOG_FEATURES_LLM_FEATURES` | Enable LLM features | `true` |
| `SUBCOG_FEATURES_AUTO_CAPTURE` | Enable auto-capture | `false` |
| `SUBCOG_FEATURES_CONSOLIDATION` | Enable consolidation | `false` |

## LLM Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBCOG_LLM_PROVIDER` | LLM provider | None |
| `SUBCOG_LLM_MODEL` | Model name | Provider default |
| `SUBCOG_LLM_TIMEOUT_MS` | Request timeout | `30000` |
| `SUBCOG_LLM_CONNECT_TIMEOUT_MS` | Connect timeout | `3000` |
| `SUBCOG_LLM_MAX_RETRIES` | Max retries | `0` |
| `SUBCOG_LLM_RETRY_BACKOFF_MS` | Retry backoff | `100` |
| `SUBCOG_LLM_BREAKER_FAILURE_THRESHOLD` | Circuit breaker failure threshold | `3` |
| `SUBCOG_LLM_BREAKER_RESET_MS` | Circuit breaker reset cooldown | `30000` |
| `SUBCOG_LLM_BREAKER_HALF_OPEN_MAX_CALLS` | Circuit breaker half-open attempts | `1` |
| `SUBCOG_LLM_LATENCY_SLO_MS` | Latency budget threshold | `2000` |
| `SUBCOG_LLM_ERROR_BUDGET_RATIO` | Error budget ratio threshold | `0.05` |
| `SUBCOG_LLM_ERROR_BUDGET_WINDOW_SECS` | Error budget window | `300` |

LLM settings can also be provided in `config.toml`. When both are set, environment
variables take precedence over config values.

### Provider-Specific

**Anthropic:**
| Variable | Description |
|----------|-------------|
| `ANTHROPIC_API_KEY` | Anthropic API key |
| `ANTHROPIC_BASE_URL` | Custom API endpoint |

**OpenAI:**
| Variable | Description |
|----------|-------------|
| `OPENAI_API_KEY` | OpenAI API key |
| `OPENAI_BASE_URL` | Custom API endpoint |
| `OPENAI_ORG_ID` | Organization ID |

**Ollama:**
| Variable | Description |
|----------|-------------|
| `OLLAMA_HOST` | Ollama server URL |

**LM Studio:**
| Variable | Description |
|----------|-------------|
| `LMSTUDIO_ENDPOINT` | LM Studio server URL |

## Search Intent Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBCOG_SEARCH_INTENT_ENABLED` | Enable intent detection | `true` |
| `SUBCOG_SEARCH_INTENT_USE_LLM` | Use LLM for detection | `true` |
| `SUBCOG_SEARCH_INTENT_LLM_TIMEOUT_MS` | LLM timeout | `200` |
| `SUBCOG_SEARCH_INTENT_MIN_CONFIDENCE` | Minimum confidence | `0.5` |

## Observability Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBCOG_METRICS_ENABLED` | Enable metrics | `false` |
| `SUBCOG_METRICS_PORT` | Metrics listener port (serve only) | `9090` |
| `SUBCOG_METRICS_PUSH_GATEWAY_ENDPOINT` | Push gateway endpoint | None |
| `SUBCOG_METRICS_PUSH_GATEWAY_USERNAME` | Push gateway username | None |
| `SUBCOG_METRICS_PUSH_GATEWAY_PASSWORD` | Push gateway password | None |
| `SUBCOG_METRICS_PUSH_GATEWAY_USE_POST` | Use POST instead of PUT | `false` |

## Hook Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBCOG_HOOK_ENABLED` | Enable hooks | `true` |
| `SUBCOG_SESSION_GUIDANCE` | Guidance level | `standard` |
| `SUBCOG_SESSION_MAX_TOKENS` | Max context tokens | `1000` |
| `SUBCOG_STOP_SYNC` | Sync on stop | `true` |
| `SUBCOG_STOP_SUMMARY` | Generate summary | `true` |
| `SUBCOG_AUTO_CAPTURE_ENABLED` | Auto-capture | `true` |
| `SUBCOG_AUTO_CAPTURE_DRY_RUN` | Dry run mode | `false` |
| `SUBCOG_POST_TOOL_ENABLED` | Post-tool hook | `true` |
| `SUBCOG_POST_TOOL_MAX_MEMORIES` | Max memories | `5` |

## PostgreSQL Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBCOG_PG_HOST` | PostgreSQL host | `localhost` |
| `SUBCOG_PG_PORT` | PostgreSQL port | `5432` |
| `SUBCOG_PG_DATABASE` | Database name | `subcog` |
| `SUBCOG_PG_USER` | Database user | `subcog` |
| `SUBCOG_PG_PASSWORD` | Database password | None |
| `SUBCOG_PG_SSL_MODE` | SSL mode | `prefer` |
| `SUBCOG_PG_POOL_SIZE` | Connection pool size | `10` |
| `DATABASE_URL` | Full connection URL | None |

## Redis Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBCOG_REDIS_URL` | Redis URL | `redis://localhost:6379` |
| `SUBCOG_REDIS_HOST` | Redis host | `localhost` |
| `SUBCOG_REDIS_PORT` | Redis port | `6379` |
| `SUBCOG_REDIS_PASSWORD` | Redis password | None |
| `SUBCOG_REDIS_DB` | Redis database | `0` |
| `SUBCOG_REDIS_PREFIX` | Key prefix | `subcog:` |
| `REDIS_URL` | Full Redis URL | None |

## Observability Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBCOG_METRICS_ENABLED` | Enable Prometheus metrics | `false` |
| `SUBCOG_METRICS_PORT` | Metrics endpoint port | `9090` |
| `SUBCOG_TRACING_ENABLED` | Enable distributed tracing | `false` |
| `SUBCOG_OTLP_ENDPOINT` | OTLP collector endpoint | None |
| `SUBCOG_OTLP_PROTOCOL` | OTLP protocol (`grpc` or `http`) | Auto |
| `SUBCOG_TRACE_SAMPLE_RATIO` | Trace sampling ratio (0.0-1.0) | `1.0` |
| `SUBCOG_LOG_FORMAT` | Log format (`json` or `pretty`) | `json` |
| `SUBCOG_LOG_LEVEL` | Log level (e.g. `info`, `debug`) | `info` |
| `SUBCOG_LOG_FILTER` | Full filter override (e.g. `subcog=debug,hyper=info`) | `RUST_LOG` |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | OpenTelemetry endpoint | None |

## Embedding Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBCOG_EMBEDDING_MODEL` | Embedding model | `all-MiniLM-L6-v2` |
| `SUBCOG_EMBEDDING_DIMENSIONS` | Vector dimensions | `384` |
| `SUBCOG_EMBEDDING_CACHE_SIZE` | Cache size | `1000` |

## Development Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBCOG_DEV_MODE` | Development mode | `false` |
| `RUST_LOG` | Rust log filter | None |
| `RUST_BACKTRACE` | Enable backtraces | `0` |

## Example Usage

### Basic Setup

```bash
export SUBCOG_DOMAIN=project
export SUBCOG_LOG_LEVEL=info
```

### With Anthropic

```bash
export SUBCOG_LLM_PROVIDER=anthropic
export ANTHROPIC_API_KEY=sk-ant-...
```

### PostgreSQL Backend

```bash
export SUBCOG_STORAGE_PERSISTENCE=postgresql
export SUBCOG_STORAGE_INDEX=postgresql
export DATABASE_URL="postgres://user:pass@host:5432/subcog"
```

### Debug Mode

```bash
export SUBCOG_LOG_LEVEL=debug
export RUST_BACKTRACE=1
```

### Production

```bash
export SUBCOG_FEATURES_AUDIT_LOG=true
export SUBCOG_METRICS_ENABLED=true
export SUBCOG_TRACING_ENABLED=true
export SUBCOG_OTLP_ENDPOINT=http://collector:4317
```

## See Also

- [Config File](config-file.md) - TOML configuration
- [Feature Flags](features.md) - Feature descriptions
- [File Locations](locations.md) - Platform paths
