# Memory Capture Skill

Capture decisions, learnings, patterns, and context as persistent memories that survive across sessions.

## Trigger Phrases

- "remember this", "capture this", "save this decision"
- "document that", "log this pattern", "note this learning"
- "TIL", "gotcha", "we decided", "going with"
- "this is important", "don't forget"

## Quick Reference

| Namespace | Use When | Signal Words |
|-----------|----------|--------------|
| `decisions` | Making architectural or design choices | "decided", "chose", "going with" |
| `patterns` | Establishing recurring practices | "always", "never", "when X do Y" |
| `learnings` | Discovering something new | "TIL", "gotcha", "discovered" |
| `context` | Recording background information | "because", "constraint", "requirement" |
| `tech-debt` | Noting future work | "TODO", "FIXME", "temporary" |
| `apis` | Documenting API contracts | "endpoint", "API", "schema" |
| `config` | Recording configuration details | "config", "environment", "setting" |
| `security` | Security-related information | "auth", "vulnerability", "permission" |
| `performance` | Performance insights | "optimization", "benchmark", "latency" |
| `testing` | Testing strategies | "edge case", "fixture", "coverage" |

## Execution Strategy

<strategy>
**MCP-First Approach:**
1. Use `mcp__subcog__subcog_capture` tool when available
2. Fall back to CLI: `subcog capture --namespace <ns> "<content>"`

**Capture Quality Guidelines:**
- Include context and rationale, not just the decision
- Add relevant tags for discoverability
- Reference related files or components
- Keep content concise but complete
</strategy>

## Interactive Capture Workflow

<workflow>
When capturing memories:

1. **Detect capture signals** in user's message
2. **Identify the namespace** from context and signal words
3. **Extract the core insight** - what should be remembered?
4. **Enhance with context** - why is this important?
5. **Suggest tags** based on content and current work
6. **Confirm with user** using AskUserQuestion if uncertain
7. **Execute capture** via MCP tool or CLI
8. **Verify success** and report the memory ID
</workflow>

## Capture Templates

<templates>
**For decisions:**
```
[Decision]: {what was decided}
[Context]: {why this decision was needed}
[Alternatives]: {what else was considered}
[Rationale]: {why this choice over alternatives}
```

**For learnings:**
```
[Discovery]: {what was learned}
[Situation]: {when/how it was discovered}
[Impact]: {why it matters}
```

**For patterns:**
```
[Pattern]: {the practice or convention}
[When]: {situations where it applies}
[Why]: {benefits of following this pattern}
```
</templates>

## MCP Tool Reference

<mcp>
**Tool:** `mcp__subcog__subcog_capture`

**Parameters:**
- `namespace` (required): One of the 10 namespaces
- `content` (required): The memory content
- `tags` (optional): Array of tags for discoverability
- `source` (optional): Source reference (file path, URL, etc.)
- `priority` (optional): 1-5, higher = more important

**Returns:**
- `memory_id`: Unique identifier
- `urn`: Memory URN for reference
- `timestamp`: Capture time
</mcp>
