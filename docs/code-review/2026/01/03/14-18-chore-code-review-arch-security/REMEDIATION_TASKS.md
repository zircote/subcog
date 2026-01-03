# Remediation Tasks

**Date**: 2026-01-03
**Branch**: `chore/code-review-arch-security`
**Total Findings**: 176 (7 Critical, 44 High, 63 Medium, 62 Low)

---

## Critical (7) - Immediate

- [x] **CRIT-001**: Wrap PostgreSQL migrations in transactions ✓ completed 2026-01-03
  - File: `src/storage/migrations.rs:186-245`
  - Action: Used `client.transaction()` / `tx.commit()` in `apply_migration()`

- [ ] **CRIT-002**: Handle mutex poisoning in usearch
  - File: `src/storage/vector/usearch.rs:67-89`
  - Action: Use `unwrap_or_else(|p| p.into_inner())` pattern

- [ ] **CRIT-003**: Add MCP tool authorization
  - File: `src/mcp/tools.rs:45-312`
  - Action: Check `auth.has_role()` before executing tools

- [ ] **CRIT-004**: Sanitize memory content before injection
  - File: `src/hooks/user_prompt.rs:134-178`
  - Action: Add `sanitize_for_context()` function to strip injection patterns

- [ ] **CRIT-005**: Implement encryption at rest
  - File: `src/storage/persistence/filesystem.rs:23-89`
  - Action: Add AES-256-GCM encryption with key from env/secrets manager

- [ ] **CRIT-006**: Add service-layer authorization
  - File: `src/services/mod.rs:156-234`
  - Action: Pass `AuthContext` to service methods, check permissions

- [ ] **CRIT-007**: Add timeouts to git operations
  - File: `src/git/notes.rs:34-89`, `src/git/remote.rs:23-67`
  - Action: Wrap git2 calls in `tokio::time::timeout(Duration::from_secs(30), ...)`

---

## High - Security (6)

- [ ] **HIGH-SEC-001**: Remove API keys from debug logs
  - File: `src/llm/anthropic.rs:67`
  - Action: Mask API key in debug output

- [ ] **HIGH-SEC-002**: Fix RSA timing vulnerability
  - File: `src/mcp/auth.rs:89-112`
  - Action: Use constant-time comparison for JWT signature

- [ ] **HIGH-SEC-003**: Add rate limiting to auth endpoints
  - File: `src/mcp/server.rs:145`
  - Action: Add tower rate limit middleware

- [ ] **HIGH-SEC-004**: Strengthen JWT secret entropy validation
  - File: `src/mcp/auth.rs:34-45`
  - Action: Require mixed case, numbers, special chars

- [ ] **HIGH-SEC-005**: Parameterize SQL LIKE queries
  - File: `src/storage/index/sqlite.rs:234`
  - Action: Use `$1` parameter instead of string interpolation

- [ ] **HIGH-SEC-006**: Add CORS configuration
  - File: `src/mcp/server.rs:89`
  - Action: Add tower-http CORS layer with explicit origins

---

## High - Performance (8)

- [ ] **HIGH-PERF-001**: Bound HashMap in ConsolidationService
  - File: `src/services/consolidation.rs:45`
  - Action: Use LRU cache with max size

- [ ] **HIGH-PERF-002**: Fix N+1 query in PostgreSQL
  - File: `src/storage/persistence/postgresql.rs:178`
  - Action: Use batch SELECT with IN clause

- [ ] **HIGH-PERF-003**: Make git operations non-blocking
  - File: `src/git/notes.rs:112`
  - Action: Use `spawn_blocking` for git2 calls

- [ ] **HIGH-PERF-004**: Add Redis connection pooling
  - File: `src/storage/index/redis.rs:34`
  - Action: Use `bb8-redis` connection pool

- [ ] **HIGH-PERF-005**: Add index on namespace column
  - File: `src/storage/index/sqlite.rs:156`
  - Action: `CREATE INDEX idx_namespace ON memories(namespace)`

- [ ] **HIGH-PERF-006**: Batch embedding generation
  - File: `src/embedding/fastembed.rs:78`
  - Action: Use `embed_batch()` instead of per-item calls

- [ ] **HIGH-PERF-007**: Reduce allocations in search
  - File: `src/services/recall.rs:89`
  - Action: Use references/iterators instead of cloning

- [ ] **HIGH-PERF-008**: Cache frequently accessed prompts
  - File: `src/services/prompt.rs:67`
  - Action: Add in-memory LRU cache with TTL

---

## High - Architecture (5)

- [ ] **HIGH-ARCH-001**: Extract ServiceContainer responsibilities
  - File: `src/services/mod.rs:1-787`
  - Action: Split into CaptureModule, RecallModule, StorageModule

- [ ] **HIGH-ARCH-002**: Break circular dependencies
  - Files: `src/services/*.rs`
  - Action: Extract shared types to separate module

- [ ] **HIGH-ARCH-003**: Clarify storage layer boundaries
  - File: `src/storage/mod.rs`
  - Action: Document and enforce layer contracts

- [ ] **HIGH-ARCH-004**: Decouple hooks from services
  - Files: `src/hooks/*.rs`
  - Action: Use dependency injection for services

- [ ] **HIGH-ARCH-005**: Consolidate configuration
  - Files: `src/config/*.rs`
  - Action: Single Config struct with validation

---

## High - Code Quality (10)

- [ ] **HIGH-QUAL-001**: Deduplicate YAML parsing
  - Files: `src/git/parser.rs`, `src/services/prompt_parser.rs`
  - Action: Extract shared `YamlFrontMatterParser`

- [ ] **HIGH-QUAL-002**: Split handle_tool_call function
  - File: `src/mcp/tools.rs:45-325`
  - Action: Extract each tool handler to separate method

- [ ] **HIGH-QUAL-003**: Define constants for magic numbers
  - Multiple files
  - Action: Add `const` definitions with doc comments

- [ ] **HIGH-QUAL-004**: Standardize error handling
  - Files: `src/services/*.rs`
  - Action: Use `?` consistently, remove `unwrap`

- [ ] **HIGH-QUAL-005**: Remove dead LM Studio code
  - File: `src/llm/lmstudio.rs`
  - Action: Delete module if unused, or add feature flag

- [ ] **HIGH-QUAL-006**: Add `#[must_use]` annotations
  - Public functions returning Result/Option
  - Action: Audit and annotate

- [ ] **HIGH-QUAL-007**: Standardize JSON naming
  - Multiple files
  - Action: Use `#[serde(rename_all = "camelCase")]` consistently

- [ ] **HIGH-QUAL-008**: Split MemoryError
  - File: `src/error.rs`
  - Action: Create domain-specific error types

- [ ] **HIGH-QUAL-009**: Reduce cloning in recall
  - File: `src/services/recall.rs`
  - Action: Use `Cow<'_, T>` or references

- [ ] **HIGH-QUAL-010**: Add config validation
  - File: `src/config/mod.rs`
  - Action: Validate at construction, not runtime

---

## High - Test Coverage (4)

- [ ] **HIGH-TEST-001**: Add PII detection tests
  - File: `src/security/pii.rs`
  - Action: Test SSN, credit card, phone, email patterns

- [ ] **HIGH-TEST-002**: Add Git Notes conflict tests
  - File: `src/git/notes.rs`
  - Action: Test merge conflict scenarios

- [ ] **HIGH-TEST-003**: Add error path tests
  - File: `tests/integration_test.rs`
  - Action: Test network failures, invalid input, auth errors

- [ ] **HIGH-TEST-004**: Add property tests for search
  - File: `src/services/recall.rs`
  - Action: Use proptest for fuzzy search edge cases

---

## High - Documentation (6)

- [ ] **HIGH-DOC-001**: Add module documentation
  - 12 modules missing `//!` docs
  - Action: Add overview, examples, and usage notes

- [ ] **HIGH-DOC-002**: Improve CLI help text
  - Files: `src/cli/*.rs`
  - Action: Add examples to each subcommand

- [ ] **HIGH-DOC-003**: Create architecture diagram
  - File: `docs/architecture.md`
  - Action: Add Mermaid diagram of components

- [ ] **HIGH-DOC-004**: Update CLAUDE.md
  - File: `CLAUDE.md:234-289`
  - Action: Remove references to deleted features

- [ ] **HIGH-DOC-005**: Create error code reference
  - File: `docs/errors.md`
  - Action: Document each error variant

- [ ] **HIGH-DOC-006**: Create troubleshooting guide
  - File: `docs/troubleshooting.md`
  - Action: Document common issues and solutions

---

## High - Database (5)

- [ ] **HIGH-DB-001**: Add pool timeouts
  - File: `src/storage/persistence/postgresql.rs:23`
  - Action: Set `acquire_timeout`, `idle_timeout`, `max_lifetime`

- [ ] **HIGH-DB-002**: Add Redis connection pool
  - File: `src/storage/index/redis.rs:34`
  - Action: Use `bb8-redis` or `deadpool-redis`

- [ ] **HIGH-DB-003**: Use prepared statements
  - File: `src/storage/index/sqlite.rs:89`
  - Action: Cache prepared statements per connection

- [ ] **HIGH-DB-004**: Add PostgreSQL indexes
  - File: `src/storage/persistence/postgresql.rs:123`
  - Action: Add indexes for common query patterns

- [ ] **HIGH-DB-005**: Add connection health checks
  - Files: `src/storage/*.rs`
  - Action: Add periodic ping, reconnect on failure

---

## Medium (63) - Planned

### Security (5)
- [ ] MEDIUM-SEC-001: Reduce error verbosity (`src/mcp/server.rs:234`)
- [ ] MEDIUM-SEC-002: Add input length limits (`src/services/capture.rs:45`)
- [ ] MEDIUM-SEC-003: Rotate session tokens (`src/mcp/auth.rs:156`)
- [ ] MEDIUM-SEC-004: Add security headers (`src/mcp/server.rs:89`)
- [ ] MEDIUM-SEC-005: Disable debug endpoints in prod (`src/cli/serve.rs:67`)

### Performance (5)
- [ ] MEDIUM-PERF-001: Use async file I/O (`src/storage/persistence/filesystem.rs`)
- [ ] MEDIUM-PERF-002: Add query result caching (`src/services/recall.rs`)
- [ ] MEDIUM-PERF-003: Optimize string concatenation (`src/hooks/user_prompt.rs`)
- [ ] MEDIUM-PERF-004: Add batch capture operations (`src/services/capture.rs`)
- [ ] MEDIUM-PERF-005: Cache normalized vectors (`src/storage/vector/usearch.rs`)

### Architecture (5)
- [ ] MEDIUM-ARCH-001: Split Config struct (`src/config/mod.rs`)
- [ ] MEDIUM-ARCH-002: Add domain events (`src/services/*.rs`)
- [ ] MEDIUM-ARCH-003: Define aggregate boundaries (`src/models/*.rs`)
- [ ] MEDIUM-ARCH-004: Fix leaky abstractions (`src/storage/traits/*.rs`)
- [ ] MEDIUM-ARCH-005: Make feature flags configurable (`src/config/features.rs`)

### Code Quality (8)
- [ ] MEDIUM-QUAL-001: Track TODO comments (`multiple files`)
- [ ] MEDIUM-QUAL-002: Standardize visibility (`src/services/*.rs`)
- [ ] MEDIUM-QUAL-003: Add Default implementations (`src/models/*.rs`)
- [ ] MEDIUM-QUAL-004: Restrict From implementations (`src/error.rs`)
- [ ] MEDIUM-QUAL-005: Add builder validation (`src/config/mod.rs`)
- [ ] MEDIUM-QUAL-006: Remove unused features (`Cargo.toml`)
- [ ] MEDIUM-QUAL-007: Add serde rename_all (`src/models/*.rs`)
- [ ] MEDIUM-QUAL-008: Standardize Option handling (`src/services/*.rs`)

### Test Coverage (6)
- [ ] MEDIUM-TEST-001: Improve hook branch coverage (`src/hooks/*.rs`)
- [ ] MEDIUM-TEST-002: Add parser fuzzing (`src/git/parser.rs`)
- [ ] MEDIUM-TEST-003: Add concurrent access tests (`src/storage/*.rs`)
- [ ] MEDIUM-TEST-004: Add benchmark regression tests (`benches/*.rs`)
- [ ] MEDIUM-TEST-005: Add snapshot tests (`src/mcp/tools.rs`)
- [ ] MEDIUM-TEST-006: Add MCP contract tests (`src/mcp/*.rs`)

### Documentation (8)
- [ ] MEDIUM-DOC-001: Create API changelog (`docs/`)
- [ ] MEDIUM-DOC-002: Create migration guide (`docs/`)
- [ ] MEDIUM-DOC-003: Add rustdoc examples (`src/lib.rs`)
- [ ] MEDIUM-DOC-004: Create performance guide (`docs/`)
- [ ] MEDIUM-DOC-005: Create security hardening guide (`docs/`)
- [ ] MEDIUM-DOC-006: Update README badges (`README.md`)
- [ ] MEDIUM-DOC-007: Create CONTRIBUTING.md (`./`)
- [ ] MEDIUM-DOC-008: Create release notes template (`docs/`)

### Database (8)
- [ ] MEDIUM-DB-001: Add query logging (`src/storage/*.rs`)
- [ ] MEDIUM-DB-002: Set transaction isolation (`src/storage/persistence/postgresql.rs`)
- [ ] MEDIUM-DB-003: Add dead letter queue (`src/storage/*.rs`)
- [ ] MEDIUM-DB-004: Enable SQLite WAL mode (`src/storage/index/sqlite.rs`)
- [ ] MEDIUM-DB-005: Add connection retry (`src/storage/*.rs`)
- [ ] MEDIUM-DB-006: Add cascade deletes (`src/storage/persistence/postgresql.rs`)
- [ ] MEDIUM-DB-007: Document backup strategy (`docs/`)
- [ ] MEDIUM-DB-008: Version schema migrations (`src/storage/persistence/postgresql.rs`)

### Penetration Testing (6)
- [ ] MEDIUM-PEN-001: Sanitize stack traces (`src/mcp/server.rs`)
- [ ] MEDIUM-PEN-002: Add request ID tracing (`src/mcp/*.rs`)
- [ ] MEDIUM-PEN-003: Log auth failures (`src/mcp/auth.rs`)
- [ ] MEDIUM-PEN-004: Add geo-blocking option (`src/mcp/server.rs`)
- [ ] MEDIUM-PEN-005: Prevent session fixation (`src/mcp/auth.rs`)
- [ ] MEDIUM-PEN-006: Add account lockout (`src/mcp/auth.rs`)

### Compliance (5)
- [ ] MEDIUM-COMP-001: Enforce data retention (`src/services/*.rs`)
- [ ] MEDIUM-COMP-002: Add GDPR deletion cascade (`src/services/capture.rs`)
- [ ] MEDIUM-COMP-003: Add consent tracking (`src/models/memory.rs`)
- [ ] MEDIUM-COMP-004: Make audit logs tamper-evident (`src/security/audit.rs`)
- [ ] MEDIUM-COMP-005: Add data classification (`src/models/*.rs`)

### Chaos Engineering (6)
- [ ] MEDIUM-CHAOS-001: Add storage circuit breakers (`src/storage/*.rs`)
- [ ] MEDIUM-CHAOS-002: Add embedding bulkhead (`src/embedding/*.rs`)
- [ ] MEDIUM-CHAOS-003: Graceful vector search degradation (`src/storage/vector/*.rs`)
- [ ] MEDIUM-CHAOS-004: Fix retry storms (`src/llm/resilience.rs`)
- [ ] MEDIUM-CHAOS-005: Add backpressure (`src/services/*.rs`)
- [ ] MEDIUM-CHAOS-006: Add health check endpoints (`src/mcp/server.rs`)

### Rust Idioms (5)
- [ ] MEDIUM-RUST-001: Use `&str` where possible (`src/models/*.rs`)
- [ ] MEDIUM-RUST-002: Add `#[inline]` on hot paths (`src/services/recall.rs`)
- [ ] MEDIUM-RUST-003: Remove unnecessary Arc (`src/storage/*.rs`)
- [ ] MEDIUM-RUST-004: Use `vec![]` macro (`multiple files`)
- [ ] MEDIUM-RUST-005: Add const fn annotations (`src/config/*.rs`)

### MCP/Claude Code (7)
- [ ] MEDIUM-MCP-001: Improve tool descriptions (`src/mcp/tools.rs`)
- [ ] MEDIUM-MCP-002: Validate resource URNs (`src/mcp/resources.rs`)
- [ ] MEDIUM-MCP-003: Add tool versioning (`src/mcp/tools.rs`)
- [ ] MEDIUM-MCP-004: Add deprecation mechanism (`src/mcp/tools.rs`)
- [ ] MEDIUM-MCP-005: Validate prompt templates (`src/mcp/prompts.rs`)
- [ ] MEDIUM-MCP-006: Add MCP version negotiation (`src/mcp/server.rs`)
- [ ] MEDIUM-MCP-007: Add tool retry guidance (`src/mcp/tools.rs`)

---

## Low (62) - Planned

*Low-priority findings for future improvement. Categories:*
- Style inconsistencies (12)
- Minor documentation gaps (10)
- Optional optimizations (15)
- Nice-to-have features (8)
- Code organization suggestions (17)

---

## Progress Tracking

| Phase | Status | Findings | Fixed |
|-------|--------|----------|-------|
| Critical | Pending | 7 | 0 |
| High | Pending | 44 | 0 |
| Medium | Pending | 63 | 0 |
| Low | Pending | 62 | 0 |
| **Total** | | **176** | **0** |

---

*Generated by MAX Code Review - 12 Specialist Agents*
