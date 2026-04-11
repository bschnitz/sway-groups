#!/bin/bash
# Run all test*.sh in this directory sequentially.

DIR="$(cd "$(dirname "$0")" && pwd)"
TOTAL_PASS=0
TOTAL_FAIL=0
FAILED_TESTS=0

pass() { echo -e "  \033[0;32mPASS\033[0m $1"; TOTAL_PASS=$((TOTAL_PASS + 1)); }
fail() { echo -e "  \033[0;31mFAIL\033[0m $1"; TOTAL_FAIL=$((TOTAL_FAIL + 1)); FAILED_TESTS=$((FAILED_TESTS + 1)); }

TESTS=($(ls "$DIR"/test*.sh 2>/dev/null | sort))

if [ ${#TESTS[@]} -eq 0 ]; then
    echo "No test*.sh files found in $DIR"
    exit 1
fi

echo -e "\033[1m=== Running ${#TESTS[@]} tests ===\033[0m"

for TEST in "${TESTS[@]}"; do
    NAME=$(basename "$TEST")
    echo ""
    echo -e "\033[1m--- $NAME ---\033[0m"

    OUTPUT=$(bash "$TEST" 2>&1)
    EXIT_CODE=$?

    echo "$OUTPUT" | tail -1 | sed 's/^/  /'

    PASS_COUNT=$(echo "$OUTPUT" | grep -c "PASS")
    FAIL_COUNT=$(echo "$OUTPUT" | grep -c "FAIL")

    if [ "$FAIL_COUNT" -gt 0 ]; then
        TOTAL_FAIL=$((TOTAL_FAIL + FAIL_COUNT))
        TOTAL_PASS=$((TOTAL_PASS + PASS_COUNT))
        FAILED_TESTS=$((FAILED_TESTS + 1))

        echo ""
        echo -e "\033[1m$NAME — $PASS_COUNT/$((PASS_COUNT + FAIL_COUNT)) PASS ($FAIL_COUNT FAIL)\033[0m"
        echo ""
        while IFS= read -r line; do
            if echo "$line" | grep -q "FAIL"; then
                echo "$line"
            fi
        done <<< "$OUTPUT"

        echo ""
        echo -e "\033[0;31mABORTED — $NAME failed\033[0m"
        notify-send "swayg" "Tests aborted: $NAME failed"
        exit 1
    fi

    TOTAL_PASS=$((TOTAL_PASS + PASS_COUNT))
    echo "$NAME — $PASS_COUNT/$PASS_COUNT PASS"
done

echo ""
echo -e "\033[1m=== All tests passed: $((TOTAL_PASS)) assertions across ${#TESTS[@]} tests ===\033[0m"
notify-send "swayg" "All tests passed: $TOTAL_PASS assertions"
