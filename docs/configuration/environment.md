# Environment Variables

For the complete reference of all environment variables, see:

**[Environment Variables Reference](../environment-variables.md)**

This consolidated reference includes:
- Core configuration
- LLM provider settings and resilience
- Search intent detection
- Deduplication service
- Storage backends (PostgreSQL, Redis, SQLite)
- Observability (logging, tracing, metrics)
- Feature flags
- Hook settings
- API key configuration examples
- Complete config file example

## Quick Reference

| Category | Common Variables |
|----------|-----------------|
| Core | `SUBCOG_REPO_PATH`, `SUBCOG_DATA_DIR`, `SUBCOG_LOG_LEVEL` |
| LLM | `SUBCOG_LLM_PROVIDER`, `SUBCOG_LLM_MODEL`, `ANTHROPIC_API_KEY` |
| Storage | `SUBCOG_STORAGE_PERSISTENCE`, `DATABASE_URL`, `REDIS_URL` |
| Observability | `SUBCOG_METRICS_ENABLED`, `SUBCOG_TRACING_ENABLED` |
| Features | `SUBCOG_ORG_SCOPE_ENABLED` |

## See Also

- [Config File](config-file.md) - TOML configuration
- [Feature Flags](features.md) - Feature descriptions
- [File Locations](locations.md) - Platform paths
