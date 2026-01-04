---
document_type: decisions
project_id: SPEC-2026-01-04-001
---

# MCP Server JSON-RPC Notification Compliance - Architecture Decision Records

## ADR-001: Notification Detection via `id` Field Absence

**Date**: 2026-01-04
**Status**: Accepted
**Deciders**: Project maintainer

### Context

We need to distinguish between JSON-RPC requests (which expect responses) and notifications (which must not receive responses). The JSON-RPC 2.0 specification defines this distinction.

### Decision

Detect notifications by checking if the `id` field is absent (`None`) in the parsed `JsonRpcRequest`. This is the canonical method per JSON-RPC 2.0 spec.

### Consequences

**Positive:**
- Simple, spec-compliant detection
- No changes to `JsonRpcRequest` struct needed
- Works for all notification types, not just known ones

**Negative:**
- None identified

**Neutral:**
- Relies on serde correctly parsing missing `id` as `None`

### Alternatives Considered

1. **Check method name prefix** (`notifications/*`): Not reliable - other protocols may use different conventions, and it's not what the spec says.

2. **Maintain allowlist of notification methods**: Over-engineered for this use case and requires maintenance.

---

## ADR-002: Empty String Return for Notification Responses

**Date**: 2026-01-04
**Status**: Accepted
**Deciders**: Project maintainer

### Context

When a notification is received, we need to signal to the caller that no response should be sent. We need a mechanism that works with the existing code structure.

### Decision

Return an empty string (`String::new()`) from `process_request()` for notifications. The caller checks `if !response.is_empty()` before writing to stdout.

### Consequences

**Positive:**
- Minimal code change
- Clear semantics (empty = nothing to send)
- No changes to return type signature

**Negative:**
- Caller must remember to check for empty string
- Empty string could theoretically be a valid response (but isn't in JSON-RPC)

**Neutral:**
- Alternative would be `Option<String>` return type, but that's a larger refactor

### Alternatives Considered

1. **Return `Option<String>`**: Cleaner semantically, but requires changing all callers and the function signature.

2. **Throw/return error**: Incorrect - notifications are valid messages, not errors.

3. **Set a flag on the server**: Stateful and error-prone.

---

## ADR-003: Always Include `id` in Error Responses

**Date**: 2026-01-04
**Status**: Accepted
**Deciders**: Project maintainer

### Context

JSON-RPC 2.0 requires the `id` field in all response objects, including error responses. The current implementation uses `skip_serializing_if = "Option::is_none"` which omits `id` when it's `None`.

### Decision

Modify `format_error()` to always include `id`. When the original request's `id` is unknown (e.g., parse errors), use `Value::Null` explicitly.

```rust
id: Some(id.unwrap_or(Value::Null))
```

### Consequences

**Positive:**
- Compliant with JSON-RPC 2.0 specification
- Clients can parse error responses correctly
- Minimal code change

**Negative:**
- None identified

**Neutral:**
- Success responses can continue to omit `id` if `None` (though this shouldn't happen in practice for valid requests)

### Alternatives Considered

1. **Create separate `JsonRpcErrorResponse` struct**: Over-engineered; the fix is one line.

2. **Remove `skip_serializing_if` globally**: Would affect success responses unnecessarily.

---

## ADR-004: HTTP Transport Returns 204 for Notifications

**Date**: 2026-01-04
**Status**: Proposed
**Deciders**: Project maintainer

### Context

For the HTTP transport, we need to decide what HTTP response to send for notifications. Since no JSON-RPC response body is appropriate, we need an HTTP status that conveys "received, no content."

### Decision

Return HTTP 204 No Content for notifications over HTTP transport.

### Consequences

**Positive:**
- Semantically correct (the notification was received, there's no content to return)
- Standard HTTP practice
- Clients won't try to parse an empty body as JSON

**Negative:**
- Some clients might expect 200 OK for all successful operations

**Neutral:**
- Could alternatively return 200 with empty body, but 204 is more explicit

### Alternatives Considered

1. **Return 200 OK with empty body**: Valid, but less semantically precise.

2. **Return 200 OK with `{}` body**: Misleading - suggests there was a response.

---

## ADR-005: Debug-Level Logging for Notifications

**Date**: 2026-01-04
**Status**: Accepted
**Deciders**: Project maintainer

### Context

We need observability into notification handling without creating log noise in production.

### Decision

Log notifications at `debug` level with the method name. Add a counter metric `mcp_notifications_total` with method label.

### Consequences

**Positive:**
- Observability for debugging
- Metrics for monitoring notification volume
- No log noise at info/warn levels

**Negative:**
- Debug logs may be verbose in development

**Neutral:**
- Consistent with existing logging patterns in the codebase
