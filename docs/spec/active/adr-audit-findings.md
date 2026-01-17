# ADR Audit Remediation Plan

**Date Created:** 2026-01-04
**Audit Date:** 2026-01-04
**Status:** PENDING
**Target Completion:** 2026-01-11

---

## Executive Summary

**Total Tasks:** 15 tasks across 5 phases
**Estimated Effort:** 16-24 hours
**Critical Path:** Phase 1 (Tombstone) -> Phase 2 (Lazy GC) -> Phase 5 (Verification)
**Overall Compliance Before:** 84.5% (49/58 ADRs)
**Target Compliance After:** 100% (with documented deviations)

---

## Critical Issues to Resolve

### CRITICAL: ADR-0053 - Tombstone Pattern
- **Impact:** Soft-delete functionality broken, lazy GC blocked
- **Files:** `src/models/domain.rs`, `src/models/memory.rs`, `src/storage/persistence/*`
- **Effort:** 8-12 hours

### 🟠 HIGH: ADR-0051 - Org-Scope Feature Gate
- **Impact:** Cannot toggle org-scope at runtime
- **File:** `src/config/features.rs`
- **Effort:** 2-3 hours

### 🟡 MEDIUM: ADR-0052 - Lazy Branch GC
- **Impact:** Blocked by ADR-0053
- **File:** `src/gc/branch.rs`
- **Effort:** 2-3 hours (after ADR-0053 complete)

### 🟢 LOW: ADR-0009 - MCP Server Documentation
- **Impact:** None (working correctly, just needs documentation)
- **Effort:** 1-2 hours

---

## Phase 1: Critical - Tombstone Pattern Implementation (ADR-0053)

**Priority:** CRITICAL
**Estimated Effort:** 8-12 hours
**Dependencies:** None
**Blocks:** ADR-0052, lazy branch GC functionality

### Task 1.1: Add Tombstoned Variant to MemoryStatus Enum
**Estimated Time:** 30 minutes
**File:** `src/models/domain.rs` (around line 260-295)

- [x] Add `Tombstoned` variant to MemoryStatus enum 
- [x] Update `as_str()` method to handle Tombstoned case 
- [x] Update `from_str()` method to parse "tombstoned" 
- [x] Verify serde serialization works correctly 
- [x] Ensure enum ordering doesn't break existing functionality 
- [x] Run unit tests for MemoryStatus 

**Code Change:**
```rust
pub enum MemoryStatus {
 Active,
 Archived,
 Superseded,
 Pending,
 Deleted,
 Tombstoned, // ADD THIS
}
```

---

### Task 1.2: Add tombstoned_at Field to Memory Struct
**Estimated Time:** 1 hour
**File:** `src/models/memory.rs` (around line 43-66)

- [x] Add `pub tombstoned_at: Option<DateTime<Utc>>` field to Memory struct 
- [x] Configure serde attributes for JSON serialization 
- [x] Ensure default value is `None` for new memories 
- [x] Update getter/setter methods if they exist 
- [x] Update all Memory constructors to include new field 
- [x] Update Display/Debug implementations if needed 
- [x] Run unit tests for Memory struct 

**Code Change:**
```rust
pub struct Memory {
 pub id: String,
 pub content: String,
 pub namespace: Namespace,
 pub domain: Domain,
 pub status: MemoryStatus,
 pub created_at: DateTime<Utc>,
 pub updated_at: DateTime<Utc>,
 pub tombstoned_at: Option<DateTime<Utc>>, // ADD THIS
 pub embedding: Option<Vec<f32>>,
 pub tags: Vec<String>,
 pub source: Option<String>,
}
```

---

### Task 1.3: Create SQLite Migration for tombstoned_at Column
**Estimated Time:** 2 hours
**File:** `src/storage/migrations/sqlite/` (new migration file)

- [x] Create new migration file with proper naming (e.g., `0004_add_tombstoned_at.sql`) 
- [x] Add `ALTER TABLE memories ADD COLUMN tombstoned_at TIMESTAMP NULL` 
- [x] Create partial index: `CREATE INDEX idx_memories_tombstoned ON memories(tombstoned_at) WHERE tombstoned_at IS NOT NULL` 
- [x] Add migration to migration runner 
- [x] Create rollback migration 
- [x] Test migration on fresh database 
- [x] Test migration on database with existing data 
- [x] Verify migration is idempotent (safe to run multiple times) 

**Migration SQL:**
```sql
-- Migration: 0004_add_tombstoned_at.sql
ALTER TABLE memories ADD COLUMN tombstoned_at TIMESTAMP NULL;
CREATE INDEX idx_memories_tombstoned ON memories(tombstoned_at) WHERE tombstoned_at IS NOT NULL;
```

---

### Task 1.4: Update PostgreSQL Schema (if applicable)
**Estimated Time:** 1 hour
**File:** `src/storage/migrations/postgresql/` (new migration file)

- [x] Create PostgreSQL migration file 
- [x] Add `ALTER TABLE memories ADD COLUMN tombstoned_at TIMESTAMPTZ NULL` 
- [x] Create partial index for performance 
- [x] Test migration on PostgreSQL instance 
- [x] Verify timezone handling works correctly 

**Migration SQL:**
```sql
-- Migration: 0004_add_tombstoned_at.sql
ALTER TABLE memories ADD COLUMN tombstoned_at TIMESTAMPTZ NULL;
CREATE INDEX idx_memories_tombstoned ON memories(tombstoned_at) WHERE tombstoned_at IS NOT NULL;
```

---

### Task 1.5: Update Memory CRUD Operations
**Estimated Time:** 3 hours
**Files:**
- `src/storage/persistence/sqlite.rs`
- `src/storage/persistence/postgresql.rs`
- `src/storage/persistence/filesystem.rs`

- [x] Update SQLite INSERT statements to include tombstoned_at 
- [x] Update SQLite SELECT statements to fetch tombstoned_at 
- [x] Update PostgreSQL INSERT statements 
- [x] Update PostgreSQL SELECT statements 
- [x] Update filesystem persistence serialization 
- [x] Update memory deserialization to handle NULL tombstoned_at 
- [x] Verify all persistence backends handle new field 
- [x] Run existing persistence tests 
- [x] Add new tests for tombstoned memories 

---

### Task 1.6: Implement Tombstone Operations
**Estimated Time:** 2-3 hours
**File:** `src/services/mod.rs` or create `src/services/tombstone.rs`

- [x] Implement `tombstone_memory(&self, id: &str) -> Result<(), MemoryError>` 
 - [x] Set status to MemoryStatus::Tombstoned 
 - [x] Set tombstoned_at to Utc::now() 
 - [x] Update in persistence layer 
 - [x] Use transaction for atomicity 
- [x] Implement `untombstone_memory(&self, id: &str) -> Result<(), MemoryError>` 
 - [x] Set status back to Active 
 - [x] Clear tombstoned_at (set to None) 
 - [x] Update in persistence layer 
- [x] Implement `purge_tombstoned(&self, older_than: Duration) -> Result<usize, MemoryError>` 
 - [x] Query tombstoned memories older than threshold 
 - [x] Permanently delete from persistence 
 - [x] Return count of purged memories 
- [x] Add error handling for not found / already tombstoned cases 
- [x] Write unit tests for all three operations 
- [x] Write integration tests 

---

### Task 1.7: Update RecallService to Filter Tombstoned by Default
**Estimated Time:** 1-2 hours
**File:** `src/services/recall.rs`

- [x] Add `WHERE status!= 'Tombstoned'` to default SQL queries 
- [x] Add `include_tombstoned: bool` parameter to search methods 
- [x] Update RecallService API to support include_tombstoned flag 
- [x] Update CLI to support `--include-tombstoned` flag 
- [x] Update MCP tools to support tombstone filtering 
- [x] Verify existing search tests still pass 
- [x] Add new tests for tombstone filtering 
- [x] Test that tombstoned memories are hidden by default 
- [x] Test that --include-tombstoned shows tombstoned memories 

---

## Phase 2: High Priority - Lazy Branch GC (ADR-0052)

**Priority:** 🟠 HIGH
**Estimated Effort:** 2-3 hours
**Dependencies:** Phase 1 complete

### Task 2.1: Verify BranchGarbageCollector Works with Tombstone
**Estimated Time:** 2 hours
**File:** `src/gc/branch.rs`

- [x] Run existing GC test suite 
- [x] Fix compilation errors from new tombstoned_at field 
- [x] Verify tombstone_memory() is called correctly 
- [x] Verify status check works: `memory.status!= MemoryStatus::Tombstoned` 
- [x] Verify tombstoned_at is set correctly 
- [x] Test branch deletion triggers tombstoning 
- [x] Test lazy GC during recall works 
- [x] Test 5-minute branch existence cache 
- [x] Test dry-run mode shows what would be tombstoned 
- [x] Fix any broken tests 

---

### Task 2.2: Add End-to-End GC Integration Test
**Estimated Time:** 1 hour
**File:** `tests/gc_integration_test.rs` (new file)

- [x] Create new integration test file 
- [x] Test: Create memory for branch "feature/test" 
- [x] Test: Delete branch "feature/test" 
- [x] Test: Recall memories (should trigger lazy GC) 
- [x] Test: Verify memory status is Tombstoned 
- [x] Test: Verify memory not in default results 
- [x] Test: Recall with --include-tombstoned 
- [x] Test: Verify memory appears in results 
- [x] Test: Run purge operation 
- [x] Test: Verify memory permanently deleted 
- [x] Ensure test cleans up after itself 

**Test Code Template:**
```rust
#[tokio::test]
async fn test_lazy_gc_flow() {
 // 1. Setup: Create memory for branch "feature/test"
 // 2. Action: Delete branch
 // 3. Action: Recall memories (triggers lazy GC)
 // 4. Assert: Memory is tombstoned
 // 5. Assert: Memory not in default results
 // 6. Action: Recall with --include-tombstoned
 // 7. Assert: Memory appears in results
}
```

---

## Phase 3: Medium Priority - Org-Scope Feature Gate (ADR-0051)

**Priority:** 🟡 MEDIUM
**Estimated Effort:** 2-3 hours
**Dependencies:** None

### Task 3.1: Add org_scope_enabled to FeatureFlags
**Estimated Time:** 30 minutes
**File:** `src/config/features.rs`

- [x] Add `pub org_scope_enabled: bool` field to FeatureFlags struct 
- [x] Set default to `false` in Default implementation 
- [x] Add serde attributes for serialization 
- [x] Update FeatureFlags::default() method 
- [x] Update any FeatureFlags builders/constructors 
- [x] Run tests to ensure no breakage 

**Code Change:**
```rust
pub struct FeatureFlags {
 pub secrets_filter: bool,
 pub pii_filter: bool,
 pub multi_domain: bool,
 pub audit_log: bool,
 pub llm_features: bool,
 pub auto_capture: bool,
 pub consolidation: bool,
 pub org_scope_enabled: bool, // ADD THIS
}

impl Default for FeatureFlags {
 fn default() -> Self {
 Self {
 //... existing defaults
 org_scope_enabled: false, // ADD THIS
 }
 }
}
```

---

### Task 3.2: Add Environment Variable Support
**Estimated Time:** 30 minutes
**File:** `src/config/mod.rs`

- [x] Add environment variable loading for SUBCOG_ORG_SCOPE_ENABLED 
- [x] Support boolean parsing (true/false, 1/0, yes/no, on/off) 
- [x] Handle invalid values gracefully (default to false) 
- [x] Add logging when org-scope enabled via env var 
- [x] Test environment variable loading 
- [x] Document env var in code comments 

**Code Change:**
```rust
// Load from environment
if let Ok(val) = env::var("SUBCOG_ORG_SCOPE_ENABLED") {
 features.org_scope_enabled = parse_bool(&val).unwrap_or(false);
}
```

---

### Task 3.3: Wire Up Feature Check (if org-scope code exists)
**Estimated Time:** 1 hour
**Files:** Search codebase for org-scope initialization

- [x] Search codebase for org-scope initialization code 
- [x] Add feature flag check before org-scope initialization 
- [x] Add warning log if org-scope disabled but PostgreSQL configured 
- [x] Add error message explaining how to enable org-scope 
- [x] Verify org-scope only initializes when flag is true 
- [x] Test that org-scope is disabled by default 
- [x] Test that org-scope can be enabled via env var 

---

### Task 3.4: Document Org-Scope Configuration
**Estimated Time:** 1 hour
**Files:**
- `CLAUDE.md`
- `README.md` (if applicable)
- `docs/configuration.md` (if exists)

- [x] Document org_scope_enabled feature flag in CLAUDE.md 
- [x] Document SUBCOG_ORG_SCOPE_ENABLED environment variable 
- [x] Document when to enable org-scope (multi-team scenarios) 
- [x] Document PostgreSQL requirements for org-scope 
- [x] Provide configuration example 
- [x] Document security considerations 
- [x] Add troubleshooting section 

**Documentation Template:**
```markdown
### Org-Scope Configuration

Org-scope enables shared memory storage across teams using PostgreSQL.

**Enable via environment variable:**
```bash
export SUBCOG_ORG_SCOPE_ENABLED=true
```

**Enable via config file:**
```toml
[features]
org_scope_enabled = true
```

**Requirements:**
- PostgreSQL connection configured
- Shared database accessible by all team members
- Proper access controls in place
```

---

## Phase 4: Documentation - ADR-0009 Deviation

**Priority:** 🟢 LOW
**Estimated Effort:** 1-2 hours
**Dependencies:** None

### Task 4.1: Document MCP Server Implementation Choice
**Estimated Time:** 1 hour

**Choose One Option:**

#### Option A: Create New ADR (Recommended)
**File:** `docs/adrs/adr_0059.md`

- [x] Create new ADR file: adr_0059.md 
- [x] Add YAML frontmatter 
- [x] Document that this supersedes ADR-0009 
- [x] Explain why a bespoke MCP server was chosen 
- [x] Reference ADRs 0054-0058 for notification compliance 
- [x] Document benefits of custom implementation 
- [x] Update README.md to show ADR-0009 as superseded 
- [x] Update ADR-0009 to reference ADR-0059 

**OR**

#### Option B: Amend ADR-0009
**File:** `docs/adrs/adr_0009.md`

- [x] Add "Implementation Update" section after audit section 
- [x] Explain deviation from original decision 
- [x] Document rationale for bespoke MCP server implementation 
- [x] Reference ADRs 0054-0058 
- [x] Note this is a beneficial deviation 
- [x] Update README.md compliance stats 

**Completion Criteria:**
- [x] Decision is documented 
- [x] Rationale is clear 
- [x] Links to supporting ADRs provided 
- [x] README.md audit section updated 

---

## Phase 5: Verification & Testing

**Priority:** CRITICAL
**Estimated Effort:** 3-4 hours
**Dependencies:** Phases 1-3 complete

### Task 5.1: Run Full Test Suite
**Estimated Time:** 1 hour

- [x] Run `cargo test --all-features` 
- [x] Run `cargo test --no-default-features` 
- [x] Run `cargo test` (default features only) 
- [x] Run `cargo clippy --all-targets --all-features` 
- [x] Run `cargo clippy --all-targets --all-features -- -D warnings` (deny warnings) 
- [x] Run `cargo fmt -- --check` 
- [x] Run `cargo doc --no-deps` (verify docs build) 
- [x] Run `cargo deny check` (supply chain audit) 
- [x] Fix any test failures 
- [x] Fix any clippy warnings 
- [x] Fix any formatting issues 
- [x] Ensure no compilation errors 

---

### Task 5.2: Manual Integration Testing
**Estimated Time:** 2 hours

#### Tombstone Testing
- [x] Create a memory in a feature branch context 
- [x] Delete the feature branch 
- [x] Run recall (should trigger lazy GC) 
- [x] Verify memory is tombstoned 
- [x] Verify memory doesn't appear in default results 
- [x] Run recall with --include-tombstoned 
- [x] Verify tombstoned memory appears 
- [x] Test `subcog gc` command 
- [x] Test `subcog gc --dry-run` 
- [x] Test `subcog gc --purge --older-than=30d` 
- [x] Verify purged memories are permanently deleted 

#### Org-Scope Testing
- [x] Test with SUBCOG_ORG_SCOPE_ENABLED=false (default) 
- [x] Verify org-scope features are disabled 
- [x] Test with SUBCOG_ORG_SCOPE_ENABLED=true 
- [x] Verify org-scope features are enabled (if implemented) 
- [x] Test with invalid env var value 
- [x] Verify graceful fallback to false 

#### MCP Server Testing
- [x] Test MCP server with notifications 
- [x] Verify notifications don't receive responses 
- [x] Verify HTTP returns 204 No Content for notifications 
- [x] Verify stdio skips response for notifications 
- [x] Verify error responses include id field 
- [x] Test with MCP client (Claude Code or compatible) 

---

### Task 5.3: Update ADR Health Status
**Estimated Time:** 30 minutes
**Files:**
- `docs/adrs/README.md`
- `docs/adrs/adr_0051.md`
- `docs/adrs/adr_0052.md`
- `docs/adrs/adr_0053.md`

- [x] Update README.md overall compliance statistics 
- [x] Change ADR-0053 from CRITICAL to COMPLIANT 
- [x] Change ADR-0052 from ️ PARTIAL to COMPLIANT 
- [x] Change ADR-0051 from NON-COMPLIANT to COMPLIANT 
- [x] Add "Remediation Completed: 2026-MM-DD" to each audit section 
- [x] Update critical issues section in README 
- [x] Recalculate compliance percentages 
- [x] Update "Next Scheduled Audit" date 
- [x] Remove tasks from critical issues section 

**Expected Final Stats:**
- Total ADRs: 58
- Compliant: 52 (89.7% minimum, or 100% if all issues resolved)
- Non-Compliant: 0-1 (only documented beneficial deviations)

---

## Summary Timeline

| Phase | Priority | Effort | Dependencies | Tasks |
|-------|----------|--------|--------------|-------|
| Phase 1: Tombstone (ADR-0053) | CRITICAL | 8-12h | None | 7 tasks |
| Phase 2: Lazy GC (ADR-0052) | 🟠 HIGH | 2-3h | Phase 1 | 2 tasks |
| Phase 3: Org-Scope (ADR-0051) | 🟡 MEDIUM | 2-3h | None | 4 tasks |
| Phase 4: Documentation (ADR-0009) | 🟢 LOW | 1-2h | None | 1 task |
| Phase 5: Verification | CRITICAL | 3-4h | Phases 1-3 | 3 tasks |
| **Total** | | **16-24h** | | **17 tasks** |

---

## Execution Strategy

### Recommended: Sequential (Solo Developer)

**Week 1 Schedule:**
- **Day 1-2 (Mon-Tue):** Phase 1 - Tombstone Implementation (8-12h)
 - Tasks 1.1-1.7
- **Day 3 (Wed):** Phase 2 - Lazy GC Verification (2-3h)
 - Tasks 2.1-2.2
- **Day 4 (Thu):** Phase 3 - Org-Scope Feature Gate (2-3h)
 - Tasks 3.1-3.4
- **Day 5 (Fri):** Phases 4-5 - Documentation + Verification (4-6h)
 - Tasks 4.1, 5.1-5.3

### Alternative: Parallel (Multiple Contributors)

**Developer A:**
- Phase 1: Tombstone Pattern (Days 1-2)
- Phase 2: Lazy GC Verification (Day 3)

**Developer B:**
- Phase 3: Org-Scope Feature Gate (Days 1-2)
- Phase 4: Documentation (Day 3)

**Both Together:**
- Phase 5: Verification (Day 4)

---

## Risk Mitigation

### Risk 1: Migration Breaks Existing Data
**Likelihood:** Medium
**Impact:** High

**Mitigation:**
- [x] Test migration on copy of production database first 
- [x] Create and test rollback migration 
- [x] Backup database before running migration 
- [x] Test on database with real data volume 
- [x] Verify migration is idempotent 
- [x] Have rollback plan documented and tested 

### Risk 2: Tombstone Logic Affects Performance
**Likelihood:** Low
**Impact:** Medium

**Mitigation:**
- [x] Add partial index on tombstoned_at (only non-NULL values) 
- [x] Use `WHERE status!= 'Tombstoned'` in default queries (indexed) 
- [x] Monitor query performance after deployment 
- [x] Run EXPLAIN ANALYZE on recall queries 
- [x] Benchmark before/after with realistic data 

### Risk 3: Breaking Changes in Memory Struct
**Likelihood:** Low
**Impact:** High

**Mitigation:**
- [x] Make tombstoned_at `Option<DateTime<Utc>>` (nullable) 
- [x] Default to None for existing memories 
- [x] Maintain backward compatibility in JSON serialization 
- [x] Test deserialization of old memory objects 
- [x] Version the Memory struct if needed 

### Risk 4: GC Tests Fail After Changes
**Likelihood:** Medium
**Impact:** Medium

**Mitigation:**
- [x] Run GC tests frequently during development 
- [x] Fix test failures immediately 
- [x] Add new tests before removing old ones 
- [x] Use TDD approach for new tombstone operations 

---

## Success Criteria

### Code Quality
- [x] All tests pass: `cargo test --all-features` 
- [x] No clippy warnings: `cargo clippy --all-targets --all-features -- -D warnings` 
- [x] Code properly formatted: `cargo fmt -- --check` 
- [x] Documentation builds: `cargo doc --no-deps` 
- [x] Supply chain audit passes: `cargo deny check` 

### Functional Requirements
- [x] ADR-0053 compliant: Tombstone pattern working end-to-end 
- [x] ADR-0052 compliant: Lazy GC tombstones stale branch memories 
- [x] ADR-0051 compliant: Org-scope feature gate present and functional 
- [x] ADR-0009 deviation properly documented 

### Testing
- [x] Unit tests pass for all new code 
- [x] Integration tests pass for tombstone flow 
- [x] Manual testing completed successfully 
- [x] Performance acceptable (no regression) 

### Documentation
- [x] README.md shows ≥95% compliance (or 100% with documented deviations) 
- [x] All ADR audit sections updated with remediation dates 
- [x] Configuration documentation updated 
- [x] User-facing docs explain new features 

### Deployment Readiness
- [x] `make ci` passes clean 
- [x] Database migrations tested 
- [x] Rollback plan documented 
- [x] Production deployment plan ready 

---

## Completion Checklist

Copy this to track progress:

```markdown
## Phase 1: Tombstone Pattern (ADR-0053) [0/7]
- [x] Task 1.1: Add Tombstoned variant to MemoryStatus enum 
- [x] Task 1.2: Add tombstoned_at field to Memory struct 
- [x] Task 1.3: Create SQLite migration for tombstoned_at 
- [x] Task 1.4: Update PostgreSQL schema 
- [x] Task 1.5: Update Memory CRUD operations 
- [x] Task 1.6: Implement tombstone operations 
- [x] Task 1.7: Update RecallService filtering 

## Phase 2: Lazy GC (ADR-0052) [0/2]
- [x] Task 2.1: Verify BranchGarbageCollector works 
- [x] Task 2.2: Add end-to-end GC integration test 

## Phase 3: Org-Scope (ADR-0051) [0/4]
- [x] Task 3.1: Add org_scope_enabled to FeatureFlags 
- [x] Task 3.2: Add environment variable support 
- [x] Task 3.3: Wire up feature check 
- [x] Task 3.4: Document org-scope configuration 

## Phase 4: Documentation (ADR-0009) [0/1]
- [x] Task 4.1: Document MCP implementation choice 

## Phase 5: Verification [0/3]
- [x] Task 5.1: Run full test suite 
- [x] Task 5.2: Manual integration testing 
- [x] Task 5.3: Update ADR health status 

## Final Gates [0/5]
- [x] `make ci` passes clean 
- [x] All critical ADR findings resolved 
- [x] README.md compliance ≥95% 
- [x] Manual testing complete 
- [x] Ready for production deployment 
```

**Progress:** 0/17 tasks complete (0%)

---

## Notes

### Resources
- ADR Audit Report: `docs/adrs/README.md`
- Individual ADR Audits: `docs/adrs/adr_00*.md` (see Audit section)
- Original Specifications: `docs/spec/completed/2026-01-03-storage-simplification/`

### Questions/Blockers
- None currently

### Decisions Made During Implementation
- (Track any deviations or design decisions here)

---

**Last Updated:** 2026-01-04
**Next Review:** After Phase 1 completion
