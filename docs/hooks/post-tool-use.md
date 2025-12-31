# post-tool-use Hook

Surfaces related memories after a tool is executed.

## Synopsis

```bash
subcog hook post-tool-use [OPTIONS]
```

## Description

The post-tool-use hook runs after Claude executes a tool. It:
1. Analyzes the tool name and result
2. Searches for related memories
3. Returns context about related past experiences

## Options

| Option | Description | Default |
|--------|-------------|---------|
| `--tool` | Name of the tool that was used | None |
| `--result` | Tool result (via stdin) | None |
| `--max-memories` | Maximum memories to surface | `5` |

## Response Format

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PostToolUse",
    "additionalContext": "## Related Memories\n\n..."
  }
}
```

## Tool Analysis

The hook analyzes tool usage to find relevant context:

### File Operations
When Read, Write, or Edit tools are used, searches for:
- Decisions about the file or module
- Patterns for the file type
- Previous issues with the file

### Search Operations
When Grep or Glob tools are used, surfaces:
- Related search patterns
- Tips for the searched content

### Git Operations
When git commands are executed, finds:
- Decisions about branching strategy
- Commit message conventions
- Previous issues with similar commits

### Test Operations
When test commands run, surfaces:
- Testing patterns
- Edge cases for the tested code
- Previous test failures

## Memory Selection

Memories are selected based on:
1. **Tool type** - Different tools trigger different searches
2. **Result content** - Analyzes result for relevant keywords
3. **File paths** - Matches against source references
4. **Recent context** - Prioritizes recent session activity

## Example Output

```bash
$ echo '{"file": "src/auth.rs", "result": "function authenticate()"}' | \
    subcog hook post-tool-use --tool Read 2>/dev/null | jq
```

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PostToolUse",
    "additionalContext": "## Related Memories\n\n### For src/auth.rs:\n\n**decisions**\n- Use JWT for session tokens (id: abc123)\n- Implement rate limiting on auth endpoints (id: def456)\n\n**learnings**\n- Token expiry should be checked server-side (id: ghi789)"
  }
}
```

## Configuration

### hooks.json

```json
{
  "matcher": { "event": "post_tool_use" },
  "hooks": [{
    "type": "command",
    "command": "subcog hook post-tool-use"
  }]
}
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBCOG_POST_TOOL_ENABLED` | Enable hook | `true` |
| `SUBCOG_POST_TOOL_MAX_MEMORIES` | Max memories | `5` |

## Tool-Specific Behavior

### Read Tool
```markdown
## Related Memories

### For {file_path}:
[Memories with matching source reference]

### For {module}:
[Memories about the module/directory]
```

### Edit Tool
```markdown
## Related Memories

### Before modifying {file_path}:
[Decisions about the file]
[Patterns to follow]
[Known issues]
```

### Bash Tool (git)
```markdown
## Related Memories

### Git Operations:
[Branching conventions]
[Commit message patterns]
```

### Bash Tool (tests)
```markdown
## Related Memories

### Testing:
[Test patterns]
[Edge cases]
[Previous failures]
```

## Performance

- Target: <50ms
- Typical: ~20ms
- Skipped if no relevant context found

## When to Skip

The hook returns empty context when:
- Tool is simple (ls, pwd)
- No relevant memories found
- Result is empty or error

## See Also

- [user-prompt-submit](user-prompt-submit.md) - Pre-prompt hook
- [pre-compact](pre-compact.md) - Pre-compaction hook
