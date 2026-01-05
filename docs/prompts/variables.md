# Variable Substitution

Prompt templates use `{{variable_name}}` syntax for placeholder substitution.

## Basic Syntax

```
Hello {{name}}!
```

Variables are enclosed in double curly braces.

## Variable Names

### Valid Names

- Alphanumeric characters: `a-z`, `A-Z`, `0-9`
- Underscores: `_`
- Must start with a letter

**Valid examples:**
```
{{name}}
{{file_path}}
{{issueType}}
{{version2}}
{{API_KEY}}
```

### Invalid Names

**Invalid:**
```
{{123name}}     # Starts with number
{{file-path}}   # Contains hyphen
{{issue.type}}  # Contains dot
{{}}            # Empty
```

### Reserved Prefixes

These prefixes are reserved for system use:

| Prefix | Use |
|--------|-----|
| `subcog_` | Subcog system variables |
| `system_` | System-injected values |
| `__` | Internal variables |

**Reserved examples (don't use):**
```
{{subcog_version}}
{{system_time}}
{{__internal}}
```

## Variable Definitions

When saving a template, you can define variables with metadata:

### CLI Definition

```bash
subcog prompt save my-template \
  --content "Review {{file}} for {{issue_type}}" \
  --var-desc "file:File to review" \
  --var-desc "issue_type:Type of issues" \
  --var-default "issue_type:general" \
  --var-required "file"
```

### YAML Definition

```yaml
name: my-template
content: "Review {{file}} for {{issue_type}}"
variables:
  - name: file
    description: File to review
    required: true
  - name: issue_type
    description: Type of issues to focus on
    required: false
    default: general
```

### JSON Definition

```json
{
  "name": "my-template",
  "content": "Review {{file}} for {{issue_type}}",
  "variables": [
    {
      "name": "file",
      "description": "File to review",
      "required": true
    },
    {
      "name": "issue_type",
      "description": "Type of issues to focus on",
      "required": false,
      "default": "general"
    }
  ]
}
```

## Variable Properties

| Property | Type | Description |
|----------|------|-------------|
| `name` | string | Variable name (required) |
| `description` | string | Human-readable description |
| `required` | boolean | Whether value must be provided |
| `default` | string | Default value if not provided |

## Execution

### Providing Values

**CLI:**
```bash
subcog prompt run template --var key=value --var key2=value2
```

**MCP:**
```json
{
  "name": "prompt_run",
  "arguments": {
    "name": "template",
    "variables": {
      "key": "value",
      "key2": "value2"
    }
  }
}
```

### Default Values

If a variable has a default and no value is provided, the default is used:

```yaml
variables:
  - name: env
    default: development
```

```bash
subcog prompt run template
# Uses env=development

subcog prompt run template --var env=production
# Uses env=production
```

### Required Variables

If a required variable is missing, an error is returned:

```yaml
variables:
  - name: file
    required: true
```

```bash
subcog prompt run template
# Error: Missing required variable: file
```

## Substitution Rules

### Simple Substitution

```
Input:  Hello {{name}}!
Values: name=World
Output: Hello World!
```

### Multiple Variables

```
Input:  {{greeting}} {{name}}, welcome to {{place}}!
Values: greeting=Hello, name=Alice, place=Subcog
Output: Hello Alice, welcome to Subcog!
```

### Same Variable Multiple Times

```
Input:  {{name}} said "My name is {{name}}"
Values: name=Bob
Output: Bob said "My name is Bob"
```

### Multiline Content

```
Input:
  ## Review: {{file}}

  {{content}}

  Reviewer: {{reviewer}}

Values: file=main.rs, content=Code looks good, reviewer=Alice
Output:
  ## Review: main.rs

  Code looks good

  Reviewer: Alice
```

## Escaping

If you need literal `{{` in output:

```
Input:  Template syntax is \{\{variable\}\}
Output: Template syntax is {{variable}}
```

## Code Block Exclusion

Variables inside fenced code blocks are treated as documentation examples and **not** extracted as template variables:

````markdown
This prompt uses {{active_variable}} which will be extracted.

```python
# Code example showing syntax
template = "Hello {{example_variable}}"
```

The {{another_variable}} after the block is also extracted.
````

**Extracted variables:** `active_variable`, `another_variable`

**Excluded (in code block):** `example_variable`

### Supported Code Block Syntaxes

| Syntax | Example |
|--------|---------|
| Triple backticks | ` ```language ... ``` ` |
| Triple tildes | `~~~ language ... ~~~` |
| Nested (tildes around backticks) | `~~~ ... ``` ... ``` ... ~~~` |

This allows prompts to include code examples with `{{variable}}` syntax without accidentally treating them as template variables.

## Validation

Templates are validated when saved:

### Checks Performed

1. **Syntax** - Valid `{{...}}` format
2. **Names** - Valid variable names
3. **Reserved** - No reserved prefixes
4. **Consistency** - Variables in content match definitions

### Validation Errors

```
Error: Invalid variable name: {{file-path}}
       Variable names must be alphanumeric with underscores

Error: Reserved variable prefix: {{subcog_id}}
       Variables starting with 'subcog_' are reserved

Error: Undefined variable: {{reviewer}}
       Variable used in content but not defined
```

## Best Practices

1. **Use descriptive names** - `file_path` not `fp`
2. **Add descriptions** - Helps users understand intent
3. **Set sensible defaults** - Reduce required inputs
4. **Mark critical vars required** - Prevent incomplete prompts
5. **Group related vars** - Use common prefixes

## See Also

- [Overview](./overview.md) - Template concepts
- [Formats](formats.md) - Template file formats
- [MCP prompt_run](../mcp/tools.md#prompt_run) - MCP execution
