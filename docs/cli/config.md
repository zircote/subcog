# subcog config

Manage configuration settings.

## Synopsis

```
subcog config [COMMAND] [OPTIONS]
```

## Description

The `config` command manages Subcog configuration at different scopes (project, user, system).

## Subcommands

| Command | Description |
|---------|-------------|
| `show` | Display current configuration |
| `get` | Get a specific configuration value |
| `set` | Set a configuration value |
| `unset` | Remove a configuration value |
| `edit` | Open configuration in editor |
| `path` | Show configuration file path |
| `init` | Initialize configuration file |

## Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--scope` | `-s` | Configuration scope | `project` |
| `--json` | | Output in JSON format | `false` |

## Scopes

| Scope | Location | Priority |
|-------|----------|----------|
| `project` | `./.subcog/config.toml` | Highest |
| `user` | `~/.config/subcog/config.toml` or `~/Library/Application Support/subcog/config.toml` | Medium |
| `system` | `/etc/subcog/config.toml` | Lowest |

Configuration is merged with higher priority values overriding lower.

## Examples

### Show Configuration

```bash
subcog config show
```

Output:
```toml
repo_path = \".\"

[features]
secrets_filter = true
pii_filter = true
multi_domain = false

[llm]
provider = \"anthropic\"
model = \"claude-sonnet-4-20250514\"
```

### Get Specific Value

```bash
subcog config get llm.provider
```

Output:
```
anthropic
```

### Set Value

```bash
subcog config set llm.model claude-sonnet-4-20250514
subcog config set features.audit_log true
```

### Set at User Scope

```bash
subcog config set -s user llm.provider anthropic
```

### Unset Value

```bash
subcog config unset features.audit_log
```

### Edit Configuration

```bash
subcog config edit
# Opens in $EDITOR
```

### Initialize Configuration

```bash
subcog config init
```

Creates `.subcog/config.toml` with defaults.

### Show Config Path

```bash
subcog config path
```

Output:
```
Project: /path/to/project/.subcog/config.toml
User: /Users/user/.config/subcog/config.toml
System: /etc/subcog/config.toml (not found)
```

## Configuration Reference

### Core Settings

```yaml
# Domain scope
domain: project  # project, user, or org

# Log level
log_level: info  # trace, debug, info, warn, error

# Git directory
git_dir: .git
```

### Storage Settings

```yaml
storage:
  # Persistence layer
  persistence: git_notes  # git_notes, postgresql, filesystem

  # Index layer
  index: sqlite  # sqlite, postgresql, redis

  # Vector layer
  vector: usearch  # usearch, pgvector, redis

  # SQLite path
  sqlite_path: ~/.subcog/index.db

  # Vector path
  vector_path: ~/.subcog/vectors.usearch
```

### Feature Flags

```yaml
features:
  # Security
  secrets_filter: true
  pii_filter: true

  # Domains
  multi_domain: false

  # Observability
  audit_log: false

  # LLM features
  llm_features: true
  auto_capture: false
  consolidation: false
```

### LLM Settings

```yaml
llm:
  provider: anthropic  # anthropic, openai, ollama, lmstudio
  model: claude-sonnet-4-20250514
  # Request timeouts, retries, and circuit breaker settings.
  # Environment variables (SUBCOG_LLM_*) override these values when set.
  timeout_ms: 30000
  connect_timeout_ms: 3000
  max_retries: 0
  retry_backoff_ms: 100
  breaker_failure_threshold: 3
  breaker_reset_ms: 30000
  breaker_half_open_max_calls: 1
  latency_slo_ms: 2000
  error_budget_ratio: 0.05
  error_budget_window_secs: 300
```

### Search Intent Settings

```yaml
search_intent:
  enabled: true
  use_llm: true
  llm_timeout_ms: 200
  min_confidence: 0.5
```

### PostgreSQL Settings

```yaml
postgresql:
  host: localhost
  port: 5432
  database: subcog
  user: subcog
  password: ${SUBCOG_PG_PASSWORD}  # Environment variable
  ssl_mode: prefer
```

### Redis Settings

```yaml
redis:
  url: redis://localhost:6379
  prefix: subcog:
```

## Environment Variable Override

All configuration can be overridden via environment variables:

```bash
SUBCOG_DOMAIN=user
SUBCOG_LOG_LEVEL=debug
SUBCOG_STORAGE_PERSISTENCE=postgresql
SUBCOG_FEATURES_SECRETS_FILTER=true
SUBCOG_LLM_PROVIDER=anthropic
```

Pattern: `SUBCOG_<SECTION>_<KEY>` (uppercase, underscores)

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Error |
| 2 | Invalid arguments |
| 3 | Configuration error |

## See Also

- [Configuration Reference](../configuration/README.md) - Full configuration documentation
- [Environment Variables](../configuration/environment.md) - All environment variables
- [Feature Flags](../configuration/features.md) - Feature flag details
