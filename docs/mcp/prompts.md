# MCP Prompts

Subcog provides 11 built-in MCP prompts for common operations and guided workflows.

## Memory Prompts

### subcog_capture

Guided memory capture with namespace selection.

**Arguments:**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `content` | string | Yes | Memory content to capture |

**Example:**

```json
{
 "method": "prompts/get",
 "params": {
 "name": "subcog_capture",
 "arguments": {
 "content": "Decided to use PostgreSQL for primary storage"
 }
 }
}
```

**Returns prompt guiding through:**
1. Namespace selection based on content analysis
2. Tag suggestions
3. Source reference prompt

---

### subcog_recall

Guided memory search with filter suggestions.

**Arguments:**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `query` | string | Yes | Search query |

**Example:**

```json
{
 "method": "prompts/get",
 "params": {
 "name": "subcog_recall",
 "arguments": {
 "query": "database storage"
 }
 }
}
```

**Returns prompt with:**
1. Search results
2. Suggested refinement filters
3. Related topics

---

### subcog_browse

Interactive memory browser with faceted discovery.

**Arguments:**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `filter` | string | No | Filter expression |
| `view` | string | No | `dashboard` or `list` (default: dashboard) |
| `top` | integer | No | Items per facet (default: 10) |

**Dashboard view shows:**
- Tag distribution with counts
- Namespace breakdown
- Recent activity timeline
- Source file clusters

---

### subcog_list (DEPRECATED)

> **️ Deprecated**: Use `subcog_recall` without a query parameter instead. The `subcog_recall` tool now supports listing all memories when query is omitted.

Formatted memory listing for export.

**Arguments:**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `filter` | string | No | Filter expression |
| `format` | string | No | `table`, `json`, `markdown` (default: table) |
| `limit` | integer | No | Maximum results (default: 50) |

---

### subcog_tutorial

Interactive tutorial for new users.

**Arguments:** None

**Returns comprehensive guide covering:**
1. Core concepts
2. Capturing memories
3. Searching and recalling
4. Integration with Claude Code
5. Best practices

---

## Search Prompts

### intent_search

Search with intent-aware namespace weighting.

**Arguments:**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `query` | string | Yes | Natural language query |
| `intent` | string | No | Override intent type |

**Intent Types:**
- `howto` - Prioritizes patterns, learnings
- `location` - Prioritizes apis, config
- `explanation` - Prioritizes decisions, context
- `comparison` - Prioritizes decisions, patterns
- `troubleshoot` - Prioritizes blockers, learnings
- `general` - Balanced weights

---

### query_suggest

Get filter suggestions for a query.

**Arguments:**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `query` | string | Yes | Initial query |

**Returns:**
- Suggested namespace filters
- Relevant tag filters
- Time range suggestions

---

### discover

Explore memories by topic or tag.

**Arguments:**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `topic` | string | No | Topic to explore |
| `tag` | string | No | Tag to explore |

**Returns:**
- Related memories
- Connected topics
- Suggested next queries

---

## Content Generation Prompts

### generate_decision

Generate a well-structured decision record.

**Arguments:**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `decision` | string | Yes | Brief decision statement |
| `context` | string | No | Background context |

**Returns ADR-style decision record:**
```markdown
## Decision: {decision}

### Context
{analyzed context}

### Decision
{expanded decision}

### Consequences
- Positive:...
- Negative:...

### Alternatives Considered
-...
```

---

### generate_tutorial

Generate a tutorial from learnings.

**Arguments:**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `topic` | string | Yes | Topic for tutorial |
| `level` | string | No | `beginner`, `intermediate`, `advanced` |

**Synthesizes relevant memories into tutorial format.**

---

### context_capture

Capture rich context from a conversation.

**Arguments:**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `conversation` | string | Yes | Conversation excerpt |

**Analyzes conversation and suggests:**
- Decisions to capture
- Patterns identified
- Learnings discovered
- Blockers mentioned

---

## Using Prompts

### Get Prompt Content

```json
{
 "jsonrpc": "2.0",
 "id": 1,
 "method": "prompts/get",
 "params": {
 "name": "subcog_capture",
 "arguments": {
 "content": "Use RRF for hybrid search fusion"
 }
 }
}
```

### List Available Prompts

```json
{
 "jsonrpc": "2.0",
 "id": 1,
 "method": "prompts/list"
}
```

**Response:**

```json
{
 "prompts": [
 {
 "name": "subcog_capture",
 "description": "Guided memory capture",
 "arguments": [
 {"name": "content", "required": true}
 ]
 },
 {
 "name": "subcog_recall",
 "description": "Guided memory search",
 "arguments": [
 {"name": "query", "required": true}
 ]
 }
 ]
}
```

---

## Prompt vs Tool

| Use Prompt When | Use Tool When |
|-----------------|---------------|
| Guided workflow needed | Direct operation |
| Suggestions helpful | Exact parameters known |
| Learning the system | Automation/scripts |
| Exploring options | Specific action |

**Example - Prompt for guidance:**
```
"I want to capture something about our database choice"
-> Use subcog_capture prompt for namespace suggestion
```

**Example - Tool for direct action:**
```
"Capture this to decisions: Use PostgreSQL"
-> Use subcog_capture tool directly
```

---

## Custom Prompts

User-defined prompts are managed separately. See:
- [subcog_prompts](tools.md#subcog_prompts) tool (v0.8.0+ consolidated API)
- [Prompt Templates](../prompts/README.md) documentation
- `subcog://_prompts` resources

> **Note**: The legacy `prompt_save`, `prompt_list`, `prompt_get`, `prompt_run`, and `prompt_delete` tools are deprecated. Use `subcog_prompts` with the appropriate `action` parameter instead.

---

## See Also

- [Tools](tools.md) - MCP tools reference
- [Resources](resources.md) - MCP resources reference
- [Prompt Templates](../prompts/README.md) - User-defined prompts
