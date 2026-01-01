# Code Review Report

**Project**: Subcog Pre-Compact Deduplication
**Branch**: `plan/pre-compact-deduplication`
**Date**: 2026-01-01
**Reviewers**: 10 Specialist Agents (Security, Performance, Architecture, Code Quality, Test Coverage, Documentation, Database, Penetration Testing, Compliance, Chaos Engineering)

---

## Executive Summary

| Dimension | Score | Status |
|-----------|-------|--------|
| Security | 6/10 | Needs Improvement |
| Performance | 5/10 | Critical Issues |
| Architecture | 6/10 | God Files Detected |
| Code Quality | 7/10 | Good with Issues |
| Test Coverage | 4/10 | Critical Gaps |
| Documentation | 6/10 | Missing Docs |
| Database | 5/10 | SQL Injection Risk |
| Resilience | 5/10 | Missing Timeouts |
| Compliance | 4/10 | SOC2 65%, GDPR 45% |

**Overall Health Score**: 5.3/10

**Total Findings**: 169
- Critical: 18
- High: 47
- Medium: 68
- Low: 36

---

## 1. Security Analysis

**Analyst**: Security Analyst Agent
**Files Reviewed**: All 104 Rust files

### Findings

#### CRITICAL (0)
None identified.

#### HIGH (1)

| ID | File | Line | Issue | Impact |
|----|------|------|-------|--------|
| SEC-H1 | `src/mcp/server.rs` | 116-137 | MCP server lacks authentication | Unauthorized access to memory operations |

#### MEDIUM (4)

| ID | File | Line | Issue | Impact |
|----|------|------|-------|--------|
| SEC-M1 | `src/llm/anthropic.rs` | 45-50 | API key validation missing | Invalid keys not rejected early |
| SEC-M2 | `src/storage/persistence/filesystem.rs` | 112-130 | Path traversal possible | Arbitrary file access |
| SEC-M3 | `src/llm/anthropic.rs` | 89-120 | Prompt injection in LLM calls | Malicious prompts executed |
| SEC-M4 | `src/mcp/server.rs` | 200-220 | No rate limiting | DoS via request flooding |

#### LOW (4)

| ID | File | Line | Issue | Impact |
|----|------|------|-------|--------|
| SEC-L1 | `src/config/mod.rs` | 45-60 | Config file permissions not checked | World-readable secrets |
| SEC-L2 | `src/llm/*.rs` | Various | API keys in memory | Memory dumps expose keys |
| SEC-L3 | `src/hooks/*.rs` | Various | Error messages leak paths | Information disclosure |
| SEC-L4 | `src/mcp/tools.rs` | 500-550 | Verbose error responses | Stack traces exposed |

### Strengths
- SQL injection protected via parameterized queries
- Secrets/PII detection implemented
- No unsafe code blocks
- Proper error handling in most places

---

## 2. Performance Analysis

**Analyst**: Performance Engineer Agent
**Files Reviewed**: Core services, storage backends

### Findings

#### CRITICAL (4)

| ID | File | Line | Issue | Impact |
|----|------|------|-------|--------|
| PERF-C1 | `src/services/recall.rs` | 89-145 | N+1 query pattern in search | O(n) database calls |
| PERF-C2 | `src/storage/index/postgresql.rs` | 280-322 | Blocking async in pool.get() | Thread starvation |
| PERF-C3 | `src/storage/index/postgresql.rs` | 45-60 | No connection pool limits | Pool exhaustion |
| PERF-C4 | `src/embedding/fastembed.rs` | 40-55 | Model loaded per call | Repeated 500ms startup |

#### HIGH (4)

| ID | File | Line | Issue | Impact |
|----|------|------|-------|--------|
| PERF-H1 | `src/mcp/resources.rs` | 800-900 | Unbounded Vec growth | Memory exhaustion |
| PERF-H2 | `src/hooks/search_intent.rs` | 450-520 | O(n²) pattern matching | Slow on large prompts |
| PERF-H3 | `src/services/consolidation.rs` | 200-280 | Full table scan for similar | Linear search |
| PERF-H4 | `src/storage/vector/usearch.rs` | 180-220 | Index rebuilt on each add | O(n log n) per insert |

#### MEDIUM (2)

| ID | File | Line | Issue | Impact |
|----|------|------|-------|--------|
| PERF-M1 | `src/embedding/fastembed.rs` | 88-103 | No embedding cache | Duplicate computations |
| PERF-M2 | `src/llm/anthropic.rs` | 60-80 | No HTTP connection reuse | New TCP per request |

#### LOW (1)

| ID | File | Line | Issue | Impact |
|----|------|------|-------|--------|
| PERF-L1 | `src/models/memory.rs` | 45-80 | Clone on every access | Avoidable allocations |

---

## 3. Architecture Analysis

**Analyst**: Architecture Reviewer Agent
**Files Reviewed**: All modules, focusing on structure

### God Files Detected

| Severity | File | Lines | Issue |
|----------|------|-------|-------|
| CRITICAL | `src/mcp/resources.rs` | 1,969 | Monolithic resource handling |
| CRITICAL | `src/mcp/tools.rs` | 1,698 | Missing Strategy pattern |
| CRITICAL | `src/hooks/search_intent.rs` | 1,612 | Complex intent detection |
| HIGH | `src/main.rs` | 1,177 | LLM factory in main |
| HIGH | `src/hooks/pre_compact.rs` | 876 | Large handler |
| HIGH | `src/cli/prompt.rs` | 654 | CLI mixed with logic |
| MEDIUM | `src/services/recall.rs` | 521 | Search logic intertwined |
| MEDIUM | `src/storage/index/sqlite.rs` | 498 | Schema mixed with queries |

### Structural Issues

| ID | Issue | Impact |
|----|-------|--------|
| ARCH-1 | Embedded content in code (examples, prompts) | Hard to maintain |
| ARCH-2 | LLM factory functions in main.rs | Should be in module |
| ARCH-3 | Configuration scattered across modules | No single source of truth |
| ARCH-4 | Trait implementations in same file as struct | Violates SRP |
| ARCH-5 | Test utilities duplicated across modules | Code duplication |

---

## 4. Code Quality Analysis

**Analyst**: Code Quality Agent
**Files Reviewed**: All source files

### Findings

#### HIGH (4)

| ID | File | Line | Issue |
|----|------|------|-------|
| CQ-H1 | `src/hooks/*.rs` | Various | Duplicated `current_timestamp()` function |
| CQ-H2 | `src/llm/*.rs` | Various | Duplicated `extract_json_from_response()` |
| CQ-H3 | `src/mcp/tools.rs` | 1200-1400 | Large match arms (>50 lines each) |
| CQ-H4 | `src/hooks/search_intent.rs` | 300-400 | Deep nesting (5+ levels) |

#### MEDIUM (9)

| ID | File | Issue |
|----|------|-------|
| CQ-M1 | Various | Magic numbers without constants |
| CQ-M2 | Various | Inconsistent error construction |
| CQ-M3 | `src/cli/*.rs` | println! instead of tracing |
| CQ-M4 | Various | Inconsistent naming (snake_case vs camelCase in JSON) |
| CQ-M5 | Various | Dead code (unused functions) |
| CQ-M6 | Various | Overly complex match expressions |
| CQ-M7 | Various | Missing #[must_use] annotations |
| CQ-M8 | Various | Inconsistent Result vs Option usage |
| CQ-M9 | Various | String concatenation instead of format! |

#### LOW (5)

| ID | Issue |
|----|-------|
| CQ-L1 | Inconsistent import ordering |
| CQ-L2 | Mixed use of `Self` vs type name |
| CQ-L3 | Unnecessary `pub` visibility |
| CQ-L4 | Redundant clones |
| CQ-L5 | Verbose where clauses |

---

## 5. Test Coverage Analysis

**Analyst**: Test Coverage Agent
**Current State**: 619 tests, ~70% coverage

### Critical Gaps

| File | Test Count | Gap |
|------|------------|-----|
| `src/cli/capture.rs` | 0 | Full CLI untested |
| `src/cli/recall.rs` | 0 | Full CLI untested |
| `src/cli/status.rs` | 0 | Full CLI untested |
| `src/cli/sync.rs` | 0 | Full CLI untested |
| `src/cli/config.rs` | 0 | Full CLI untested |
| `src/cli/serve.rs` | 0 | Full CLI untested |
| `src/cli/hook.rs` | 0 | Full CLI untested |
| `src/cli/prompt.rs` | 0 | Full CLI untested |

### Missing Test Types

| Type | Status | Estimate |
|------|--------|----------|
| Unit tests for CLI | Missing | ~80 tests |
| Edge case tests for dedup | Missing | ~30 tests |
| Integration tests (cross-module) | Missing | ~40 tests |
| Property-based tests (more) | Partial | ~20 tests |
| Stress/load tests | Missing | ~15 tests |
| Fuzz tests | Missing | ~15 tests |

**Estimated Tests Needed**: ~200-250 additional tests

---

## 6. Documentation Analysis

**Analyst**: Documentation Reviewer Agent

### Missing Documentation

#### HIGH Priority

| Item | Location | Issue |
|------|----------|-------|
| `HookCommand` enum | `src/cli/hook.rs` | No docstrings |
| `SubcogConfig` fields | `src/config/mod.rs` | 15+ undocumented fields |
| `LlmProvider` trait | `src/llm/mod.rs` | No usage examples |
| `VectorBackend` trait | `src/storage/traits/vector.rs` | No examples |
| Deduplication service | `CLAUDE.md` | Not mentioned |

#### MEDIUM Priority

| Item | Issue |
|------|-------|
| Error types | Missing when/why documentation |
| MCP resources | No usage examples in docs |
| Hook lifecycle | No sequence diagrams |
| Configuration | No example config files |

---

## 7. Database Analysis

**Analyst**: Database Expert Agent

### Findings

#### CRITICAL (2)

| ID | File | Line | Issue |
|----|------|------|-------|
| DB-C1 | `src/storage/index/postgresql.rs` | 156 | SQL injection via table name interpolation |
| DB-C2 | `src/storage/index/postgresql.rs` | 45-60 | No connection pool configuration |

#### HIGH (8)

| ID | File | Issue |
|----|------|-------|
| DB-H1 | `sqlite.rs` | Missing indexes on namespace, domain columns |
| DB-H2 | `sqlite.rs` | No transaction support for batch operations |
| DB-H3 | `sqlite.rs` | BM25 normalization calculation incorrect |
| DB-H4 | `postgresql.rs` | No prepared statement caching |
| DB-H5 | `postgresql.rs` | No TLS configuration |
| DB-H6 | `redis.rs` | No connection pooling |
| DB-H7 | `redis.rs` | Unbounded SCAN operations |
| DB-H8 | `sqlite.rs` | No WAL mode for concurrent reads |

#### MEDIUM (12)

Various query optimization opportunities, missing EXPLAIN ANALYZE, inefficient JOINs.

#### LOW (6)

Naming conventions, comment quality, schema documentation.

---

## 8. Penetration Testing Analysis

**Analyst**: Penetration Tester Agent

### Findings

#### CRITICAL (0)
None identified.

#### HIGH (5)

| ID | File | Line | Vulnerability | CVSS |
|----|------|------|---------------|------|
| PEN-H1 | `postgresql.rs` | 156 | SQL injection (table names) | 8.1 |
| PEN-H2 | `filesystem.rs` | 112-130 | Path traversal | 7.5 |
| PEN-H3 | `parser.rs` | 45-80 | YAML DoS (billion laughs) | 7.5 |
| PEN-H4 | `filesystem.rs` | 200-220 | File size not validated | 6.5 |
| PEN-H5 | `mod.rs` | 89 | URL decode injection | 6.1 |

#### MEDIUM (6)

| ID | Issue |
|----|-------|
| PEN-M1 | Redis query injection |
| PEN-M2 | MCP lacks authentication |
| PEN-M3 | Information disclosure in errors |
| PEN-M4 | No input sanitization on tags |
| PEN-M5 | Regex ReDoS potential |
| PEN-M6 | Memory ID predictable |

#### LOW (2)

| ID | Issue |
|----|-------|
| PEN-L1 | Timing attacks on auth |
| PEN-L2 | Error messages leak internals |

---

## 9. Compliance Analysis

**Analyst**: Compliance Auditor Agent

### Compliance Readiness

| Framework | Score | Status |
|-----------|-------|--------|
| SOC2 | 65% | Gaps in encryption, access control |
| GDPR | 45% | No deletion, no consent tracking |
| HIPAA | 35% | Missing audit logs, encryption |
| PCI-DSS | 40% | No encryption at rest |

### CRITICAL Gaps (7)

| ID | Requirement | Gap |
|----|-------------|-----|
| COMP-C1 | Encryption at rest | Not implemented |
| COMP-C2 | GDPR right to deletion | No delete capability |
| COMP-C3 | TLS for data in transit | Not enforced |
| COMP-C4 | RBAC | No role-based access |
| COMP-C5 | Audit logging | Incomplete |
| COMP-C6 | Data classification | Not implemented |
| COMP-C7 | Consent tracking | Missing |

### HIGH Gaps (12)

Access control, key management, backup/recovery, incident response, vendor management, change control, etc.

---

## 10. Chaos Engineering / Resilience Analysis

**Analyst**: Chaos Engineer Agent

### Findings

#### CRITICAL (3)

| ID | File | Line | Issue |
|----|------|------|-------|
| CHAOS-C1 | `git/remote.rs` | 95-134 | No timeout on git fetch/push |
| CHAOS-C2 | `mcp/server.rs` | 116-137 | Unbounded stdio loop |
| CHAOS-C3 | `storage/index/sqlite.rs` | 82-85 | Mutex can poison and deadlock |

#### HIGH (3)

| ID | File | Issue |
|----|------|-------|
| CHAOS-H1 | `postgresql.rs` | Connection pool exhaustion possible |
| CHAOS-H2 | `redis.rs` | No timeout on Redis commands |
| CHAOS-H3 | `search_intent.rs` | Spawned thread continues after timeout |

#### MEDIUM (3)

| ID | Issue |
|----|-------|
| CHAOS-M1 | Vector search missing backpressure |
| CHAOS-M2 | Embedding generation no timeout |
| CHAOS-M3 | Sync service missing retry with backoff |

#### LOW (3)

| ID | Issue |
|----|-------|
| CHAOS-L1 | Capture service no circuit breaker |
| CHAOS-L2 | SystemTime unwrap_or silent failure |
| CHAOS-L3 | File I/O no timeout |

### Resilience Strengths

- LLM resilience module with circuit breaker, retry, backoff
- HTTP client configurable timeouts
- Search intent graceful degradation

---

## Recommendations

### Immediate Actions (< 24 hours)
1. Fix SQL injection in PostgreSQL table name interpolation
2. Add timeouts to git remote operations
3. Implement rate limiting on MCP server
4. Configure PostgreSQL connection pool limits
5. Add mutex timeout and poison recovery for SQLite

### Short-term (< 1 week)
1. Decompose god files (mcp/resources.rs, mcp/tools.rs, search_intent.rs)
2. Add missing indexes to SQLite schema
3. Implement N+1 query fix in RecallService
4. Add CLI tests (0 → 80+)
5. Fix path traversal vulnerability

### Medium-term (< 1 month)
1. Implement encryption at rest
2. Add GDPR deletion capability
3. Implement RBAC
4. Add comprehensive audit logging
5. Increase test coverage to 85%+

---

## Appendix: Files Reviewed

```
src/
├── cli/ (8 files, 0 tests)
├── config/ (2 files)
├── embedding/ (3 files)
├── git/ (3 files)
├── hooks/ (7 files)
├── llm/ (6 files)
├── mcp/ (4 files)
├── models/ (7 files)
├── observability/ (4 files)
├── security/ (4 files)
├── services/ (9 files)
│   └── deduplication/ (7 files, 64+ tests)
└── storage/ (10 files)

Total: 104 files, 27,338 lines of code
```
