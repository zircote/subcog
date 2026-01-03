# Issue #45: Git Notes Removal & SQLite Consolidation

**Issue**: [#45](https://github.com/zircote/subcog/issues/45) - Storage backend config ignored
**Branch**: `plan/storage-simplification`
**Root Cause**: `CaptureService` still writes to git notes despite storage simplification spec claiming removal

---

## Phase 1: CaptureService SQLite Migration

- [x] `src/services/capture.rs:193-216` - Replace git notes write with SQLite index write ✓
- [x] `src/services/capture.rs` - Generate memory ID from UUID instead of git note SHA ✓
- [x] `src/services/capture.rs` - Remove `NotesManager` import ✓
- [ ] `src/services/capture.rs` - Ensure `IndexBackend::insert()` is called for every capture
- [ ] `src/services/capture.rs` - Update tests to verify SQLite-only storage

## Phase 2: RecallService SQLite Verification

- [ ] `src/services/recall.rs` - Verify reads from SQLite `IndexBackend` only
- [ ] `src/services/recall.rs` - Remove any git notes fallback logic if present
- [ ] `src/services/mod.rs` - Verify `ServiceContainer` passes SQLite backend to services

## Phase 3: Git Notes Module Deletion

- [ ] DELETE `src/git/notes.rs`
- [ ] `src/git/mod.rs` - Remove `pub mod notes` export
- [ ] `src/git/remote.rs` - Remove notes-specific sync logic (keep branch/context detection)

## Phase 4: Storage Persistence Layer Cleanup

- [ ] DELETE `src/storage/persistence/git_notes.rs`
- [ ] `src/storage/persistence/mod.rs` - Remove `GitNotesBackend` export
- [ ] `src/storage/mod.rs` - Remove GitNotes references
- [ ] `src/storage/traits/persistence.rs` - Remove GitNotes from documentation
- [ ] `src/storage/resilience.rs` - Remove GitNotes references

## Phase 5: Prompt Storage Migration

- [ ] DELETE `src/storage/prompt/git_notes.rs`
- [ ] `src/storage/prompt/mod.rs` - Remove git_notes module export
- [ ] `src/services/prompt.rs` - Remove GitNotes prompt backend, ensure SQLite-only
- [ ] `src/services/prompt.rs` - Verify `SqlitePromptStore` is used exclusively

## Phase 6: Config Cleanup

- [ ] `src/config/mod.rs` - Remove `StorageBackendType::GitNotes` enum variant
- [ ] `src/config/mod.rs` - Remove "git_notes" from `StorageBackendType::parse()`
- [ ] `src/config/mod.rs` - Remove `StorageConfig.project` field (consolidate to `user`)
- [ ] `example.config.toml` - Remove `[storage.project]` section
- [ ] `example.config.toml` - Document SQLite + facets architecture

## Phase 7: Commands Update

- [ ] `src/commands/core.rs` - Remove `StorageBackendType::GitNotes` match arm
- [ ] `src/commands/core.rs` - Update consolidate command to SQLite-only
- [ ] `src/commands/config.rs` - Remove GitNotes display logic
- [ ] `src/commands/config.rs` - Show SQLite path and facet info instead

## Phase 8: Services Cleanup

- [ ] `src/services/mod.rs` - Remove GitNotes references
- [ ] `src/services/data_subject.rs` - Remove GitNotes references
- [ ] `src/services/enrichment.rs` - Remove GitNotes references
- [ ] `src/services/sync.rs` - Update sync to work with SQLite (or remove if not needed)

## Phase 9: Documentation

- [ ] `README.md` - Update storage architecture section to SQLite + facets
- [ ] `CLAUDE.md` - Update storage documentation
- [ ] `commands/sync.md` - Update or deprecate based on new architecture
- [ ] Update completed spec `docs/spec/completed/2026-01-03-storage-simplification/` to reflect actual state

## Phase 10: Verification

- [ ] `make ci` passes (format, lint-strict, test, doc, deny, msrv, bench)
- [ ] `subcog capture` writes ONLY to SQLite
- [ ] `subcog recall` reads ONLY from SQLite
- [ ] `subcog status` shows SQLite database info
- [ ] No `refs/notes/subcog` created on new captures
- [ ] Close Issue #45 with PR reference

---
<!-- BEGIN deep-clean findings -->

## Phase 11: CRITICAL Security Fixes

- [ ] `src/storage/prompt/postgresql.rs:192-282` - SQL injection via dynamic table name in migrations - validate/sanitize table names
- [ ] `src/security/audit.rs:326-347` - TOCTOU race condition on file permission setting - use atomic file creation with proper mode
- [ ] `src/mcp/auth.rs:167-187` - Authorization bypass for unknown tool names falls through to default scope - explicit deny for unknown tools
- [ ] `src/mcp/auth.rs:89-102` - JWT secret entropy validation missing - enforce minimum 32 bytes with character class requirements
- [ ] `src/storage/index/sqlite.rs:89-102` - Unbounded LRU cache memory exhaustion - add maximum size limit with eviction
- [ ] `src/hooks/search_intent/hybrid.rs:105-157` - Thread spawning without graceful cancellation causes resource leaks - use async/await with cancellation tokens

## Phase 12: HIGH Security Fixes

- [ ] `src/security/secrets.rs:31-114` - Missing patterns for GCP/Azure credentials, Slack tokens, Twilio keys - add comprehensive cloud provider patterns
- [ ] `src/security/pii.rs:45-89` - No international SSN/tax ID formats - add EU VAT, UK NIN, CA SIN patterns
- [ ] `src/mcp/server.rs:156-189` - HTTP transport lacks rate limiting - implement per-client rate limits
- [ ] `src/services/prompt.rs:234-267` - Template injection via variable expansion - sanitize user-provided variable values
- [ ] `src/storage/prompt/sqlite.rs:110-146` - Missing WAL mode and pragmas (unlike main SqliteBackend) - add WAL/busy_timeout/synchronous
- [ ] `src/services/recall.rs:178,266,529,544` - String clones in search hit recording loop - use references or Cow<str>
- [ ] `src/services/deduplication/recent.rs:129,230,266` - RwLock poisoning risk - migrate to parking_lot::RwLock
- [ ] `src/security/audit.rs:89-134` - Audit log integrity not cryptographically verified - add HMAC chain or append-only signing

## Phase 13: HIGH Performance Fixes

- [ ] `src/storage/index/sqlite.rs:134-139` - Single Mutex serializes all SQLite operations - implement connection pool (r2d2 or deadpool)
- [ ] `src/services/recall.rs:312-345` - SearchHit clone in RRF fusion includes embedding vectors - use Arc<SearchHit> or indices
- [ ] `src/embedding/fastembed.rs:67-89` - Model loaded synchronously on first embed - lazy init with tokio::spawn_blocking
- [ ] `src/hooks/search_intent/keyword.rs:50,154-155` - Redundant string clones in keyword matching - use &str or Cow<'static, str>
- [ ] `src/services/context.rs:280-285` - truncate_content() always allocates - return Cow<'_, str> for zero-copy when no truncation needed

## Phase 14: HIGH Architecture Fixes

- [ ] `src/storage/mod.rs` - CompositeStorage mixes persistence/index/vector concerns - split into separate coordinator types
- [ ] `src/services/mod.rs:45-89` - ServiceContainer God object anti-pattern - break into domain-specific factories
- [ ] `src/mcp/tools/handlers/` - Tool handlers have inconsistent error handling - standardize Result<ToolResult, Error> pattern
- [ ] `src/config/mod.rs:234-289` - Config validation scattered across modules - centralize with builder pattern and fail-fast
- [ ] `src/lib.rs:105-110` - Error::OperationFailed uses String for cause - use Box<dyn Error + Send + Sync> for source chain

## Phase 15: HIGH Database Fixes

- [ ] `src/storage/index/sqlite.rs:178-234` - Missing indexes on (namespace, created_at), (source, status) - add compound indexes
- [ ] `src/storage/prompt/sqlite.rs:89-123` - No VACUUM/ANALYZE scheduled - add maintenance commands to status/admin
- [ ] `src/storage/index/sqlite.rs:267-312` - FTS5 queries vulnerable to syntax injection - escape special characters
- [ ] `src/storage/persistence/postgresql.rs:156-189` - Migrations run on every startup - add migration version tracking

## Phase 16: HIGH Test Coverage Gaps

- [ ] `src/mcp/auth.rs` - 0% test coverage for JWT authentication - add unit tests for token validation, expiry, scopes
- [ ] `src/security/audit.rs` - No tests for audit log rotation, integrity, GDPR compliance - add comprehensive test suite
- [ ] `src/hooks/search_intent/llm.rs` - No tests for LLM classifier timeout/fallback paths - add timeout simulation tests
- [ ] `src/storage/persistence/postgresql.rs` - No integration tests - add testcontainers PostgreSQL tests
- [ ] `src/storage/index/sqlite.rs` - Missing concurrent access tests - add multi-threaded stress tests
- [ ] `src/services/deduplication/` - Missing edge case tests for hash collisions, cache eviction - add property-based tests
- [ ] `src/embedding/fastembed.rs` - No tests for model loading failures - add mock/stub tests
- [ ] `src/mcp/server.rs` - No end-to-end MCP protocol tests - add JSON-RPC roundtrip tests
- [ ] `src/hooks/` - Session lifecycle not tested (start→prompt→tool→stop) - add integration test
- [ ] `src/services/consolidation.rs` - No tests for edge merging, tier transitions - add state machine tests
- [ ] `src/security/redactor.rs` - No tests for partial redaction, format preservation - add property tests
- [ ] `src/services/sync.rs` - No tests for conflict resolution, partial sync - add scenario tests

## Phase 17: MEDIUM Security Fixes

- [ ] `src/llm/anthropic.rs:89-134` - API keys in memory not zeroized on drop - use secrecy::Secret<String>
- [ ] `src/config/mod.rs:45-67` - Config file permissions not validated on load - warn if world-readable
- [ ] `src/mcp/tools/handlers/core.rs:178-223` - Input length not validated before processing - add MAX_INPUT_LENGTH check
- [ ] `src/hooks/user_prompt.rs:89-134` - User prompt content logged at debug level - redact or remove sensitive logging
- [ ] `src/storage/prompt/filesystem.rs:67-89` - Path traversal possible via prompt names - validate/sanitize file paths

## Phase 18: MEDIUM Performance Fixes

- [ ] `src/hooks/search_intent/hybrid.rs:89-134` - LLM timeout (200ms) may be too aggressive - make configurable, add backoff
- [ ] `src/services/topic_index.rs:156-189` - Topic index rebuilt from scratch on updates - implement incremental updates
- [ ] `src/storage/vector/usearch.rs:89-123` - Index not memory-mapped for large datasets - add mmap support
- [ ] `src/embedding/mod.rs:45-67` - Embeddings computed synchronously - batch and parallelize with rayon
- [ ] `src/services/recall.rs:234-267` - BM25 scoring computed per-query - cache IDF values
- [ ] `src/hooks/session_start.rs:67-89` - Context loading blocks session start - make async with timeout

## Phase 19: MEDIUM Architecture Fixes

- [ ] `src/models/memory.rs:45-89` - Memory struct has 15+ fields - split into MemoryContent, MemoryMetadata, MemoryState
- [ ] `src/services/capture.rs:123-167` - Capture validation logic duplicated - extract CaptureValidator trait
- [ ] `src/hooks/mod.rs:34-67` - Hook handlers tightly coupled to services - introduce HookContext abstraction
- [ ] `src/mcp/resources.rs:89-134` - URN parsing duplicated across handlers - centralize in UrnParser
- [ ] `src/llm/mod.rs:156-189` - LLM provider selection uses string matching - use enum dispatch
- [ ] `src/services/deduplication/config.rs:217-256` - Builder methods marked const with mut self - remove const from mutating builders

## Phase 20: MEDIUM Code Quality Fixes

- [ ] `src/services/context.rs:289-293` - add_topic_if_unique uses O(n) contains check - use HashSet for deduplication
- [ ] `src/hooks/search_intent/keyword.rs:97-98` - cast_precision_loss suppressed - use explicit .min(0.95) guard
- [ ] `src/models/domain.rs:89-134` - Namespace::from_str duplicates Display logic - derive with strum
- [ ] `src/services/prompt_parser.rs:156-189` - Parser has 8 match arms for formats - use serde untagged enum
- [ ] `src/commands/core.rs:234-267` - Command handlers exceed 100 lines - extract to dedicated modules
- [ ] `src/storage/traits/persistence.rs:45-89` - Trait has 12 methods - split into read/write/admin traits
- [ ] `src/services/deduplication/recent.rs:78-79` - expect() panics on invalid capacity - return Result instead

## Phase 21: MEDIUM Database Fixes

- [ ] `src/storage/index/sqlite.rs:312-345` - No prepared statement caching - use rusqlite::CachedStatement
- [ ] `src/storage/prompt/sqlite.rs:178-212` - Prompts table missing updated_at trigger - add ON UPDATE CURRENT_TIMESTAMP
- [ ] `src/storage/persistence/postgresql.rs:234-267` - Connection pool sizing hardcoded - make configurable via StorageConfig
- [ ] `src/storage/index/sqlite.rs:389-423` - Bulk insert uses individual statements - use INSERT...VALUES batching

## Phase 22: MEDIUM Compliance Fixes

- [ ] `src/services/data_subject.rs:89-134` - GDPR right-to-erasure incomplete - implement cascading delete across all stores
- [ ] `src/security/audit.rs:234-267` - Audit retention policy not enforced - add automatic purge after retention period
- [ ] `src/mcp/tools/handlers/core.rs:312-345` - Data export (GDPR Art. 20) not implemented - add export command
- [ ] `src/config/mod.rs:312-345` - No consent management for LLM data sharing - add explicit opt-in config
- [ ] `src/security/pii.rs:134-167` - PII detection results not logged for audit - add structured audit events

## Phase 23: MEDIUM Chaos/Resilience Fixes

- [ ] `src/storage/mod.rs:89-123` - No circuit breaker for storage backends - implement with tokio-retry and exponential backoff
- [ ] `src/embedding/fastembed.rs:134-167` - ONNX runtime crashes not caught - wrap in catch_unwind for graceful degradation
- [ ] `src/services/sync.rs:89-134` - No conflict resolution for concurrent syncs - implement last-writer-wins or merge
- [ ] `src/llm/mod.rs:234-267` - LLM fallback chain not configurable - allow ordered provider list
- [ ] `src/hooks/stop.rs:67-89` - Stop hook has no timeout - add 30s deadline with force exit
- [ ] `src/storage/index/sqlite.rs:456-489` - No WAL checkpoint management - add periodic checkpointing
- [ ] `src/mcp/server.rs:234-267` - No graceful shutdown signal handling - implement SIGTERM handler
- [ ] `src/services/recall.rs:456-489` - Search timeout not enforced - add query deadline

## Phase 24: MEDIUM Rust Idiom Fixes

- [ ] `src/llm/mod.rs:346-349` - build_http_client fallback silently hides errors - add tracing::warn
- [ ] `src/services/deduplication/semantic.rs:51,69` - Generic bounds repeated everywhere - use trait alias pattern
- [ ] `src/llm/mod.rs:357-379` - Unnecessary string allocation in error paths - use Cow<str>
- [ ] `src/services/prompt.rs:312-345` - Ownership transfer in prompt operations - prefer &PromptTemplate over owned

## Phase 25: MEDIUM Dependency Fixes

- [ ] `Cargo.toml` - base64 duplication (0.13.1 + 0.22.1) via fastembed→tokenizers - upgrade tokenizers or add bans.skip
- [ ] `Cargo.toml` - fastembed in default features causes 29MB binary - move to opt-in `full` feature
- [ ] `Cargo.toml` - ort v2.0.0-rc.9 is pre-release - monitor for stable v2.0.0 and upgrade
- [ ] `Cargo.toml` - reqwest rustls-tls feature obsolete in v0.13+ - upgrade when available
- [ ] `deny.toml` - RUSTSEC-2023-0071 (rsa timing attack) ignored - document in THREAT_MODEL.md

## Phase 26: LOW Documentation Fixes

- [ ] `src/lib.rs` - Module-level docs missing for 8 modules - add //! documentation
- [ ] `src/mcp/tools/handlers/` - Tool handlers missing # Examples sections - add usage examples
- [ ] `src/services/deduplication/` - DeduplicationService API not documented - add comprehensive rustdoc
- [ ] `src/hooks/search_intent/` - SearchIntentDetector internals undocumented - add algorithm explanation
- [ ] `src/storage/traits/` - Trait contracts not documented - add invariants and guarantees
- [ ] `README.md` - Performance targets not documented - add latency/throughput expectations
- [ ] `CLAUDE.md` - Hook response format not fully documented - add JSON schema
- [ ] `docs/` - No architecture decision records for storage changes - create ADRs

## Phase 27: LOW Code Quality Fixes

- [ ] `src/models/prompt.rs:89-134` - PromptTemplate validation in multiple places - centralize in impl block
- [ ] `src/services/enrichment.rs:156-189` - Magic numbers for enrichment thresholds - extract to named constants
- [ ] `src/commands/prompt.rs:234-267` - CLI output formatting inconsistent - use consistent table/JSON format
- [ ] `src/hooks/pre_compact.rs:89-123` - Compact detection heuristics hardcoded - make configurable
- [ ] `src/config/features.rs:45-67` - Feature flag defaults scattered - centralize in FeatureDefaults struct

## Phase 28: LOW Test Improvements

- [ ] `tests/` - No fuzz testing for parsers - add cargo-fuzz targets for YAML/JSON parsing
- [ ] `tests/` - No load testing for MCP server - add criterion benchmarks for RPC throughput
- [ ] `tests/` - No chaos testing for storage failures - add proptest with fault injection
- [ ] `benches/` - search_intent benchmark only tests keywords - add LLM path benchmarks
- [ ] `tests/` - Integration tests use real git - add mock git for faster CI

## Phase 29: LOW Chaos/Resilience Improvements

- [ ] `src/storage/` - No health check endpoints - add /health for each backend
- [ ] `src/mcp/server.rs` - No connection draining on shutdown - implement graceful drain
- [ ] `src/services/` - No bulkhead isolation between services - consider tokio::task budget
- [ ] `src/observability/` - No distributed tracing correlation - add trace_id propagation

## Phase 30: CI/Dependency Fixes

- [ ] `.github/workflows/ci.yml` - MSRV check failing with zune-jpeg 0.5.7 (unsafe AVX2 code) - pin or exclude from MSRV
- [ ] `Cargo.toml` - Add quarterly dependency audit schedule - document in CLAUDE.md
- [ ] `deny.toml` - Add pre-release version warnings - warn on rc/alpha/beta deps

<!-- END deep-clean findings -->

**Total: 42 original + 123 deep-clean = 165 tasks across 30 phases**

**Started**: 2026-01-03
**Status**: Not Started
**Deep-Clean Date**: 2026-01-03
**Agents**: Security Analyst, Performance Engineer, Architecture Reviewer, Code Quality Analyst, Test Coverage Analyst, Documentation Reviewer, Database Expert, Penetration Tester, Compliance Auditor, Chaos Engineer, Rust Idioms Expert, Dependency Auditor
