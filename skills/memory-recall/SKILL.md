# Memory Recall Skill

Search and surface relevant memories to inform current work with decisions, patterns, and learnings from past sessions.

## Trigger Phrases

- "what did we decide about", "how do we handle"
- "find memories about", "search for", "recall"
- "what's our approach to", "what patterns do we use"
- "any gotchas with", "lessons learned about"
- "previous decisions on", "remind me"

## Quick Reference

| Search Mode | Best For | Example |
|-------------|----------|---------|
| `hybrid` (default) | General queries, balanced results | "database storage decision" |
| `vector` | Conceptual similarity, fuzzy matching | "how to handle errors gracefully" |
| `text` | Exact terms, specific keywords | "PostgreSQL" |

## Execution Strategy

<strategy>
**MCP-First Approach:**
1. Use `mcp__subcog__subcog_recall` tool when available
2. Fall back to CLI: `subcog recall "<query>" --mode <mode> --limit <n>`

**Search Quality Guidelines:**
- Start broad, then narrow with namespace filters
- Use `vector` mode for conceptual searches
- Use `text` mode when you know exact terms
- `hybrid` mode (default) works best for most queries
</strategy>

## Intelligent Search Workflow

<workflow>
When searching memories:

1. **Understand the intent** - what does the user need to know?
2. **Identify keywords** - extract searchable terms
3. **Choose search mode** based on query type
4. **Apply namespace filter** if domain is clear
5. **Execute search** via MCP tool or CLI
6. **Interpret results** - explain relevance scores
7. **Synthesize findings** - summarize key insights
</workflow>

## Score Interpretation

<scores>
| Score Range | Meaning | Action |
|-------------|---------|--------|
| 0.9 - 1.0 | Exact or near-exact match | High confidence |
| 0.7 - 0.9 | Strong relevance | Good match |
| 0.5 - 0.7 | Moderate relevance | Related but may need refinement |
| < 0.5 | Low relevance | Try different query |
</scores>

## Search Strategies by Use Case

<strategies>
**Finding a specific decision:**
```
Mode: text or hybrid
Namespace: decisions
Example: "PostgreSQL storage decision"
```

**Finding patterns for a domain:**
```
Mode: vector (conceptual similarity)
Namespace: patterns
Example: "how to handle API errors gracefully"
```

**Debugging help (gotchas):**
```
Mode: hybrid
Namespace: learnings
Example: "authentication token refresh issues"
```
</strategies>

## MCP Tool Reference

<mcp>
**Tool:** `mcp__subcog__subcog_recall`

**Parameters:**
- `query` (required): Natural language search query
- `mode` (optional): "hybrid" (default), "vector", or "text"
- `namespace` (optional): Filter to specific namespace
- `limit` (optional): Maximum results (default: 10, max: 50)

**Returns:**
- Array of `MemoryResult` objects with memory_id, namespace, content, score, tags, timestamp
</mcp>
