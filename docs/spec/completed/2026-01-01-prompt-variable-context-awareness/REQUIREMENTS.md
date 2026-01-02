---
document_type: requirements
project_id: SPEC-2026-01-01-002
version: 1.0.0
last_updated: 2026-01-01
status: draft
github_issue: 29
---

# Prompt Variable Context-Aware Extraction - Requirements

## Executive Summary

This specification addresses two related capabilities:

1. **Bug Fix (Issue #29)**: The variable extraction logic incorrectly captures `{{variable}}` patterns inside fenced code blocks and documentation examples. Only actual runtime variables should be extracted.

2. **Enhancement**: Add LLM-assisted frontmatter enrichment at save time, generating descriptions, defaults, required flags, and prompt-level metadata automatically.

## Problem Statement

### Problem 1: False Positive Variable Extraction

When saving prompts containing documentation or examples with `{{variable}}` syntax, the system extracts ALL patterns regardless of context.

**Example Input**:
```markdown
Scan {{PROJECT_ROOT_PATH}} for issues.

## Example Output
```markdown
**Generated:** {{timestamp}}
**Files:** {{count}}
`` `
```

**Current Behavior**: Extracts 3 variables: `PROJECT_ROOT_PATH`, `timestamp`, `count`
**Expected Behavior**: Extracts 1 variable: `PROJECT_ROOT_PATH`

### Problem 2: Sparse Frontmatter

When users save prompts, variables are auto-detected but lack:
- Human-readable descriptions
- Sensible default values
- Required/optional classification
- Prompt-level metadata (description, tags)

Users must manually add this metadata, which is error-prone and tedious.

## Goals and Success Criteria

### Primary Goals

1. **Context-aware extraction**: Variables inside fenced code blocks are ignored
2. **Automatic enrichment**: LLM generates complete frontmatter at save time

### Success Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| False positive rate | 0% | Variables in code blocks not extracted |
| Enrichment coverage | 100% | All variables have descriptions |
| Save latency | <2s | Including LLM enrichment call |
| User satisfaction | Reduced manual editing | Frontmatter ready to use |

### Non-Goals (Explicit Exclusions)

- Skip inline code (single backticks) - only fenced blocks
- Skip XML tags or other patterns - fenced blocks only
- Real-time streaming of enrichment - batch response is fine

## Functional Requirements

### FR-1: Fenced Code Block Detection

- **FR-1.1**: Detect fenced code blocks using triple backticks (` ``` `)
- **FR-1.2**: Handle code blocks with language identifiers (` ```rust `, ` ```markdown `)
- **FR-1.3**: Handle nested or adjacent code blocks correctly
- **FR-1.4**: Track byte positions of code blocks for exclusion

### FR-2: Context-Aware Variable Extraction

- **FR-2.1**: Skip `{{variable}}` patterns inside fenced code blocks
- **FR-2.2**: Extract variables outside code blocks as before
- **FR-2.3**: Maintain backward compatibility for prompts without code blocks
- **FR-2.4**: Preserve existing validation (unbalanced braces, empty names, reserved prefixes)

### FR-3: LLM-Assisted Frontmatter Enrichment

- **FR-3.1**: Trigger enrichment on every `prompt save` operation
- **FR-3.2**: Generate for each variable:
  - `description`: Human-readable explanation
  - `required`: Boolean based on semantic analysis
  - `default`: Sensible default value if optional
  - `validation_hint`: Format or constraint guidance (optional)
- **FR-3.3**: Generate prompt-level metadata:
  - `description`: What the prompt does
  - `tags`: Categorization tags (3-5)
- **FR-3.4**: Use existing LLM provider infrastructure (Anthropic, OpenAI, Ollama, LM Studio)

### FR-4: Enrichment Behavior

- **FR-4.1**: If user provides explicit `variables:` in frontmatter, use those as-is (no override)
- **FR-4.2**: If user provides partial metadata, enrich only missing fields
- **FR-4.3**: If LLM fails, fall back to current behavior (name only, required=true)
- **FR-4.4**: Log enrichment results for debugging

### FR-5: CLI Integration

- **FR-5.1**: `subcog prompt save` always enriches (no flag needed)
- **FR-5.2**: Add `--no-enrich` flag to skip LLM enrichment
- **FR-5.3**: Add `--dry-run` flag to show enrichment without saving
- **FR-5.4**: Display enrichment results in save confirmation

### FR-6: MCP Integration

- **FR-6.1**: `prompt_save` tool uses enrichment by default
- **FR-6.2**: Add `skip_enrichment` parameter to `prompt_save`
- **FR-6.3**: Return enriched frontmatter in tool response

## Non-Functional Requirements

### NFR-1: Performance

| Operation | Target |
|-----------|--------|
| Code block detection | <1ms |
| Variable extraction (with context) | <5ms |
| LLM enrichment call | <2s |
| Total save operation | <3s |

### NFR-2: Reliability

- Graceful fallback when LLM unavailable
- No data loss if enrichment fails
- Maintain exact content (only frontmatter changes)

### NFR-3: Compatibility

- Backward compatible with existing saved prompts
- No breaking changes to variable substitution
- Existing tests continue to pass

## Technical Constraints

- Must use existing `LlmProvider` trait
- Must follow project Rust standards (no panics, proper errors)
- Must integrate with existing `PromptService` and `PromptParser`

## Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| LLM rate limiting | Medium | Medium | Graceful fallback, retry logic |
| Inconsistent LLM output | Medium | Low | JSON schema validation, fallback |
| Nested code blocks edge cases | Low | Low | Comprehensive test suite |
| Performance regression | Low | Medium | Benchmark tests, caching |

## Acceptance Criteria

### Bug Fix (Issue #29)
- [ ] Variables inside ` ``` ` blocks are NOT extracted
- [ ] Variables outside code blocks ARE extracted
- [ ] Mixed content (variables inside and outside) handled correctly
- [ ] Existing prompts without code blocks work unchanged
- [ ] All existing tests pass

### Frontmatter Enrichment
- [ ] Every variable gets a description
- [ ] Required/optional classification is reasonable
- [ ] Defaults provided for optional variables
- [ ] Prompt description and tags generated
- [ ] `--no-enrich` flag skips LLM call
- [ ] Graceful fallback when LLM unavailable
- [ ] User-provided frontmatter respected (not overwritten)
