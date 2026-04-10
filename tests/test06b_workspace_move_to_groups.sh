#!/bin/bash
# Test: workspace move (remove from all groups, add to specified groups).

SG="$HOME/.cargo/bin/swayg"
DB=~/.local/share/swayg/swayg.db
GROUP_A="__test_group_a__"
GROUP_B="__test_group_b__"
GROUP_C="__test_group_c__"
WS1="__tg_ws1__"
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

echo -e "\033[1m=== Test: workspace move to groups ===\033[0m"

rm -rf "$PID_DIR"
mkdir -p "$PID_DIR"

# --- 1. Precondition checks ---

run "Precondition: $GROUP_A does not exist" \
    "sqlite3 \"\$DB\" \"SELECT count(*) FROM groups WHERE name = '$GROUP_A';\""
GA_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_A';")
if [ "$GA_COUNT" = "0" ]; then pass "$GROUP_A does not exist in DB"; else fail "$GROUP_A must not exist in DB (count=$GA_COUNT)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi

run "Precondition: $GROUP_B does not exist" \
    "sqlite3 \"\$DB\" \"SELECT count(*) FROM groups WHERE name = '$GROUP_B';\""
GB_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_B';")
if [ "$GB_COUNT" = "0" ]; then pass "$GROUP_B does not exist in DB"; else fail "$GROUP_B must not exist in DB (count=$GB_COUNT)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi

run "Precondition: $GROUP_C does not exist" \
    "sqlite3 \"\$DB\" \"SELECT count(*) FROM groups WHERE name = '$GROUP_C';\""
GC_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_C';")
if [ "$GC_COUNT" = "0" ]; then pass "$GROUP_C does not exist in DB"; else fail "$GROUP_C must not exist in DB (count=$GC_COUNT)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi

run "Precondition: $WS1 does not exist in DB" \
    "sqlite3 \"\$DB\" \"SELECT count(*) FROM workspaces WHERE name = '$WS1';\""
WS1_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS1';")
if [ "$WS1_COUNT" = "0" ]; then pass "$WS1 does not exist in DB"; else fail "$WS1 must not exist in DB (count=$WS1_COUNT)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi

run "Precondition: $WS1 does not exist in sway" \
    "swaymsg -t get_workspaces 2>/dev/null | jq -r '.[].name' | grep -c '^$WS1\$'"
WS1_SWAY=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[].name' | grep -c "^${WS1}$")
if [ "$WS1_SWAY" = "0" ]; then pass "$WS1 does not exist in sway"; else fail "$WS1 must not exist in sway (count=$WS1_SWAY)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi

# --- 2. Remember original state ---
ORIG_GROUP=$($SG group active $OUTPUT 2>/dev/null)
ORIG_WS=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')

echo ""
echo "  Original group: '$ORIG_GROUP'"
echo "  Original workspace: '$ORIG_WS'"

# --- 3. Setup ---
run "Setup: init + create groups + workspace + kitty" \
    "$SG init >/dev/null 2>&1
    $SG group select $OUTPUT '$GROUP_A' --create 2>&1
    kitty --class '$WS1' >/dev/null 2>&1 & echo \$! > '$PID_DIR/$WS1.pid'
    sleep 0.5
    $SG container move '$WS1' --switch-to-workspace 2>&1
    $SG group select $OUTPUT '$GROUP_B' --create 2>&1
    $SG workspace add '$WS1' 2>&1
    $SG group select $OUTPUT '$ORIG_GROUP' 2>&1"

run "Verify setup" \
    "true"

GA_EXISTS=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_A';")
if [ "$GA_EXISTS" = "1" ]; then pass "group '$GROUP_A' exists"; else fail "group '$GROUP_A' exists (count=$GA_EXISTS)"; fi

GB_EXISTS=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_B';")
if [ "$GB_EXISTS" = "1" ]; then pass "group '$GROUP_B' exists"; else fail "group '$GROUP_B' exists (count=$GB_EXISTS)"; fi

WS1_IN_GA=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS1' AND g.name = '$GROUP_A';")
if [ "$WS1_IN_GA" = "1" ]; then pass "$WS1 is in group '$GROUP_A'"; else fail "$WS1 is in group '$GROUP_A' (count=$WS1_IN_GA)"; fi

WS1_IN_GB=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS1' AND g.name = '$GROUP_B';")
if [ "$WS1_IN_GB" = "1" ]; then pass "$WS1 is in group '$GROUP_B'"; else fail "$WS1 is in group '$GROUP_B' (count=$WS1_IN_GB)"; fi

WS1_SWAY=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[].name' | grep -c "^${WS1}$")
if [ "$WS1_SWAY" = "1" ]; then pass "$WS1 is in sway"; else fail "$WS1 is in sway (count=$WS1_SWAY)"; fi

# --- 4. Test: move to Group C (doesn't exist, should auto-create) ---
run "Move $WS1 to $GROUP_C (auto-create)" \
    "$SG workspace move '$WS1' --groups '$GROUP_C' 2>&1"

run "Verify: removed from A and B, added to C" \
    "true"

WS1_NOT_IN_GA=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS1' AND g.name = '$GROUP_A';")
if [ "$WS1_NOT_IN_GA" = "0" ]; then pass "$WS1 NOT in group '$GROUP_A' (removed from all)"; else fail "$WS1 NOT in group '$GROUP_A' (count=$WS1_NOT_IN_GA)"; fi

WS1_NOT_IN_GB=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS1' AND g.name = '$GROUP_B';")
if [ "$WS1_NOT_IN_GB" = "0" ]; then pass "$WS1 NOT in group '$GROUP_B' (removed from all)"; else fail "$WS1 NOT in group '$GROUP_B' (count=$WS1_NOT_IN_GB)"; fi

WS1_IN_GC=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS1' AND g.name = '$GROUP_C';")
if [ "$WS1_IN_GC" = "1" ]; then pass "$WS1 is in group '$GROUP_C'"; else fail "$WS1 is in group '$GROUP_C' (count=$WS1_IN_GC)"; fi

GC_EXISTS=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_C';")
if [ "$GC_EXISTS" = "1" ]; then pass "group '$GROUP_C' auto-created"; else fail "group '$GROUP_C' auto-created (count=$GC_EXISTS)"; fi

# --- 5. Test: move to Group A and Group B (comma-separated) ---
run "Move $WS1 to $GROUP_A,$GROUP_B" \
    "$SG workspace move '$WS1' --groups '$GROUP_A,$GROUP_B' 2>&1"

run "Verify: in A and B, not in C" \
    "true"

WS1_IN_GA=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS1' AND g.name = '$GROUP_A';")
if [ "$WS1_IN_GA" = "1" ]; then pass "$WS1 is in group '$GROUP_A'"; else fail "$WS1 is in group '$GROUP_A' (count=$WS1_IN_GA)"; fi

WS1_IN_GB=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS1' AND g.name = '$GROUP_B';")
if [ "$WS1_IN_GB" = "1" ]; then pass "$WS1 is in group '$GROUP_B'"; else fail "$WS1 is in group '$GROUP_B' (count=$WS1_IN_GB)"; fi

WS1_NOT_IN_GC=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS1' AND g.name = '$GROUP_C';")
if [ "$WS1_NOT_IN_GC" = "0" ]; then pass "$WS1 NOT in group '$GROUP_C' (removed)"; else fail "$WS1 NOT in group '$GROUP_C' (count=$WS1_NOT_IN_GC)"; fi

# --- 6. Cleanup ---
run "Switch back to original group '$ORIG_GROUP'" \
    "$SG group select $OUTPUT '$ORIG_GROUP' 2>&1"

run "Kill kitty $WS1" \
    "kill \$(cat '$PID_DIR/$WS1.pid') 2>/dev/null; sleep 0.5"
WS1_GONE=$(swaymsg -t get_tree 2>/dev/null | jq -r '.. | objects | select(.app_id? != null) | .app_id' | grep -c "^${WS1}$")
if [ "$WS1_GONE" = "0" ]; then pass "kitty '$WS1' is gone"; else fail "kitty '$WS1' is gone (count=$WS1_GONE)"; fi

run "Auto-delete $GROUP_C" \
    "$SG group select $OUTPUT '$GROUP_C' 2>&1"
run "Switch back" \
    "$SG group select $OUTPUT '$ORIG_GROUP' 2>&1"
GC_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_C';")
if [ "$GC_GONE" = "0" ]; then pass "$GROUP_C auto-deleted"; else fail "$GROUP_C auto-deleted (count=$GC_GONE)"; fi

run "Auto-delete $GROUP_A" \
    "$SG group select $OUTPUT '$GROUP_A' 2>&1"
run "Switch back" \
    "$SG group select $OUTPUT '$ORIG_GROUP' 2>&1"
GA_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_A';")
if [ "$GA_GONE" = "0" ]; then pass "$GROUP_A auto-deleted"; else fail "$GROUP_A auto-deleted (count=$GA_GONE)"; fi

run "Auto-delete $GROUP_B" \
    "$SG group select $OUTPUT '$GROUP_B' 2>&1"
run "Switch back" \
    "$SG group select $OUTPUT '$ORIG_GROUP' 2>&1"
GB_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_B';")
if [ "$GB_GONE" = "0" ]; then pass "$GROUP_B auto-deleted"; else fail "$GROUP_B auto-deleted (count=$GB_GONE)"; fi

# --- 7. Post-condition ---
run "Init to sync DB state" \
    "$SG init >/dev/null 2>&1"

run "Post-condition: no test data in DB" \
    "true"

GROUP_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name IN ('$GROUP_A', '$GROUP_B', '$GROUP_C');")
WS_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS1';")
WSGRP_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id WHERE g.name IN ('$GROUP_A', '$GROUP_B', '$GROUP_C');")
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
