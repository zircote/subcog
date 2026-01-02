# Access Control Policy (COMP-H1)

## Purpose
Define access control policies for the subcog memory system.

## Scope
All users and systems accessing subcog data.

## Policy

### 1. Authentication Requirements
- [ ] All HTTP transport connections require JWT authentication
- [ ] Minimum 32-character secrets for JWT signing
- [ ] Token expiration: [Define duration]

### 2. Authorization Levels
| Role | Capture | Recall | Delete | Admin |
|------|---------|--------|--------|-------|
| Reader | No | Yes | No | No |
| Writer | Yes | Yes | No | No |
| Admin | Yes | Yes | Yes | Yes |

### 3. Access Review
- Frequency: [Quarterly/Monthly]
- Responsible party: [Define]
- Documentation: [Location]

## Technical Implementation
- JWT claims include `scopes` field for role-based access
- See `src/mcp/auth.rs` for implementation

## Related Documents
- [KEY_MANAGEMENT.md](KEY_MANAGEMENT.md)
- [SESSION_MANAGEMENT.md](SESSION_MANAGEMENT.md)
