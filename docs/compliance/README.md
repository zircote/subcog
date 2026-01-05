# Compliance Documentation

This directory contains policy templates and procedures for SOC 2 / GDPR compliance. These documents should be customized for your organization's specific requirements.

## Document Index

| ID | Document | Purpose | Status |
|----|----------|---------|--------|
| COMP-H1 | [ACCESS_CONTROL_POLICY.md](ACCESS_CONTROL_POLICY.md) | Define access control policies | Template |
| COMP-H2 | [KEY_MANAGEMENT.md](KEY_MANAGEMENT.md) | Key management procedures | Template |
| COMP-H3 | [BACKUP_RECOVERY.md](BACKUP_RECOVERY.md) | Backup and recovery procedures | Template |
| COMP-H4 | [INCIDENT_RESPONSE.md](INCIDENT_RESPONSE.md) | Incident response plan | Template |
| COMP-H5 | [VENDOR_MANAGEMENT.md](VENDOR_MANAGEMENT.md) | Third-party vendor management | Template |
| COMP-H6 | [CHANGE_CONTROL.md](CHANGE_CONTROL.md) | Change control process | Template |
| COMP-H7 | [DATA_RETENTION.md](DATA_RETENTION.md) | Data retention policies | Template |
| COMP-H8 | [SESSION_MANAGEMENT.md](SESSION_MANAGEMENT.md) | Session management policies | Template |
| COMP-H9 | [INPUT_VALIDATION.md](INPUT_VALIDATION.md) | Input validation framework | Template |
| COMP-H10 | [SECURITY_AWARENESS.md](SECURITY_AWARENESS.md) | Security awareness training | Template |
| COMP-H11 | [MONITORING_ALERTING.md](MONITORING_ALERTING.md) | Monitoring and alerting | Template |
| COMP-H12 | [VULNERABILITY_MANAGEMENT.md](VULNERABILITY_MANAGEMENT.md) | Vulnerability management | Template |
| COMP-H13 | [DEPLOYMENT_PLAN.md](DEPLOYMENT_PLAN.md) | Production deployment plan | Template |

## Technical Controls Already Implemented

The following security controls are already implemented in the codebase:

### Authentication (SEC-H1)
- JWT authentication for MCP HTTP transport (`src/mcp/auth.rs`)
- Environment-based configuration: `SUBCOG_MCP_JWT_SECRET`, `SUBCOG_MCP_JWT_ISSUER`, `SUBCOG_MCP_JWT_AUDIENCE`
- HS256 algorithm with minimum 32-character secret

### Audit Logging (COMP-C5)
- Comprehensive audit logging (`src/security/audit.rs`)
- Events: capture, recall, deletion, access denied
- Configurable retention and output formats

### Data Deletion (COMP-C2)
- GDPR-compliant deletion via `PersistenceBackend::delete()` and `IndexBackend::remove()`

### Secret Detection (enabled via config)
- Pattern-based secret detection (`src/security/secrets.rs`)
- API keys, tokens, private keys, database URLs

### PII Detection (enabled via config)
- Pattern-based PII detection (`src/security/pii.rs`)
- Emails, phone numbers, SSNs, credit cards, IP addresses

### TLS for Database (COMP-C3)
- PostgreSQL TLS support via `postgres-tls` feature

## Implementation Notes

### For Organizations Adopting Subcog

1. **Review templates**: Each document is a starting point; customize for your organization
2. **Enable security features**: Set `features.secrets_filter` and `features.pii_filter` in config
3. **Configure audit logging**: Enable `features.audit_log` and set retention policy
4. **Use TLS**: Enable `postgres-tls` feature for PostgreSQL connections
5. **JWT for HTTP**: Configure JWT authentication for non-local deployments

### Compliance Roadmap

Items requiring architectural decisions before implementation:

| Item | Description | Dependency |
|------|-------------|------------|
| COMP-C1 | Encryption at rest | Key management system |
| COMP-C4 | Role-based access control | Authentication system |
| COMP-C6 | Data classification levels | Schema changes |
| COMP-C7 | Consent tracking | Schema changes |
