# Configuration

Subcog supports flexible configuration through TOML files, environment variables, and command-line options.
For a full template with every option and sane defaults, see `example.config.toml` in the repository root.

## Quick Reference

| Topic | Description |
|-------|-------------|
| [Config File](config-file.md) | TOML configuration format |
| [Environment Variables](environment.md) | All environment variables |
| [Feature Flags](features.md) | Optional feature toggles |
| [File Locations](locations.md) | OS-specific paths |

## Configuration Sources

Configuration is loaded from multiple sources with the following precedence (highest to lowest):

1. **Command-line options** - `--config`, `--verbose`, etc.
2. **Environment variables** - `SUBCOG_*`
3. **Project config** - `.subcog/config.toml`
4. **User config (XDG)** - `~/.config/subcog/config.toml`
5. **User config (macOS)** - `~/Library/Application Support/subcog/config.toml`
6. **System config** - `/etc/subcog/config.toml`
7. **Defaults** - Built-in defaults

## Minimal Configuration

Subcog works out-of-the-box with defaults. Minimal setup:

```toml
# .subcog/config.toml
repo_path = "."
```

## Example Configuration

```toml
repo_path = "."
max_results = 10

default_search_mode = "hybrid"

[features]
secrets_filter = true
pii_filter = true
llm_features = true
org_scope_enabled = false

audit_log = false

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"

[search_intent]
enabled = true
use_llm = true
llm_timeout_ms = 200
min_confidence = 0.5

[observability.logging]
level = "info"
```

## Environment Variable Mapping

Environment variables follow the pattern: `SUBCOG_<SECTION>_<KEY>`

## See Also

- [Config File](config-file.md) - TOML configuration format
- [Environment Variables](environment.md) - All variables
- [Feature Flags](features.md) - Feature details
- [File Locations](locations.md) - Platform paths
