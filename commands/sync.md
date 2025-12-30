---
description: Sync memories with git remote (push, fetch, or full sync)
allowed-tools: Bash
argument-hint: "[--push | --fetch | --full]"
---

# /subcog:sync

Synchronize memories with the configured git remote repository.

## Usage

```
/subcog:sync              # Full sync (fetch + push)
/subcog:sync --push       # Push local changes to remote
/subcog:sync --fetch      # Fetch remote changes to local
/subcog:sync --full       # Explicit full sync
```

## Arguments

<arguments>
1. **--push**: Push local memories to remote (upload only)
2. **--fetch**: Fetch remote memories to local (download only)
3. **--full** (default): Bidirectional sync (fetch then push)
</arguments>

## Execution Strategy

<strategy>
**CLI Execution:**
Uses the `subcog sync` CLI command:
```bash
subcog sync          # Full sync
subcog sync --push   # Push only
subcog sync --fetch  # Fetch only
```

**Note:** Sync operates on git notes, which are stored in `refs/notes/subcog`.
</strategy>

## Examples

<examples>
**Full sync (recommended):**
```
/subcog:sync
```

**Push only (after capturing new memories):**
```
/subcog:sync --push
```

**Fetch only (get team's memories):**
```
/subcog:sync --fetch
```
</examples>
