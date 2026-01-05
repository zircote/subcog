# Event Bus Reference

## Event Taxonomy

Subcog emits events grouped into the following categories:

- **System**: process lifecycle, shutdown, startup.
- **Memory lifecycle**: capture, recall, update, delete, tombstone, consolidation.
- **Security**: redaction, audit logging, access control.
- **Performance**: search latency, GC duration, embedding latency.
- **MCP**: server lifecycle, auth, tool execution.
- **Hooks**: invocation, classification, capture decision outcomes.
