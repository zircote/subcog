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

- [ ] **1.1.1** Research Redis Stack vector similarity search API
  - [ ] Review Redis VSS documentation
  - [ ] Identify required Redis modules (RediSearch 2.4+)
  - [ ] Document connection requirements

- [ ] **1.1.2** Implement `VectorBackend` trait methods
  - [ ] `dimensions()` - Return configured dimension count
  - [ ] `upsert()` - Store vector with `FT.CREATE` / `HSET`
  - [ ] `search()` - KNN search with `FT.SEARCH ... KNN`
  - [ ] `remove()` - Delete vector with `DEL` / `FT.DEL`
  - [ ] `count()` - Count vectors with `FT.INFO`
  - [ ] `clear()` - Truncate index

- [ ] **1.1.3** Add configuration support
  - [ ] Redis connection string in config
  - [ ] Index name configuration
  - [ ] Vector dimension validation

- [ ] **1.1.4** Write tests
  - [ ] Unit tests with mock Redis
  - [ ] Integration tests (requires Redis Stack)
  - [ ] Add to CI with Redis container

- [ ] **1.1.5** Update documentation
  - [ ] docs/storage/vector.md - Redis configuration
  - [ ] example.config.toml - Redis vector example

---

### 1.2 HTTP Transport for MCP Server

**File:** `src/mcp/server.rs:154`
**Impact:** External MCP clients cannot connect
**Reference:** docs/mcp/README.md lines 58-60

#### Tasks

- [ ] **1.2.1** Design HTTP transport layer
  - [ ] Choose HTTP framework (axum recommended)
  - [ ] Define endpoint structure (`/v1/mcp`)
  - [ ] Plan SSE for server-initiated messages

- [ ] **1.2.2** Implement HTTP server
  - [ ] Create `HttpTransport` struct
  - [ ] Implement JSON-RPC over HTTP POST
  - [ ] Add CORS configuration
  - [ ] Implement SSE for subscriptions

- [ ] **1.2.3** Add CLI options
  - [ ] `--transport http` flag in `serve` command
  - [ ] `--port` flag (default: 8080)
  - [ ] `--host` flag (default: 127.0.0.1)

- [ ] **1.2.4** Security considerations
  - [ ] Optional API key authentication
  - [ ] Rate limiting
  - [ ] Request size limits

- [ ] **1.2.5** Write tests
  - [ ] Unit tests for HTTP handler
  - [ ] Integration tests for full request cycle
  - [ ] SSE subscription tests

- [ ] **1.2.6** Update documentation
  - [ ] docs/mcp/protocol.md - HTTP transport details
  - [ ] docs/cli/serve.md - HTTP options

---

## Phase 2: PARTIAL Implementations (P2)

### 2.1 usearch HNSW Integration

**File:** `src/storage/vector/usearch.rs`
**Issue:** Uses O(n) brute-force, not actual HNSW
**Reference:** docs/storage/README.md line 60

#### Tasks

- [ ] **2.1.1** Add usearch crate dependency
  - [ ] Add `usearch` to Cargo.toml
  - [ ] Verify SIMD/platform compatibility
  - [ ] Configure feature flags

- [ ] **2.1.2** Refactor to use usearch Index
  - [ ] Replace `HashMap<String, Vec<f32>>` with `usearch::Index`
  - [ ] Configure HNSW parameters (ef_construction, M)
  - [ ] Implement proper ANN search

- [ ] **2.1.3** Maintain file persistence
  - [ ] Use usearch's native save/load
  - [ ] Fallback to JSON for compatibility

- [ ] **2.1.4** Benchmark improvements
  - [ ] Add benchmarks in `benches/`
  - [ ] Compare brute-force vs HNSW at various scales
  - [ ] Document performance characteristics

- [ ] **2.1.5** Update tests
  - [ ] Verify search accuracy within tolerance
  - [ ] Test with 10k+ vectors

---

### 2.2 PreCompact Hook Enhancements

**File:** `src/hooks/pre_compact.rs`
**Reference:** docs/hooks/pre-compact.md lines 140-157

#### Tasks

- [ ] **2.2.1** Implement semantic similarity deduplication
  - [ ] Add embedding generation for candidates
  - [ ] Compute cosine similarity against existing memories
  - [ ] Skip if >90% similar memory exists
  - [ ] Add `EmbeddingService` dependency injection

- [ ] **2.2.2** Implement recent capture check
  - [ ] Query index for memories captured in last 5 minutes
  - [ ] Skip candidates matching recent content
  - [ ] Add configurable window (`SUBCOG_AUTO_CAPTURE_WINDOW_SECS`)

- [ ] **2.2.3** Add context language detection
  - [ ] Add `contains_context_language()` function
  - [ ] Detect: "because", "constraint", "requirement", "context:", "important:", "note:"
  - [ ] Map to `Namespace::Context`

- [ ] **2.2.4** Optional LLM analysis mode
  - [ ] Add `--llm-analyze` flag
  - [ ] Use LLM to classify ambiguous content
  - [ ] Add configuration `SUBCOG_AUTO_CAPTURE_USE_LLM`

- [ ] **2.2.5** Update tests
  - [ ] Test semantic deduplication logic
  - [ ] Test time-based deduplication
  - [ ] Test context language detection

---

### 2.3 Stop Hook Enhancements

**File:** `src/hooks/stop.rs`
**Reference:** docs/hooks/stop.md lines 40-54, 129-134

#### Tasks

- [ ] **2.3.1** Extend `SessionSummary` struct
  - [ ] Add `namespace_counts: HashMap<String, NamespaceStats>`
  - [ ] Add `tags_used: Vec<(String, usize)>`
  - [ ] Add `query_patterns: Vec<String>`
  - [ ] Add `resources_read: Vec<String>`

- [ ] **2.3.2** Implement namespace breakdown
  - [ ] Track captures per namespace during session
  - [ ] Track recalls per namespace
  - [ ] Format as table in output

- [ ] **2.3.3** Implement tags analysis
  - [ ] Collect tags from captures
  - [ ] Rank by frequency
  - [ ] Include top 10 in summary

- [ ] **2.3.4** Implement query pattern tracking
  - [ ] Log search queries during session
  - [ ] Identify common patterns
  - [ ] Suggest related memories

- [ ] **2.3.5** Implement resources tracking
  - [ ] Track MCP resources read
  - [ ] Count unique resources
  - [ ] Include in summary

- [ ] **2.3.6** Update tests
  - [ ] Verify namespace breakdown in output
  - [ ] Verify tags analysis
  - [ ] Verify query patterns

---

### 2.4 Org Scope Prompts

**File:** `src/storage/prompt/mod.rs:92`
**Reference:** docs/prompts/storage.md

#### Tasks

- [ ] **2.4.1** Design org-scope storage
  - [ ] Define org identifier resolution
  - [ ] Plan storage path (`~/.config/subcog/orgs/{org}/prompts/`)
  - [ ] Handle org membership

- [ ] **2.4.2** Implement org-scope in `PromptStorageBackend`
  - [ ] Add `DomainScope::Org` handling in each backend
  - [ ] Filesystem: org directory structure
  - [ ] SQLite: org column in prompts table
  - [ ] Git Notes: org namespace in refs

- [ ] **2.4.3** Add org configuration
  - [ ] `SUBCOG_ORG` environment variable
  - [ ] Config file `org` field
  - [ ] Auto-detect from git remote

- [ ] **2.4.4** Update MCP resources
  - [ ] Enable `subcog://org/_prompts`
  - [ ] Enable `subcog://org/_prompts/{name}`

- [ ] **2.4.5** Write tests
  - [ ] Test org-scope CRUD operations
  - [ ] Test domain cascade (project → user → org)

---

## Phase 3: MISSING Features (P3)

### 3.1 `namespaces` CLI Command

**Documentation:** docs/cli/namespaces.md
**Target:** `src/cli/namespaces.rs`

#### Tasks

- [ ] **3.1.1** Create `src/cli/namespaces.rs`
  - [ ] Define `NamespacesArgs` struct
  - [ ] Implement `run()` function
  - [ ] List all namespaces with descriptions

- [ ] **3.1.2** Add to CLI module
  - [ ] Register in `src/cli/mod.rs`
  - [ ] Add `Namespaces` variant to `Commands` enum

- [ ] **3.1.3** Implement output formats
  - [ ] Table format (default)
  - [ ] JSON format (`--json`)
  - [ ] Include signal words and descriptions

- [ ] **3.1.4** Write tests
  - [ ] Test table output
  - [ ] Test JSON output

---

### 3.2 `subcog://namespaces` MCP Resource

**Documentation:** docs/mcp/resources.md lines 207-208
**File:** `src/mcp/resources.rs`

#### Tasks

- [ ] **3.2.1** Add to `list_resources()`
  - [ ] Add `subcog://namespaces` ResourceDefinition
  - [ ] Add `subcog://namespaces/{ns}` template

- [ ] **3.2.2** Implement `get_namespaces_resource()`
  - [ ] Handle `subcog://namespaces` - list all
  - [ ] Handle `subcog://namespaces/{ns}` - get memories in namespace

- [ ] **3.2.3** Add routing in `get_resource()`
  - [ ] Add `"namespaces"` case in match statement
  - [ ] Route to `get_namespaces_resource()`

- [ ] **3.2.4** Write tests
  - [ ] Test list namespaces
  - [ ] Test get namespace memories

---

### 3.3 `subcog://_prompts` Aggregate Resource

**Documentation:** docs/mcp/resources.md line 171
**File:** `src/mcp/resources.rs`

#### Tasks

- [ ] **3.3.1** Add to `list_resources()`
  - [ ] Add `subcog://_prompts` ResourceDefinition

- [ ] **3.3.2** Implement aggregate prompts handler
  - [ ] Query all domains (project, user, org)
  - [ ] Combine and deduplicate by name
  - [ ] Return merged list

- [ ] **3.3.3** Update resource routing
  - [ ] Handle `_prompts` in cross-domain route
  - [ ] Distinguish from namespace patterns

- [ ] **3.3.4** Write tests
  - [ ] Test aggregate listing
  - [ ] Test deduplication

---

### 3.4 `generate_tutorial` MCP Prompt

**Documentation:** docs/mcp/prompts.md
**File:** `src/mcp/prompts.rs`

#### Tasks

- [ ] **3.4.1** Define prompt template
  - [ ] Design tutorial generation structure
  - [ ] Define input parameters (topic, level, format)
  - [ ] Create comprehensive prompt content

- [ ] **3.4.2** Add to prompts list
  - [ ] Register in `list_prompts()`
  - [ ] Implement `get_prompt()` handler

- [ ] **3.4.3** Write tests
  - [ ] Test prompt retrieval
  - [ ] Test parameter substitution

---

### 3.5 Shell Completions

**Documentation:** docs/cli/README.md lines 76-87
**File:** `src/main.rs` or `src/cli/mod.rs`

#### Tasks

- [ ] **3.5.1** Add clap completions feature
  - [ ] Add `clap_complete` dependency
  - [ ] Enable `derive` feature for completions

- [ ] **3.5.2** Implement `completions` subcommand
  - [ ] Add `Completions` command variant
  - [ ] Accept shell type argument (bash, zsh, fish, powershell)
  - [ ] Generate completion script to stdout

- [ ] **3.5.3** Update documentation
  - [ ] Add installation instructions per shell
  - [ ] Include in docs/cli/README.md

- [ ] **3.5.4** Write tests
  - [ ] Test script generation for each shell

---

### 3.6 `prompt import/share` Subcommands

**Documentation:** docs/prompts/mcp.md
**File:** `src/cli/prompt.rs`

#### Tasks

- [ ] **3.6.1** Implement `import` subcommand
  - [ ] Accept file path or URL
  - [ ] Parse prompt format (YAML, JSON, MD)
  - [ ] Validate and save to target domain

- [ ] **3.6.2** Implement `share` subcommand
  - [ ] Export prompt to file
  - [ ] Support output formats (YAML, JSON, MD)
  - [ ] Include metadata and variables

- [ ] **3.6.3** Add to CLI module
  - [ ] Add `Import` and `Share` subcommands
  - [ ] Wire up to prompt service

- [ ] **3.6.4** Write tests
  - [ ] Test import from file
  - [ ] Test export to file
  - [ ] Test round-trip (export → import)

---

## Verification Checklist

### Per-Task Verification

- [ ] Code compiles without warnings (`cargo build`)
- [ ] All tests pass (`cargo test`)
- [ ] Clippy clean (`cargo clippy --all-targets`)
- [ ] Format correct (`cargo fmt -- --check`)
- [ ] Documentation updated

### Phase Completion Gates

#### Phase 1 Complete When:
- [ ] Redis Vector backend passes integration tests
- [ ] HTTP transport serves MCP requests
- [ ] `make ci` passes

#### Phase 2 Complete When:
- [ ] usearch benchmarks show O(log n) performance
- [ ] PreCompact hook semantic dedup verified
- [ ] Stop hook summary shows namespace breakdown
- [ ] Org prompts CRUD functional
- [ ] `make ci` passes

#### Phase 3 Complete When:
- [ ] `subcog namespaces` outputs table
- [ ] All MCP resources listed in docs are functional
- [ ] Shell completions install correctly
- [ ] `make ci` passes

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
