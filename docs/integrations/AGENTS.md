# Subcog Memory System Integration

This file configures AI coding assistants to use Subcog for persistent memory across sessions. Place this file at `~/.codex/AGENTS.md` (global) or in your project root (project-specific).

## Memory System Overview

You have access to **Subcog**, a persistent memory system via MCP tools. Subcog stores decisions, patterns, learnings, and context across coding sessions, enabling you to recall prior knowledge and maintain continuity.

**MCP Server**: Ensure `subcog serve` is configured in your MCP settings.

---

## Session Protocol

### On Session Start

**ALWAYS** begin each session by checking memory status:

```
Use the subcog_status tool to check memory system status.
```

If this is your first interaction in a project, also run:
```
Use the subcog_recall tool with query "project setup OR architecture OR conventions" to surface relevant context.
```

### During the Session

Follow these memory protocols throughout:

#### MCP Tool Invocation Rule

If the user types a token that matches an MCP tool name (for example,
`subcog_status`, `subcog_recall`, or `prompt_list`), treat it as a request to run
that MCP tool, not a shell command, unless the user explicitly says "shell" or
"run in terminal".

#### 1. CAPTURE Protocol (Save Important Information)

**When to capture** - Immediately save when you detect these signals:

| Signal Type | Trigger Phrases | Namespace |
|------------|-----------------|-----------|
| **Decisions** | "we'll use", "decided", "choosing", "going with", "let's go with" | `decisions` |
| **Patterns** | "pattern", "convention", "always", "never", "rule", "standard" | `patterns` |
| **Learnings** | "TIL", "turns out", "discovered", "realized", "gotcha", "learned" | `learnings` |
| **Blockers/Solutions** | "fixed", "solved", "the issue was", "workaround", "resolved" | `context` |
| **Tech Debt** | "TODO", "FIXME", "temporary", "need to refactor", "tech debt" | `tech-debt` |
| **API Behaviors** | "API returns", "endpoint behavior", "response format" | `apis` |
| **Configuration** | "config", "environment", "settings", "env var" | `config` |
| **Security** | "vulnerability", "security", "auth", "permission" | `security` |
| **Performance** | "slow", "optimization", "cache", "bottleneck" | `performance` |
| **Testing** | "test", "coverage", "fixture", "mock" | `testing` |

**Explicit capture commands** - Always honor:
- "capture this", "remember this", "save to memory"
- "@subcog capture", "subcog remember"

**How to capture**:
```
Use the subcog_capture tool with:
- content: The information to save (be specific and include context)
- namespace: One of the 10 namespaces above
- tags: Relevant keywords for later retrieval
- source: File path or URL if applicable
```

**Capture quality guidelines**:
- Include the "why" not just the "what"
- Add relevant file paths or code references
- Use descriptive tags for discoverability
- Keep content concise but complete (aim for 1-3 paragraphs)

#### 2. RECALL Protocol (Search for Prior Knowledge)

**When to recall** - Search memory when you detect these intents:

| Intent Type | Trigger Phrases | Search Strategy |
|------------|-----------------|-----------------|
| **HowTo** | "how do I", "how to", "implement", "create" | Search patterns + learnings |
| **Location** | "where is", "find", "locate" | Search with file paths, project terms |
| **Explanation** | "what is", "explain", "describe" | Search decisions + context |
| **Comparison** | "difference between", "vs", "compare" | Search decisions + patterns |
| **Troubleshoot** | "error", "fix", "not working", "debug" | Search context + learnings |

**Explicit recall commands** - Always honor:
- "recall", "search memories", "what do we know about"
- "find memories", "check memory for"

**How to recall**:
```
Use the subcog_recall tool with:
- query: Natural language search query
- filter: Optional GitHub-style filters (ns:decisions tag:rust since:7d)
- detail: "medium" for summaries, "everything" for full content
- limit: 5-10 for focused results, 15-20 for broad exploration
```

**Recall before acting**:
- Before implementing a feature: recall related decisions and patterns
- Before debugging: recall similar issues and solutions
- Before architectural changes: recall design decisions

#### 3. Tool-Context Protocol

After using file or code tools, consider whether related memories would help:

| Tool Used | Memory Query Strategy |
|-----------|----------------------|
| Read/Write file | Recall by file path or module name |
| Search/Grep | Recall by search pattern topic |
| Bash command | Recall by command or tool name |
| LSP operations | Recall by symbol or type name |

---

## Namespace Reference

| Namespace | Purpose | Examples |
|-----------|---------|----------|
| `decisions` | Architectural and design choices | "Using PostgreSQL for storage", "Chose React over Vue" |
| `patterns` | Code conventions and standards | "Always use snake_case for Python", "Error handling pattern" |
| `learnings` | Insights and discoveries | "TIL: Rust lifetimes work like X", "Gotcha with async/await" |
| `context` | Project-specific knowledge | "Auth flow explanation", "Data pipeline overview" |
| `tech-debt` | Known issues to address later | "TODO: Refactor auth module", "FIXME: N+1 query" |
| `apis` | External API behaviors | "Stripe webhook format", "GitHub API rate limits" |
| `config` | Environment and settings | "Required env vars", "Docker compose setup" |
| `security` | Security considerations | "Auth requirements", "Data encryption approach" |
| `performance` | Optimization notes | "Query optimization needed", "Caching strategy" |
| `testing` | Test-related knowledge | "Test fixtures location", "Mocking patterns" |

---

## Session End Checklist

Before ending a session, review and capture:

1. **Decisions made**: Any architectural or design choices?
2. **Patterns established**: Any new conventions or standards?
3. **Learnings discovered**: Any "aha" moments or gotchas?
4. **Blockers resolved**: Any solutions worth remembering?
5. **Tech debt created**: Any TODOs or shortcuts taken?

If you made significant progress, run:
```
Use the subcog_sync tool with direction "push" to sync memories to remote.
```

---

## Advanced Features

### Prompt Templates

Save reusable prompts for common tasks:
```
Use the prompt_save tool with:
- name: "code-review"
- content: "Review {{file}} for {{focus_area}} issues"
- tags: ["review", "quality"]
```

Run saved prompts:
```
Use the prompt_run tool with:
- name: "code-review"
- variables: {"file": "src/main.rs", "focus_area": "security"}
```

### Memory Consolidation

Periodically consolidate related memories:
```
Use the subcog_consolidate tool with:
- namespace: "learnings"
- strategy: "merge" (or "summarize", "dedupe")
```

### Filter Syntax

Use GitHub-style filters for precise searches:
- `ns:decisions` - Filter by namespace
- `tag:rust` - Filter by tag
- `-tag:deprecated` - Exclude tag
- `since:7d` - Recent memories (7 days)
- `source:src/*` - Filter by source path

Example: `ns:decisions tag:database -tag:deprecated since:30d`

---

## Quality Guidelines

### Good Captures

```
Content: "Decided to use SQLite for local development instead of PostgreSQL
to simplify onboarding. Production will still use PostgreSQL. This affects
the docker-compose.yml and requires DATABASE_URL to be set differently."

Namespace: decisions
Tags: ["database", "sqlite", "postgresql", "development", "docker"]
Source: "docker-compose.yml"
```

### Poor Captures (Avoid)

```
Content: "Use SQLite"
Namespace: decisions
Tags: []
```

The good capture includes context, rationale, and implications. The poor capture lacks actionable detail.

---

## Troubleshooting

**Memory not found?**
- Try broader search terms
- Use `detail: "everything"` for full content
- Check `subcog_status` for memory counts

**Too many results?**
- Add namespace filter: `ns:decisions`
- Add time filter: `since:7d`
- Use more specific query terms

**Duplicate memories?**
- Use `subcog_consolidate` with `strategy: "dedupe"`
- Add more specific tags when capturing
