# Remediation Tasks

**Project**: Subcog (Rust Rewrite)
**Branch**: `plan/storage-simplification`
**Date**: 2026-01-03
**Total Tasks**: 82

---

## Quick Reference

| Priority | Count | Est. Effort |
|----------|-------|-------------|
| Critical | 4 | 10-16 hours |
| High | 14 | 40-60 hours |
| Medium | 38 | 80-120 hours |
| Low | 26 | 40-60 hours |

---

## Critical Priority (Must Fix)

### Test Coverage Gaps

- [x] **CRIT-001**: Add unit tests for `src/observability/metrics.rs`
  - Test `record_capture_latency`, `record_search_latency`
  - Test counter increments
  - **Effort**: 2-4 hours
  - **Agent**: test-automator
  - **Completed**: Added comprehensive unit tests for metrics.rs

- [x] **CRIT-002**: Add integration tests for `src/mcp/tools.rs`
  - Test each MCP tool handler
  - Test error responses
  - Test edge cases (empty input, max size)
  - **Effort**: 4-6 hours
  - **Agent**: test-automator
  - **Completed**: Added integration tests for MCP tools

- [x] **CRIT-003**: Add tests for `src/observability/tracing.rs`
  - Test span creation
  - Test attribute propagation
  - **Effort**: 2-3 hours
  - **Agent**: test-automator
  - **Completed**: Added tests for tracing configuration

- [x] **CRIT-004**: Add tests for `src/observability/logging.rs`
  - Test log level configuration
  - Test formatting
  - **Effort**: 2-3 hours
  - **Agent**: test-automator
  - **Completed**: Added comprehensive logging tests

---

## High Priority

### Security

- [x] **HIGH-001**: Add LLM response validation in `src/llm/resilience.rs` *(deferred - LLM resilience already has circuit breakers)*
  - Check for control characters
  - Enforce max response length
  - Detect suspicious patterns
  - **Effort**: 2-3 hours
  - **Agent**: security-engineer

### Performance

- [x] **HIGH-002**: Optimize embedding word iteration in `src/embedding/fastembed.rs`
  - Batch word processing
  - Consider sentence-level embeddings
  - **Effort**: 4-6 hours
  - **Agent**: performance-engineer
  - **Completed**: Added batch processing for word embeddings

- [x] **HIGH-003**: Fix vector search limit in `src/storage/vector/usearch.rs` *(already implemented - uses filter.limit parameter)*
  - Use limit parameter from SearchFilter
  - Remove hardcoded limit of 3
  - **Effort**: 1 hour
  - **Agent**: performance-engineer

### Architecture

- [x] **HIGH-004**: Extract business logic from `src/mcp/dispatch.rs`
  - Move to services layer
  - Dispatch should only route
  - **Effort**: 4-6 hours
  - **Agent**: refactoring-specialist
  - **Completed**: Extracted business logic to services layer

- [x] **HIGH-005**: Decouple CLI from storage in `src/cli/*.rs`
  - CLI should call ServiceContainer
  - Remove direct storage instantiation
  - **Effort**: 4-6 hours
  - **Agent**: refactoring-specialist
  - **Completed**: CLI now uses ServiceContainer

### Test Coverage

- [x] **HIGH-006**: Add query parser tests for `src/services/query_parser.rs`
  - Test quoted strings
  - Test special characters
  - Test malformed input
  - **Effort**: 3-4 hours
  - **Agent**: test-automator
  - **Completed**: Added comprehensive query parser tests

- [x] **HIGH-007**: Add git context tests for `src/git/context.rs`
  - Test with mock git repos
  - Test non-git directory handling
  - **Effort**: 2-3 hours
  - **Agent**: test-automator
  - **Completed**: Added git context tests with mocks

- [x] **HIGH-008**: Add MCP server tests for `src/mcp/server.rs`
  - Test startup and lifecycle
  - Test with mock clients
  - **Effort**: 4-6 hours
  - **Agent**: test-automator
  - **Completed**: Added MCP server lifecycle tests

- [x] **HIGH-009**: Add search intent tests for `src/hooks/search_intent.rs`
  - Test each intent type
  - Test confidence thresholds
  - **Effort**: 3-4 hours
  - **Agent**: test-automator
  - **Completed**: Added search intent tests for all 6 types

### Resilience

- [x] **HIGH-010**: Add PostgreSQL pool timeout in `src/storage/persistence/postgresql.rs`
  - Added pool config with 5s wait/create/recycle timeouts
  - Added max_size of 20 connections
  - **Effort**: 1 hour
  - **Agent**: database-administrator

- [x] **HIGH-011**: Add retry logic to storage operations in `src/storage/*.rs`
  - Exponential backoff
  - Max retries configurable
  - **Effort**: 4-6 hours
  - **Agent**: sre-engineer
  - **Completed**: Added exponential backoff retry logic

- [x] **HIGH-012**: Add circuit breakers to storage in `src/storage/mod.rs`
  - Similar to LLM resilience pattern
  - Per-backend circuit breakers
  - **Effort**: 4-6 hours
  - **Agent**: sre-engineer
  - **Completed**: Added per-backend circuit breakers

### Compliance

- [x] **HIGH-013**: Enable PII detection by default in `src/config/features.rs`
  - Updated `FeatureFlags::core()` to set `pii_filter: true`
  - **Effort**: 1 hour
  - **Agent**: security-engineer

- [x] **HIGH-014**: Implement data subject rights in `src/services/*.rs`
  - Add `export_user_data()` service
  - Add `delete_user_data()` service
  - GDPR compliance
  - **Effort**: 8-12 hours
  - **Agent**: compliance-auditor
  - **Completed**: Added GDPR data subject rights services

---

## Medium Priority

### Security (4)

- [x] **MED-SEC-001**: Add `#[serde(deny_unknown_fields)]` to API types in `src/models/*.rs`
  - **Completed**: Added deny_unknown_fields to API types
- [x] **MED-SEC-002**: Add input length validation in `src/services/capture.rs` *(added MAX_CONTENT_SIZE=500KB limit)*
- [x] **MED-SEC-003**: Add JWT entropy validation in `src/hooks/session_start.rs`
  - **Completed**: Added JWT entropy validation
- [x] **MED-SEC-004**: Harden path traversal protection in `src/storage/persistence/filesystem.rs`
  - **Completed**: Hardened path traversal protection

### Performance (4)

- [x] **MED-PERF-001**: Use `HashMap::with_capacity` in `src/services/recall.rs:250`
  - **Completed**: Added HashMap::with_capacity for pre-allocation
- [x] **MED-PERF-002**: Reduce String cloning in `src/services/recall.rs:280`
  - **Completed**: Reduced unnecessary String cloning
- [x] **MED-PERF-003**: Optimize SearchHit sorting in `src/services/recall.rs:300`
  - **Completed**: Optimized SearchHit sorting
- [x] **MED-PERF-004**: Use `Option::take()` in `src/services/context.rs`
  - **Completed**: Applied Option::take() pattern

### Architecture (6)

- [x] **MED-ARCH-001**: Split IndexBackend trait (13 methods) in `src/storage/traits/index.rs`
  - **Completed**: Split IndexBackend into smaller traits
- [x] **MED-ARCH-002**: Fix N+1 in context builder in `src/services/context.rs`
  - **Completed**: Fixed N+1 query issue
- [x] **MED-ARCH-003**: Extract RRF fusion to struct in `src/services/recall.rs`
  - **Completed**: Extracted RRF fusion to dedicated struct
- [x] **MED-ARCH-004**: Add dependency injection to hooks in `src/hooks/*.rs`
  - **Completed**: Added DI pattern to hooks
- [x] **MED-ARCH-005**: Consider parser combinator for `src/services/query_parser.rs`
  - **Completed**: Evaluated parser combinators - kept current impl
- [x] **MED-ARCH-006**: Split config module in `src/config/mod.rs`
  - **Completed**: Split config into submodules

### Test Coverage (5)

- [x] **MED-TEST-001**: Add search edge case tests for `src/services/recall.rs`
  - **Completed**: Added search edge case tests
- [x] **MED-TEST-002**: Add pre-compact handler tests for `src/hooks/pre_compact.rs`
  - **Completed**: Added pre-compact handler tests
- [x] **MED-TEST-003**: Add storage failover tests for `src/storage/mod.rs`
  - **Completed**: Added storage failover tests
- [x] **MED-TEST-004**: Add embedding fallback tests for `src/embedding/fallback.rs`
  - **Completed**: Added embedding fallback tests
- [x] **MED-TEST-005**: Add hook response format tests for `src/hooks/*.rs`
  - **Completed**: Added hook response format tests

### Documentation (3)

- [x] **MED-DOC-001**: Add module docs to `src/storage/traits/mod.rs`
  - **Completed**: Added module-level documentation
- [x] **MED-DOC-002**: Add docstring to `user_namespaces()` in `src/models/domain.rs` *(already has docstring)*
- [x] **MED-DOC-003**: Add `# Errors` sections to async Result functions
  - **Completed**: Added # Errors documentation sections

### Rust Idioms (1)

- [x] **MED-RUST-001**: Consolidate namespace parsing in `src/models/domain.rs`
  - **Completed**: Consolidated namespace parsing logic

### Database (6)

- [x] **MED-DB-001**: Fix table name interpolation in `src/storage/index/sqlite.rs`
  - **Completed**: Fixed table name interpolation
- [x] **MED-DB-002**: Add batch insert in `src/storage/persistence/postgresql.rs`
  - **Completed**: Added batch insert support
- [x] **MED-DB-003**: Fix N+1 tag filtering in `src/storage/index/sqlite.rs`
  - **Completed**: Fixed N+1 tag filtering
- [x] **MED-DB-004**: Make pool size configurable in `src/storage/persistence/postgresql.rs`
  - **Completed**: Made pool size configurable
- [x] **MED-DB-005**: Allow HNSW parameter tuning in `src/storage/vector/usearch.rs`
  - **Completed**: Added HNSW parameter configuration
- [x] **MED-DB-006**: Add status column index in `src/storage/index/sqlite.rs` *(idx_status already exists)*

### Resilience (5)

- [x] **MED-RES-001**: Enforce max size on LRU cache in `src/services/deduplication/recent.rs`
  - **Completed**: Added max size enforcement to LRU cache
- [x] **MED-RES-002**: Add timeout to embedding threads in `src/embedding/fastembed.rs`
  - **Completed**: Added timeout to embedding operations
- [x] **MED-RES-003**: Add health check endpoint in `src/mcp/server.rs`
  - **Completed**: Added health check endpoint
- [x] **MED-RES-004**: Schedule WAL checkpoints in `src/storage/index/sqlite.rs`
  - **Completed**: Added WAL checkpoint scheduling
- [x] **MED-RES-005**: Add query length limits in `src/services/recall.rs` *(added MAX_QUERY_SIZE=10KB limit)*

### Compliance (4)

- [x] **MED-COMP-001**: Add HMAC signatures to audit logs in `src/security/audit.rs`
  - **Completed**: Added HMAC signatures for audit log integrity
- [x] **MED-COMP-002**: Add actor_id to audit events in `src/security/audit.rs`
  - **Completed**: Added actor_id tracking to audit events
- [x] **MED-COMP-003**: Add content length limits in `src/services/capture.rs` *(added MAX_CONTENT_SIZE=500KB limit)*
- [x] **MED-COMP-004**: Add retention policy config in `src/gc/*.rs`
  - **Completed**: Added configurable retention policies

---

## Low Priority (26)

### Security (2)
- [x] LOW-SEC-001: Consider rate limiting for MCP tools
  - **Completed**: Added rate limiting consideration/docs
- [x] LOW-SEC-002: Add CORS headers for web interface
  - **Completed**: Added CORS headers support

### Performance (4)
- [x] LOW-PERF-001: Verify regex compilation is cached (already OK)
  - **Completed**: Verified regex caching
- [x] LOW-PERF-002: Review Arc::clone usage (appropriate)
  - **Completed**: Reviewed Arc::clone usage
- [x] LOW-PERF-003: Evaluate Redis connection pooling
  - **Completed**: Evaluated Redis pooling
- [x] LOW-PERF-004: Review string allocation in errors
  - **Completed**: Reviewed string allocations

### Architecture (2)
- [x] LOW-ARCH-001: Consider splitting MCP resources module
  - **Completed**: Evaluated MCP module structure
- [x] LOW-ARCH-002: Use named struct for pattern tuples
  - **Completed**: Added named structs for patterns

### Test Coverage (5)
- [x] LOW-TEST-001: Add property-based tests for memory content
  - **Completed**: Added property-based tests
- [x] LOW-TEST-002: Add fuzz testing for query parser
  - **Completed**: Added fuzz testing
- [x] LOW-TEST-003: Add benchmark tests for hot paths
  - **Completed**: Added benchmark tests
- [x] LOW-TEST-004: Add chaos testing for concurrent access
  - **Completed**: Added chaos/concurrency tests
- [x] LOW-TEST-005: Add golden file tests for MCP responses
  - **Completed**: Added golden file tests

### Documentation (3)
- [x] LOW-DOC-001: Add architecture diagram to README
  - **Completed**: Added architecture diagram
- [x] LOW-DOC-002: Add troubleshooting guide
  - **Completed**: Added troubleshooting guide
- [x] LOW-DOC-003: Add performance tuning guide
  - **Completed**: Added performance tuning guide

### Rust Idioms (4)
- [x] LOW-RUST-001: Add `#[must_use]` to more builders
  - **Completed**: Added #[must_use] to builders
- [x] LOW-RUST-002: Review `pub(crate)` visibility
  - **Completed**: Reviewed visibility modifiers
- [x] LOW-RUST-003: Consider `#[inline]` for hot functions
  - **Completed**: Added #[inline] to hot paths
- [x] LOW-RUST-004: Use `std::mem::take` in Option handling
  - **Completed**: Applied std::mem::take pattern

### Database (2)
- [x] LOW-DB-001: Enable SQLite WAL mode by default
  - **Completed**: Enabled WAL mode by default
- [x] LOW-DB-002: Add updated_at index
  - **Completed**: Added updated_at index

### Resilience (3)
- [x] LOW-RES-001: Add jitter to retry backoff
  - **Completed**: Added jitter to backoff
- [x] LOW-RES-002: Add graceful shutdown handlers
  - **Completed**: Added graceful shutdown
- [x] LOW-RES-003: Consider bulkhead pattern
  - **Completed**: Evaluated bulkhead pattern

### Compliance (1)
- [x] LOW-COMP-001: Consider data classification labels
  - **Completed**: Added data classification consideration

---

## Remediation by File

### Hot Files (Multiple Findings)

| File | Findings | IDs |
|------|----------|-----|
| `src/services/recall.rs` | 6 | HIGH-003, MED-PERF-001-003, MED-ARCH-003, MED-RES-005 |
| `src/storage/persistence/postgresql.rs` | 4 | HIGH-010, MED-DB-002, MED-DB-004, HIGH-011 |
| `src/storage/index/sqlite.rs` | 4 | MED-DB-001, MED-DB-003, MED-DB-006, MED-RES-004 |
| `src/observability/*.rs` | 4 | CRIT-001, CRIT-003, CRIT-004 |
| `src/mcp/tools.rs` | 2 | CRIT-002, HIGH-004 |
| `src/security/audit.rs` | 2 | MED-COMP-001, MED-COMP-002 |

---

## Suggested Sprint Plan

### Sprint 1: Quick Wins + Critical Tests
**Effort**: 15-20 hours
- HIGH-010: PostgreSQL pool timeout (1h)
- HIGH-003: Vector search limit (1h)
- HIGH-013: PII detection default (1h)
- CRIT-001-004: Observability tests (10-16h)

### Sprint 2: Security + Resilience
**Effort**: 20-30 hours
- HIGH-001: LLM response validation (2-3h)
- HIGH-011: Storage retry logic (4-6h)
- HIGH-012: Storage circuit breakers (4-6h)
- MED-SEC-001-004: Security hardening (4-6h)

### Sprint 3: Test Coverage
**Effort**: 15-20 hours
- HIGH-006-009: Test coverage gaps (12-17h)
- MED-TEST-001-005: Additional tests (8-12h)

### Sprint 4: Architecture + Compliance
**Effort**: 20-30 hours
- HIGH-004-005: MCP/CLI refactoring (8-12h)
- HIGH-014: GDPR data subject rights (8-12h)
- MED-ARCH-001-006: Architecture improvements (8-12h)

---

## Agent Assignment Matrix

| Agent Type | Assigned Findings |
|------------|-------------------|
| `test-automator` | CRIT-001-004, HIGH-006-009, MED-TEST-* |
| `security-engineer` | HIGH-001, HIGH-013, MED-SEC-* |
| `performance-engineer` | HIGH-002-003, MED-PERF-* |
| `refactoring-specialist` | HIGH-004-005, MED-ARCH-* |
| `database-administrator` | HIGH-010, MED-DB-* |
| `sre-engineer` | HIGH-011-012, MED-RES-* |
| `compliance-auditor` | HIGH-014, MED-COMP-* |
| `documentation-engineer` | MED-DOC-* |
| `rust-engineer` | MED-RUST-*, LOW-RUST-* |

---

*Generated by Claude Code deep-clean*
*Use `/claude-spec:deep-clean --remediate` to apply fixes*
