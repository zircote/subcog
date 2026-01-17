# Feature Flags

Subcog uses feature flags to enable optional functionality. This allows the system to run in minimal mode while providing advanced features when configured.

Feature flags are configured in `config.toml`. The only environment override currently supported is `SUBCOG_ORG_SCOPE_ENABLED`.

## Feature Tiers

| Tier | Features | Requirements |
|------|----------|--------------|
| **Core** | Capture, search, CLI, MCP | None |
| **Enhanced** | Security, multi-domain, audit | Configuration |
| **LLM-Powered** | Auto-capture, consolidation, enrichment | LLM provider |

## Security Features

### secrets_filter

Detects and blocks content containing secrets.

```yaml
features:
 secrets_filter: true
```

**Detected patterns:**
- API keys (AWS, GCP, Azure, GitHub, etc.)
- Access tokens (Bearer, OAuth, JWT)
- Passwords in connection strings
- Private keys (SSH, PGP)
- Cloud credentials

**Behavior:**
- `true`: Block capture, return error
- `false`: Allow capture (not recommended)

---

### pii_filter

Detects and redacts personally identifiable information.

```yaml
features:
 pii_filter: true
```

**Detected patterns:**
- Email addresses
- Phone numbers
- Social Security Numbers
- Credit card numbers
- IP addresses (optionally)

**Behavior:**
- `true`: Redact PII with `[REDACTED]`
- `false`: Store as-is

## Scoping Features

### multi_domain

Enables storing and searching memories across multiple domains.

```yaml
features:
 multi_domain: false
```

**When enabled:**
- Project, user, and org memories accessible
- Cross-domain search with `subcog://_` URIs
- Domain inheritance (org -> user -> project)

**When disabled:**
- Only project-scoped memories
- Simpler storage model
- Lower resource usage

### org_scope_enabled

Enables org-scoped storage (shared PostgreSQL persistence).

```yaml
features:
 org_scope_enabled: false
```

**When enabled:**
- Org-scoped storage is allowed when org configuration is provided
- Shared memories can be stored in PostgreSQL
- Requires org-scope initialization at runtime

**When disabled:**
- Org-scoped storage is rejected
- Project/user scopes only

**Environment:** `SUBCOG_ORG_SCOPE_ENABLED`

## Observability Features

### audit_log

Enables SOC2/GDPR-compliant audit logging.

```yaml
features:
 audit_log: false
```

**Logged events:**
- Memory captures (who, what, when)
- Memory access (who accessed what)
- Configuration changes
- Authentication events (MCP)

**Log format:**
```json
{
 "timestamp": "2024-01-15T10:30:00Z",
 "event": "memory_capture",
 "actor": "user@example.com",
 "resource": "subcog://project/decisions/abc123",
 "action": "create",
 "result": "success"
}
```

## LLM Features

### llm_features

Master toggle for all LLM-powered functionality.

```yaml
features:
 llm_features: true
```

**Enables:**
- Enhanced search intent detection
- Memory enrichment
- Memory consolidation
- Auto-capture

**Requires:**
- LLM provider configuration
- Valid API key

---

### auto_capture

Automatically captures important context from hooks.

```yaml
features:
 auto_capture: false
```

**Behavior:**
- Scans prompts for decision/pattern/learning signals
- Auto-captures with appropriate namespace
- Deduplicates against existing memories

**Requires:** `llm_features: true`

---

### consolidation

Enables LLM-powered memory consolidation.

```yaml
features:
 consolidation: false
```

**Behavior:**
- Identifies similar memories
- Merges duplicates using LLM
- Creates summaries of related memories

**Requires:** `llm_features: true`

## Search Intent Configuration

The search intent system detects user intent from prompts and injects relevant memories. Configure it under `[search_intent]`.

### Basic Settings

```toml
[search_intent]
enabled = true # Enable search intent detection
use_llm = true # Use LLM for enhanced classification
llm_timeout_ms = 200 # LLM classification timeout
min_confidence = 0.5 # Minimum confidence to inject memories
base_count = 5 # Memories for low-confidence matches
max_count = 15 # Memories for high-confidence matches
max_tokens = 4000 # Token budget for injected context
```

**Environment Variables:**
- `SUBCOG_SEARCH_INTENT_ENABLED`
- `SUBCOG_SEARCH_INTENT_USE_LLM`
- `SUBCOG_SEARCH_INTENT_LLM_TIMEOUT_MS`
- `SUBCOG_SEARCH_INTENT_MIN_CONFIDENCE`

### Namespace Weights

Namespace weights are multipliers applied to relevance scores during search. Higher values prioritize that namespace for the given intent type. Default is 1.0.

**Intent Types:**
| Intent | Trigger Examples | Default Priority |
|--------|------------------|------------------|
| `HowTo` | "how do I...", "implement..." | patterns > learnings > decisions |
| `Troubleshoot` | "error...", "fix...", "debug..." | blockers > learnings > decisions |
| `Location` | "where is...", "find..." | decisions > context > patterns |
| `Explanation` | "what is...", "explain..." | decisions > context > patterns |
| `Comparison` | "X vs Y", "difference between..." | decisions > patterns > learnings |
| `General` | "search...", "show me..." | balanced weights |

**Configuration Example:**

```toml
# Boost blockers heavily for troubleshooting
[search_intent.weights.troubleshoot]
blockers = 2.0
learnings = 1.5
tech-debt = 1.2
decisions = 1.0

# Prioritize patterns for how-to questions
[search_intent.weights.howto]
patterns = 2.0
learnings = 1.5
decisions = 1.0

# Custom weights for location queries
[search_intent.weights.location]
decisions = 1.8
context = 1.5
apis = 1.3
config = 1.2
```

**Available Namespaces:**
- `decisions` - Architectural and design decisions
- `patterns` - Discovered patterns and conventions
- `learnings` - Lessons from debugging or issues
- `context` - Important contextual information
- `tech-debt` - Technical debts and improvements
- `blockers` - Blockers and impediments
- `progress` - Work progress and milestones
- `apis` - API endpoints and contracts
- `config` - Configuration and environment
- `security` - Security-related information
- `performance` - Performance optimizations
- `testing` - Testing strategies and edge cases

**Behavior:**
- Weights are multipliers (1.0 = no change, 2.0 = double priority)
- Unspecified namespaces default to 1.0
- Config values override hard-coded defaults
- Works with both keyword and LLM detection modes

## Search Intent Configuration

The search intent system detects user intent from prompts and injects relevant memories. Configure it under `[search_intent]`.

### Basic Settings

```toml
[search_intent]
enabled = true # Enable search intent detection
use_llm = true # Use LLM for enhanced classification
llm_timeout_ms = 200 # LLM classification timeout
min_confidence = 0.5 # Minimum confidence to inject memories
base_count = 5 # Memories for low-confidence matches
max_count = 15 # Memories for high-confidence matches
max_tokens = 4000 # Token budget for injected context
```

**Environment Variables:**
- `SUBCOG_SEARCH_INTENT_ENABLED`
- `SUBCOG_SEARCH_INTENT_USE_LLM`
- `SUBCOG_SEARCH_INTENT_LLM_TIMEOUT_MS`
- `SUBCOG_SEARCH_INTENT_MIN_CONFIDENCE`

### Namespace Weights

Namespace weights are multipliers applied to relevance scores during search. Higher values prioritize that namespace for the given intent type. Default is 1.0.

**Intent Types:**
| Intent | Trigger Examples | Default Priority |
|--------|------------------|------------------|
| `HowTo` | "how do I...", "implement..." | patterns > learnings > decisions |
| `Troubleshoot` | "error...", "fix...", "debug..." | blockers > learnings > decisions |
| `Location` | "where is...", "find..." | decisions > context > patterns |
| `Explanation` | "what is...", "explain..." | decisions > context > patterns |
| `Comparison` | "X vs Y", "difference between..." | decisions > patterns > learnings |
| `General` | "search...", "show me..." | balanced weights |

**Configuration Example:**

```toml
# Boost blockers heavily for troubleshooting
[search_intent.weights.troubleshoot]
blockers = 2.0
learnings = 1.5
tech-debt = 1.2
decisions = 1.0

# Prioritize patterns for how-to questions
[search_intent.weights.howto]
patterns = 2.0
learnings = 1.5
decisions = 1.0

# Custom weights for location queries
[search_intent.weights.location]
decisions = 1.8
context = 1.5
apis = 1.3
config = 1.2
```

**Available Namespaces:**
- `decisions` - Architectural and design decisions
- `patterns` - Discovered patterns and conventions
- `learnings` - Lessons from debugging or issues
- `context` - Important contextual information
- `tech-debt` - Technical debts and improvements
- `blockers` - Blockers and impediments
- `progress` - Work progress and milestones
- `apis` - API endpoints and contracts
- `config` - Configuration and environment
- `security` - Security-related information
- `performance` - Performance optimizations
- `testing` - Testing strategies and edge cases

**Behavior:**
- Weights are multipliers (1.0 = no change, 2.0 = double priority)
- Unspecified namespaces default to 1.0
- Config values override hard-coded defaults
- Works with both keyword and LLM detection modes

## Feature Dependencies

```
┌─────────────────────────────────────────────────────────┐
│ LLM Provider │
│ │ │
│ llm_features │
│ / | \ │
│ auto_capture │ consolidation │
│ │ │
│ search_intent │
│ (enhanced mode) │
└─────────────────────────────────────────────────────────┘
```

## Recommended Configurations

### Personal Development

```yaml
features:
 secrets_filter: true
 pii_filter: false
 multi_domain: false
 audit_log: false
 llm_features: true
 auto_capture: true
 consolidation: false
```

### Team Development

```yaml
features:
 secrets_filter: true
 pii_filter: true
 multi_domain: true
 audit_log: false
 llm_features: true
 auto_capture: false
 consolidation: true
```

### Enterprise/Compliance

```yaml
features:
 secrets_filter: true
 pii_filter: true
 multi_domain: true
 audit_log: true
 llm_features: true
 auto_capture: false
 consolidation: true
```

### Minimal/Offline

```yaml
features:
 secrets_filter: true
 pii_filter: true
 multi_domain: false
 audit_log: false
 llm_features: false
 auto_capture: false
 consolidation: false
```

## Checking Feature Status

```bash
subcog status
```

Shows feature status in output:

```
Features
────────
 Secrets filter
 PII filter
 Multi-domain (disabled)
 Audit log (disabled)
 LLM features
 Auto-capture
 Consolidation (disabled)
```

## See Also

- [Config File](config-file.md) - Full configuration format
- [Environment Variables](environment.md) - All variables
- [Architecture](../architecture/README.md) - System design
