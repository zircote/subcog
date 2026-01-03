# Changelog

All notable changes to this specification will be documented in this file.

## [1.0.0] - 2026-01-03

### Added
- **REQUIREMENTS.md**: Complete PRD with 10 P0, 8 P1, and 5 P2 requirements
- **ARCHITECTURE.md**: Technical design with 10 component specifications
- **IMPLEMENTATION_PLAN.md**: 5-phase plan with 32 tasks (24-40 hours estimated)
- **DECISIONS.md**: 7 Architecture Decision Records (ADRs)
- **RESEARCH_NOTES.md**: Codebase analysis and best practices research

### Key Decisions
- ADR-001: Remove git-notes storage layer (fixes critical capture bug)
- ADR-002: Consolidate to user-level storage with faceting
- ADR-003: Inline facet columns (denormalized)
- ADR-004: Fresh start - no migration of legacy data
- ADR-005: Feature-gate org-scope implementation
- ADR-006: Lazy branch garbage collection
- ADR-007: Tombstone pattern for soft deletes

### Status
- Moved to **In Review** - ready for stakeholder approval

## [0.1.0] - 2026-01-03

### Added
- Initial project creation from GitHub Issue #43
- Requirements elicitation completed
- Project workspace initialized at `docs/spec/active/2026-01-03-storage-simplification/`
