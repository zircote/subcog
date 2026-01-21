#!/bin/bash
# Test entity extraction via MCP server
# This script starts subcog serve, sends a test request, and checks the response

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}=== Entity Extraction Test ===${NC}"

# Check for OPENAI_API_KEY
if [ -z "$OPENAI_API_KEY" ]; then
    echo -e "${RED}ERROR: OPENAI_API_KEY is not set${NC}"
    exit 1
fi

# Build first
echo -e "${YELLOW}Building subcog...${NC}"
cargo build --release 2>&1 | tail -3

SUBCOG_BIN="./target/release/subcog"
if [ ! -f "$SUBCOG_BIN" ]; then
    echo -e "${RED}ERROR: Binary not found at $SUBCOG_BIN${NC}"
    exit 1
fi

# Create a test using the MCP JSON-RPC protocol
# Start server in background, capture its stdio

echo -e "${YELLOW}Testing entity extraction via MCP...${NC}"

# Create the JSON-RPC request for entity extraction
REQUEST=$(cat <<'EOF'
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"subcog_extract_entities","arguments":{"content":"We decided to use PostgreSQL for the database because it has great JSONB support. John Smith approved this after reviewing Redis.","store":false}}}
EOF
)

echo -e "${YELLOW}Request:${NC}"
echo "$REQUEST" | jq .

# Run subcog serve and send the request
# We need to initialize the MCP connection first
INIT_REQUEST='{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}'

echo -e "${YELLOW}Sending to MCP server...${NC}"

# Use a temp file for the response
RESPONSE_FILE=$(mktemp)

# Send init + request to subcog serve via stdio
(echo "$INIT_REQUEST"; sleep 0.5; echo "$REQUEST") | \
    RUST_LOG=debug timeout 30 $SUBCOG_BIN serve 2>&1 | tee "$RESPONSE_FILE" || true

echo -e "${YELLOW}Response:${NC}"
cat "$RESPONSE_FILE"

# Check if we got a valid response
if grep -q '"entities"' "$RESPONSE_FILE"; then
    echo -e "${GREEN}SUCCESS: Entity extraction returned entities${NC}"
elif grep -q '"error"' "$RESPONSE_FILE"; then
    echo -e "${RED}ERROR: Entity extraction failed${NC}"
    grep '"error"' "$RESPONSE_FILE" | head -1
else
    echo -e "${YELLOW}UNKNOWN: Could not parse response${NC}"
fi

rm -f "$RESPONSE_FILE"
