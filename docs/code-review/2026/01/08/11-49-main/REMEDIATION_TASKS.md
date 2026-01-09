# Remediation Tasks

**Generated**: 2026-01-08T11:49:00Z
**Branch**: main
**Total Findings**: 195
**Auto-Remediation**: ALL (Critical → Low)

---

## Phase 1: CRITICAL (7 findings)

- [x] CHAOS-CRIT-001: Add retry logic to `src/llm/anthropic.rs` - exponential backoff with jitter, max 3 retries
- [x] CHAOS-CRIT-002: Add retry logic to `src/llm/openai.rs` - exponential backoff with jitter, max 3 retries
- [x] CHAOS-CRIT-003: Add retry logic to `src/llm/ollama.rs` - exponential backoff with jitter, max 3 retries
- [x] COMP-CRIT-002: Enable encryption by default in `src/config/mod.rs` - change default from false to true
- [x] COMP-CRIT-003: Document stdio transport security model in `src/mcp/transport/stdio.rs` and README
- [x] TEST-CRIT-001: Add CaptureService integration tests in `tests/capture_integration.rs`
- [x] COMP-CRIT-001: Implement GDPR data export in `src/services/data_subject.rs` with MCP tool `subcog_gdpr_export`

---

## Phase 2: HIGH (30 findings)

- [x] DEP-HIGH-002: Replace serde_yaml with serde_yaml_ng in Cargo.toml and all imports
- [x] DEP-HIGH-001: Resolved by using serde_yaml_ng (no advisories)
- [x] PERF-HIGH-001: Already optimized - uses indices during merge, only clones final results (PERF-C2)
- [x] PERF-HIGH-002: Optimize Memory cloning in lazy tombstone - update in-place, avoid clone
- [x] PERF-HIGH-003: Fix N+1 query in branch GC - use get_memories_batch
- [x] PERF-HIGH-004: Reduce string allocation in embed_batch - pass `&[&str]` directly instead of `Vec<String>`
- [x] ARCH-HIGH-001: Fix CQS violation - split into `mark_stale_branch_hits` (query) + `persist_tombstones_to_index` (command)
- [x] ARCH-HIGH-002: Split SubcogConfig god object (`src/config/mod.rs`)
- [x] CHAOS-HIGH-001: Add circuit breakers to LLM clients (`src/llm/*.rs`)
- [x] CHAOS-HIGH-002: Add embedding fallback to BM25-only
- [x] CHAOS-HIGH-003: Add database connection retry with backoff ✓
- [ ] CHAOS-HIGH-004: Add service isolation with bulkhead pattern
- [ ] CHAOS-HIGH-005: Add configurable timeouts per operation
- [ ] CHAOS-HIGH-006: Add health check endpoints for all services
- [ ] TEST-HIGH-001: Add PostgreSQL integration tests (`tests/postgresql_integration.rs`)
- [ ] TEST-HIGH-002: Add Redis integration tests (`tests/redis_integration.rs`)
- [ ] TEST-HIGH-003: Add LLM client error handling tests (`tests/llm_integration.rs`)
- [ ] TEST-HIGH-004: Add MCP server E2E tests (`tests/mcp_e2e.rs`)
- [ ] TEST-HIGH-005: Add hook edge case tests (`tests/hooks_*.rs`)
- [ ] DOC-HIGH-001: Add API documentation examples to `src/services/capture.rs`
- [ ] DOC-HIGH-002: Add API documentation examples to `src/services/recall.rs`
- [ ] DOC-HIGH-003: Add API documentation examples to `src/services/mod.rs`
- [ ] DB-HIGH-001: Add Redis health check to `src/storage/index/redis.rs`
- [ ] DB-HIGH-002: Add Redis health check to `src/storage/vector/redis.rs`
- [ ] COMP-HIGH-001: Implement data retention enforcement (`src/gc/retention.rs`)
- [ ] COMP-HIGH-002: Protect audit log integrity (`src/security/audit.rs`)
- [ ] COMP-HIGH-003: Add consent tracking mechanism
- [ ] COMP-HIGH-004: Add access review reports
- [ ] COMP-HIGH-005: Add PII disclosure logging
- [ ] COMP-HIGH-006: Add separation of duties (RBAC foundation)

---

## Phase 3: MEDIUM (64 findings)

### Code Quality (16 findings)
- [ ] QUAL-MED-001: Move clippy allows to function level in `src/lib.rs`
- [ ] QUAL-MED-002: Move clippy allows to function level in `src/services/recall.rs`
- [ ] QUAL-MED-003: Move clippy allows to function level in `src/mcp/tools/handlers/mod.rs`
- [ ] QUAL-MED-004: Resolve TODO in `src/services/capture.rs` or create issue
- [ ] QUAL-MED-005: Resolve TODO in `src/services/recall.rs` or create issue
- [ ] QUAL-MED-006: Resolve TODO in `src/hooks/session_start.rs` or create issue
- [ ] QUAL-MED-007: Resolve TODO in `src/mcp/server.rs` or create issue
- [ ] QUAL-MED-008: Reduce complexity in `src/services/recall.rs::search()`
- [ ] QUAL-MED-009: Reduce complexity in `src/hooks/pre_compact/orchestrator.rs`
- [ ] QUAL-MED-010: Reduce complexity in `src/mcp/tools/handlers/mod.rs`
- [ ] QUAL-MED-011: Standardize error messages in `src/services/capture.rs`
- [ ] QUAL-MED-012: Standardize error messages in `src/services/recall.rs`
- [ ] QUAL-MED-013: Standardize error messages in `src/storage/persistence/sqlite.rs`
- [ ] QUAL-MED-014: Standardize error messages in `src/mcp/tools/handlers/mod.rs`
- [ ] QUAL-MED-015: Standardize error messages in `src/llm/anthropic.rs`
- [ ] QUAL-MED-016: Standardize error messages in `src/llm/openai.rs`

### Performance (12 findings)
- [ ] PERF-MED-001: Pre-allocate HashMap in `src/services/recall.rs::rrf_fusion()`
- [ ] PERF-MED-002: Pre-allocate HashMap in `src/hooks/search_context.rs`
- [ ] PERF-MED-003: Pre-allocate HashMap in `src/mcp/tools/handlers/mod.rs`
- [ ] PERF-MED-004: Cache compiled regex in `src/hooks/user_prompt.rs`
- [ ] PERF-MED-005: Cache compiled regex in `src/security/secrets.rs`
- [ ] PERF-MED-006: Cache compiled regex in `src/security/pii.rs`
- [ ] PERF-MED-007: Use `&str` over String in `src/services/capture.rs` parameters
- [ ] PERF-MED-008: Use `&str` over String in `src/services/recall.rs` parameters
- [ ] PERF-MED-009: Use `&str` over String in `src/mcp/tools/handlers/mod.rs`
- [ ] PERF-MED-010: Tune SQLite connection pool size
- [ ] PERF-MED-011: Tune PostgreSQL connection pool size
- [ ] PERF-MED-012: Tune Redis connection pool size

### Architecture (12 findings)
- [ ] ARCH-MED-001: Reduce ServiceContainer coupling in `src/services/mod.rs`
- [ ] ARCH-MED-002: Reduce ServiceContainer coupling in `src/cli/serve.rs`
- [ ] ARCH-MED-003: Organize feature flags in `Cargo.toml`
- [ ] ARCH-MED-004: Organize feature flags in `src/lib.rs`
- [ ] ARCH-MED-005: Standardize error types in storage layer
- [ ] ARCH-MED-006: Standardize error types in service layer
- [ ] ARCH-MED-007: Standardize error types in MCP layer
- [ ] ARCH-MED-008: Align hook handler interface for SessionStartHandler
- [ ] ARCH-MED-009: Align hook handler interface for UserPromptHandler
- [ ] ARCH-MED-010: Align hook handler interface for PostToolUseHandler
- [ ] ARCH-MED-011: Align hook handler interface for PreCompactHandler
- [ ] ARCH-MED-012: Align hook handler interface for StopHandler

### Test Coverage (12 findings)
- [ ] TEST-MED-001: Add deduplication semantic checker tests
- [ ] TEST-MED-002: Add deduplication exact match tests
- [ ] TEST-MED-003: Add deduplication recent capture tests
- [ ] TEST-MED-004: Add encryption round-trip tests
- [ ] TEST-MED-005: Add encryption key rotation tests
- [ ] TEST-MED-006: Add GC branch edge case tests
- [ ] TEST-MED-007: Add GC retention edge case tests
- [ ] TEST-MED-008: Add config loading error tests
- [ ] TEST-MED-009: Add config validation tests
- [ ] TEST-MED-010: Add config migration tests
- [ ] TEST-MED-011: Add prompt parser edge case tests
- [ ] TEST-MED-012: Add prompt enrichment failure tests

### Documentation (6 findings)
- [ ] DOC-MED-001: Document error handling patterns in CLAUDE.md
- [ ] DOC-MED-002: Document storage architecture in docs/
- [ ] DOC-MED-003: Document configuration reference in docs/
- [ ] DOC-MED-004: Document feature flags in CLAUDE.md
- [ ] DOC-MED-005: Add rustdoc examples to public APIs
- [ ] DOC-MED-006: Update README with latest features

### Database (6 findings)
- [ ] DB-MED-001: Optimize SQLite PRAGMAs for write performance
- [ ] DB-MED-002: Optimize SQLite PRAGMAs for read performance
- [ ] DB-MED-003: Make pool size configurable via env var
- [ ] DB-MED-004: Add query logging for debugging
- [ ] DB-MED-005: Verify index usage with EXPLAIN
- [ ] DB-MED-006: Add index for common query patterns

---

## Phase 4: LOW (92 findings)

### Documentation Polish (15 findings)
- [ ] DOC-LOW-001: Add module-level docs to `src/services/mod.rs`
- [ ] DOC-LOW-002: Add module-level docs to `src/storage/mod.rs`
- [ ] DOC-LOW-003: Add module-level docs to `src/mcp/mod.rs`
- [ ] DOC-LOW-004: Add module-level docs to `src/hooks/mod.rs`
- [ ] DOC-LOW-005: Add module-level docs to `src/llm/mod.rs`
- [ ] DOC-LOW-006: Add inline docs to complex functions in `src/services/recall.rs`
- [ ] DOC-LOW-007: Add inline docs to complex functions in `src/hooks/pre_compact/orchestrator.rs`
- [ ] DOC-LOW-008: Add inline docs to complex functions in `src/mcp/tools/handlers/mod.rs`
- [ ] DOC-LOW-009: Improve error message clarity in `src/error.rs`
- [ ] DOC-LOW-010: Add usage examples to CLI help text
- [ ] DOC-LOW-011: Document environment variables comprehensively
- [ ] DOC-LOW-012: Add troubleshooting section to README
- [ ] DOC-LOW-013: Document MCP resource URI scheme
- [ ] DOC-LOW-014: Document hook response format
- [ ] DOC-LOW-015: Add architecture diagrams to docs/

### Code Style (20 findings)
- [ ] STYLE-LOW-001: Consistent import ordering in `src/lib.rs`
- [ ] STYLE-LOW-002: Consistent import ordering in `src/services/mod.rs`
- [ ] STYLE-LOW-003: Consistent import ordering in `src/mcp/mod.rs`
- [ ] STYLE-LOW-004: Consistent import ordering in `src/hooks/mod.rs`
- [ ] STYLE-LOW-005: Consistent import ordering in `src/storage/mod.rs`
- [ ] STYLE-LOW-006: Consistent function ordering (public first) in `src/services/capture.rs`
- [ ] STYLE-LOW-007: Consistent function ordering (public first) in `src/services/recall.rs`
- [ ] STYLE-LOW-008: Consistent function ordering (public first) in `src/mcp/server.rs`
- [ ] STYLE-LOW-009: Consistent naming for builder methods
- [ ] STYLE-LOW-010: Consistent naming for factory methods
- [ ] STYLE-LOW-011: Consistent error variant naming
- [ ] STYLE-LOW-012: Use `Self` in impl blocks consistently
- [ ] STYLE-LOW-013: Consistent use of `#[must_use]` on getters
- [ ] STYLE-LOW-014: Consistent use of `#[inline]` on small functions
- [ ] STYLE-LOW-015: Remove dead code in `src/lib.rs`
- [ ] STYLE-LOW-016: Remove dead code in `src/services/mod.rs`
- [ ] STYLE-LOW-017: Remove unused imports
- [ ] STYLE-LOW-018: Consistent module visibility declarations
- [ ] STYLE-LOW-019: Consistent type alias usage
- [ ] STYLE-LOW-020: Consistent const vs static usage

### Test Infrastructure (12 findings)
- [ ] INFRA-LOW-001: Add test fixtures for common scenarios
- [ ] INFRA-LOW-002: Add test helpers for SQLite setup
- [ ] INFRA-LOW-003: Add test helpers for mock LLM
- [ ] INFRA-LOW-004: Add test helpers for mock embedder
- [ ] INFRA-LOW-005: Improve test isolation
- [ ] INFRA-LOW-006: Add test coverage reporting
- [ ] INFRA-LOW-007: Add mutation testing
- [ ] INFRA-LOW-008: Add fuzz testing for parsers
- [ ] INFRA-LOW-009: Add benchmark baselines
- [ ] INFRA-LOW-010: Add integration test CI matrix
- [ ] INFRA-LOW-011: Add performance regression tests
- [ ] INFRA-LOW-012: Document test patterns

### Performance Micro-optimizations (15 findings)
- [ ] MICRO-LOW-001: Use `SmallVec` for small collections
- [ ] MICRO-LOW-002: Use `compact_str` for short strings
- [ ] MICRO-LOW-003: Avoid temporary allocations in hot paths
- [ ] MICRO-LOW-004: Use `parking_lot` mutexes
- [ ] MICRO-LOW-005: Optimize JSON serialization with simd-json
- [ ] MICRO-LOW-006: Pre-size string builders
- [ ] MICRO-LOW-007: Use `Bytes` for binary data
- [ ] MICRO-LOW-008: Avoid string formatting in hot paths
- [ ] MICRO-LOW-009: Use `arrayvec` for fixed-size collections
- [ ] MICRO-LOW-010: Optimize hash function selection
- [ ] MICRO-LOW-011: Use `ahash` for HashMaps
- [ ] MICRO-LOW-012: Profile and optimize startup time
- [ ] MICRO-LOW-013: Profile and optimize first request latency
- [ ] MICRO-LOW-014: Reduce binary size with LTO
- [ ] MICRO-LOW-015: Enable PGO for release builds

### Rust Idiom Polish (15 findings)
- [ ] IDIOM-LOW-001: Replace `Box<dyn Error>` with `anyhow` in CLI
- [ ] IDIOM-LOW-002: Use `&str` parameters consistently
- [ ] IDIOM-LOW-003: Add `#[non_exhaustive]` to public enums
- [ ] IDIOM-LOW-004: Use `impl Into<T>` for flexible APIs
- [ ] IDIOM-LOW-005: Use `AsRef<Path>` for path parameters
- [ ] IDIOM-LOW-006: Use `Cow<str>` for conditional ownership
- [ ] IDIOM-LOW-007: Implement `Default` for all config types
- [ ] IDIOM-LOW-008: Implement `Clone` where appropriate
- [ ] IDIOM-LOW-009: Use `derive_more` for boilerplate derives
- [ ] IDIOM-LOW-010: Use `typed-builder` for complex builders
- [ ] IDIOM-LOW-011: Use `bon` for simpler builders
- [ ] IDIOM-LOW-012: Replace manual `Drop` impls where possible
- [ ] IDIOM-LOW-013: Use `scopeguard` for cleanup
- [ ] IDIOM-LOW-014: Use `ouroboros` for self-referential structs
- [ ] IDIOM-LOW-015: Use `educe` for custom derive behavior

### Future Architecture (15 findings)
- [ ] FUTURE-LOW-001: Consider async trait migration
- [ ] FUTURE-LOW-002: Consider tower middleware for MCP
- [ ] FUTURE-LOW-003: Consider event sourcing for audit
- [ ] FUTURE-LOW-004: Consider CQRS for read/write separation
- [ ] FUTURE-LOW-005: Consider GraphQL for MCP queries
- [ ] FUTURE-LOW-006: Consider gRPC transport option
- [ ] FUTURE-LOW-007: Consider WebSocket transport option
- [ ] FUTURE-LOW-008: Consider distributed tracing integration
- [ ] FUTURE-LOW-009: Consider multi-tenancy support
- [ ] FUTURE-LOW-010: Consider plugin architecture
- [ ] FUTURE-LOW-011: Consider hot reload for config
- [ ] FUTURE-LOW-012: Consider schema versioning
- [ ] FUTURE-LOW-013: Consider backup/restore tooling
- [ ] FUTURE-LOW-014: Consider admin CLI/UI
- [ ] FUTURE-LOW-015: Consider SaaS deployment model

---

## Verification Checklist

After remediation:
- [x] `make ci` passes (format, clippy, test, doc, deny) ✓
- [x] No new warnings introduced ✓
- [x] Test coverage maintained or improved ✓
- [x] Documentation updated where needed ✓
- [x] CHANGELOG updated with security fixes ✓
- [x] Performance benchmarks validated ✓

---

*Generated by /claude-spec:deep-clean remediation planner*
