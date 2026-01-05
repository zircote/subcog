# Tracing Conventions

## Span Naming

Use the following naming pattern for all spans:

- `subcog.{component}.{operation}`

Examples:
- `subcog.memory.capture`
- `subcog.memory.recall`
- `subcog.memory.consolidate`
- `subcog.gc.retention`
- `subcog.mcp.call_tool`
- `subcog.hook.user_prompt_submit`

For sub-spans, append a step name:

- `subcog.memory.capture.validate`
- `subcog.memory.capture.index`
- `subcog.memory.recall.search`
- `subcog.mcp.call_tool.execute`

## Required Attributes

Every span MUST include:

- `request_id`: Correlation identifier for end-to-end tracing
- `component`: Component name (e.g., `memory`, `mcp`, `hooks`, `gc`)
- `operation`: Operation name (e.g., `capture`, `recall`, `call_tool`)
- `status`: `success` or `error`

When applicable, include these attributes:

- `memory_id`
- `namespace`
- `domain`
- `tool_name`
- `hook`
- `transport`
- `error`

## Context Propagation

- Always attach spans to the current `request_id` context.
- Spawned tasks MUST inherit the current span/context.
