# Subcog Consolidate Fixes Task Plan

## Goal
Resolve negative findings around consolidation and reindexing so tools behave predictably with existing memory data, and document context for later followup.

## Findings
- `subcog_consolidate` uses `recall.search` with `query="*"` by default, which is treated as a literal term and can return zero results even when memories exist.
 - Code: `src/mcp/tools/handlers/core.rs` -> `execute_consolidate`.
 - Contrast: `execute_recall` explicitly switches to `recall.list_all` when query is `"*"` or empty.
 - Symptom: `subcog_consolidate` reports "No memories found" even though `subcog://_/decisions` or `subcog://project/decisions` shows non-zero count.
 - Data validation: `subcog_consolidate` works when `query` is a real term (e.g., `query=postgresql` yields results).
- `subcog_reindex` fails with `datatype mismatch` during `read_list_row`, blocking index rebuild.
 - Error surfaced by `subcog_reindex` tool: `Reindex failed: operation 'read_list_row' failed: datatype mismatch`.
 - Code path: `src/services/mod.rs` -> `reindex_scope` -> `index.list_all` -> `src/storage/index/sqlite.rs` -> `list_all` -> `read_list_row`.
 - Suspected cause: row type mismatch for `score` (currently read as `f64`) or inconsistent schema between FTS/index tables after migrations.

## Plan
 - [x] Reproduce and scope the consolidate default query issue. 
 - Use `subcog_consolidate` with no query / `"*"` and compare against `subcog_recall` with `query="*"` and `filter="ns:decisions"`.
 - Record expected vs actual counts (use `subcog://_/decisions` for count reference).
 - Observed: `subcog_consolidate` (dry_run, namespace decisions) -> "No memories found"; `subcog_recall` with `query="*"` + `filter="ns:decisions"` -> 5 results (project scope).
 - [x] Fix consolidate to use `recall.list_all` for wildcard/empty queries. 
 - Mirror logic from `execute_recall` in `src/mcp/tools/handlers/core.rs`.
 - Add tests to cover:
 - wildcard/empty query -> list_all
 - non-empty query -> search
 - [x] Investigate reindex datatype mismatch. 
 - Inspect SQLite schema for `memories` and FTS/index tables used by `list_all`.
 - Reproduce by calling reindex on the current project scope and capture full error details.
 - Identify the column or row causing mismatch (e.g., `score` typing, schema drift).
 - Root cause: `list_all` was invoked with `limit=usize::MAX`, producing a LIMIT value larger than SQLite's signed 64-bit range.
 - [x] Implement fix or migration. 
 - If schema drift: add migration or repair step and document it.
 - If read type mismatch: adjust read type or query to use consistent numeric type.
 - Add regression test for reindex that exercises `list_all` path.
 - Re-run `subcog_reindex` to verify.
 - Fix: clamp `list_all` limit to `i64::MAX` and added regression test for `usize::MAX`.
 - Verified: `cargo run --bin subcog -- reindex` completed successfully (254 memories indexed).

## Exit Criteria
- `subcog_consolidate` returns memories for default/empty queries without requiring a user query.
- `subcog_reindex` succeeds without datatype mismatch errors.
- Regression tests cover both issues.

## Notes / Context
- Environment: project repo at `zircote/subcog`.
- MCP behavior:
 - `subcog_recall` handles `"*"` by switching to `list_all`.
 - `subcog_consolidate` currently always calls `search` and does not handle `"*"` specially.
