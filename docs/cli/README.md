# CLI Reference

Subcog provides a comprehensive command-line interface for managing memories, running the MCP server, and handling Claude Code hooks.

## Command Overview

| Command | Description |
|---------|-------------|
| [capture](./capture.md) | Capture a memory to persistent storage |
| [recall](./recall.md) | Search and retrieve memories |
| [status](./status.md) | Display system status and statistics |
| [consolidate](consolidate.md) | Merge and deduplicate similar memories |
| [config](config.md) | Manage configuration settings |
| [serve](serve.md) | Run the MCP server |
| [hook](hook.md) | Handle Claude Code hook events |
| [prompt](prompt.md) | Manage prompt templates |
| [namespaces](./namespaces.md) | List available memory namespaces |

## Global Options

These options are available for all commands:

```
OPTIONS:
    -c, --config <PATH>     Path to configuration file
    -v, --verbose           Increase verbosity (-v, -vv, -vvv)
    -q, --quiet             Suppress output
        --json              Output in JSON format
    -h, --help              Print help information
    -V, --version           Print version information
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Invalid arguments |
| 3 | Configuration error |
| 4 | Storage error |
| 5 | Network error |

## Environment Variables

All commands respect these environment variables:

| Variable | Description |
|----------|-------------|
| `SUBCOG_CONFIG_PATH` | Override config file location |
| `SUBCOG_LOG_LEVEL` | Set log level (trace, debug, info, warn, error) |
| `SUBCOG_DOMAIN` | Override domain scope |
| `SUBCOG_GIT_DIR` | Set git directory path |
| `NO_COLOR` | Disable colored output |

## Piping and Redirection

All commands support standard Unix piping:

```bash
# Capture from stdin
echo "Important decision" | subcog capture --namespace decisions -

# Output to file
subcog recall --json "pattern" > results.json

# Combine with other tools
subcog recall --format json "api" | jq '.[] | .content'
```

## Shell Completion

Generate shell completion scripts:

```bash
# Bash
subcog completions bash > /etc/bash_completion.d/subcog

# Zsh
subcog completions zsh > ~/.zfunc/_subcog

# Fish
subcog completions fish > ~/.config/fish/completions/subcog.fish

# PowerShell
subcog completions powershell > subcog.ps1
```

## Command Categories

### Memory Operations
- [capture](./capture.md) - Store new memories
- [recall](./recall.md) - Search existing memories
- [consolidate](consolidate.md) - Merge similar memories

### System Operations
- [status](./status.md) - Check system health
- [config](config.md) - Manage configuration

### Integration
- [serve](serve.md) - MCP server
- [hook](hook.md) - Claude Code hooks

### Utilities
- [prompt](prompt.md) - Prompt templates
- [namespaces](./namespaces.md) - List namespaces
