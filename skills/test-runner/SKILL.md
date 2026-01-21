# Subcog Test Runner Skill

Orchestrate automated functional tests for Subcog MCP tools. Validates all documented functionality against expected behavior.

## Trigger Phrases

- "run subcog tests", "test subcog", "/run-tests"
- "validate subcog functionality", "functional tests"
- "run automated tests", "execute test suite"

## Quick Reference

| Mode | What It Does | Trigger |
|------|--------------|---------|
| Full | Run all tests | `/run-tests` |
| Category | Run specific category | `/run-tests --category crud` |
| Tag | Run tests with tag | `/run-tests --tag critical` |
| Dry-run | Show tests without running | `/run-tests --dry-run` |

## Execution Strategy

<strategy>
**Sequential Test Execution:**
1. Load test definitions from `tests/functional/tests.yaml`
2. Initialize state tracking in `.claude/test-state.json`
3. For each test:
   - Execute the action (MCP tool call)
   - Capture the response
   - Validate against expect rules
   - Record result and continue
4. Generate final report

**Key Behaviors:**
- Tests execute ONE AT A TIME for clear validation
- Each test waits for user confirmation ("next") before proceeding
- Failed tests are logged but don't block subsequent tests
- Variables saved from tests are substituted in dependent tests
</strategy>

## Workflow

<workflow>
When invoked:

### Phase 1: Initialization

1. **Verify Environment**
   ```
   Call subcog_status to verify MCP server is running
   ```

2. **Load Test Suite**
   - Read `tests/functional/tests.yaml`
   - Parse test definitions
   - Build dependency graph
   - Filter by category/tag if specified

3. **Initialize State**
   Write to `.claude/test-state.json`:
   ```json
   {
     "mode": "running",
     "total_tests": 50,
     "current_index": 0,
     "current_test": null,
     "results": [],
     "saved_vars": {},
     "started_at": "2024-01-20T12:00:00Z"
   }
   ```

4. **Show Test Plan**
   ```
   # Subcog Functional Test Suite

   Total tests: 50
   Categories: initialization (4), crud (10), search (6), ...

   Type 'next' to begin, 'skip' to skip a test, 'abort' to stop.
   ```

### Phase 2: Test Execution Loop

For each test:

1. **Present Test**
   ```
   ## Test 5/50: capture_decision
   Category: crud
   Description: Capture a decision memory

   Action:
   > Call subcog_capture with:
   >   content: "TEST_DECISION: Use YAML for test definitions"
   >   namespace: "decisions"
   >   tags: ["test", "automated", "cleanup-required"]

   Expected:
   - Response contains "memory_id"
   - Response matches regex "[a-f0-9]{12}"

   Type 'next' to execute...
   ```

2. **Execute Test Action**
   - Perform the MCP tool call
   - Capture full response

3. **Validate Response**
   For each expect rule:
   - `contains`: Check substring presence
   - `not_contains`: Check substring absence
   - `regex`: Check pattern match

   Mark test as PASS or FAIL

4. **Save Variables**
   If test has `save_as` and passes:
   - Extract value (memory_id, entity_id, etc.)
   - Store in `saved_vars` for dependent tests

5. **Record Result**
   ```json
   {
     "id": "capture_decision",
     "status": "pass",
     "duration_ms": 234,
     "saved": {"captured_decision_id": "abc123def456"}
   }
   ```

6. **Show Result**
   ```
   ✅ PASS: capture_decision (234ms)
   Saved: captured_decision_id = abc123def456

   Progress: 5/50 (10%)
   Type 'next' to continue...
   ```

### Phase 3: Report Generation

After all tests:

1. **Generate Summary**
   ```
   # Test Results Summary

   Total: 50 | Passed: 47 | Failed: 2 | Skipped: 1
   Duration: 3m 24s

   ## Failed Tests

   1. [FAIL] entity_extract
      Expected: regex "(Alice|Anthropic)"
      Actual: "LLM features disabled"

   2. [FAIL] relationship_infer
      Expected: contains "inferred"
      Actual: timeout after 30s

   ## Results by Category

   | Category | Pass | Fail | Skip |
   |----------|------|------|------|
   | initialization | 4 | 0 | 0 |
   | crud | 10 | 0 | 0 |
   | entities | 4 | 1 | 0 |
   ...
   ```

2. **Write Report**
   Save to `tests/functional/report.md`

3. **Cleanup Status**
   Report if cleanup tests ran successfully
</workflow>

## State File Schema

<state>
`.claude/test-state.json`:
```json
{
  "mode": "running|paused|complete|aborted",
  "total_tests": 50,
  "current_index": 5,
  "current_test": {
    "id": "capture_decision",
    "started_at": "2024-01-20T12:01:00Z"
  },
  "results": [
    {
      "id": "init_basic",
      "status": "pass|fail|skip",
      "duration_ms": 150,
      "error": null,
      "saved": {}
    }
  ],
  "saved_vars": {
    "captured_decision_id": "abc123def456",
    "test_person_id": "entity_xyz789"
  },
  "started_at": "2024-01-20T12:00:00Z",
  "completed_at": null
}
```
</state>

## Validation Rules

<validation>
**contains**: Case-sensitive substring match
```yaml
expect:
  - contains: "memory_id"
```

**not_contains**: Ensure string is absent
```yaml
expect:
  - not_contains: "error"
```

**regex**: Perl-compatible regex match
```yaml
expect:
  - regex: "[a-f0-9]{12}"
  - regex: "(pass|success|created)"
```

**All rules must pass** for test to succeed.
</validation>

## Variable Substitution

<variables>
Tests can reference saved variables:
```yaml
- id: get_memory
  action: "Call subcog_get with memory_id: '${captured_decision_id}'"
  depends_on: capture_decision
```

Variables are extracted from tool responses:
- `memory_id` from capture results
- `entity_id` from entity creation
- Custom patterns via regex capture groups
</variables>

## Commands During Test Run

| Command | Action |
|---------|--------|
| `next` / `n` | Execute current test and proceed |
| `skip` / `s` | Skip current test |
| `retry` / `r` | Re-run last failed test |
| `abort` / `a` | Stop testing, generate report |
| `status` | Show progress summary |
| `report` | Show results so far |
| `vars` | Show saved variables |

## MCP Tool Reference

<mcp>
**All Subcog MCP tools** are tested. Key ones:

- `subcog_init` - Session initialization
- `subcog_capture` - Memory creation
- `subcog_recall` - Memory search
- `subcog_get` / `subcog_update` / `subcog_delete` - CRUD
- `subcog_entities` - Knowledge graph entities
- `subcog_relationships` - Entity relationships
- `subcog_graph` - Graph operations
- `subcog_prompts` - Prompt templates
- `subcog_templates` - Context templates
- `subcog_consolidate` - Memory consolidation
- `subcog_gdpr_export` - Privacy compliance

**Other Tools:**
- `Read` - Load test definitions
- `Write` - Save state and reports
- `Bash` - Auxiliary operations
</mcp>

## Example Session

```
User: /run-tests --category crud