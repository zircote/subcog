# Remediation Tasks

**Project**: Subcog Pre-Compact Deduplication
**Generated**: 2026-01-01
**Mode**: MAXALL (All severities)

---

## Progress Overview

| Severity | Total | Fixed | Remaining |
|----------|-------|-------|-----------|
| Critical | 18 | 0 | 18 |
| High | 47 | 0 | 47 |
| Medium | 68 | 0 | 68 |
| Low | 36 | 0 | 36 |
| **Total** | **169** | **0** | **169** |

---

## Critical Findings (Fix Immediately)

### Security & Database

- [ ] **DB-C1**: Fix SQL injection via table name interpolation
  - File: `src/storage/index/postgresql.rs:156`
  - Action: Use whitelisted table names or parameterized identifiers
  - Agent: `security-engineer`

- [ ] **DB-C2**: Configure PostgreSQL connection pool
  - File: `src/storage/index/postgresql.rs:45-60`
  - Action: Add `max_size: 20, timeouts: { wait: 5s, create: 10s }`
  - Agent: `database-administrator`

### Performance

- [ ] **PERF-C1**: Fix N+1 query pattern in RecallService
  - File: `src/services/recall.rs:89-145`
  - Action: Batch queries with IN clause
  - Agent: `performance-engineer`

- [ ] **PERF-C2**: Fix blocking async in PostgreSQL pool.get()
  - File: `src/storage/index/postgresql.rs:280-322`
  - Action: Use `pool.get().await` with timeout
  - Agent: `performance-engineer`

- [ ] **PERF-C3**: Cache FastEmbed model instead of loading per call
  - File: `src/embedding/fastembed.rs:40-55`
  - Action: Lazy-static or OnceCell for model
  - Agent: `performance-engineer`

### Resilience

- [ ] **CHAOS-C1**: Add timeout to git fetch/push operations
  - File: `src/git/remote.rs:95-134`
  - Action: Wrap with 30-second timeout
  - Agent: `chaos-engineer`

- [ ] **CHAOS-C2**: Add rate limiting to MCP stdio loop
  - File: `src/mcp/server.rs:116-137`
  - Action: Implement 1000 req/min limit
  - Agent: `chaos-engineer`

- [ ] **CHAOS-C3**: Handle SQLite mutex poisoning
  - File: `src/storage/index/sqlite.rs:82-85`
  - Action: Add timeout and poison recovery
  - Agent: `chaos-engineer`

### Compliance

- [ ] **COMP-C1**: Implement encryption at rest
  - Files: Storage backends
  - Action: Add AES-256 encryption layer
  - Agent: `security-engineer`

- [ ] **COMP-C2**: Implement GDPR deletion capability
  - Files: Storage traits
  - Action: Add `delete()` method to all backends
  - Agent: `compliance-auditor`

- [ ] **COMP-C3**: Enforce TLS for PostgreSQL connections
  - File: `src/storage/index/postgresql.rs`
  - Action: Add `sslmode=require` to connection
  - Agent: `security-engineer`

- [ ] **COMP-C4**: Implement RBAC
  - Files: MCP server, services
  - Action: Add role-based access control
  - Agent: `security-engineer`

- [ ] **COMP-C5**: Complete audit logging
  - Files: All write operations
  - Action: Log all mutations with user context
  - Agent: `compliance-auditor`

- [ ] **COMP-C6**: Implement data classification
  - Files: Models, storage
  - Action: Add sensitivity levels to memories
  - Agent: `compliance-auditor`

- [ ] **COMP-C7**: Add consent tracking
  - Files: Capture service
  - Action: Track consent for data storage
  - Agent: `compliance-auditor`

### Architecture

- [ ] **ARCH-C1**: Decompose mcp/resources.rs (1,969 lines)
  - File: `src/mcp/resources.rs`
  - Action: Extract resource handlers to separate files
  - Agent: `refactoring-specialist`

- [ ] **ARCH-C2**: Decompose mcp/tools.rs (1,698 lines)
  - File: `src/mcp/tools.rs`
  - Action: Use Strategy pattern for tools
  - Agent: `refactoring-specialist`

- [ ] **ARCH-C3**: Decompose search_intent.rs (1,612 lines)
  - File: `src/hooks/search_intent.rs`
  - Action: Extract classifiers to modules
  - Agent: `refactoring-specialist`

---

## High Findings (Fix Within 1 Week)

### Security

- [ ] **SEC-H1**: Add MCP server authentication
  - File: `src/mcp/server.rs:116-137`
  - Agent: `security-engineer`

### Performance

- [ ] **PERF-H1**: Add bounds to Vec growth in resources
  - File: `src/mcp/resources.rs:800-900`
  - Agent: `performance-engineer`

- [ ] **PERF-H2**: Optimize O(n²) pattern matching
  - File: `src/hooks/search_intent.rs:450-520`
  - Agent: `performance-engineer`

- [ ] **PERF-H3**: Add index for consolidation queries
  - File: `src/services/consolidation.rs:200-280`
  - Agent: `database-administrator`

- [ ] **PERF-H4**: Implement incremental index updates
  - File: `src/storage/vector/usearch.rs:180-220`
  - Agent: `performance-engineer`

### Resilience

- [ ] **CHAOS-H1**: Configure PostgreSQL pool exhaustion protection
  - File: `src/storage/index/postgresql.rs`
  - Agent: `chaos-engineer`

- [ ] **CHAOS-H2**: Add timeout to Redis commands
  - File: `src/storage/index/redis.rs:168-191`
  - Agent: `chaos-engineer`

- [ ] **CHAOS-H3**: Cancel spawned thread after timeout
  - File: `src/hooks/search_intent.rs:817-838`
  - Agent: `chaos-engineer`

### Database

- [ ] **DB-H1**: Add indexes on namespace, domain columns
  - File: `src/storage/index/sqlite.rs`
  - Agent: `database-administrator`

- [ ] **DB-H2**: Add transaction support for batch operations
  - File: `src/storage/index/sqlite.rs`
  - Agent: `database-administrator`

- [ ] **DB-H3**: Fix BM25 normalization calculation
  - File: `src/storage/index/sqlite.rs`
  - Agent: `database-administrator`

- [ ] **DB-H4**: Add prepared statement caching
  - File: `src/storage/index/postgresql.rs`
  - Agent: `database-administrator`

- [ ] **DB-H5**: Add TLS configuration for PostgreSQL
  - File: `src/storage/index/postgresql.rs`
  - Agent: `database-administrator`

- [ ] **DB-H6**: Add Redis connection pooling
  - File: `src/storage/index/redis.rs`
  - Agent: `database-administrator`

- [ ] **DB-H7**: Add limit to SCAN operations
  - File: `src/storage/index/redis.rs`
  - Agent: `database-administrator`

- [ ] **DB-H8**: Enable WAL mode for SQLite
  - File: `src/storage/index/sqlite.rs`
  - Agent: `database-administrator`

### Penetration Testing

- [ ] **PEN-H1**: Fix SQL injection in table names
  - File: `src/storage/index/postgresql.rs:156`
  - Agent: `penetration-tester`

- [ ] **PEN-H2**: Fix path traversal vulnerability
  - File: `src/storage/persistence/filesystem.rs:112-130`
  - Agent: `penetration-tester`

- [ ] **PEN-H3**: Prevent YAML billion laughs attack
  - File: `src/git/parser.rs:45-80`
  - Agent: `penetration-tester`

- [ ] **PEN-H4**: Validate file size before processing
  - File: `src/storage/persistence/filesystem.rs:200-220`
  - Agent: `penetration-tester`

- [ ] **PEN-H5**: Fix URL decode injection
  - File: `src/mcp/mod.rs:89`
  - Agent: `penetration-tester`

### Code Quality

- [ ] **CQ-H1**: Extract common current_timestamp() utility
  - Files: `src/hooks/*.rs`
  - Agent: `refactoring-specialist`

- [ ] **CQ-H2**: Extract common extract_json_from_response()
  - Files: `src/llm/*.rs`
  - Agent: `refactoring-specialist`

- [ ] **CQ-H3**: Refactor large match arms
  - File: `src/mcp/tools.rs:1200-1400`
  - Agent: `refactoring-specialist`

- [ ] **CQ-H4**: Reduce nesting depth
  - File: `src/hooks/search_intent.rs:300-400`
  - Agent: `refactoring-specialist`

### Architecture

- [ ] **ARCH-H1**: Extract LLM factory from main.rs
  - File: `src/main.rs:1177 lines`
  - Agent: `refactoring-specialist`

- [ ] **ARCH-H2**: Decompose pre_compact.rs
  - File: `src/hooks/pre_compact.rs:876 lines`
  - Agent: `refactoring-specialist`

- [ ] **ARCH-H3**: Separate CLI logic from prompt.rs
  - File: `src/cli/prompt.rs:654 lines`
  - Agent: `refactoring-specialist`

### Documentation

- [ ] **DOC-H1**: Add docstrings to HookCommand
  - File: `src/cli/hook.rs`
  - Agent: `documentation-engineer`

- [ ] **DOC-H2**: Document SubcogConfig fields
  - File: `src/config/mod.rs`
  - Agent: `documentation-engineer`

- [ ] **DOC-H3**: Add LlmProvider usage examples
  - File: `src/llm/mod.rs`
  - Agent: `documentation-engineer`

- [ ] **DOC-H4**: Add VectorBackend examples
  - File: `src/storage/traits/vector.rs`
  - Agent: `documentation-engineer`

- [ ] **DOC-H5**: Add deduplication to CLAUDE.md
  - File: `CLAUDE.md`
  - Agent: `documentation-engineer`

### Test Coverage

- [ ] **TEST-H1**: Add CLI capture tests
  - File: `src/cli/capture.rs` (0 tests)
  - Agent: `test-automator`

- [ ] **TEST-H2**: Add CLI recall tests
  - File: `src/cli/recall.rs` (0 tests)
  - Agent: `test-automator`

- [ ] **TEST-H3**: Add CLI status tests
  - File: `src/cli/status.rs` (0 tests)
  - Agent: `test-automator`

- [ ] **TEST-H4**: Add CLI sync tests
  - File: `src/cli/sync.rs` (0 tests)
  - Agent: `test-automator`

- [ ] **TEST-H5**: Add CLI config tests
  - File: `src/cli/config.rs` (0 tests)
  - Agent: `test-automator`

- [ ] **TEST-H6**: Add CLI serve tests
  - File: `src/cli/serve.rs` (0 tests)
  - Agent: `test-automator`

- [ ] **TEST-H7**: Add CLI hook tests
  - File: `src/cli/hook.rs` (0 tests)
  - Agent: `test-automator`

- [ ] **TEST-H8**: Add CLI prompt tests
  - File: `src/cli/prompt.rs` (0 tests)
  - Agent: `test-automator`

### Compliance (High)

- [ ] **COMP-H1**: Implement access control policies
- [ ] **COMP-H2**: Add key management system
- [ ] **COMP-H3**: Implement backup/recovery procedures
- [ ] **COMP-H4**: Create incident response plan
- [ ] **COMP-H5**: Document vendor management
- [ ] **COMP-H6**: Implement change control process
- [ ] **COMP-H7**: Add data retention policies
- [ ] **COMP-H8**: Implement session management
- [ ] **COMP-H9**: Add input validation framework
- [ ] **COMP-H10**: Create security awareness docs
- [ ] **COMP-H11**: Implement monitoring/alerting
- [ ] **COMP-H12**: Add vulnerability management

---

## Medium Findings (Fix Within 1 Month)

### Security (4)
- [ ] SEC-M1: API key validation
- [ ] SEC-M2: Path traversal protection
- [ ] SEC-M3: Prompt injection mitigation
- [ ] SEC-M4: Rate limiting implementation

### Performance (2)
- [ ] PERF-M1: Embedding cache
- [ ] PERF-M2: HTTP connection reuse

### Resilience (3)
- [ ] CHAOS-M1: Vector search backpressure
- [ ] CHAOS-M2: Embedding timeout
- [ ] CHAOS-M3: Sync retry with backoff

### Database (12)
- [ ] Query optimization (various)
- [ ] EXPLAIN ANALYZE coverage
- [ ] JOIN efficiency improvements

### Penetration Testing (6)
- [ ] PEN-M1: Redis query sanitization
- [ ] PEN-M2: MCP authentication
- [ ] PEN-M3: Error information disclosure
- [ ] PEN-M4: Tag input sanitization
- [ ] PEN-M5: Regex ReDoS prevention
- [ ] PEN-M6: Memory ID unpredictability

### Code Quality (9)
- [ ] CQ-M1 - CQ-M9: Various quality improvements

### Architecture (2)
- [ ] ARCH-M1: Refactor recall.rs
- [ ] ARCH-M2: Separate schema from sqlite.rs

### Compliance (18)
- [ ] Various medium-priority compliance items

### Documentation (Various)
- [ ] Error type documentation
- [ ] MCP resource examples
- [ ] Hook lifecycle diagrams
- [ ] Configuration examples

---

## Low Findings (Fix When Convenient)

### Security (4)
- [ ] SEC-L1 - SEC-L4: Minor security improvements

### Performance (1)
- [ ] PERF-L1: Reduce unnecessary clones

### Resilience (3)
- [ ] CHAOS-L1 - CHAOS-L3: Minor resilience improvements

### Database (6)
- [ ] Naming conventions
- [ ] Comment quality
- [ ] Schema documentation

### Penetration Testing (2)
- [ ] PEN-L1: Timing attack mitigation
- [ ] PEN-L2: Error message cleanup

### Code Quality (5)
- [ ] CQ-L1 - CQ-L5: Minor code quality improvements

### Compliance (9)
- [ ] Various low-priority compliance items

---

## Remediation Log

| Date | Finding ID | Status | Commit | Notes |
|------|-----------|--------|--------|-------|
| | | | | |

---

## Verification Checklist

After all remediations:

- [ ] All tests pass (`cargo test`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Format check passes (`cargo fmt --check`)
- [ ] Documentation builds (`cargo doc`)
- [ ] Supply chain check (`cargo deny check`)
- [ ] Integration tests pass
- [ ] pr-review-toolkit verification complete
