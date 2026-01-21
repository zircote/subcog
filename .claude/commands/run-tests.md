---
name: run-tests
description: Run automated functional tests for Subcog MCP tools. Execute the test suite to validate all MCP tool functionality including memory CRUD, search, knowledge graph, prompts, templates, and maintenance operations.
argument-hint: "[--category <name>] [--tag <tag>] [--verbose] [--dry-run] [--skip-cleanup]"
disable-model-invocation: true
allowed-tools: Read, Write, Bash, Glob, Grep, mcp__plugin_subcog_subcog__subcog_status, mcp__plugin_subcog_subcog__subcog_capture, mcp__plugin_subcog_subcog__subcog_recall, mcp__plugin_subcog_subcog__subcog_get, mcp__plugin_subcog_subcog__subcog_update, mcp__plugin_subcog_subcog__subcog_delete, mcp__plugin_subcog_subcog__subcog_entities, mcp__plugin_subcog_subcog__subcog_relationships, mcp__plugin_subcog_subcog__subcog_graph, mcp__plugin_subcog_subcog__subcog_prompts, mcp__plugin_subcog_subcog__subcog_templates, mcp__plugin_subcog_subcog__subcog_consolidate, mcp__plugin_subcog_subcog__subcog_enrich, mcp__plugin_subcog_subcog__subcog_reindex, mcp__plugin_subcog_subcog__subcog_gdpr_export
---

# /subcog:run-tests

Execute the automated Subcog functional test suite to validate all MCP tools.

## Usage

```
/subcog:run-tests [options]
```

### Options

- `--category <name>` - Run only tests in specified category
- `--tag <tag>` - Run only tests with specified tag
- `--skip-cleanup` - Don't run cleanup tests at the end
- `--verbose` - Show detailed output for each test
- `--dry-run` - Show tests that would run without executing

## Execution Strategy

<strategy>
**Test Orchestration Flow:**

1. Read test definitions from `tests/functional/tests.yaml`
2. Initialize test state in `.claude/test-state.json`
3. Execute tests sequentially, tracking results
4. Validate each test response against expected patterns
5. Generate summary report

**State Management:**
- State file tracks: current test, results, saved variables
- Each test can save output values for dependent tests
- Tests with `depends_on` wait for dependencies to pass
</strategy>

## Workflow

<workflow>
When invoked:

1. **Initialize Test Environment**
   - Read `tests/functional/tests.yaml`
   - Parse test definitions and resolve dependencies
   - Create `.claude/test-state.json` with initial state
   - Verify Subcog MCP server is available via `subcog_status`

2. **Execute Tests Sequentially**
   For each test:
   a. Check dependencies are satisfied
   b. Substitute saved variables in action string
   c. Present the test action to execute
   d. Wait for execution
   e. Validate response against `expect` rules
   f. Record pass/fail and save any `save_as` values
   g. Continue to next test

3. **Report Results**
   - Show pass/fail counts by category
   - List any failed tests with details
   - Provide cleanup status
   - Write report to `tests/functional/report.md`
</workflow>

## Test Format Reference

<format>
```yaml
tests:
  - id: unique_test_id
    description: "Human readable description"
    category: category_name
    action: |
      The exact prompt to execute.
      Can reference ${saved_variable} from previous tests.
    expect:
      - contains: "string that must appear"
      - not_contains: "string that must NOT appear"
      - regex: "pattern.*to.*match"
    tags: [tag1, tag2]
    save_as: variable_name  # Save memory_id or other output
    depends_on: other_test_id  # or [id1, id2]
    skip: false
```
</format>

## Categories

| Category | Description |
|----------|-------------|
| initialization | Session start and status checks |
| crud | Memory create, read, update, delete |
| search | Recall with different modes and filters |
| filters | GitHub-style filter syntax validation |
| entities | Knowledge graph entity operations |
| relationships | Entity relationship management |
| graph | Graph traversal and visualization |
| prompts | Prompt template management |
| templates | Context template management |
| maintenance | Consolidation, enrichment, reindex |
| privacy | GDPR compliance features |
| cleanup | Test data cleanup |

## Interactive Mode

When running tests, type:
- `next` or `n` - Execute next test
- `skip` or `s` - Skip current test
- `retry` or `r` - Retry failed test
- `abort` or `a` - Stop testing
- `status` - Show current progress
- `report` - Show results so far
