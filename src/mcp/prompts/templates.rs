//! Prompt template content strings.
//!
//! Contains all the static content used in prompt generation.

// Tutorial content
// Note: These strings contain double quotes, so we use r"..."# syntax

pub const TUTORIAL_OVERVIEW: &str = r#"
## What is Subcog?

Subcog is a **persistent memory system** for AI coding assistants. It helps you:

- **Remember decisions** you've made across sessions
- **Recall learnings** when they're relevant
- **Build up patterns** and best practices over time
- **Maintain context** even after compaction

## Key Concepts

1. **Memories**: Pieces of knowledge captured from your coding sessions
2. **Namespaces**: Categories like `decisions`, `patterns`, `learnings`
3. **Search**: Hybrid semantic + text search to find relevant memories
4. **Hooks**: Automatic integration with Claude Code

## Quick Start

```bash
# Capture a decision
subcog capture --namespace decisions "Use PostgreSQL for storage"

# Search for memories
subcog recall "database choice"

# Check status
subcog status
```

Would you like me to dive deeper into any of these areas?
"#;

pub const TUTORIAL_CAPTURE: &str = r#"
## Capturing Memories

Memories are the core unit of Subcog. Here's how to capture them effectively:

### Basic Capture

```bash
subcog capture --namespace decisions "Use PostgreSQL for primary storage"
```

### With Metadata

```bash
subcog capture --namespace patterns \
  --tags "rust,error-handling" \
  --source "src/main.rs:42" \
  "Always use thiserror for custom error types"
```

### What to Capture

- **Decisions**: Why you chose X over Y
- **Patterns**: Recurring approaches that work
- **Learnings**: "Aha!" moments and gotchas
- **Context**: Important background information

### Best Practices

1. Be specific - include the "why"
2. Add relevant tags for searchability
3. Reference source files when applicable
4. Use the right namespace
"#;

pub const TUTORIAL_SEARCH: &str = r#"
## Searching Memories

Subcog uses hybrid search combining semantic understanding with keyword matching.

### Basic Search

```bash
subcog recall "database storage decision"
```

### Search Modes

- **Hybrid** (default): Best of both worlds
- **Vector**: Pure semantic similarity
- **Text**: Traditional keyword matching

### Filtering

```bash
# By namespace
subcog recall --namespace decisions "storage"

# Limit results
subcog recall --limit 5 "API design"
```

### Tips for Better Results

1. Use natural language queries
2. Include context words
3. Try different search modes for different needs
4. Review scores to gauge relevance
"#;

pub const TUTORIAL_NAMESPACES: &str = r#"
## Understanding Namespaces

Namespaces organize memories by type:

| Namespace | Use For |
|-----------|---------|
| `decisions` | Architectural choices, "we decided to..." |
| `patterns` | Recurring solutions, conventions |
| `learnings` | Debugging insights, TILs |
| `context` | Background info, constraints |
| `tech-debt` | Future improvements needed |
| `apis` | Endpoint docs, contracts |
| `config` | Environment, settings |
| `security` | Auth patterns, vulnerabilities |
| `performance` | Optimization notes |
| `testing` | Test strategies, edge cases |

### Choosing the Right Namespace

- **Decision language** ("let's use", "we chose") -> `decisions`
- **Pattern language** ("always", "never", "when X do Y") -> `patterns`
- **Learning language** ("TIL", "gotcha", "realized") -> `learnings`
- **Context language** ("because", "constraint", "requirement") -> `context`
"#;

pub const TUTORIAL_WORKFLOWS: &str = r#"
## Integration Workflows

Subcog integrates with Claude Code through hooks:

### Available Hooks

1. **SessionStart**: Injects relevant context
2. **UserPromptSubmit**: Detects capture signals
3. **PostToolUse**: Surfaces related memories
4. **PreCompact**: Auto-captures before compaction
5. **Stop**: Session summary and sync

### Configuration

Add to `~/.claude/settings.json`:

```json
{
  "hooks": {
    "SessionStart": [{ "command": "subcog hook session-start" }],
    "UserPromptSubmit": [{ "command": "subcog hook user-prompt-submit" }],
    "Stop": [{ "command": "subcog hook stop" }]
  }
}
```

### MCP Server

For Claude Desktop:

```json
{
  "mcpServers": {
    "subcog": {
      "command": "subcog",
      "args": ["serve"]
    }
  }
}
```
"#;

pub const TUTORIAL_BEST_PRACTICES: &str = r"
## Best Practices

### Capture Discipline

1. **Capture decisions when made** - don't wait
2. **Include rationale** - why, not just what
3. **Be searchable** - think about future queries
4. **Tag consistently** - use existing tags when possible

### Memory Hygiene

1. **Review periodically** - consolidate duplicates
2. **Archive outdated** - don't delete, archive
3. **Update when wrong** - memories can be superseded

### Search Effectively

1. **Start broad, narrow down** - use filters progressively
2. **Try multiple modes** - hybrid, vector, text
3. **Trust the scores** - >0.7 is usually relevant

### Integration Tips

1. **Enable hooks** - let Subcog work automatically
2. **Check context** - review what's being injected
3. **Sync regularly** - keep memories backed up
";

pub const GENERATE_TUTORIAL_STRUCTURE: &str = r"
## Tutorial Structure Guide

Organize your tutorial with these sections:

1. **Overview** - What this topic is about and why it matters
2. **Prerequisites** - What the reader should know beforehand
3. **Core Concepts** - Main ideas explained clearly
4. **Practical Examples** - Working code or scenarios
5. **Common Pitfalls** - Mistakes to avoid (informed by learnings memories)
6. **Best Practices** - Patterns and conventions (informed by patterns memories)
7. **Summary** - Key takeaways
8. **References** - Links to relevant memories and resources
";

pub const GENERATE_TUTORIAL_RESPONSE: &str = r"
I'll generate a comprehensive tutorial on this topic by:

1. Searching the memory collection for relevant decisions, patterns, learnings, and context
2. Organizing the content according to the specified format
3. Incorporating real insights from the project's history
4. Adding practical examples and code snippets where applicable

Let me start by searching for memories related to this topic...
";

pub const CAPTURE_ASSISTANT_SYSTEM: &str = r"
I'll analyze the context and suggest memories to capture. For each suggestion, I'll provide:

1. **Content**: The memory text to capture
2. **Namespace**: The appropriate category
3. **Tags**: Relevant keywords for searchability
4. **Rationale**: Why this should be captured

Let me analyze the context you provided...
";

pub const SEARCH_HELP_SYSTEM: &str = r#"
I'll help you craft effective search queries. Subcog supports:

**Hybrid Search (default)**
- Combines semantic understanding with keyword matching
- Best for natural language queries
- Example: "how we handle authentication errors"

**Vector Search**
- Pure semantic similarity
- Best for conceptual queries
- Example: "patterns for resilient services"

**Text Search**
- Traditional BM25 keyword matching
- Best for exact terms
- Example: "PostgreSQL"

Let me suggest some queries for your goal...
"#;

pub const BROWSE_DASHBOARD_INSTRUCTIONS: &str = r"
## Dashboard Layout

Present the data in this format:

```
┌─────────────────────────────────────────────────────────────────┐
│  SUBCOG MEMORY BROWSER                           {count} memories│
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  NAMESPACES                          TAGS (top N)               │
│  ───────────                         ──────────────             │
│  {namespace} [{count}] {bar}         {tag} [{count}] {bar}      │
│  ...                                 ...                        │
│                                                                 │
│  TIME                                STATUS                     │
│  ────                                ──────                     │
│  today     [{count}]                 active   [{count}]         │
│  this week [{count}]                 archived [{count}]         │
│  ...                                                            │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Filter Syntax Reference

| Filter | Meaning | Example |
|--------|---------|---------|
| `ns:X` | namespace equals | `ns:decisions` |
| `tag:X` | has tag | `tag:rust` |
| `tag:X,Y` | has any tag (OR) | `tag:rust,mcp` |
| `tag:X tag:Y` | has all tags (AND) | `tag:rust tag:error` |
| `-tag:X` | exclude tag | `-tag:test` |
| `tag:*X` | tag wildcard | `tag:*-testing` |
| `since:Nd` | created in last N days | `since:7d` |
| `source:X` | source matches | `source:src/*` |
| `status:X` | status equals | `status:archived` |

Show example filter commands the user can use to drill down.
";

pub const BROWSE_SYSTEM_RESPONSE: &str = r"
I'll create a memory browser dashboard for you. Let me fetch the memories using `mcp__plugin_subcog_subcog__subcog_recall`.

I'll call the tool with the specified filter to get server-side filtered results, then compute:
1. Namespace distribution with counts
2. Tag frequency (top N most common)
3. Time-based grouping (today, this week, this month, older)
4. Status breakdown (active, archived)

I'll present this as a visual dashboard with ASCII bar charts showing relative proportions.
";

pub const LIST_FORMAT_INSTRUCTIONS: &str = r"
## URN Format

Rich URN encodes scope, namespace, and ID:
```
subcog://{scope}/{namespace}/{id}
```
Examples:
- `subcog://project/decisions/abc123...` - project-scoped decision
- `subcog://org/acme/patterns/def456...` - org-scoped pattern
- `subcog://acme/myrepo/learnings/ghi789...` - repo-scoped learning

## Output Formats

### Table Format (default)
Present results directly from `subcog_recall` output. Each line shows:
```
{n}. subcog://{scope}/{namespace}/{id} | {score} [{tags}]
   {content_summary}
```

Group by namespace with counts when helpful.

### Compact Format
```
subcog://{scope}/{namespace}/{id} [{tags}]
```

### Detailed Format
```
### subcog://{scope}/{namespace}/{id}
- **Score**: {score}
- **Tags**: tag1, tag2
- **Source**: {source}
- **Content**: {full_content}
```

## Filter Syntax

- `ns:decisions` - filter by namespace
- `tag:rust` - filter by tag
- `tag:rust,mcp` - OR filter (must have ANY)
- `tag:rust tag:error` - AND filter (must have ALL)
- `-tag:test` - exclude tag
- `since:7d` - time filter
- `source:src/*` - source pattern
- `status:active` - status filter
";

// Phase 4: Intent-aware prompt constants

pub const INTENT_SEARCH_INSTRUCTIONS: &str = r#"
## Intent-Aware Search

I'll analyze your query to determine the best search approach:

**Intent Detection**:
1. **Factual lookup**: "What was the decision about X?" → Direct search
2. **Exploration**: "How do we handle X?" → Broader semantic search
3. **Troubleshooting**: "Why is X failing?" → Include patterns, learnings
4. **Context gathering**: "What do we know about X?" → Multi-namespace search

**Search Strategy**:
- Use `mcp__plugin_subcog_subcog__subcog_recall` with appropriate mode based on intent
- Apply namespace filters when intent is clear
- Include related terms for broader exploration

**Tools to use**:
```json
{ "query": "<refined_query>", "mode": "hybrid", "limit": 10, "detail": "medium" }
```

For troubleshooting queries, also check:
- `ns:learnings` for past debugging insights
- `ns:patterns` for established approaches
- `ns:decisions` for architectural context
"#;

pub const INTENT_SEARCH_RESPONSE: &str = r"
I'll analyze your query to understand your intent and craft the most effective search strategy.

Let me:
1. Identify the type of information you're looking for
2. Determine relevant namespaces to search
3. Refine the query for optimal results
4. Search using the appropriate mode

I'll call `mcp__plugin_subcog_subcog__subcog_recall` with the refined query and present the results organized by relevance.
";

pub const QUERY_SUGGEST_INSTRUCTIONS: &str = r#"
## Query Suggestions

Help the user discover what's in their memory collection.

**Exploration Strategies**:
1. **Topic-based**: Use `subcog://topics` resource to see available topics
2. **Namespace-based**: List what's in each namespace
3. **Tag-based**: Find common tags and their distributions
4. **Time-based**: See recent vs. older memories

**Resources to use**:
- Read `subcog://topics` for topic overview
- Use `mcp__plugin_subcog_subcog__subcog_recall` with `*` query to browse all
- Apply `ns:X` filter to explore specific namespaces

**Suggested queries based on common needs**:
- "What decisions have we made about <topic>?"
- "Show me patterns for <domain>"
- "What did we learn from <issue>?"
- "Context for <feature>"
"#;

pub const QUERY_SUGGEST_RESPONSE: &str = r"
I'll help you explore your memory collection. Let me:

1. Check available topics using the `subcog://topics` resource
2. Analyze namespace distribution
3. Identify frequently tagged concepts
4. Suggest relevant queries for your focus area

Based on what I find, I'll provide:
- Specific search queries to try
- Namespaces worth exploring
- Related topics you might not have considered
";

pub const CONTEXT_CAPTURE_INSTRUCTIONS: &str = r#"
## Context-Aware Capture Analysis

Analyze the provided context to identify capture-worthy content.

**Capture Signals to look for**:
- Decision language: "let's use", "we decided", "going with"
- Pattern language: "always", "never", "when X do Y", "the pattern is"
- Learning language: "TIL", "gotcha", "realized", "the issue was"
- Context language: "because", "constraint", "requirement", "the reason"

**For each suggestion, provide**:
```
Namespace: <appropriate namespace>
Content: <memory text>
Tags: <comma-separated tags>
Confidence: <0.0-1.0>
Rationale: <why this should be captured>
```

**Filtering rules**:
- Only suggest if confidence >= threshold
- Skip purely mechanical/trivial content
- Prefer actionable insights over raw observations
- Dedupe against what might already exist
"#;

pub const CONTEXT_CAPTURE_RESPONSE: &str = r"
I'll analyze the conversation to identify valuable memories worth capturing.

For each potential memory, I'll:
1. Classify the type (decision, pattern, learning, context)
2. Extract the key insight
3. Suggest appropriate tags
4. Estimate confidence level
5. Explain why it's worth capturing

I'll filter suggestions below your confidence threshold and focus on actionable, reusable knowledge.
";

pub const DISCOVER_INSTRUCTIONS: &str = r"
## Memory Discovery & Navigation

Explore the memory graph through related topics and connections.

**Discovery modes**:
1. **From topic**: Find memories about a specific topic, then show related topics
2. **From memory**: Given a memory ID, find semantically similar memories
3. **Overview**: Show top topics across namespaces

**Resources to use**:
- `subcog://topics` for topic listing
- `subcog://topics/{topic}` for specific topic drill-down
- `subcog://search?q=X` for similarity exploration

**Visualization**:
Present discoveries as a navigable tree:
```
Starting Point: {topic or memory}
├─ Direct Matches (N memories)
│   ├─ memory1: {summary}
│   └─ memory2: {summary}
└─ Related Topics
    ├─ {related_topic_1} (M memories)
    └─ {related_topic_2} (K memories)
```

For each hop, show 3-5 most relevant items.
";

pub const DISCOVER_RESPONSE: &str = r"
I'll explore your memory collection to find connections and related topics.

Starting with your specified point (or an overview if none given), I'll:
1. Find directly matching memories
2. Identify related topics based on tags and content
3. Navigate to connected concepts
4. Present a navigable tree of discoveries

Each hop shows the most relevant items, up to your specified depth. I'll highlight interesting connections between seemingly unrelated topics.
";
