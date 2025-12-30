---
description: Search persistent memories using semantic, hybrid, or text search
allowed-tools: mcp__subcog__subcog_recall, mcp__subcog__subcog_namespaces, Bash
argument-hint: "<query> [--namespace <ns>] [--mode hybrid|vector|text] [--limit N]"
---

# /subcog:recall

Search the memory system for relevant decisions, learnings, patterns, and context.

## Usage

```
/subcog:recall "database storage decision"
/subcog:recall --namespace decisions "storage"
/subcog:recall --mode vector "error handling patterns"
/subcog:recall --limit 5 "API design"
```

## Arguments

<arguments>
1. **query** (required): Natural language search query
2. **--namespace**: Filter by memory category (decisions, patterns, learnings, etc.)
3. **--mode**: Search mode
   - `hybrid` (default): Combines semantic + keyword (best for most queries)
   - `vector`: Pure semantic similarity (best for conceptual searches)
   - `text`: Traditional BM25 keyword matching (best for exact terms)
4. **--limit**: Maximum results (default: 10, max: 50)
</arguments>

## Execution Strategy

<strategy>
**MCP-First Approach:**
1. Try `mcp__subcog__subcog_recall` first
2. If MCP unavailable, fallback to CLI: `subcog recall "{{query}}" --mode {{mode}} --limit {{limit}}`

**Result Interpretation:**
- Score 0.9+: Very high relevance (likely exact match)
- Score 0.7-0.9: Good relevance (closely related)
- Score 0.5-0.7: Moderate relevance (broader context)
- Score <0.5: Low relevance (may be tangential)
</strategy>

## Search Tips

<tips>
**For decisions:**
```
/subcog:recall --namespace decisions "storage backend"
```

**For patterns:**
```
/subcog:recall --mode vector "resilient service patterns"
```

**For debugging help:**
```
/subcog:recall --namespace learnings "gotcha"
```

**For exact terms:**
```
/subcog:recall --mode text "PostgreSQL"
```
</tips>

## Examples

<examples>
**Find storage decisions:**
```
/subcog:recall "database storage decision"
```

**Search learnings about Rust:**
```
/subcog:recall --namespace learnings "Rust async"
```

**Semantic search for patterns:**
```
/subcog:recall --mode vector "how to handle errors gracefully"
```
</examples>
