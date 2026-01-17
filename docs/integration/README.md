# Subcog Integration Guides

This directory contains integration guides for using Subcog with various AI coding assistants.

## Quick Start

| File | Purpose |
|------|---------|
| [claude-code-setup.md](claude-code-setup.md) | Full setup guide for Claude Code |
| [CLAUDE.md.snippet](CLAUDE.md.snippet) | Copy-paste snippet for CLAUDE.md |

## Claude Code Integration

Subcog integrates with Claude Code through:

1. **MCP Server** - Provides memory tools via Model Context Protocol
2. **Hooks** - Automatic context injection and memory capture
3. **CLAUDE.md** - Protocol guidance for consistent memory usage

### Installation Steps

1. Install Subcog: `cargo install subcog`
2. Configure MCP server in `~/.claude/settings.json`
3. (Optional) Set up hooks in `.claude/hooks.json`
4. Add protocol guidance to your `CLAUDE.md`

See [claude-code-setup.md](claude-code-setup.md) for detailed instructions.

## Adding to Your Project

### Option 1: Full Integration (Recommended)

Copy the MCP and hooks configuration from [claude-code-setup.md](claude-code-setup.md), then add the Subcog Memory Protocol section to your `CLAUDE.md`.

### Option 2: Minimal Integration

Copy the contents of [CLAUDE.md.snippet](CLAUDE.md.snippet) directly to your:
- Project `CLAUDE.md` for project-specific memory
- Global `~/.claude/CLAUDE.md` for cross-project memory

## Verification

After setup, verify integration by:

```bash
# Check Subcog is installed
subcog --version

# Check system status
subcog status

# Test MCP server
subcog mcp-server --help
```

In Claude Code, call `mcp__subcog__subcog_status` to verify the MCP connection.

## See Also

- [Hooks Documentation](../hooks/README.md) - Hook configuration details
- [MCP Integration](../prompts/mcp.md) - MCP tool reference
- [Configuration](../configuration/README.md) - Full configuration options
