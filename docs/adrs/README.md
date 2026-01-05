# ADR Compliance Summary

**Audit Date:** 2026-01-04
**Audited By:** Claude Code
**Codebase:** /Users/AllenR1_1/Projects/zircote/subcog

## Overview

| # | ADR | Status | Health | Action Required |
|---|-----|--------|--------|-----------------|
| 1 | [ADR-0001: Rust as Implementation Language](./adr_0001.md) | Accepted | ✅ Compliant | None |
| 2 | [ADR-0002: Three-Layer Storage Architecture](./adr_0002.md) | Accepted | ✅ Compliant | None |
| 3 | [ADR-0003: Feature Tier System](./adr_0003.md) | Accepted | ⚠️ Partial | Update tier definitions to reflect current storage or reintroduce Tier 1 persistence mapping. |
| 4 | [ADR-0004: Event Bus for Cross-Component Communication](./adr_0004.md) | Accepted | ✅ Compliant | None |
| 5 | [ADR-0005: URN Scheme for Memory Addressing](./adr_0005.md) | Accepted | ✅ Compliant | None |
| 6 | [ADR-0007: fastembed for Embedding Generation](./adr_0007.md) | Accepted | ✅ Compliant | None |
| 7 | [ADR-0008: usearch for Vector Search](./adr_0008.md) | Accepted | ✅ Compliant | None |
| 8 | [ADR-0009: rmcp for MCP Server Implementation](./adr_0009.md) | Accepted | ✅ Compliant | None |
| 9 | [ADR-0010: OpenTelemetry for Observability](./adr_0010.md) | Accepted | ✅ Compliant | None |
| 10 | [ADR-0011: Hybrid Detection Strategy (Keyword + LLM)](./adr_0011.md) | Accepted | ✅ Compliant | None |
| 11 | [ADR-0012: Namespace Weighting Over Query Rewriting](./adr_0012.md) | Accepted | ✅ Compliant | None |
| 12 | [ADR-0013: In-Memory Topic Index](./adr_0013.md) | Accepted | ✅ Compliant | None |
| 13 | [ADR-0014: 200ms LLM Timeout](./adr_0014.md) | Accepted | ✅ Compliant | None |
| 14 | [ADR-0015: Token Budget for Injected Memories](./adr_0015.md) | Accepted | ✅ Compliant | None |
| 15 | [ADR-0016: Confidence Threshold for Injection](./adr_0016.md) | Accepted | ✅ Compliant | None |
| 16 | [ADR-0017: Short-Circuit Evaluation Order](./adr_0017.md) | Accepted | ✅ Compliant | None |
| 17 | [ADR-0018: Content Hash Storage as Tags](./adr_0018.md) | Accepted | ✅ Compliant | None |
| 18 | [ADR-0019: Per-Namespace Similarity Thresholds](./adr_0019.md) | Accepted | ✅ Compliant | None |
| 19 | [ADR-0020: In-Memory LRU Cache for Recent Captures](./adr_0020.md) | Accepted | ✅ Compliant | None |
| 20 | [ADR-0021: Fail-Open on Deduplication Errors](./adr_0021.md) | Accepted | ✅ Compliant | None |
| 21 | [ADR-0022: Semantic Check Minimum Length](./adr_0022.md) | Accepted | ✅ Compliant | None |
| 22 | [ADR-0023: RecallService for Deduplication Lookups](./adr_0023.md) | Accepted | ✅ Compliant | None |
| 23 | [ADR-0024: Hook Output Format for Skip Reporting](./adr_0024.md) | Accepted | ✅ Compliant | None |
| 24 | [ADR-0025: Fenced Code Blocks Only](./adr_0025.md) | Accepted | ✅ Compliant | None |
| 25 | [ADR-0026: LLM Enrichment Always On](./adr_0026.md) | Accepted | ✅ Compliant | None |
| 26 | [ADR-0027: Full Enrichment Scope](./adr_0027.md) | Accepted | ✅ Compliant | None |
| 27 | [ADR-0028: Regex-Based Code Block Detection](./adr_0028.md) | Accepted | ✅ Compliant | None |
| 28 | [ADR-0029: Existing LLM Provider Infrastructure](./adr_0029.md) | Accepted | ✅ Compliant | None |
| 29 | [ADR-0030: User Frontmatter Preservation](./adr_0030.md) | Accepted | ✅ Compliant | None |
| 30 | [ADR-0031: Graceful Fallback Strategy](./adr_0031.md) | Accepted | ✅ Compliant | None |
| 31 | [ADR-0032: Use fastembed-rs for Embeddings](./adr_0032.md) | Accepted | ✅ Compliant | None |
| 32 | [ADR-0033: Lazy Load Embedding Model](./adr_0033.md) | Accepted | ✅ Compliant | None |
| 33 | [ADR-0034: Three-Layer Storage Synchronization Strategy](./adr_0034.md) | Superseded | ❓ Unverifiable | None (superseded by ADR-0047) |
| 34 | [ADR-0035: Score Normalization to 0.0-1.0 Range](./adr_0035.md) | Accepted | ✅ Compliant | None |
| 35 | [ADR-0036: Graceful Degradation Strategy](./adr_0036.md) | Accepted | ✅ Compliant | None |
| 36 | [ADR-0037: Model Selection - all-MiniLM-L6-v2](./adr_0037.md) | Accepted | ✅ Compliant | None |
| 37 | [ADR-0038: Vector Index Implementation - usearch](./adr_0038.md) | Accepted | ✅ Compliant | None |
| 38 | [ADR-0039: Backward Compatibility with Existing Memories](./adr_0039.md) | Accepted | ⚠️ Partial | Expose migration status in the status command or update ADR expectations. |
| 39 | [ADR-0040: SQLite for User-Scope Persistence](./adr_0040.md) | Accepted | ✅ Compliant | None |
| 40 | [ADR-0041: Conditional Git Notes in CaptureService](./adr_0041.md) | Superseded | ❓ Unverifiable | None (superseded by ADR-0047) |
| 41 | [ADR-0042: Factory Method Pattern for ServiceContainer](./adr_0042.md) | Accepted | ✅ Compliant | None |
| 42 | [ADR-0043: No-Op SyncService for User Scope](./adr_0043.md) | Accepted | ✅ Compliant | None |
| 43 | [ADR-0044: User Data Directory Location](./adr_0044.md) | Accepted | ✅ Compliant | None |
| 44 | [ADR-0045: URN Format for User Scope](./adr_0045.md) | Accepted | ✅ Compliant | None |
| 45 | [ADR-0046: Fallback Order in `from_current_dir_or_user()`](./adr_0046.md) | Accepted | ✅ Compliant | None |
| 46 | [ADR-0047: Remove Git-Notes Storage Layer](./adr_0047.md) | Accepted | ✅ Compliant | None |
| 47 | [ADR-0048: Consolidate to User-Level Storage with Faceting](./adr_0048.md) | Accepted | ✅ Compliant | None |
| 48 | [ADR-0049: Inline Facet Columns (Denormalized)](./adr_0049.md) | Accepted | ✅ Compliant | None |
| 49 | [ADR-0050: Fresh Start - No Migration of Legacy Data](./adr_0050.md) | Accepted | ✅ Compliant | None |
| 50 | [ADR-0051: Feature-Gate Org-Scope Implementation](./adr_0051.md) | Accepted | ✅ Compliant | None |
| 51 | [ADR-0052: Lazy Branch Garbage Collection](./adr_0052.md) | Accepted | ✅ Compliant | None |
| 52 | [ADR-0053: Tombstone Pattern for Soft Deletes](./adr_0053.md) | Accepted | ✅ Compliant | None |
| 53 | [ADR-0054: Notification Detection via `id` Field Absence](./adr_0054.md) | Accepted | ✅ Compliant | None |
| 54 | [ADR-0055: Empty String Return for Notification Responses](./adr_0055.md) | Accepted | ✅ Compliant | None |
| 55 | [ADR-0056: Always Include `id` in Error Responses](./adr_0056.md) | Accepted | ✅ Compliant | None |
| 56 | [ADR-0057: HTTP Transport Returns 204 for Notifications](./adr_0057.md) | Accepted | ✅ Compliant | None |
| 57 | [ADR-0058: Debug-Level Logging for Notifications](./adr_0058.md) | Accepted | ✅ Compliant | None |

## Critical Findings

None.

## Recommendations

- Update ADR-0003 tier definitions to reflect the current storage architecture.
- Expose migration status in the status command or revise ADR-0039 expectations.
