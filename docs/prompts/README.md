# Prompt Templates

Subcog supports user-defined prompt templates with variable substitution, enabling
reusable prompts across sessions and teams.

## Documentation

| Topic | Description |
|-------|-------------|
| [Overview](./overview.md) | What are prompt templates |
| [Variables](variables.md) | Variable substitution syntax |
| [Formats](formats.md) | YAML, JSON, Markdown, Plain text |
| [Storage](storage.md) | Domain-scoped storage |
| [MCP Integration](mcp.md) | Accessing prompts via MCP |
| [System Prompts](SYSTEM_PROMPTS.md) | LLM system prompts (security, customization) |

## Quick Start

### Create a Template

```bash
subcog prompt save code-review \
  --content "Review {{file}} for {{issue_type}} issues" \
  --description "Code review template"
```

### Use a Template

```bash
subcog prompt run code-review \
  --var file=src/main.rs \
  --var issue_type=security
```

Output:
```
Review src/main.rs for security issues
```

## Key Features

### Variable Substitution

Use `{{variable_name}}` syntax:

```
Review {{file}} for:
- {{issue_type}} issues
- Best practices
- Edge cases
```

### Domain Scoping

Store templates at different levels:
- **Project** - Repository-specific
- **User** - Personal templates
- **Org** - Team-shared templates

### Multi-Format Support

Create templates in multiple formats:
- YAML with metadata
- JSON for programmatic use
- Markdown for rich content
- Plain text for simple templates

### MCP Integration

Access templates via MCP tools:
- `prompt_save` - Create templates
- `prompt_list` - List templates
- `prompt_get` - Retrieve templates
- `prompt_run` - Execute with variables
- `prompt_delete` - Remove templates

## Example Templates

### Code Review

```yaml
name: code-review
description: Comprehensive code review
content: |
  Review {{file}} for:

  ## Focus Areas
  - {{issue_type}} issues
  - Code quality
  - Performance implications

  ## Checklist
  - [ ] Input validation
  - [ ] Error handling
  - [ ] Test coverage
variables:
  - name: file
    description: File to review
    required: true
  - name: issue_type
    description: Type of issues
    default: general
tags: [review, quality]
```

### API Documentation

```yaml
name: api-doc
description: API endpoint documentation
content: |
  ## {{method}} {{endpoint}}

  {{description}}

  ### Request
  ```json
  {{request_body}}
  ```

  ### Response
  ```json
  {{response_body}}
  ```
variables:
  - name: method
    required: true
  - name: endpoint
    required: true
  - name: description
    required: true
  - name: request_body
    default: "{}"
  - name: response_body
    default: "{}"
tags: [api, documentation]
```

### Bug Report

```yaml
name: bug-report
description: Bug report template
content: |
  ## Bug: {{title}}

  ### Environment
  - OS: {{os}}
  - Version: {{version}}

  ### Steps to Reproduce
  {{steps}}

  ### Expected Behavior
  {{expected}}

  ### Actual Behavior
  {{actual}}
variables:
  - name: title
    required: true
  - name: os
    default: "Not specified"
  - name: version
    default: "Latest"
  - name: steps
    required: true
  - name: expected
    required: true
  - name: actual
    required: true
tags: [bug, issue]
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `subcog prompt save` | Save a template |
| `subcog prompt list` | List templates |
| `subcog prompt get` | Get a template |
| `subcog prompt run` | Execute a template |
| `subcog prompt delete` | Delete a template |
| `subcog prompt export` | Export to file |
| `subcog prompt save --from-file` | Import from file |
| `subcog prompt share` | Share across domains |

## MCP Resources

| Resource | Description |
|----------|-------------|
| `subcog://_prompts` | All prompts |
| `subcog://project/_prompts` | Project prompts |
| `subcog://user/_prompts` | User prompts |
| `subcog://org/_prompts` | Org prompts |
| `subcog://project/_prompts/{name}` | Specific prompt |

## Best Practices

1. **Use descriptive names** - `security-review` not `sr`
2. **Add descriptions** - Helps with discovery
3. **Tag templates** - Enables filtering
4. **Set defaults** - Reduces required inputs
5. **Use appropriate domain** - Project for repo-specific

## See Also

- [Variables](variables.md) - Variable syntax
- [Formats](formats.md) - Template formats
- [CLI prompt](../cli/prompt.md) - CLI reference
- [MCP prompt_*](../mcp/tools.md#prompt_save) - MCP tools
- [System Prompts](SYSTEM_PROMPTS.md) - LLM system prompt customization
