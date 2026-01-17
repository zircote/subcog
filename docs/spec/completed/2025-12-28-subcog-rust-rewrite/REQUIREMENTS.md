---
document_type: requirements
project_id: SPEC-2025-12-28-001
version: 1.0.0
last_updated: 2025-12-28T16:00:00Z
status: draft
source: docs/research/2025-12-28-rust-rewrite/PRD.md
---

# Subcog: Memory System Rust Rewrite - Product Requirements Document

## Executive Summary

This PRD defines requirements for a complete rewrite of the `git-notes-memory` system from Python to Rust. The rewrite aims to:

1. **Consolidate all validated features** from the Python POC (90%+ success rate)
2. **Improve performance** with native code and single-binary distribution
3. **Implement truly pluggable storage backends** - git notes is just the beginning; Redis, PostgreSQL, Pinecone, and future backends must be first-class citizens through configuration
4. **Implement MCP tools** for AI agent integration
5. **Provide industry best-of-breed observability** - full execution profiling, distributed tracing, structured logging, and audit capabilities
6. **Enable explicit opt-out for enhanced features** - LLM-powered features, consolidation, and other enhancements must be optional; core memory + semantic search must work standalone
7. **Ensure seamless feature integration** - every capability must integrate cohesively with others; no isolated features

---

## Problem Statement

### The Problem

The Python implementation of `git-notes-memory` successfully validated the concept of AI-assisted memory capture and semantic recall, but has inherent limitations:

| Aspect | Python | Rust (Target) |
|--------|--------|---------------|
| **Distribution** | Requires Python runtime, pip, virtualenv | Single static binary |
| **Startup Time** | ~500ms+ with model loading | ~10ms cold start |
| **Memory Usage** | Unpredictable (GC pauses) | Predictable, minimal |
| **Concurrency** | GIL limitations | True parallelism |
| **Type Safety** | Runtime errors | Compile-time guarantees |
| **Security** | Memory safety via runtime | Memory safety via compiler |

### Impact

- Developers lose context across coding sessions
- Decisions are forgotten and repeated
- Knowledge silos between project and personal learnings
- No standardized way for AI agents to access memory

### Current State

The Python POC validated:
- Git notes as persistent storage with YAML front matter
- SQLite + sqlite-vec for semantic search
- Hook-based integration with Claude Code
- LLM-powered implicit capture and consolidation
- Multi-domain memories (project + user scope)
- Secrets filtering and PII protection

---

## Goals and Success Criteria

### Primary Goals

**G1: Full Feature Parity**
Implement all validated features from Python POC without regression.

**G2: Performance Improvement**
- Single-binary distribution (<100MB)
- <10ms cold start
- <30ms capture pipeline
- <50ms search (10K memories)

**G3: Pluggable Storage**
Trait-based storage abstraction supporting multiple backends.

**G4: MCP Tools Integration**
First-class Model Context Protocol tools for AI agent integration.

**G5: Maintainability**
Clean architecture, comprehensive tests, documentation.

### Success Metrics

| Metric | Target | Measurement Method |
|--------|--------|-------------------|
| Test Coverage | ≥80% | cargo tarpaulin |
| Capture Latency | <30ms | p99 in benchmarks |
| Search Latency | <50ms | p99 in benchmarks |
| Binary Size | <100MB | release build |
| Cold Start | <10ms | time to first op |
| Search Accuracy | ~90% relevance | semantic search benchmarks |

### Non-Goals

**NG1: Web Interface**
No web UI; CLI and MCP tools only.

**NG2: Multi-User/Cloud**
Single-user, local-first; no multi-tenant cloud service.

**NG3: Real-Time Sync**
Batch sync on session boundaries; no real-time streaming.

**NG4: Custom Embedding Training**
Use pre-trained models; no fine-tuning infrastructure.

---

## User Analysis

### Primary Users

**Developer with Claude Code**
- Uses Claude Code for daily development
- Wants decisions and learnings captured automatically
- Needs semantic recall of past context
- Values privacy (local-first)

**AI Agent (via MCP)**
- Needs programmatic access to memory system
- Uses MCP tools for capture/recall
- Requires URN-based resource addressing
- Expects consistent JSON responses

### User Stories

#### Memory Capture

| ID | Story | Priority |
|----|-------|----------|
| US-C1 | As a developer, I want to capture decisions during coding sessions so I can recall the rationale later | P0 |
| US-C2 | As a developer, I want inline markers like `[remember]` to capture memories without leaving my workflow | P0 |
| US-C3 | As a developer, I want global memories (preferences, patterns) that persist across projects | P0 |
| US-C4 | As a developer, I want automatic capture of high-confidence insights without manual intervention | P1 |

#### Memory Recall

| ID | Story | Priority |
|----|-------|----------|
| US-R1 | As a developer, I want semantic search to find relevant memories even with different phrasing | P0 |
| US-R2 | As a developer, I want automatic context injection at session start with relevant memories | P0 |
| US-R3 | As a developer, I want to filter memories by namespace, domain, or time range | P1 |
| US-R4 | As a developer, I want LLM-powered reasoning for temporal queries ("when did we decide...") | P2 |

#### Memory Consolidation

| ID | Story | Priority |
|----|-------|----------|
| US-CO1 | As a developer, I want memories to be summarized over time to reduce noise | P2 |
| US-CO2 | As a developer, I want outdated memories to be archived automatically | P2 |
| US-CO3 | As a developer, I want to see relationships between memories (supersedes, references) | P2 |

#### Integration

| ID | Story | Priority |
|----|-------|----------|
| US-I1 | As a developer, I want memory tools available via MCP for any AI agent | P0 |
| US-I2 | As a developer, I want to sync memories across machines via git remote | P0 |
| US-I3 | As a developer, I want secrets filtered from captured memories automatically | P0 |

---

## Feature Tiers

**CRITICAL**: The memory system MUST support explicit opt-out for enhanced features. Users must be able to run the system with only core memory and semantic search capabilities, without requiring LLM providers, external services, or advanced features.

### Tier 1: Core (Always Available, Zero External Dependencies)

| Feature | Description |
|---------|-------------|
| Memory capture | namespace, summary, content, domain |
| Semantic search | Vector similarity via embeddings |
| BM25 full-text search | Keyword-based fallback |
| Hybrid search | RRF fusion of vector + BM25 |
| Git notes persistence | Authoritative storage |
| Index synchronization | git notes <-> search index |
| Multi-domain memories | Project + user scope |
| Progressive hydration | SUMMARY -> FULL -> FILES |
| Basic metrics and logging | Counters, timers |
| MCP tools | capture, recall, status, sync |

### Tier 2: Enhanced (Opt-in, No External Services Required)

| Feature | Description |
|---------|-------------|
| Entity extraction | NER-based matching |
| Temporal extraction | Date/time matching |
| Advanced filtering | Tags, date ranges, specs |
| Memory relationships | edges: references, relates_to |
| Tiered storage | HOT/WARM/COLD without LLM |
| Hook system | Claude Code integration |
| Secrets filtering | PII detection |
| Advanced observability | OTLP export, profiling |

### Tier 3: LLM-Powered (Opt-in, Requires LLM Provider)

| Feature | Description |
|---------|-------------|
| Implicit capture | Auto-detect capture-worthy content |
| Memory consolidation | Clustering + summarization |
| Supersession detection | LLM determines outdated memories |
| Temporal reasoning | "when did we decide..." queries |
| Query expansion | LLM rewrites queries for better recall |
| Smart capture suggestions | AI-powered recommendations |

### Feature Dependencies Matrix

| Feature | Tier | Requires LLM | Requires External Service | Can Disable |
|---------|------|--------------|---------------------------|-------------|
| Memory capture | Core | No | No | No (core) |
| Semantic search | Core | No | No | No (core) |
| BM25 search | Core | No | No | No (core) |
| Hybrid search | Core | No | No | No (core) |
| Git notes storage | Core | No | No | No (core) |
| Index sync | Core | No | No | No (core) |
| Multi-domain | Core | No | No | No (core) |
| Basic metrics | Core | No | No | No (core) |
| MCP tools | Core | No | No | No (core) |
| Entity extraction | Enhanced | No | No | **Yes** |
| Temporal extraction | Enhanced | No | No | **Yes** |
| Hook system | Enhanced | No | No | **Yes** |
| Secrets filtering | Enhanced | No | No | **Yes** |
| OTLP export | Enhanced | No | OTLP endpoint | **Yes** |
| Implicit capture | LLM | **Yes** | LLM provider | **Yes** |
| Consolidation | LLM | **Yes** | LLM provider | **Yes** |
| Supersession detection | LLM | **Yes** | LLM provider | **Yes** |
| Temporal reasoning | LLM | **Yes** | LLM provider | **Yes** |
| Query expansion | LLM | **Yes** | LLM provider | **Yes** |

---

## Functional Requirements

### FR-CAPTURE: Memory Capture

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-C01 | Capture memory with namespace, summary, content | P0 |
| FR-C02 | Store as git note with YAML front matter | P0 |
| FR-C03 | Generate embedding for semantic search | P0 |
| FR-C04 | Index in SQLite with metadata | P0 |
| FR-C05 | Support 10 namespaces (decisions, learnings, etc.) | P0 |
| FR-C06 | Validate summary ≤100 chars, content ≤100KB | P0 |
| FR-C07 | Support domain selection (project/user) | P0 |
| FR-C08 | Graceful degradation if embedding fails | P0 |
| FR-C09 | Return CaptureResult with memory ID and URN | P0 |
| FR-C10 | Atomic file locking for concurrency | P1 |

### FR-RECALL: Memory Recall

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-R01 | Semantic search via vector similarity | P0 |
| FR-R02 | BM25 full-text search fallback | P0 |
| FR-R03 | Hybrid search with RRF fusion | P0 |
| FR-R04 | Filter by namespace, domain, spec, tags | P0 |
| FR-R05 | Configurable result limit (default 10) | P0 |
| FR-R06 | Return MemoryResult with distance score and URN | P0 |
| FR-R07 | Progressive hydration (summary -> full -> files) | P1 |
| FR-R08 | Temporal filtering (date range) | P1 |
| FR-R09 | Entity-based boosting | P2 |
| FR-R10 | LLM query expansion (opt-in) | P2 |

### FR-SYNC: Synchronization

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-S01 | Rebuild index from git notes | P0 |
| FR-S02 | Fetch notes from remote | P0 |
| FR-S03 | Merge notes with cat_sort_uniq strategy | P0 |
| FR-S04 | Push notes to remote | P0 |
| FR-S05 | Idempotent refspec configuration | P0 |
| FR-S06 | Track sync state (last sync timestamp) | P1 |

### FR-HOOKS: Claude Code Integration

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-H01 | SessionStart: Inject context, fetch remote | P0 |
| FR-H02 | UserPromptSubmit: Detect capture markers | P0 |
| FR-H03 | PostToolUse: Surface related memories | P1 |
| FR-H04 | PreCompact: Auto-capture before compaction | P1 |
| FR-H05 | Stop: Session analysis, sync, push | P0 |
| FR-H06 | All hooks output valid JSON | P0 |
| FR-H07 | Hook timing <100ms overhead | P0 |
| FR-H08 | Adaptive token budget for context | P0 |

### FR-DOMAIN: Domain Management

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-D01 | PROJECT domain: repo-scoped memories | P0 |
| FR-D02 | USER domain: global cross-project memories | P0 |
| FR-D03 | User memories in separate bare git repo | P0 |
| FR-D04 | Domain markers ([global], [user]) | P0 |
| FR-D05 | Merged search across domains | P0 |
| FR-D06 | Project memories prioritized in results | P0 |

### FR-SUB: Implicit Capture & Consolidation (LLM-Powered)

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-SUB01 | Provider-agnostic LLM client (Anthropic/OpenAI/Ollama) | P0 |
| FR-SUB02 | Confidence-based auto-capture (0.9+) | P1 |
| FR-SUB03 | Review queue for medium confidence (0.7-0.9) | P1 |
| FR-SUB04 | Adversarial content detection | P1 |
| FR-SUB05 | Tiered storage (HOT/WARM/COLD/ARCHIVED) | P2 |
| FR-SUB06 | Semantic clustering of related memories | P2 |
| FR-SUB07 | LLM-powered summarization of clusters | P2 |
| FR-SUB08 | Supersession detection for contradictions | P2 |
| FR-SUB09 | Memory edge relationships (SUPERSEDES, CONSOLIDATES, REFERENCES) | P2 |
| FR-SUB10 | Retention score with decay formula | P2 |

### FR-SEC: Secrets & PII Filtering

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-SEC01 | Detect secrets (API keys, tokens, passwords) | P0 |
| FR-SEC02 | Detect PII (SSN, credit cards, phones) | P0 |
| FR-SEC03 | Four strategies: REDACT, MASK, BLOCK, WARN | P0 |
| FR-SEC04 | Configurable strategy per secret type | P1 |
| FR-SEC05 | Allowlist for false positives | P1 |
| FR-SEC06 | SOC2/GDPR audit logging | P1 |
| FR-SEC07 | Path traversal prevention | P0 |
| FR-SEC08 | Git command injection prevention | P0 |

---

## Non-Functional Requirements

### Performance

| Requirement | Target |
|-------------|--------|
| Cold start | <10ms |
| Capture pipeline | <30ms |
| Vector search (10K memories) | <50ms |
| BM25 search (10K memories) | <20ms |
| Hybrid search (10K memories) | <80ms |
| Hook overhead | <100ms |
| SessionStart context | <2000ms |
| Embedding generation | <20ms |
| Concurrent captures | 100/s |
| Concurrent searches | 500/s |
| Memory capacity | 100K+ memories |

### Resource Constraints

| Resource | Limit |
|----------|-------|
| Binary size | <100MB |
| Memory (idle) | <50MB |
| Memory (active) | <500MB |
| Disk (per 10K memories) | ~100MB |

### Security

- Detect and handle secrets (API keys, tokens, passwords)
- Detect and handle PII (SSN, credit cards, phones)
- Path traversal prevention in file operations
- Git command injection prevention
- SOC2/GDPR audit trail compliance

### Reliability

| Scenario | Behavior |
|----------|----------|
| LLM provider unavailable | Tier 3 features disabled, Tier 1-2 continue |
| Embedding model unavailable | Graceful degradation to BM25-only |
| OTLP endpoint unavailable | Metrics/traces buffered locally |
| Git remote unavailable | Local operations continue |
| Secrets filter error | Content blocked with warning |

---

## Technical Constraints

### Language and Runtime
- Rust 2021 edition
- MSRV: 1.75+
- Async runtime: Tokio

### Dependencies
- Embeddings: fastembed
- Vector search: usearch
- Git: git2
- Database: rusqlite
- CLI: clap
- MCP: rmcp
- Observability: tracing + opentelemetry

### Compatibility
- macOS ARM64 (primary)
- Linux x86_64
- Windows x86_64 (best effort)

---

## Data Models

### Memory Namespaces

| Namespace | Purpose |
|-----------|---------|
| decisions | Architecture Decision Records |
| learnings | Technical insights and discoveries |
| blockers | Issues and impediments |
| progress | Session progress and milestones |
| reviews | Code review feedback |
| patterns | Reusable patterns and conventions |
| retrospective | Post-mortem insights |
| inception | Project kickoff context |
| elicitation | Requirements gathering |
| research | Research findings |

### Memory Status

| Status | Description |
|--------|-------------|
| active | Current, searchable |
| resolved | Issue resolved, still searchable |
| archived | Historical, excluded from default search |
| tombstone | Superseded, audit access only |

### Memory Tiers

| Tier | Score Range | Description |
|------|-------------|-------------|
| HOT | ≥0.6 | Active, reflexive retrieval |
| WARM | ≥0.3 | Summaries, moderate activity |
| COLD | ≥0.1 | Historical, explicit search only |
| ARCHIVED | <0.1 | Superseded, audit only |

### Storage Domains

| Domain | Scope | Default Storage |
|--------|-------|-----------------|
| `project:{id}` | Repository | Git Notes + usearch |
| `user` | Personal | SQLite + usearch |
| `org:{id}` | Organization | PostgreSQL + pgvector |

---

## MCP Tools Specification

### memory.capture

Capture a new memory to git-backed storage.

**Input Schema:**
```json
{
 "namespace": "decisions|learnings|blockers|progress|reviews|patterns|...",
 "summary": "One-line summary (≤100 chars)",
 "content": "Full markdown content (≤100KB)",
 "domain": "project|user",
 "tags": ["tag1", "tag2"],
 "spec": "optional-spec-reference"
}
```

**Response:**
```json
{
 "success": true,
 "memory_id": "decisions:abc1234:0",
 "uri": "subcog://project:my-app/decisions/abc1234:0",
 "indexed": true,
 "warning": null
}
```

### memory.recall

Search and retrieve relevant memories.

**Input Schema:**
```json
{
 "query": "Search query in natural language",
 "limit": 10,
 "namespace": "optional-filter",
 "domain": "all|project|user",
 "mode": "hybrid|vector|bm25",
 "min_similarity": 0.0
}
```

**Response:**
```json
{
 "results": [
 {
 "uri": "subcog://project:my-app/decisions/abc1234:0",
 "namespace": "decisions",
 "summary": "Use PostgreSQL for data layer",
 "distance": 0.15,
 "domain": "project",
 "timestamp": "2025-01-15T10:30:00Z"
 }
 ],
 "total": 1,
 "resource_template": "subcog://{domain}/{namespace}/{id}"
}
```

### memory.status

Get memory system status and statistics.

### memory.sync

Synchronize memory index with git notes and optionally remote.

### memory.consolidate

Trigger memory consolidation (requires LLM).

### memory.configure

View or update memory system configuration.

---

## Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Rust learning curve | High | Medium | Detailed PRD with code examples |
| Embedding model portability | Medium | High | Use fastembed with bundled models |
| MCP integration complexity | Medium | Medium | Follow official rmcp SDK |
| SQLite-vec alternative | Low | Medium | Use usearch (proven Rust support) |
| Performance regression | Low | High | Benchmarks from day 1 |

---

## Open Questions

- [ ] Exact fastembed model bundling strategy
- [ ] Redis cluster vs single-node for org domain
- [ ] Embedding dimension trade-offs (384 vs 768)
- [ ] Hook timing budget allocation across operations

---

## Glossary

| Term | Definition |
|------|------------|
| URN | Uniform Resource Name for memories: `subcog://{domain}/{namespace}/{id}` |
| MCP | Model Context Protocol - standard for AI agent tools |
| RRF | Reciprocal Rank Fusion - algorithm for combining search results |
| BM25 | Best Match 25 - probabilistic full-text ranking |
| HNSW | Hierarchical Navigable Small World - vector search algorithm |

---

## References

See [RESEARCH_NOTES.md](./RESEARCH_NOTES.md) for background research that informed this specification.
