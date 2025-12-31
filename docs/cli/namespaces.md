# subcog namespaces

List available memory namespaces.

## Synopsis

```
subcog namespaces [OPTIONS]
```

## Description

The `namespaces` command lists all available memory namespaces with their descriptions and signal words. This helps users choose the appropriate namespace when capturing memories.

## Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--format` | `-f` | Output format (table, json, yaml) | `table` |
| `--verbose` | `-v` | Show signal words | `false` |

## Examples

### Basic List

```bash
subcog namespaces
```

Output:
```
NAMESPACE     DESCRIPTION
decisions     Architectural and design decisions
patterns      Discovered patterns and conventions
learnings     Lessons learned from debugging
context       Important background information
tech-debt     Technical debt tracking
blockers      Blockers and impediments
progress      Work progress and milestones
apis          API documentation and contracts
config        Configuration details
security      Security findings and notes
testing       Test strategies and edge cases
```

### With Signal Words

```bash
subcog namespaces -v
```

Output:
```
NAMESPACE     DESCRIPTION                          SIGNAL WORDS
decisions     Architectural and design decisions   decided, chose, going with
patterns      Discovered patterns and conventions  always, never, convention
learnings     Lessons learned from debugging       TIL, learned, discovered
context       Important background information     because, constraint, requirement
tech-debt     Technical debt tracking              TODO, FIXME, temporary, hack
blockers      Blockers and impediments             blocked, waiting, depends on
progress      Work progress and milestones         completed, milestone, shipped
apis          API documentation and contracts      endpoint, request, response
config        Configuration details                environment, setting, variable
security      Security findings and notes          vulnerability, CVE, auth
testing       Test strategies and edge cases       test, edge case, coverage
```

### JSON Output

```bash
subcog namespaces --format json
```

Output:
```json
[
  {
    "namespace": "decisions",
    "description": "Architectural and design decisions",
    "signal_words": ["decided", "chose", "going with"]
  },
  {
    "namespace": "patterns",
    "description": "Discovered patterns and conventions",
    "signal_words": ["always", "never", "convention"]
  },
  ...
]
```

## Namespace Reference

### decisions

**Purpose**: Record architectural and design decisions

**Use when**: Making technology choices, defining patterns, choosing approaches

**Examples**:
- "Decided to use PostgreSQL for primary storage"
- "Going with REST over GraphQL for this API"
- "Chose builder pattern for configuration"

### patterns

**Purpose**: Document discovered patterns and conventions

**Use when**: Identifying recurring approaches, establishing conventions

**Examples**:
- "Always use Result types in library code"
- "Never store secrets in environment variables"
- "Convention: prefix all test files with test_"

### learnings

**Purpose**: Capture lessons learned from debugging

**Use when**: Discovering something new, debugging issues

**Examples**:
- "TIL: async closures need explicit lifetime annotations"
- "Learned that clippy's pedantic lints catch subtle bugs"
- "Discovered the root cause was a race condition"

### context

**Purpose**: Store important background information

**Use when**: Recording constraints, requirements, business logic

**Examples**:
- "We can't use library X because of licensing"
- "Constraint: must support offline mode"
- "Requirement: 99.9% uptime SLA"

### tech-debt

**Purpose**: Track technical debt items

**Use when**: Noting shortcuts, temporary solutions, future improvements

**Examples**:
- "TODO: refactor this after MVP"
- "FIXME: this is a temporary workaround"
- "Hack: using sleep instead of proper sync"

### blockers

**Purpose**: Record blockers and impediments

**Use when**: Documenting what's blocking progress

**Examples**:
- "Blocked by missing API documentation"
- "Waiting on security review"
- "Depends on infrastructure team deployment"

### progress

**Purpose**: Track work progress and milestones

**Use when**: Recording completed work, achievements

**Examples**:
- "Completed the authentication module"
- "Milestone: first successful deployment"
- "Shipped v2.0 to production"

### apis

**Purpose**: Document API contracts and specifications

**Use when**: Defining endpoints, request/response formats

**Examples**:
- "POST /api/users expects { name, email }"
- "Response includes pagination headers"
- "API uses OAuth 2.0 for authentication"

### config

**Purpose**: Record configuration details

**Use when**: Documenting settings, environment variables

**Examples**:
- "DATABASE_URL must be set in production"
- "Setting LOG_LEVEL=debug for troubleshooting"
- "Variable FEATURE_FLAGS controls feature rollout"

### security

**Purpose**: Track security findings and notes

**Use when**: Documenting vulnerabilities, security measures

**Examples**:
- "Vulnerability found in dependency X (CVE-2024-1234)"
- "Auth tokens expire after 24 hours"
- "Input validation added for SQL injection prevention"

### testing

**Purpose**: Document test strategies and edge cases

**Use when**: Recording test approaches, edge cases to cover

**Examples**:
- "Test with empty input, null values, unicode"
- "Edge case: concurrent writes to same record"
- "Coverage goal: 80% for critical paths"

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Error |

## See Also

- [capture](capture.md) - Capture memories with namespaces
- [recall](recall.md) - Search by namespace
- [MCP subcog_namespaces](../mcp/tools.md#subcog_namespaces) - MCP equivalent
