# Changelog

All notable changes to this specification will be documented in this file.

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
