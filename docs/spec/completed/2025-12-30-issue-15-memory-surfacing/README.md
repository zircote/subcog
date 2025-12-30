---
project_id: SPEC-2025-12-30-001
project_name: "Proactive Memory Surfacing: Search Intent Detection & MCP Resources"
slug: issue-15-memory-surfacing
status: completed
created: 2025-12-30T12:00:00Z
approved: 2025-12-30T17:13:14Z
approved_by: "Robert Allen <zircote@gmail.com>"
started: 2025-12-30T17:30:00Z
completed: 2025-12-30T23:50:00Z
final_effort: "1 day, 77 tasks, 388 tests"
outcome: success
expires: 2026-03-30T12:00:00Z
superseded_by: null
tags: [memory, search-intent, mcp, hooks, llm, user-experience]
stakeholders: []
github_issue: 15
github_subissues: [16, 17, 18, 19, 20, 21]
github_related: [24]
---

# Proactive Memory Surfacing: Search Intent Detection & MCP Resources

## Overview

This project transforms subcog's memory system from **reactive** (memories surfaced only on explicit recall) to **proactive** (memories automatically surfaced when relevant based on detected user intent).

## Problem Statement

Currently, subcog's memory system is reactive - memories are only surfaced when:
1. The assistant explicitly calls `memory.recall`
2. SessionStart injects generic context

This misses opportunities to provide relevant context when:
- User prompts suggest information seeking ("how do I...", "where is...")
- The assistant uses search tools (Grep, Glob, Read)
- Topics discussed have relevant prior decisions

## Proposed Solution

Implement a two-pronged approach:

1. **Enhanced UserPromptSubmit Hook** - Detect search intent in user prompts and inject relevant memories before the assistant starts working
2. **MCP Resource-based Surfacing** - Expose memories as queryable and topic-based resources the assistant can access

## Key Deliverables

| Phase | Deliverable | Issue |
|-------|-------------|-------|
| 1 | Foundation - Search Intent Detection | #16 |
| 2 | Adaptive Memory Injection | #17 |
| 3 | MCP Resources - Query & Topic | #18 |
| 4 | Enhanced MCP Prompts | #19 |
| 5 | LLM Intent Classification | #20 |
| 6 | Hook Guidance & Polish | #21 |

## Success Criteria

- UserPromptSubmit detects search intent with >80% accuracy
- Keyword detection completes in <10ms
- LLM classification completes in <200ms with proper fallback
- Total UserPromptSubmit latency <200ms
- All three new/enhanced prompts work correctly
- Feature degrades gracefully when components unavailable

## Quick Links

- [Requirements](./REQUIREMENTS.md)
- [Architecture](./ARCHITECTURE.md)
- [Implementation Plan](./IMPLEMENTATION_PLAN.md)
- [Decisions](./DECISIONS.md)
- [Parent Issue #15](https://github.com/zircote/subcog/issues/15)
