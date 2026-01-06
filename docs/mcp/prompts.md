# UX Helper Prompts (CLI-only)

Subcog ships built-in UX helper prompts for interactive workflows. These are **not**
exposed via MCP. Use the CLI to list and run them:

```bash
subcog prompt list --tags ux-helper
subcog prompt run subcog_browse --interactive
```

## Available Prompts

Canonical names are shown below; aliases are listed in parentheses.

| Prompt | Description | Variables |
|--------|-------------|-----------|
| `subcog_tutorial` (`subcog`) | Interactive learning guide | `familiarity`, `focus` |
| `subcog_generate_tutorial` (`generate_tutorial`) | Generate tutorial using memories | `topic`, `level`, `format` |
| `subcog_capture_assistant` (`subcog_capture`) | Suggest captures and namespaces | `context` |
| `subcog_review` | Review or summarize memories | `namespace`, `action` |
| `subcog_document_decision` (`generate_decision`) | Structure an architecture decision | `decision`, `alternatives` |
| `subcog_search_help` (`subcog_recall`) | Craft effective search queries | `goal` |
| `subcog_browse` | Memory browser dashboard | `filter`, `view`, `top` |
| `subcog_list` | Formatted memory listing | `filter`, `format`, `limit` |
| `subcog_intent_search` (`intent_search`) | Intent-aware search workflow | `query`, `context`, `intent` |
| `subcog_query_suggest` (`query_suggest`) | Query suggestions for exploration | `topic`, `namespace` |
| `subcog_context_capture` (`context_capture`) | Context-aware capture suggestions | `conversation`, `threshold` |
| `subcog_discover` (`discover`) | Explore related memories and topics | `start`, `depth`, `topic`, `tag` |

## Examples

```bash
# Explore memories with a dashboard
subcog prompt run subcog_browse --var filter="ns:decisions tag:database"

# Get search query suggestions
subcog prompt run subcog_query_suggest --var topic="authentication"

# Document a decision
subcog prompt run subcog_document_decision --var decision="Use SQLite for dev"
```
