# Feature Flags

Subcog uses feature flags to enable optional functionality. This allows the system to run in minimal mode while providing advanced features when configured.

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

**Environment:** `SUBCOG_FEATURES_SECRETS_FILTER`

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

**Environment:** `SUBCOG_FEATURES_PII_FILTER`

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
- Domain inheritance (org → user → project)

**When disabled:**
- Only project-scoped memories
- Simpler storage model
- Lower resource usage

**Environment:** `SUBCOG_FEATURES_MULTI_DOMAIN`

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
  "resource": "urn:subcog:project:decisions:abc123",
  "action": "create",
  "result": "success"
}
```

**Environment:** `SUBCOG_FEATURES_AUDIT_LOG`

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

**Environment:** `SUBCOG_FEATURES_LLM_FEATURES`

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

**Environment:** `SUBCOG_FEATURES_AUTO_CAPTURE`

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

**Environment:** `SUBCOG_FEATURES_CONSOLIDATION`

## Feature Dependencies

```
┌─────────────────────────────────────────────────────────┐
│                    LLM Provider                          │
│                         │                                │
│                    llm_features                          │
│                    /    |    \                           │
│            auto_capture │  consolidation                 │
│                         │                                │
│                search_intent                             │
│                (enhanced mode)                           │
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
  ✓ Secrets filter
  ✓ PII filter
  ✗ Multi-domain (disabled)
  ✗ Audit log (disabled)
  ✓ LLM features
  ✓ Auto-capture
  ✗ Consolidation (disabled)
```

## See Also

- [Config File](config-file.md) - Full configuration format
- [Environment Variables](environment.md) - All variables
- [Architecture](../architecture/README.md) - System design
