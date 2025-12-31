# stop Hook

Analyzes the session and synchronizes memories when a Claude Code session ends.

## Synopsis

```bash
subcog hook stop [OPTIONS]
```

## Description

The stop hook runs when a Claude Code session ends. It:
1. Analyzes session activity
2. Syncs memories with git remote
3. Returns a summary of the session

## Options

| Option | Description | Default |
|--------|-------------|---------|
| `--sync` | Sync after analysis | `true` |
| `--summary` | Generate session summary | `true` |

## Response Format

```json
{
  "hookSpecificOutput": {
    "hookEventName": "Stop",
    "additionalContext": "Session summary:\n- Captures: 5\n- Recalls: 12\n- Synced: 5 memories pushed"
  }
}
```

## Session Analysis

The hook analyzes:

### Capture Activity
- Number of memories captured
- Breakdown by namespace
- Most used tags

### Recall Activity
- Number of searches performed
- Most common query patterns
- Search modes used

### Memory Access
- Resources read
- Topics explored
- Time spent per namespace

## Sync Behavior

If `--sync` is enabled (default):

1. **Push local changes** - New memories pushed to remote
2. **Report sync status** - Number pushed, any conflicts
3. **Handle errors** - Report but don't fail session end

Sync can be disabled for offline work:

```bash
subcog hook stop --sync=false
```

## Example Output

```bash
$ subcog hook stop 2>/dev/null | jq
```

```json
{
  "hookSpecificOutput": {
    "hookEventName": "Stop",
    "additionalContext": "## Session Summary\n\n### Activity\n- **Duration**: 45 minutes\n- **Captures**: 5 memories\n- **Recalls**: 12 searches\n\n### Memories Captured\n| Namespace | Count |\n|-----------|-------|\n| decisions | 2 |\n| learnings | 2 |\n| patterns | 1 |\n\n### Sync Status\n- Pushed: 5 memories\n- Fetched: 0 memories\n- Status: Success"
  }
}
```

## Configuration

### hooks.json

```json
{
  "matcher": { "event": "stop" },
  "hooks": [{
    "type": "command",
    "command": "subcog hook stop"
  }]
}
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBCOG_STOP_SYNC` | Enable sync on stop | `true` |
| `SUBCOG_STOP_SUMMARY` | Generate summary | `true` |

## Summary Content

The session summary includes:

### Session Metadata
```markdown
## Session Summary

**Session ID**: fea5608c-85f8-4a89-b155-a830dfcda507
**Duration**: 45 minutes
**Working Directory**: /path/to/project
```

### Activity Metrics
```markdown
### Activity
- **Captures**: 5 memories
- **Recalls**: 12 searches
- **Resources Read**: 8
```

### Namespace Breakdown
```markdown
### By Namespace
| Namespace | Captured | Recalled |
|-----------|----------|----------|
| decisions | 2 | 4 |
| patterns | 1 | 3 |
| learnings | 2 | 5 |
```

### Sync Status
```markdown
### Sync Status
- **Pushed**: 5 memories to origin
- **Fetched**: 0 memories
- **Conflicts**: None
```

## Error Handling

The hook is designed to never fail the session end:

| Error | Behavior |
|-------|----------|
| Sync fails | Reports error, continues |
| Analysis fails | Returns minimal summary |
| Git unavailable | Skips sync, reports status |

## Performance

- Target: <200ms
- Typical: ~150ms
- With sync: ~500ms (depends on remote)

## Offline Mode

When working offline:

```bash
SUBCOG_STOP_SYNC=false subcog hook stop
```

Or configure in hooks.json:

```json
{
  "matcher": { "event": "stop" },
  "hooks": [{
    "type": "command",
    "command": "subcog hook stop --sync=false"
  }]
}
```

Memories accumulate locally and sync on next connected session.

## See Also

- [session-start](session-start.md) - Session start hook
- [sync](../cli/sync.md) - Manual sync command
- [status](../cli/status.md) - Check memory statistics
