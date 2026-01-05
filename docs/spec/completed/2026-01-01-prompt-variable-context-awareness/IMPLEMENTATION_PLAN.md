---
document_type: implementation_plan
project_id: SPEC-2026-01-01-002
version: 1.1.0
last_updated: 2026-01-02
status: complete
estimated_effort: 2-3 days
---

# Prompt Variable Context-Aware Extraction - Implementation Plan

## Overview

This plan implements two features in four phases:

1. **Phase 1**: Code block detection and context-aware variable extraction (Bug Fix)
2. **Phase 2**: Prompt enrichment service with LLM integration
3. **Phase 3**: CLI and MCP integration
4. **Phase 4**: Testing, documentation, and cleanup

## Phase Summary

| Phase | Name | Tasks | Priority |
|-------|------|-------|----------|
| 1 | Code Block Detection | 5 | P0 (Bug Fix) |
| 2 | Enrichment Service | 6 | P0 |
| 3 | Integration | 5 | P0 |
| 4 | Testing & Docs | 4 | P1 |

---

## Phase 1: Code Block Detection (Bug Fix)

**Goal**: Fix Issue #29 - skip variables inside fenced code blocks

### Task 1.1: Add Code Block Detection Function

**File**: `src/models/prompt.rs`

**Description**: Add `detect_code_blocks()` function with regex pattern

**Acceptance Criteria**:
- [x] `CodeBlockRegion` struct defined with start, end, language
- [x] `CODE_BLOCK_PATTERN` regex using LazyLock
- [x] Function returns sorted list of regions
- [x] Handles language identifiers (```rust, ```markdown)

**Estimated Effort**: 1 hour

### Task 1.2: Add Exclusion-Aware Variable Extraction

**File**: `src/models/prompt.rs`

**Description**: Modify `extract_variables()` to skip code block regions

**Acceptance Criteria**:
- [x] New internal function `extract_variables_with_exclusions()`
- [x] Original `extract_variables()` calls detection then extraction
- [x] Variables inside code blocks are NOT extracted
- [x] Variables outside code blocks ARE extracted
- [x] Backward compatible (empty content, no code blocks)

**Estimated Effort**: 1 hour

### Task 1.3: Add Position-in-Region Helper

**File**: `src/models/prompt.rs`

**Description**: Helper to check if a byte position falls within exclusion regions

**Acceptance Criteria**:
- [x] `fn is_in_exclusion(position: usize, regions: &[CodeBlockRegion]) -> bool`
- [x] Efficient (binary search if needed, but regions are typically few)

**Estimated Effort**: 30 minutes

### Task 1.4: Unit Tests for Code Block Detection

**File**: `src/models/prompt.rs` (test module)

**Description**: Comprehensive tests for code block detection

**Test Cases**:
- [x] Single code block
- [x] Multiple code blocks
- [x] Code block with language identifier
- [x] Empty code block
- [x] Unclosed code block (edge case)
- [x] No code blocks

**Estimated Effort**: 1 hour

### Task 1.5: Unit Tests for Context-Aware Extraction

**File**: `src/models/prompt.rs` (test module)

**Description**: Tests for variable extraction with exclusions

**Test Cases**:
- [x] Variable only outside code block → extracted
- [x] Variable only inside code block → NOT extracted
- [x] Variables both inside and outside → only outside extracted
- [x] Multiple code blocks with variables
- [x] Variable at exact boundary of code block

**Estimated Effort**: 1 hour

---

## Phase 2: Enrichment Service

**Goal**: Add LLM-powered frontmatter generation

### Task 2.1: Create Enrichment Types

**File**: `src/services/prompt_enrichment.rs` (new)

**Description**: Define request/response types for enrichment

**Acceptance Criteria**:
- [x] `EnrichmentRequest` struct
- [x] `EnrichmentResult` struct
- [x] `PartialMetadata` for preserving user-provided values
- [x] Serde traits for JSON handling

**Estimated Effort**: 30 minutes

### Task 2.2: Create Enrichment System Prompt

**File**: `src/llm/system_prompt.rs`

**Description**: Add system prompt for frontmatter enrichment

**Acceptance Criteria**:
- [x] `PROMPT_ENRICHMENT_PROMPT` constant
- [x] Clear JSON schema in prompt
- [x] Examples of good output
- [x] Security: XML tags to isolate user content

**Estimated Effort**: 30 minutes

### Task 2.3: Implement PromptEnrichmentService

**File**: `src/services/prompt_enrichment.rs`

**Description**: Core enrichment logic using LLM

**Acceptance Criteria**:
- [x] `PromptEnrichmentService<P: LlmProvider>` struct
- [x] `enrich()` method that calls LLM
- [x] JSON parsing of LLM response
- [x] Merge with existing metadata (don't overwrite user values)

**Estimated Effort**: 2 hours

### Task 2.4: Add Fallback Logic

**File**: `src/services/prompt_enrichment.rs`

**Description**: Graceful fallback when LLM fails

**Acceptance Criteria**:
- [x] `EnrichmentResult::basic_from_variables()` fallback constructor
- [x] Timeout handling (5 second max)
- [x] Retry once on parse failure
- [x] Logging of fallback reasons

**Estimated Effort**: 1 hour

### Task 2.5: Export from Services Module

**File**: `src/services/mod.rs`

**Description**: Export enrichment service

**Acceptance Criteria**:
- [x] `pub mod prompt_enrichment;`
- [x] Re-export key types

**Estimated Effort**: 15 minutes

### Task 2.6: Unit Tests for Enrichment Service

**File**: `src/services/prompt_enrichment.rs` (test module)

**Description**: Tests with mock LLM provider

**Test Cases**:
- [x] Successful enrichment with valid JSON
- [x] Fallback on LLM error
- [x] Fallback on invalid JSON
- [x] Partial metadata preservation
- [x] Empty variables list

**Estimated Effort**: 1.5 hours

---

## Phase 3: Integration

**Goal**: Wire enrichment into CLI and MCP

### Task 3.1: Update PromptService

**File**: `src/services/prompt.rs`

**Description**: Add enrichment to save flow

**Acceptance Criteria**:
- [x] `save_with_enrichment()` method
- [x] Option to skip enrichment
- [x] Uses `PromptEnrichmentService`

**Estimated Effort**: 1 hour

### Task 3.2: Update CLI Save Command

**File**: `src/cli/prompt.rs`

**Description**: Add `--no-enrich` and `--dry-run` flags

**Acceptance Criteria**:
- [x] `--no-enrich` flag skips LLM call
- [x] `--dry-run` shows enrichment without saving
- [x] Display enriched metadata in output
- [x] Error message if LLM unavailable (with fallback note)

**Estimated Effort**: 1 hour

### Task 3.3: Update MCP prompt_save Tool

**File**: `src/mcp/tools.rs`

**Description**: Add `skip_enrichment` parameter

**Acceptance Criteria**:
- [x] New parameter in schema
- [x] Pass through to service
- [x] Include enriched metadata in response

**Estimated Effort**: 45 minutes

### Task 3.4: Integration Test - CLI Flow

**File**: `tests/integration_test.rs` or new test file

**Description**: End-to-end CLI tests

**Test Cases**:
- [x] Save with enrichment (mock LLM)
- [x] Save with `--no-enrich`
- [x] Save prompt with code blocks

**Estimated Effort**: 1 hour

### Task 3.5: Integration Test - MCP Flow

**File**: `tests/integration_test.rs`

**Description**: End-to-end MCP tests

**Test Cases**:
- [x] `prompt_save` with enrichment
- [x] `prompt_save` with `skip_enrichment: true`
- [x] Verify response includes enriched metadata

**Estimated Effort**: 1 hour

---

## Phase 4: Testing, Documentation & Cleanup

**Goal**: Ensure quality and document changes

### Task 4.1: Run Full Test Suite

**Command**: `cargo test --all-features`

**Acceptance Criteria**:
- [x] All existing tests pass
- [x] All new tests pass
- [x] No clippy warnings

**Estimated Effort**: 30 minutes

### Task 4.2: Update CLAUDE.md

**File**: `CLAUDE.md`

**Description**: Document new behavior

**Acceptance Criteria**:
- [x] Note about code block exclusion
- [x] Document enrichment feature
- [x] Update CLI command reference

**Estimated Effort**: 30 minutes

### Task 4.3: Update Help Content

**File**: `src/help/content/prompts.md`

**Description**: Update user-facing help

**Acceptance Criteria**:
- [x] Explain code block handling
- [x] Document enrichment behavior
- [x] Examples of prompts with code blocks

**Estimated Effort**: 30 minutes

### Task 4.4: Update CHANGELOG

**File**: `CHANGELOG.md`

**Description**: Document changes for release

**Acceptance Criteria**:
- [x] Bug fix entry for Issue #29
- [x] Feature entry for enrichment
- [x] Breaking changes (if any)

**Estimated Effort**: 15 minutes

---

## Dependency Graph

```
Phase 1 (Bug Fix):
  1.1 ──┬──▶ 1.2 ──▶ 1.3
        │
        └──▶ 1.4
              │
              └──▶ 1.5

Phase 2 (Enrichment):
  2.1 ──┬──▶ 2.3 ──▶ 2.4
        │      │
  2.2 ──┘      └──▶ 2.5 ──▶ 2.6

Phase 3 (Integration):
  3.1 ──┬──▶ 3.2
        │
        └──▶ 3.3
              │
              └──▶ 3.4, 3.5

Phase 4 (Finalization):
  All above ──▶ 4.1 ──▶ 4.2, 4.3, 4.4
```

## Risk Mitigation Tasks

| Risk | Mitigation Task | Phase |
|------|-----------------|-------|
| LLM unavailable | Task 2.4: Fallback logic | Phase 2 |
| Invalid LLM JSON | Task 2.4: Retry + fallback | Phase 2 |
| Performance regression | Task 4.1: Benchmark check | Phase 4 |
| Breaking changes | Task 1.5: Backward compat tests | Phase 1 |

## Testing Checklist

- [x] Unit tests for code block detection (1.4)
- [x] Unit tests for variable extraction (1.5)
- [x] Unit tests for enrichment service (2.6)
- [x] Integration tests for CLI (3.4)
- [x] Integration tests for MCP (3.5)
- [x] Full test suite passes (4.1)

## Documentation Tasks

- [x] Update CLAUDE.md (4.2)
- [x] Update help content (4.3)
- [x] Update CHANGELOG (4.4)
- [ ] Close Issue #29 with PR reference

## Launch Checklist

- [x] All tests passing
- [x] No clippy warnings
- [x] Documentation complete
- [x] CHANGELOG updated
- [ ] PR created and reviewed
- [ ] Issue #29 closed
