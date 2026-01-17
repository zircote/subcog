# Subcog Integration Guide for GitHub Copilot

This guide provides instructions for integrating Subcog with GitHub Copilot.

## Quick Start

### 1. Install Subcog

```bash
cargo install subcog
```

### 2. Configure MCP Server

GitHub Copilot supports MCP servers at repository and organization levels. See [GitHub Docs: Extending Copilot with MCP](https://docs.github.com/copilot/how-tos/agents/copilot-coding-agent/extending-copilot-coding-agent-with-mcp).

#### Repository-Level Configuration

Configure MCP servers in your repository settings on GitHub.com using JSON:

```json
{
  "mcpServers": {
    "subcog": {
      "command": "subcog",
      "args": ["serve"]
    }
  }
}
```

### 3. Create Custom Agent (Optional)

Create `.github/agents/memory.agent.md`:

```yaml
---
name: memory-agent
description: Agent with persistent memory via Subcog
tools: ['subcog/*']
mcp-servers:
  subcog:
    type: 'local'
    command: 'subcog'
    args: ['serve']
    tools: ["*"]
---

## Subcog Memory Protocol (MANDATORY)

At the start of EVERY session, call `subcog_init` to load memory context and best practices.

During the session:
- **Before implementing**: Recall relevant decisions and patterns
- **Capture immediately**: When decisions, patterns, learnings, or fixes are identified
- Access Subcog only via MCP tools, never shell commands
```

### 4. Add to AGENTS.md

For simpler integration, add to your repository's `AGENTS.md`:

```markdown
## Subcog Memory Protocol (MANDATORY)

At the start of EVERY session, call `subcog_init` to load memory context and best practices.

During the session:
- **Before implementing**: Recall relevant decisions and patterns
- **Capture immediately**: When decisions, patterns, learnings, or fixes are identified
- Access Subcog only via MCP tools, never shell commands
```

## Verification

Once configured, verify with:

```
subcog_status: {}
```

## See Also

- [GitHub Docs: MCP and Copilot Coding Agent](https://docs.github.com/en/copilot/concepts/agents/coding-agent/mcp-and-coding-agent)
- [GitHub Docs: Custom Agents Configuration](https://docs.github.com/en/copilot/reference/custom-agents-configuration)
- [Full Protocol Reference](../hooks/README.md)
- [Configuration](../configuration/README.md)
