#!/bin/bash
# Test summary script for dns-proxy

echo "Running all tests and generating summary..."
echo "=========================================="
echo ""

# Run tests and capture output
TEST_OUTPUT=$(cargo test 2>&1)

# Extract test suite results
echo "📊 TEST SUITE SUMMARY"
echo "===================="
echo ""

# Process output line by line
CURRENT_SUITE=""
TOTAL_PASSED=0
TOTAL_FAILED=0
SHOW_SUITE=false
SUITE_COUNT=0

while IFS= read -r line; do
    # Detect test suite start
    if [[ $line =~ ^[[:space:]]+Running[[:space:]]+(.+) ]]; then
        CURRENT_SUITE="${BASH_REMATCH[1]}"
        CURRENT_SUITE="${CURRENT_SUITE%% (*}"
        SHOW_SUITE=false
    fi
    
    # Detect test result
    if [[ $line =~ test[[:space:]]+result:.*([0-9]+)[[:space:]]+passed\;[[:space:]]+([0-9]+)[[:space:]]+failed ]]; then
        PASSED="${BASH_REMATCH[1]}"
        FAILED="${BASH_REMATCH[2]}"
        
        # Only show test suites with actual tests
        if [ "$PASSED" -gt 0 ] || [ "$FAILED" -gt 0 ]; then
            if [ "$SHOW_SUITE" = false ]; then
                echo "📦 $CURRENT_SUITE"
                SHOW_SUITE=true
                SUITE_COUNT=$((SUITE_COUNT + 1))
            fi
            
            TOTAL_PASSED=$((TOTAL_PASSED + PASSED))
            TOTAL_FAILED=$((TOTAL_FAILED + FAILED))
            
            if [ "$FAILED" -gt 0 ]; then
                echo "   ❌ $PASSED passed, $FAILED failed"
            else
                echo "   ✅ $PASSED passed, $FAILED failed"
            fi
            echo ""
        fi
    fi
done <<< "$TEST_OUTPUT"

# Add suite count before summary
if [ "$SUITE_COUNT" -gt 0 ]; then
    echo "Total test suites: $SUITE_COUNT"
    echo ""
fi

# Extract failed tests if any
FAILED_TESTS=$(echo "$TEST_OUTPUT" | grep -A 5 "^failures:" | grep "^    test " | sed 's/^    //')

if [ -n "$FAILED_TESTS" ]; then
    echo "❌ FAILED TESTS"
    echo "=============="
    echo "$FAILED_TESTS"
    echo ""
fi

# Overall summary
echo "📈 OVERALL SUMMARY"
echo "================="
echo "Total tests passed: $TOTAL_PASSED"
echo "Total tests failed: $TOTAL_FAILED"

if [ "$TOTAL_FAILED" -eq 0 ]; then
    echo ""
    echo "🎉 All tests passed!"
    exit 0
else
    echo ""
    echo "⚠️  Some tests failed. See details above."
    exit 1
fi
