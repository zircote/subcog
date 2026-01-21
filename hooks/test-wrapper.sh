#!/usr/bin/env bash
#
# Test Mode Wrapper for Subcog Hooks
#
# This script wraps the normal subcog hook commands to add test mode awareness.
# When a test run is active, it intercepts specific commands (next, skip, etc.)
# and routes them to the test runner.
#
# Usage (called by hooks.json):
#   ./test-wrapper.sh user-prompt-submit
#   ./test-wrapper.sh stop
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
STATE_FILE="$PROJECT_ROOT/.claude/test-state.json"
RUNNER="$PROJECT_ROOT/tests/functional/runner.sh"

# Output a JSON replace response with properly escaped content
json_replace() {
  local content="$1"
  python3 -c "
import json
import sys
content = sys.stdin.read()
print(json.dumps({'replace': content}))
" <<<"$content"
}

# Check if test mode is active
is_test_mode() {
  if [[ ! -f "$STATE_FILE" ]]; then
    return 1
  fi

  local mode
  mode=$(python3 -c "
import json
with open('$STATE_FILE') as f:
    data = json.load(f)
print(data.get('mode', ''))
" 2>/dev/null || echo "")

  [[ "$mode" == "running" ]]
}

# Handle UserPromptSubmit hook
handle_user_prompt_submit() {
  # Read the hook input from stdin
  local input
  input=$(cat)

  # Extract the prompt from the input
  local prompt
  prompt=$(echo "$input" | python3 -c "
import json
import sys
try:
    data = json.load(sys.stdin)
    print(data.get('prompt', ''))
except:
    print('')
" 2>/dev/null || echo "")

  # Normalize prompt (lowercase, trim)
  local normalized
  normalized=$(echo "$prompt" | tr '[:upper:]' '[:lower:]' | xargs)

  # Check if we're in test mode
  if is_test_mode; then
    case "$normalized" in
    next | n)
      # Get next test and replace prompt with its action
      local next_output
      next_output=$("$RUNNER" next 2>/dev/null || echo "")

      if [[ -n "$next_output" ]]; then
        json_replace "Execute the following test:

$next_output"
        return 0
      fi
      ;;
    skip | s)
      local skip_output
      skip_output=$("$RUNNER" skip 2>/dev/null || echo "")
      json_replace "$skip_output"
      return 0
      ;;
    abort | a)
      local abort_output
      abort_output=$("$RUNNER" abort 2>/dev/null || echo "")
      json_replace "$abort_output"
      return 0
      ;;
    status)
      local status_output
      status_output=$("$RUNNER" status 2>/dev/null || echo "")
      json_replace "$status_output"
      return 0
      ;;
    report)
      local report_output
      report_output=$("$RUNNER" report 2>/dev/null || echo "")
      json_replace "$report_output"
      return 0
      ;;
    vars)
      local vars_output
      vars_output=$("$RUNNER" vars 2>/dev/null || echo "")
      json_replace "$vars_output"
      return 0
      ;;
    validate*)
      # Extract response text after "validate"
      local response_text="${prompt#validate }"
      response_text="${response_text#v }"
      local validate_output
      validate_output=$("$RUNNER" validate "$response_text" 2>/dev/null || echo "")
      json_replace "$validate_output"
      return 0
      ;;
    esac
  fi

  # Check for /run-tests command to initialize test mode
  if [[ "$normalized" == "/run-tests"* || "$normalized" == "/subcog:run-tests"* ]]; then
    # Parse any options
    local args=""
    if [[ "$prompt" == *"--category"* ]]; then
      local cat
      cat=$(echo "$prompt" | grep -oP '(?<=--category\s)[^\s]+' || echo "")
      [[ -n "$cat" ]] && args="$args --category $cat"
    fi
    if [[ "$prompt" == *"--tag"* ]]; then
      local tag
      tag=$(echo "$prompt" | grep -oP '(?<=--tag\s)[^\s]+' || echo "")
      [[ -n "$tag" ]] && args="$args --tag $tag"
    fi

    # Initialize test run
    local init_output
    # shellcheck disable=SC2086
    init_output=$("$RUNNER" init $args 2>/dev/null || echo "Test runner initialization failed")
    json_replace "$init_output"
    return 0
  fi

  # Not in test mode or not a test command - pass through to normal hook
  echo "$input" | subcog hook user-prompt-submit
}

# Handle Stop hook
handle_stop() {
  # Read the hook input from stdin
  local input
  input=$(cat)

  # If in test mode, we could capture the assistant's response for validation
  # For now, just pass through to normal hook
  echo "$input" | subcog hook stop
}

# Main
main() {
  local event="${1:-}"

  case "$event" in
  user-prompt-submit)
    handle_user_prompt_submit
    ;;
  stop)
    handle_stop
    ;;
  session-start)
    # Pass through to normal hook
    subcog hook session-start
    ;;
  post-tool-use)
    # Pass through to normal hook
    subcog hook post-tool-use
    ;;
  pre-compact)
    # Pass through to normal hook
    subcog hook pre-compact
    ;;
  *)
    echo "Unknown event: $event" >&2
    exit 1
    ;;
  esac
}

main "$@"
