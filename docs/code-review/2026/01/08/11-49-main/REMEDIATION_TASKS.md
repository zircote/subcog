# Remediation Tasks

**Generated**: 2026-01-08T11:49:00Z
**Branch**: main
**Total Findings**: 195
**Auto-Remediation**: ALL (Critical → Low)

---

## Remediation Priority Order

Tasks are ordered by severity, then by implementation complexity (simpler first within each severity level).

---

## Phase 1: CRITICAL (7 findings)

### CHAOS-CRIT-001/002/003: Add retry logic to LLM clients
**Files**: `src/llm/anthropic.rs`, `src/llm/openai.rs`, `src/llm/ollama.rs`
**Effort**: ~2 hours
**Action**:
- Add exponential backoff retry with jitter
- Configure max retries (default: 3)
- Add timeout per attempt
- Log retry attempts at warn level

```rust
// Implementation pattern
use tokio_retry::{strategy::ExponentialBackoff, Retry};

let retry_strategy = ExponentialBackoff::from_millis(100)
    .factor(2)
    .max_delay(Duration::from_secs(10))
    .take(3);

Retry::spawn(retry_strategy, || async {
    self.do_request(prompt).await
}).await
```

### COMP-CRIT-002: Enable encryption by default
**File**: `src/config/mod.rs`
**Effort**: ~15 minutes
**Action**:
- Change `encryption.enabled` default from `false` to `true`
- Update tests that assume unencrypted storage
- Add migration note to CHANGELOG

### COMP-CRIT-003: Document stdio transport security model
**File**: `src/mcp/transport/stdio.rs`, `README.md`
**Effort**: ~30 minutes
**Action**:
- Add security warning in module docs
- Document stdio is for local development only
- Recommend HTTP transport with auth for production
- Consider adding environment variable guard

### TEST-CRIT-001: Add CaptureService integration tests
**File**: `tests/capture_integration.rs` (new)
**Effort**: ~2 hours
**Action**:
- Full capture flow: request → storage → index → vector
- Test with SQLite backend
- Test error handling paths
- Add to CI matrix

### COMP-CRIT-001: Implement GDPR data export
**Files**: `src/services/export.rs` (new), `src/mcp/tools/handlers/core.rs`
**Effort**: ~4 hours
**Action**:
- Add `export_user_data(user_id)` service method
- Export all memories, prompts, and metadata
- Support JSON and CSV formats
- Add MCP tool `subcog_export`
- Document in GDPR compliance section

---

## Phase 2: HIGH (30 findings)

### DEP-HIGH-002: Replace serde_yaml with serde_yml
**Files**: `Cargo.toml`, all files using serde_yaml
**Effort**: ~30 minutes
**Action**:
- Replace `serde_yaml` with `serde_yml` in Cargo.toml
- Update all import statements
- Verify YAML parsing behavior unchanged

### DEP-HIGH-001: Acknowledge RUSTSEC-2023-0071
**File**: `deny.toml`
**Effort**: ~15 minutes
**Action**:
- Add advisory to ignore list with justification
- RSA timing is transitive via ort (fastembed)
- Document that we don't use RSA directly
- Track ort stable release for fix

### PERF-HIGH-001: Optimize SearchHit cloning in RRF fusion
**File**: `src/services/recall.rs`
**Effort**: ~1 hour
**Action**:
- Use references instead of cloning
- Consider `Cow<SearchHit>` for deferred cloning
- Profile with criterion before/after

### PERF-HIGH-002: Optimize Memory cloning in lazy tombstone
**File**: `src/services/recall.rs`
**Effort**: ~1 hour
**Action**:
- Move instead of clone where possible
- Use `Arc<Memory>` for shared ownership
- Extract query from mutation

### PERF-HIGH-003: Fix N+1 query in branch GC
**File**: `src/gc/branch.rs`
**Effort**: ~1.5 hours
**Action**:
- Batch load branch metadata
- Use single query with IN clause
- Add pagination for large result sets

### PERF-HIGH-004: Reduce string allocation in embed_batch
**File**: `src/embedding/fastembed.rs`
**Effort**: ~45 minutes
**Action**:
- Use `&[&str]` instead of `Vec<String>`
- Pre-allocate output vector
- Consider streaming for large batches

### ARCH-HIGH-001: Fix CQS violation in lazy_tombstone_stale_branches
**File**: `src/services/recall.rs`
**Effort**: ~2 hours
**Action**:
- Separate query from command
- Extract mutation to dedicated method
- Call mutation explicitly in caller
- Update tests

### ARCH-HIGH-002: Split SubcogConfig god object
**File**: `src/config/mod.rs`
**Effort**: ~3 hours
**Action**:
- Extract `StorageConfig`
- Extract `SecurityConfig`
- Extract `ObservabilityConfig`
- Use builder pattern for composition

### CHAOS-HIGH-001: Add circuit breakers to external services
**Files**: `src/llm/*.rs`, `src/embedding/fastembed.rs`
**Effort**: ~3 hours
**Action**:
- Implement simple circuit breaker pattern
- States: Closed → Open → Half-Open
- Configure failure threshold and reset timeout
- Add metrics for circuit state

### CHAOS-HIGH-002-006: Add graceful degradation
**Various files**
**Effort**: ~4 hours
**Action**:
- Embedding fallback to BM25-only
- Database connection retry with backoff
- Service isolation with bulkhead pattern
- Configurable timeouts per operation
- Health check endpoints for all services

### TEST-HIGH-001/002: Add PostgreSQL/Redis integration tests
**Files**: `tests/postgresql_integration.rs`, `tests/redis_integration.rs` (new)
**Effort**: ~4 hours
**Action**:
- Use testcontainers for isolated instances
- Test CRUD operations
- Test connection failure handling
- Add to CI with container services

### TEST-HIGH-003: Add LLM client error handling tests
**File**: `tests/llm_integration.rs` (new)
**Effort**: ~2 hours
**Action**:
- Mock HTTP responses for error cases
- Test timeout handling
- Test rate limit handling
- Test malformed response handling

### TEST-HIGH-004: Add MCP server E2E tests
**File**: `tests/mcp_e2e.rs` (new)
**Effort**: ~3 hours
**Action**:
- Spawn server in subprocess
- Test tool execution via JSON-RPC
- Test resource access
- Test error responses

### TEST-HIGH-005: Add hook edge case tests
**Files**: `tests/hooks_*.rs`
**Effort**: ~2 hours
**Action**:
- Test with empty context
- Test with malformed input
- Test timeout scenarios
- Test concurrent hook execution

### DOC-HIGH-001/002/003: Add API documentation examples
**Files**: `src/services/capture.rs`, `src/services/recall.rs`, `src/services/mod.rs`
**Effort**: ~2 hours
**Action**:
- Add `# Examples` section to doc comments
- Ensure examples compile with `cargo test --doc`
- Include error handling examples

### DB-HIGH-001/002: Add Redis health checks
**Files**: `src/storage/index/redis.rs`, `src/storage/vector/redis.rs`
**Effort**: ~1 hour
**Action**:
- Add `health_check()` method to backends
- PING command on connection
- Validate connection before use
- Add to health endpoint

### COMP-HIGH-001: Implement data retention enforcement
**File**: `src/gc/retention.rs`
**Effort**: ~3 hours
**Action**:
- Add configurable retention periods per namespace
- Schedule automatic cleanup
- Audit log deletions
- Add GDPR documentation

### COMP-HIGH-002: Protect audit log integrity
**File**: `src/security/audit.rs`
**Effort**: ~2 hours
**Action**:
- Add HMAC signature to log entries
- Chain entries with hash links
- Prevent tampering detection
- Document verification procedure

### COMP-HIGH-003-006: Add compliance controls
**Various files**
**Effort**: ~8 hours (can be deferred)
**Action**:
- Consent tracking mechanism
- Access review reports
- PII disclosure logging
- Separation of duties (RBAC foundation)

---

## Phase 3: MEDIUM (64 findings)

### Code quality improvements
**Various files**
**Effort**: ~4 hours
**Action**:
- Move `#[allow(clippy::...)]` to function level
- Resolve TODO comments or create issues
- Reduce function complexity
- Standardize error messages

### Performance optimizations
**Various files**
**Effort**: ~3 hours
**Action**:
- Pre-allocate HashMaps
- Cache compiled regexes
- Use `&str` over `String` where possible
- Tune connection pool sizes

### Architecture improvements
**Various files**
**Effort**: ~4 hours
**Action**:
- Reduce ServiceContainer coupling
- Organize feature flags
- Standardize error types by layer
- Align hook handler interfaces

### Test coverage expansion
**Various test files**
**Effort**: ~6 hours
**Action**:
- Deduplication semantic checker tests
- Encryption round-trip tests
- GC edge cases
- Config loading errors

### Documentation updates
**Various files**
**Effort**: ~4 hours
**Action**:
- Error handling patterns
- Storage architecture
- Configuration reference
- Feature flag documentation

### Database optimizations
**Storage files**
**Effort**: ~3 hours
**Action**:
- Optimize SQLite PRAGMAs
- Make pool size configurable
- Add query logging
- Verify index usage

### Security hardening
**MCP files**
**Effort**: ~2 hours
**Action**:
- Generic error messages for auth failures
- Add YAML complexity limits
- Pagination limits on listings
- Security headers for HTTP

### Dependency management
**Cargo.toml**
**Effort**: ~2 hours
**Action**:
- Evaluate chrono alternatives
- Optimize feature flags
- Remove duplicate dependencies
- Update outdated dependencies

### Rust idiom improvements
**Various files**
**Effort**: ~2 hours
**Action**:
- Replace `Box<dyn Error>` with `anyhow`
- Use `&str` parameters consistently
- Add `#[non_exhaustive]` to public enums

---

## Phase 4: LOW (92 findings)

Low severity findings are documented but not prioritized for immediate remediation. They can be addressed opportunistically during related work.

### Categories:
- Documentation polish (15 findings)
- Code style consistency (20 findings)
- Test infrastructure improvements (12 findings)
- Performance micro-optimizations (15 findings)
- Rust idiom polish (15 findings)
- Future architecture considerations (15 findings)

---

## Estimated Total Effort

| Phase | Findings | Estimated Hours |
|-------|----------|-----------------|
| CRITICAL | 7 | ~10 hours |
| HIGH | 30 | ~45 hours |
| MEDIUM | 64 | ~30 hours |
| LOW | 92 | ~20 hours |
| **TOTAL** | **193** | **~105 hours** |

---

## Remediation Order (Auto-Execute)

When `--all` flag is active, execute in this order:

1. **Critical fixes** (blocking issues)
2. **Dependency updates** (quick wins)
3. **Performance fixes** (measurable impact)
4. **Test coverage** (quality gates)
5. **Documentation** (knowledge capture)
6. **Architecture improvements** (long-term health)
7. **Low-priority polish** (opportunistic)

---

## Verification Checklist

After remediation:
- [x] `make ci` passes (format, clippy, test, doc, deny) ✓
- [ ] No new warnings introduced
- [ ] Test coverage maintained or improved
- [ ] Documentation updated where needed
- [ ] CHANGELOG updated with security fixes
- [ ] Performance benchmarks validated

---

*Generated by /claude-spec:deep-clean remediation planner*
