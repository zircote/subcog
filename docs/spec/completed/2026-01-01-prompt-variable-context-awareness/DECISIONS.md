---
document_type: decisions
project_id: SPEC-2026-01-01-002
---

# Prompt Variable Context-Aware Extraction - Architecture Decision Records

## ADR-001: Fenced Code Blocks Only

**Date**: 2026-01-01
**Status**: Accepted
**Deciders**: User, Claude

### Context

When excluding `{{variable}}` patterns from extraction, we need to decide which contexts to skip. Options include:
- Fenced code blocks only (` ``` `)
- Fenced + inline code (`` ` ``)
- Fenced + XML tags (`<example>`)
- All common documentation patterns

### Decision

Skip **fenced code blocks only** (triple backticks).

### Consequences

**Positive:**
- Minimal code change
- Handles 90%+ of documentation cases
- Clear, predictable behavior
- Easy to explain to users

**Negative:**
- Inline code with `{{var}}` will still be extracted
- XML-style tags like `<template>` won't be protected

**Neutral:**
- Users can use fenced blocks as a reliable escape mechanism

---

## ADR-002: LLM Enrichment Always On

**Date**: 2026-01-01
**Status**: Accepted
**Deciders**: User, Claude

### Context

When should LLM enrichment trigger during prompt save?
- Always on save
- Only when `--enrich` flag provided
- Only when frontmatter is missing
- Interactive confirmation

### Decision

**Always on save** by default, with `--no-enrich` opt-out flag.

### Consequences

**Positive:**
- Consistent behavior - all prompts get rich metadata
- No extra flags to remember
- Better out-of-box experience

**Negative:**
- Slower saves (~2s for LLM call)
- Requires LLM configuration
- Network dependency

**Mitigation:**
- Graceful fallback when LLM unavailable
- `--no-enrich` flag for fast saves

---

## ADR-003: Full Enrichment Scope

**Date**: 2026-01-01
**Status**: Accepted
**Deciders**: User, Claude

### Context

What should the LLM generate for enrichment?
- Description only
- Description + defaults
- Full enrichment (description, required, default, validation hints)

### Decision

**Full enrichment** for both variables and prompt-level metadata.

### Consequences

**Positive:**
- Rich, usable frontmatter from first save
- Variables are self-documenting
- Reduces manual editing

**Negative:**
- Larger LLM response to parse
- More fields that could be wrong
- Higher token cost

**Mitigation:**
- User can edit generated frontmatter
- User-provided values are preserved (not overwritten)

---

## ADR-004: Regex-Based Code Block Detection

**Date**: 2026-01-01
**Status**: Accepted
**Deciders**: Claude (technical)

### Context

How to detect fenced code blocks in content?
- Full Markdown parser (pulldown-cmark, comrak)
- Custom regex pattern
- Character-by-character state machine

### Decision

Use **regex pattern** with LazyLock for static compilation.

```rust
static CODE_BLOCK_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"```([a-zA-Z0-9_-]*)\n([\s\S]*?)```")
        .expect("static regex: code block pattern")
});
```

### Consequences

**Positive:**
- Consistent with existing codebase patterns (secrets.rs, pii.rs)
- No new dependencies
- Fast execution (<1ms)
- Simple to understand and maintain

**Negative:**
- May not handle all edge cases (nested, escaped)
- Less robust than full parser

**Alternatives Considered:**

1. **pulldown-cmark**: Full Markdown parser, but heavyweight for this use case
2. **State machine**: More control, but more code to maintain

---

## ADR-005: Existing LLM Provider Infrastructure

**Date**: 2026-01-01
**Status**: Accepted
**Deciders**: Claude (technical)

### Context

How to implement LLM calls for enrichment?
- Use existing `LlmProvider` trait
- Implement new MCP sampling handler
- Direct API calls

### Decision

Use **existing `LlmProvider` trait** from `src/llm/mod.rs`.

### Consequences

**Positive:**
- Reuses proven infrastructure
- Supports multiple providers (Anthropic, OpenAI, Ollama, LM Studio)
- Configuration already exists
- Error handling patterns established

**Negative:**
- MCP sampling remains unimplemented (declared but no handler)
- Tighter coupling to local LLM config

**Note:** MCP sampling implementation is out of scope for this spec.

---

## ADR-006: User Frontmatter Preservation

**Date**: 2026-01-01
**Status**: Accepted
**Deciders**: Claude (technical)

### Context

When user provides partial frontmatter, how should enrichment behave?
- Overwrite all with LLM output
- Merge (LLM fills gaps, user values preserved)
- Skip enrichment entirely if any frontmatter present

### Decision

**Merge** - LLM fills gaps, but user-provided values are never overwritten.

### Consequences

**Positive:**
- User intent is respected
- Can provide partial metadata and let LLM complete
- No surprises (user sees their values preserved)

**Negative:**
- More complex merge logic
- User can't "reset" to LLM-suggested values without removing their values

**Implementation:**
```rust
fn merge_enrichment(user: &PartialMetadata, llm: &EnrichmentResult) -> EnrichmentResult {
    EnrichmentResult {
        description: user.description.clone().unwrap_or_else(|| llm.description.clone()),
        tags: if user.tags.is_empty() { llm.tags.clone() } else { user.tags.clone() },
        variables: merge_variables(&user.variables, &llm.variables),
    }
}
```

---

## ADR-007: Graceful Fallback Strategy

**Date**: 2026-01-01
**Status**: Accepted
**Deciders**: Claude (technical)

### Context

What happens when LLM enrichment fails (unavailable, timeout, invalid response)?

### Decision

**Graceful fallback** to basic variable extraction (name + required=true).

### Consequences

**Positive:**
- Save never fails due to enrichment
- User can still work offline
- Consistent with subcog's degradation philosophy

**Negative:**
- Silent degradation may surprise users
- Metadata quality inconsistent when LLM flaky

**Implementation:**
```rust
// Log the fallback reason
tracing::warn!("Enrichment failed, using fallback: {}", error);

// Return basic metadata
EnrichmentResult::basic_from_variables(variables)
```

**User Feedback:**
- CLI shows: "Note: LLM enrichment unavailable, using basic metadata"
- MCP response includes: `"enrichment_status": "fallback"`
