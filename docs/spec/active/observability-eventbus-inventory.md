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

_(pending inventory)_

## Metrics & Exporters

_(pending inventory)_

## Coverage Gaps

_(pending inventory)_
