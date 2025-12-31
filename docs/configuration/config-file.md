# Configuration File

Subcog uses YAML configuration files for persistent settings.

## File Locations

Configuration files are loaded in order (later overrides earlier):

| Scope | Path |
|-------|------|
| System | `/etc/subcog/config.yaml` |
| User | `~/.subcog/config.yaml` |
| Project | `.subcog/config.yaml` |

## Configuration Sections

### domain

Scope for memory storage.

```yaml
domain: project  # project, user, or org
```

| Value | Description |
|-------|-------------|
| `project` | Repository-scoped memories (default) |
| `user` | User-wide memories |
| `org` | Organization-level memories |

### log_level

Logging verbosity.

```yaml
log_level: info  # trace, debug, info, warn, error
```

### git_dir

Custom git directory path.

```yaml
git_dir: .git
```

### storage

Storage backend configuration.

```yaml
storage:
  # Persistence layer (authoritative storage)
  persistence: git_notes  # git_notes, postgresql, filesystem

  # Index layer (searchable)
  index: sqlite  # sqlite, postgresql, redis

  # Vector layer (embeddings)
  vector: usearch  # usearch, pgvector, redis

  # SQLite configuration
  sqlite_path: ~/.subcog/index.db

  # Vector index path
  vector_path: ~/.subcog/vectors.usearch

  # Embedding model
  embedding_model: all-MiniLM-L6-v2
  embedding_dimensions: 384
```

### features

Feature flags for optional functionality.

```yaml
features:
  # Security
  secrets_filter: true   # Detect and block secrets
  pii_filter: true       # Detect and redact PII

  # Scoping
  multi_domain: false    # Enable multi-domain support

  # Observability
  audit_log: false       # SOC2/GDPR audit logging

  # LLM Features
  llm_features: true     # Enable LLM-powered features
  auto_capture: false    # Auto-capture from hooks
  consolidation: false   # Enable consolidation
```

### llm

LLM provider configuration.

```yaml
llm:
  # Provider selection
  provider: anthropic  # anthropic, openai, ollama, lmstudio

  # Model selection
  model: claude-sonnet-4-20250514

  # API settings
  api_key: ${ANTHROPIC_API_KEY}  # Environment variable reference
  base_url: https://api.anthropic.com

  # Request settings
  timeout_ms: 30000
  max_retries: 3
  retry_delay_ms: 1000
```

### search_intent

Search intent detection settings.

```yaml
search_intent:
  enabled: true           # Enable intent detection
  use_llm: true          # Use LLM for classification
  llm_timeout_ms: 200    # LLM timeout
  min_confidence: 0.5    # Minimum confidence threshold
```

### postgresql

PostgreSQL connection settings.

```yaml
postgresql:
  host: localhost
  port: 5432
  database: subcog
  user: subcog
  password: ${SUBCOG_PG_PASSWORD}  # Use env var

  # Connection pool
  pool_size: 10
  pool_timeout_ms: 5000

  # SSL
  ssl_mode: prefer  # disable, prefer, require

  # Schema
  schema: public
```

### redis

Redis connection settings.

```yaml
redis:
  url: redis://localhost:6379

  # Or individual settings
  host: localhost
  port: 6379
  password: ${SUBCOG_REDIS_PASSWORD}
  db: 0

  # Namespace prefix
  prefix: subcog:

  # Connection settings
  pool_size: 10
  timeout_ms: 5000
```

### hooks

Hook behavior configuration.

```yaml
hooks:
  # Session start
  session_start:
    guidance_level: standard  # minimal, standard, full
    max_tokens: 1000

  # User prompt
  user_prompt:
    max_memories: 15
    skip_detection: false

  # Stop
  stop:
    sync: true
    summary: true
```

### observability

Telemetry and monitoring settings.

```yaml
observability:
  # Metrics
  metrics_enabled: false
  metrics_port: 9090

  # Tracing
  tracing_enabled: false
  otlp_endpoint: http://localhost:4317

  # Logging
  structured_logging: true
  log_format: json  # json, pretty
```

## Environment Variable Substitution

Use `${VAR_NAME}` to reference environment variables:

```yaml
llm:
  api_key: ${ANTHROPIC_API_KEY}

postgresql:
  password: ${SUBCOG_PG_PASSWORD}
```

## Example Configurations

### Minimal (Git Notes Only)

```yaml
domain: project
```

### Development

```yaml
domain: project
log_level: debug

features:
  secrets_filter: true
  llm_features: true
  auto_capture: true
```

### Production (PostgreSQL)

```yaml
domain: project
log_level: info

storage:
  persistence: postgresql
  index: postgresql
  vector: pgvector

postgresql:
  host: ${DB_HOST}
  port: 5432
  database: subcog_prod
  user: subcog
  password: ${DB_PASSWORD}
  ssl_mode: require
  pool_size: 20

features:
  secrets_filter: true
  pii_filter: true
  audit_log: true

observability:
  metrics_enabled: true
  tracing_enabled: true
```

### Team Collaboration

```yaml
domain: project

features:
  multi_domain: true

llm:
  provider: anthropic
  model: claude-sonnet-4-20250514
```

## Validation

Validate configuration:

```bash
subcog config show --validate
```

## See Also

- [Environment Variables](environment.md) - All variables
- [Feature Flags](features.md) - Feature details
- [File Locations](locations.md) - Platform paths
