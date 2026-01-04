# subcog sync

Synchronize memories with git remote.

## Synopsis

```
subcog sync [OPTIONS] [DIRECTION]
```

## Description

The `sync` command synchronizes memories with a remote repository. Since memories are stored in SQLite with project/branch facets, sync exports memories to git and pushes/fetches from the remote.

## Arguments

| Argument | Description | Default |
|----------|-------------|---------|
| `[DIRECTION]` | Sync direction: `push`, `fetch`, `full` | `full` |

## Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--remote` | `-r` | Remote name | `origin` |
| `--force` | `-f` | Force push/fetch | `false` |
| `--dry-run` | | Show what would be synced | `false` |
| `--project` | `-p` | Project ID to sync | current project |

## Sync Directions

| Direction | Description |
|-----------|-------------|
| `push` | Export local memories and push to remote |
| `fetch` | Fetch remote and import memories |
| `full` | Fetch then push (default) |

## Examples

### Full Sync

```bash
subcog sync
```

Output:
```
Syncing with origin...
  Fetching from remote...
  ← 3 new memories imported
  Exporting to remote...
  → 5 memories exported
Sync complete: 3 fetched, 5 pushed
```

### Push Only

```bash
subcog sync push
```

### Fetch Only

```bash
subcog sync fetch
```

### Sync with Different Remote

```bash
subcog sync -r upstream
```

### Force Push

```bash
subcog sync push --force
```

### Dry Run

```bash
subcog sync --dry-run
```

Output:
```
Would sync with origin:
  Fetch: 3 memories would be imported
  Push: 5 memories would be exported
```

## How Sync Works

1. **Export**: Memories are exported from SQLite to a portable format
2. **Git operations**: Standard git push/fetch to remote
3. **Import**: Fetched memories are imported into SQLite with facets preserved

### Facet Preservation

During sync, memory facets are preserved:
- `project_id`: Project identifier (git remote URL)
- `branch`: Git branch name
- `file_path`: Source file path (if captured)

## Authentication

Sync uses standard Git authentication:
- SSH keys
- HTTPS credentials
- Git credential helpers

Ensure your remote is configured correctly:
```bash
git remote -v
```

## Performance

| Operation | Typical Time |
|-----------|--------------|
| Fetch (100 memories) | ~2s |
| Push (100 memories) | ~3s |
| Full sync (100 memories) | ~5s |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Sync successful |
| 1 | Sync failed |
| 5 | Network error |
| 6 | Authentication error |

## Troubleshooting

### Memories Not Syncing

Check local memory count:
```bash
subcog status
```

### Permission Denied

Check remote access:
```bash
git ls-remote origin
```

### Conflict Resolution

If conflicts occur during sync:
1. Fetch first to get remote changes
2. Review imported memories with `subcog recall`
3. Push your local changes

## See Also

- [status](./status.md) - Check sync status
- [gc](./gc.md) - Clean up stale branch memories
- [MCP subcog_sync](../mcp/tools.md#subcog_sync) - MCP equivalent
