# Manual Test Scripts

These interactive scripts guide you through manual verification of ADR implementations.

## Usage

```bash
cd tests/manual
chmod +x *.sh

# Run each test script
./test-tombstone.sh        # ADR-0053 Tombstone Pattern
./test-org-scope.sh         # ADR-0051 Org-Scope Feature Gate
./test-mcp-notifications.sh # ADR-0054-0058 MCP Notifications
```

## What Each Script Tests

### test-tombstone.sh (ADR-0053)
- Creates memory in feature branch
- Deletes branch
- Verifies lazy GC tombstones the memory
- Tests tombstone filtering (hidden by default)
- Tests --include-tombstoned flag
- Tests gc/purge commands

### test-org-scope.sh (ADR-0051)
- Verifies org-scope disabled by default
- Tests SUBCOG_ORG_SCOPE_ENABLED env var
- Tests invalid values handled gracefully
- Verifies boolean parsing (true/false/1/0/yes/no)

### test-mcp-notifications.sh (ADR-0054-0058)
- Tests MCP server notification handling
- Verifies notifications don't receive responses
- Verifies HTTP 204 No Content (if HTTP transport used)
- Verifies error responses include id field
- Verifies parse errors return id:null

## Marking Tests Complete

After running each script and verifying all tests pass, update:
`docs/spec/active/adr-audit-findings.md`

Mark the corresponding tasks as complete with actual verification notes.

## Expected Results

All tests should pass. If any fail, it indicates:
1. Lazy GC not wired into RecallService (expected - ADR-0052 implementation partial)
2. CLI flags not implemented yet (--include-tombstoned, gc commands)
3. Org-scope initialization not implemented (expected - only flag exists)

The core implementation (data models, service logic) is complete and tested via unit tests.
