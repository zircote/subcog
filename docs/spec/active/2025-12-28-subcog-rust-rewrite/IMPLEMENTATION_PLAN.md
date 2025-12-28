# Implementation Plan

This document defines the phased implementation plan for Subcog, the Rust rewrite of the git-notes-memory system.

---

## Implementation Overview

| Phase | Focus | Key Deliverables | Dependencies |
|-------|-------|------------------|--------------|
| **Phase 1** | Core Foundation | Memory capture, vector search, git notes, CLI | None |
| **Phase 2** | Hook Integration | All 5 Claude Code hooks | Phase 1 |
| **Phase 3** | MCP Server | 6 MCP tools, stdio transport | Phase 1 |
| **Phase 4** | Advanced Features | Multi-domain, hybrid search, secrets, remote sync | Phases 1-3 |
| **Phase 5** | Subconsciousness | LLM client, auto-capture, consolidation | Phase 4 |

---

## Phase 1: Core Foundation (MVP)

**Objective**: Establish the foundation for memory capture, storage, and semantic search.

### 1.1 Project Setup

- [ ] **1.1.1** Initialize Rust project with cargo
  - Create `Cargo.toml` with dependencies
  - Configure edition 2024, MSRV 1.80
  - Set up workspace structure

- [ ] **1.1.2** Configure development tooling
  - Set up clippy with pedantic lints
  - Configure rustfmt (100 char line length)
  - Add cargo-deny for supply chain security
  - Set up cargo-tarpaulin for coverage

- [ ] **1.1.3** Create module structure
  - `src/lib.rs` - Library entry point
  - `src/main.rs` - CLI entry point
  - `src/models/` - Data structures
  - `src/storage/` - Storage abstraction
  - `src/services/` - Business logic
  - `src/git/` - Git operations
  - `src/embedding/` - Embedding generation

### 1.2 Data Models

- [ ] **1.2.1** Implement core types
  - `Memory` struct with all fields
  - `MemoryId` newtype wrapper
  - `MemoryResult` with distance score
  - `Namespace` enum (10 variants)
  - `Domain` enum (Project, User, Org)
  - `MemoryStatus` enum

- [ ] **1.2.2** Implement serialization
  - YAML front matter format (serde_yml)
  - JSON serialization (serde_json)
  - Git notes content format

- [ ] **1.2.3** Add validation
  - Summary ≤100 characters
  - Content ≤100KB
  - ID format validation
  - Timestamp handling (chrono)

### 1.3 Storage Layer

- [ ] **1.3.1** Define storage traits
  - `PersistenceBackend` trait
  - `IndexBackend` trait
  - `VectorBackend` trait
  - Associated types for stats/results

- [ ] **1.3.2** Implement SQLite index backend
  - Create schema with FTS5
  - Implement `index()` method
  - Implement `search_text()` with BM25
  - Implement `search_filter()`
  - Add WAL mode and optimizations

- [ ] **1.3.3** Implement usearch vector backend
  - Initialize HNSW index (384 dimensions)
  - Implement `store_embedding()`
  - Implement `search_knn()`
  - Handle index persistence

- [ ] **1.3.4** Implement CompositeStorage
  - Orchestrate three layers
  - Implement hybrid search (RRF fusion)
  - Add atomic write operations

### 1.4 Git Notes Integration

- [ ] **1.4.1** Implement git notes CRUD
  - Read notes from `refs/notes/mem/{namespace}`
  - Write notes with YAML front matter
  - Update existing notes
  - Delete notes

- [ ] **1.4.2** Implement notes parsing
  - Parse YAML front matter
  - Extract markdown content
  - Handle malformed notes gracefully

- [ ] **1.4.3** Add local sync
  - Rebuild index from git notes
  - Detect stale index
  - Incremental sync

### 1.5 Embedding Generation

- [ ] **1.5.1** Integrate fastembed
  - Load all-MiniLM-L6-v2 model
  - Implement embedding generation
  - Add model caching

- [ ] **1.5.2** Add fallback handling
  - Detect model unavailability
  - Fall back to BM25-only
  - Log degradation warning

- [ ] **1.5.3** Optimize performance
  - Batch embedding generation
  - Cache frequently used embeddings
  - Add progress indicators

### 1.6 Capture Service

- [ ] **1.6.1** Implement CaptureService
  - Validate input
  - Generate embedding
  - Write to git notes
  - Index in SQLite
  - Store in usearch

- [ ] **1.6.2** Add CaptureResult
  - Return memory ID
  - Return URN
  - Include warnings

### 1.7 Recall Service

- [ ] **1.7.1** Implement RecallService
  - Vector search
  - BM25 search
  - Hybrid search (RRF)

- [ ] **1.7.2** Add filtering
  - Namespace filter
  - Domain filter
  - Tag filter
  - Date range filter

- [ ] **1.7.3** Implement hydration
  - Load full content from git notes
  - Progressive hydration (summary → full)

### 1.8 CLI Interface

- [ ] **1.8.1** Implement CLI with clap
  - `capture` subcommand
  - `recall` subcommand
  - `status` subcommand
  - `sync` subcommand
  - Global options (--format, --verbose, --quiet)

- [ ] **1.8.2** Add output formatting
  - Text format (human-readable)
  - JSON format (machine-readable)
  - YAML format

- [ ] **1.8.3** Add stdin support
  - Pipe content to capture
  - Read from file

### 1.9 Testing & Quality

- [ ] **1.9.1** Unit tests
  - Model serialization tests
  - Storage layer tests
  - Service layer tests
  - Target: 80%+ coverage

- [ ] **1.9.2** Integration tests
  - End-to-end capture/recall
  - Git notes round-trip
  - Search accuracy tests

- [ ] **1.9.3** Performance benchmarks
  - Capture latency (<30ms)
  - Search latency (<50ms)
  - Cold start (<10ms)

### Phase 1 Definition of Done

- [ ] Can capture memory via CLI
- [ ] Can search memories semantically
- [ ] Git notes properly created
- [ ] Sync with local git notes works
- [ ] Performance benchmarks pass
- [ ] 80%+ test coverage

---

## Phase 2: Hook Integration

**Objective**: Integrate with all 5 Claude Code hooks for seamless AI assistant experience.

### 2.1 Hook Framework

- [ ] **2.1.1** Create hook CLI subcommand
  - `subcog hook <type>` command
  - Stdin JSON input handling
  - Stdout JSON output

- [ ] **2.1.2** Implement HookHandler trait
  - Input deserialization
  - Output serialization
  - Error handling (never fail)

- [ ] **2.1.3** Add hook configuration
  - Per-hook enable/disable
  - Timing configuration
  - Feature flags

### 2.2 SessionStart Hook

- [ ] **2.2.1** Implement context injection
  - Load relevant memories
  - Format as XML context
  - Respect token budget

- [ ] **2.2.2** Add remote fetch option
  - Fetch from git remote
  - Handle network errors gracefully

- [ ] **2.2.3** Build context builder
  - Project-aware context
  - Branch-aware context
  - Recent memories prioritization

### 2.3 UserPromptSubmit Hook

- [ ] **2.3.1** Implement signal detection
  - Detect [decision] markers
  - Detect [learned] markers
  - Detect [blocker] markers
  - Detect [progress] markers

- [ ] **2.3.2** Generate capture suggestions
  - Extract summary from prompt
  - Infer namespace
  - Calculate confidence

### 2.4 PostToolUse Hook

- [ ] **2.4.1** Implement memory surfacing
  - Detect file operations
  - Search related memories
  - Format as context

- [ ] **2.4.2** Add tool filtering
  - Only trigger for Read/Edit/Write
  - Exclude patterns (*.lock, etc.)

### 2.5 PreCompact Hook

- [ ] **2.5.1** Implement auto-capture
  - Analyze conversation summary
  - Detect capture-worthy content
  - Auto-save with confidence

- [ ] **2.5.2** Add confidence thresholds
  - High confidence: auto-capture
  - Medium: suggest for review

### 2.6 Stop Hook

- [ ] **2.6.1** Implement session finalization
  - Sync index with git notes
  - Optional remote push
  - Session analysis

- [ ] **2.6.2** Add cleanup
  - Flush pending operations
  - Update statistics

### 2.7 Testing

- [ ] **2.7.1** Hook JSON contract tests
  - Valid input/output formats
  - Error case handling
  - Empty input handling

- [ ] **2.7.2** Integration tests
  - Simulate Claude Code calls
  - Verify timing requirements

### Phase 2 Definition of Done

- [ ] All 5 hooks functional
- [ ] Hook timing <100ms (except SessionStart <2000ms)
- [ ] JSON output valid on all paths
- [ ] Integration tests pass

---

## Phase 3: MCP Server

**Objective**: Implement MCP server with all 6 tools for AI agent integration.

### 3.1 MCP Server Setup

- [ ] **3.1.1** Integrate rmcp crate
  - Configure server capabilities
  - Set up stdio transport
  - Handle JSON-RPC protocol

- [ ] **3.1.2** Implement server lifecycle
  - Initialization
  - Shutdown handling
  - Error recovery

### 3.2 MCP Tools

- [ ] **3.2.1** Implement memory.capture tool
  - Input schema validation
  - Call CaptureService
  - Return structured result

- [ ] **3.2.2** Implement memory.recall tool
  - Query parsing
  - Filter application
  - Result formatting

- [ ] **3.2.3** Implement memory.status tool
  - Aggregate statistics
  - Format response

- [ ] **3.2.4** Implement memory.sync tool
  - Local sync
  - Remote sync (optional)

- [ ] **3.2.5** Implement memory.consolidate tool
  - Dry-run support
  - Full/incremental modes

- [ ] **3.2.6** Implement memory.configure tool
  - Get configuration
  - Set configuration

### 3.3 MCP Resources

- [ ] **3.3.1** Implement resource URNs
  - Parse `subcog://mem/{domain}/{namespace}/{id}`
  - Build URNs from memories
  - Resource templates

- [ ] **3.3.2** Implement resource handlers
  - Single memory resource
  - Namespace listing
  - Domain listing

- [ ] **3.3.3** Add subscriptions
  - Resource change notifications
  - Subscription management

### 3.4 MCP Prompts

- [ ] **3.4.1** Implement pre-defined prompts
  - capture-decision prompt
  - recall-context prompt

### 3.5 Testing

- [ ] **3.5.1** Tool contract tests
  - Schema validation
  - Error responses

- [ ] **3.5.2** Integration tests
  - Full MCP session simulation

### Phase 3 Definition of Done

- [ ] memory.capture works
- [ ] memory.recall works
- [ ] memory.status works
- [ ] memory.sync works
- [ ] memory.consolidate works
- [ ] memory.configure works
- [ ] Resources addressable via URN

---

## Phase 4: Advanced Features

**Objective**: Implement multi-domain, secrets filtering, remote sync, and observability.

### 4.1 Multi-Domain Memories

- [ ] **4.1.1** Implement domain separation
  - PROJECT domain (repo-scoped)
  - USER domain (global, separate bare repo)
  - ORG domain (optional)

- [ ] **4.1.2** Add domain markers
  - Detect [global], [user] markers
  - Auto-assign domain

- [ ] **4.1.3** Implement merged search
  - Search across domains
  - Project memories prioritized

### 4.2 Secrets Filtering

- [ ] **4.2.1** Implement secret detection
  - API key patterns
  - AWS key patterns
  - Private key detection
  - Password detection
  - JWT token detection

- [ ] **4.2.2** Implement PII detection
  - SSN with checksum
  - Credit cards (Luhn)
  - Phone numbers (E.164)

- [ ] **4.2.3** Add filter strategies
  - REDACT: Replace with [REDACTED:type]
  - MASK: Partial content (abc...xyz)
  - BLOCK: Reject entirely
  - WARN: Pass with warning

- [ ] **4.2.4** Add allowlist support
  - Configuration-driven
  - Per-pattern overrides

### 4.3 Remote Sync

- [ ] **4.3.1** Implement fetch
  - Fetch from git remote
  - Handle network errors
  - Merge with cat_sort_uniq

- [ ] **4.3.2** Implement push
  - Push to git remote
  - Handle conflicts
  - Idempotent refspec

- [ ] **4.3.3** Add sync state
  - Track last sync timestamp
  - Detect sync needed

### 4.4 Audit Logging

- [ ] **4.4.1** Implement audit events
  - memory.created
  - memory.deleted
  - memory.accessed
  - secrets.detected
  - sync.remote
  - config.changed

- [ ] **4.4.2** Add audit storage
  - JSON log files
  - Rotation policy
  - Retention (90 days default)

### 4.5 Observability

- [ ] **4.5.1** Add metrics
  - Counters (operations, searches)
  - Histograms (latencies)
  - Gauges (memory counts)

- [ ] **4.5.2** Add tracing
  - Instrument all operations
  - Span attributes
  - Context propagation

- [ ] **4.5.3** Add logging
  - Structured JSON logs
  - Log levels
  - stderr output

- [ ] **4.5.4** Add OTLP export
  - Configure endpoint
  - gRPC/HTTP protocols
  - Buffering on failure

### 4.6 Testing

- [ ] **4.6.1** Domain tests
  - Cross-domain search
  - Domain isolation

- [ ] **4.6.2** Security tests
  - Secret detection accuracy
  - PII detection accuracy
  - Filter strategy tests

- [ ] **4.6.3** Sync tests
  - Remote fetch/push
  - Conflict resolution

### Phase 4 Definition of Done

- [ ] User domain captures work
- [ ] Hybrid search improves accuracy
- [ ] Secrets properly redacted
- [ ] Remote sync functional
- [ ] Audit logs generated
- [ ] Metrics/traces exported

---

## Phase 5: Subconsciousness (LLM-Powered)

**Objective**: Implement LLM-powered features for implicit capture, consolidation, and temporal reasoning.

### 5.1 LLM Client Abstraction

- [ ] **5.1.1** Define LLMProvider trait
  - Chat completion method
  - Health check
  - Provider name

- [ ] **5.1.2** Implement Anthropic client
  - Claude API integration
  - API key from env
  - Error handling

- [ ] **5.1.3** Implement OpenAI client
  - OpenAI API integration
  - Model selection

- [ ] **5.1.4** Implement Ollama client
  - Local model support
  - No API key required

- [ ] **5.1.5** Implement LM Studio client
  - OpenAI-compatible endpoint
  - Local model support

### 5.2 Implicit Capture

- [ ] **5.2.1** Implement content analysis
  - Detect capture-worthy content
  - Classify namespace
  - Generate summary

- [ ] **5.2.2** Add confidence scoring
  - High (0.9+): auto-capture
  - Medium (0.7-0.9): review queue
  - Low (<0.7): skip

- [ ] **5.2.3** Add adversarial detection
  - Detect prompt injection
  - Filter malicious content

### 5.3 Memory Consolidation

- [ ] **5.3.1** Implement clustering
  - Semantic clustering
  - Minimum/maximum cluster size
  - Similarity threshold

- [ ] **5.3.2** Implement summarization
  - Generate cluster summaries
  - Create summary memories
  - Link to sources

- [ ] **5.3.3** Implement tiered storage
  - HOT tier (score ≥0.6)
  - WARM tier (score ≥0.3)
  - COLD tier (score <0.3)
  - ARCHIVED (superseded)

- [ ] **5.3.4** Implement retention scoring
  - Recency factor (exponential decay)
  - Activation factor (retrieval count)
  - Importance factor (namespace weight)
  - Supersession penalty

### 5.4 Supersession Detection

- [ ] **5.4.1** Implement contradiction detection
  - Compare new vs existing memories
  - LLM-based analysis

- [ ] **5.4.2** Implement edge creation
  - SUPERSEDES relationship
  - CONSOLIDATES relationship
  - REFERENCES relationship

### 5.5 Temporal Reasoning

- [ ] **5.5.1** Implement temporal queries
  - "when did we decide..."
  - "what changed since..."

- [ ] **5.5.2** Add LLM reasoning
  - Parse temporal intent
  - Search with context
  - Generate narrative response

### 5.6 Query Expansion

- [ ] **5.6.1** Implement query rewriting
  - LLM expands query
  - Multiple search terms

### 5.7 Testing

- [ ] **5.7.1** LLM client tests
  - Mock responses
  - Error handling

- [ ] **5.7.2** Consolidation tests
  - Clustering accuracy
  - Tier assignment

- [ ] **5.7.3** Feature flag tests
  - LLM features disabled
  - Graceful degradation

### Phase 5 Definition of Done

- [ ] Provider-agnostic LLM client
- [ ] Auto-capture with confidence
- [ ] Tier assignment working
- [ ] Consolidation produces summaries
- [ ] Temporal reasoning functional

---

## Cross-Cutting Concerns

### Configuration

- [ ] TOML configuration file support
- [ ] Environment variable overrides
- [ ] Validation on startup
- [ ] Feature flag runtime checking

### Documentation

- [ ] README with quick start
- [ ] CLI help text (clap derived)
- [ ] API documentation (rustdoc)
- [ ] MCP tool schema documentation

### CI/CD Pipeline

- [ ] GitHub Actions workflow
- [ ] Format check (`cargo fmt`)
- [ ] Lint (`cargo clippy`)
- [ ] Test (`cargo test`)
- [ ] Coverage reporting
- [ ] Supply chain audit (`cargo deny`)
- [ ] MSRV check
- [ ] Binary release automation

### Migration

- [ ] Python → Rust migration tool
- [ ] Data verification
- [ ] Hook configuration update

---

## Success Criteria

### Quantitative

| Metric | Target | Measurement |
|--------|--------|-------------|
| Test Coverage | ≥80% | cargo tarpaulin |
| Capture Latency | <30ms | p99 in benchmarks |
| Search Latency | <50ms | p99 in benchmarks |
| Binary Size | <100MB | release build |
| Cold Start | <10ms | time to first op |

### Qualitative

- [ ] All Python features have Rust equivalents
- [ ] Documentation complete
- [ ] No data corruption in stress tests
- [ ] Graceful degradation for all failure modes
- [ ] Clean architecture (no circular dependencies)

### User Acceptance

- [ ] Existing users can migrate without data loss
- [ ] CLI UX matches or exceeds Python version
- [ ] MCP tools work with Claude Desktop
- [ ] Performance improvement noticeable

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2025-12-28 | Claude Opus 4.5 | Initial implementation plan from research documents |
