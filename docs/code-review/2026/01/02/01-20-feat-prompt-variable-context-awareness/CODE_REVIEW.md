# Code Review Report

## Metadata
- **Project**: Subcog (Rust Memory System for AI Coding Assistants)
- **Review Date**: 2026-01-02
- **Reviewer**: Claude Code Deep-Clean Agent (6 parallel specialists)
- **Scope**: Full codebase (111 Rust source files, ~35,000 LOC)
- **Branch**: feat/prompt-variable-context-awareness
- **Commit**: 69343f1
- **LSP Available**: Yes (semantic code analysis enabled)
- **Methodology**: LSP semantic analysis + comprehensive file reading

---

## Executive Summary

### Overall Health Score: 7.0/10

| Dimension | Score | Critical | High | Medium | Low |
|-----------|-------|----------|------|--------|-----|
| Security | 8/10 | 0 | 1 | 3 | 4 |
| Performance | 7/10 | 0 | 3 | 4 | 4 |
| Architecture | 6/10 | 3 | 3 | 4 | 4 |
| Code Quality | 7/10 | 0 | 5 | 4 | 5 |
| Test Coverage | 5/10 | 2 | 4 | 4 | 2 |
| Documentation | 7/10 | 0 | 4 | 8 | 6 |

**Total Findings**: 77
- Critical: 5
- High: 20
- Medium: 27
- Low: 25

### Key Findings

1. **CRITICAL - Architecture**: Layer violations in MCP/CLI service creation - services should be injected, not constructed in transport layer
2. **CRITICAL - Architecture**: Legacy singleton `LegacyServiceContainer` coexists with modern instance-based design
3. **CRITICAL - Test Coverage**: Largest module (`sqlite.rs`, 1217 lines) has zero unit tests
4. **HIGH - Performance**: RRF fusion cloning in hot path (lines 284, 299 in recall.rs)
5. **HIGH - Security**: Weak JWT secret validation - only checks length, not entropy

### Recommended Action Plan

1. **Immediate** (before merge):
   - Fix RRF fusion cloning in recall.rs (performance)
   - Add XML escaping for LLM prompt injection defense

2. **This Sprint**:
   - Add unit tests for sqlite.rs (1217 lines untested)
   - Migrate layer-violating service creation to factory pattern
   - Add domain index for database queries

3. **Next Sprint**:
   - Remove legacy singleton pattern
   - Decompose god classes (PreCompactHandler at 1040 lines)
   - Add comprehensive environment variable documentation

4. **Backlog**:
   - Address clippy suppressions systematically
   - Improve test coverage for LLM resilience layer
   - Add documentation for MCP tool arguments

---

## Critical Findings (5)

### C1. Layer Violation: MCP Tools Directly Access Configuration and Create Services

**Location**: `src/mcp/tools.rs:26-29`, `src/cli/prompt.rs:91-94`
**Category**: Architecture
**Severity**: CRITICAL

**Description**:
MCP tools layer creates `PromptService` with full configuration, breaking the architecture layer model:
- MCP should consume pre-configured services, not bootstrap them
- Configuration loading should not be imported by transport layer
- Creates implicit dependency on SubcogConfig initialization order

**Evidence**:
```rust
// src/mcp/tools.rs:26-29
fn create_prompt_service(repo_path: &Path) -> PromptService {
    let config = SubcogConfig::load_default().with_repo_path(repo_path);
    PromptService::with_subcog_config(config).with_repo_path(repo_path)
}
```

**Impact**: Layer violations enable circular dependencies. Configuration leaks into transport/CLI layers. Difficult to swap implementations for testing.

**Remediation**:
Create factory function in services module that handles service initialization. Have MCP/CLI import from service factory, not directly instantiate.

```rust
// In services/mod.rs
pub fn prompt_service_for_repo(repo_path: &Path) -> Result<PromptService> {
    let config = SubcogConfig::load_default().with_repo_path(repo_path);
    Ok(PromptService::with_subcog_config(config).with_repo_path(repo_path))
}

// In mcp/tools.rs
use crate::services::prompt_service_for_repo;
let service = prompt_service_for_repo(repo_path)?;
```

---

### C2. Legacy Singleton ServiceContainer Coexists With Modern Instance-Based Design

**Location**: `src/services/mod.rs:353-475`
**Category**: Architecture
**Severity**: CRITICAL

**Description**:
The codebase maintains both:
- **Modern**: `ServiceContainer::for_repo()` / `ServiceContainer::from_current_dir()` (lines 90-129)
- **Legacy**: `LegacyServiceContainer` with global `OnceCell` (lines 354-475)

This creates two parallel service initialization paths with different semantics.

**Impact**: Maintainability nightmare. Different initialization behavior between codepaths leads to subtle bugs. Users can inadvertently use legacy path through deprecated exports.

**Remediation**:
Remove `LegacyServiceContainer` entirely and migrate all callsites. Add deprecation warnings to exports if legacy support required.

---

### C3. Missing Send+Sync Bounds on Generic Deduplication Service

**Location**: `src/services/deduplication/service.rs:56-67`
**Category**: Architecture
**Severity**: CRITICAL

**Description**:
```rust
pub struct DeduplicationService<E: Embedder, V: VectorBackend> {
    ...
    semantic: Option<SemanticSimilarityChecker<E, V>>,
    ...
}
```
The generic types `E` and `V` lack explicit `Send + Sync` bounds, even though the struct is used across thread boundaries in hooks.

**Impact**: Potential race conditions and unsoundness in multithreaded hook execution (SessionStart, PostToolUse, PreCompact).

**Remediation**:
```rust
pub struct DeduplicationService<E: Embedder + Send + Sync, V: VectorBackend + Send + Sync>
```

---

### C4. Critical Test Gap: sqlite.rs (1217 lines) Has Zero Unit Tests

**Location**: `src/storage/index/sqlite.rs`
**Category**: Test Coverage
**Severity**: CRITICAL

**Description**:
The largest module in the codebase (1217 lines) implementing FTS5 full-text search and SQLite indexing has no unit tests.

**Missing Tests**:
- FTS5 query syntax tests (operators, special chars)
- Concurrent index updates tests
- Search ranking/scoring tests
- Migration version tests
- Index corruption recovery tests
- Database lock handling
- Memory query parameter injection tests
- Search performance baseline tests

**Impact**: Core search functionality is untested. Regressions can go undetected.

**Remediation**:
Add comprehensive test module with at least 10 tests covering:
1. Basic CRUD operations
2. FTS5 query edge cases
3. Index migration
4. Concurrent access patterns

---

### C5. Critical Test Gap: LLM Resilience Layer Completely Untested

**Location**: `src/llm/resilience.rs` (400+ lines)
**Category**: Test Coverage
**Severity**: CRITICAL

**Description**:
The resilience layer implementing circuit breaker, retries, and error budget has zero unit tests.

**Missing Tests**:
- Circuit breaker state machine tests (open → half-open → closed)
- Retry logic tests (backoff, jitter, max attempts)
- Error budget tracking tests
- Latency SLO tests
- Concurrent call handling tests
- Failure mode tests (timeout, partial failure, cascading)
- Configuration validation tests

**Impact**: Production reliability is at risk. Circuit breaker bugs could cause cascading failures.

**Remediation**:
Add 12+ tests covering:
1. State machine transitions
2. Retry behavior with mocked failures
3. Error budget exhaustion scenarios
4. Timeout handling

---

## High Priority Findings (20)

### H1. Weak JWT Secret Validation in MCP HTTP Transport

**Location**: `src/mcp/auth.rs:69-84, 98-104`
**Category**: Security
**Severity**: HIGH

**Description**:
JWT authentication enforces minimum secret length of 32 characters but does not validate entropy or randomness.

**Evidence**:
```rust
if secret.len() < MIN_SECRET_LENGTH {
    return Err(Error::OperationFailed {
        operation: "jwt_config".to_string(),
        cause: format!("JWT secret must be at least {MIN_SECRET_LENGTH} characters"),
    });
}
// Only checks length, not entropy!
```

**Remediation**: Add entropy validation using zxcvbn-rs or require cryptographically random secrets.

---

### H2. RRF Fusion Double Clone - SearchHit Cloning in Hot Path

**Location**: `src/services/recall.rs:284, 299`
**Category**: Performance
**Severity**: HIGH

**Description**:
SearchHit structs containing full Memory data + embedded vectors are cloned twice per hybrid search result.

**Evidence**:
```rust
.or_insert((rrf_score, Some(hit.clone())));  // Line 284
.or_insert((rrf_score, Some(hit.clone())));  // Line 299
```

For limit=10, this results in 40 clones at ~12KB each = 480KB allocated just for RRF fusion.

**Remediation**: Use `Rc<SearchHit>` or `Arc<SearchHit>` instead of owned clones.

---

### H3. String ID Conversion in RRF Loop

**Location**: `src/services/recall.rs:278, 289`
**Category**: Performance
**Severity**: HIGH

**Description**:
Creates unnecessary String allocations per hybrid search result.

```rust
let id = hit.memory.id.as_str().to_string();  // Allocates new String
```

**Remediation**: Use `hit.memory.id` directly (already a String) or reference with `&hit.memory.id`.

---

### H4. Missing Index on Domain Column

**Location**: `src/storage/index/sqlite.rs:189-214`
**Category**: Performance
**Severity**: HIGH

**Description**:
Domain filters become full table scans on searches without namespace/status filters.

**Evidence**:
- idx_memories_namespace: Created ✓
- idx_memories_status: Created ✓
- idx_memories_domain: MISSING ✗

**Remediation**: Add `CREATE INDEX idx_memories_domain ON memories(domain)`.

---

### H5. Prompt Injection - Insufficient XML Escaping

**Location**: `src/llm/anthropic.rs:218-248`
**Category**: Security
**Severity**: MEDIUM (upgraded due to attack surface)

**Description**:
User content is not XML-escaped before wrapping in `<user_content>` tags, allowing potential tag breakout.

**Remediation**:
```rust
fn escape_xml(s: &str) -> String {
    s.replace("&", "&amp;")
     .replace("<", "&lt;")
     .replace(">", "&gt;")
}
let escaped_content = escape_xml(content);
```

---

### H6. God Class: PreCompactHandler (1040 lines)

**Location**: `src/hooks/pre_compact.rs:1-1040`
**Category**: Architecture
**Severity**: HIGH

**Description**:
Single handler class responsible for content analysis, memory auto-capture, deduplication orchestration, LLM-based classification, metrics recording, and hook response formatting.

**Remediation**: Extract `MemoryAnalyzer`, `CaptureOrchestrator`, and `ResponseFormatter` classes.

---

### H7. Excessive Module-Level Clippy Suppressions

**Location**: `src/services/mod.rs:5-20`, `src/storage/mod.rs`
**Category**: Code Quality
**Severity**: HIGH

**Description**:
8+ global lint suppressions hide legitimate issues behind blanket allowances.

```rust
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::option_if_let_else)]
#![allow(clippy::significant_drop_tightening)]
// ... 5 more
```

**Remediation**: Remove global suppressions, add targeted `#[allow(...)]` on specific functions with explanatory comments.

---

### H8. Excessive Nesting in PostgreSQL Storage

**Location**: `src/storage/prompt/postgresql.rs`
**Category**: Code Quality
**Severity**: HIGH

**Description**:
Deep if-let and match nesting (4+ levels) reduces readability.

**Remediation**: Extract helper methods using early returns and `?` operator.

---

### H9. Lines-of-Code Exceeding Best Practices

**Location**: Multiple files
**Category**: Code Quality
**Severity**: HIGH

**Files with >1000 LOC**:
- `src/mcp/prompts.rs`: 1787 lines
- `src/mcp/tools.rs`: 1540 lines
- `src/main.rs`: 1510 lines
- `src/hooks/search_intent.rs`: 1375 lines
- `src/config/mod.rs`: 1241 lines

**Remediation**: Split into submodules.

---

### H10. Parameter List Exceeding 5 Arguments

**Location**: `src/cli/prompt.rs:115-124`
**Category**: Code Quality
**Severity**: HIGH

**Description**:
`cmd_prompt_save` has 9 parameters.

**Remediation**: Use builder pattern or struct wrapper:
```rust
struct SavePromptOptions {
    name: String,
    content: Option<String>,
    // ... rest of parameters
}
```

---

### H11. Inconsistent Clone Usage - Excessive Allocations

**Location**: Multiple files (200+ call sites)
**Category**: Code Quality
**Severity**: HIGH

**Description**:
200+ `.clone()` calls throughout codebase, many avoidable.

**Remediation**: Systematically replace with references/borrows where possible.

---

### H12. HookHandler Trait Lacks Error Handling Specification

**Location**: `src/hooks/mod.rs:1-44`
**Category**: Architecture
**Severity**: HIGH

**Description**:
`PreCompactHandler::new()` creates handler with all `None` fields. Calling `handle()` on this will silently fail or panic.

**Remediation**: Make handler construction require dependencies or return `Result<PreCompactHandler>` from factory.

---

### H13. Environment Variable Documentation Missing

**Location**: `src/config/mod.rs`
**Category**: Documentation
**Severity**: HIGH

**Description**:
No centralized documentation of 20+ environment variables including:
- `SUBCOG_CONFIG_PATH`, `SUBCOG_DATA_DIR`, `SUBCOG_REPO_PATH`
- `SUBCOG_SEARCH_INTENT_*` (4 variables)
- `SUBCOG_DEDUP_*` (8 variables)
- `SUBCOG_MCP_JWT_*` (3 variables)
- LLM provider keys

**Remediation**: Create `docs/environment-variables.md` with all supported variables.

---

### H14. MCP Tool Arguments Documentation Missing

**Location**: `src/mcp/tool_types.rs`
**Category**: Documentation
**Severity**: HIGH

**Description**:
Tool argument structs lack field-level documentation. Missing docs for `CaptureArgs`, `RecallArgs`, `PromptSaveArgs`, `PromptRunArgs`.

**Remediation**: Add comprehensive doc comments to all `*Args` structs.

---

### H15. CLI Module Documentation Missing

**Location**: `src/cli/mod.rs`
**Category**: Documentation
**Severity**: HIGH

**Description**:
Public functions exported without module-level context explaining CLI command routing.

**Remediation**: Add comprehensive module comment explaining command patterns.

---

### H16. LLM Provider Trait Documentation Incomplete

**Location**: `src/llm/mod.rs`
**Category**: Documentation
**Severity**: HIGH

**Description**:
Trait methods lack implementation guidance for timeout expectations, retry behavior, and error recovery patterns.

**Remediation**: Add comprehensive trait documentation with implementation examples.

---

### H17. Test Gap: llm_factory.rs Completely Untested

**Location**: `src/cli/llm_factory.rs` (82 lines)
**Category**: Test Coverage
**Severity**: HIGH

**Description**:
Factory functions for all LLM clients have no tests.

**Missing Tests**:
- Null/missing config handling
- Config override precedence (env vars vs config file)
- Client builder chain tests
- Fallback/default value tests

**Remediation**: Add 6 tests for builder chain validation.

---

### H18. Test Gap: PostgreSQL Storage Backend Untested

**Location**: `src/storage/persistence/postgresql.rs` (400+ lines)
**Category**: Test Coverage
**Severity**: HIGH

**Description**:
Production-critical storage backend has no integration tests.

**Remediation**: Add 8 integration tests with test PostgreSQL.

---

### H19. Test Gap: Filesystem Storage Backend Untested

**Location**: `src/storage/persistence/filesystem.rs` (400+ lines)
**Category**: Test Coverage
**Severity**: HIGH

**Description**:
Fallback storage backend has no tests for file I/O error handling, concurrent access, or corruption recovery.

**Remediation**: Add 6 tests using temp directories.

---

### H20. Word-Level Processing in Embedding with Nested Loops

**Location**: `src/embedding/fastembed.rs:46-74`
**Category**: Performance
**Severity**: MEDIUM (architectural)

**Description**:
O(n*m) nested loop in hot path with 8 inner iterations per word, causing poor cache locality.

**Remediation**: Batch hash computation, use better index distribution algorithm.

---

## Medium Priority Findings (27)

### M1. TOCTOU in Audit File Operations
**Location**: `src/security/audit.rs:373-385`
**Category**: Security
**Severity**: MEDIUM

Race condition between checking file existence and writing. Remediation: Use atomic file operations with path canonicalization.

---

### M2. Missing Per-Client Rate Limiting
**Location**: `src/mcp/server.rs:24-28`
**Category**: Security
**Severity**: MEDIUM

Rate limit applies globally, not per-client. Single malicious client can exhaust limit for all users.

---

### M3. API Key Format Validation Minimal
**Location**: `src/llm/anthropic.rs:93-96`
**Category**: Security
**Severity**: LOW (upgraded)

Only checks prefix and length, not character set or checksum.

---

### M4. ServiceContainer Mutex Synchronization Overhead
**Location**: `src/services/mod.rs:74, 137-156`
**Category**: Architecture
**Severity**: MEDIUM

Lock held during potentially expensive disk operations in `recall_for_scope()`.

---

### M5. N+1 Optimization Trap in IndexBackend
**Location**: `src/storage/traits/index.rs:81-84`
**Category**: Architecture
**Severity**: MEDIUM

Default `get_memories_batch()` implementation creates N queries. Callers must remember to override.

---

### M6. Error Type Proliferation
**Location**: `src/lib.rs:71-118`
**Category**: Architecture
**Severity**: MEDIUM

All errors use same `Error` type. No distinction between fatal vs recoverable.

---

### M7. Incomplete Deduplication Module Visibility
**Location**: `src/services/deduplication/mod.rs:39-53`
**Category**: Architecture
**Severity**: MEDIUM

All internal components are public, allowing circumvention of orchestrator.

---

### M8. FTS Query String Building with Repeated Allocations
**Location**: `src/storage/index/sqlite.rs:616-624`
**Category**: Performance
**Severity**: MEDIUM

For 10-term query: 21 String allocations for 100-byte output.

---

### M9. Mutex Lock Contention in SQLiteBackend
**Location**: `src/storage/index/sqlite.rs:14-70`
**Category**: Performance
**Severity**: MEDIUM

std::sync::Mutex is OS-level, not async-aware. No timeout protection.

---

### M10-M27. Additional Medium Findings
- Dead code suppressions without explanation (2 findings)
- Cast warnings not addressed in SQLite storage
- Unused self in multiple files (4 locations)
- Repetitive match arms in main command handler
- DRY violation in regex initialization
- Missing documentation for deduplication config
- Missing documentation for prompt enrichment
- Missing documentation for search intent
- Missing documentation for prompt storage
- Missing documentation for prompt variables
- Storage backend error handling undocumented
- Hook handler response format undocumented

---

## Low Priority Findings (25)

### L1-L4. Security Low Findings
- Missing OWASP headers in HTTP server
- Overly broad secrets regex patterns
- Missing audit log file permissions validation
- Weak API key format validation (character set)

### L5-L11. Performance Low Findings
- Inefficient string label generation in metrics
- Query value clone in event recording loop
- Pre-allocation patterns (some already addressed)
- Thread sleep spin-wait in lock acquisition (dead code)

### L12-L18. Architecture Low Findings
- Inconsistent configuration loading pattern
- PromptTemplate variable extraction edge cases
- Hardcoded constants scattered across modules
- SearchIntent module in wrong layer
- Missing DomainScope documentation

### L19-L25. Documentation/Quality Low Findings
- Error type documentation incomplete
- Service container domain hierarchy undocumented
- RecallService score calculation undocumented
- Configuration loading order undocumented
- Security filtering behavior undocumented
- String-based enum parsing patterns
- Unused import warnings

---

## Appendix

### Files Reviewed

All 111 Rust source files in `src/` were analyzed by specialist agents.

**Key Directories**:
- `src/cli/` - 12 files (CLI commands)
- `src/services/` - 13 files + deduplication/ (business logic)
- `src/mcp/` - 8 files (MCP server)
- `src/hooks/` - 9 files (Claude Code hooks)
- `src/storage/` - 4 subdirectories (three-layer storage)
- `src/models/` - 8 files (data structures)
- `src/llm/` - 7 files (LLM clients)
- `src/security/` - 5 files (security features)

### Tools & Methods

**Agents Deployed**:
1. Security Analyst - OWASP Top 10, secrets scanning, auth review
2. Performance Engineer - Hot path analysis, allocation tracking
3. Architecture Reviewer - SOLID principles, layer violations, coupling
4. Code Quality Analyst - DRY, complexity, linting
5. Test Coverage Analyst - Unit test gaps, integration test needs
6. Documentation Reviewer - Doc comments, README, API docs

**LSP Operations Used**:
- goToDefinition - Navigating to implementations
- findReferences - Detecting dead code, usage patterns
- hover - Type information and documentation
- documentSymbol - File structure analysis

### Recommendations for Future Reviews

1. **Add automated checks**:
   - Clippy CI with pedantic + nursery lints (already configured)
   - Coverage threshold enforcement (target: 70%)
   - Complexity metrics (max cyclomatic complexity: 15)

2. **CI integration**:
   - Pre-commit hooks for formatting
   - PR checks for test coverage delta
   - Security scanning with cargo-deny

3. **Documentation standards**:
   - Require doc comments on all public items
   - Automated doc generation in CI
