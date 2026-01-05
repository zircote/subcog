---
project_id: SPEC-2025-12-28-001
project_name: "Subcog: Memory System Rust Rewrite"
slug: subcog-rust-rewrite
status: draft
created: 2025-12-28T16:00:00Z
approved: null
started: null
completed: null
expires: 2026-03-28T16:00:00Z
superseded_by: null
tags: [rust, memory-system, mcp, claude-code, semantic-search, git-notes]
stakeholders: []
source_documents:
  - docs/research/2025-12-28-rust-rewrite/PRD.md
  - docs/research/2025-12-28-rust-rewrite/STORAGE_AND_OBSERVABILITY.md
  - docs/research/2025-12-28-rust-rewrite/MCP_RESOURCES_AND_LLM.md
  - docs/research/2025-12-28-rust-rewrite/ACCESS_INTERFACES.md
  - docs/research/2025-12-28-rust-rewrite/SEAMLESS_INTEGRATION.md
  - docs/research/2025-12-28-rust-rewrite/RESEARCH_PLAN.md
---

# Subcog: Memory System Rust Rewrite

## Executive Summary

Complete rewrite of the `git-notes-memory` Python system in Rust, delivering:

- **Single-binary distribution** (<100MB, <10ms cold start)
- **Pluggable storage backends** (Git Notes, SQLite+usearch, PostgreSQL+pgvector, Redis)
- **MCP tools integration** for AI agent interoperability
- **Three-tier feature architecture** (Core, Enhanced, LLM-powered) with explicit opt-out
- **Industry-grade observability** (OpenTelemetry, OTLP export, audit logging)
- **Full Claude Code hook integration** (all 5 hooks)

## Project Scope

### In Scope

- Full feature parity with Python POC (validated at 90%+ success rate)
- Rust implementation with trait-based storage abstraction
- MCP server with 6 tools (capture, recall, status, sync, consolidate, configure)
- Multi-domain memories (project, user, org)
- Semantic search (vector + BM25 hybrid with RRF fusion)
- Secrets filtering and PII detection
- LLM-powered features (implicit capture, consolidation, temporal reasoning)

### Out of Scope

- Web interface (CLI and MCP only)
- Multi-user cloud service (local-first, single-user)
- Real-time sync (batch on session boundaries)
- Custom embedding model training

## Key Design Principles

| Principle | Description |
|-----------|-------------|
| **Pluggable Storage** | Backend selection via configuration; git notes, Redis, PostgreSQL+pgvector all supported |
| **Feature Tiers** | Core tier works without LLM; Enhanced and LLM tiers are opt-in |
| **Full Observability** | Every operation is traceable, measurable, and auditable |
| **Seamless Integration** | Features compose cleanly; no feature works in isolation |
| **Configuration-Driven** | All backends and features selectable through unified config |

## Documents

| Document | Purpose |
|----------|---------|
| [REQUIREMENTS.md](./REQUIREMENTS.md) | Product Requirements Document |
| [ARCHITECTURE.md](./ARCHITECTURE.md) | Technical architecture and design |
| [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md) | Phased task breakdown |
| [DECISIONS.md](./DECISIONS.md) | Architecture Decision Records |
| [RESEARCH_NOTES.md](./RESEARCH_NOTES.md) | Research findings summary |
| [MCP_AND_HOOKS.md](./MCP_AND_HOOKS.md) | MCP server and Claude Code hooks integration |
| [CONSOLIDATION_AND_ENRICHMENT.md](./CONSOLIDATION_AND_ENRICHMENT.md) | Memory consolidation pipeline and enrichment |
| [CHANGELOG.md](./CHANGELOG.md) | Specification evolution history |

## Quick Stats

- **Total Requirements**: 60+ functional requirements across 7 categories
- **MCP Tools**: 6 tools with full JSON schema
- **Storage Backends**: 4 (Git Notes, SQLite+usearch, PostgreSQL+pgvector, Redis)
- **LLM Providers**: 4 (Anthropic, OpenAI, Ollama, LMStudio)
- **Hooks**: 5 Claude Code hooks (SessionStart, UserPromptSubmit, PostToolUse, PreCompact, Stop)
- **Estimated Phases**: 5 phases (Foundation, Hooks, MCP, Advanced, Subconsciousness)

## Source Research

This specification was generated from comprehensive research documents:

- **PRD.md** (v2.1.0) - Core requirements, architecture overview, phasing
- **STORAGE_AND_OBSERVABILITY.md** - Three-layer storage, trait definitions, observability
- **MCP_RESOURCES_AND_LLM.md** - URN scheme (`subcog://{domain}/{namespace}/{id}`), LLM providers
- **ACCESS_INTERFACES.md** - CLI, MCP server, streaming API, hooks
- **SEAMLESS_INTEGRATION.md** - Event bus, pipeline composition, error propagation

## Next Steps

1. Review specification documents
2. Run `/claude-spec:approve subcog-rust-rewrite` when ready
3. Begin Phase 1 implementation via `/claude-spec:implement`
