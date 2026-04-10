#!/bin/bash
# Test: nav move-to — move focused container to workspace, auto-add to active group, DB sync.

SG="$HOME/.cargo/bin/swayg"
DB=~/.local/share/swayg/swayg.db
GROUP="__test_nav_move__"
WS_TARGET="__tg_move_target__"
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

echo -e "\033[1m=== Test: nav move-to ===\033[0m"

rm -rf "$PID_DIR"
mkdir -p "$PID_DIR"

# --- 1. Precondition checks (BEFORE init) ---

run "Precondition: $GROUP does not exist" \
    'sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '"'$GROUP'"';"'
G_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP';")
if [ "$G_COUNT" = "0" ]; then pass "$GROUP does not exist in DB"; else fail "$GROUP must not exist in DB (count=$G_COUNT)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi

run "Precondition: $WS_TARGET does not exist in DB" \
    'sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '"'$WS_TARGET'"';"'
WS_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS_TARGET';")
if [ "$WS_COUNT" = "0" ]; then pass "$WS_TARGET does not exist in DB"; else fail "$WS_TARGET must not exist in DB (count=$WS_COUNT)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi

run "Precondition: $WS_TARGET does not exist in sway" \
    'swaymsg -t get_workspaces 2>/dev/null | jq -r ".[].name" | grep -c "^'"$WS_TARGET"'$"'
WS_SWAY=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[].name' | grep -c "^${WS_TARGET}$")
if [ "$WS_SWAY" = "0" ]; then pass "$WS_TARGET does not exist in sway"; else fail "$WS_TARGET must not exist in sway (count=$WS_SWAY)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi

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

# --- 3. Setup: init, create group, switch to it, launch kitty ---
run "Setup: init + create group + launch kitty" \
    "$SG init >/dev/null 2>&1
     $SG group select $OUTPUT '$GROUP' --create 2>&1
     kitty --class '__tg_move_kitty__' >/dev/null 2>&1 & echo \$! > $PID_DIR/__tg_move_kitty__.pid
     sleep 0.5"

run "Verify setup" 'true'

GROUP_EXISTS=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP';")
if [ "$GROUP_EXISTS" = "1" ]; then pass "group '$GROUP' exists"; else fail "group '$GROUP' exists (count=$GROUP_EXISTS)"; fi

KITY_PID=$(cat "$PID_DIR/__tg_move_kitty__.pid" 2>/dev/null)
if [ -n "$KITY_PID" ] && kill -0 "$KITY_PID" 2>/dev/null; then pass "kitty is running (pid=$KITY_PID)"; else fail "kitty is running"; fi

ACTIVE_GROUP=$(sqlite3 "$DB" "SELECT active_group FROM outputs WHERE name = '$OUTPUT';")
if [ "$ACTIVE_GROUP" = "$GROUP" ]; then pass "active group = '$GROUP'"; else fail "active group = '$GROUP' (got '$ACTIVE_GROUP')"; fi

# --- 4. Test: nav move-to (container in current group's workspace) ---
run "nav move-to '$WS_TARGET'" \
    "$SG nav move-to '$WS_TARGET' 2>&1"

MOVE_EXIT=$?
if [ "$MOVE_EXIT" -eq 0 ]; then pass "nav move-to succeeded (exit 0)"; else fail "nav move-to succeeded (exit=$MOVE_EXIT)"; fi

WS_IN_DB=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS_TARGET';")
if [ "$WS_IN_DB" = "1" ]; then pass "'$WS_TARGET' exists in DB"; else fail "'$WS_TARGET' exists in DB (count=$WS_IN_DB)"; fi

WS_IN_GROUP=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS_TARGET' AND g.name = '$GROUP';")
if [ "$WS_IN_GROUP" = "1" ]; then pass "'$WS_TARGET' in group '$GROUP'"; else fail "'$WS_TARGET' in group '$GROUP' (count=$WS_IN_GROUP)"; fi

WS_IN_SWAY=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[].name' | grep -c "^${WS_TARGET}$")
if [ "$WS_IN_SWAY" = "1" ]; then pass "'$WS_TARGET' exists in sway"; else fail "'$WS_TARGET' exists in sway (count=$WS_IN_SWAY)"; fi

# --- 5. Test: kitty is now on target workspace ---
WS_OF_KITTY=$(swaymsg -t get_tree 2>/dev/null | jq -r '[.. | objects | select(.type? == "workspace")] | .[] | {name: .name, apps: [.. | objects | select(.app_id? != null) | .app_id]} | select(.apps | index("__tg_move_kitty__")) | .name')
if [ "$WS_OF_KITTY" = "$WS_TARGET" ]; then pass "kitty is on workspace '$WS_TARGET'"; else fail "kitty is on workspace '$WS_TARGET' (got '$WS_OF_KITTY')"; fi

# --- 6. Cleanup ---
run "Kill kitty" \
    'kill $(cat '"$PID_DIR"'/__tg_move_kitty__.pid) 2>/dev/null; sleep 0.5'

KITY_PID2=$(cat "$PID_DIR/__tg_move_kitty__.pid" 2>/dev/null)
if [ -n "$KITY_PID2" ] && kill -0 "$KITY_PID2" 2>/dev/null; then fail "kitty '__tg_move_kitty__' is gone"; else pass "kitty '__tg_move_kitty__' is gone"; fi

run "Switch back to original workspace" \
    "swaymsg workspace \"$ORIG_WS\" >/dev/null 2>&1; sleep 0.1"

FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$ORIG_WS" ]; then pass "focused on original workspace '$ORIG_WS'"; else fail "focused on original workspace '$ORIG_WS' (got '$FOCUSED')"; fi

# --- 7. Post-condition ---
run "Post-condition: init to reset DB" \
    '$SG init >/dev/null 2>&1'

run "Post-condition: no test data in DB" \
    "sqlite3 \"\$DB\" \"SELECT count(*) FROM groups WHERE name = '$GROUP';\"; sqlite3 \"\$DB\" \"SELECT count(*) FROM workspaces WHERE name = '$WS_TARGET';\""

GROUP_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP';")
WS_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS_TARGET';")
if [ "$GROUP_GONE" = "0" ] && [ "$WS_GONE" = "0" ]; then
    pass "no test data remains in DB"
else
    fail "no test data remains in DB (group=$GROUP_GONE, ws=$WS_GONE)"
fi

echo ""
echo -e "\033[1m=== Summary ===\033[0m"
echo "  $PASS passed, $FAIL failed"
if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
