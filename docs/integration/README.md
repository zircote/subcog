# Subcog Integration Guides

This directory contains integration guides for using Subcog with various AI coding assistants.

## Quick Start

| Platform | Guide | Features |
|----------|-------|----------|
| **Claude Code** | [claude-code-setup.md](claude-code-setup.md) | MCP server, hooks, CLAUDE.md |
| **GitHub Copilot** | [copilot-setup.md](copilot-setup.md) | MCP server, custom agents, AGENTS.md |
| **OpenAI** | [openai-setup.md](openai-setup.md) | MCP server |
| **Google Gemini** | [gemini-setup.md](gemini-setup.md) | MCP server |
| **OpenCode** | [opencode-setup.md](opencode-setup.md) | MCP server, TypeScript plugins |

### Copy-Paste Snippets

| File | Purpose |
|------|---------|
| [CLAUDE.md.snippet](CLAUDE.md.snippet) | Minimal protocol for CLAUDE.md or AGENTS.md |

---

## Platform Guides

### Claude Code

Full MCP integration with hooks for automatic context injection.

- **MCP Server** - Provides memory tools via Model Context Protocol
- **Hooks** - Automatic context injection and memory capture
- **CLAUDE.md** - Protocol guidance for consistent memory usage

See [claude-code-setup.md](claude-code-setup.md) for detailed instructions.

### GitHub Copilot

MCP integration with custom agent profiles.

- **MCP Server** - Native Model Context Protocol support
- **Custom Agents** - `.github/agents/*.agent.md` profiles
- **AGENTS.md** - Protocol guidance for Copilot

See [copilot-setup.md](copilot-setup.md) for detailed instructions.

### OpenAI

MCP integration for OpenAI-based clients.

- **MCP Server** - Native Model Context Protocol support

See [openai-setup.md](openai-setup.md) for detailed instructions.

### Google Gemini

MCP integration for Gemini clients.

- **MCP Server** - Native Model Context Protocol support

See [gemini-setup.md](gemini-setup.md) for detailed instructions.

### OpenCode

MCP integration with TypeScript plugin system.

- **MCP Server** - Native Model Context Protocol support
- **TypeScript Plugins** - Hook into session and tool events

See [opencode-setup.md](opencode-setup.md) for detailed instructions.

---

## Adding to Your Project

### Option 1: Full Integration (Recommended)

Copy the MCP configuration from your platform's guide, then add the Subcog Memory Protocol section to your `CLAUDE.md`, `AGENTS.md`, or system prompt.

### Option 2: Minimal Integration

Copy the contents of [CLAUDE.md.snippet](CLAUDE.md.snippet) directly to your:
- Project `CLAUDE.md` for Claude Code
- `AGENTS.md` for GitHub Copilot
- System prompt for other platforms

## Verification

After setup, verify integration by:

```bash
# Check Subcog is installed
subcog --version

# Check system status
subcog status

# Test MCP server
subcog serve --help
```

In your AI assistant, call `subcog_status` to verify the MCP connection.

## See Also

- [Hooks Documentation](../hooks/README.md) - Hook configuration details
- [MCP Integration](../prompts/mcp.md) - MCP tool reference
- [Configuration](../configuration/README.md) - Full configuration options
