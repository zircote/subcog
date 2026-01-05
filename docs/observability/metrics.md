# Metrics Conventions

## Naming

- Use `snake_case`.
- Prefix with domain (`memory_`, `mcp_`, `hook_`, `event_bus_`, `gc_`).
- Units in name for timers (`_duration_ms`).

## Required Labels

All metrics MUST include:

- `component`
- `operation`

When applicable, include:

- `status`
- `namespace`
- `domain`
- `hook_type`
- `tool_name`

## Cardinality Rules

- Avoid unbounded labels (raw queries, memory content).
- Use enums or bounded sets.
