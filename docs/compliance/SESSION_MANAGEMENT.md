# Session Management (COMP-H8)

## Purpose
Define session security policies.

## JWT Token Configuration

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| Algorithm | HS256 | Symmetric, performant |
| Expiration | 1 hour | Balance security/usability |
| Secret Length | 32+ chars | Minimum entropy |

## Session Policies

### Token Lifecycle
1. Client authenticates
2. Server issues JWT with claims
3. Client includes token in Authorization header
4. Server validates on each request
5. Token expires, client re-authenticates

### Security Controls
- [ ] Token expiration enforced
- [ ] Issuer/audience validation (optional)
- [ ] No token storage in local storage (for web)
- [ ] HTTPS required for HTTP transport

### Revocation
Currently stateless (JWT). For revocation:
- Rotate JWT secret (invalidates all tokens)
- Future: implement token blacklist

## Technical Implementation
```bash
# Required
export SUBCOG_MCP_JWT_SECRET="your-32-char-or-longer-secret"

# Optional
export SUBCOG_MCP_JWT_ISSUER="https://auth.example.com"
export SUBCOG_MCP_JWT_AUDIENCE="subcog-api"
```

See `src/mcp/auth.rs` for implementation.
