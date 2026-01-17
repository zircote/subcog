# Subcog Integration Guide for OpenAI / ChatGPT

This guide provides instructions for integrating Subcog's persistent memory system with OpenAI-based coding workflows, including ChatGPT, GPT-4, custom GPTs, and any MCP-compatible OpenAI client.

## Overview

OpenAI models can interact with Subcog through:
1. **MCP Server** - Native Model Context Protocol integration (recommended)
2. **CLI commands** - Direct shell access (Code Interpreter, terminal)
3. **Custom Instructions** - Protocol guidance in system prompts

---

## Quick Start (MCP Server)

### 1. Install Subcog

```bash
# Install from crates.io
cargo install subcog

# Verify installation
subcog --version
```

### 2. Configure MCP Server

For any MCP-compatible OpenAI client, add to your MCP configuration:

```json
{
  "mcpServers": {
    "subcog": {
      "command": "subcog",
      "args": ["serve"],
      "env": {
        "SUBCOG_LOG_LEVEL": "info"
      }
    }
  }
}
```

This exposes all Subcog tools:
- `subcog_capture` - Store memories
- `subcog_recall` - Search memories (semantic + text hybrid)
- `subcog_status` - System health
- `subcog_get`, `subcog_update`, `subcog_delete` - Memory CRUD
- And 20+ more tools for knowledge graph, consolidation, etc.

### 3. Verify Connection

Once configured, the model can call Subcog tools directly:

```
subcog_status: {}
subcog_recall: { "query": "database architecture" }
subcog_capture: { "content": "Using PostgreSQL", "namespace": "decisions" }
```

---

## CLI Usage (Alternative)

If MCP is not available, use Subcog directly via shell commands:

```bash
# Capture a memory
subcog capture --namespace decisions --content "Using PostgreSQL for data storage" --tags database,sql

# Search memories
subcog recall "database architecture"

# Check status
subcog status
```

---

## Custom Instructions / System Prompt

Add to your GPT's system instructions or custom instructions:

```markdown
## Subcog Memory Protocol (MANDATORY)

At the start of EVERY session, call `subcog_init` to load memory context and best practices.

During the session:
- **Before implementing**: Recall relevant decisions and patterns
- **Capture immediately**: When decisions, patterns, learnings, or fixes are identified
- Access Subcog only via MCP tools, never shell commands
```

---

## Code Interpreter Integration

When using ChatGPT with Code Interpreter:

```python
import subprocess

def subcog_capture(content, namespace="learnings", tags=None):
    """Capture a memory to Subcog."""
    cmd = ["subcog", "capture", "-n", namespace, "-c", content]
    if tags:
        cmd.extend(["-t", ",".join(tags)])
    result = subprocess.run(cmd, capture_output=True, text=True)
    return result.stdout

def subcog_recall(query, filter=None, limit=10):
    """Search Subcog memories."""
    cmd = ["subcog", "recall", query, "--limit", str(limit)]
    if filter:
        cmd.extend(["--filter", filter])
    result = subprocess.run(cmd, capture_output=True, text=True)
    return result.stdout

def subcog_status():
    """Check Subcog status."""
    result = subprocess.run(["subcog", "status"], capture_output=True, text=True)
    return result.stdout
```

---

## GPT Actions (Custom GPTs)

For Custom GPTs with Actions, you can expose Subcog via a REST wrapper:

### 1. Create REST Wrapper

```python
# subcog_api.py - Simple Flask wrapper
from flask import Flask, request, jsonify
import subprocess
import json

app = Flask(__name__)

@app.route("/recall", methods=["POST"])
def recall():
    data = request.json
    query = data.get("query", "")
    filter_str = data.get("filter", "")

    cmd = ["subcog", "recall", query, "--format", "json"]
    if filter_str:
        cmd.extend(["--filter", filter_str])

    result = subprocess.run(cmd, capture_output=True, text=True)
    return jsonify(json.loads(result.stdout))

@app.route("/capture", methods=["POST"])
def capture():
    data = request.json
    cmd = [
        "subcog", "capture",
        "-n", data.get("namespace", "learnings"),
        "-c", data.get("content"),
        "--format", "json"
    ]
    if data.get("tags"):
        cmd.extend(["-t", ",".join(data["tags"])])

    result = subprocess.run(cmd, capture_output=True, text=True)
    return jsonify(json.loads(result.stdout))

if __name__ == "__main__":
    app.run(port=8080)
```

### 2. OpenAPI Schema for GPT Action

```yaml
openapi: 3.0.0
info:
  title: Subcog Memory API
  version: 1.0.0
servers:
  - url: https://your-server.com
paths:
  /recall:
    post:
      operationId: recallMemories
      summary: Search memories
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              properties:
                query:
                  type: string
                filter:
                  type: string
      responses:
        '200':
          description: Memory search results
  /capture:
    post:
      operationId: captureMemory
      summary: Store a memory
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              required: [content, namespace]
              properties:
                content:
                  type: string
                namespace:
                  type: string
                tags:
                  type: array
                  items:
                    type: string
      responses:
        '200':
          description: Memory captured
```

---

## Filter Syntax Reference

```bash
# By namespace
subcog recall --filter "ns:decisions"

# By tags
subcog recall --filter "tag:rust tag:api"

# Exclude tags
subcog recall --filter "-tag:deprecated"

# Time-based
subcog recall --filter "since:7d"      # Last 7 days
subcog recall --filter "since:30d"     # Last 30 days

# Combined
subcog recall "auth" --filter "ns:patterns tag:security since:30d"
```

---

## Troubleshooting

| Issue | Solution |
|-------|----------|
| Command not found | Ensure `~/.cargo/bin` is in PATH |
| No memories found | Check `subcog status`, try broader query |
| Permission errors | Check file permissions on `~/.local/share/subcog` |

---

## See Also

- [CLI Reference](../cli/README.md) - Full CLI documentation
- [Configuration](../configuration/README.md) - Configuration options
