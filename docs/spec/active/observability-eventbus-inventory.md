# Observability & Event Bus Inventory

**Date:** 2026-01-04  
**Owner:** TBD

## Event Emissions & Consumers

**Producers:**
- Capture service emits `MemoryEvent::Captured` and `MemoryEvent::Redacted` (`src/services/capture.rs`).
- Recall service emits `MemoryEvent::Retrieved` for search results (`src/services/recall.rs`).
- Consolidation service emits `MemoryEvent::Consolidated` (`src/services/consolidation.rs`).

**Consumers:**
- Audit logger subscribes to the global event bus and logs all events (`src/security/audit.rs`).

## Tracing Spans & Correlation

**Instrumented spans (`#[instrument]`):**
- Services: capture, recall, consolidation, tombstone, enrichment, prompt enrichment, sync, data subject deletion.
- Hooks: session_start, pre_compact, post_tool_use, stop, user_prompt.
- GC: branch and retention collectors.
- Storage (SQLite index): checkpoint, index, remove, clear, get_memory, batch get, reindex.

**Correlation fields:**
- Span fields commonly include `operation`, `memory_id`, and `backend` where applicable.
- No standardized `request_id` or correlation ID is currently present in spans/logs.

## Metrics & Exporters

_(pending inventory)_

## Coverage Gaps

_(pending inventory)_
