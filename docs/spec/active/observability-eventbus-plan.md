# Observability + Event Bus Expansion Plan

**Date:** 2026-01-04  
**Owner:** TBD  
**Status:** Draft  
**Scope:** Event bus, tracing, logging, metrics, telemetry

## Goals

- Provide end-to-end visibility for internal calls and stack flow.
- Standardize logging, tracing, and metrics across services.
- Expand event bus coverage with clear ownership and backpressure behavior.
- Ensure OpenTelemetry is first-class and configurable.

## Non-Goals

- Replacing existing logging/tracing libraries.
- Introducing external message brokers.

## Plan

### Phase 0: Baseline & Inventory
- [x] Audit current event emissions and consumers (capture, recall, consolidation, sync, GC, MCP, hooks) ✓
- [x] Inventory existing tracing spans and log fields for request correlation ✓
- [x] Document current metrics, exporters, and label cardinality risks ✓
- [x] Identify missing observability coverage for critical paths ✓

### Phase 1: Event Bus Expansion
- [x] Define event taxonomy (system, memory lifecycle, security, performance, MCP, hooks) ✓
- [x] Add event payload schema guidelines (required fields, redaction rules) ✓
- [x] Add event bus subscription helpers (filtered subscribers by event type) ✓
- [x] Emit events from all memory lifecycle operations (capture/update/delete/tombstone/recall) ✓
- [x] Emit events for MCP lifecycle (startup, auth, tool execution, request errors) ✓
- [x] Emit events for hook lifecycle (invocation, classification, capture decisions, failures) ✓
- [ ] Add event bus health metrics (publish rate, drop rate, lag)
- [ ] Add unit tests for event dispatch and subscriber filtering

### Phase 2: Tracing & Context Propagation
- [ ] Define trace/span naming conventions and required attributes
- [ ] Add request correlation IDs to CLI/MCP/hook flows (parity)
- [ ] Instrument capture/recall/consolidation/GC with spans and sub-spans
- [ ] Instrument MCP tool execution and hook handlers with spans
- [ ] Propagate context across async boundaries (tokio tasks, hooks, MCP)
- [ ] Add trace sampling configuration with defaults
- [ ] Add tests for trace context propagation in at least two critical flows

### Phase 3: Logging Standardization
- [ ] Define structured log schema (level, event, request_id, memory_id, domain)
- [ ] Ensure logs include correlation IDs and span context
- [ ] Add log redaction rules for sensitive content
- [ ] Normalize log levels across services (error/warn/info/debug)
- [ ] Add log format validation tests (json format, required keys)

### Phase 4: Metrics & Telemetry
- [ ] Define metric naming conventions and required labels
- [ ] Add metrics for event bus (queue depth, publish/subscribe counts)
- [ ] Add metrics for memory lifecycle latency (capture/recall/GC/consolidation)
- [ ] Add metrics for MCP request latency and error rates
- [ ] Add metrics for hook execution latency and error rates
- [ ] Verify OTLP exporter settings (grpc/http) and env var docs
- [ ] Add smoke tests for metrics registry and OTLP initialization

### Phase 5: Documentation & Rollout
- [ ] Update user-facing docs for observability configuration
- [ ] Add troubleshooting steps for tracing/logging/metrics
- [ ] Add deployment checklist for OTLP endpoints and log sinks
- [ ] Provide a minimal “observability quickstart” example
- [ ] Define rollout steps and rollback criteria

### Phase 6: Verification
- [ ] Run `make ci` after instrumentation changes
- [ ] Perform manual trace capture for capture/recall, MCP, and hook flows
- [ ] Validate log/trace correlation end-to-end
- [ ] Confirm no sensitive data leaks in logs/traces

## Deliverables

- Expanded event bus coverage with documented schema.
- End-to-end traces for core flows.
- Consistent structured logging with correlation IDs.
- Metrics aligned to SLOs and exported via OTLP/Prometheus.

## Decisions

- [x] Initial SLO focus: **latency**
- [x] Highest-priority trace sampling flows: **capture and recall**
- [x] Separate event streams for security-sensitive events: **yes**
