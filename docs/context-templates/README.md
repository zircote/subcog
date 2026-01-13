# Context Templates

Context Templates are user-defined templates for formatting memories and statistics in hooks and MCP tool responses. They support variable substitution, iteration over collections, and multiple output formats.

## Overview

Context Templates differ from [Prompt Templates](../prompts/README.md):

| Feature | Prompt Templates | Context Templates |
|---------|------------------|-------------------|
| Purpose | Reusable prompts for LLM interactions | Format memories for display/injection |
| Variables | User-provided only | Auto-populated from memories + user-provided |
| Iteration | Not supported | `{{#each}}` syntax for collections |
| Output formats | Text only | Markdown, JSON, XML |
| Versioning | Manual | Auto-increment on save |

## Template Syntax

### Variables

Variables use double-brace syntax: `{{variable_name}}`

```markdown
# {{title}}

Found {{total_count}} memories in {{namespace}}.
```

### Auto-Variables

These variables are automatically populated from the render context:

| Variable | Description |
|----------|-------------|
| `{{memories}}` | List of memories (for iteration) |
| `{{memory.id}}` | Memory ID (in iteration) |
| `{{memory.content}}` | Memory content (in iteration) |
| `{{memory.namespace}}` | Memory namespace (in iteration) |
| `{{memory.tags}}` | Memory tags (in iteration) |
| `{{memory.score}}` | Search relevance score (in iteration) |
| `{{memory.created_at}}` | Creation timestamp (in iteration) |
| `{{memory.updated_at}}` | Last update timestamp (in iteration) |
| `{{memory.domain}}` | Memory domain (in iteration) |
| `{{total_count}}` | Total number of memories |
| `{{namespace_counts}}` | Counts per namespace |
| `{{statistics}}` | Full statistics object |

### Iteration

Use `{{#each collection}}...{{/each}}` to iterate over lists:

```markdown
{{#each memories}}
- **{{memory.namespace}}**: {{memory.content}}
  Score: {{memory.score}}
{{/each}}
```

The item prefix is derived from the collection name:
- `memories` → `memory`
- `items` → `item`
- `entries` → `entry`

### Custom Collections

You can iterate over custom collections:

```markdown
{{#each items}}
- {{item.name}}: {{item.value}}
{{/each}}
```

Provide custom collections via the `variables` parameter when rendering.

## Output Formats

### Markdown (default)

Template content is output as-is after variable substitution.

### JSON

Converts markdown structure to JSON:

```json
{
  "sections": [
    {"level": 1, "title": "Search Results", "content": "..."},
    {"level": 2, "title": "Decisions", "content": "..."}
  ],
  "raw": "# Search Results\n\n..."
}
```

### XML

Converts markdown structure to XML:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<context>
  <section level="1" title="Search Results">
    <item>First memory content</item>
    <item>Second memory content</item>
  </section>
</context>
```

## MCP Tools

### `context_template_save`

Save or update a context template. Version auto-increments on each save.

```yaml
context_template_save:
  name: search-results          # Required: kebab-case name
  content: |                    # Required: template content
    # {{title}}
    {{#each memories}}
    - {{memory.content}}
    {{/each}}
  description: Format search results
  tags: [search, formatting]
  domain: project               # project, user, or org
  output_format: markdown       # markdown, json, or xml
  variables:                    # Optional: variable metadata
    - name: title
      description: Section title
      default: "Search Results"
      required: false
```

### `context_template_list`

List templates with optional filtering.

```yaml
context_template_list:
  domain: user                  # Optional: filter by domain
  tags: [formatting]            # Optional: filter by tags
  name_pattern: "search-*"      # Optional: glob pattern
  limit: 20                     # Optional: max results (default 20)
```

### `context_template_get`

Fetch a template by name.

```yaml
context_template_get:
  name: search-results          # Required: template name
  version: 2                    # Optional: specific version (default: latest)
  domain: user                  # Optional: domain scope
```

### `context_template_render`

Render a template with memories and variables.

```yaml
context_template_render:
  name: search-results          # Required: template name
  version: 1                    # Optional: specific version
  query: "authentication"       # Optional: search query for memories
  namespaces: [decisions]       # Optional: filter namespaces
  limit: 10                     # Optional: max memories (default 10)
  format: json                  # Optional: override output format
  variables:                    # Optional: custom variables
    title: "Auth Patterns"
```

### `context_template_delete`

Delete a template or specific version.

```yaml
context_template_delete:
  name: search-results          # Required: template name
  domain: project               # Required: domain scope
  version: 1                    # Optional: specific version (default: all)
```

## Configuration

Configure context templates in `~/.config/subcog/config.toml`:

```toml
[context_templates]
enabled = true
default_format = "markdown"  # markdown, json, xml

# Per-hook template overrides (optional)
[context_templates.hooks.session_start]
template = "session-context"
version = 1                  # Optional: use specific version
format = "markdown"          # Optional: override format

[context_templates.hooks.user_prompt_submit]
template = "prompt-context"
format = "markdown"

[context_templates.hooks.post_tool_use]
template = "tool-context"
```

## Domain Scoping

Templates are scoped by domain:

| Domain | Scope | Use Case |
|--------|-------|----------|
| `project` | Current repository | Project-specific formatting |
| `user` | User-wide | Personal templates across projects |
| `org` | Organization | Shared team templates |

Resolution order: project → user → org

## Versioning

Templates use auto-increment versioning:

1. First save creates version 1
2. Subsequent saves create version 2, 3, etc.
3. Specify `version` when rendering to use a specific version
4. Omit `version` to use the latest

## Example Templates

### Search Results Formatter

```yaml
context_template_save:
  name: search-results
  content: |
    # {{title}}

    Found **{{total_count}}** memories matching your query.

    {{#each memories}}
    ## {{memory.namespace}}

    {{memory.content}}

    _Relevance: {{memory.score}} | Created: {{memory.created_at}}_

    ---
    {{/each}}
  tags: [search, display]
  domain: user
```

### Session Context Builder

```yaml
context_template_save:
  name: session-context
  content: |
    # Project Context

    ## Recent Decisions
    {{#each memories}}
    - **{{memory.namespace}}**: {{memory.content}}
    {{/each}}

    ## Statistics
    - Total memories: {{total_count}}
  tags: [hooks, session]
  domain: project
```

### XML API Response

```yaml
context_template_save:
  name: api-response
  content: |
    # API Response

    {{#each memories}}
    - {{memory.content}}
    {{/each}}
  output_format: xml
  tags: [api, integration]
  domain: user
```

## Best Practices

1. **Use kebab-case names**: `search-results`, `session-context`
2. **Add descriptive tags**: Enable filtering and discovery
3. **Provide variable defaults**: Make templates usable without explicit values
4. **Use user domain for reusable templates**: Share across projects
5. **Test with dry-run**: Preview rendered output before hook integration
6. **Keep templates focused**: One purpose per template

## Troubleshooting

| Issue | Solution |
|-------|----------|
| Template not found | Check domain scope, verify name spelling |
| Variable not substituted | Ensure variable is provided or has default |
| Iteration empty | Verify collection exists in context |
| Wrong format output | Check `output_format` setting |
| Version conflict | Templates auto-increment; use explicit version if needed |

## See Also

- [Prompt Templates](../prompts/README.md) - For LLM prompt management
- [Hooks](../hooks/README.md) - Hook integration with templates
- [MCP Tools](../mcp/README.md) - Full MCP tool reference
