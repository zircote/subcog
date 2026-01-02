# Subcog Deficiency Remediation Plan

**Source:** [FEATURES_REPORT.md](./FEATURES_REPORT.md)
**Created:** 2026-01-01
**Target:** Close all 14 identified deficiencies

---

## Overview

| Priority | Category | Tasks | Effort |
|----------|----------|-------|--------|
| P1 | STUB Implementations | 2 items | High |
| P2 | PARTIAL Implementations | 4 items | Medium-High |
| P3 | MISSING Features | 6 items | Low-Medium |
| **Total** | | **14 items** | |

---

## Phase 1: Critical STUB Implementations (P1)

### 1.1 Redis Vector Backend

**File:** `src/storage/vector/redis.rs`
**Impact:** Distributed/cloud deployments blocked
**Reference:** docs/storage/vector.md

#### Tasks

- [x] **1.1.1** Research Redis Stack vector similarity search API ✓
  - [x] Review Redis VSS documentation
  - [x] Identify required Redis modules (RediSearch 2.4+)
  - [x] Document connection requirements

- [x] **1.1.2** Implement `VectorBackend` trait methods ✓
  - [x] `dimensions()` - Return configured dimension count
  - [x] `upsert()` - Store vector with `FT.CREATE` / `HSET`
  - [x] `search()` - KNN search with `FT.SEARCH ... KNN`
  - [x] `remove()` - Delete vector with `DEL` / `FT.DEL`
  - [x] `count()` - Count vectors with `FT.INFO`
  - [x] `clear()` - Truncate index

- [x] **1.1.3** Add configuration support ✓
  - [x] Redis connection string in config
  - [x] Index name configuration
  - [x] Vector dimension validation

- [x] **1.1.4** Write tests ✓
  - [x] Unit tests with mock Redis
  - [x] Integration tests (requires Redis Stack)
  - [x] Add to CI with Redis container

- [x] **1.1.5** Update documentation ✓
  - [x] docs/storage/vector.md - Redis configuration
  - [x] example.config.toml - Redis vector example

---

### 1.2 HTTP Transport for MCP Server

**File:** `src/mcp/server.rs:154`
**Impact:** External MCP clients cannot connect
**Reference:** docs/mcp/README.md lines 58-60

#### Tasks

- [x] **1.2.1** Design HTTP transport layer ✓
  - [x] Choose HTTP framework (axum recommended)
  - [x] Define endpoint structure (`/v1/mcp`)
  - [x] Plan SSE for server-initiated messages

- [x] **1.2.2** Implement HTTP server ✓
  - [x] Create `HttpTransport` struct
  - [x] Implement JSON-RPC over HTTP POST
  - [x] Add CORS configuration
  - [x] Implement SSE for subscriptions

- [x] **1.2.3** Add CLI options ✓
  - [x] `--transport http` flag in `serve` command
  - [x] `--port` flag (default: 8080)
  - [x] `--host` flag (default: 127.0.0.1)

- [x] **1.2.4** Security considerations ✓
  - [x] Optional API key authentication
  - [x] Rate limiting
  - [x] Request size limits

- [x] **1.2.5** Write tests ✓
  - [x] Unit tests for HTTP handler
  - [x] Integration tests for full request cycle
  - [x] SSE subscription tests

- [x] **1.2.6** Update documentation ✓
  - [x] docs/mcp/protocol.md - HTTP transport details
  - [x] docs/cli/serve.md - HTTP options

---

## Phase 2: PARTIAL Implementations (P2)

### 2.1 usearch HNSW Integration

**File:** `src/storage/vector/usearch.rs`
**Issue:** Uses O(n) brute-force, not actual HNSW
**Reference:** docs/storage/README.md line 60

#### Tasks

- [x] **2.1.1** Add usearch crate dependency
  - [x] Add `usearch` to Cargo.toml
  - [x] Verify SIMD/platform compatibility
  - [x] Configure feature flags

- [x] **2.1.2** Refactor to use usearch Index
  - [x] Replace `HashMap<String, Vec<f32>>` with `usearch::Index`
  - [x] Configure HNSW parameters (ef_construction, M)
  - [x] Implement proper ANN search

- [x] **2.1.3** Maintain file persistence
  - [x] Use usearch's native save/load
  - [x] Fallback to JSON for compatibility

- [x] **2.1.4** Benchmark improvements
  - [x] Add benchmarks in `benches/`
  - [x] Compare brute-force vs HNSW at various scales
  - [x] Document performance characteristics

- [x] **2.1.5** Update tests
  - [x] Verify search accuracy within tolerance
  - [x] Test with 10k+ vectors

---

### 2.2 PreCompact Hook Enhancements

**File:** `src/hooks/pre_compact.rs`
**Reference:** docs/hooks/pre-compact.md lines 140-157

#### Tasks

- [x] **2.2.1** Implement semantic similarity deduplication
  - [x] Add embedding generation for candidates
  - [x] Compute cosine similarity against existing memories
  - [x] Skip if >90% similar memory exists
  - [x] Add `EmbeddingService` dependency injection

- [x] **2.2.2** Implement recent capture check
  - [x] Query index for memories captured in last 5 minutes
  - [x] Skip candidates matching recent content
  - [x] Add configurable window (`SUBCOG_AUTO_CAPTURE_WINDOW_SECS`)

- [x] **2.2.3** Add context language detection
  - [x] Add `contains_context_language()` function
  - [x] Detect: "because", "constraint", "requirement", "context:", "important:", "note:"
  - [x] Map to `Namespace::Context`

- [x] **2.2.4** Optional LLM analysis mode
  - [x] Add `--llm-analyze` flag
  - [x] Use LLM to classify ambiguous content
  - [x] Add configuration `SUBCOG_AUTO_CAPTURE_USE_LLM`

- [x] **2.2.5** Update tests
  - [x] Test semantic deduplication logic
  - [x] Test time-based deduplication
  - [x] Test context language detection

---

### 2.3 Stop Hook Enhancements

**File:** `src/hooks/stop.rs`
**Reference:** docs/hooks/stop.md lines 40-54, 129-134

#### Tasks

- [x] **2.3.1** Extend `SessionSummary` struct
  - [x] Add `namespace_counts: HashMap<String, NamespaceStats>`
  - [x] Add `tags_used: Vec<(String, usize)>`
  - [x] Add `query_patterns: Vec<String>`
  - [x] Add `resources_read: Vec<String>`

- [x] **2.3.2** Implement namespace breakdown
  - [x] Track captures per namespace during session
  - [x] Track recalls per namespace
  - [x] Format as table in output

- [x] **2.3.3** Implement tags analysis
  - [x] Collect tags from captures
  - [x] Rank by frequency
  - [x] Include top 10 in summary

- [x] **2.3.4** Implement query pattern tracking
  - [x] Log search queries during session
  - [x] Identify common patterns
  - [x] Suggest related memories

- [x] **2.3.5** Implement resources tracking
  - [x] Track MCP resources read
  - [x] Count unique resources
  - [x] Include in summary

- [x] **2.3.6** Update tests
  - [x] Verify namespace breakdown in output
  - [x] Verify tags analysis
  - [x] Verify query patterns

---

### 2.4 Org Scope Prompts

**File:** `src/storage/prompt/mod.rs:92`
**Reference:** docs/prompts/storage.md

#### Tasks

- [x] **2.4.1** Design org-scope storage
  - [x] Define org identifier resolution
  - [x] Plan storage path (`~/.config/subcog/orgs/{org}/prompts/`)
  - [x] Handle org membership

- [x] **2.4.2** Implement org-scope in `PromptStorageBackend`
  - [x] Add `DomainScope::Org` handling in each backend
  - [x] Filesystem: org directory structure
  - [x] SQLite: org column in prompts table
  - [x] Git Notes: org namespace in refs

- [x] **2.4.3** Add org configuration
  - [x] `SUBCOG_ORG` environment variable
  - [x] Config file `org` field
  - [x] Auto-detect from git remote

- [x] **2.4.4** Update MCP resources
  - [x] Enable `subcog://org/_prompts`
  - [x] Enable `subcog://org/_prompts/{name}`

- [x] **2.4.5** Write tests
  - [x] Test org-scope CRUD operations
  - [x] Test domain cascade (project → user → org)

---

## Phase 3: MISSING Features (P3)

### 3.1 `namespaces` CLI Command

**Documentation:** docs/cli/namespaces.md
**Target:** `src/cli/namespaces.rs`

#### Tasks

- [x] **3.1.1** Create `src/cli/namespaces.rs`
  - [x] Define `NamespacesArgs` struct
  - [x] Implement `run()` function
  - [x] List all namespaces with descriptions

- [x] **3.1.2** Add to CLI module
  - [x] Register in `src/cli/mod.rs`
  - [x] Add `Namespaces` variant to `Commands` enum

- [x] **3.1.3** Implement output formats
  - [x] Table format (default)
  - [x] JSON format (`--json`)
  - [x] Include signal words and descriptions

- [x] **3.1.4** Write tests
  - [x] Test table output
  - [x] Test JSON output

---

### 3.2 `subcog://namespaces` MCP Resource

**Documentation:** docs/mcp/resources.md lines 207-208
**File:** `src/mcp/resources.rs`

#### Tasks

- [x] **3.2.1** Add to `list_resources()`
  - [x] Add `subcog://namespaces` ResourceDefinition
  - [x] Add `subcog://namespaces/{ns}` template

- [x] **3.2.2** Implement `get_namespaces_resource()`
  - [x] Handle `subcog://namespaces` - list all
  - [x] Handle `subcog://namespaces/{ns}` - get memories in namespace

- [x] **3.2.3** Add routing in `get_resource()`
  - [x] Add `"namespaces"` case in match statement
  - [x] Route to `get_namespaces_resource()`

- [x] **3.2.4** Write tests
  - [x] Test list namespaces
  - [x] Test get namespace memories

---

### 3.3 `subcog://_prompts` Aggregate Resource

**Documentation:** docs/mcp/resources.md line 171
**File:** `src/mcp/resources.rs`

#### Tasks

- [x] **3.3.1** Add to `list_resources()`
  - [x] Add `subcog://_prompts` ResourceDefinition

- [x] **3.3.2** Implement aggregate prompts handler
  - [x] Query all domains (project, user, org)
  - [x] Combine and deduplicate by name
  - [x] Return merged list

- [x] **3.3.3** Update resource routing
  - [x] Handle `_prompts` in cross-domain route
  - [x] Distinguish from namespace patterns

- [x] **3.3.4** Write tests
  - [x] Test aggregate listing
  - [x] Test deduplication

---

### 3.4 `generate_tutorial` MCP Prompt

**Documentation:** docs/mcp/prompts.md
**File:** `src/mcp/prompts.rs`

#### Tasks

- [x] **3.4.1** Define prompt template
  - [x] Design tutorial generation structure
  - [x] Define input parameters (topic, level, format)
  - [x] Create comprehensive prompt content

- [x] **3.4.2** Add to prompts list
  - [x] Register in `list_prompts()`
  - [x] Implement `get_prompt()` handler

- [x] **3.4.3** Write tests
  - [x] Test prompt retrieval
  - [x] Test parameter substitution

---

### 3.5 Shell Completions

**Documentation:** docs/cli/README.md lines 76-87
**File:** `src/main.rs` or `src/cli/mod.rs`

#### Tasks

- [x] **3.5.1** Add clap completions feature
  - [x] Add `clap_complete` dependency
  - [x] Enable `derive` feature for completions

- [x] **3.5.2** Implement `completions` subcommand
  - [x] Add `Completions` command variant
  - [x] Accept shell type argument (bash, zsh, fish, powershell)
  - [x] Generate completion script to stdout

- [x] **3.5.3** Update documentation
  - [x] Add installation instructions per shell
  - [x] Include in docs/cli/README.md

- [x] **3.5.4** Write tests
  - [x] Test script generation for each shell

---

### 3.6 `prompt import/share` Subcommands

**Documentation:** docs/prompts/mcp.md
**File:** `src/cli/prompt.rs`

#### Tasks

- [x] **3.6.1** Implement `import` subcommand
  - [x] Accept file path or URL
  - [x] Parse prompt format (YAML, JSON, MD)
  - [x] Validate and save to target domain

- [x] **3.6.2** Implement `share` subcommand
  - [x] Export prompt to file
  - [x] Support output formats (YAML, JSON, MD)
  - [x] Include metadata and variables

- [x] **3.6.3** Add to CLI module
  - [x] Add `Import` and `Share` subcommands
  - [x] Wire up to prompt service

- [x] **3.6.4** Write tests
  - [x] Test import from file
  - [x] Test export to file
  - [x] Test round-trip (export → import)

---

## Verification Checklist

### Per-Task Verification

- [x] Code compiles without warnings (`cargo build`)
- [x] All tests pass (`cargo test`)
- [x] Clippy clean (`cargo clippy --all-targets`)
- [x] Format correct (`cargo fmt -- --check`)
- [x] Documentation updated

### Phase Completion Gates

#### Phase 1 Complete When:
- [x] Redis Vector backend passes integration tests
- [x] HTTP transport serves MCP requests
- [x] `make ci` passes

#### Phase 2 Complete When:
- [x] usearch benchmarks show O(log n) performance
- [x] PreCompact hook semantic dedup verified
- [x] Stop hook summary shows namespace breakdown
- [x] Org prompts CRUD functional
- [x] `make ci` passes

#### Phase 3 Complete When:
- [x] `subcog namespaces` outputs table
- [x] All MCP resources listed in docs are functional
- [x] Shell completions install correctly
- [x] `make ci` passes

---

## Execution Notes

### Parallel Execution Opportunities

These task groups can be executed in parallel:

1. **Group A (Storage):** 1.1 Redis Vector + 2.1 usearch HNSW
2. **Group B (MCP):** 1.2 HTTP Transport + 3.2-3.4 MCP Resources/Prompts
3. **Group C (Hooks):** 2.2 PreCompact + 2.3 Stop
4. **Group D (CLI):** 3.1 namespaces + 3.5 completions + 3.6 import/share

### Dependencies

```
1.1 Redis Vector ──────────────────────────────────────┐
                                                       ├─→ Phase 1 Complete
1.2 HTTP Transport ────────────────────────────────────┘

2.1 usearch HNSW ──────────────────────┐
                                       │
2.2 PreCompact (requires embedding) ───┼───────────────┐
                                       │               │
2.3 Stop Hook ─────────────────────────┤               ├─→ Phase 2 Complete
                                       │               │
2.4 Org Prompts ───────────────────────┘               │
                                                       │
3.1 namespaces CLI ────────────────────────────────────┤
                                                       │
3.2 subcog://namespaces (depends on 3.1) ──────────────┤
                                                       ├─→ Phase 3 Complete
3.3 subcog://_prompts (depends on 2.4) ────────────────┤
                                                       │
3.4 generate_tutorial ─────────────────────────────────┤
                                                       │
3.5 Shell completions ─────────────────────────────────┤
                                                       │
3.6 prompt import/share ───────────────────────────────┘
```

---

## Success Metrics

| Metric | Target |
|--------|--------|
| Implementation Rate | 100% (up from 81%) |
| STUB items | 0 (down from 2) |
| PARTIAL items | 0 (down from 6) |
| MISSING items | 0 (down from 6) |
| All tests passing | Yes |
| CI pipeline green | Yes |

---

*Plan generated from FEATURES_REPORT.md audit.*
