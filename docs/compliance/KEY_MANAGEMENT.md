# Key Management (COMP-H2)

## Purpose
Define procedures for cryptographic key management.

## Key Types

| Key Type | Purpose | Rotation | Storage |
|----------|---------|----------|---------|
| JWT Secret | Token signing | [90 days] | Environment variable |
| Database TLS | Connection encryption | [Annual] | Certificate store |
| API Keys | External service auth | [90 days] | Secret manager |

## Procedures

### Key Generation
- Use cryptographically secure random generators
- Minimum entropy: 256 bits for symmetric keys
- JWT secrets: minimum 32 characters

### Key Storage
- [ ] Never store keys in code or version control
- [ ] Use environment variables or secret managers
- [ ] Encrypt keys at rest when possible

### Key Rotation
1. Generate new key
2. Update secret manager
3. Graceful deployment with dual-key support
4. Revoke old key after grace period

### Key Revocation
- Immediate revocation for compromised keys
- Document incident in [INCIDENT_RESPONSE.md](INCIDENT_RESPONSE.md)

## Technical Implementation
- JWT configuration via `SUBCOG_MCP_JWT_SECRET`
- PostgreSQL TLS via `postgres-tls` feature
