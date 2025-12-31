# subcog prompt

Manage prompt templates.

## Synopsis

```
subcog prompt <COMMAND> [OPTIONS]
```

## Description

The `prompt` command manages reusable prompt templates with variable substitution. Templates can be stored at project, user, or org scope and shared across sessions.

## Subcommands

| Command | Description |
|---------|-------------|
| `save` | Save a new prompt template |
| `list` | List available prompts |
| `get` | Get a specific prompt |
| `run` | Execute a prompt with variables |
| `delete` | Delete a prompt |
| `export` | Export a prompt to file |
| `import` | Import a prompt from file |
| `share` | Share prompt across domains |

---

## prompt save

Save a new prompt template.

### Synopsis

```
subcog prompt save <NAME> [OPTIONS]
```

### Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--content` | `-c` | Prompt content | None |
| `--file` | `-f` | Load from file | None |
| `--description` | `-d` | Description | None |
| `--tags` | `-t` | Comma-separated tags | None |
| `--domain` | | Domain scope | `project` |

### Variable Syntax

Use `{{variable_name}}` for substitution:
- Valid: alphanumeric and underscores
- Reserved: `subcog_*`, `system_*`, `__*`

### Examples

```bash
# Simple prompt
subcog prompt save greet --content "Hello {{name}}!"

# With description
subcog prompt save code-review \
  --content "Review {{file}} for {{issue_type}} issues" \
  --description "Code review template"

# From file
subcog prompt save security-audit --file prompts/security.md

# With tags
subcog prompt save api-design \
  --content "Design API for {{feature}}" \
  --tags "api,design"

# To user domain
subcog prompt save my-template \
  --content "{{content}}" \
  --domain user
```

---

## prompt list

List available prompts.

### Synopsis

```
subcog prompt list [OPTIONS]
```

### Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--domain` | `-d` | Filter by domain | All |
| `--tags` | `-t` | Filter by tags | None |
| `--format` | | Output format (table, json) | `table` |
| `--limit` | `-l` | Maximum results | `50` |

### Examples

```bash
# List all prompts
subcog prompt list

# Filter by domain
subcog prompt list --domain user

# Filter by tags
subcog prompt list --tags review

# JSON output
subcog prompt list --format json
```

Output:
```
NAME            DOMAIN    TAGS              DESCRIPTION
code-review     project   [review,code]     Code review template
security-audit  project   [security]        Security audit checklist
my-template     user      []                Personal template
api-design      org       [api,design]      API design template
```

---

## prompt get

Get a specific prompt.

### Synopsis

```
subcog prompt get <NAME> [OPTIONS]
```

### Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--domain` | `-d` | Domain to search | All (cascade) |
| `--format` | | Output format | `yaml` |

### Domain Cascade

If domain not specified, searches: Project → User → Org

### Examples

```bash
# Get prompt
subcog prompt get code-review

# From specific domain
subcog prompt get code-review --domain user

# JSON format
subcog prompt get code-review --format json
```

Output:
```yaml
name: code-review
description: Code review template
content: |
  Review {{file}} for {{issue_type}} issues.

  Check:
  - Security vulnerabilities
  - Performance issues
  - Code style
variables:
  - name: file
    description: File to review
    required: true
  - name: issue_type
    description: Type of issues
    default: general
tags: [review, code]
domain: project
```

---

## prompt run

Execute a prompt with variable substitution.

### Synopsis

```
subcog prompt run <NAME> [OPTIONS]
```

### Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--var` | `-v` | Variable (key=value), repeatable | None |
| `--domain` | `-d` | Domain to search | All |
| `--output` | `-o` | Output file | stdout |

### Examples

```bash
# Run with variables
subcog prompt run code-review \
  --var file=src/main.rs \
  --var issue_type=security

# Multiple variables
subcog prompt run api-design \
  -v feature=authentication \
  -v method=OAuth2 \
  -v version=v2

# Output to file
subcog prompt run template --var x=1 -o output.md
```

Output:
```
Review src/main.rs for security issues.

Check:
- Security vulnerabilities
- Performance issues
- Code style
```

---

## prompt delete

Delete a prompt.

### Synopsis

```
subcog prompt delete <NAME> [OPTIONS]
```

### Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--domain` | `-d` | Domain (required) | None |
| `--force` | `-f` | Skip confirmation | `false` |

### Examples

```bash
# Delete from project
subcog prompt delete old-template --domain project

# Force delete
subcog prompt delete old-template --domain project --force
```

---

## prompt export

Export a prompt to file.

### Synopsis

```
subcog prompt export <NAME> [OPTIONS]
```

### Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--format` | `-f` | Format (yaml, json, md) | `yaml` |
| `--output` | `-o` | Output file | stdout |

### Examples

```bash
# Export to YAML
subcog prompt export code-review -o code-review.yaml

# Export to JSON
subcog prompt export code-review -f json -o code-review.json

# Export to Markdown
subcog prompt export code-review -f md -o code-review.md
```

---

## prompt import

Import a prompt from file.

### Synopsis

```
subcog prompt import <FILE> [OPTIONS]
```

### Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--domain` | `-d` | Target domain | `project` |
| `--name` | `-n` | Override name | From file |

### Supported Formats

- YAML (`.yaml`, `.yml`)
- JSON (`.json`)
- Markdown (`.md`)
- Plain text (`.txt`)

### Examples

```bash
# Import YAML
subcog prompt import prompts/review.yaml

# Import to user domain
subcog prompt import prompts/review.yaml --domain user

# Override name
subcog prompt import prompts/review.yaml --name my-review
```

---

## prompt share

Share prompt across domains.

### Synopsis

```
subcog prompt share <NAME> --from <DOMAIN> --to <DOMAIN>
```

### Examples

```bash
# Share from project to org
subcog prompt share code-review --from project --to org

# Share from user to project
subcog prompt share my-template --from user --to project
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Error |
| 2 | Invalid arguments |
| 8 | Prompt not found |
| 9 | Validation error |

## See Also

- [Prompt Templates](../prompts/README.md) - Full prompts documentation
- [Variable Substitution](../prompts/variables.md) - Variable syntax
- [MCP prompt_*](../mcp/tools.md#prompt_save) - MCP tools for prompts
