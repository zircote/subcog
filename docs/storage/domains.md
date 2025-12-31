# Domain Scoping

Subcog supports scoping memories to different domains for organization and access control.

## Domain Types

| Domain | Scope | Use Case |
|--------|-------|----------|
| `project` | Current repository | Repo-specific decisions |
| `user` | User-wide (global) | Personal learnings |
| `org` | Organization-level | Cross-repo patterns |

## Project Domain

Default scope for most memories.

### Identification

Project domain is derived from:
1. Git remote URL: `origin` → `zircote/subcog`
2. Directory name if no remote

### Storage

```
refs/notes/subcog/project/zircote-subcog/...
```

### Use Cases

- Repository-specific decisions
- Project architecture patterns
- Local configuration details
- API contracts for this project

### Example

```bash
subcog capture -n decisions "Use PostgreSQL for this project"
# Stored in: project domain (zircote/subcog)
```

---

## User Domain

User-wide memories that apply across projects.

### Identification

Uses the system username or configured identity.

### Storage

```
~/.subcog/global/...
# or
refs/notes/subcog/global/...
```

### Use Cases

- Personal learnings
- Universal patterns
- Preferred tools and configurations
- Career learnings

### Example

```bash
subcog capture -n learnings -d user "TIL: Always use Result in Rust"
# Stored in: user domain (global)
```

---

## Organization Domain

Organization-level memories shared across repositories.

### Identification

Derived from:
1. Git remote URL organization: `zircote/subcog` → `zircote`
2. Configured organization name

### Storage

```
refs/notes/subcog/org/zircote/...
```

### Use Cases

- Company coding standards
- Cross-repo patterns
- Shared API conventions
- Team decisions

### Example

```bash
subcog capture -n patterns -d org "Always use kebab-case for URLs"
# Stored in: org domain (zircote)
```

---

## Domain Resolution

When searching, domains are resolved in order:

1. **Project** (most specific)
2. **User** (personal)
3. **Org** (team)

Higher priority domains override lower for same-key items.

## Cross-Domain Queries

Use `_` wildcard for cross-domain queries:

### MCP Resources

```
subcog://_                    # All memories, all domains
subcog://_/decisions          # All decisions, all domains
```

### CLI

```bash
subcog recall --filter "ns:decisions" "storage"
# Searches project first, then user, then org
```

### Filter Syntax

Not yet supported in filter syntax (planned).

## Multi-Domain Configuration

Enable multi-domain support:

```yaml
features:
  multi_domain: true
```

When disabled:
- Only project domain accessible
- Simpler storage model
- Lower overhead

## URN Format

Domain is part of the canonical URN:

```
urn:subcog:{domain}:{namespace}:{id}
```

Examples:
```
urn:subcog:project:decisions:dc58d23a...
urn:subcog:global:learnings:1314b968...
urn:subcog:zircote:patterns:a1b2c3d4...
```

## Domain Inheritance

Memories can reference or link across domains:

```yaml
---
id: abc123
namespace: decisions
domain: project
references:
  - urn:subcog:zircote:patterns:def456
---
Follows organization pattern for error handling
```

## Prompt Template Domains

Prompts also support domain scoping:

| Domain | Use Case |
|--------|----------|
| `project` | Project-specific templates |
| `user` | Personal templates |
| `org` | Shared team templates |

```bash
subcog prompt save my-template --domain user
subcog prompt list --domain org
```

## Best Practices

1. **Use project for repo-specific context**
2. **Use user for personal learnings**
3. **Use org for team standards**
4. **Enable multi-domain only when needed**
5. **Reference org patterns from project decisions**

## See Also

- [URN Guide](../URN-GUIDE.md) - Complete URN documentation
- [Configuration](../configuration/README.md) - Domain configuration
- [MCP Resources](../mcp/resources.md) - Domain-scoped resources
