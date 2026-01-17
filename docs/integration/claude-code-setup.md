# Subcog Integration Guide for Claude Code

This guide provides instructions for integrating Subcog's persistent memory system into your Claude Code workflow. Add these configurations to your `CLAUDE.md` (project-level) or global `~/.claude/CLAUDE.md` (user-level) to ensure consistent memory protocol adherence.

## Quick Start

### 1. Install Subcog

```bash
# Install from crates.io
cargo install subcog

# Or build from source
git clone https://github.com/zircote/subcog.git
cd subcog && make dev
```

### 2. Configure MCP Server

Add to your `~/.claude/settings.json`:

```json
{
  "mcpServers": {
    "subcog": {
      "command": "subcog",
      "args": ["serve"],
      "env": {
        "SUBCOG_LOG_LEVEL": "info"
      }
    }
  }
}
```

### 3. Configure Hooks (Optional but Recommended)

Create `.claude/hooks.json` in your project root:

```json
{
  "hooks": [
    {
      "matcher": { "event": "session_start" },
      "hooks": [{
        "type": "command",
        "command": "subcog hook session-start"
      }]
    },
    {
      "matcher": { "event": "user_prompt_submit" },
      "hooks": [{
        "type": "command",
        "command": "sh -c 'subcog hook user-prompt-submit \"$PROMPT\"'"
      }]
    },
    {
      "matcher": { "event": "post_tool_use" },
      "hooks": [{
        "type": "command",
        "command": "subcog hook post-tool-use"
      }]
    },
    {
      "matcher": { "event": "pre_compact" },
      "hooks": [{
        "type": "command",
        "command": "subcog hook pre-compact"
      }]
    },
    {
      "matcher": { "event": "stop" },
      "hooks": [{
        "type": "command",
        "command": "subcog hook stop"
      }]
    }
  ]
}
```

---

## CLAUDE.md Integration

Add the following section to your project's `CLAUDE.md` or your global `~/.claude/CLAUDE.md`:

```markdown
# Subcog Memory Protocol

Subcog provides persistent memory across coding sessions. Follow this protocol to ensure knowledge is captured and surfaced effectively.

## Required Workflow

### Session Start
1. Call `mcp__subcog__prompt_understanding` with `{}` to load usage guidance
2. Call `mcp__subcog__subcog_status` to verify memory system health
3. Call `mcp__subcog__subcog_recall` with relevant query to retrieve prior context

### Before Substantive Responses
- Use `mcp__subcog__subcog_recall` to search for relevant memories
- Check for existing decisions, patterns, and learnings related to the topic

### When to Capture Memories

Capture immediately when you detect:

| Signal | Namespace | Example |
|--------|-----------|---------|
| "we decided", "going with", "choosing" | `decisions` | Architecture choices, technology selections |
| "always", "never", "convention", "rule" | `patterns` | Coding standards, project conventions |
| "turns out", "gotcha", "realized" | `learnings` | Debugging insights, unexpected behaviors |
| "TODO", "temporary", "needs refactor" | `tech-debt` | Known issues, planned improvements |
| Project background, team agreements | `context` | Onboarding info, team decisions |

### Memory Quality Guidelines

When capturing:
- Include the **why** (rationale), not just the **what**
- Add relevant file paths via `source` parameter
- Use descriptive, searchable tags
- Keep content concise (1-3 paragraphs)

## MCP Tool Reference

### Core Operations

| Tool | Purpose |
|------|---------|
| `subcog_capture` | Store a new memory (required: content, namespace) |
| `subcog_recall` | Search memories (semantic + text hybrid search) |
| `subcog_get` | Retrieve memory by ID |
| `subcog_update` | Update memory content/tags |
| `subcog_delete` | Remove a memory |
| `subcog_status` | Check system health and statistics |

### Available Namespaces

| Namespace | Purpose |
|-----------|---------|
| `decisions` | Architecture and design choices |
| `patterns` | Coding conventions and standards |
| `learnings` | Insights and discoveries |
| `context` | Project background and state |
| `tech-debt` | Known issues and TODOs |
| `apis` | API documentation and contracts |
| `config` | Configuration details |
| `security` | Security policies and findings |
| `performance` | Performance observations |
| `testing` | Test strategies and edge cases |

### Search Filter Syntax

Use GitHub-style filters with `subcog_recall`:

```
ns:decisions tag:rust since:7d           # Recent Rust decisions
ns:patterns source:src/api/*             # API patterns
ns:learnings -tag:deprecated             # Active learnings
tag:security tag:auth                    # Security + auth intersection
```

## Example Captures

### Decision
```yaml
subcog_capture:
  content: "Decided to use SQLite for persistence. Rationale: single-file storage, no external dependencies, excellent Rust support via rusqlite."
  namespace: decisions
  tags: [database, sqlite, storage, architecture]
  source: src/storage/mod.rs
```

### Pattern
```yaml
subcog_capture:
  content: "All error types must use thiserror with #[error(...)] attributes. No panics in library code - use Result types exclusively."
  namespace: patterns
  tags: [error-handling, rust, conventions]
```

### Learning
```yaml
subcog_capture:
  content: "FTS5 requires content to be re-indexed after schema changes. Run subcog_reindex if search results seem stale after migrations."
  namespace: learnings
  tags: [sqlite, fts5, search, gotcha]
```
```

---

## AGENTS.md Integration (GitHub Copilot)

For projects using GitHub Copilot with Subcog, add to `.github/copilot-instructions.md` or `AGENTS.md`:

```markdown
# Subcog Memory Integration

This project uses Subcog for persistent memory. When working on this codebase:

## Before Making Changes

1. Search for relevant context:
   - Query `subcog_recall` with keywords related to your task
   - Check `ns:decisions` for architectural constraints
   - Check `ns:patterns` for coding conventions

2. Review existing decisions before proposing alternatives

## After Making Changes

Capture significant decisions or learnings:
- New architectural decisions → `ns:decisions`
- Discovered patterns or conventions → `ns:patterns`
- Debugging insights or gotchas → `ns:learnings`
- Known issues or TODOs → `ns:tech-debt`

## Memory Namespaces

| Namespace | When to Use |
|-----------|-------------|
| `decisions` | Technology choices, architectural decisions |
| `patterns` | Coding standards, conventions |
| `learnings` | Discoveries, debugging insights |
| `context` | Project background, onboarding info |
| `tech-debt` | Known issues, future improvements |
```

---

## Environment Variables

Configure Subcog behavior via environment variables:

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBCOG_LOG_LEVEL` | Logging verbosity | `info` |
| `SUBCOG_SEARCH_INTENT_ENABLED` | Enable smart search intent detection | `true` |
| `SUBCOG_DEDUP_ENABLED` | Enable automatic deduplication | `true` |
| `SUBCOG_AUTO_EXTRACT_ENTITIES` | Extract entities from memories | `false` |
| `SUBCOG_LLM_PROVIDER` | LLM provider for enrichment | `anthropic` |

---

## Troubleshooting

| Issue | Solution |
|-------|----------|
| Memories not found | Run `subcog_status`, try broader query |
| Search seems stale | Run `subcog_reindex` to rebuild index |
| Duplicates appearing | Enable `SUBCOG_DEDUP_ENABLED=true` |
| MCP tools not available | Check `~/.claude/settings.json` MCP config |

---

## Full Protocol Reference

For complete tool documentation, run:
```bash
subcog --help
```

Or call `mcp__subcog__prompt_understanding` in Claude Code for comprehensive usage guidance.
