# MCP Prompt Integration

Access and manage prompt templates through MCP tools and resources.

**Note:** MCP prompt capabilities are disabled. Built-in UX helper prompts are
CLI-only (`subcog prompt list --tags ux-helper`).

## MCP Tools

Subcog prompt tools are accessible via the `subcog:` prefix in Claude Code.

### Tool Invocation Syntax

| Tool | Claude Code Syntax |
|------|-------------------|
| `prompt_save` | `subcog:prompt:save` |
| `prompt_list` | `subcog:prompt:list` |
| `prompt_get` | `subcog:prompt:get` |
| `prompt_run` | `subcog:prompt:run` |
| `prompt_delete` | `subcog:prompt:delete` |

### prompt_save

Save a new prompt template.

**Claude Code:**
```
subcog:prompt:save code-review --content "Review {{file}} for {{issue_type}}" --domain project --tags review,quality
```

**MCP JSON-RPC:**
```json
{
  "name": "subcog:prompt:save",
  "arguments": {
    "name": "code-review",
    "description": "Code review template",
    "content": "Review {{file}} for {{issue_type}} issues",
    "domain": "project",
    "tags": ["review", "quality"],
    "variables": [
      {"name": "file", "required": true},
      {"name": "issue_type", "default": "general"}
    ]
  }
}
```

### prompt_list

List available prompts.

**Claude Code:**
```
subcog:prompt:list --domain project --tags review --limit 20
```

**MCP JSON-RPC:**
```json
{
  "name": "subcog:prompt:list",
  "arguments": {
    "domain": "project",
    "tags": ["review"],
    "limit": 20
  }
}
```

**Response:**
```json
{
  "prompts": [
    {
      "name": "code-review",
      "description": "Code review template",
      "domain": "project",
      "tags": ["review", "quality"],
      "variables": ["file", "issue_type"]
    }
  ],
  "total": 1
}
```

### prompt_get

Get a specific prompt.

**Claude Code:**
```
subcog:prompt:get code-review
```

**MCP JSON-RPC:**
```json
{
  "name": "subcog:prompt:get",
  "arguments": {
    "name": "code-review"
  }
}
```

**Response:**
```json
{
  "name": "code-review",
  "description": "Code review template",
  "content": "Review {{file}} for {{issue_type}} issues",
  "domain": "project",
  "tags": ["review", "quality"],
  "variables": [
    {"name": "file", "required": true},
    {"name": "issue_type", "required": false, "default": "general"}
  ]
}
```

### prompt_run

Execute a prompt with variable substitution.

**Claude Code:**
```
subcog:prompt:run code-review --var file=src/main.rs --var issue_type=security
```

**MCP JSON-RPC:**
```json
{
  "name": "subcog:prompt:run",
  "arguments": {
    "name": "code-review",
    "variables": {
      "file": "src/main.rs",
      "issue_type": "security"
    }
  }
}
```

**Response:**
```json
{
  "content": "Review src/main.rs for security issues",
  "variables_used": {
    "file": "src/main.rs",
    "issue_type": "security"
  }
}
```

### prompt_delete

Delete a prompt template.

**Claude Code:**
```
subcog:prompt:delete old-template --domain project
```

**MCP JSON-RPC:**
```json
{
  "name": "subcog:prompt:delete",
  "arguments": {
    "name": "old-template",
    "domain": "project"
  }
}
```

## MCP Resources

### List Resources

| Resource | Description |
|----------|-------------|
| `subcog://_prompts` | All prompts across domains |
| `subcog://project/_prompts` | Project prompts |
| `subcog://user/_prompts` | User prompts |
| `subcog://org/_prompts` | Org prompts |

**Example Request:**
```json
{
  "method": "resources/read",
  "params": {
    "uri": "subcog://project/_prompts"
  }
}
```

**Response:**
```json
{
  "contents": [{
    "uri": "subcog://project/_prompts",
    "mimeType": "application/json",
    "text": "{\"prompts\": [{\"name\": \"code-review\", ...}]}"
  }]
}
```

### Get Resources

| Resource | Description |
|----------|-------------|
| `subcog://project/_prompts/{name}` | Specific project prompt |
| `subcog://user/_prompts/{name}` | Specific user prompt |
| `subcog://org/_prompts/{name}` | Specific org prompt |

**Example Request:**
```json
{
  "method": "resources/read",
  "params": {
    "uri": "subcog://project/_prompts/code-review"
  }
}
```

**Response:**
```json
{
  "contents": [{
    "uri": "subcog://project/_prompts/code-review",
    "mimeType": "application/json",
    "text": "{\"name\": \"code-review\", \"content\": \"Review {{file}}...\", ...}"
  }]
}
```

## Usage Patterns

### Create Template via MCP

```json
// 1. Save the template
{
  "method": "tools/call",
  "params": {
    "name": "subcog:prompt:save",
    "arguments": {
      "name": "security-audit",
      "content": "Audit {{file}} for security issues:\n- SQL injection\n- XSS\n- CSRF",
      "tags": ["security"]
    }
  }
}
```

### Use Template via MCP

```json
// 2. Run the template
{
  "method": "tools/call",
  "params": {
    "name": "subcog:prompt:run",
    "arguments": {
      "name": "security-audit",
      "variables": {"file": "src/auth.rs"}
    }
  }
}
```

### List and Select

```json
// 1. List available templates
{
  "method": "tools/call",
  "params": {
    "name": "subcog:prompt:list",
    "arguments": {"tags": ["review"]}
  }
}

// 2. Get specific template details
{
  "method": "tools/call",
  "params": {
    "name": "subcog:prompt:get",
    "arguments": {"name": "code-review"}
  }
}

// 3. Execute with variables
{
  "method": "tools/call",
  "params": {
    "name": "subcog:prompt:run",
    "arguments": {
      "name": "code-review",
      "variables": {"file": "main.rs", "issue_type": "performance"}
    }
  }
}
```

### Browse via Resources

```json
// 1. List all project prompts
{
  "method": "resources/read",
  "params": {"uri": "subcog://project/_prompts"}
}

// 2. Get specific prompt
{
  "method": "resources/read",
  "params": {"uri": "subcog://project/_prompts/code-review"}
}
```

## Integration with UX Helper Prompts (CLI-only)

User templates complement built-in UX helper prompts:

| Built-in | User Template Use |
|----------|-------------------|
| `subcog_capture_assistant` | Create custom capture templates |
| `subcog_search_help` | Create custom search templates |
| `generate_decision` | Create domain-specific decision templates |

### Example: Custom Capture Template

```json
{
  "name": "subcog:prompt:save",
  "arguments": {
    "name": "capture-decision",
    "content": "## Decision: {{title}}\n\n### Context\n{{context}}\n\n### Decision\n{{decision}}\n\n### Consequences\n{{consequences}}",
    "variables": [
      {"name": "title", "required": true},
      {"name": "context", "required": true},
      {"name": "decision", "required": true},
      {"name": "consequences", "default": "TBD"}
    ]
  }
}
```

Then use with subcog_capture:
```json
{
  "name": "subcog_capture",
  "arguments": {
    "namespace": "decisions",
    "content": "[Result of prompt_run capture-decision]"
  }
}
```

## Error Handling

| Error Code | Meaning |
|------------|---------|
| -32607 | Prompt not found |
| -32608 | Validation error (invalid name, missing required var) |
| -32602 | Invalid parameters |

**Error Response:**
```json
{
  "error": {
    "code": -32607,
    "message": "Prompt not found: unknown-template"
  }
}
```

## See Also

- [MCP Tools](../mcp/tools.md#prompt_save) - Full tool documentation
- [MCP Resources](../mcp/resources.md#prompts) - Resource documentation
- [Variables](variables.md) - Variable substitution
