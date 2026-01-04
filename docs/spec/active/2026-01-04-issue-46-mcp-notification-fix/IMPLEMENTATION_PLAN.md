---
document_type: implementation_plan
project_id: SPEC-2026-01-04-001
version: 1.0.0
last_updated: 2026-01-04T04:30:00Z
status: draft
estimated_effort: 2-4 hours
---

# MCP Server JSON-RPC Notification Compliance - Implementation Plan

## Overview

This is a focused bug fix with minimal scope. The implementation is straightforward and can be completed in a single phase with careful attention to both transports (stdio and http).

## Phase Summary

| Phase | Description | Tasks | Estimated Effort |
|-------|-------------|-------|------------------|
| Phase 1 | Implementation & Testing | 7 | 2-4 hours |

---

## Phase 1: Implementation & Testing

**Goal**: Fix notification handling and error response `id` field

### Tasks

#### Task 1.1: Add notification detection helper

- **Description**: Add `is_notification()` method to `McpServer`
- **File**: `src/mcp/server.rs`
- **Estimated Effort**: 15 minutes
- **Dependencies**: None
- **Acceptance Criteria**:
  - [ ] Method `is_notification(&JsonRpcRequest) -> bool` exists
  - [ ] Returns `true` when `id` is `None`
  - [ ] Returns `false` when `id` is `Some(_)`

#### Task 1.2: Update stdio transport to skip notification responses

- **Description**: Modify `process_request()` and stdio loop to suppress responses for notifications
- **File**: `src/mcp/server.rs`
- **Estimated Effort**: 30 minutes
- **Dependencies**: Task 1.1
- **Acceptance Criteria**:
  - [ ] `process_request()` returns empty string for notifications
  - [ ] Stdio loop skips `writeln!` when response is empty
  - [ ] Debug log emitted for notifications
  - [ ] Metric incremented for notifications

#### Task 1.3: Update HTTP transport to skip notification responses

- **Description**: Modify `handle_http_request()` to suppress responses for notifications
- **File**: `src/mcp/server.rs`
- **Estimated Effort**: 30 minutes
- **Dependencies**: Task 1.1
- **Acceptance Criteria**:
  - [ ] HTTP handler returns 204 No Content for notifications
  - [ ] Or returns empty body with 200 OK (TBD based on MCP spec)
  - [ ] Debug log emitted for notifications

#### Task 1.4: Fix error response `id` field

- **Description**: Ensure error responses always include `id` field (use `null` if unknown)
- **File**: `src/mcp/server.rs`
- **Estimated Effort**: 20 minutes
- **Dependencies**: None
- **Acceptance Criteria**:
  - [ ] `format_error()` always produces response with `id` field
  - [ ] Parse errors have `"id": null`
  - [ ] Request errors have `"id": <original_id>`

#### Task 1.5: Add unit tests

- **Description**: Add tests for notification detection and response suppression
- **File**: `src/mcp/server.rs`
- **Estimated Effort**: 30 minutes
- **Dependencies**: Tasks 1.1-1.4
- **Acceptance Criteria**:
  - [ ] Test `is_notification()` with/without id
  - [ ] Test notification returns empty response
  - [ ] Test error response includes `id: null`
  - [ ] All existing tests pass

#### Task 1.6: Add integration test

- **Description**: Test end-to-end notification handling
- **File**: `tests/mcp_notification_test.rs` or inline in `server.rs`
- **Estimated Effort**: 30 minutes
- **Dependencies**: Tasks 1.1-1.4
- **Acceptance Criteria**:
  - [ ] Send initialize request + notification, verify single response
  - [ ] Verify notification produces no output

#### Task 1.7: Verify with Python MCP client

- **Description**: Test with the actual Python mcp client that reported the issue
- **Estimated Effort**: 30 minutes
- **Dependencies**: Tasks 1.1-1.6
- **Acceptance Criteria**:
  - [ ] Python mcp 1.25.0 can connect without pydantic errors
  - [ ] Initialize handshake completes successfully

---

## Dependency Graph

```
Task 1.1 (notification detection)
    │
    ├──► Task 1.2 (stdio transport)
    │
    └──► Task 1.3 (http transport)

Task 1.4 (error id fix) ─────────────┐
                                     │
                                     ▼
                              Task 1.5 (unit tests)
                                     │
                                     ▼
                              Task 1.6 (integration test)
                                     │
                                     ▼
                              Task 1.7 (Python client test)
```

## Testing Checklist

- [ ] Unit tests for `is_notification()`
- [ ] Unit tests for notification response suppression
- [ ] Unit tests for error response `id` field
- [ ] Integration test: stdio transport notification handling
- [ ] Integration test: http transport notification handling (if http feature enabled)
- [ ] Manual test: Python mcp client connection
- [ ] All existing tests pass (`cargo test`)
- [ ] Clippy clean (`cargo clippy`)
- [ ] Format check (`cargo fmt -- --check`)

## Launch Checklist

- [ ] All tests passing
- [ ] `make ci` passes
- [ ] PR created with issue reference (Fixes #46)
- [ ] Code reviewed
- [ ] Merged to develop

## Risk Mitigation

| Risk | Mitigation Task |
|------|-----------------|
| Breaking existing clients | Unit tests verify request handling unchanged |
| HTTP transport differences | Explicit test for HTTP transport |
| Missing notification types | Debug logging reveals any unknown notifications |

## Post-Implementation

- [ ] Close GitHub issue #46
- [ ] Update CHANGELOG.md
- [ ] Consider documenting supported MCP notification types
