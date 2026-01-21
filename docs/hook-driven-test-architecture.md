# Hook-Driven Automated Test Framework for Claude Code

## Executive Summary

This document details a novel approach to automated testing of AI coding assistants using Claude Code's hook system. The framework transforms the conversational interface into an automated test harness, enabling systematic validation of MCP (Model Context Protocol) tools without manual intervention.

**Key Innovation**: Rather than testing from outside the AI session, this framework operates *within* the conversation, using hooks to intercept user prompts and inject test actions, creating a self-driving test loop.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                     Claude Code Session                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   User Types: "next"                                            │
│        │                                                        │
│        ▼                                                        │
│   ┌─────────────────────────────────────────────────────────┐   │
│   │           UserPromptSubmit Hook                         │   │
│   │   (hooks/test-wrapper.sh)                               │   │
│   │                                                         │   │
│   │   1. Detect test mode (check state file)                │   │
│   │   2. Intercept command (next/skip/validate/etc)         │   │
│   │   3. Call runner.sh with command                        │   │
│   │   4. Return {"replace": "new prompt"}                   │   │
│   └─────────────────────────────────────────────────────────┘   │
│        │                                                        │
│        ▼                                                        │
│   User prompt replaced with test action:                        │
│   "Execute the following test:                                  │
│    Call subcog_init with include_recall: true"                  │
│        │                                                        │
│        ▼                                                        │
│   Claude executes the MCP tool call                             │
│        │                                                        │
│        ▼                                                        │
│   User types: "validate <response>"                             │
│        │                                                        │
│        ▼                                                        │
│   Hook validates against expected patterns                      │
│   Records PASS/FAIL, advances to next test                      │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## Core Components

### 1. Test Definition Schema (`tests/functional/tests.json`)

The test suite is defined in a JSON file with the following structure:

```json
{
  "version": "1.0",
  "description": "Subcog MCP Server Functional Tests",
  "tests": [
    {
      "id": "unique_test_identifier",
      "description": "Human-readable test description",
      "category": "initialization|crud|search|filters|...",
      "action": "The prompt/instruction for Claude to execute",
      "expect": [
        { "contains": "text that must appear in response" },
        { "not_contains": "text that must NOT appear" },
        { "regex": "pattern.*to\\s+match" }
      ],
      "save": {
        "variable_name": "regex to capture (group 1)"
      },
      "tags": ["critical", "smoke", "regression"]
    }
  ]
}
```

**Validation Types**:

| Type | Purpose | Example |
|------|---------|---------|
| `contains` | Response must include this text | `"contains": "success"` |
| `not_contains` | Response must NOT include this text | `"not_contains": "error"` |
| `regex` | Response must match pattern | `"regex": "ID:\\s+(\\w+)"` |

**Variable Substitution**: Tests can reference saved variables using `${variable_name}` syntax, enabling dependent tests:

```json
{
  "id": "create_memory",
  "action": "Create a test memory",
  "save": { "memory_id": "ID:\\s+(\\w+)" }
},
{
  "id": "retrieve_memory",
  "action": "Retrieve memory ${memory_id}",
  "expect": [{ "contains": "content" }]
}
```

---

### 2. State Management (`.claude/test-state.json`)

Persistent state enables the framework to survive across hook invocations:

```json
{
  "mode": "running|completed|aborted",
  "total_tests": 53,
  "current_index": 5,
  "current_test": {
    "id": "test_id",
    "description": "...",
    "action": "...",
    "expect": [...]
  },
  "results": [
    {
      "id": "init_basic",
      "status": "pass|fail|skip",
      "failures": ["Missing: 'expected text'"],
      "timestamp": "2024-01-20T12:00:00Z"
    }
  ],
  "saved_vars": {
    "memory_id": "abc123",
    "entity_id": "def456"
  },
  "filter_category": null,
  "filter_tag": null,
  "started_at": "2024-01-20T12:00:00Z",
  "completed_at": null
}
```

---

### 3. Test Runner (`tests/functional/runner.sh`)

A bash script that orchestrates test execution with the following commands:

| Command | Description |
|---------|-------------|
| `init [--category X] [--tag Y]` | Initialize test run, optionally filter tests |
| `next` | Get next test action to execute |
| `validate "response"` | Validate response against expected patterns |
| `skip` | Skip current test |
| `status` | Show progress summary |
| `report` | Generate full test report |
| `vars` | Display saved variables |
| `abort` | Stop test run |

**Key Functions**:

```bash
# Get state field (returns JSON for complex objects)
get_state_field() {
  local field="$1"
  echo "$state" | python3 -c "
import json, sys
data = json.load(sys.stdin)
val = data.get('$field', '')
if isinstance(val, (dict, list)):
    print(json.dumps(val))
else:
    print(val)
"
}

# Validate response against expectations
cmd_validate() {
  local response="$1"
  local test_json=$(get_state_field "current_test")
  local expects=$(echo "$test_json" | python3 -c "
import json, sys, re
test = json.load(sys.stdin)
response = '''$response'''
failures = []

for exp in test.get('expect', []):
    if 'contains' in exp:
        if exp['contains'] not in response:
            failures.append(f\"Missing: '{exp['contains']}'\")
    if 'not_contains' in exp:
        if exp['not_contains'] in response:
            failures.append(f\"Unexpected: '{exp['not_contains']}'\")
    if 'regex' in exp:
        if not re.search(exp['regex'], response):
            failures.append(f\"Pattern not found: '{exp['regex']}'\")

print(json.dumps(failures))
")
  # ... record result and advance
}
```

---

### 4. Hook Wrapper (`hooks/test-wrapper.sh`)

The critical integration point that intercepts user prompts:

```bash
#!/usr/bin/env bash
# Hook wrapper that enables test mode

# JSON helper for proper escaping
json_replace() {
  local content="$1"
  python3 -c "
import json, sys
content = sys.stdin.read()
print(json.dumps({'replace': content}))
" <<< "$content"
}

# Check if test mode is active
is_test_mode() {
  [[ -f "$STATE_FILE" ]] || return 1
  local mode=$(python3 -c "
import json
with open('$STATE_FILE') as f:
    print(json.load(f).get('mode', ''))
")
  [[ "$mode" == "running" ]]
}

handle_user_prompt_submit() {
  local input=$(cat)
  local prompt=$(echo "$input" | python3 -c "
import json, sys
print(json.load(sys.stdin).get('prompt', ''))
")
  local normalized=$(echo "$prompt" | tr '[:upper:]' '[:lower:]' | xargs)

  if is_test_mode; then
    case "$normalized" in
      next|n)
        local output=$("$RUNNER" next)
        json_replace "Execute the following test:

$output"
        return 0
        ;;
      validate*)
        local response="${prompt#validate }"
        local output=$("$RUNNER" validate "$response")
        json_replace "$output"
        return 0
        ;;
      # ... other commands
    esac
  fi

  # Pass through to normal hook if not in test mode
  echo "$input" | subcog hook user-prompt-submit
}
```

---

### 5. Hook Configuration (`hooks/hooks.json`)

Registers the wrapper with Claude Code:

```json
{
  "hooks": {
    "UserPromptSubmit": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "${CLAUDE_PLUGIN_ROOT}/hooks/test-wrapper.sh user-prompt-submit"
          }
        ]
      }
    ]
  }
}
```

---

## Process Flow

### Test Execution Cycle

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│ /run-tests   │────▶│    init      │────▶│  State:      │
│              │     │              │     │  mode=running│
└──────────────┘     └──────────────┘     │  index=0     │
                                          └──────────────┘
                                                 │
        ┌────────────────────────────────────────┘
        ▼
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│ User: "next" │────▶│ Hook returns │────▶│ Claude sees: │
│              │     │ test action  │     │ "Call tool X"│
└──────────────┘     └──────────────┘     └──────────────┘
                                                 │
                                                 ▼
                                          ┌──────────────┐
                                          │ Claude calls │
                                          │ MCP tool     │
                                          └──────────────┘
                                                 │
        ┌────────────────────────────────────────┘
        ▼
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│ User:        │────▶│ Hook calls   │────▶│ Result:      │
│ "validate X" │     │ validate()   │     │ ✅ PASS or   │
└──────────────┘     └──────────────┘     │ ❌ FAIL      │
                                          └──────────────┘
                                                 │
                                                 ▼
                                          ┌──────────────┐
                                          │ Repeat until │
                                          │ all tests    │
                                          │ complete     │
                                          └──────────────┘
```

### Validation Logic

```python
def validate(response, expectations):
    failures = []

    for exp in expectations:
        if 'contains' in exp:
            if exp['contains'] not in response:
                failures.append(f"Missing: '{exp['contains']}'")

        if 'not_contains' in exp:
            if exp['not_contains'] in response:
                failures.append(f"Unexpected: '{exp['not_contains']}'")

        if 'regex' in exp:
            if not re.search(exp['regex'], response):
                failures.append(f"Pattern not found: '{exp['regex']}'")

    return 'pass' if not failures else 'fail', failures
```

---

## Usage Guide

### Starting a Test Run

```
/run-tests                    # Run all 53 tests
/run-tests --category crud    # Run only CRUD tests
/run-tests --tag critical     # Run only critical tests
```

### Interactive Commands

| Command | Shortcut | Action |
|---------|----------|--------|
| `next` | `n` | Display next test action |
| `skip` | `s` | Skip current test |
| `abort` | `a` | Stop test run |
| `status` | - | Show progress |
| `report` | - | Generate results |
| `vars` | - | Show saved variables |
| `validate <text>` | `v <text>` | Validate response |

### Example Session

```
User: /run-tests

Claude: # Subcog Functional Test Suite
        **Total tests:** 53
        Type **next** to begin testing.

User: next

Claude: ## Test 1/53: init_basic
        **Category:** initialization
        **Description:** subcog_init with defaults returns guidance and status

        ### Action
        Call subcog_init with include_recall: true, recall_limit: 5

User: [Executes the MCP tool call]

Claude: Session initialized successfully. Usage Guidance: ...
        System Status: healthy

User: validate Session initialized successfully. Usage Guidance shows. System Status: healthy

Claude: ## ✅ PASS: init_basic
        Type **next** to continue to the next test.

User: next
...

User: report

Claude: # Subcog Functional Test Report
        **Total:** 53 | **Passed:** 48 | **Failed:** 3 | **Skipped:** 2
```

---

## Key Implementation Insights

### 1. JSON Escaping is Critical

The hook must return valid JSON. Multiline content requires proper escaping:

```bash
# WRONG - breaks on newlines and special chars
echo "{\"replace\": \"$output\"}"

# CORRECT - Python handles all escaping
json_replace() {
  python3 -c "
import json, sys
print(json.dumps({'replace': sys.stdin.read()}))
" <<< "$1"
}
```

### 2. State Must Persist Between Invocations

Each hook invocation is stateless. Use a JSON file to maintain:
- Current test index
- Accumulated results
- Saved variables for dependent tests

### 3. Complex Objects Need Special Handling

When extracting nested objects from state, serialize properly:

```python
# WRONG - Python repr uses single quotes
print(data.get('current_test', ''))
# Output: {'id': 'test1', ...}  <- Invalid JSON!

# CORRECT - json.dumps for complex types
val = data.get('current_test', '')
if isinstance(val, (dict, list)):
    print(json.dumps(val))
else:
    print(val)
```

### 4. Hook Passthrough for Normal Operation

When not in test mode, pass input to normal hooks:

```bash
if is_test_mode; then
  # Handle test commands
else
  # Normal operation - pass through
  echo "$input" | subcog hook user-prompt-submit
fi
```

---

## Extending to Other Use Cases

This pattern can be adapted for:

### 1. API Integration Testing
Test any MCP server or API by defining expected request/response patterns.

### 2. Workflow Automation
Create guided multi-step workflows with validation at each step.

### 3. Training/Onboarding
Build interactive tutorials where the hook guides users through exercises.

### 4. Regression Testing
Automatically run through feature checks after deployments.

### 5. Compliance Validation
Verify AI responses meet specific criteria or policies.

---

## Files Reference

| File | Purpose |
|------|---------|
| `tests/functional/tests.json` | Test definitions (53 tests) |
| `tests/functional/runner.sh` | Test orchestration script |
| `hooks/test-wrapper.sh` | Hook integration wrapper |
| `hooks/hooks.json` | Hook registration |
| `.claude/test-state.json` | Runtime state (auto-created) |
| `tests/functional/report.md` | Generated test report |

---

## Conclusion

This hook-driven testing framework demonstrates a powerful capability of Claude Code: the ability to intercept and transform user input programmatically. By treating the conversation as a controllable interface, we can build sophisticated automation that would otherwise require external tooling.

The pattern is generalizable beyond testing—any scenario where you need to guide, validate, or transform the human-AI interaction can leverage this architecture.
