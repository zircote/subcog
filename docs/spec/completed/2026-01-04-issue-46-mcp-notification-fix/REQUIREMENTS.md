---
document_type: requirements
project_id: SPEC-2026-01-04-001
version: 1.0.0
last_updated: 2026-01-04T04:30:00Z
status: draft
---

# MCP Server JSON-RPC Notification Compliance - Product Requirements Document

## Executive Summary

The subcog MCP server violates JSON-RPC 2.0 specification in its handling of notifications. This causes compatibility issues with standard MCP clients, particularly Python clients using pydantic for message validation. The fix requires detecting notifications (messages without `id`) and suppressing responses for them, plus ensuring any error responses include the required `id` field.

## Problem Statement

### The Problem

The MCP server sends responses to JSON-RPC notifications, which violates the protocol specification. Additionally, error responses are missing the required `id` field, making them unparseable by strict JSON-RPC clients.

### Impact

- **Immediate**: Python MCP clients (mcp 1.25.0+) fail with pydantic validation errors
- **Broader**: Any JSON-RPC 2.0 compliant client may have issues parsing responses
- **Trust**: Protocol non-compliance undermines confidence in the MCP implementation

### Current State

When the server receives `notifications/initialized`:

```json
{"jsonrpc":"2.0","method":"notifications/initialized"}
```

It responds with:

```json
{"jsonrpc":"2.0","error":{"code":-32601,"message":"Method not found: notifications/initialized"}}
```

This is wrong for two reasons:
1. No response should be sent for notifications
2. The error response is missing the `id` field

## Goals and Success Criteria

### Primary Goal

Make the MCP server fully compliant with JSON-RPC 2.0 notification handling.

### Success Metrics

| Metric | Target | Measurement Method |
|--------|--------|-------------------|
| Notification responses | 0 | No stdout output for notification messages |
| Error response validity | 100% | All error responses include `id` field |
| Client compatibility | Python mcp 1.25.0+ works | Integration test with Python client |
| Existing tests | All pass | `cargo test` |

### Non-Goals (Explicit Exclusions)

- Adding support for new MCP notification types (beyond fixing the handling)
- Implementing bidirectional notifications (server-to-client)
- Changing the transport layer (stdio/http)

## User Analysis

### Primary Users

- **Who**: Developers using MCP clients to connect to subcog
- **Needs**: Standard JSON-RPC 2.0 compliance for interoperability
- **Context**: Automated tooling, IDE integrations, AI agent frameworks

### User Stories

1. As an MCP client developer, I want the server to silently accept notifications so that my client doesn't receive unexpected responses.

2. As a Python developer using pydantic-based MCP clients, I want valid JSON-RPC error responses so that my client can parse them without validation errors.

3. As a user of any JSON-RPC 2.0 compliant client, I want the server to follow the specification so that I can rely on standard libraries.

## Functional Requirements

### Must Have (P0)

| ID | Requirement | Rationale | Acceptance Criteria |
|----|-------------|-----------|---------------------|
| FR-001 | Detect notifications by absence of `id` field | JSON-RPC 2.0 defines notifications as requests without `id` | Messages without `id` are classified as notifications |
| FR-002 | Suppress responses for notifications | JSON-RPC 2.0 spec: "The Server MUST NOT reply to a Notification" | No output written to stdout for notification messages |
| FR-003 | Include `id` in all error responses | JSON-RPC 2.0 spec requires `id` in responses | Error responses have `id` field (value from request, or `null` if unknown) |
| FR-004 | Handle `notifications/initialized` silently | MCP protocol sends this after initialize | No response, no error logged for this specific notification |

### Should Have (P1)

| ID | Requirement | Rationale | Acceptance Criteria |
|----|-------------|-----------|---------------------|
| FR-101 | Log notifications at debug level | Observability without noise | Debug log entry for received notifications |
| FR-102 | Track notification metrics | Monitor notification volume | `mcp_notifications_total` counter metric |

### Nice to Have (P2)

| ID | Requirement | Rationale | Acceptance Criteria |
|----|-------------|-----------|---------------------|
| FR-201 | Support additional MCP notification types | Future-proofing | Configurable list of known notification methods |

## Non-Functional Requirements

### Performance

- Notification detection must add <1ms latency to message processing
- No additional allocations for notification path (early return)

### Compatibility

- Must work with Python mcp client 1.25.0+
- Must work with any JSON-RPC 2.0 compliant client
- Must not break existing request/response handling

### Maintainability

- Clear separation between notification and request handling
- Unit tests for notification detection logic
- Integration test with real MCP client

## Technical Constraints

- Must work with existing `JsonRpcRequest` struct
- Must work with both stdio and http transports
- Cannot change the public API of the MCP server

## Dependencies

### Internal Dependencies

- `src/mcp/server.rs` - Main server implementation
- `src/mcp/dispatch.rs` - Method dispatch logic

### External Dependencies

- None (no new crates required)

## Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Breaking existing clients that depend on error responses | Low | Medium | Notification suppression is spec-compliant; clients shouldn't rely on errors |
| Missing other notification types | Medium | Low | Log unknown notifications at debug level for discovery |
| HTTP transport differences | Low | Medium | Test both transports explicitly |

## Open Questions

- [x] Are there other MCP notification types we should handle? → `notifications/initialized` is the primary one; others can be added later
- [x] Should we validate known notification types? → No, just suppress response for all messages without `id`

## Appendix

### Glossary

| Term | Definition |
|------|------------|
| Notification | JSON-RPC message without `id` field; fire-and-forget, no response expected |
| Request | JSON-RPC message with `id` field; expects a response |
| MCP | Model Context Protocol - protocol for AI model-tool communication |

### References

- [JSON-RPC 2.0 Specification](https://www.jsonrpc.org/specification)
- [MCP Protocol Documentation](https://modelcontextprotocol.io/)
- [GitHub Issue #46](https://github.com/zircote/subcog/issues/46)
