# Subcog Integration Guides

This directory contains integration guides for using Subcog with various AI coding assistants.

## Quick Start

| Platform | Guide | Features |
|----------|-------|----------|
| **Claude Code** | [claude-code-setup.md](claude-code-setup.md) | MCP server, hooks, CLAUDE.md |
| **OpenAI / ChatGPT** | [openai-setup.md](openai-setup.md) | CLI, Code Interpreter, GPT Actions |
| **Google Gemini** | [gemini-setup.md](gemini-setup.md) | Function calling, Vertex AI |
| **OpenCode** | [opencode-setup.md](opencode-setup.md) | MCP server, CLI |

### Copy-Paste Snippets

| File | Purpose |
|------|---------|
| [CLAUDE.md.snippet](CLAUDE.md.snippet) | Minimal protocol for CLAUDE.md |

---

## Platform Guides

### Claude Code

Full MCP integration with hooks for automatic context injection.

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

### OpenAI / ChatGPT

CLI-based integration with support for Code Interpreter and Custom GPTs.

- **Code Interpreter** - Python wrapper for subprocess calls
- **Custom GPTs** - REST API wrapper with OpenAPI schema
- **System Instructions** - Protocol guidance for prompts

See [openai-setup.md](openai-setup.md) for detailed instructions.

### Google Gemini

Function calling integration with Vertex AI support.

- **Function Calling** - Native Gemini tool declarations
- **AI Studio** - System instructions integration
- **Vertex AI** - Enterprise deployment patterns

See [gemini-setup.md](gemini-setup.md) for detailed instructions.

### OpenCode

MCP and CLI integration for the open-source assistant.

- **MCP Server** - Native Model Context Protocol support
- **CLI Commands** - Direct shell access
- **Configuration** - Project-level settings

See [opencode-setup.md](opencode-setup.md) for detailed instructions.

---

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
subcog serve --help
```

In Claude Code, call `mcp__subcog__subcog_status` to verify the MCP connection.

## See Also

- [Hooks Documentation](../hooks/README.md) - Hook configuration details
- [MCP Integration](../prompts/mcp.md) - MCP tool reference
- [Configuration](../configuration/README.md) - Full configuration options
