# Subcog Integration Guide for OpenAI / ChatGPT

This guide provides instructions for integrating Subcog's persistent memory system with OpenAI-based coding workflows, including ChatGPT, GPT-4, and custom GPTs.

## Overview

OpenAI models can interact with Subcog through:
1. **CLI commands** - Direct shell access (Code Interpreter, terminal)
2. **MCP Bridge** - Via compatible clients
3. **Custom Instructions** - Protocol guidance in system prompts

---

## Quick Start

### 1. Install Subcog

```bash
# Install from crates.io
cargo install subcog

# Verify installation
subcog --version
```

### 2. CLI Usage

Use Subcog directly via shell commands:

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
## Subcog Memory Protocol

You have access to Subcog for persistent memory across sessions.

### CLI Commands

| Command | Purpose |
|---------|---------|
| `subcog capture -n <namespace> -c "<content>" -t tag1,tag2` | Store memory |
| `subcog recall "<query>"` | Search memories |
| `subcog recall --filter "ns:decisions"` | Filter by namespace |
| `subcog status` | Check system health |
| `subcog get <id>` | Get memory by ID |

### Namespaces

| Namespace | Purpose |
|-----------|---------|
| `decisions` | Architecture choices |
| `patterns` | Coding conventions |
| `learnings` | Insights and discoveries |
| `context` | Project background |
| `tech-debt` | Known issues |

### Workflow

1. **Before responding**: Run `subcog recall` with relevant keywords
2. **Capture decisions**: When user makes choices, run `subcog capture -n decisions`
3. **Capture learnings**: When discovering insights, run `subcog capture -n learnings`

### Example Commands

```bash
# Search for authentication patterns
subcog recall "authentication" --filter "ns:patterns"

# Capture a decision
subcog capture -n decisions -c "Using JWT for API auth. Rationale: stateless, scalable" -t auth,jwt,api

# List recent memories
subcog recall --filter "since:7d" --limit 10
```
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
