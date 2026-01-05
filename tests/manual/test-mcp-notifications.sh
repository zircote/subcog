#!/bin/bash
# ADR-0054-0058 MCP Notification Compliance - Manual Verification Script

PASS="✅"
FAIL="❌"
WAIT="⏸"

echo "============================================"
echo "ADR-0054-0058 MCP Notification Testing"
echo "============================================"
echo ""

echo "This test requires Claude Code or compatible MCP client."
echo ""

# Test 1: Start MCP server
echo "${WAIT} Test 1: Start MCP server"
read -p "Press ENTER to start subcog MCP server..."
echo "  Run in another terminal: cargo run --bin subcog -- serve"
echo "  Server listens on stdio"
read -p "Is server running? (y/n): " running
if [ "$running" != "y" ]; then
    echo "  ${FAIL} Start server first"
    exit 1
fi
echo "  ${PASS} Server started"
echo ""

# Test 2: Send notification via MCP client
echo "${WAIT} Test 2: Send notifications/initialized"
echo "  MANUAL STEPS:"
echo "  1. Connect Claude Code to subcog MCP server"
echo "  2. Verify no error responses for notifications/initialized"
echo "  3. Check server logs for debug-level logging"
read -p "Did notifications work correctly (no responses)? (y/n): " notif_ok
if [ "$notif_ok" = "y" ]; then
    echo "  ${PASS} Notifications handled correctly"
else
    echo "  ${FAIL} Notification handling issue"
fi
echo ""

# Test 3: Verify HTTP transport (if implemented)
echo "${WAIT} Test 3: HTTP transport notification behavior"
echo "  If HTTP transport is implemented:"
echo "  - Use an MCP client that supports streamable HTTP (SSE)"
echo "  - Send notifications/initialized"
echo "  - Verify no response messages are emitted for notifications"
read -p "Did HTTP notifications suppress responses? (y/skip): " http_ok
if [ "$http_ok" = "y" ]; then
    echo "  ${PASS} HTTP notifications suppressed"
elif [ "$http_ok" = "skip" ]; then
    echo "  ${WAIT} HTTP transport not tested"
fi
echo ""

# Test 4: Error responses include ID
echo "${WAIT} Test 4: Error responses include id field"
echo "  Send invalid request via MCP client:"
echo "  {\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"invalid_method\"}"
echo ""
echo "  Expected response:"
echo "  {\"jsonrpc\":\"2.0\",\"id\":1,\"error\":{...}}"
read -p "Did error response include id field? (y/n): " id_ok
if [ "$id_ok" = "y" ]; then
    echo "  ${PASS} Error responses include id"
else
    echo "  ${FAIL} ID missing from error response"
fi
echo ""

# Test 5: Parse error with null ID
echo "${WAIT} Test 5: Parse error returns id:null"
echo "  Send malformed JSON"
echo "  Expected: {\"jsonrpc\":\"2.0\",\"id\":null,\"error\":{...}}"
read -p "Did parse error include id:null? (y/n): " null_ok
if [ "$null_ok" = "y" ]; then
    echo "  ${PASS} Parse errors include id:null"
else
    echo "  ${FAIL} ID handling incorrect"
fi
echo ""

echo "============================================"
echo "Test Summary"
echo "============================================"
echo "MCP Notification Compliance:"
echo "- ADR-0054: Notification detection via id field absence"
echo "- ADR-0055: Empty string return for notifications"
echo "- ADR-0056: Always include id in error responses"
echo "- ADR-0057: HTTP notifications do not emit responses"
echo "- ADR-0058: Debug-level logging"
echo ""
echo "All features implemented in src/mcp/server.rs"
echo "Verified by unit tests, but manual MCP client testing recommended"
