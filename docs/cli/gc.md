# subcog gc

Garbage collection for stale branch memories.

## Synopsis

```
subcog gc [OPTIONS]
```

## Description

The `gc` command identifies and tombstones memories associated with branches that no longer exist. This helps keep your memory database clean by marking obsolete branch-specific context as inactive.

## Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--dry-run` | `-n` | Show what would be tombstoned without making changes | `false` |
| `--branch` | `-b` | Target a specific branch for cleanup | all stale |
| `--purge` | | Permanently delete tombstoned memories | `false` |
| `--older-than` | | Only purge tombstones older than duration (e.g., 30d) | none |
| `--project` | `-p` | Project ID to GC | current project |

## How It Works

1. **Branch Detection**: Reads distinct branches from the memory database
2. **Existence Check**: Verifies each branch exists in the git repository
3. **Tombstoning**: Marks memories from deleted branches with `tombstoned_at` timestamp
4. **Optional Purge**: Permanently removes old tombstoned memories

### Tombstone Pattern

Memories are soft-deleted by setting:
- `status = "tombstoned"`
- `tombstoned_at = <current_timestamp>`

Tombstoned memories are excluded from search by default but remain in the database for audit purposes.

## Examples

### Dry Run (Preview)

```bash
subcog gc --dry-run
```

Output:
```
Garbage Collection Preview
══════════════════════════

Project: github.com/zircote/subcog

Stale branches detected: 3

  feature/old-auth (12 memories)
  bugfix/issue-42 (5 memories)
  experiment/prototype (8 memories)

Total: 25 memories would be tombstoned

Run without --dry-run to apply changes.
```

### Run GC

```bash
subcog gc
```

Output:
```
Garbage Collection
══════════════════

Project: github.com/zircote/subcog

Tombstoned 25 memories from 3 stale branches:
  ✓ feature/old-auth (12 memories)
  ✓ bugfix/issue-42 (5 memories)
  ✓ experiment/prototype (8 memories)

Use --purge to permanently delete tombstoned memories.
```

### GC Specific Branch

```bash
subcog gc --branch feature/old-auth
```

### Purge Old Tombstones

```bash
subcog gc --purge --older-than 30d
```

Output:
```
Purging tombstoned memories older than 30 days...

Permanently deleted 15 memories:
  decisions: 3
  patterns: 2
  learnings: 8
  context: 2
```

### Purge All Tombstones

```bash
subcog gc --purge
```

## Lazy GC

In addition to manual GC, Subcog performs lazy garbage collection:

- **During search**: Branches are checked for existence when memories are returned
- **Auto-tombstone**: If a memory's branch no longer exists, it's tombstoned on access
- **Transparent**: Users don't see stale branch memories in search results

This means manual `subcog gc` is typically only needed for bulk cleanup.

## Including Tombstoned Memories

To include tombstoned memories in search:

```bash
subcog recall "old decision" --include-tombstoned
```

Or via MCP:
```json
{
  "filter": "include_tombstoned:true"
}
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | GC completed successfully |
| 1 | GC failed |
| 3 | Configuration error |
| 4 | Storage error |

## Safety

- **Dry-run by default**: Use `--dry-run` first to preview changes
- **Soft delete**: Memories are tombstoned, not deleted
- **Purge requires flag**: Permanent deletion requires explicit `--purge`
- **Time-based purge**: Use `--older-than` to only purge old tombstones

## See Also

- [status](./status.md) - View memory statistics
- [recall](./recall.md) - Search memories
- [Storage Architecture](../storage/README.md) - Storage layer details
- [MCP subcog_gc](../mcp/tools.md#subcog_gc) - MCP equivalent
