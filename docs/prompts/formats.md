# Template Formats

Subcog supports multiple formats for prompt templates.

## Supported Formats

| Format | Extension | Best For |
|--------|-----------|----------|
| YAML | `.yaml`, `.yml` | Full metadata |
| JSON | `.json` | Programmatic use |
| Markdown | `.md` | Rich content |
| Plain text | `.txt` | Simple templates |

## YAML Format

Most complete format with full metadata support.

### Structure

```yaml
name: template-name
description: Human-readable description
content: |
  Template content with {{variables}}
  Can be multiline
variables:
  - name: variable_name
    description: Variable description
    required: true
    default: default_value
tags:
  - tag1
  - tag2
```

### Example

```yaml
name: code-review
description: Comprehensive code review template
content: |
  ## Code Review: {{file}}

  ### Focus Areas
  - {{issue_type}} issues
  - Code quality
  - Performance

  ### Checklist
  - [ ] Input validation
  - [ ] Error handling
  - [ ] Test coverage

  ### Notes
  {{notes}}
variables:
  - name: file
    description: File path to review
    required: true
  - name: issue_type
    description: Type of issues to focus on
    default: general
  - name: notes
    description: Additional notes
    default: None
tags:
  - review
  - code-quality
```

### Import

```bash
subcog prompt save --from-file template.yaml
```

---

## JSON Format

Good for programmatic creation and API use.

### Structure

```json
{
  "name": "template-name",
  "description": "Human-readable description",
  "content": "Template content with {{variables}}",
  "variables": [
    {
      "name": "variable_name",
      "description": "Variable description",
      "required": true,
      "default": "default_value"
    }
  ],
  "tags": ["tag1", "tag2"]
}
```

### Example

```json
{
  "name": "api-endpoint",
  "description": "API endpoint documentation",
  "content": "## {{method}} {{path}}\n\n{{description}}\n\n### Request\n```json\n{{request}}\n```\n\n### Response\n```json\n{{response}}\n```",
  "variables": [
    {
      "name": "method",
      "description": "HTTP method",
      "required": true
    },
    {
      "name": "path",
      "description": "Endpoint path",
      "required": true
    },
    {
      "name": "description",
      "description": "Endpoint description",
      "required": true
    },
    {
      "name": "request",
      "description": "Request body example",
      "default": "{}"
    },
    {
      "name": "response",
      "description": "Response body example",
      "default": "{}"
    }
  ],
  "tags": ["api", "documentation"]
}
```

### Import

```bash
subcog prompt save --from-file template.json
```

---

## Markdown Format

Uses YAML frontmatter with Markdown content.

### Structure

```markdown
---
name: template-name
description: Human-readable description
variables:
  - name: var1
    required: true
  - name: var2
    default: value
tags: [tag1, tag2]
---

# Template Content

Your prompt content with {{var1}} and {{var2}}.
```

### Example

```markdown
---
name: feature-spec
description: Feature specification template
variables:
  - name: feature_name
    description: Name of the feature
    required: true
  - name: priority
    description: Feature priority
    default: medium
  - name: requirements
    description: List of requirements
    required: true
tags: [spec, feature, planning]
---

# Feature Specification: {{feature_name}}

## Priority
{{priority}}

## Requirements
{{requirements}}

## Acceptance Criteria
- [ ] All requirements implemented
- [ ] Tests passing
- [ ] Documentation updated

## Notes
Additional implementation notes go here.
```

### Import

```bash
subcog prompt save --from-file template.md
```

---

## Plain Text Format

Simple format without metadata. Variables are extracted from content.

### Structure

```
Template content with {{variable1}} and {{variable2}}.
```

### Example

```
Review {{file}} for {{issue_type}} issues.

Check:
- Security vulnerabilities
- Performance problems
- Code style

Reviewer: {{reviewer}}
```

### Import

```bash
subcog prompt save my-template --file template.txt
```

Variables are automatically extracted from `{{...}}` patterns.

---

## Format Detection

When loading from file, format is detected by:

1. **Extension** - `.yaml`, `.json`, `.md`, `.txt`
2. **Content** - YAML frontmatter, JSON structure

### Detection Order

1. Check file extension
2. If `.md`, look for YAML frontmatter
3. If unknown, try JSON parse
4. If fails, try YAML parse
5. Default to plain text

---

## Export Formats

Export templates in any format:

```bash
# Export as YAML
subcog prompt export template-name -f yaml -o template.yaml

# Export as JSON
subcog prompt export template-name -f json -o template.json

# Export as Markdown
subcog prompt export template-name -f md -o template.md
```

---

## Format Comparison

| Feature | YAML | JSON | Markdown | Plain Text |
|---------|------|------|----------|------------|
| Metadata | Full | Full | Full | None |
| Multiline | Easy | Escaped | Natural | Natural |
| Comments | Yes | No | No | No |
| Readability | High | Medium | High | High |
| Programmatic | Medium | High | Low | Low |

---

## Best Practices

1. **Use YAML** for templates with rich metadata
2. **Use JSON** for programmatic generation
3. **Use Markdown** for documentation-heavy templates
4. **Use plain text** for simple, quick templates
5. **Include variables section** even if empty for clarity

## See Also

- [Variables](variables.md) - Variable substitution
- [Storage](storage.md) - How templates are stored
- [CLI prompt](../cli/prompt.md) - Import/export commands
