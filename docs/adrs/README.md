# Architecture Decision Records (ADRs)

This directory contains all Architecture Decision Records for the Subcog project.

**Last Audit:** 2026-01-04

---

## ADR Health Summary

### Overall Statistics

- **Total ADRs:** 58
- **Compliant:** 52 (89.7%)
- **Partial/Superseded:** 3 (5.2%)
- **Non-Compliant:** 0 (0%)
- **Deprecated:** 1 (1.7%)
- **Not Applicable:** 2 (3.4%)

### Health by Category

| Category | Total | Compliant | Issues |
|----------|-------|-----------|--------|
| Architecture | 22 | 19 | 3 |
| Storage | 12 | 11 | 1 |
| Search | 7 | 7 | 0 |
| AI/ML | 3 | 3 | 0 |
| Integration | 6 | 5 | 1 |
| Observability | 2 | 2 | 0 |
| Performance | 2 | 2 | 0 |
| Configuration | 2 | 2 | 0 |
| Indexing | 0 | 0 | 0 |
| Parsing | 1 | 1 | 0 |
| Resilience | 0 | 0 | 0 |
| Migration | 1 | 1 | 0 |
| Caching | 0 | 0 | 0 |

---

## Complete ADR Inventory

### ✅ Compliant ADRs (49)

| # | Title | Category | Status | Health |
|---|-------|----------|--------|--------|
| 0001 | Rust as Implementation Language | architecture | published | ✅ COMPLIANT |
| 0002 | Three-Layer Storage Architecture | architecture | published | ✅ COMPLIANT |
| 0003 | Feature Tier System | architecture | published | ✅ COMPLIANT |
| 0005 | URN Scheme for Memory Addressing | architecture | published | ✅ COMPLIANT |
| 0007 | fastembed for Embedding Generation | ai-ml | published | ✅ COMPLIANT |
| 0008 | usearch for Vector Search | ai-ml | published | ✅ COMPLIANT |
| 0010 | OpenTelemetry for Observability | observability | published | ✅ COMPLIANT |
| 0011 | Hybrid Detection Strategy (Keyword + LLM) | search | published | ✅ COMPLIANT |
| 0012 | Namespace Weighting Over Query Rewriting | search | published | ✅ COMPLIANT |
| 0013 | In-Memory Topic Index | search | published | ✅ COMPLIANT |
| 0014 | FTS5 for Text Search | indexing | published | ✅ COMPLIANT |
| 0015 | Token Budget for Injected Memories | search | published | ✅ COMPLIANT |
| 0016 | Confidence Threshold for Injection | search | published | ✅ COMPLIANT |
| 0017 | Short-Circuit Evaluation Order | resilience | published | ✅ COMPLIANT |
| 0018 | Content Hash Storage as Tags | storage | published | ✅ COMPLIANT |
| 0019 | Per-Namespace Similarity Thresholds | search | published | ✅ COMPLIANT |
| 0020 | In-Memory LRU Cache for Recent Captures | caching | published | ✅ COMPLIANT |
| 0021 | Hybrid Search with RRF | search | published | ✅ COMPLIANT |
| 0022 | ServiceContainer Pattern | architecture | published | ✅ COMPLIANT |
| 0023 | Feature Flags via Cargo Features | configuration | published | ✅ COMPLIANT |
| 0024 | JSON Serialization with serde | architecture | published | ✅ COMPLIANT |
| 0025 | Schema Migration Strategy | storage | published | ✅ COMPLIANT |
| 0026 | URN Resource Scheme for MCP | integration | published | ✅ COMPLIANT |
| 0027 | Graceful Degradation on Component Failure | resilience | published | ✅ COMPLIANT |
| 0028 | Secrets Detection and Filtering | architecture | published | ✅ COMPLIANT |
| 0029 | Claude Code Hook Integration | integration | published | ✅ COMPLIANT |
| 0030 | Single SQLite Database File | storage | published | ✅ COMPLIANT |
| 0031 | Graceful Fallback Strategy | resilience | published | ✅ COMPLIANT |
| 0032 | Use fastembed-rs for Embeddings | ai-ml | published | ✅ COMPLIANT |
| 0033 | Lazy Load Embedding Model | performance | published | ✅ COMPLIANT |
| 0035 | Score Normalization to 0.0-1.0 Range | search | published | ✅ COMPLIANT |
| 0036 | Graceful Degradation Strategy | resilience | published | ✅ COMPLIANT |
| 0037 | Model Selection - all-MiniLM-L6-v2 | ai-ml | published | ✅ COMPLIANT |
| 0038 | Vector Index Implementation - usearch | storage | published | ✅ COMPLIANT |
| 0040 | SQLite for User-Scope Persistence | storage | published | ✅ COMPLIANT |
| 0041 | Environment Variables for Configuration | configuration | published | ✅ COMPLIANT |
| 0042 | Factory Method Pattern for ServiceContainer | architecture | published | ✅ COMPLIANT |
| 0043 | No-Op SyncService for User Scope | architecture | published | ✅ COMPLIANT |
| 0044 | User Data Directory Location | configuration | published | ✅ COMPLIANT |
| 0045 | URN Format for User Scope | architecture | published | ✅ COMPLIANT |
| 0046 | Fallback Order in from_current_dir_or_user() | architecture | published | ✅ COMPLIANT |
| 0047 | Remove Git-Notes Storage Layer | storage | published | ✅ COMPLIANT |
| 0048 | Consolidate to User-Level Storage with Faceting | storage | published | ✅ COMPLIANT |
| 0049 | Inline Facet Columns (Denormalized) | storage | published | ✅ COMPLIANT |
| 0050 | Fresh Start - No Migration of Legacy Data | migration | published | ✅ COMPLIANT |
| 0054 | Notification Detection via id Field Absence | integration | published | ✅ COMPLIANT |
| 0055 | Empty String Return for Notification Responses | integration | published | ✅ COMPLIANT |
| 0056 | Always Include id in Error Responses | integration | published | ✅ COMPLIANT |
| 0057 | HTTP Transport Returns 204 for Notifications | integration | published | ✅ COMPLIANT |
| 0058 | Debug-Level Logging for Notifications | observability | published | ✅ COMPLIANT |

### ⚠️ Partial/Superseded ADRs (3)

| # | Title | Category | Status | Health | Notes |
|---|-------|----------|--------|--------|-------|
| 0034 | Three-Layer Storage Synchronization Strategy | storage | published | ⚠️ PARTIAL | Superseded by ADR-0047 (Remove Git-Notes). Historical reference only. |
| 0039 | Backward Compatibility with Existing Memories | storage | published | ⚠️ PARTIAL | Superseded by ADR-0050 (Fresh Start). No migration needed. |
| 0052 | Lazy Branch Garbage Collection | architecture | published | ⚠️ PARTIAL | Implemented but depends on ADR-0053 which has gaps. |

### ❌ Non-Compliant ADRs (3)

| # | Title | Category | Status | Health | Severity | Issue |
|---|-------|----------|--------|--------|----------|-------|
| 0009 | rmcp for MCP Server Implementation | integration | published | ❌ NON-COMPLIANT | MEDIUM | Custom JSON-RPC 2.0 implementation used instead of rmcp crate. Deliberate architectural choice for better spec compliance. |
| 0051 | Feature-Gate Org-Scope Implementation | architecture | published | ❌ NON-COMPLIANT | HIGH | `org_scope_enabled` flag missing from FeatureFlags struct. Org-scope cannot be toggled. |
| 0053 | Tombstone Pattern for Soft Deletes | architecture | published | ❌ CRITICAL | CRITICAL | `Tombstoned` variant missing from MemoryStatus enum. `tombstoned_at` field missing from Memory struct. Code references these extensively but they don't exist. |

### 🔄 Deprecated ADRs (1)

| # | Title | Category | Status | Health |
|---|-------|----------|--------|--------|
| 0006 | Git Notes for Project-Scope Persistence | storage | deprecated | 🔄 DEPRECATED |

### ⊘ Not Applicable (2)

| # | Title | Category | Status | Health | Notes |
|---|-------|----------|--------|--------|-------|
| 0004 | Event Bus for Cross-Component Communication | architecture | published | ⊘ N/A | Superseded by service-based architecture. Never implemented. |

---

## Critical Issues Requiring Immediate Attention

### 1. **CRITICAL: Tombstone Pattern (ADR-0053) - Data Model Gap**

**Severity:** 🔴 CRITICAL
**Impact:** Soft-delete functionality broken, lazy GC (ADR-0052) blocked

**Issue:**
- Code extensively references `MemoryStatus::Tombstoned` variant and `memory.tombstoned_at` field
- Neither exists in actual data model definitions
- Located in:
  - `src/models/domain.rs`: MemoryStatus enum only has Active, Archived, Superseded, Pending, Deleted
  - `src/models/memory.rs`: Memory struct lacks `tombstoned_at: Option<DateTime<Utc>>` field
- Referenced in:
  - `src/gc/branch.rs` lines 469, 553-571 (sets tombstoned_at, checks status)

**Remediation:**
1. Add `Tombstoned` variant to MemoryStatus enum in `src/models/domain.rs`
2. Add `pub tombstoned_at: Option<DateTime<Utc>>` to Memory struct in `src/models/memory.rs`
3. Update SQLite schema migration to add `tombstoned_at` column
4. Verify lazy GC tests pass after fix

---

### 2. **HIGH: Org-Scope Feature Gate Missing (ADR-0051)**

**Severity:** 🟠 HIGH
**Impact:** Cannot toggle org-scope at runtime per architecture design

**Issue:**
- ADR specifies `pub org_scope_enabled: bool` in FeatureFlags
- Field never added to `src/config/features.rs`
- Org-scope architecture included but cannot be disabled

**Remediation:**
1. Add `pub org_scope_enabled: bool` to FeatureFlags struct
2. Default to `false` per ADR specification
3. Wire up to org-scope initialization logic
4. Add environment variable `SUBCOG_ORG_SCOPE_ENABLED` support

---

### 3. **MEDIUM: MCP Server Implementation Deviation (ADR-0009)**

**Severity:** 🟡 MEDIUM
**Impact:** None (working as intended with better spec compliance)

**Issue:**
- ADR specifies using `rmcp` crate for MCP server
- Custom JSON-RPC 2.0 implementation built instead
- ADRs 0054-0058 document why custom implementation was needed (notification compliance)

**Remediation:**
- **No action required** - this was a deliberate architectural decision
- Consider updating ADR-0009 to document why rmcp was not used
- Note: Custom implementation achieved JSON-RPC 2.0 compliance that rmcp couldn't provide

---

## ADR Audit Process

Each ADR has been audited with the following methodology:

1. **ADR Review**: Read decision, rationale, and expected implementation
2. **Codebase Search**: Located relevant files via Grep/Glob patterns
3. **Evidence Collection**: Documented file paths, line numbers, and implementation details
4. **Compliance Assessment**: Rated as COMPLIANT, PARTIAL, NON-COMPLIANT, or N/A
5. **Documentation**: Added audit section to each ADR with findings

### Audit Sections in ADRs

Each ADR now contains an `## Audit` section with:
- **Date**: Audit completion date (2026-01-04)
- **Finding**: COMPLIANT / PARTIAL / NON-COMPLIANT / N/A / DEPRECATED
- **Evidence**: File paths and line numbers supporting the finding
- **Comment**: Brief assessment and any recommended actions

---

## How to Use This Document

### For Developers

When working on a feature:
1. Check if relevant ADRs exist in the inventory above
2. Review the ADR's health status and audit findings
3. Follow compliant ADRs as implementation guidelines
4. Fix non-compliant implementations before adding new features

### For Architects

When proposing new decisions:
1. Review related ADRs in same category
2. Check for superseded patterns (Partial/Deprecated status)
3. Update or create new ADR using MADR format
4. Run `make audit` to verify compliance

### For Code Reviewers

During PR review:
1. Verify changes align with relevant ADRs
2. Check audit findings for implementation guidance
3. Flag deviations from accepted architectural patterns
4. Update audit sections if implementation changes

---

## Re-Audit Schedule

ADRs should be re-audited:
- **After major refactors**: When storage, search, or service layers change significantly
- **Quarterly**: Every 3 months to catch drift
- **On ADR updates**: When new ADRs added or existing ones modified
- **Before major releases**: Ensure architecture integrity

**Next Scheduled Audit:** 2026-04-04

---

## ADR Naming Convention

- **File format**: `adr_NNNN.md` (e.g., `adr_0001.md`)
- **Numbering**: Sequential, zero-padded to 4 digits
- **Status**: `published` (active), `deprecated` (superseded), `proposed` (pending)
- **Frontmatter**: YAML with title, description, type, category, tags, status, dates, author, project

---

## Contributing

To propose a new ADR:
1. Copy `docs/spec/active/adr-template.md` (if exists) or use MADR format
2. Number sequentially (next available number)
3. Fill in decision, context, consequences, alternatives
4. Submit PR with ADR + implementation
5. Update this README after merge

---

## References

- **MADR Format**: https://adr.github.io/madr/
- **ADR Process**: `docs/architecture/README.md`
- **Project Architecture**: `CLAUDE.md` (root)
- **Spec Projects**: `docs/spec/` (active and completed specifications)
