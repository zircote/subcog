# Specification Changelog

All notable changes to the Subcog Rust Rewrite specification will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

---

## [1.0.0] - 2025-12-28

### Added

#### Core Specification
- Initial specification created from comprehensive research documents
- Full feature parity requirements with Python POC (git-notes-memory)
- Three-tier feature architecture (Core, Enhanced, LLM-powered)
- Five-phase implementation plan

#### Documents Created
- **README.md**: Project metadata, executive summary, document index
- **REQUIREMENTS.md**: Comprehensive PRD with 60+ functional requirements
- **ARCHITECTURE.md**: Technical architecture with three-layer storage
- **IMPLEMENTATION_PLAN.md**: Phased task breakdown with checkboxes
- **DECISIONS.md**: 10 architectural decision records (ADRs)
- **RESEARCH_NOTES.md**: Summary of research findings
- **CHANGELOG.md**: This file

#### Storage Architecture
- Three-layer separation: Persistence, Index, Vector
- Trait-based abstraction for pluggable backends
- Backend implementations: Git Notes, SQLite+usearch, PostgreSQL+pgvector, Redis
- Composite storage orchestration with hybrid search (RRF fusion)

#### Access Interfaces
- CLI interface with clap (capture, recall, status, sync, consolidate, config, serve, hook)
- MCP server with rmcp (6 tools, resources, prompts, subscriptions)
- Hook system for Claude Code integration (5 hooks)
- URN scheme: `subcog://{domain}/{namespace}/{id}`

#### Observability
- OpenTelemetry integration (tracing + metrics)
- OTLP export support
- SOC2/GDPR audit logging
- Structured logging with configurable levels

#### Security
- Secrets detection (API keys, AWS keys, private keys, passwords, JWTs)
- PII detection (SSN, credit cards, phone numbers)
- Four filter strategies: REDACT, MASK, BLOCK, WARN
- Allowlist configuration

#### LLM Integration
- Provider-agnostic LLM client trait
- Support for Anthropic, OpenAI, Ollama, LM Studio
- LLM-powered features: implicit capture, consolidation, temporal reasoning

### Source Documents
Specification generated from research in `docs/research/2025-12-28-rust-rewrite/`:
- PRD.md (v2.1.0)
- STORAGE_AND_OBSERVABILITY.md (v1.0.0)
- MCP_RESOURCES_AND_LLM.md (v1.0.0)
- ACCESS_INTERFACES.md (v1.0.0)
- SEAMLESS_INTEGRATION.md (v1.0.0)
- RESEARCH_PLAN.md (v1.0.0)

---

## [Unreleased]

### Added
- **MCP_AND_HOOKS.md**: Detailed documentation of MCP server lifecycle (long-lived process) and Claude Code hooks integration (short-lived processes)
 - Covers stdio JSON-RPC transport, shared storage layer, concurrency model
 - All 5 hook specifications with JSON contracts and timing requirements
 - Configuration examples for Claude Code `settings.json`
- **CONSOLIDATION_AND_ENRICHMENT.md**: Memory consolidation pipeline and enrichment process
 - Six-stage pipeline: Cluster -> Summarize -> Tier -> Supersede -> Edge -> Persist
 - Memory tiering system (HOT, WARM, COLD, ARCHIVED)
 - Retention score calculation formula and components
 - Enrichment flow to hooks and SessionStart context building
 - XML output format specification for `additionalContext`

### Planned
- Approval workflow completion
- Phase 1 implementation kickoff
- CI/CD pipeline setup

---

## Version History

| Version | Date | Status | Notes |
|---------|------|--------|-------|
| 1.0.0 | 2025-12-28 | Draft | Initial specification |

---

## How to Read This Changelog

### Version Numbers
- **Major (X.0.0)**: Breaking changes to requirements or architecture
- **Minor (0.X.0)**: New features or significant additions
- **Patch (0.0.X)**: Clarifications, typo fixes, minor updates

### Categories
- **Added**: New requirements, features, or documentation
- **Changed**: Modifications to existing specifications
- **Deprecated**: Features planned for removal
- **Removed**: Deleted requirements or features
- **Fixed**: Corrections to errors in the specification
- **Security**: Security-related changes

---

## Contributing to This Changelog

When making changes to the specification:

1. Add an entry under `[Unreleased]`
2. Use the appropriate category
3. Reference the affected document(s)
4. Include rationale for non-trivial changes

Example:
```markdown
### Changed
- **REQUIREMENTS.md**: Updated FR-C06 summary limit from 100 to 150 characters
 - Rationale: User feedback indicated 100 chars too restrictive
```
