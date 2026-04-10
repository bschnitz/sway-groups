#!/bin/bash
# Test: group create — success, error when already exists, no side effects.

SG="$HOME/.cargo/bin/swayg"
DB=~/.local/share/swayg/swayg.db
GROUP="__test_create__"
OUTPUT="eDP-1"

PASS=0
FAIL=0
STEP=0

pass() { echo -e "  \033[0;32mPASS\033[0m $1"; PASS=$((PASS + 1)); }
fail() { echo -e "  \033[0;31mFAIL\033[0m $1"; FAIL=$((FAIL + 1)); }

run() {
    STEP=$((STEP + 1))
    echo ""
    echo -e "\033[1m--- Step $STEP: $1\033[0m"
    shift
    echo -e "\033[33m> $@\033[0m"
    eval "$@"
}

trap '[ "$FAIL" -gt 0 ] && swaymsg workspace "$ORIG_WS" >/dev/null 2>&1' EXIT

echo -e "\033[1m=== Test: group create ===\033[0m"

# --- 1. Precondition checks (BEFORE init) ---

run "Precondition: $GROUP does not exist" \
    'sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '"'$GROUP'"';"'
G_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP';")
if [ "$G_COUNT" = "0" ]; then pass "$GROUP does not exist in DB"; else fail "$GROUP must not exist in DB (count=$G_COUNT)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi

# --- 2. Remember original state ---
ORIG_GROUP=$($SG group active $OUTPUT 2>/dev/null)
ORIG_WS=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')

echo ""
echo "  Original group: '$ORIG_GROUP'"
echo "  Original workspace: '$ORIG_WS'"

if [ -z "$ORIG_GROUP" ]; then
    fail "could not determine original group"
    echo "Results: $PASS passed, $FAIL failed (ABORTED)"
    exit 1
fi

pass "remembered original group '$ORIG_GROUP'"
pass "remembered original workspace '$ORIG_WS'"

# --- 3. Setup ---
run "Setup: init" \
    '$SG init >/dev/null 2>&1'

run "Verify setup" 'true'

GROUP_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP';")
if [ "$GROUP_COUNT" = "0" ]; then pass "no test group in DB after init"; else fail "no test group in DB after init (count=$GROUP_COUNT)"; fi

# --- 4. Test: create group (success) ---
run "Create group '$GROUP'" \
    "$SG group create '$GROUP' 2>&1"

CREATE_EXIT=$?
if [ "$CREATE_EXIT" -eq 0 ]; then pass "group create succeeded (exit 0)"; else fail "group create succeeded (exit=$CREATE_EXIT)"; fi

GA_EXISTS=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP';")
if [ "$GA_EXISTS" = "1" ]; then pass "group '$GROUP' exists in DB"; else fail "group '$GROUP' exists in DB (count=$GA_EXISTS)"; fi

ACTIVE_UNCHANGED=$(sqlite3 "$DB" "SELECT active_group FROM outputs WHERE name = '$OUTPUT';")
if [ "$ACTIVE_UNCHANGED" = "$ORIG_GROUP" ]; then pass "active group unchanged after create"; else fail "active group unchanged after create (got '$ACTIVE_UNCHANGED', expected '$ORIG_GROUP')"; fi

# --- 5. Test: create same group again (error) ---
run "Create group '$GROUP' again (should fail)" \
    "DUP_OUT=\$(\"$SG\" group create '$GROUP' 2>&1); DUP_EXIT=\$?; echo \"\$DUP_OUT\""

if [ "$DUP_EXIT" -ne 0 ]; then pass "group create failed (exit $DUP_EXIT)"; else fail "group create should have failed (exit 0)"; fi

echo "$DUP_OUT" | grep -q "already exists"
if [ $? -eq 0 ]; then pass "error contains 'already exists'"; else fail "error contains 'already exists' (output: '$DUP_OUT')"; fi

STILL_ONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP';")
if [ "$STILL_ONE" = "1" ]; then pass "still exactly 1 group '$GROUP' in DB (no duplicate)"; else fail "still exactly 1 group '$GROUP' in DB (count=$STILL_ONE)"; fi

ACTIVE_STILL=$(sqlite3 "$DB" "SELECT active_group FROM outputs WHERE name = '$OUTPUT';")
if [ "$ACTIVE_STILL" = "$ORIG_GROUP" ]; then pass "active group still unchanged after failed create"; else fail "active group still unchanged after failed create (got '$ACTIVE_STILL')"; fi

# --- 6. Cleanup ---
run "Switch back to original workspace" \
    "swaymsg workspace \"$ORIG_WS\" >/dev/null 2>&1"

FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$ORIG_WS" ]; then pass "focused on original workspace '$ORIG_WS'"; else fail "focused on original workspace '$ORIG_WS' (got '$FOCUSED')"; fi

# --- 7. Post-condition ---
run "Post-condition: init to reset DB" \
    '$SG init >/dev/null 2>&1'

run "Post-condition: no test data in DB" \
    "sqlite3 \"\$DB\" \"SELECT count(*) FROM groups WHERE name = '$GROUP';\""

GROUP_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP';")
if [ "$GROUP_GONE" = "0" ]; then pass "no test data remains in DB"; else fail "no test data remains in DB (count=$GROUP_GONE)"; fi

echo ""
echo -e "\033[1m=== Summary ===\033[0m"
echo "  $PASS passed, $FAIL failed"
if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
