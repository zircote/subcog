# Configuration File

Subcog uses TOML configuration files for persistent settings.

For a full copy-paste template with every option and sane defaults, see
`example.config.toml` in the repository root.

## File Locations

Configuration files are loaded in order (later overrides earlier):

| Scope | Path |
|-------|------|
| System | `/etc/subcog/config.toml` |
| User (XDG) | `~/.config/subcog/config.toml` |
| User (macOS) | `~/Library/Application Support/subcog/config.toml` |
| Project | `.subcog/config.toml` |

Environment variables override config file values when both are set.

## Core Settings

```toml
repo_path = "/path/to/repo"
data_dir = "/path/to/data"
max_results = 10
default_search_mode = "hybrid" # text, vector, hybrid
```

## Feature Flags

```toml
[features]
secrets_filter = true
pii_filter = true
multi_domain = false
audit_log = false
llm_features = true
auto_capture = false
consolidation = false
org_scope_enabled = false
```

## LLM

```toml
[llm]
provider = "anthropic" # anthropic, openai, ollama, lmstudio
model = "claude-sonnet-4-20250514"
api_key = "" # leave empty to use env vars
base_url = "https://api.anthropic.com"

timeout_ms = 30000
connect_timeout_ms = 3000
max_retries = 0
retry_backoff_ms = 100
breaker_failure_threshold = 3
breaker_reset_ms = 30000
breaker_half_open_max_calls = 1
latency_slo_ms = 2000
error_budget_ratio = 0.05
error_budget_window_secs = 300
```

Empty strings for `model`, `api_key`, and `base_url` are treated as unset.

## Search Intent

```toml
[search_intent]
enabled = true
use_llm = true
llm_timeout_ms = 200
min_confidence = 0.5
base_count = 5
max_count = 15
max_tokens = 4000
```

## Observability

```toml
[observability.logging]
format = "json" # json, pretty
level = "info"
filter = "subcog=info"

[observability.tracing]
enabled = false
sample_ratio = 1.0
service_name = "subcog"
resource_attributes = ["env=prod", "region=us-east-1"]

[observability.tracing.otlp]
endpoint = "http://collector:4317"
protocol = "grpc" # grpc, http

[observability.metrics]
enabled = false
port = 9090

[observability.metrics.push_gateway]
endpoint = "http://push-gateway:9091/metrics/job/subcog"
username = ""
password = ""
use_http_post = false
```

## Environment Variable Substitution

Use `${VAR_NAME}` to reference environment variables:

```toml
[llm]
api_key = "${ANTHROPIC_API_KEY}"
```

## Example Configurations

### Minimal

```toml
repo_path = "."
```

### Development

```toml
repo_path = "."

default_search_mode = "hybrid"

[features]
secrets_filter = true
llm_features = true
auto_capture = true
org_scope_enabled = false

[observability.logging]
level = "debug"
```

### Production

```toml
repo_path = "/srv/subcog"

default_search_mode = "hybrid"

[features]
secrets_filter = true
pii_filter = true
audit_log = true
org_scope_enabled = false

[observability.metrics]
enabled = true

[observability.tracing]
enabled = true

[observability.tracing.otlp]
endpoint = "http://collector:4317"
protocol = "grpc"
```

## Complete Example

```toml
repo_path = "."
data_dir = ".subcog"
max_results = 10
default_search_mode = "hybrid" # text, vector, hybrid

[features]
secrets_filter = false
pii_filter = false
multi_domain = false
audit_log = false
llm_features = false
auto_capture = false
consolidation = false
org_scope_enabled = false

[llm]
provider = "anthropic" # anthropic, openai, ollama, lmstudio
model = "claude-sonnet-4-20250514"
api_key = "" # leave empty to use env vars
base_url = "https://api.anthropic.com/v1"
timeout_ms = 30000
connect_timeout_ms = 3000
max_retries = 0
retry_backoff_ms = 100
breaker_failure_threshold = 3
breaker_reset_ms = 30000
breaker_half_open_max_calls = 1
latency_slo_ms = 2000
error_budget_ratio = 0.05
error_budget_window_secs = 300

[search_intent]
enabled = true
use_llm = true
llm_timeout_ms = 200
min_confidence = 0.5
base_count = 5
max_count = 15
max_tokens = 4000

[observability.logging]
format = "json" # json, pretty
level = "info"
filter = "subcog=info"

[observability.tracing]
enabled = false
sample_ratio = 1.0
service_name = "subcog"
resource_attributes = []

[observability.tracing.otlp]
endpoint = "http://localhost:4317"
protocol = "grpc" # grpc, http

[observability.metrics]
enabled = false
port = 9090

[observability.metrics.push_gateway]
endpoint = ""
username = ""
password = ""
use_http_post = false
```

## See Also

- [Environment Variables](environment.md) - All variables
- [Feature Flags](features.md) - Feature details
- [File Locations](locations.md) - Platform paths
