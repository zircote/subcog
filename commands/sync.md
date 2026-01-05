---
description: Sync memories with git remote (deprecated - SQLite is now authoritative)
allowed-tools: Bash
argument-hint: "[--push | --fetch | --full]"
---

# /subcog:sync

> **Deprecated**: With the migration to SQLite as authoritative storage, remote sync
> is no longer supported. This command returns no-op results for all operations.
> Memories are now stored locally in SQLite with project/branch/path faceting.

## Usage

```
/subcog:sync              # No-op (returns empty stats)
/subcog:sync --push       # No-op (returns empty stats)
/subcog:sync --fetch      # No-op (returns empty stats)
/subcog:sync --full       # No-op (returns empty stats)
```

## Arguments

<arguments>
1. **--push**: Previously pushed local memories to remote (now no-op)
2. **--fetch**: Previously fetched remote memories to local (now no-op)
3. **--full** (default): Previously did bidirectional sync (now no-op)
</arguments>

## Current Architecture

<strategy>
**Storage Model:**
- Memories are stored in SQLite at `~/.config/subcog/memories.db`
- Project isolation via facets (repo URL, branch name, working directory)
- No remote synchronization - SQLite is the authoritative store

**For Team Sharing:**
If you need to share memories across team members, consider:
- Using PostgreSQL backend with shared connection string
- Exporting/importing memories via `subcog export` / `subcog import`
</strategy>

## Examples

<examples>
**Check sync status (will show no-op):**
```
/subcog:sync
```
Output: `Sync completed: 0 pushed, 0 pulled, 0 conflicts`

**Alternative - Check database status:**
```
subcog status
```
Shows SQLite database path and memory count.
</examples>
