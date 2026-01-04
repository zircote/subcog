#!/bin/bash
# ADR-0053 Tombstone Pattern - Manual Verification Script

set -e

SUBCOG="cargo run --bin subcog --"
PASS="✅"
FAIL="❌"
WAIT="⏸"

echo "=================================="
echo "ADR-0053 Tombstone Pattern Testing"
echo "=================================="
echo ""

# Test 1: Create memory in feature branch
echo "${WAIT} Test 1: Create memory in feature branch"
read -p "Press ENTER to create test branch and capture memory..."
git checkout -b test-tombstone-manual
MEMORY_ID=$($SUBCOG capture --namespace decisions "Test memory for manual tombstone verification" | grep "ID:" | awk '{print $2}')
echo "  Memory ID: $MEMORY_ID"
echo "  ${PASS} Memory captured"
echo ""

# Test 2: Verify memory exists
echo "${WAIT} Test 2: Verify memory exists in recall"
read -p "Press ENTER to search for the memory..."
$SUBCOG recall "tombstone verification" | grep "$MEMORY_ID" && echo "  ${PASS} Memory found in results" || echo "  ${FAIL} Memory NOT found"
echo ""

# Test 3: Delete branch
echo "${WAIT} Test 3: Delete feature branch"
read -p "Press ENTER to delete the test branch..."
git checkout develop
git branch -D test-tombstone-manual
echo "  ${PASS} Branch deleted"
echo ""

# Test 4: Trigger lazy GC (if implemented)
echo "${WAIT} Test 4: Trigger lazy GC by running recall"
echo "  Note: Lazy GC during recall is in ADR-0052"
read -p "Press ENTER to run recall (should trigger GC if implemented)..."
$SUBCOG recall "test memory" > /tmp/recall-output.txt
echo "  ${PASS} Recall executed"
echo ""

# Test 5: Check if memory was tombstoned
echo "${WAIT} Test 5: Verify memory status"
echo "  MANUAL CHECK:"
echo "  - Run: subcog status"
echo "  - Or check database: sqlite3 .subcog/index.db 'SELECT status FROM memories WHERE id=\"$MEMORY_ID\"'"
read -p "Was the memory tombstoned? (y/n): " tombstoned
if [ "$tombstoned" = "y" ]; then
    echo "  ${PASS} Memory was tombstoned"
else
    echo "  ${FAIL} Memory NOT tombstoned (lazy GC may not be wired up in RecallService)"
fi
echo ""

# Test 6: Verify tombstoned memories hidden by default
echo "${WAIT} Test 6: Verify tombstoned memories hidden in default search"
read -p "Press ENTER to search again..."
$SUBCOG recall "test" | grep "$MEMORY_ID" && echo "  ${FAIL} Tombstoned memory appears (should be hidden)" || echo "  ${PASS} Tombstoned memory correctly hidden"
echo ""

# Test 7: Test include-tombstoned flag (if CLI supports it)
echo "${WAIT} Test 7: Test --include-tombstoned flag"
echo "  Note: Flag may not be implemented in CLI yet"
read -p "Press ENTER to try recall with --include-tombstoned..."
$SUBCOG recall --include-tombstoned "test" 2>&1 | grep "$MEMORY_ID" && echo "  ${PASS} Tombstoned memory appears with flag" || echo "  ${WAIT} Flag not implemented or memory not found"
echo ""

# Test 8: Test GC command (if exists)
echo "${WAIT} Test 8: Test subcog gc command"
read -p "Press ENTER to try subcog gc..."
$SUBCOG gc 2>&1 || echo "  ${WAIT} GC command may not exist yet"
echo ""

# Test 9: Test purge command
echo "${WAIT} Test 9: Test subcog gc --purge"
read -p "Press ENTER to try purge command..."
$SUBCOG gc --purge --older-than=30d 2>&1 || echo "  ${WAIT} Purge command may not exist yet"
echo ""

echo "=================================="
echo "Test Summary"
echo "=================================="
echo "Verify the following worked correctly:"
echo "1. Memory captured in feature branch"
echo "2. Memory found in search"
echo "3. Branch deleted"
echo "4. Recall executed"
echo "5. Memory tombstoned (if lazy GC implemented)"
echo "6. Tombstoned memory hidden in default search"
echo "7. --include-tombstoned flag works (if implemented)"
echo "8. GC command exists"
echo "9. Purge command exists"
echo ""
echo "Any failures indicate incomplete implementation or missing CLI integration."
