# Changelog

All notable changes to this specification will be documented in this file.

## [1.1.0] - 2026-01-02

### Implementation Complete

All 4 phases (19 tasks) have been implemented and verified.

### Added
- **Code Block Detection** (`src/models/prompt.rs`)
  - `find_code_blocks()` function using regex to detect fenced code blocks
  - Support for triple backticks and triple tildes
  - `CodeBlockRegion` struct with start/end byte positions
  - `is_in_code_block()` helper function

- **Context-Aware Variable Extraction** (`src/models/prompt.rs`)
  - `extract_variables_excluding_code_blocks()` function
  - Variables inside fenced code blocks are treated as documentation examples
  - `ExtractedVariable` struct with name and position

- **LLM-Assisted Metadata Enrichment** (`src/services/prompt_enrichment.rs`)
  - `PromptEnrichmentService` with `enrich()` method
  - `EnrichedPromptMetadata` struct for LLM response parsing
  - `PartialMetadata` struct for preserving user-provided values
  - `EnrichmentStatus` enum (Full, Fallback, Skipped)
  - System prompt for LLM-based metadata generation
  - Graceful fallback when LLM unavailable

- **Integration** (`src/services/prompt.rs`, `src/cli/prompt.rs`, `src/mcp/tools.rs`)
  - `SaveOptions` struct with `skip_enrichment` and `dry_run` flags
  - `SaveResult` struct with template, id, and enrichment status
  - `save_with_enrichment<P: LlmProvider>()` method
  - CLI `--no-enrich` and `--dry-run` flags
  - MCP `skip_enrichment` parameter in prompt_save tool

### Fixed
- **Issue #29**: Variables inside code blocks no longer extracted as template variables

### Documentation
- Updated CLAUDE.md with new features
- Updated `docs/cli/prompt.md` with new options
- Updated `docs/prompts/variables.md` with code block exclusion docs

### Tests
- 16 unit tests for code block detection
- 10 unit tests for enrichment service
- All 685 tests passing

---

## [1.0.0] - 2026-01-01

### Added
- Initial project creation from GitHub Issue #29
- Project scaffold with README.md
- REQUIREMENTS.md with 6 functional requirements (FR-1 through FR-6)
- ARCHITECTURE.md with component designs and data flow diagrams
- IMPLEMENTATION_PLAN.md with 4 phases and 20 tasks
- DECISIONS.md with 7 Architecture Decision Records (ADRs)

### Scope Expansion
- Original scope: Bug fix for code block detection only
- Expanded scope: Added LLM-assisted frontmatter enrichment feature
- User-requested feature: Auto-generate variable descriptions, defaults, tags at save time

### Key Decisions (ADRs)
- ADR-001: Skip fenced code blocks only (not inline code)
- ADR-002: LLM enrichment always on by default (`--no-enrich` to skip)
- ADR-003: Full enrichment scope (description, required, default, validation hints)
- ADR-004: Regex-based code block detection with LazyLock
- ADR-005: Use existing LlmProvider infrastructure
- ADR-006: User frontmatter preservation (merge, don't overwrite)
- ADR-007: Graceful fallback when LLM unavailable

---

## [COMPLETED] - 2026-01-02

### Project Closed
- Final status: Success
- Actual effort: 8 hours (planned: 16-24 hours, ~50% under budget)
- All 4 phases complete (20/20 tasks)
- Moved to: docs/spec/completed/2026-01-01-prompt-variable-context-awareness

### Implementation Delivered
- Code block detection with regex pattern for triple backticks and tildes
- Context-aware variable extraction skipping code blocks
- PromptEnrichmentService with LLM-powered metadata generation
- Graceful fallback when LLM unavailable
- CLI --no-enrich and --dry-run flags
- MCP skip_enrichment parameter
- 685 tests passing, clippy clean

### Retrospective Summary
- What went well: Spec-first planning, existing infrastructure leverage, comprehensive tests
- What to improve: Update PROGRESS.md incrementally during implementation
- Key learning: ADRs prevent scope creep and anchor design decisions
