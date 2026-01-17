# Changelog

All notable changes to this specification will be documented in this file.

## [COMPLETED] - 2026-01-04

### Implementation Summary
- **Started**: 2026-01-04T15:15:00Z
- **Completed**: 2026-01-04T16:30:00Z
- **Duration**: ~1.5 hours (planned 2-4 hours, 38-63% under budget)
- **Outcome**: Success - All 7 tasks delivered, 100% scope completion
- **PR**: https://github.com/zircote/subcog/pull/47

### Deliverables
- Added `is_notification()` const fn to `JsonRpcRequest` (src/mcp/server.rs:1033)
- Updated stdio transport to skip writeln for notifications (src/mcp/server.rs:441-452)
- Updated HTTP transport to return 204 No Content (src/mcp/server.rs:1258-1282)
- Fixed `format_error()` to always include `id` field (src/mcp/server.rs:979-992)
- Added 12 new unit tests for Issue #46 compliance
- All 1019+ tests passing, `make ci` clean

### Verification Results
- Initialize request (id: 1) -> Response with id: 1 
- notifications/initialized -> NO RESPONSE (fixed!)
- ping request (id: 2) -> Response with id: 2 
- unknown method (id: 99) -> Error with id: 99 (fixed!)
- parse error -> Error with id: null (fixed!)

## [Unreleased]

## [2026-01-04]

### Approved
- Spec approved by Robert Allen <zircote@gmail.com>
- Ready for implementation via /claude-spec:implement

### Added
- Initial project creation
- REQUIREMENTS.md with full PRD
- ARCHITECTURE.md with technical design
- IMPLEMENTATION_PLAN.md with 7 tasks
- DECISIONS.md with 5 ADRs

### Research Conducted
- Analyzed existing MCP server implementation in `src/mcp/server.rs`
- Confirmed rmcp was not yet adopted at the time
- Identified exact locations for notification detection and response suppression
- Reviewed JSON-RPC 2.0 specification for compliance requirements
