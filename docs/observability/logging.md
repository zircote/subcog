# Logging Schema

## Required Fields

Structured logs MUST include the following fields:

- `timestamp`
- `level`
- `event`
- `message`
- `request_id`
- `component`
- `operation`

## Context Fields

Include when applicable:

- `memory_id`
- `namespace`
- `domain`
- `project_id`
- `branch`
- `file_path`
- `tool_name`
- `hook`
- `status`
- `error`

## Trace/Span Fields

When tracing is enabled, include:

- `trace_id`
- `span_id`
- `parent_span_id`

## Redaction Rules

- Never log raw memory content.
- Avoid logging secrets or free-form user input.
- Use hashes or truncated previews when needed.
