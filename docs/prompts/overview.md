# Prompt Templates Overview

Prompt templates allow you to create reusable prompts with variable substitution.

## What Are Prompt Templates?

A prompt template is a saved piece of text containing placeholders that get replaced with actual values when executed.

```
Review {{file}} for {{issue_type}} issues
           ▲              ▲
           │              └── Variable: issue_type
           └── Variable: file
```

When run with `file=main.rs` and `issue_type=security`:

```
Review main.rs for security issues
```

## Why Use Templates?

### Consistency

Same prompt structure across uses:
- Team uses consistent review formats
- Standardized documentation patterns
- Repeatable workflows

### Efficiency

Write once, use many times:
- No retyping complex prompts
- Reduce errors from manual entry
- Quick access to common patterns

### Sharing

Share across team and projects:
- Project templates for repo-specific tasks
- User templates for personal preferences
- Org templates for team standards

## Template Components

### Name

Unique identifier (kebab-case):
```
code-review
api-documentation
bug-report-template
```

### Description

Human-readable explanation:
```
Comprehensive code review with security focus
```

### Content

The prompt text with variables:
```
Review {{file}} for {{issue_type}} issues.

Check for:
- Security vulnerabilities
- Performance problems
- Code quality
```

### Variables

Defined placeholders:

```yaml
variables:
  - name: file
    description: File path to review
    required: true
  - name: issue_type
    description: Type of issues
    default: general
    required: false
```

### Tags

For organization and filtering:
```
tags: [review, security, code-quality]
```

### Domain

Where the template is stored:
- `project` - Current repository
- `user` - User-wide
- `org` - Organization-wide

## Template Lifecycle

```
Create          Store           Retrieve        Execute
   │               │                │              │
   ▼               ▼                ▼              ▼
┌─────────┐  ┌──────────┐    ┌──────────┐   ┌──────────┐
│  Write  │→ │  Save to │ →  │  Get by  │ → │ Variable │
│ Content │  │  Storage │    │   Name   │   │  Subst.  │
└─────────┘  └──────────┘    └──────────┘   └──────────┘
     │            │               │              │
     ▼            ▼               ▼              ▼
  Variables    SQLite         Domain         Rendered
  Defined      Storage        Cascade         Output
```

## Quick Examples

### Simple Template

```bash
subcog prompt save greet --content "Hello {{name}}!"
subcog prompt run greet --var name=World
# Output: Hello World!
```

### Template with Defaults

```bash
subcog prompt save deploy \
  --content "Deploy {{service}} to {{env}}" \
  --description "Deployment command"
```

```bash
subcog prompt run deploy --var service=api --var env=production
# Output: Deploy api to production
```

### Template from File

Create `review.yaml`:
```yaml
name: security-review
description: Security-focused code review
content: |
  ## Security Review: {{file}}

  ### OWASP Top 10 Checklist
  - [ ] Injection
  - [ ] Broken Authentication
  - [ ] Sensitive Data Exposure
  ...
variables:
  - name: file
    required: true
tags: [security, review]
```

```bash
subcog prompt save --from-file review.yaml
```

## Comparison with UX Helper Prompts (CLI-only)

| Feature | User Templates | Built-in UX Helper Prompts |
|---------|----------------|----------------------------|
| Customizable | Yes | No |
| Stored | SQLite | Hardcoded |
| Variables | User-defined | Fixed |
| Shareable | Yes | No |
| Domain-scoped | Yes | No |

User templates complement built-in UX helper prompts by allowing customization.

## See Also

- [Variables](variables.md) - Variable syntax and validation
- [Formats](formats.md) - Supported file formats
- [Storage](storage.md) - How templates are stored
