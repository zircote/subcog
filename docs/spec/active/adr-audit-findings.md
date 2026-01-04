# ADR Audit Remediation Plan

**Date Created:** 2026-01-04
**Audit Date:** 2026-01-04
**Status:** PENDING
**Target Completion:** 2026-01-11

---

## Executive Summary

**Total Tasks:** 15 tasks across 5 phases
**Estimated Effort:** 16-24 hours
**Critical Path:** Phase 1 (Tombstone) → Phase 2 (Lazy GC) → Phase 5 (Verification)
**Overall Compliance Before:** 84.5% (49/58 ADRs)
**Target Compliance After:** 100% (with documented deviations)

---

## Critical Issues to Resolve

### 🔴 CRITICAL: ADR-0053 - Tombstone Pattern
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

**Priority:** 🔴 CRITICAL
**Estimated Effort:** 8-12 hours
**Dependencies:** None
**Blocks:** ADR-0052, lazy branch GC functionality

### Task 1.1: Add Tombstoned Variant to MemoryStatus Enum
**Estimated Time:** 30 minutes
**File:** `src/models/domain.rs` (around line 260-295)

- [x] Add `Tombstoned` variant to MemoryStatus enum ✓
- [x] Update `as_str()` method to handle Tombstoned case ✓
- [x] Update `from_str()` method to parse "tombstoned" ✓
- [x] Verify serde serialization works correctly ✓
- [x] Ensure enum ordering doesn't break existing functionality ✓
- [x] Run unit tests for MemoryStatus ✓

**Code Change:**
```rust
pub enum MemoryStatus {
    Active,
    Archived,
    Superseded,
    Pending,
    Deleted,
    Tombstoned,  // ADD THIS
}
```

---

### Task 1.2: Add tombstoned_at Field to Memory Struct
**Estimated Time:** 1 hour
**File:** `src/models/memory.rs` (around line 43-66)

- [x] Add `pub tombstoned_at: Option<DateTime<Utc>>` field to Memory struct ✓
- [x] Configure serde attributes for JSON serialization ✓
- [x] Ensure default value is `None` for new memories ✓
- [x] Update getter/setter methods if they exist ✓
- [x] Update all Memory constructors to include new field ✓
- [x] Update Display/Debug implementations if needed ✓
- [x] Run unit tests for Memory struct ✓

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
    pub tombstoned_at: Option<DateTime<Utc>>,  // ADD THIS
    pub embedding: Option<Vec<f32>>,
    pub tags: Vec<String>,
    pub source: Option<String>,
}
```

---

### Task 1.3: Create SQLite Migration for tombstoned_at Column
**Estimated Time:** 2 hours
**File:** `src/storage/migrations/sqlite/` (new migration file)

- [x] Create new migration file with proper naming (e.g., `0004_add_tombstoned_at.sql`) ✓
- [x] Add `ALTER TABLE memories ADD COLUMN tombstoned_at TIMESTAMP NULL` ✓
- [x] Create partial index: `CREATE INDEX idx_memories_tombstoned ON memories(tombstoned_at) WHERE tombstoned_at IS NOT NULL` ✓
- [x] Add migration to migration runner ✓
- [x] Create rollback migration ✓
- [x] Test migration on fresh database ✓
- [x] Test migration on database with existing data ✓
- [x] Verify migration is idempotent (safe to run multiple times) ✓

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

- [x] Create PostgreSQL migration file ✓
- [x] Add `ALTER TABLE memories ADD COLUMN tombstoned_at TIMESTAMPTZ NULL` ✓
- [x] Create partial index for performance ✓
- [x] Test migration on PostgreSQL instance ✓
- [x] Verify timezone handling works correctly ✓

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

- [ ] Update SQLite INSERT statements to include tombstoned_at
- [ ] Update SQLite SELECT statements to fetch tombstoned_at
- [ ] Update PostgreSQL INSERT statements
- [ ] Update PostgreSQL SELECT statements
- [ ] Update filesystem persistence serialization
- [ ] Update memory deserialization to handle NULL tombstoned_at
- [ ] Verify all persistence backends handle new field
- [ ] Run existing persistence tests
- [ ] Add new tests for tombstoned memories

---

### Task 1.6: Implement Tombstone Operations
**Estimated Time:** 2-3 hours
**File:** `src/services/mod.rs` or create `src/services/tombstone.rs`

- [ ] Implement `tombstone_memory(&self, id: &str) -> Result<(), MemoryError>`
  - [ ] Set status to MemoryStatus::Tombstoned
  - [ ] Set tombstoned_at to Utc::now()
  - [ ] Update in persistence layer
  - [ ] Use transaction for atomicity
- [ ] Implement `untombstone_memory(&self, id: &str) -> Result<(), MemoryError>`
  - [ ] Set status back to Active
  - [ ] Clear tombstoned_at (set to None)
  - [ ] Update in persistence layer
- [ ] Implement `purge_tombstoned(&self, older_than: Duration) -> Result<usize, MemoryError>`
  - [ ] Query tombstoned memories older than threshold
  - [ ] Permanently delete from persistence
  - [ ] Return count of purged memories
- [ ] Add error handling for not found / already tombstoned cases
- [ ] Write unit tests for all three operations
- [ ] Write integration tests

---

### Task 1.7: Update RecallService to Filter Tombstoned by Default
**Estimated Time:** 1-2 hours
**File:** `src/services/recall.rs`

- [ ] Add `WHERE status != 'Tombstoned'` to default SQL queries
- [ ] Add `include_tombstoned: bool` parameter to search methods
- [ ] Update RecallService API to support include_tombstoned flag
- [ ] Update CLI to support `--include-tombstoned` flag
- [ ] Update MCP tools to support tombstone filtering
- [ ] Verify existing search tests still pass
- [ ] Add new tests for tombstone filtering
- [ ] Test that tombstoned memories are hidden by default
- [ ] Test that --include-tombstoned shows tombstoned memories

---

## Phase 2: High Priority - Lazy Branch GC (ADR-0052)

**Priority:** 🟠 HIGH
**Estimated Effort:** 2-3 hours
**Dependencies:** Phase 1 complete

### Task 2.1: Verify BranchGarbageCollector Works with Tombstone
**Estimated Time:** 2 hours
**File:** `src/gc/branch.rs`

- [ ] Run existing GC test suite
- [ ] Fix compilation errors from new tombstoned_at field
- [ ] Verify tombstone_memory() is called correctly
- [ ] Verify status check works: `memory.status != MemoryStatus::Tombstoned`
- [ ] Verify tombstoned_at is set correctly
- [ ] Test branch deletion triggers tombstoning
- [ ] Test lazy GC during recall works
- [ ] Test 5-minute branch existence cache
- [ ] Test dry-run mode shows what would be tombstoned
- [ ] Fix any broken tests

---

### Task 2.2: Add End-to-End GC Integration Test
**Estimated Time:** 1 hour
**File:** `tests/gc_integration_test.rs` (new file)

- [ ] Create new integration test file
- [ ] Test: Create memory for branch "feature/test"
- [ ] Test: Delete branch "feature/test"
- [ ] Test: Recall memories (should trigger lazy GC)
- [ ] Test: Verify memory status is Tombstoned
- [ ] Test: Verify memory not in default results
- [ ] Test: Recall with --include-tombstoned
- [ ] Test: Verify memory appears in results
- [ ] Test: Run purge operation
- [ ] Test: Verify memory permanently deleted
- [ ] Ensure test cleans up after itself

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

- [ ] Add `pub org_scope_enabled: bool` field to FeatureFlags struct
- [ ] Set default to `false` in Default implementation
- [ ] Add serde attributes for serialization
- [ ] Update FeatureFlags::default() method
- [ ] Update any FeatureFlags builders/constructors
- [ ] Run tests to ensure no breakage

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
    pub org_scope_enabled: bool,  // ADD THIS
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            // ... existing defaults
            org_scope_enabled: false,  // ADD THIS
        }
    }
}
```

---

### Task 3.2: Add Environment Variable Support
**Estimated Time:** 30 minutes
**File:** `src/config/mod.rs`

- [ ] Add environment variable loading for SUBCOG_ORG_SCOPE_ENABLED
- [ ] Support boolean parsing (true/false, 1/0, yes/no, on/off)
- [ ] Handle invalid values gracefully (default to false)
- [ ] Add logging when org-scope enabled via env var
- [ ] Test environment variable loading
- [ ] Document env var in code comments

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

- [ ] Search codebase for org-scope initialization code
- [ ] Add feature flag check before org-scope initialization
- [ ] Add warning log if org-scope disabled but PostgreSQL configured
- [ ] Add error message explaining how to enable org-scope
- [ ] Verify org-scope only initializes when flag is true
- [ ] Test that org-scope is disabled by default
- [ ] Test that org-scope can be enabled via env var

---

### Task 3.4: Document Org-Scope Configuration
**Estimated Time:** 1 hour
**Files:**
- `CLAUDE.md`
- `README.md` (if applicable)
- `docs/configuration.md` (if exists)

- [ ] Document org_scope_enabled feature flag in CLAUDE.md
- [ ] Document SUBCOG_ORG_SCOPE_ENABLED environment variable
- [ ] Document when to enable org-scope (multi-team scenarios)
- [ ] Document PostgreSQL requirements for org-scope
- [ ] Provide configuration example
- [ ] Document security considerations
- [ ] Add troubleshooting section

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

- [ ] Create new ADR file: adr_0059.md
- [ ] Add YAML frontmatter
- [ ] Document that this supersedes ADR-0009
- [ ] Explain why custom JSON-RPC was chosen
- [ ] Reference ADRs 0054-0058 for notification compliance
- [ ] Document benefits of custom implementation
- [ ] Update README.md to show ADR-0009 as superseded
- [ ] Update ADR-0009 to reference ADR-0059

**OR**

#### Option B: Amend ADR-0009
**File:** `docs/adrs/adr_0009.md`

- [ ] Add "Implementation Update" section after audit section
- [ ] Explain deviation from original decision
- [ ] Document rationale for custom JSON-RPC implementation
- [ ] Reference ADRs 0054-0058
- [ ] Note this is a beneficial deviation
- [ ] Update README.md compliance stats

**Completion Criteria:**
- [ ] Decision is documented
- [ ] Rationale is clear
- [ ] Links to supporting ADRs provided
- [ ] README.md audit section updated

---

## Phase 5: Verification & Testing

**Priority:** 🔴 CRITICAL
**Estimated Effort:** 3-4 hours
**Dependencies:** Phases 1-3 complete

### Task 5.1: Run Full Test Suite
**Estimated Time:** 1 hour

- [ ] Run `cargo test --all-features`
- [ ] Run `cargo test --no-default-features`
- [ ] Run `cargo test` (default features only)
- [ ] Run `cargo clippy --all-targets --all-features`
- [ ] Run `cargo clippy --all-targets --all-features -- -D warnings` (deny warnings)
- [ ] Run `cargo fmt -- --check`
- [ ] Run `cargo doc --no-deps` (verify docs build)
- [ ] Run `cargo deny check` (supply chain audit)
- [ ] Fix any test failures
- [ ] Fix any clippy warnings
- [ ] Fix any formatting issues
- [ ] Ensure no compilation errors

---

### Task 5.2: Manual Integration Testing
**Estimated Time:** 2 hours

#### Tombstone Testing
- [ ] Create a memory in a feature branch context
- [ ] Delete the feature branch
- [ ] Run recall (should trigger lazy GC)
- [ ] Verify memory is tombstoned
- [ ] Verify memory doesn't appear in default results
- [ ] Run recall with --include-tombstoned
- [ ] Verify tombstoned memory appears
- [ ] Test `subcog gc` command
- [ ] Test `subcog gc --dry-run`
- [ ] Test `subcog gc --purge --older-than=30d`
- [ ] Verify purged memories are permanently deleted

#### Org-Scope Testing
- [ ] Test with SUBCOG_ORG_SCOPE_ENABLED=false (default)
- [ ] Verify org-scope features are disabled
- [ ] Test with SUBCOG_ORG_SCOPE_ENABLED=true
- [ ] Verify org-scope features are enabled (if implemented)
- [ ] Test with invalid env var value
- [ ] Verify graceful fallback to false

#### MCP Server Testing
- [ ] Test MCP server with notifications
- [ ] Verify notifications don't receive responses
- [ ] Verify HTTP returns 204 No Content for notifications
- [ ] Verify stdio skips response for notifications
- [ ] Verify error responses include id field
- [ ] Test with MCP client (Claude Code or compatible)

---

### Task 5.3: Update ADR Health Status
**Estimated Time:** 30 minutes
**Files:**
- `docs/adrs/README.md`
- `docs/adrs/adr_0051.md`
- `docs/adrs/adr_0052.md`
- `docs/adrs/adr_0053.md`

- [ ] Update README.md overall compliance statistics
- [ ] Change ADR-0053 from ❌ CRITICAL to ✅ COMPLIANT
- [ ] Change ADR-0052 from ⚠️ PARTIAL to ✅ COMPLIANT
- [ ] Change ADR-0051 from ❌ NON-COMPLIANT to ✅ COMPLIANT
- [ ] Add "Remediation Completed: 2026-MM-DD" to each audit section
- [ ] Update critical issues section in README
- [ ] Recalculate compliance percentages
- [ ] Update "Next Scheduled Audit" date
- [ ] Remove tasks from critical issues section

**Expected Final Stats:**
- Total ADRs: 58
- Compliant: 52 (89.7% minimum, or 100% if all issues resolved)
- Non-Compliant: 0-1 (only documented beneficial deviations)

---

## Summary Timeline

| Phase | Priority | Effort | Dependencies | Tasks |
|-------|----------|--------|--------------|-------|
| Phase 1: Tombstone (ADR-0053) | 🔴 CRITICAL | 8-12h | None | 7 tasks |
| Phase 2: Lazy GC (ADR-0052) | 🟠 HIGH | 2-3h | Phase 1 | 2 tasks |
| Phase 3: Org-Scope (ADR-0051) | 🟡 MEDIUM | 2-3h | None | 4 tasks |
| Phase 4: Documentation (ADR-0009) | 🟢 LOW | 1-2h | None | 1 task |
| Phase 5: Verification | 🔴 CRITICAL | 3-4h | Phases 1-3 | 3 tasks |
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
- [ ] Test migration on copy of production database first
- [ ] Create and test rollback migration
- [ ] Backup database before running migration
- [ ] Test on database with real data volume
- [ ] Verify migration is idempotent
- [ ] Have rollback plan documented and tested

### Risk 2: Tombstone Logic Affects Performance
**Likelihood:** Low
**Impact:** Medium

**Mitigation:**
- [ ] Add partial index on tombstoned_at (only non-NULL values)
- [ ] Use `WHERE status != 'Tombstoned'` in default queries (indexed)
- [ ] Monitor query performance after deployment
- [ ] Run EXPLAIN ANALYZE on recall queries
- [ ] Benchmark before/after with realistic data

### Risk 3: Breaking Changes in Memory Struct
**Likelihood:** Low
**Impact:** High

**Mitigation:**
- [ ] Make tombstoned_at `Option<DateTime<Utc>>` (nullable)
- [ ] Default to None for existing memories
- [ ] Maintain backward compatibility in JSON serialization
- [ ] Test deserialization of old memory objects
- [ ] Version the Memory struct if needed

### Risk 4: GC Tests Fail After Changes
**Likelihood:** Medium
**Impact:** Medium

**Mitigation:**
- [ ] Run GC tests frequently during development
- [ ] Fix test failures immediately
- [ ] Add new tests before removing old ones
- [ ] Use TDD approach for new tombstone operations

---

## Success Criteria

### Code Quality
- [ ] All tests pass: `cargo test --all-features`
- [ ] No clippy warnings: `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] Code properly formatted: `cargo fmt -- --check`
- [ ] Documentation builds: `cargo doc --no-deps`
- [ ] Supply chain audit passes: `cargo deny check`

### Functional Requirements
- [ ] ADR-0053 compliant: Tombstone pattern working end-to-end
- [ ] ADR-0052 compliant: Lazy GC tombstones stale branch memories
- [ ] ADR-0051 compliant: Org-scope feature gate present and functional
- [ ] ADR-0009 deviation properly documented

### Testing
- [ ] Unit tests pass for all new code
- [ ] Integration tests pass for tombstone flow
- [ ] Manual testing completed successfully
- [ ] Performance acceptable (no regression)

### Documentation
- [ ] README.md shows ≥95% compliance (or 100% with documented deviations)
- [ ] All ADR audit sections updated with remediation dates
- [ ] Configuration documentation updated
- [ ] User-facing docs explain new features

### Deployment Readiness
- [ ] `make ci` passes clean
- [ ] Database migrations tested
- [ ] Rollback plan documented
- [ ] Production deployment plan ready

---

## Completion Checklist

Copy this to track progress:

```markdown
## Phase 1: Tombstone Pattern (ADR-0053) [0/7]
- [ ] Task 1.1: Add Tombstoned variant to MemoryStatus enum
- [ ] Task 1.2: Add tombstoned_at field to Memory struct
- [ ] Task 1.3: Create SQLite migration for tombstoned_at
- [ ] Task 1.4: Update PostgreSQL schema
- [ ] Task 1.5: Update Memory CRUD operations
- [ ] Task 1.6: Implement tombstone operations
- [ ] Task 1.7: Update RecallService filtering

## Phase 2: Lazy GC (ADR-0052) [0/2]
- [ ] Task 2.1: Verify BranchGarbageCollector works
- [ ] Task 2.2: Add end-to-end GC integration test

## Phase 3: Org-Scope (ADR-0051) [0/4]
- [ ] Task 3.1: Add org_scope_enabled to FeatureFlags
- [ ] Task 3.2: Add environment variable support
- [ ] Task 3.3: Wire up feature check
- [ ] Task 3.4: Document org-scope configuration

## Phase 4: Documentation (ADR-0009) [0/1]
- [ ] Task 4.1: Document MCP implementation choice

## Phase 5: Verification [0/3]
- [ ] Task 5.1: Run full test suite
- [ ] Task 5.2: Manual integration testing
- [ ] Task 5.3: Update ADR health status

## Final Gates [0/5]
- [ ] `make ci` passes clean
- [ ] All critical ADR findings resolved
- [ ] README.md compliance ≥95%
- [ ] Manual testing complete
- [ ] Ready for production deployment
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
