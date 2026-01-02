#!/bin/bash
# Test MCP server JSON-RPC interactions

set -e

CONFIG_FILE="${CONFIG_FILE:-./scripts/e2e-test.config.toml}"
PUSH_GATEWAY="${PUSH_GATEWAY:-http://localhost:9091}"

echo "=== Testing MCP Server ==="
echo "Config: $CONFIG_FILE"
echo ""

# Create a temp file for output
OUTFILE=$(mktemp)
trap "rm -f $OUTFILE" EXIT

# Send multiple JSON-RPC requests through stdin
{
  # Initialize
  echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"e2e-test","version":"1.0.0"}}}'
  sleep 0.3

  # List resources
  echo '{"jsonrpc":"2.0","id":2,"method":"resources/list"}'
  sleep 0.3

  # Read a resource
  echo '{"jsonrpc":"2.0","id":3,"method":"resources/read","params":{"uri":"subcog://namespaces"}}'
  sleep 0.3

  # List tools
  echo '{"jsonrpc":"2.0","id":4,"method":"tools/list"}'
  sleep 0.3

  # Call subcog_status tool
  echo '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"subcog_status","arguments":{}}}'
  sleep 0.3

  # Call subcog_recall tool
  echo '{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"subcog_recall","arguments":{"query":"test"}}}'
  sleep 0.3

  # Call subcog_namespaces tool
  echo '{"jsonrpc":"2.0","id":7,"method":"tools/list"}'
  sleep 0.3

  # List prompts
  echo '{"jsonrpc":"2.0","id":8,"method":"prompts/list"}'
  sleep 0.5

} | SUBCOG_CONFIG_FILE="$CONFIG_FILE" \
    SUBCOG_METRICS_PORT=0 \
    SUBCOG_METRICS_PUSH_GATEWAY_ENDPOINT="${PUSH_GATEWAY}/metrics/job/subcog/instance/mcp" \
    timeout 10 ./target/release/subcog serve 2>&1 | tee "$OUTFILE"

echo ""
echo "=== MCP Test Results ==="

# Count responses
RESPONSES=$(grep -c '"jsonrpc"' "$OUTFILE" || echo "0")
echo "Received $RESPONSES JSON-RPC responses"

# Check for errors
ERRORS=$(grep -c '"error"' "$OUTFILE" || echo "0")
if [ "$ERRORS" -gt 0 ]; then
    echo "Warning: $ERRORS responses contained errors"
fi

# Check metrics were pushed
METRICS_PUSHED=$(grep -c "Metrics pushed successfully" "$OUTFILE" || echo "0")
echo "Metrics pushed: $METRICS_PUSHED times"

echo ""
echo "=== Done ==="
