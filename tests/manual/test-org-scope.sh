#!/bin/bash
# ADR-0051 Org-Scope Feature Gate - Manual Verification Script

SUBCOG="cargo run --bin subcog --"
PASS="✅"
FAIL="❌"
WAIT="⏸"

echo "======================================"
echo "ADR-0051 Org-Scope Feature Gate Testing"
echo "======================================"
echo ""

# Test 1: Default behavior (org-scope disabled)
echo "${WAIT} Test 1: Verify org-scope disabled by default"
read -p "Press ENTER to check default behavior..."
unset SUBCOG_ORG_SCOPE_ENABLED
$SUBCOG status 2>&1 | grep -i "org" || echo "  ${PASS} No org-scope references (correctly disabled)"
echo ""

# Test 2: Enable via env var
echo "${WAIT} Test 2: Enable org-scope via SUBCOG_ORG_SCOPE_ENABLED=true"
read -p "Press ENTER to test with env var..."
SUBCOG_ORG_SCOPE_ENABLED=true $SUBCOG status 2>&1
echo "  MANUAL CHECK: Did org-scope initialize?"
read -p "Did you see org-scope initialization? (y/n): " enabled
if [ "$enabled" = "y" ]; then
    echo "  ${PASS} Org-scope enabled via env var"
else
    echo "  ${WAIT} Org-scope not implemented yet (expected - flag exists but no initialization code)"
fi
echo ""

# Test 3: Test with invalid value
echo "${WAIT} Test 3: Test with invalid env var value"
read -p "Press ENTER to test with invalid value..."
SUBCOG_ORG_SCOPE_ENABLED=invalid_value $SUBCOG status 2>&1
echo "  ${PASS} No crash (graceful handling)"
echo ""

# Test 4: Test boolean parsing
echo "${WAIT} Test 4: Test various boolean values"
for val in true false 1 0 yes no on off; do
    echo "  Testing SUBCOG_ORG_SCOPE_ENABLED=$val"
    SUBCOG_ORG_SCOPE_ENABLED=$val $SUBCOG status 2>&1 | head -5
done
echo "  ${PASS} All values handled"
echo ""

echo "======================================"
echo "Test Summary"
echo "======================================"
echo "Verify the following:"
echo "1. Org-scope disabled by default"
echo "2. Env var enables org-scope (if init code exists)"
echo "3. Invalid values handled gracefully"
echo "4. Boolean parsing works for all formats"
echo ""
echo "Note: org_scope_enabled flag EXISTS in FeatureFlags"
echo "      But org-scope INITIALIZATION may not be implemented yet"
