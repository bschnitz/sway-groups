#!/bin/bash
# Test: group select with auto-creation and switching back.

SG="$HOME/.cargo/bin/swayg"
DB=~/.local/share/swayg/swayg.db
__test_group__="__test_group__"
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

echo -e "\033[1m=== Test: group select ===\033[0m"

# --- Check preconditions BEFORE init ---
run "Check that test group does not already exist (precondition)" \
    'sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '"'$__test_group__'"';"'

GROUP_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$__test_group__';")
if [ "$GROUP_COUNT" = "0" ]; then
    pass "group '$__test_group__' does not exist yet"
else
    fail "group '$__test_group__' must not exist before test (count=$GROUP_COUNT)"
    echo ""
    echo "Results: $PASS passed, $FAIL failed (ABORTED)"
    exit 1
fi

# --- Remember current state ---
ORIG_GROUP=$($SG group active $OUTPUT 2>/dev/null)
ORIG_WS=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')

echo ""
echo "  Original group: '$ORIG_GROUP'"
echo "  Original workspace: '$ORIG_WS'"

if [ -z "$ORIG_GROUP" ]; then
    fail "could not determine original group"
    echo ""
    echo "Results: $PASS passed, $FAIL failed (ABORTED)"
    exit 1
fi

pass "remembered original group '$ORIG_GROUP'"
pass "remembered original workspace '$ORIG_WS'"

# --- Init ---
run "Init fresh DB" \
    '$SG init >/dev/null 2>&1'

if [ $? -eq 0 ]; then
    pass "init succeeded"
else
    fail "init failed"
fi

# --- Switch to test group (should auto-create) ---
run "Select test group (with --create)" \
    '$SG group select $OUTPUT '"'$__test_group__'"' --create 2>&1'

# --- Verify group was created ---
GROUP_EXISTS=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$__test_group__';")
if [ "$GROUP_EXISTS" = "1" ]; then
    pass "group '$__test_group__' was created"
else
    fail "group '$__test_group__' was created (count=$GROUP_EXISTS)"
fi

# --- Verify active group changed ---
ACTIVE=$($SG group active $OUTPUT 2>/dev/null)
if [ "$ACTIVE" = "$__test_group__" ]; then
    pass "active group changed to '$__test_group__'"
else
    fail "active group changed to '$__test_group__' (got '$ACTIVE')"
fi

# --- Switch back to original group ---
run "Switch back to original group '$ORIG_GROUP'" \
    '$SG group select $OUTPUT '"'$ORIG_GROUP'"' 2>&1'

# --- Verify focused workspace is original ---
FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$ORIG_WS" ]; then
    pass "focused on original workspace '$ORIG_WS'"
else
    fail "focused on original workspace '$ORIG_WS' (got '$FOCUSED')"
fi

# --- Verify test group was auto-deleted ---
GROUP_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$__test_group__';")
if [ "$GROUP_GONE" = "0" ]; then
    pass "group '$__test_group__' was auto-deleted"
else
    fail "group '$__test_group__' was auto-deleted (count=$GROUP_GONE)"
fi

# --- Post-condition: no test data remains ---
run "Post-condition: no test data in DB" \
    'sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '"'$__test_group__'"';"; sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id WHERE g.name = '"'$__test_group__'"';"'

GROUP_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$__test_group__';")
WSGRP_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id WHERE g.name = '$__test_group__';")
if [ "$GROUP_GONE" = "0" ] && [ "$WSGRP_GONE" = "0" ]; then
    pass "no test data remains in DB"
else
    fail "no test data remains in DB (group=$GROUP_GONE, ws_groups=$WSGRP_GONE)"
fi

echo ""
echo -e "\033[1m=== Summary ===\033[0m"
echo "  $PASS passed, $FAIL failed"
if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
