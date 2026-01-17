# ADR Compliance Remediation Plan

**Date:** 2026-01-04
**Status:** Completed
**Scope:** ADR-0005, ADR-0009, ADR-0034, ADR-0041, ADR-0048, ADR-0049, ADR-0052, ADR-0015, ADR-0018

## Goals

- Align MCP server implementation with ADR-0009 (rmcp).
- Remove all legacy notes storage usage per ADR-0034/0041.
- Add project_id/branch/file_path facets and SQLite columns for ADR-0048/0049 and unblock ADR-0052.
- Add hash tagging on capture and enforce token budgets for injected memories (ADR-0018/0015).
- Validate ADR-0005 alignment with current URN format and update code if required by the revised intent.

## Plan

### 1) ADR-0005 Alignment Check (URN Format)

**Outcome:** Code behavior matches the updated ADR intent.

Tasks:
- [x] Re-read `docs/adrs/adr_0005.md` to confirm the revised URN structure requirements. 
- [x] Inventory URN construction and parsing paths (capture, recall, search filters, serialization). 
- [x] If ADR requires a different URN path segment, update generators and parsers consistently. 
- [x] Update tests and any documentation that references the URN scheme. 

### 2) ADR-0009 MCP Server Migration (rmcp)

**Outcome:** MCP server uses rmcp and conforms to the ADR.

Tasks:
- [x] Audit current MCP server entry points and notification handling behavior. 
- [x] Add rmcp dependency and implement server wiring to replace bespoke MCP server. 
- [x] Map existing MCP handlers to rmcp interfaces; ensure notification responses are suppressed. 
- [x] Update error handling to rmcp error model. 
- [x] Update tests/fixtures for MCP protocol compatibility. 
- [x] Update docs and remove references to bespoke MCP server implementation. 

### 3) ADR-0034/0041 Legacy Notes Removal

**Outcome:** No legacy notes storage persists; all legacy notes code paths removed.

Tasks:
- [x] Locate all legacy notes read/write paths, flags, and config references. 
- [x] Remove legacy notes implementation and related CLI options. 
- [x] Delete or migrate tests that depend on legacy notes; replace with the current persistence layer. 
- [x] Remove any docs, examples, or specs referencing legacy notes. 
- [x] Confirm no runtime fallback or hidden legacy notes behavior remains. 

### 4) ADR-0048/0049 Facets + SQLite Columns

**Outcome:** project_id/branch/file_path facets exist end-to-end.

Tasks:
- [x] Add `project_id`, `branch`, and `file_path` to core memory models and search filters. 
- [x] Extend SQLite schema with new columns and indexes; add migrations. 
- [x] Update persistence layer CRUD to read/write the new columns. 
- [x] Update capture pipeline to populate facets from context. 
- [x] Update query builders and filter logic to support the facets. 
- [x] Add tests for facets across capture, search, and storage. 

### 5) ADR-0052 Unblock (Branch GC)

**Outcome:** Branch GC uses new facets and behaves per ADR-0052.

Tasks:
- [x] Wire branch GC filters to use `project_id` and `branch` facets. 
- [x] Update branch cache lookups and filters to use new model fields. 
- [x] Add/adjust GC tests to cover branch-specific filtering. 

### 6) ADR-0018 Hash Tagging on Capture

**Outcome:** Hash tags are appended to captured memories automatically.

Tasks:
- [x] Reuse existing hash-tag generation utilities. 
- [x] Add capture-time tag injection for hash tags (avoid duplicates). 
- [x] Validate tags are persisted and returned in search/recall. 
- [x] Add tests to assert hash tags are present after capture. 

### 7) ADR-0015 Token Budget Enforcement for Injected Memories

**Outcome:** Injected memory context respects token budgets.

Tasks:
- [x] Identify the injection assembly path that produces prompt context. 
- [x] Use configured max token budgets to prune or truncate memory content. 
- [x] Ensure ordering/prioritization is deterministic when trimming. 
- [x] Add tests for overflow behavior and correct truncation. 

### 8) Verification & Documentation Updates

**Outcome:** ADR audits updated and behavior validated.

Tasks:
- [x] Run targeted tests for MCP, capture, search, and GC paths. 
- [x] Update ADR audit sections for the addressed ADRs. 
- [x] Refresh `docs/adrs/README.md` compliance status. 

## Dependencies

- Facet work (ADR-0048/0049) must land before branch GC updates (ADR-0052).
- MCP migration should be completed before updating ADR-0009 audit status.

## Risks

- MCP migration could impact client compatibility; plan for protocol regression tests.
- Schema changes require migration planning and local data validation.
