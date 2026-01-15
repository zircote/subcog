---
adr_paths:
  - docs/adrs

default_format: custom-frontmatter
file_pattern: "adr_{id}.md"

numbering:
  pattern: "0000"
  start_from: 62
  padding: 4

statuses:
  workflow:
    - proposed
    - accepted
    - deprecated
    - superseded
  additional:
    - published
    - rejected
  allow_rejected: true

frontmatter:
  required:
    - title
    - description
    - status
    - created
  optional:
    - type
    - category
    - tags
    - updated
    - author
    - project
    - technologies
    - audience
    - confidence
    - completeness

sections:
  required:
    - Status
    - Context
    - Decision
    - Consequences
  optional:
    - Considered Options
    - Decision Outcome
    - Compliance Verification
    - Links

git:
  enabled: true
  auto_commit: false
  commit_template: "docs(adr): {action} ADR-{id} {title}"

index:
  file: docs/adrs/README.md
  format: compliance-table
  columns:
    - number
    - title
    - status
    - health
    - action_required
---

# Subcog ADR Configuration

Architecture Decision Records for the Subcog persistent memory system.

## Project Context

Subcog is a Rust-based persistent memory system for AI coding assistants. ADRs document key architectural decisions including:

- Storage architecture (three-layer: persistence, index, vector)
- Technology choices (fastembed, usearch, rmcp)
- API design (MCP tools, hooks)
- Performance and observability patterns

## Decision Process

1. **Propose**: Create ADR with status `proposed`
2. **Review**: Team discussion and refinement
3. **Accept**: Change status to `accepted` after consensus
4. **Implement**: Reference ADR in implementation PRs
5. **Verify**: Update compliance status in README.md

## Conventions

- **Numbering**: 4-digit sequential (0001, 0002, ...)
- **Naming**: `adr_NNNN.md` (underscore separator)
- **Location**: `docs/adrs/`
- **Index**: Compliance table in `docs/adrs/README.md`
- **Frontmatter**: Extended YAML with metadata fields

## Categories

- `architecture` - Core system design
- `api` - API and interface design
- `performance` - Performance optimization
- `observability` - Logging, tracing, metrics
- `security` - Security and compliance
- `storage` - Storage backends and strategies

## Tags

Common tags: `rust`, `mcp`, `sqlite`, `embeddings`, `hooks`, `performance`
