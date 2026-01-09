#!/bin/bash
# E2E Metrics Test Script for Subcog
# This script exercises all components and validates metrics are properly captured

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SUBCOG_BIN="${SUBCOG_BIN:-./target/release/subcog}"
PUSH_GATEWAY="${PUSH_GATEWAY:-http://localhost:9091}"
CONFIG_FILE="${CONFIG_FILE:-./scripts/e2e-test.config.toml}"

# Metrics endpoint
METRICS_URL="${PUSH_GATEWAY}/metrics"

echo -e "${BLUE}=== Subcog E2E Metrics Test ===${NC}"
echo "Binary: $SUBCOG_BIN"
echo "Push Gateway: $PUSH_GATEWAY"
echo "Config: $CONFIG_FILE"
echo ""

# Check prerequisites
if ! command -v curl &> /dev/null; then
    echo -e "${RED}Error: curl is required${NC}"
    exit 1
fi

if ! command -v jq &> /dev/null; then
    echo -e "${YELLOW}Warning: jq not found, JSON output will be raw${NC}"
fi

if [ ! -f "$SUBCOG_BIN" ]; then
    echo -e "${YELLOW}Binary not found, building...${NC}"
    cargo build --release
fi

# Check push gateway is running by testing the push endpoint (not just scrape endpoint)
echo -e "${YELLOW}Checking push gateway availability...${NC}"

# First check if the metrics scrape endpoint responds
if ! curl -s --connect-timeout 5 "${METRICS_URL}" > /dev/null 2>&1; then
    echo -e "${RED}Error: Push gateway not accessible at ${PUSH_GATEWAY}${NC}"
    echo ""
    echo "The push gateway must be running for this test. Start it with:"
    echo "  docker-compose -f docker/docker-compose.observability.yml up -d pushgateway"
    echo ""
    echo "Or run all observability services:"
    echo "  docker-compose -f docker/docker-compose.observability.yml up -d"
    exit 1
fi

# Test the actual push endpoint with a dummy metric (must end with newline)
TEST_METRIC=$'e2e_test_probe 1\n'
PUSH_RESULT=$(curl -s -w "%{http_code}" -o /dev/null --connect-timeout 5 \
    -X POST "${PUSH_GATEWAY}/metrics/job/subcog_e2e_test" \
    -H "Content-Type: text/plain" \
    --data-binary "${TEST_METRIC}" 2>/dev/null)

if [ "$PUSH_RESULT" != "200" ] && [ "$PUSH_RESULT" != "202" ]; then
    echo -e "${RED}Error: Push gateway push endpoint not working (HTTP ${PUSH_RESULT})${NC}"
    echo ""
    echo "The scrape endpoint is accessible but push is failing."
    echo "This may indicate the push gateway is starting up or misconfigured."
    echo ""
    echo "Try restarting the push gateway:"
    echo "  docker-compose -f docker/docker-compose.observability.yml restart pushgateway"
    exit 1
fi

# Clean up test metric
curl -s -X DELETE "${PUSH_GATEWAY}/metrics/job/subcog_e2e_test" 2>/dev/null || true

echo -e "${GREEN}Push gateway is accessible and push endpoint is working${NC}"
echo ""

# Clear existing metrics for clean test
echo -e "${BLUE}Clearing existing subcog metrics...${NC}"
for instance in hooks-session-start hooks-user-prompt-submit hooks-post-tool-use hooks-pre-compact hooks-stop mcp; do
    curl -s -X DELETE "${PUSH_GATEWAY}/metrics/job/subcog/instance/${instance}" 2>/dev/null || true
done
echo ""

# Function to run hook with metrics enabled
run_hook() {
    local hook_type="$1"
    local input="$2"

    echo -e "${YELLOW}Running ${hook_type} hook...${NC}"
    echo "$input" | SUBCOG_CONFIG_PATH="$CONFIG_FILE" \
        SUBCOG_METRICS_ENABLED=true \
        SUBCOG_METRICS_PUSH_GATEWAY_ENDPOINT="${PUSH_GATEWAY}/metrics/job/subcog" \
        "$SUBCOG_BIN" hook "$hook_type" 2>&1 | head -5
    echo ""
}

# Function to run MCP tool
run_mcp_tool() {
    local tool_name="$1"
    shift

    echo -e "${YELLOW}Running MCP tool: ${tool_name}...${NC}"
    SUBCOG_CONFIG_PATH="$CONFIG_FILE" \
        SUBCOG_METRICS_ENABLED=true \
        SUBCOG_METRICS_PUSH_GATEWAY_ENDPOINT="${PUSH_GATEWAY}/metrics/job/subcog" \
        "$SUBCOG_BIN" "$@" 2>&1 | head -10
    echo ""
}

# === Test All Hook Types ===
echo -e "${BLUE}=== Testing Hook Types ===${NC}"
echo ""

# 1. SessionStart hook
run_hook "session-start" '{
    "hookName": "SessionStart",
    "hookSpecificData": {}
}'

# 2. UserPromptSubmit hook (with search intent detection)
run_hook "user-prompt-submit" '{
    "hookName": "UserPromptSubmit",
    "hookSpecificData": {
        "userPromptContent": "How do I implement authentication in Rust?"
    }
}'

# 3. Another UserPromptSubmit (different intent)
run_hook "user-prompt-submit" '{
    "hookName": "UserPromptSubmit",
    "hookSpecificData": {
        "userPromptContent": "Where is the configuration file located?"
    }
}'

# 4. PostToolUse hook
run_hook "post-tool-use" '{
    "hookName": "PostToolUse",
    "hookSpecificData": {
        "toolName": "read_file",
        "toolResult": "File content here"
    }
}'

# 5. Another PostToolUse hook
run_hook "post-tool-use" '{
    "hookName": "PostToolUse",
    "hookSpecificData": {
        "toolName": "write_file",
        "toolResult": "success"
    }
}'

# 6. PreCompact hook (with decision content to trigger auto-capture)
run_hook "pre-compact" '{
    "sections": [
        {"role": "user", "content": "We need to decide on a database for metrics storage."},
        {"role": "assistant", "content": "We decided to use PostgreSQL with TimescaleDB extension for time-series metrics. This architectural decision will provide the scalability we need."},
        {"role": "user", "content": "Good idea. Lets use PostgreSQL with TimescaleDB for our metrics backend."},
        {"role": "assistant", "content": "The decision to use PostgreSQL with TimescaleDB provides excellent compression and retention policies for metrics. I have selected the hypertable approach for automatic partitioning."}
    ]
}'

# 7. Another PreCompact hook (with learning content)
run_hook "pre-compact" '{
    "sections": [
        {"role": "user", "content": "Why are the Grafana panels showing no data?"},
        {"role": "assistant", "content": "TIL that the OTLP receiver was bound to 127.0.0.1 instead of 0.0.0.0, so external containers could not reach it. This is a common gotcha with containerized services."},
        {"role": "user", "content": "That fixed it! Thanks for the debugging help."},
        {"role": "assistant", "content": "I learned that you should always bind container services to 0.0.0.0 for inter-container communication. This is a key caveat when running services in Docker."}
    ]
}'

# 8. Stop hook
run_hook "stop" '{
    "hookName": "Stop",
    "hookSpecificData": {}
}'

# === Test MCP Server (for resource reads metrics) ===
echo -e "${BLUE}=== Testing MCP Server ===${NC}"
echo ""

echo -e "${YELLOW}Sending MCP JSON-RPC requests...${NC}"

# Send multiple JSON-RPC requests through stdin using subshell
{
  # Initialize
  echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"e2e-test","version":"1.0.0"}}}'
  sleep 0.3

  # List resources
  echo '{"jsonrpc":"2.0","id":2,"method":"resources/list"}'
  sleep 0.3

  # Read a resource
  echo '{"jsonrpc":"2.0","id":3,"method":"resources/read","params":{"uri":"subcog://topics"}}'
  sleep 0.3

  # List tools
  echo '{"jsonrpc":"2.0","id":4,"method":"tools/list"}'
  sleep 0.3

  # Call subcog_status tool
  echo '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"subcog_status","arguments":{}}}'
  sleep 0.3

  # Call subcog_recall tool
  echo '{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"subcog_recall","arguments":{"query":"metrics test"}}}'
  sleep 0.3

  # Call subcog_namespaces tool
  echo '{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"subcog_namespaces","arguments":{}}}'
  sleep 0.3

  # List prompts
  echo '{"jsonrpc":"2.0","id":8,"method":"prompts/list"}'
  sleep 0.5

} | SUBCOG_CONFIG_PATH="$CONFIG_FILE" \
    SUBCOG_METRICS_PORT=0 \
    SUBCOG_METRICS_PUSH_GATEWAY_ENDPOINT="${PUSH_GATEWAY}/metrics/job/subcog/instance/mcp" \
    timeout 15 "$SUBCOG_BIN" serve 2>&1 | head -30

echo ""
echo -e "${GREEN}MCP server test completed${NC}"
echo ""

# === Test CLI Commands (for storage/recall metrics) ===
echo -e "${BLUE}=== Testing CLI Commands ===${NC}"
echo ""

# Capture a memory
echo -e "${YELLOW}Capturing test memories...${NC}"
SUBCOG_CONFIG_PATH="$CONFIG_FILE" \
    SUBCOG_METRICS_ENABLED=true \
    SUBCOG_METRICS_PUSH_GATEWAY_ENDPOINT="${PUSH_GATEWAY}/metrics/job/subcog" \
    "$SUBCOG_BIN" capture --namespace decisions "E2E test: Use Prometheus for metrics" 2>&1 | head -5

SUBCOG_CONFIG_PATH="$CONFIG_FILE" \
    SUBCOG_METRICS_ENABLED=true \
    SUBCOG_METRICS_PUSH_GATEWAY_ENDPOINT="${PUSH_GATEWAY}/metrics/job/subcog" \
    "$SUBCOG_BIN" capture --namespace learnings "E2E test: Push gateway requires trailing newline" 2>&1 | head -5
echo ""

# Recall memories
echo -e "${YELLOW}Recalling memories...${NC}"
SUBCOG_CONFIG_PATH="$CONFIG_FILE" \
    SUBCOG_METRICS_ENABLED=true \
    SUBCOG_METRICS_PUSH_GATEWAY_ENDPOINT="${PUSH_GATEWAY}/metrics/job/subcog" \
    "$SUBCOG_BIN" recall "metrics" 2>&1 | head -10
echo ""

# Check status
echo -e "${YELLOW}Checking status...${NC}"
SUBCOG_CONFIG_PATH="$CONFIG_FILE" \
    SUBCOG_METRICS_ENABLED=true \
    SUBCOG_METRICS_PUSH_GATEWAY_ENDPOINT="${PUSH_GATEWAY}/metrics/job/subcog" \
    "$SUBCOG_BIN" status 2>&1 | head -15
echo ""

# === Verify Metrics ===
echo -e "${BLUE}=== Verifying Metrics in Push Gateway ===${NC}"
echo ""

# Function to check metric exists
check_metric() {
    local metric_name="$1"
    # Count data lines only (not TYPE/HELP comments), matching metric name followed by { or space
    local count
    count=$(curl -s "${METRICS_URL}" 2>/dev/null | grep -E "^${metric_name}(\{| )" | wc -l | tr -d ' ')
    if [ "$count" -gt 0 ]; then
        echo -e "${GREEN}✓ ${metric_name}: ${count} time series${NC}"
        return 0
    else
        echo -e "${RED}✗ ${metric_name}: not found${NC}"
        return 1
    fi
}

# Track failures
failures=0

echo "Hook Metrics:"
check_metric "hook_executions_total" || ((failures++))
check_metric "hook_duration_ms" || ((failures++))
check_metric "hook_memory_lookup_total" || ((failures++))
check_metric "hook_auto_capture_total" || ((failures++))
echo ""

echo "LLM Metrics (optional - only when resilience wrapper used):"
check_metric "llm_requests_total" || echo "  (expected if using ResilientLlmClient)"
check_metric "llm_request_duration_ms" || echo "  (expected if using ResilientLlmClient)"
check_metric "llm_circuit_breaker_state" || echo "  (expected if using ResilientLlmClient)"
check_metric "llm_error_budget_ratio" || echo "  (expected if using ResilientLlmClient)"
echo ""

echo "Search Intent Metrics:"
check_metric "search_intent_llm_started" || ((failures++))
check_metric "search_intent_llm_completed" || ((failures++))
echo ""

echo "Storage Metrics:"
check_metric "storage_operations_total" || ((failures++))
check_metric "storage_operation_duration_ms" || ((failures++))
echo ""

echo "Memory Search Metrics:"
check_metric "memory_search_total" || ((failures++))
check_metric "memory_search_duration_ms" || ((failures++))
echo ""

# === Summary ===
echo -e "${BLUE}=== Summary ===${NC}"
echo ""

# Count unique instances
instances=$(curl -s "${METRICS_URL}" 2>/dev/null | grep -oE 'instance="[^"]+"' | sort -u | wc -l)
echo "Unique instances: $instances"

# Count total metrics
total_metrics=$(curl -s "${METRICS_URL}" 2>/dev/null | grep -cE "^[a-z_]+\{.*job=\"subcog\"" || echo "0")
echo "Total subcog metrics: $total_metrics"

# Show instance breakdown
echo ""
echo "Metrics by instance:"
curl -s "${METRICS_URL}" 2>/dev/null | grep 'job="subcog"' | grep -oE 'instance="[^"]+"' | sort | uniq -c | sort -rn

if [ "$failures" -gt 0 ]; then
    echo ""
    echo -e "${RED}FAILED: ${failures} metrics not found${NC}"
    exit 1
else
    echo ""
    echo -e "${GREEN}SUCCESS: All expected metrics found${NC}"
fi

# === Optional: Show raw metrics ===
if [ "$1" == "-v" ] || [ "$1" == "--verbose" ]; then
    echo ""
    echo -e "${BLUE}=== Raw Metrics (subcog only) ===${NC}"
    curl -s "${METRICS_URL}" 2>/dev/null | grep 'job="subcog"'
fi
