---
project_id: SPEC-2026-01-04-001
project_name: "MCP Server JSON-RPC Notification Compliance"
slug: issue-46-mcp-notification-fix
status: approved
created: 2026-01-04T04:30:00Z
approved: 2026-01-04T15:09:30Z
approved_by: "Robert Allen <zircote@gmail.com>"
started: null
completed: null
expires: 2026-04-04T04:30:00Z
superseded_by: null
tags: [mcp, json-rpc, notifications, bug-fix, protocol-compliance]
stakeholders: []
github_issue: 46
github_url: https://github.com/zircote/subcog/issues/46
---

# MCP Server JSON-RPC Notification Compliance

## Summary

Fix the MCP server's handling of JSON-RPC notifications to comply with the JSON-RPC 2.0 specification. Currently, the server incorrectly sends error responses to notifications (which should be fire-and-forget), and when it does send error responses, they're missing the required `id` field.

## Problem

1. **Notifications receive responses**: The server responds to `notifications/initialized` with an error, but per JSON-RPC 2.0 spec, notifications (requests without `id`) MUST NOT receive any response.

2. **Malformed error responses**: When the server does send an error response, it omits the `id` field entirely, making it invalid JSON-RPC. The spec requires `id` to be present (set to `null` if the original request's id couldn't be determined).

## Impact

- Python MCP clients using pydantic cannot parse the malformed error responses
- Breaks compatibility with standard MCP client implementations
- Violates JSON-RPC 2.0 specification

## Documents

- [REQUIREMENTS.md](./REQUIREMENTS.md) - Product requirements
- [ARCHITECTURE.md](./ARCHITECTURE.md) - Technical design
- [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md) - Phased tasks
- [DECISIONS.md](./DECISIONS.md) - Architecture decision records

## Quick Links

- GitHub Issue: [#46](https://github.com/zircote/subcog/issues/46)
- JSON-RPC 2.0 Spec: https://www.jsonrpc.org/specification
