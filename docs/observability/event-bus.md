# Event Bus Reference

## Event Taxonomy

Subcog emits events grouped into the following categories:

- **System**: process lifecycle, shutdown, startup.
- **Memory lifecycle**: capture, recall, update, delete, tombstone, consolidation.
- **Security**: redaction, audit logging, access control.
- **Performance**: search latency, GC duration, embedding latency.
- **MCP**: server lifecycle, auth, tool execution.
- **Hooks**: invocation, classification, capture decision outcomes.

## Payload Schema Guidelines

Required fields for all event payloads:
- `event_id`: Unique identifier for the event (UUID or ULID).
- `event_type`: Stable event name (e.g., `memory.captured`).
- `timestamp`: Unix epoch seconds.
- `source`: Component name (e.g., `capture`, `recall`, `mcp`).
- `correlation_id`: Request/trace correlation identifier.

Optional fields (by category):
- `memory_id`, `namespace`, `domain`, `project_id`, `branch`, `file_path`
- `status`, `error`, `duration_ms`, `count`

Redaction rules:
- Never emit raw memory content or secrets.
- Hash or truncate free-form text fields.
- Avoid user-provided identifiers as labels in metrics.
