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
## Subcog Memory Protocol (MANDATORY)

At the start of EVERY session, call `subcog_init` to load memory context and best practices.

During the session:
- **Before implementing**: Recall relevant decisions and patterns
- **Capture immediately**: When decisions, patterns, learnings, or fixes are identified
- Access Subcog only via MCP tools, never shell commands
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
