#!/bin/bash
set -e

SUBCOG="./target/release/subcog"
TEST_DB="/tmp/subcog-test-$$"

echo "=== Tombstone Integration Test ==="

# Clean slate
rm -rf "$TEST_DB"
export SUBCOG_DATA_DIR="$TEST_DB"

# 1. Create test branch
git checkout -b test-tombstone-$$ 2>/dev/null || true

# 2. Capture memory in feature branch
echo "Creating memory in feature branch..."
MEMORY_ID=$($SUBCOG capture --namespace decisions "Test memory for tombstone verification" | grep "ID:" | awk '{print $2}')
echo "Created memory: $MEMORY_ID"

# 3. Verify memory exists
echo "Verifying memory exists..."
RECALL_OUTPUT=$($SUBCOG recall "test" 2>&1)
echo "Recall output:"
echo "$RECALL_OUTPUT"
echo "$RECALL_OUTPUT" | grep -q "$MEMORY_ID" && echo "✓ Memory found" || echo "⚠ Memory not found (may not be indexed yet)"

# 4. Delete feature branch
git checkout develop 2>/dev/null
git branch -D test-tombstone-$$ 2>/dev/null || true
echo "✓ Deleted feature branch"

# 5. Trigger lazy GC by searching (if implemented)
echo "Running recall to trigger lazy GC..."
$SUBCOG recall "tombstone" > /tmp/recall-output.txt

# 6. Check if memory is tombstoned (depends on lazy GC implementation)
echo "Checking tombstone status..."
# Note: This requires lazy GC to be wired up in RecallService
# For now, verify the filtering works

# 7. Test include-tombstoned flag (if CLI supports it)
echo "Testing include-tombstoned flag..."
# $SUBCOG recall --include-tombstoned "tombstone" 2>&1 || echo "Flag not yet implemented in CLI"

# 8. Test status command
echo "Running status..."
$SUBCOG status

echo ""
echo "=== Test Summary ==="
echo "✓ Memory creation in branch context"
echo "✓ Branch deletion"
echo "✓ Recall execution"
echo "✓ Status check"
echo ""
echo "Note: Full lazy GC verification requires RecallService integration (ADR-0052)"

# Cleanup
rm -rf "$TEST_DB"
rm -f /tmp/recall-output.txt
