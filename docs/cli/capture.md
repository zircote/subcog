# subcog capture

Capture a memory to persistent storage.

## Synopsis

```
subcog capture [OPTIONS] <CONTENT>
subcog capture [OPTIONS] --from-file <PATH>
subcog capture [OPTIONS] -
```

## Description

The `capture` command stores a new memory in the persistent storage layer (SQLite by default). Memories are categorized by namespace and can be tagged for easy retrieval.

## Arguments

| Argument | Description |
|----------|-------------|
| `<CONTENT>` | The memory content to capture |
| `-` | Read content from stdin |

## Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--namespace` | `-n` | Memory namespace | `learnings` |
| `--tags` | `-t` | Comma-separated tags | None |
| `--source` | `-s` | Source file reference | None |
| `--from-file` | `-f` | Read content from file | None |
| `--domain` | `-d` | Domain scope (project, user, org) | `project` |
| `--dry-run` | | Show what would be captured | `false` |

## Namespaces

| Namespace | Purpose |
|-----------|---------|
| `decisions` | Architectural and design decisions |
| `patterns` | Discovered patterns and conventions |
| `learnings` | Lessons learned from debugging |
| `context` | Important background information |
| `tech-debt` | Technical debt tracking |
| `blockers` | Blockers and impediments |
| `progress` | Work progress and milestones |
| `apis` | API documentation and contracts |
| `config` | Configuration details |
| `security` | Security findings and notes |
| `testing` | Test strategies and edge cases |

## Examples

### Basic Capture

```bash
# Capture a decision
subcog capture -n decisions "Use PostgreSQL for primary storage because of JSONB support"

# Capture a learning
subcog capture -n learnings "TIL: Rust's ? operator requires From trait implementation"
```

### Capture with Tags

```bash
subcog capture -n patterns -t "rust,error-handling" \
  "Always use Result types in library code, never unwrap"
```

### Capture with Source Reference

```bash
subcog capture -n decisions -s "src/storage/mod.rs" \
  "Implemented three-layer storage: persistence, index, vector"
```

### Capture from File

```bash
subcog capture -n context --from-file ARCHITECTURE.md
```

### Capture from Stdin

```bash
echo "Important context from meeting notes" | subcog capture -n context -
```

### Capture to User Domain

```bash
subcog capture -n learnings -d user \
  "Universal Rust tip: prefer borrowing over ownership"
```

### Dry Run

```bash
subcog capture --dry-run -n decisions "Test capture"
```

Output:
```
Would capture:
  Namespace: decisions
  Domain: project
  Content: Test capture
  Tags: []
  Source: None
```

## Output

On success, returns the memory ID and URN:

```json
{
  "id": "dc58d23a35876f5a59426e81aaa81d796efa7fc1",
  "urn": "subcog://project/decisions/dc58d23a35876f5a59426e81aaa81d796efa7fc1"
}
```

## Security

Content is scanned for:
- **Secrets**: API keys, tokens, passwords
- **PII**: Email addresses, phone numbers

If secrets are detected:
- With `--secrets-filter` enabled: Content is redacted
- Without filter: Capture is blocked with error

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Capture successful |
| 1 | Capture failed |
| 2 | Invalid arguments |
| 4 | Content blocked (secrets/PII detected) |

## See Also

- [recall](./recall.md) - Search memories
- [status](./status.md) - Check capture statistics
- [MCP subcog_capture](../mcp/tools.md#subcog_capture) - MCP equivalent
