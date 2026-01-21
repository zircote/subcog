# Troubleshooting Guide

This guide covers common issues and solutions when using subcog.

## Table of Contents

- [Installation Issues](#installation-issues)
- [CLI Issues](#cli-issues)
- [MCP Server Issues](#mcp-server-issues)
- [Storage Issues](#storage-issues)
- [Search Issues](#search-issues)
- [Hook Issues](#hook-issues)
- [Performance Issues](#performance-issues)
- [Debug Mode](#debug-mode)

---

## Installation Issues

### Binary not found after installation

**Symptom**: `subcog: command not found`

**Solutions**:

1. Ensure Cargo bin directory is in your PATH:
   ```bash
   export PATH="$HOME/.cargo/bin:$PATH"
   ```

2. Verify installation:
   ```bash
   ls -la ~/.cargo/bin/subcog
   ```

3. Reinstall:
   ```bash
   cargo install --path . --force
   ```

### Build fails with missing dependencies

**Symptom**: Compilation errors mentioning system libraries

**Solutions**:

For macOS:
```bash
xcode-select --install
```

For Ubuntu/Debian:
```bash
sudo apt-get install build-essential pkg-config libssl-dev
```

For Fedora:
```bash
sudo dnf install gcc pkg-config openssl-devel
```

---

## CLI Issues

### "No memories found" when memories exist

**Symptom**: `subcog recall` returns no results despite having captured memories

**Causes and Solutions**:

1. **Wrong project context**: Memories are project-scoped by default
   ```bash
   # Check current project
   subcog status

   # Search across all scopes
   subcog recall "query" --include-user
   ```

2. **Memories are tombstoned**: Branch was deleted
   ```bash
   # Include tombstoned memories
   subcog recall "query" --include-tombstoned
   ```

3. **Index out of sync**: Rebuild the index
   ```bash
   subcog migrate embeddings --force
   ```

### Capture fails with "content blocked"

**Symptom**: `Error: Content blocked - secrets detected`

**Cause**: Subcog detected potential secrets (API keys, passwords)

**Solutions**:

1. Remove the secret from content before capturing
2. Use environment variable references instead:
   ```bash
   # Instead of: "API key is sk-abc123..."
   subcog capture --namespace config "API key stored in OPENAI_API_KEY env var"
   ```

3. Skip security check (not recommended for production):
   ```bash
   subcog capture --namespace decisions --skip-security "Content with secret"
   ```

### Command hangs or times out

**Symptom**: CLI commands don't complete

**Solutions**:

1. Check git remote connectivity:
   ```bash
   git fetch --dry-run
   ```

2. Increase timeout for sync operations:
   ```bash
   export SUBCOG_SYNC_TIMEOUT_MS=60000
   subcog sync
   ```

3. Use offline mode:
   ```bash
   export SUBCOG_OFFLINE=true
   subcog capture --namespace decisions "My decision"
   ```

---

## MCP Server Issues

### Server fails to start

**Symptom**: `subcog serve` exits immediately or errors

**Solutions**:

1. Check if port is in use:
   ```bash
   lsof -i :3000
   ```

2. Use a different port:
   ```bash
   subcog serve --port 3001
   ```

3. Check logs for detailed error:
   ```bash
   RUST_LOG=debug subcog serve
   ```

### Claude Desktop doesn't see subcog tools

**Symptom**: MCP tools not appearing in Claude Desktop

**Solutions**:

1. Verify config location:
   ```bash
   # macOS
   cat ~/Library/Application\ Support/Claude/claude_desktop_config.json

   # Linux
   cat ~/.config/Claude/claude_desktop_config.json
   ```

2. Ensure correct configuration:
   ```json
   {
     "mcpServers": {
       "subcog": {
         "command": "npx",
         "args": ["-y", "@zircote/subcog", "serve"]
       }
     }
   }
   ```

3. Restart Claude Desktop completely (not just the window)

4. Check subcog is in PATH for Claude Desktop:
   ```json
   {
     "mcpServers": {
       "subcog": {
         "command": "/Users/you/.cargo/bin/subcog",
         "args": ["serve"]
       }
     }
   }
   ```

### Authentication errors with JWT

**Symptom**: `401 Unauthorized` from HTTP transport

**Solutions**:

1. Verify JWT secret is set:
   ```bash
   echo $SUBCOG_JWT_SECRET
   ```

2. Check token expiration (default: 1 hour)

3. Generate a new token with proper claims

---

## Storage Issues

### "Database is locked" errors

**Symptom**: `Error: database is locked`

**Cause**: Multiple processes accessing SQLite simultaneously

**Solutions**:

1. Close other subcog processes:
   ```bash
   pkill -f subcog
   ```

2. Check for stale lock files:
   ```bash
   ls -la .subcog/*.lock
   rm .subcog/*.lock  # If subcog is not running
   ```

3. Increase SQLite busy timeout:
   ```bash
   export SUBCOG_SQLITE_BUSY_TIMEOUT_MS=30000
   ```

### Storage path not writable

**Symptom**: `Permission denied` errors

**Solutions**:

1. Check directory permissions:
   ```bash
   ls -la .subcog/
   ```

2. Fix permissions:
   ```bash
   chmod 755 .subcog/
   chmod 644 .subcog/*.db
   ```

3. Use a different storage path:
   ```bash
   export SUBCOG_DATA_DIR=/tmp/subcog
   ```

### Corrupted database

**Symptom**: `database disk image is malformed`

**Solutions**:

1. Check database integrity:
   ```bash
   sqlite3 .subcog/index.db "PRAGMA integrity_check;"
   ```

2. Attempt recovery:
   ```bash
   sqlite3 .subcog/index.db ".recover" | sqlite3 .subcog/index_recovered.db
   mv .subcog/index.db .subcog/index.db.bak
   mv .subcog/index_recovered.db .subcog/index.db
   ```

3. Rebuild from scratch (memories in persistence layer are preserved):
   ```bash
   rm .subcog/index.db .subcog/vectors.usearch
   subcog migrate embeddings
   ```

---

## Search Issues

### Search returns irrelevant results

**Symptom**: High-scoring results don't match query

**Solutions**:

1. Use more specific queries:
   ```bash
   # Instead of: subcog recall "database"
   subcog recall "PostgreSQL connection pooling decision"
   ```

2. Filter by namespace:
   ```bash
   subcog recall "database" --namespace decisions
   ```

3. Use raw scores to debug:
   ```bash
   subcog recall "query" --raw
   ```

### Semantic search not working

**Symptom**: Only keyword matches returned, semantically similar content missed

**Causes and Solutions**:

1. **Embeddings not generated**: Migrate existing memories
   ```bash
   subcog migrate embeddings --dry-run
   subcog migrate embeddings
   ```

2. **Vector backend unavailable**: Check usearch initialization
   ```bash
   ls -la .subcog/vectors.usearch
   ```

3. **Model loading failed**: Check fastembed model cache
   ```bash
   ls -la ~/.cache/fastembed/
   # Clear and re-download if needed
   rm -rf ~/.cache/fastembed/
   subcog recall "test query"
   ```

### Search is slow

**Symptom**: Queries take >100ms

**Solutions**:

1. Reduce result limit:
   ```bash
   subcog recall "query" --limit 10
   ```

2. Check index size:
   ```bash
   subcog status
   ```

3. Rebuild index for optimization:
   ```bash
   subcog migrate embeddings --force
   ```

See [PERFORMANCE.md](PERFORMANCE.md) for detailed tuning.

---

## Hook Issues

### Hooks not triggering

**Symptom**: Claude Code hooks don't run

**Solutions**:

1. Verify hooks are configured in `~/.claude/settings.json`:
   ```json
   {
     "hooks": {
       "SessionStart": [{ "command": "subcog hook session-start" }]
     }
   }
   ```

2. Test hooks manually:
   ```bash
   echo '{"sessionId":"test"}' | subcog hook session-start
   ```

3. Check hook timeout settings:
   ```bash
   export SUBCOG_HOOK_TIMEOUT_MS=5000
   ```

### Hooks cause Claude Code to hang

**Symptom**: Claude Code becomes unresponsive after hook execution

**Solutions**:

1. Reduce hook timeout:
   ```toml
   # ~/.config/subcog/config.toml
   [hooks]
   session_start_timeout_ms = 2000
   user_prompt_timeout_ms = 50
   ```

2. Disable problematic hooks temporarily:
   ```json
   {
     "hooks": {
       "SessionStart": []
     }
   }
   ```

3. Check for infinite loops in hook output

### Pre-compact hook duplicates

**Symptom**: Same memory captured multiple times

**Cause**: Deduplication service not active

**Solutions**:

1. Enable deduplication:
   ```bash
   export SUBCOG_DEDUP_ENABLED=true
   ```

2. Adjust similarity threshold:
   ```bash
   export SUBCOG_DEDUP_DEFAULT_THRESHOLD=0.90
   ```

---

## Performance Issues

### High memory usage

**Symptom**: subcog process using >500MB RAM

**Solutions**:

1. Reduce embedding cache:
   ```bash
   export SUBCOG_EMBEDDING_CACHE_SIZE=100
   ```

2. Limit vector index in-memory size:
   ```bash
   export SUBCOG_VECTOR_MEMORY_LIMIT_MB=200
   ```

3. Use disk-backed mode for large indexes

### Slow startup

**Symptom**: CLI takes >100ms to start

**Solutions**:

1. Pre-warm the embedding model:
   ```bash
   subcog status  # First run loads model
   ```

2. Use lazy initialization:
   ```bash
   export SUBCOG_LAZY_INIT=true
   ```

3. Check disk I/O:
   ```bash
   iostat -x 1
   ```

---

## Debug Mode

### Enable verbose logging

```bash
# Full debug output
RUST_LOG=debug subcog recall "query"

# Component-specific logging
RUST_LOG=subcog::storage=debug,subcog::embedding=info subcog recall "query"

# Trace level (very verbose)
RUST_LOG=trace subcog recall "query" 2>&1 | head -100
```

### Enable tracing

```bash
# Start with OpenTelemetry tracing
export SUBCOG_OTLP_ENDPOINT=http://localhost:4317
subcog serve
```

### Collect diagnostics

```bash
# Generate diagnostic report
subcog status --verbose > diagnostics.txt
echo "---" >> diagnostics.txt
echo "Environment:" >> diagnostics.txt
env | grep SUBCOG >> diagnostics.txt
echo "---" >> diagnostics.txt
echo "Storage:" >> diagnostics.txt
ls -la .subcog/ >> diagnostics.txt
```

---

## Getting Help

If these solutions don't resolve your issue:

1. **Search existing issues**: [GitHub Issues](https://github.com/zircote/subcog/issues)

2. **File a bug report** with:
   - Subcog version (`subcog --version`)
   - Operating system and version
   - Steps to reproduce
   - Expected vs actual behavior
   - Diagnostic output (see above)

3. **Check the documentation**:
   - [README.md](../README.md)
   - [QUICKSTART.md](QUICKSTART.md)
   - [PERFORMANCE.md](PERFORMANCE.md)
   - [environment-variables.md](environment-variables.md)
