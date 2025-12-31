# Configuration

Subcog supports flexible configuration through YAML files, environment variables, and command-line options.

## Quick Reference

| Topic | Description |
|-------|-------------|
| [Config File](config-file.md) | YAML configuration format |
| [Environment Variables](environment.md) | All environment variables |
| [Feature Flags](features.md) | Optional feature toggles |
| [File Locations](locations.md) | OS-specific paths |

## Configuration Sources

Configuration is loaded from multiple sources with the following precedence (highest to lowest):

1. **Command-line options** - `--config`, `--verbose`, etc.
2. **Environment variables** - `SUBCOG_*`
3. **Project config** - `.subcog/config.yaml`
4. **User config** - `~/.subcog/config.yaml`
5. **System config** - `/etc/subcog/config.yaml`
6. **Defaults** - Built-in defaults

## Quick Start

### Create Project Configuration

```bash
subcog config init
```

Creates `.subcog/config.yaml`:

```yaml
domain: project

storage:
  persistence: git_notes
  index: sqlite
  vector: usearch

features:
  secrets_filter: true
  pii_filter: true
```

### View Current Configuration

```bash
subcog config show
```

### Set a Value

```bash
subcog config set llm.provider anthropic
```

## Minimal Configuration

Subcog works out-of-the-box with defaults. Minimal setup:

```yaml
# .subcog/config.yaml
domain: project
```

## Full Configuration Example

```yaml
# Domain scope
domain: project  # project, user, or org

# Logging
log_level: info  # trace, debug, info, warn, error

# Storage backends
storage:
  persistence: git_notes  # git_notes, postgresql, filesystem
  index: sqlite           # sqlite, postgresql, redis
  vector: usearch         # usearch, pgvector, redis

  # SQLite paths
  sqlite_path: ~/.subcog/index.db
  vector_path: ~/.subcog/vectors.usearch

# Feature flags
features:
  secrets_filter: true
  pii_filter: true
  multi_domain: false
  audit_log: false
  llm_features: true
  auto_capture: false
  consolidation: false

# LLM configuration
llm:
  provider: anthropic  # anthropic, openai, ollama, lmstudio
  model: claude-sonnet-4-20250514
  timeout_ms: 30000
  max_retries: 3

# Search intent detection
search_intent:
  enabled: true
  use_llm: true
  llm_timeout_ms: 200
  min_confidence: 0.5

# PostgreSQL (if using)
postgresql:
  host: localhost
  port: 5432
  database: subcog
  user: subcog
  password: ${SUBCOG_PG_PASSWORD}
  ssl_mode: prefer

# Redis (if using)
redis:
  url: redis://localhost:6379
  prefix: subcog:
```

## Environment Variable Mapping

Environment variables follow the pattern: `SUBCOG_<SECTION>_<KEY>`

| Config Key | Environment Variable |
|------------|---------------------|
| `domain` | `SUBCOG_DOMAIN` |
| `log_level` | `SUBCOG_LOG_LEVEL` |
| `storage.persistence` | `SUBCOG_STORAGE_PERSISTENCE` |
| `features.secrets_filter` | `SUBCOG_FEATURES_SECRETS_FILTER` |
| `llm.provider` | `SUBCOG_LLM_PROVIDER` |

## Best Practices

1. **Use project config for project-specific settings**
2. **Use user config for personal preferences** (LLM keys, themes)
3. **Use environment variables for secrets** (API keys, passwords)
4. **Never commit API keys** to version control

## See Also

- [Config File](config-file.md) - Detailed format reference
- [Environment Variables](environment.md) - Complete variable list
- [Feature Flags](features.md) - Feature descriptions
- [File Locations](locations.md) - Platform-specific paths
