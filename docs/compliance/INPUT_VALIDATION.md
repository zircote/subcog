# Input Validation Framework (COMP-H9)

## Purpose
Define input validation requirements.

## Validation Layers

| Layer | Location | Purpose |
|-------|----------|---------|
| CLI | `clap` | Argument parsing |
| MCP | `serde` + custom | JSON-RPC validation |
| Storage | Backend traits | Sanitization |
| Security | Redactor | PII/secret detection |

## Validation Rules

### Content
- Maximum size: 1MB (`MAX_FILE_SIZE`)
- YAML front matter: 64KB (`MAX_FRONT_MATTER_SIZE`)
- UTF-8 encoding required

### Identifiers
- Memory IDs: UUID v4 format
- Namespace: Enum validation
- Tags: String array, no SQL injection

### Paths
- Path traversal protection (`is_safe_filename()`)
- Base path validation (`starts_with()`)
- No shell metacharacters

### Database
- Table name whitelist (`ALLOWED_TABLE_NAMES`)
- Prepared statements for all queries
- No string interpolation in SQL

## Security Filters

### Secret Detection
Patterns detected (`src/security/secrets.rs`):
- AWS keys, API keys, tokens
- Private keys, certificates
- Database URLs, connection strings

### PII Detection
Patterns detected (`src/security/pii.rs`):
- Email addresses
- Phone numbers
- SSNs, credit cards
- IP addresses

## Implementation
Enable via configuration:
```yaml
features:
  secrets_filter: true
  pii_filter: true
```
