# Subcog Integration Guide for Google Gemini

This guide provides instructions for integrating Subcog with Gemini clients that support MCP.

## Quick Start

### 1. Install Subcog

```bash
cargo install subcog
```

### 2. Configure MCP Server

Add to your MCP configuration:

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

### 3. Add Protocol to System Prompt

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

- [Full Protocol Reference](../hooks/README.md)
- [Configuration](../configuration/README.md)
