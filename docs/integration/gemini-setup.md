# Subcog Integration Guide for Google Gemini

This guide provides instructions for integrating Subcog's persistent memory system with Google Gemini, including Gemini Pro, Gemini Ultra, AI Studio, and any MCP-compatible Gemini client.

## Overview

Gemini can interact with Subcog through:
1. **MCP Server** - Native Model Context Protocol integration (recommended)
2. **Function Calling** - Gemini's native function calling feature
3. **CLI commands** - Via code execution capabilities
4. **System Instructions** - Protocol guidance

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

For any MCP-compatible Gemini client, add to your MCP configuration:

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

Once configured, Gemini can call Subcog tools directly:

```
subcog_status: {}
subcog_recall: { "query": "database architecture" }
subcog_capture: { "content": "Using Firestore", "namespace": "decisions" }
```

---

## CLI Usage (Alternative)

If MCP is not available, use Subcog via shell:

```bash
# Capture a memory
subcog capture --namespace decisions --content "Using Firestore for real-time sync" --tags gcp,firebase

# Search memories
subcog recall "database sync"

# Check status
subcog status
```

---

## System Instructions

Add to your Gemini system instructions:

```markdown
## Subcog Memory Protocol

You have access to Subcog for persistent memory. Use shell commands to interact.

### Commands

| Command | Purpose |
|---------|---------|
| `subcog capture -n <ns> -c "<text>" -t tags` | Store memory |
| `subcog recall "<query>"` | Search memories |
| `subcog recall --filter "ns:decisions"` | Filter search |
| `subcog status` | System health |

### Namespaces

- `decisions` - Architecture choices
- `patterns` - Coding conventions
- `learnings` - Insights discovered
- `context` - Project background
- `tech-debt` - Known issues

### Workflow

1. Search before responding: `subcog recall "relevant keywords"`
2. Capture decisions: `subcog capture -n decisions -c "..." -t tag1,tag2`
3. Capture learnings: `subcog capture -n learnings -c "..."`
```

---

## Function Calling Integration

Define Subcog functions for Gemini's function calling:

### Function Declarations

```python
subcog_tools = [
    {
        "name": "subcog_recall",
        "description": "Search persistent memories for relevant context",
        "parameters": {
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query keywords"
                },
                "namespace": {
                    "type": "string",
                    "enum": ["decisions", "patterns", "learnings", "context", "tech-debt"],
                    "description": "Filter by namespace"
                },
                "limit": {
                    "type": "integer",
                    "description": "Max results to return",
                    "default": 10
                }
            },
            "required": ["query"]
        }
    },
    {
        "name": "subcog_capture",
        "description": "Store a memory for future reference",
        "parameters": {
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "Memory content to store"
                },
                "namespace": {
                    "type": "string",
                    "enum": ["decisions", "patterns", "learnings", "context", "tech-debt"],
                    "description": "Category for the memory"
                },
                "tags": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Searchable tags"
                }
            },
            "required": ["content", "namespace"]
        }
    },
    {
        "name": "subcog_status",
        "description": "Check Subcog memory system health and statistics",
        "parameters": {
            "type": "object",
            "properties": {}
        }
    }
]
```

### Function Implementations

```python
import subprocess
import json

def execute_subcog_function(function_name: str, args: dict) -> str:
    """Execute a Subcog function and return results."""

    if function_name == "subcog_recall":
        cmd = ["subcog", "recall", args["query"], "--format", "json"]
        if args.get("namespace"):
            cmd.extend(["--filter", f"ns:{args['namespace']}"])
        if args.get("limit"):
            cmd.extend(["--limit", str(args["limit"])])

    elif function_name == "subcog_capture":
        cmd = [
            "subcog", "capture",
            "-n", args["namespace"],
            "-c", args["content"],
            "--format", "json"
        ]
        if args.get("tags"):
            cmd.extend(["-t", ",".join(args["tags"])])

    elif function_name == "subcog_status":
        cmd = ["subcog", "status", "--format", "json"]

    else:
        return json.dumps({"error": f"Unknown function: {function_name}"})

    result = subprocess.run(cmd, capture_output=True, text=True)
    return result.stdout if result.returncode == 0 else result.stderr
```

### Usage with Gemini API

```python
import google.generativeai as genai

genai.configure(api_key="YOUR_API_KEY")

model = genai.GenerativeModel(
    model_name="gemini-pro",
    tools=subcog_tools,
    system_instruction="""
You have access to Subcog persistent memory.
Before answering questions, use subcog_recall to check for relevant context.
Capture important decisions and learnings with subcog_capture.
"""
)

chat = model.start_chat()

# When Gemini calls a function, execute it
response = chat.send_message("What database did we decide to use?")

for part in response.parts:
    if hasattr(part, 'function_call'):
        func_name = part.function_call.name
        func_args = dict(part.function_call.args)
        result = execute_subcog_function(func_name, func_args)

        # Send result back to Gemini
        response = chat.send_message(
            genai.protos.Content(
                parts=[genai.protos.Part(
                    function_response=genai.protos.FunctionResponse(
                        name=func_name,
                        response={"result": result}
                    )
                )]
            )
        )
```

---

## AI Studio Integration

For Google AI Studio:

1. **Create a new prompt** in AI Studio
2. **Add system instructions** with the Subcog protocol
3. **Enable code execution** if available
4. **Define tools** using the function declarations above

---

## Vertex AI Integration

For enterprise Vertex AI deployments:

```python
from vertexai.generative_models import GenerativeModel, Tool, FunctionDeclaration

# Define tools
subcog_tool = Tool(
    function_declarations=[
        FunctionDeclaration(
            name="subcog_recall",
            description="Search persistent memories",
            parameters={
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "namespace": {"type": "string"},
                },
                "required": ["query"]
            }
        ),
        # ... other functions
    ]
)

model = GenerativeModel(
    "gemini-1.5-pro",
    tools=[subcog_tool],
    system_instruction="Use Subcog for persistent memory..."
)
```

---

## Filter Syntax Reference

```bash
# By namespace
subcog recall --filter "ns:decisions"

# By tags
subcog recall --filter "tag:gcp tag:firebase"

# Time-based
subcog recall --filter "since:7d"

# Combined
subcog recall "auth" --filter "ns:patterns tag:security"
```

---

## Troubleshooting

| Issue | Solution |
|-------|----------|
| Function not called | Check tool declarations match exactly |
| Empty results | Verify `subcog status` shows memories |
| Timeout | Increase timeout in API call settings |

---

## See Also

- [CLI Reference](../cli/README.md) - Full CLI documentation
- [Configuration](../configuration/README.md) - Configuration options
