# Subcog Integration Guide for Non-Claude CLI Assistants

This guide explains how to integrate Subcog persistent memory with AI coding assistants that don't have native hooks support, such as OpenAI Codex CLI, OpenCode, Aider, and similar tools.

## Table of Contents

- [Overview](#overview)
- [How Claude Code Hooks Work](#how-claude-code-hooks-work)
- [Replicating Hooks via System Prompts](#replicating-hooks-via-system-prompts)
- [OpenAI Codex CLI Setup](#openai-codex-cli-setup)
- [OpenCode Setup](#opencode-setup)
- [Generic Setup](#generic-setup)
- [MCP Tool Reference](#mcp-tool-reference)
- [Testing Your Integration](#testing-your-integration)
- [Limitations](#limitations)

---

## Overview

Subcog is a persistent memory system that stores decisions, patterns, learnings, and context across coding sessions. Claude Code uses a hooks system to automatically inject memory context and capture important information. Other CLI tools lack this hooks system, so we must encode these behaviors as explicit instructions.

### What This Guide Provides

1. **System prompt templates** that encode hook behaviors as LLM instructions
2. **MCP server configuration** for various CLI tools
3. **Memory protocols** the LLM should follow
4. **Testing guidance** to verify the integration works

---

## How Claude Code Hooks Work

Claude Code provides 5 lifecycle hooks that Subcog uses:

| Hook | Trigger | What It Does |
|------|---------|--------------|
| **SessionStart** | Session begins | Injects memory context, shows statistics |
| **UserPromptSubmit** | User sends prompt | Detects capture signals, surfaces related memories |
| **PostToolUse** | Tool completes | Finds memories related to tool context |
| **PreCompact** | Context compaction | Auto-captures important content before cleanup |
| **Stop** | Session ends | Generates summary, syncs to remote |

### What CAN Be Replicated via Prompts

| Capability | Replication Approach | Fidelity |
|-----------|---------------------|----------|
| Memory capture on signals | Instruction-based pattern matching | HIGH |
| Memory recall on queries | Explicit tool invocation rules | HIGH |
| Namespace selection | Decision tree in prompt | HIGH |
| Session start context | Manual status check instruction | MEDIUM |
| Related memory surfacing | Post-tool recall guidance | LOW |

### What CANNOT Be Replicated

- **Automatic lifecycle events** - No session start/end triggers
- **Pre-compaction capture** - No context cleanup signal
- **Server-side deduplication** - Requires Subcog service
- **Timing constraints** - No deadline-aware execution
- **Proactive injection** - Must be explicit tool calls

---

## Replicating Hooks via System Prompts

The core strategy is to encode hook behaviors as explicit instructions:

### SessionStart -> Manual Status Check

```
At the start of each session, call subcog_status to check the memory system.
If working on a known project, call subcog_recall with relevant project terms.
```

### UserPromptSubmit -> Capture Signal Detection

```
IMMEDIATELY capture when you detect these signals:
- Decisions: "we'll use", "decided", "choosing"
- Patterns: "pattern", "convention", "always", "never"
- Learnings: "TIL", "turns out", "discovered", "gotcha"
- Solutions: "fixed", "solved", "the issue was"
- Tech Debt: "TODO", "FIXME", "temporary", "refactor"
```

### UserPromptSubmit -> Search Intent Detection

```
Search memory BEFORE acting when you detect:
- HowTo: "how do I", "implement", "create"
- Location: "where is", "find", "locate"
- Troubleshoot: "error", "fix", "debug"
```

### MCP Tool Invocation Rule

```
If the user types a token that matches an MCP tool name (for example,
subcog_status, subcog_recall, or prompt_list), treat it as a request to run
that MCP tool, not a shell command, unless the user explicitly says "shell" or
"run in terminal".
```

### PostToolUse -> Related Memory Lookup

```
After using file/code tools, consider recalling related memories:
- After Read/Write: recall by file path or module name
- After Search/Grep: recall by search topic
- After Bash: recall by command or tool name
```

### Stop -> Session End Checklist

```
Before ending a session, review and capture:
1. Decisions made
2. Patterns established
3. Learnings discovered
4. Blockers resolved
5. Tech debt created
```

> **Note**: `subcog_sync` is deprecated as of v0.8.0. SQLite is now the authoritative storage layer and memories persist automatically.

---

## OpenAI Codex CLI Setup

### 1. Configure MCP Server

Add to `~/.codex/config.toml`:

```toml
[mcp_servers.subcog]
command = "subcog"
args = ["serve"]
```

Or use the CLI:

```bash
codex mcp add subcog --command "subcog serve"
```

### 2. Install AGENTS.md

Copy the provided [`AGENTS.md`](./AGENTS.md) to one of:

- **Global**: `~/.codex/AGENTS.md` (applies to all projects)
- **Project**: `<project-root>/AGENTS.md` (project-specific)

### 3. Verify Setup

```bash
codex
> Use the subcog_status tool to check memory status
```

You should see memory count and system status.

### File Locations Summary

| File | Location | Purpose |
|------|----------|---------|
| `config.toml` | `~/.codex/config.toml` | MCP server config |
| `AGENTS.md` | `~/.codex/AGENTS.md` or project root | System prompt |
| `AGENTS.override.md` | Same locations | Temporary overrides |

### References

- [Codex CLI Documentation](https://developers.openai.com/codex/cli/)
- [AGENTS.md Guide](https://developers.openai.com/codex/guides/agents-md/)
- [Basic Configuration](https://developers.openai.com/codex/config-basic/)

---

## OpenCode Setup

### 1. Configure MCP Server

Add to `opencode.json` in your project root or global config:

```json
{
 "mcp": {
 "subcog": {
 "type": "local",
 "command": ["subcog", "serve"],
 "enabled": true
 }
 }
}
```

> **Note**: Verify the exact MCP schema format in the [OpenCode MCP documentation](https://opencode.ai/docs/mcp-servers/), as it may change.

### 2. Create Agent Definition

Create `.opencode/agent/subcog-memory.md`:

```markdown
---
name: subcog-memory
description: AI coding assistant with persistent memory via Subcog
mode: primary
---

[Copy content from opencode-subcog-agent.md]
```

### 3. Configure Instructions

Add to `opencode.json`:

```json
{
 "instructions": [
 ".opencode/agent/subcog-memory.md"
 ]
}
```

### 4. Verify Setup

```bash
opencode
> Use the subcog_status tool
```

### File Locations Summary

| File | Location | Purpose |
|------|----------|---------|
| `opencode.json` | Project root or `~/.config/opencode/` | Main config |
| Agent files | `.opencode/agent/` or `~/.config/opencode/agent/` | Agent definitions |
| Commands | `.opencode/command/` | Custom commands |

### References

- [OpenCode Documentation](https://opencode.ai/docs/)
- [OpenCode Agents](https://opencode.ai/docs/agents/)
- [OpenCode MCP Servers](https://opencode.ai/docs/mcp-servers/)
- [OpenCode Config](https://opencode.ai/docs/config/)

---

## Generic Setup

For other CLI tools that support MCP and custom system prompts:

### 1. Run Subcog as MCP Server

```bash
subcog serve
```

Or configure as a stdio server in your tool's MCP settings.

### 2. Add System Prompt

Use the core protocol from [`AGENTS.md`](./AGENTS.md), adapting the format to your tool's requirements.

### Core Protocol Summary

```
## Memory Protocol

### Session Start
- Call subcog_status to verify memory system
- Call subcog_recall for project context

### During Session
CAPTURE when you detect:
- Decisions: "we'll use", "decided", "choosing"
- Patterns: "pattern", "convention", "always"
- Learnings: "TIL", "turns out", "discovered"
- Solutions: "fixed", "solved", "workaround"
- Tech Debt: "TODO", "FIXME", "temporary"

RECALL before acting on:
- HowTo: "how do I", "implement", "create"
- Location: "where is", "find"
- Troubleshoot: "error", "fix", "debug"

### Session End
- Review for uncaptured decisions/learnings
- Call subcog_sync with direction "push"
```

---

## MCP Tool Reference

### Core Memory Tools

| Tool | Required Params | Optional Params | Description |
|------|-----------------|-----------------|-------------|
| `subcog_capture` | `content`, `namespace` | `tags`, `source`, `domain` | Save a memory |
| `subcog_recall` | - | `query`, `filter`, `namespace`, `mode`, `detail`, `limit`, `offset` | Search or list memories |
| `subcog_get` | `memory_id` | - | Retrieve a memory by ID |
| `subcog_update` | `memory_id` | `content`, `tags` | Update memory |
| `subcog_delete` | `memory_id` | `hard` | Delete a memory |
| `subcog_status` | - | - | Get system status |
| `subcog_namespaces` | - | - | List namespaces |
| `subcog_consolidate` | `namespace` | `query`, `strategy`, `dry_run` | Merge memories |
| `subcog_enrich` | `memory_id` | `enrich_tags`, `enrich_structure`, `add_context` | Enhance metadata |
| `subcog_reindex` | - | `repo_path` | Rebuild index |

> **Note**: `subcog_recall` now supports listing all memories when `query` is omitted. `subcog_sync` is deprecated (SQLite is now authoritative).

### Consolidated Tools (v0.8.0+)

| Tool | Required Params | Description |
|------|-----------------|-------------|
| `subcog_prompts` | `action` | Prompt management: save, list, get, run, delete |
| `subcog_templates` | `action` | Context template management: save, list, get, render, delete |
| `subcog_graph` | `operation` | Knowledge graph: neighbors, path, stats, visualize |
| `subcog_entities` | `action` | Entity management: create, get, list, delete, extract, merge |
| `subcog_relationships` | `action` | Relationship management: create, get, list, delete, infer |

### Legacy Prompt Tools (Deprecated)

> **️ Deprecated**: Use `subcog_prompts` with the appropriate `action` parameter instead.

| Tool | Replacement |
|------|-------------|
| `prompt_save` | `subcog_prompts` with `action: save` |
| `prompt_list` | `subcog_prompts` with `action: list` |
| `prompt_get` | `subcog_prompts` with `action: get` |
| `prompt_run` | `subcog_prompts` with `action: run` |
| `prompt_delete` | `subcog_prompts` with `action: delete` |

### Namespace Reference

| Namespace | Purpose |
|-----------|---------|
| `decisions` | Architectural and design choices |
| `patterns` | Code conventions and standards |
| `learnings` | Insights and discoveries |
| `context` | Project-specific knowledge |
| `tech-debt` | Known issues to address later |
| `apis` | External API behaviors |
| `config` | Environment and settings |
| `security` | Security considerations |
| `performance` | Optimization notes |
| `testing` | Test-related knowledge |

### Filter Syntax

GitHub-style filters for `subcog_recall`:

```
ns:decisions # Filter by namespace
tag:rust # Include tag
-tag:deprecated # Exclude tag
since:7d # Recent (days)
source:src/* # By source path
```

Example: `ns:patterns tag:error-handling -tag:deprecated since:30d`

---

## Testing Your Integration

### 1. Verify MCP Connection

```
> Use subcog_status to check the memory system
```

Expected: Memory count, namespace breakdown, system status

### 2. Test Capture

```
> We decided to use PostgreSQL for the database. Please capture this decision.
```

Expected: LLM calls `subcog_capture` with namespace `decisions`

### 3. Test Recall

```
> What database decisions have we made?
```

Expected: LLM calls `subcog_recall` with query about databases

### 4. Test Signal Detection

```
> TIL that Rust's borrow checker prevents data races at compile time.
```

Expected: LLM recognizes "TIL" signal and captures to `learnings`

### 5. Test Session End

```
> I'm done for today. Please sync any captured memories.
```

Expected: LLM calls `subcog_sync` with direction "push"

---

## Limitations

### Compared to Claude Code Hooks

| Feature | Claude Code | Prompt-Based |
|---------|-------------|--------------|
| Automatic context injection | SessionStart hook | Manual status check |
| Capture signal detection | <50ms server-side | ️ LLM pattern matching |
| Deduplication | 3-tier server-side | Not available |
| Pre-compaction capture | PreCompact hook | No signal available |
| Timing enforcement | Deadline-aware | Not possible |
| Related memory surfacing | PostToolUse hook | ️ Manual guidance |

### Mitigation Strategies

1. **No auto-injection**: Start sessions with explicit `subcog_status` and `subcog_recall`
2. **Signal detection**: Include comprehensive signal patterns in system prompt
3. **No deduplication**: Use descriptive tags to find existing memories before capturing
4. **No pre-compact**: Include session-end checklist in prompt
5. **Related memories**: Include post-tool recall guidance

---

## Troubleshooting

### MCP Server Not Connecting

1. Verify Subcog is installed: `subcog --version`
2. Test server manually: `subcog serve`
3. Check tool's MCP logs for connection errors

### Memories Not Being Captured

1. Verify capture signals are in your system prompt
2. Check namespace spelling (must match exactly)
3. Test with explicit capture command

### Recall Returning Empty

1. Verify memories exist: `subcog status`
2. Try broader search terms
3. Remove namespace filters temporarily

### Sync Failing

1. Check git remote is configured
2. Verify git credentials
3. Run `subcog sync` manually to see errors

---

## Files in This Directory

| File | Purpose |
|------|---------|
| [`README.md`](./README.md) | This integration guide |
| [`AGENTS.md`](./AGENTS.md) | OpenAI Codex system prompt |
| [`opencode-subcog-agent.md`](./opencode-subcog-agent.md) | OpenCode agent definition |
| [`opencode.json`](./opencode.json) | Example OpenCode config |

---

## Contributing

If you've successfully integrated Subcog with a CLI tool not covered here, please contribute:

1. Add a section to this README
2. Provide the system prompt/config format
3. Document any tool-specific quirks
4. Include testing guidance

---

## References

- [Subcog Documentation](../../README.md)
- [OpenAI Codex CLI](https://developers.openai.com/codex/cli/)
- [OpenCode Documentation](https://opencode.ai/docs/)
- [Model Context Protocol](https://modelcontextprotocol.io/)
