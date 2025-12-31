# Security

Security and privacy features in Subcog.

## Security Features

| Feature | Description | Default |
|---------|-------------|---------|
| Secrets Filter | Detect and block API keys, tokens | Enabled |
| PII Filter | Detect and redact personal data | Enabled |
| Audit Log | SOC2/GDPR-compliant logging | Disabled |
| Content Validation | Validate input before storage | Enabled |

## Secrets Detection

Prevents storage of sensitive credentials.

### Detected Patterns

| Category | Examples |
|----------|----------|
| API Keys | AWS, GCP, Azure, GitHub, Stripe |
| Access Tokens | Bearer tokens, OAuth tokens, JWT |
| Private Keys | SSH, PGP, RSA |
| Passwords | Connection strings, config files |
| Credentials | AWS credentials, database passwords |

### Implementation

```rust
pub struct SecretsDetector {
    patterns: Vec<CompiledPattern>,
}

impl SecretsDetector {
    pub fn check(&self, content: &str) -> Result<(), SecurityError> {
        for pattern in &self.patterns {
            if pattern.regex.is_match(content) {
                return Err(SecurityError::SecretDetected {
                    pattern_name: pattern.name.clone(),
                });
            }
        }
        Ok(())
    }
}
```

### Pattern Examples

```rust
patterns: [
    // AWS Access Key
    Pattern {
        name: "aws_access_key",
        regex: r"AKIA[0-9A-Z]{16}",
    },
    // GitHub Token
    Pattern {
        name: "github_token",
        regex: r"gh[ps]_[a-zA-Z0-9]{36}",
    },
    // Generic API Key
    Pattern {
        name: "generic_api_key",
        regex: r"(?i)(api[_-]?key|apikey)\s*[:=]\s*['\"]?[\w-]{20,}",
    },
]
```

### Behavior

When secrets are detected:
1. Capture is blocked
2. Error returned with pattern name
3. Event logged (if audit enabled)

## PII Detection

Detects and redacts personally identifiable information.

### Detected Patterns

| Category | Examples |
|----------|----------|
| Email | user@example.com |
| Phone | +1-555-123-4567 |
| SSN | 123-45-6789 |
| Credit Card | 4111-1111-1111-1111 |
| IP Address | 192.168.1.1 (optional) |

### Implementation

```rust
pub struct PiiDetector {
    patterns: Vec<PiiPattern>,
    redactor: Redactor,
}

impl PiiDetector {
    pub fn redact(&self, content: &str) -> String {
        let mut result = content.to_string();

        for pattern in &self.patterns {
            result = pattern.regex.replace_all(&result, |caps: &Captures| {
                self.redactor.redact(&caps[0], &pattern.category)
            }).to_string();
        }

        result
    }
}
```

### Redaction Formats

| Category | Example | Redacted |
|----------|---------|----------|
| Email | user@example.com | [EMAIL REDACTED] |
| Phone | 555-123-4567 | [PHONE REDACTED] |
| SSN | 123-45-6789 | [SSN REDACTED] |
| Credit Card | 4111... | [CC REDACTED] |

## Audit Logging

SOC2/GDPR-compliant audit trail.

### Events Logged

| Event | Data Captured |
|-------|--------------|
| memory_capture | actor, memory_id, namespace, timestamp |
| memory_access | actor, memory_id, access_type, timestamp |
| memory_delete | actor, memory_id, reason, timestamp |
| search_executed | actor, query_hash, result_count, timestamp |
| config_change | actor, setting, old_value, new_value, timestamp |
| sync_executed | actor, direction, count, timestamp |

### Log Format

```json
{
  "timestamp": "2024-01-15T10:30:00.000Z",
  "event_type": "memory_capture",
  "event_id": "evt_abc123",
  "actor": {
    "type": "user",
    "id": "user@example.com"
  },
  "resource": {
    "type": "memory",
    "id": "dc58d23a...",
    "namespace": "decisions"
  },
  "action": "create",
  "result": "success",
  "metadata": {
    "source": "cli",
    "tags": ["database"]
  }
}
```

### Configuration

```yaml
features:
  audit_log: true

observability:
  audit_log_path: /var/log/subcog/audit.log
  audit_log_format: json
  audit_log_retention_days: 90
```

## Content Validation

Input validation before storage.

### Validation Rules

| Rule | Description |
|------|-------------|
| Max Length | Content < 100KB |
| Valid UTF-8 | Reject invalid encoding |
| Namespace Valid | Must be known namespace |
| Tags Valid | Alphanumeric, hyphens only |
| Source Valid | Valid file path format |

### Implementation

```rust
pub fn validate_capture_request(req: &CaptureRequest) -> Result<()> {
    // Content length
    if req.content.len() > MAX_CONTENT_LENGTH {
        return Err(ValidationError::ContentTooLarge);
    }

    // Valid namespace
    if !Namespace::is_valid(&req.namespace) {
        return Err(ValidationError::InvalidNamespace);
    }

    // Valid tags
    for tag in &req.tags {
        if !is_valid_tag(tag) {
            return Err(ValidationError::InvalidTag(tag.clone()));
        }
    }

    Ok(())
}
```

## Access Control

### Domain-Based Access

Memories are scoped to domains:
- Project memories: Accessible in project only
- User memories: Accessible by user only
- Org memories: Accessible by org members

### MCP Authentication

HTTP transport can be protected:
```yaml
mcp:
  transport: http
  auth:
    type: bearer
    token: ${SUBCOG_MCP_TOKEN}
```

## Data Protection

### At Rest

- Git notes: Protected by filesystem permissions
- SQLite: File permissions (600)
- PostgreSQL: Database authentication

### In Transit

- MCP stdio: Local only, no network
- MCP HTTP: TLS recommended
- Git sync: SSH/HTTPS

## Security Best Practices

1. **Enable secrets filter** - Always on for production
2. **Enable PII filter** - Unless explicitly storing PII
3. **Use environment variables** - Never store API keys in config
4. **Restrict file permissions** - 600 for data files
5. **Enable audit logging** - For compliance requirements
6. **Use HTTPS for remote** - When using HTTP transport

## Threat Model

### Threats Addressed

| Threat | Mitigation |
|--------|------------|
| Secret leakage | Secrets detection |
| PII exposure | PII redaction |
| Unauthorized access | Domain scoping |
| Audit trail gaps | Audit logging |
| Malformed input | Content validation |

### Out of Scope

- Multi-tenant isolation (single-user system)
- Encryption at rest (relies on filesystem)
- Network intrusion (local-first design)

## See Also

- [Configuration](../configuration/features.md) - Security feature flags
- [Audit Logging](../configuration/environment.md) - Audit configuration
