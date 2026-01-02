# subcog sync

Synchronize memories with git remote.

## Synopsis

```
subcog sync [OPTIONS] [DIRECTION]
```

## Description

The `sync` command synchronizes memories stored in Git Notes with a remote repository. This enables team collaboration and backup of memories across machines.

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

## Sync Directions

| Direction | Description |
|-----------|-------------|
| `push` | Push local notes to remote |
| `fetch` | Fetch remote notes to local |
| `full` | Fetch then push (default) |

## Examples

### Full Sync

```bash
subcog sync
```

Output:
```
Syncing with origin...
  Fetching refs/notes/subcog...
  ← 3 new memories fetched
  Pushing refs/notes/subcog...
  → 5 memories pushed
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
  Fetch: 3 memories would be fetched
  Push: 5 memories would be pushed
```

## Git Notes Reference

Subcog stores memories in Git Notes under:
- `refs/notes/subcog` - Main memory storage
- `refs/notes/_prompts` - Prompt templates

The sync command handles both references automatically.

## Conflict Resolution

When the same memory is modified locally and remotely:

1. **Fetch-only**: Remote version overwrites local
2. **Push-only**: Local version overwrites remote
3. **Full sync**: Remote is fetched first, then local is pushed

For true conflict resolution, use:
```bash
git notes --ref=subcog merge origin/refs/notes/subcog
```

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

### Notes Not Syncing

Ensure the notes ref exists:
```bash
git notes --ref=subcog list
```

### Permission Denied

Check remote access:
```bash
git ls-remote origin refs/notes/subcog
```

### Merge Conflicts

If conflicts occur during fetch:
```bash
# View conflicting notes
git notes --ref=subcog merge --abort

# Manual merge
git notes --ref=subcog merge origin/refs/notes/subcog
```

## See Also

- [status](./status.md) - Check sync status
- [MCP subcog_sync](../mcp/tools.md#subcog_sync) - MCP equivalent
