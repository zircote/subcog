---
description: Capture a memory (decision, learning, pattern, or context) to persistent storage
allowed-tools: mcp__subcog__subcog_capture, mcp__subcog__subcog_namespaces, AskUserQuestion, Bash
argument-hint: "[--namespace <ns>] <content> | --interactive"
---

# /subcog:capture

Capture decisions, learnings, patterns, and context as persistent memories.

## Usage

```
/subcog:capture "Use PostgreSQL for primary storage"
/subcog:capture --namespace decisions "Chose Rust for single-binary distribution"
/subcog:capture --interactive
```

## Arguments

<arguments>
1. **content** (required unless --interactive): The memory content to capture
2. **--namespace**: One of: decisions, patterns, learnings, context, tech-debt, apis, config, security, performance, testing
3. **--interactive**: Guide user through capture with questions
</arguments>

## Execution Strategy

<strategy>
**MCP-First Approach:**
1. Try `mcp__subcog__subcog_capture` first
2. If MCP unavailable, fallback to CLI: `subcog capture --namespace <ns> "<content>"`

**Namespace Detection:**
If no namespace specified, detect from signal words:
- "decided", "chose", "going with" → decisions
- "always", "never", "when X do Y" → patterns
- "TIL", "learned", "gotcha" → learnings
- "because", "constraint" → context
- "TODO", "FIXME", "temporary" → tech-debt
</strategy>

## Interactive Mode

<interactive>
When `--interactive` is specified:
1. Use AskUserQuestion to determine namespace
2. Ask for the core insight/decision
3. Ask for context/rationale
4. Suggest tags based on content
5. Confirm before capturing
</interactive>

## Examples

<examples>
**Quick capture with auto-detected namespace:**
```
/subcog:capture "TIL that SQLite FTS5 requires porter tokenizer for English stemming"
```
Detected: learnings (signal: "TIL")

**Explicit namespace:**
```
/subcog:capture --namespace decisions "Use thiserror for custom error types in library code"
```

**Interactive mode:**
```
/subcog:capture --interactive
```
Will prompt for namespace, content, context, and tags.
</examples>
