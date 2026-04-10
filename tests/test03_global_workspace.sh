#!/bin/bash
# Test: global workspaces, visibility, unglobal, auto-delete of all-global groups.

SG="$HOME/.cargo/bin/swayg"
DB=~/.local/share/swayg/swayg.db
TEST_GROUP="__test_group__"
WS1="__tg_ws1__"
WS2="__tg_ws2__"
OUTPUT="eDP-1"
PID_DIR="/tmp/sway-group-tests"

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

echo -e "\033[1m=== Test: global workspaces ===\033[0m"

rm -rf "$PID_DIR"
mkdir -p "$PID_DIR"

# --- 1. Precondition checks ---

run "Precondition: test group does not exist" \
    'sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '"'$TEST_GROUP'"';"'

GROUP_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$TEST_GROUP';")
if [ "$GROUP_COUNT" = "0" ]; then
    pass "__test_group__ does not exist in DB"
else
    fail "__test_group__ must not exist in DB (count=$GROUP_COUNT)"
    echo "Results: $PASS passed, $FAIL failed (ABORTED)"
    exit 1
fi

run "Precondition: $WS1 does not exist in DB" \
    'sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '"'$WS1'"';"'

WS1_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS1';")
if [ "$WS1_COUNT" = "0" ]; then
    pass "$WS1 does not exist in DB"
else
    fail "$WS1 must not exist in DB (count=$WS1_COUNT)"
    echo "Results: $PASS passed, $FAIL failed (ABORTED)"
    exit 1
fi

run "Precondition: $WS2 does not exist in DB" \
    'sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '"'$WS2'"';"'

WS2_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS2';")
if [ "$WS2_COUNT" = "0" ]; then
    pass "$WS2 does not exist in DB"
else
    fail "$WS2 must not exist in DB (count=$WS2_COUNT)"
    echo "Results: $PASS passed, $FAIL failed (ABORTED)"
    exit 1
fi

run "Precondition: $WS1 does not exist in sway" \
    'swaymsg -t get_workspaces 2>/dev/null | jq -r ".[].name" | grep -c "^'"$WS1"'$"'

WS1_SWAY=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[].name' | grep -c "^${WS1}$")
if [ "$WS1_SWAY" = "0" ]; then
    pass "$WS1 does not exist in sway"
else
    fail "$WS1 must not exist in sway (count=$WS1_SWAY)"
    echo "Results: $PASS passed, $FAIL failed (ABORTED)"
    exit 1
fi

run "Precondition: $WS2 does not exist in sway" \
    'swaymsg -t get_workspaces 2>/dev/null | jq -r ".[].name" | grep -c "^'"$WS2"'$"'

WS2_SWAY=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[].name' | grep -c "^${WS2}$")
if [ "$WS2_SWAY" = "0" ]; then
    pass "$WS2 does not exist in sway"
else
    fail "$WS2 must not exist in sway (count=$WS2_SWAY)"
    echo "Results: $PASS passed, $FAIL failed (ABORTED)"
    exit 1
fi

# --- 2. Remember original state ---
ORIG_GROUP=$($SG group active $OUTPUT 2>/dev/null)
ORIG_WS=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')

echo ""
echo "  Original group: '$ORIG_GROUP'"
echo "  Original workspace: '$ORIG_WS'"

# --- 3. Init ---
run "Init fresh DB" \
    '$SG init >/dev/null 2>&1'

if [ $? -eq 0 ]; then
    pass "init succeeded"
else
    fail "init failed"
fi

# --- 4. Select test group ---
run "Select test group (with --create)" \
    '$SG group select $OUTPUT '"'$TEST_GROUP'"' --create 2>&1'

GROUP_EXISTS=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$TEST_GROUP';")
if [ "$GROUP_EXISTS" = "1" ]; then
    pass "group '$TEST_GROUP' was created"
else
    fail "group '$TEST_GROUP' was created (count=$GROUP_EXISTS)"
fi

# --- 5. Launch and move kitty 1 ---
run "Launch kitty with app_id $WS1" \
    'kitty --class "'$WS1'" >/dev/null 2>&1 & echo $! > '"$PID_DIR/$WS1.pid"'; sleep 0.5'

run "Move container to $WS1 (--switch-to-workspace)" \
    '$SG container move "'$WS1'" --switch-to-workspace 2>&1'

FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$WS1" ]; then
    pass "focused on '$WS1'"
else
    fail "focused on '$WS1' (got '$FOCUSED')"
fi

# --- 6. Launch and move kitty 2 ---
run "Launch kitty with app_id $WS2" \
    'kitty --class "'$WS2'" >/dev/null 2>&1 & echo $! > '"$PID_DIR/$WS2.pid"'; sleep 0.5'

run "Move container to $WS2 (--switch-to-workspace)" \
    '$SG container move "'$WS2'" --switch-to-workspace 2>&1'

FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$WS2" ]; then
    pass "focused on '$WS2'"
else
    fail "focused on '$WS2' (got '$FOCUSED')"
fi

# --- 7. Set __tg_ws1__ as global ---
run "Set $WS1 as global" \
    '$SG workspace global "'$WS1'" 2>&1'

WS1_GLOBAL=$(sqlite3 "$DB" "SELECT is_global FROM workspaces WHERE name = '$WS1';")
if [ "$WS1_GLOBAL" = "1" ]; then
    pass "$WS1 is global in DB"
else
    fail "$WS1 is global in DB (got '$WS1_GLOBAL')"
fi

# --- 8. Switch back to original group ---
run "Switch back to original group '$ORIG_GROUP'" \
    '$SG group select $OUTPUT '"'$ORIG_GROUP'"' 2>&1'

FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$ORIG_WS" ]; then
    pass "focused on original workspace '$ORIG_WS'"
else
    fail "focused on original workspace '$ORIG_WS' (got '$FOCUSED')"
fi

# --- 9. Verify global visibility ---
OUT=$($SG workspace list --visible --plain --output $OUTPUT 2>/dev/null)

echo "$OUT" | grep -q "$WS1"
if [ $? -eq 0 ]; then
    pass "$WS1 is visible in group '$ORIG_GROUP' (global)"
else
    fail "$WS1 is visible in group '$ORIG_GROUP' (global)"
fi

echo "$OUT" | grep -q "$WS2"
if [ $? -ne 0 ]; then
    pass "$WS2 is NOT visible in group '$ORIG_GROUP' (not global)"
else
    fail "$WS2 is NOT visible in group '$ORIG_GROUP' (not global)"
fi

# --- 10. Verify global workspace has no group, non-global still in group ---
OUT=$($SG workspace list --plain --group $TEST_GROUP --output $OUTPUT 2>/dev/null)

echo "$OUT" | grep -q "$WS1"
if [ $? -ne 0 ]; then
    pass "$WS1 is NOT in group '$TEST_GROUP' (global, no group membership)"
else
    fail "$WS1 is NOT in group '$TEST_GROUP' (global, no group membership)"
fi

echo "$OUT" | grep -q "$WS2"
if [ $? -eq 0 ]; then
    pass "$WS2 is visible in group '$TEST_GROUP'"
else
    fail "$WS2 is visible in group '$TEST_GROUP'"
fi

# --- 11. Unglobal __tg_ws1__ ---
run "Unglobal $WS1" \
    '$SG workspace unglobal "'$WS1'" 2>&1'

WS1_NOT_GLOBAL=$(sqlite3 "$DB" "SELECT is_global FROM workspaces WHERE name = '$WS1';")
if [ "$WS1_NOT_GLOBAL" = "0" ]; then
    pass "$WS1 is no longer global"
else
    fail "$WS1 is no longer global (got '$WS1_NOT_GLOBAL')"
fi

# --- 12. Verify __tg_ws1__ now visible in active group (unglobal adds to active group) ---
OUT=$($SG workspace list --visible --plain --output $OUTPUT 2>/dev/null)

echo "$OUT" | grep -q "$WS1"
if [ $? -eq 0 ]; then
    pass "$WS1 is visible in group '$ORIG_GROUP' after unglobal"
else
    fail "$WS1 is visible in group '$ORIG_GROUP' after unglobal"
fi

# --- 13a. Auto-delete: switch from global workspace ---
run "Switch to test group" \
    '$SG group select $OUTPUT '"'$TEST_GROUP'"' 2>&1'

run "Set $WS2 as global" \
    '$SG workspace global "'$WS2'" 2>&1'

WS2_GLOBAL=$(sqlite3 "$DB" "SELECT is_global FROM workspaces WHERE name = '$WS2';")
if [ "$WS2_GLOBAL" = "1" ]; then
    pass "$WS2 is global in DB"
else
    fail "$WS2 is global in DB (got '$WS2_GLOBAL')"
fi

run "Kill kitty $WS1 to remove non-global workspace from sway" \
    'kill $(cat '"$PID_DIR/$WS1.pid"') 2>/dev/null; sleep 0.5'

WS1_GONE=$(swaymsg -t get_tree 2>/dev/null | jq -r '.. | objects | select(.app_id? != null) | .app_id' | grep -c "^${WS1}$")
if [ "$WS1_GONE" = "0" ]; then
    pass "kitty '$WS1' is gone"
else
    fail "kitty '$WS1' is gone (count=$WS1_GONE)"
fi

run "Switch to $WS2 (let sway auto-delete empty $WS1)" \
    'swaymsg workspace "'$WS2'" >/dev/null 2>&1; sleep 0.5'

WS1_SWAY_GONE=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[].name' | grep -c "^${WS1}$")
if [ "$WS1_SWAY_GONE" = "0" ]; then
    pass "$WS1 is gone from sway"
else
    fail "$WS1 is gone from sway (count=$WS1_SWAY_GONE)"
fi

OUT=$($SG group list --output $OUTPUT 2>/dev/null)
echo "$OUT" | grep -q "$TEST_GROUP"
if [ $? -eq 0 ]; then
    pass "__test_group__ still exists (has global workspaces)"
else
    fail "__test_group__ still exists (has global workspaces)"
fi

run "Switch back from global workspace (should auto-delete __test_group__)" \
    '$SG group select $OUTPUT '"'$ORIG_GROUP'"' 2>&1'

FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$ORIG_WS" ]; then
    pass "focused on original workspace '$ORIG_WS'"
else
    fail "focused on original workspace '$ORIG_WS' (got '$FOCUSED')"
fi

GROUP_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$TEST_GROUP';")
if [ "$GROUP_GONE" = "0" ]; then
    pass "__test_group__ auto-deleted (switched from global workspace)"
else
    fail "__test_group__ auto-deleted (switched from global workspace) (count=$GROUP_GONE)"
fi

# --- 13b. Auto-delete: switch from empty workspace (only global workspaces remain) ---

run "Create test group again" \
    '$SG group select $OUTPUT '"'$TEST_GROUP'"' --create 2>&1'

run "Launch kitty with app_id $WS1" \
    'kitty --class "'$WS1'" >/dev/null 2>&1 & echo $! > '"$PID_DIR/$WS1.pid"'; sleep 0.5'

run "Move container to $WS1 (--switch-to-workspace)" \
    '$SG container move "'$WS1'" --switch-to-workspace 2>&1'

run "Set $WS1 as global" \
    '$SG workspace global "'$WS1'" 2>&1'

WS1_GLOBAL=$(sqlite3 "$DB" "SELECT is_global FROM workspaces WHERE name = '$WS1';")
if [ "$WS1_GLOBAL" = "1" ]; then
    pass "$WS1 is global in DB"
else
    fail "$WS1 is global in DB (got '$WS1_GLOBAL')"
fi

OUT=$($SG group list --output $OUTPUT 2>/dev/null)
echo "$OUT" | grep -q "$TEST_GROUP"
if [ $? -eq 0 ]; then
    pass "__test_group__ still exists (has global workspace)"
else
    fail "__test_group__ still exists (has global workspace)"
fi

run "Switch back to original group (from empty workspace '0', should auto-delete)" \
    '$SG group select $OUTPUT '"'$ORIG_GROUP'"' 2>&1'

FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$ORIG_WS" ]; then
    pass "focused on original workspace '$ORIG_WS'"
else
    fail "focused on original workspace '$ORIG_WS' (got '$FOCUSED')"
fi

GROUP_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$TEST_GROUP';")
if [ "$GROUP_GONE" = "0" ]; then
    pass "__test_group__ auto-deleted (switched from empty workspace, only global remained)"
else
    fail "__test_group__ auto-deleted (switched from empty workspace, only global remained) (count=$GROUP_GONE)"
fi

# --- 14. Cleanup ---
run "Kill remaining kitty $WS1" \
    'kill $(cat '"$PID_DIR/$WS1.pid"') 2>/dev/null; kill $(cat '"$PID_DIR/$WS2.pid"') 2>/dev/null; sleep 0.5'

WS1_GONE=$(swaymsg -t get_tree 2>/dev/null | jq -r '.. | objects | select(.app_id? != null) | .app_id' | grep -c "^${WS1}$")
if [ "$WS1_GONE" = "0" ]; then
    pass "kitty '$WS1' is gone"
else
    fail "kitty '$WS1' is gone (count=$WS1_GONE)"
fi

WS2_GONE=$(swaymsg -t get_tree 2>/dev/null | jq -r '.. | objects | select(.app_id? != null) | .app_id' | grep -c "^${WS2}$")
if [ "$WS2_GONE" = "0" ]; then
    pass "kitty '$WS2' is gone"
else
    fail "kitty '$WS2' is gone (count=$WS2_GONE)"
fi

# --- Post-condition: clean up and verify no test data remains ---
run "Init to sync DB state" \
    '$SG init >/dev/null 2>&1'

run "Post-condition: no test data in DB" \
    'sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '"'$TEST_GROUP'"';"; sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name IN ('"'"$WS1"'"', '"'"$WS2"'"');"; sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id WHERE g.name = '"'$TEST_GROUP'"';"'

GROUP_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$TEST_GROUP';")
WS_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name IN ('$WS1', '$WS2');")
WSGRP_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id WHERE g.name = '$TEST_GROUP';")
if [ "$GROUP_GONE" = "0" ] && [ "$WS_GONE" = "0" ] && [ "$WSGRP_GONE" = "0" ]; then
    pass "no test data remains in DB"
else
    fail "no test data remains in DB (group=$GROUP_GONE, ws=$WS_GONE, ws_groups=$WSGRP_GONE)"
fi

echo ""
echo -e "\033[1m=== Summary ===\033[0m"
echo "  $PASS passed, $FAIL failed"
if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
