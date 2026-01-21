#!/usr/bin/env bash
#
# Subcog Functional Test Runner
#
# This script orchestrates automated testing of Subcog MCP tools.
# It manages test state, validates responses, and generates reports.
#
# Usage:
#   ./runner.sh init [--category CAT] [--tag TAG]  # Initialize test run
#   ./runner.sh next                               # Get next test action
#   ./runner.sh validate "response text"           # Validate test response
#   ./runner.sh status                             # Show current status
#   ./runner.sh report                             # Generate report
#   ./runner.sh abort                              # Abort test run
#   ./runner.sh reset                              # Clear state
#
# State is stored in: .claude/test-state.json
# Tests are defined in: tests/functional/tests.yaml

set -euo pipefail

# Paths
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
STATE_FILE="$PROJECT_ROOT/.claude/test-state.json"
TESTS_FILE="$SCRIPT_DIR/tests.json"
REPORT_FILE="$SCRIPT_DIR/report.md"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Ensure .claude directory exists
mkdir -p "$PROJECT_ROOT/.claude"

#------------------------------------------------------------------------------
# Helper Functions
#------------------------------------------------------------------------------

log_info() {
  echo -e "${BLUE}[INFO]${NC} $*" >&2
}

log_success() {
  echo -e "${GREEN}[PASS]${NC} $*" >&2
}

log_error() {
  echo -e "${RED}[FAIL]${NC} $*" >&2
}

log_warn() {
  echo -e "${YELLOW}[WARN]${NC} $*" >&2
}

# Get test count from JSON
get_test_count() {
  python3 -c "
import json
with open('$TESTS_FILE') as f:
    data = json.load(f)
print(len(data.get('tests', [])))
"
}

# Get test by index
get_test() {
  local index="$1"
  python3 -c "
import json
with open('$TESTS_FILE') as f:
    data = json.load(f)
tests = data.get('tests', [])
if $index < len(tests):
    print(json.dumps(tests[$index]))
else:
    print('null')
"
}

# Read state
read_state() {
  if [[ -f "$STATE_FILE" ]]; then
    cat "$STATE_FILE"
  else
    echo '{}'
  fi
}

# Write state
write_state() {
  echo "$1" >"$STATE_FILE"
}

# Get state field (returns JSON for complex objects)
get_state_field() {
  local field="$1"
  local state
  state=$(read_state)
  echo "$state" | python3 -c "
import json
import sys
data = json.load(sys.stdin)
val = data.get('$field', '')
if isinstance(val, (dict, list)):
    print(json.dumps(val))
else:
    print(val)
"
}

# Update state field
update_state() {
  local field="$1"
  local value="$2"
  local state
  state=$(read_state)
  echo "$state" | python3 -c "
import json
import sys
data = json.load(sys.stdin)
# Handle different value types
val = '''$value'''
try:
    val = json.loads(val)
except:
    pass
data['$field'] = val
print(json.dumps(data, indent=2))
" >"$STATE_FILE"
}

# Append to results array
append_result() {
  local result_json="$1"
  local state
  state=$(read_state)
  echo "$state" | python3 -c "
import json
import sys
data = json.load(sys.stdin)
if 'results' not in data:
    data['results'] = []
result = json.loads('''$result_json''')
data['results'].append(result)
print(json.dumps(data, indent=2))
" >"$STATE_FILE"
}

# Set saved variable
set_saved_var() {
  local var_name="$1"
  local var_value="$2"
  local state
  state=$(read_state)
  echo "$state" | python3 -c "
import json
import sys
data = json.load(sys.stdin)
if 'saved_vars' not in data:
    data['saved_vars'] = {}
data['saved_vars']['$var_name'] = '$var_value'
print(json.dumps(data, indent=2))
" >"$STATE_FILE"
}

# Get saved variable
get_saved_var() {
  local var_name="$1"
  local state
  state=$(read_state)
  echo "$state" | python3 -c "
import json
import sys
data = json.load(sys.stdin)
print(data.get('saved_vars', {}).get('$var_name', ''))
"
}

# Substitute variables in string
substitute_vars() {
  local text="$1"
  local state
  state=$(read_state)
  echo "$state" | python3 -c "
import json
import sys
import re
data = json.load(sys.stdin)
text = '''$text'''
saved_vars = data.get('saved_vars', {})
for var_name, var_value in saved_vars.items():
    text = text.replace(f'\${{{var_name}}}', str(var_value))
print(text)
"
}

#------------------------------------------------------------------------------
# Commands
#------------------------------------------------------------------------------

# Initialize a new test run
cmd_init() {
  local category=""
  local tag=""

  # Parse arguments
  while [[ $# -gt 0 ]]; do
    case "$1" in
    --category)
      category="$2"
      shift 2
      ;;
    --tag)
      tag="$2"
      shift 2
      ;;
    *)
      shift
      ;;
    esac
  done

  if [[ ! -f "$TESTS_FILE" ]]; then
    log_error "Tests file not found: $TESTS_FILE"
    exit 1
  fi

  local total_tests
  total_tests=$(get_test_count)

  local now
  now=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

  # Build initial state
  local state
  state=$(python3 -c "
import json
print(json.dumps({
    'mode': 'running',
    'total_tests': $total_tests,
    'current_index': 0,
    'current_test': None,
    'results': [],
    'saved_vars': {},
    'filter_category': '$category' if '$category' else None,
    'filter_tag': '$tag' if '$tag' else None,
    'started_at': '$now',
    'completed_at': None
}, indent=2))
")

  write_state "$state"

  log_info "Test run initialized"
  log_info "Total tests: $total_tests"
  [[ -n "$category" ]] && log_info "Category filter: $category"
  [[ -n "$tag" ]] && log_info "Tag filter: $tag"

  # Output for Claude
  echo "# Subcog Functional Test Suite"
  echo ""
  echo "**Total tests:** $total_tests"
  [[ -n "$category" ]] && echo "**Category filter:** $category"
  [[ -n "$tag" ]] && echo "**Tag filter:** $tag"
  echo ""
  echo "Type **next** to begin testing."
}

# Get and present the next test
cmd_next() {
  local mode
  mode=$(get_state_field "mode")

  if [[ "$mode" != "running" ]]; then
    echo "Test run is not active (mode: $mode). Run 'init' first."
    return 1
  fi

  local current_index
  current_index=$(get_state_field "current_index")

  local total_tests
  total_tests=$(get_state_field "total_tests")

  if [[ "$current_index" -ge "$total_tests" ]]; then
    update_state "mode" "complete"
    update_state "completed_at" "$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
    echo "All tests complete! Run 'report' to see results."
    return 0
  fi

  # Get test at current index
  local test_json
  test_json=$(get_test "$current_index")

  if [[ "$test_json" == "null" ]]; then
    log_error "Could not load test at index $current_index"
    return 1
  fi

  # Parse test details
  local test_id test_desc test_action test_category
  test_id=$(echo "$test_json" | python3 -c "import json,sys; print(json.load(sys.stdin).get('id',''))")
  test_desc=$(echo "$test_json" | python3 -c "import json,sys; print(json.load(sys.stdin).get('description',''))")
  test_action=$(echo "$test_json" | python3 -c "import json,sys; print(json.load(sys.stdin).get('action',''))")
  test_category=$(echo "$test_json" | python3 -c "import json,sys; print(json.load(sys.stdin).get('category',''))")

  # Check filters
  local filter_category filter_tag
  filter_category=$(get_state_field "filter_category")
  filter_tag=$(get_state_field "filter_tag")

  # TODO: Implement filter skipping logic

  # Substitute variables in action
  test_action=$(substitute_vars "$test_action")

  # Update current test in state
  update_state "current_test" "$test_json"

  # Calculate progress
  local progress_pct
  progress_pct=$(((current_index * 100) / total_tests))

  # Output test for Claude
  local test_num=$((current_index + 1))
  echo "## Test $test_num/$total_tests: $test_id"
  echo ""
  echo "**Category:** $test_category"
  echo "**Description:** $test_desc"
  echo ""
  echo "### Action"
  echo ""
  echo "$test_action"
  echo ""
  echo "---"
  echo "*Progress: $test_num/$total_tests ($progress_pct%)*"
  echo ""
  echo "Execute the action above, then type **validate** with the result."
}

# Validate the response from the last test
cmd_validate() {
  local response="${1:-}"

  local mode
  mode=$(get_state_field "mode")

  if [[ "$mode" != "running" ]]; then
    echo "Test run is not active."
    return 1
  fi

  local current_test
  current_test=$(get_state_field "current_test")

  if [[ -z "$current_test" || "$current_test" == "null" ]]; then
    echo "No current test to validate."
    return 1
  fi

  # Parse test expectations
  local test_id expects save_as
  test_id=$(echo "$current_test" | python3 -c "import json,sys; print(json.load(sys.stdin).get('id',''))")

  # Validate against expectations
  local validation_result
  validation_result=$(python3 -c "
import json
import re
import sys

test = json.loads('''$current_test''')
response = '''$response'''

expects = test.get('expect', [])
failures = []
passes = []

for exp in expects:
    if isinstance(exp, dict):
        if 'contains' in exp:
            if exp['contains'] in response:
                passes.append(f\"contains '{exp['contains']}'\" )
            else:
                failures.append(f\"Missing: '{exp['contains']}'\")
        elif 'not_contains' in exp:
            if exp['not_contains'] not in response:
                passes.append(f\"not_contains '{exp['not_contains']}'\")
            else:
                failures.append(f\"Should not contain: '{exp['not_contains']}'\")
        elif 'regex' in exp:
            if re.search(exp['regex'], response, re.IGNORECASE):
                passes.append(f\"regex '{exp['regex']}'\")
            else:
                failures.append(f\"Pattern not found: '{exp['regex']}'\")

result = {
    'passed': len(failures) == 0,
    'passes': passes,
    'failures': failures,
    'test_id': test.get('id'),
    'save_as': test.get('save_as')
}
print(json.dumps(result))
")

  local passed
  passed=$(echo "$validation_result" | python3 -c "import json,sys; print(json.load(sys.stdin)['passed'])")

  local current_index
  current_index=$(get_state_field "current_index")

  # Try to extract memory_id or entity_id if save_as is specified
  local save_as
  save_as=$(echo "$validation_result" | python3 -c "import json,sys; print(json.load(sys.stdin).get('save_as') or '')")

  if [[ -n "$save_as" && "$passed" == "True" ]]; then
    # Extract ID from response
    local extracted_id
    extracted_id=$(echo "$response" | python3 -c "
import re
import sys
text = sys.stdin.read()
# Try common patterns
patterns = [
    r'memory_id[\":\s]+([a-f0-9]{12})',
    r'entity_id[\":\s]+([a-zA-Z0-9_-]+)',
    r'id[\":\s]+([a-f0-9]{12})',
    r'\"id\":\s*\"([^\"]+)\"',
]
for p in patterns:
    m = re.search(p, text, re.IGNORECASE)
    if m:
        print(m.group(1))
        break
")
    if [[ -n "$extracted_id" ]]; then
      set_saved_var "$save_as" "$extracted_id"
      log_info "Saved $save_as = $extracted_id"
    fi
  fi

  # Record result
  local result_json
  result_json=$(python3 -c "
import json
validation = json.loads('''$validation_result''')
print(json.dumps({
    'id': validation['test_id'],
    'status': 'pass' if validation['passed'] else 'fail',
    'passes': validation['passes'],
    'failures': validation['failures']
}))
")
  append_result "$result_json"

  # Advance to next test
  update_state "current_index" "$((current_index + 1))"
  update_state "current_test" "null"

  # Output result
  if [[ "$passed" == "True" ]]; then
    echo "## ✅ PASS: $test_id"
  else
    echo "## ❌ FAIL: $test_id"
    echo ""
    echo "**Failures:**"
    echo "$validation_result" | python3 -c "
import json,sys
data = json.load(sys.stdin)
for f in data['failures']:
    print(f'- {f}')
"
  fi

  echo ""
  echo "Type **next** to continue to the next test."
}

# Show current status
cmd_status() {
  local mode total current results_count
  mode=$(get_state_field "mode")
  total=$(get_state_field "total_tests")
  current=$(get_state_field "current_index")

  # Count results
  local state
  state=$(read_state)
  local pass_count fail_count skip_count
  pass_count=$(echo "$state" | python3 -c "
import json,sys
data = json.load(sys.stdin)
print(len([r for r in data.get('results',[]) if r.get('status')=='pass']))
")
  fail_count=$(echo "$state" | python3 -c "
import json,sys
data = json.load(sys.stdin)
print(len([r for r in data.get('results',[]) if r.get('status')=='fail']))
")
  skip_count=$(echo "$state" | python3 -c "
import json,sys
data = json.load(sys.stdin)
print(len([r for r in data.get('results',[]) if r.get('status')=='skip']))
")

  echo "# Test Run Status"
  echo ""
  echo "**Mode:** $mode"
  echo "**Progress:** $current / $total"
  echo ""
  echo "| Status | Count |"
  echo "|--------|-------|"
  echo "| ✅ Pass | $pass_count |"
  echo "| ❌ Fail | $fail_count |"
  echo "| ⏭️ Skip | $skip_count |"
}

# Generate final report
cmd_report() {
  local state
  state=$(read_state)

  python3 -c "
import json
import sys
from datetime import datetime

data = json.loads('''$state''')

results = data.get('results', [])
total = data.get('total_tests', 0)
passed = len([r for r in results if r.get('status') == 'pass'])
failed = len([r for r in results if r.get('status') == 'fail'])
skipped = len([r for r in results if r.get('status') == 'skip'])

# Calculate duration
started = data.get('started_at', '')
completed = data.get('completed_at', '')
duration = 'N/A'
if started and completed:
    try:
        start_dt = datetime.fromisoformat(started.replace('Z', '+00:00'))
        end_dt = datetime.fromisoformat(completed.replace('Z', '+00:00'))
        secs = (end_dt - start_dt).total_seconds()
        mins = int(secs // 60)
        secs = int(secs % 60)
        duration = f'{mins}m {secs}s'
    except:
        pass

print('# Subcog Functional Test Report')
print()
print(f'**Total:** {len(results)} | **Passed:** {passed} | **Failed:** {failed} | **Skipped:** {skipped}')
print(f'**Duration:** {duration}')
print()

if failed > 0:
    print('## Failed Tests')
    print()
    for r in results:
        if r.get('status') == 'fail':
            print(f\"### ❌ {r['id']}\")
            print()
            for f in r.get('failures', []):
                print(f'- {f}')
            print()

print('## All Results')
print()
print('| Test ID | Status |')
print('|---------|--------|')
for r in results:
    status_icon = '✅' if r['status'] == 'pass' else '❌' if r['status'] == 'fail' else '⏭️'
    print(f\"| {r['id']} | {status_icon} {r['status']} |\")
" | tee "$REPORT_FILE"

  log_info "Report saved to: $REPORT_FILE"
}

# Abort the test run
cmd_abort() {
  update_state "mode" "aborted"
  update_state "completed_at" "$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
  log_warn "Test run aborted."
  echo "Test run aborted. Run 'report' to see partial results."
}

# Reset/clear state
cmd_reset() {
  rm -f "$STATE_FILE"
  log_info "Test state cleared."
  echo "Test state has been reset."
}

# Skip current test
cmd_skip() {
  local current_test
  current_test=$(get_state_field "current_test")

  if [[ -z "$current_test" || "$current_test" == "null" ]]; then
    echo "No current test to skip."
    return 1
  fi

  local test_id
  test_id=$(echo "$current_test" | python3 -c "import json,sys; print(json.load(sys.stdin).get('id',''))")

  local current_index
  current_index=$(get_state_field "current_index")

  # Record skip
  append_result "{\"id\": \"$test_id\", \"status\": \"skip\", \"passes\": [], \"failures\": []}"

  # Advance
  update_state "current_index" "$((current_index + 1))"
  update_state "current_test" "null"

  echo "⏭️ Skipped: $test_id"
  echo ""
  echo "Type **next** to continue."
}

# Show saved variables
cmd_vars() {
  local state
  state=$(read_state)

  echo "# Saved Variables"
  echo ""
  echo "$state" | python3 -c "
import json,sys
data = json.load(sys.stdin)
vars = data.get('saved_vars', {})
if not vars:
    print('*No variables saved yet*')
else:
    for k, v in vars.items():
        print(f'- **{k}**: \`{v}\`')
"
}

#------------------------------------------------------------------------------
# Main
#------------------------------------------------------------------------------

main() {
  local cmd="${1:-help}"
  shift || true

  case "$cmd" in
  init)
    cmd_init "$@"
    ;;
  next | n)
    cmd_next
    ;;
  validate | v)
    cmd_validate "$*"
    ;;
  status | s)
    cmd_status
    ;;
  report | r)
    cmd_report
    ;;
  abort | a)
    cmd_abort
    ;;
  reset)
    cmd_reset
    ;;
  skip)
    cmd_skip
    ;;
  vars)
    cmd_vars
    ;;
  help | --help | -h)
    echo "Subcog Test Runner"
    echo ""
    echo "Commands:"
    echo "  init [--category CAT] [--tag TAG]  Initialize test run"
    echo "  next (n)                           Get next test"
    echo "  validate (v) \"response\"            Validate response"
    echo "  skip                               Skip current test"
    echo "  status (s)                         Show progress"
    echo "  report (r)                         Generate report"
    echo "  vars                               Show saved variables"
    echo "  abort (a)                          Abort test run"
    echo "  reset                              Clear state"
    echo "  help                               Show this help"
    ;;
  *)
    log_error "Unknown command: $cmd"
    echo "Run '$0 help' for usage."
    exit 1
    ;;
  esac
}

main "$@"
