#!/bin/bash
# Test: create workspaces with containers, navigate, cleanup.

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

# Always switch back to original workspace on exit if there were failures
trap '[ "$FAIL" -gt 0 ] && swaymsg workspace "$ORIG_WS" >/dev/null 2>&1' EXIT

echo -e "\033[1m=== Test: new workspace with containers and workspaces ===\033[0m"

# Clean PID dir
rm -rf "$PID_DIR"
mkdir -p "$PID_DIR"

# --- 1. Precondition checks (BEFORE init) ---

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

ACTIVE=$($SG group active $OUTPUT 2>/dev/null)
if [ "$ACTIVE" = "$TEST_GROUP" ]; then
    pass "active group changed to '$TEST_GROUP'"
else
    fail "active group changed to '$TEST_GROUP' (got '$ACTIVE')"
fi

# --- 5. Launch kitty with class __tg_ws1__ ---
run "Launch kitty with app_id $WS1" \
    'kitty --class "'$WS1'" >/dev/null 2>&1 & echo $! > '"$PID_DIR/$WS1.pid"'; sleep 0.5'

WS1_PID=$(cat "$PID_DIR/$WS1.pid")
WS1_APPID=$(swaymsg -t get_tree 2>/dev/null | jq -r '.. | objects | select(.app_id? != null) | .app_id' | grep -c "^${WS1}$")
if [ "$WS1_APPID" -ge 1 ]; then
    pass "kitty with app_id '$WS1' is running (pid=$WS1_PID)"
else
    fail "kitty with app_id '$WS1' is running (count=$WS1_APPID)"
fi

# --- 6. Move focused container to __tg_ws1__ and switch ---
run "Move container to $WS1 (--switch-to-workspace)" \
    '$SG container move "'$WS1'" --switch-to-workspace 2>&1'

FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$WS1" ]; then
    pass "focused on '$WS1'"
else
    fail "focused on '$WS1' (got '$FOCUSED')"
fi

WS1_KITTY_WS=$(swaymsg -t get_tree 2>/dev/null | jq -r '[.. | objects | select(.type? == "workspace")] | .[] | {name: .name, apps: [.. | objects | select(.app_id? != null) | .app_id]} | select(.apps | index("'$WS1'")) | .name')
if [ "$WS1_KITTY_WS" = "$WS1" ]; then
    pass "kitty '$WS1' is on workspace '$WS1'"
else
    fail "kitty '$WS1' is on workspace '$WS1' (got '$WS1_KITTY_WS')"
fi

WS1_IN_DB=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS1';")
if [ "$WS1_IN_DB" = "1" ]; then
    pass "$WS1 is in DB"
else
    fail "$WS1 is in DB (count=$WS1_IN_DB)"
fi

WS1_IN_GROUP=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS1' AND g.name = '$TEST_GROUP';")
if [ "$WS1_IN_GROUP" = "1" ]; then
    pass "$WS1 is in group '$TEST_GROUP'"
else
    fail "$WS1 is in group '$TEST_GROUP' (count=$WS1_IN_GROUP)"
fi

# --- 7. Launch kitty with class __tg_ws2__ ---
run "Launch kitty with app_id $WS2" \
    'kitty --class "'$WS2'" >/dev/null 2>&1 & echo $! > '"$PID_DIR/$WS2.pid"'; sleep 0.5'

WS2_PID=$(cat "$PID_DIR/$WS2.pid")
WS2_APPID=$(swaymsg -t get_tree 2>/dev/null | jq -r '.. | objects | select(.app_id? != null) | .app_id' | grep -c "^${WS2}$")
if [ "$WS2_APPID" -ge 1 ]; then
    pass "kitty with app_id '$WS2' is running (pid=$WS2_PID)"
else
    fail "kitty with app_id '$WS2' is running (count=$WS2_APPID)"
fi

# --- 8. Move focused container to __tg_ws2__ and switch ---
run "Move container to $WS2 (--switch-to-workspace)" \
    '$SG container move "'$WS2'" --switch-to-workspace 2>&1'

FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$WS2" ]; then
    pass "focused on '$WS2'"
else
    fail "focused on '$WS2' (got '$FOCUSED')"
fi

WS2_KITTY_WS=$(swaymsg -t get_tree 2>/dev/null | jq -r '[.. | objects | select(.type? == "workspace")] | .[] | {name: .name, apps: [.. | objects | select(.app_id? != null) | .app_id]} | select(.apps | index("'$WS2'")) | .name')
if [ "$WS2_KITTY_WS" = "$WS2" ]; then
    pass "kitty '$WS2' is on workspace '$WS2'"
else
    fail "kitty '$WS2' is on workspace '$WS2' (got '$WS2_KITTY_WS')"
fi

WS2_IN_DB=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS2';")
if [ "$WS2_IN_DB" = "1" ]; then
    pass "$WS2 is in DB"
else
    fail "$WS2 is in DB (count=$WS2_IN_DB)"
fi

WS2_IN_GROUP=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS2' AND g.name = '$TEST_GROUP';")
if [ "$WS2_IN_GROUP" = "1" ]; then
    pass "$WS2 is in group '$TEST_GROUP'"
else
    fail "$WS2 is in group '$TEST_GROUP' (count=$WS2_IN_GROUP)"
fi

# --- 9. Switch back to original group ---
run "Switch back to original group '$ORIG_GROUP'" \
    '$SG group select $OUTPUT '"'$ORIG_GROUP'"' 2>&1'

FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$ORIG_WS" ]; then
    pass "focused on original workspace '$ORIG_WS'"
else
    fail "focused on original workspace '$ORIG_WS' (got '$FOCUSED')"
fi

GROUP_STILL=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$TEST_GROUP';")
if [ "$GROUP_STILL" = "1" ]; then
    pass "__test_group__ NOT auto-deleted (still has workspaces)"
else
    fail "__test_group__ NOT auto-deleted (count=$GROUP_STILL)"
fi

# --- 10. Kill test kitties via PID ---
run "Kill test kitties (via PID)" \
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

# --- 11. Switch to test group and back (should auto-delete now) ---
run "Switch to test group (should still exist)" \
    '$SG group select $OUTPUT '"'$TEST_GROUP'"' 2>&1'

run "Switch back to original group '$ORIG_GROUP'" \
    '$SG group select $OUTPUT '"'$ORIG_GROUP'"' 2>&1'

FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$ORIG_WS" ]; then
    pass "focused on original workspace '$ORIG_WS'"
else
    fail "focused on original workspace '$ORIG_WS' (got '$FOCUSED')"
fi

GROUP_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$TEST_GROUP';")
if [ "$GROUP_GONE" = "0" ]; then
    pass "__test_group__ auto-deleted"
else
    fail "__test_group__ auto-deleted (count=$GROUP_GONE)"
fi

# --- Post-condition: no test data remains ---
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
