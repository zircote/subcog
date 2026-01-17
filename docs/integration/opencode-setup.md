# Subcog Integration Guide for OpenCode

This guide provides instructions for integrating Subcog with OpenCode.

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
      "command": "subcog",
      "args": ["serve"]
    }
  }
}
```

### 3. Add Protocol to Configuration

```markdown
## Subcog Memory Protocol (MANDATORY)

At the start of EVERY session, call `subcog_init` to load memory context and best practices.

During the session:
- **Before implementing**: Recall relevant decisions and patterns
- **Capture immediately**: When decisions, patterns, learnings, or fixes are identified
- Access Subcog only via MCP tools, never shell commands
```

### 4. Hooks (Optional)

OpenCode supports hooks via its [TypeScript plugin system](https://opencode.ai/docs/plugins/). Create `.opencode/plugin/subcog.ts`:

```typescript
import type { Plugin } from "@opencode-ai/plugin"

export const SubcogPlugin: Plugin = async ({ $ }) => {
  return {
    "session.created": async () => {
      await $`subcog hook session-start`
    },
    "tool.execute.after": async () => {
      await $`subcog hook post-tool-use`
    }
  }
}
```

See [OpenCode Plugins documentation](https://opencode.ai/docs/plugins/) for details.

## Verification

Once configured, verify with:

```
subcog_status: {}
```

## See Also

- [Full Protocol Reference](../hooks/README.md)
- [Configuration](../configuration/README.md)
