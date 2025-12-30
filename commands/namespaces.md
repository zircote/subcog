---
description: List and explain available memory namespaces
allowed-tools: mcp__subcog__subcog_namespaces, Bash
---

# /subcog:namespaces

Display all available memory namespaces with descriptions and usage guidance.

## Usage

```
/subcog:namespaces
```

## Execution Strategy

<strategy>
**MCP-First Approach:**
1. Try `mcp__subcog__subcog_namespaces` first
2. If MCP unavailable, fallback to CLI: `subcog namespaces`
</strategy>

## Namespaces Reference

<namespaces>
| Namespace | Purpose | Signal Words |
|-----------|---------|--------------|
| **decisions** | Architectural and design decisions | "decided", "chose", "let's use", "going with" |
| **patterns** | Discovered patterns and conventions | "pattern", "always", "never", "convention", "when X do Y" |
| **learnings** | Lessons learned from debugging | "TIL", "learned", "discovered", "gotcha", "caveat" |
| **context** | Important contextual information | "because", "constraint", "requirement", "background" |
| **tech-debt** | Technical debts and future work | "TODO", "FIXME", "refactor", "temporary", "workaround" |
| **apis** | API endpoints and contracts | "endpoint", "API", "contract", "schema" |
| **config** | Configuration and environment | "config", "environment", "setting", "variable" |
| **security** | Security-related information | "security", "auth", "vulnerability", "permission" |
| **performance** | Performance optimizations | "performance", "optimization", "benchmark", "latency" |
| **testing** | Testing strategies and edge cases | "test", "edge case", "coverage", "fixture" |
</namespaces>

## Choosing the Right Namespace

<guidance>
**Ask yourself:**

1. **Is this a choice we made?** → `decisions`
2. **Is this a recurring practice?** → `patterns`
3. **Did I just discover something?** → `learnings`
4. **Is this background info?** → `context`
5. **Is this something to fix later?** → `tech-debt`
6. **Is this about an API?** → `apis`
7. **Is this about settings?** → `config`
8. **Is this security-related?** → `security`
9. **Is this about speed?** → `performance`
10. **Is this about testing?** → `testing`
</guidance>

## Examples by Namespace

<examples>
**decisions:**
```
"Use PostgreSQL for primary storage due to JSON support and reliability"
"Chose Rust over Go for single-binary distribution requirement"
```

**patterns:**
```
"Always use thiserror for custom error types in library code"
"When adding a new endpoint, always update the OpenAPI spec first"
```

**learnings:**
```
"TIL: SQLite FTS5 requires porter tokenizer for English stemming"
"Gotcha: the Anthropic API returns 529 when overloaded, not 503"
```

**context:**
```
"Legacy system requires XML responses for backwards compatibility"
"Constraint: must support offline operation for field users"
```

**tech-debt:**
```
"TODO: replace hand-rolled auth with proper OAuth implementation"
"FIXME: this retry logic should use exponential backoff"
```
</examples>
