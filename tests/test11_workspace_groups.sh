#!/bin/bash
# Test: workspace groups — show group memberships for a workspace.

SG="$HOME/.cargo/bin/swayg"
DB=~/.local/share/swayg/swayg.db
GROUP_A="__test_ga__"
GROUP_B="__test_gb__"
GROUP_C="__test_gc__"
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

echo -e "\033[1m=== Test: workspace groups ===\033[0m"

rm -rf "$PID_DIR"
mkdir -p "$PID_DIR"

# --- 1. Precondition checks (BEFORE init) ---

for G in $GROUP_A $GROUP_B $GROUP_C; do
    run "Precondition: $G does not exist" \
        'sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '"'$G'"';"'
    G_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$G';")
    if [ "$G_COUNT" = "0" ]; then pass "$G does not exist in DB"; else fail "$G must not exist in DB (count=$G_COUNT)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi
done

run "Precondition: $WS1 does not exist in DB" \
    'sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '"'$WS1'"';"'
WS1_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS1';")
if [ "$WS1_COUNT" = "0" ]; then pass "$WS1 does not exist in DB"; else fail "$WS1 must not exist in DB (count=$WS1_COUNT)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi

run "Precondition: $WS1 does not exist in sway" \
    'swaymsg -t get_workspaces 2>/dev/null | jq -r ".[].name" | grep -c "^'"$WS1"'$"'
WS1_SWAY=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[].name' | grep -c "^${WS1}$")
if [ "$WS1_SWAY" = "0" ]; then pass "$WS1 does not exist in sway"; else fail "$WS1 must not exist in sway (count=$WS1_SWAY)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi

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

# --- 3. Setup (one run() block per rule 46) ---
run "Setup: init + create groups + kitty + move + add to second group" \
    "$SG init >/dev/null 2>&1
    $SG group select $OUTPUT '$GROUP_A' --create 2>&1
    $SG group create '$GROUP_B' 2>&1
    kitty --class '$WS1' >/dev/null 2>&1 & echo \$! > $PID_DIR/$WS1.pid
    sleep 0.5
    $SG container move '$WS1' --switch-to-workspace 2>&1
    $SG workspace add '$WS1' --group '$GROUP_B' 2>&1
    $SG group create '$GROUP_C' 2>&1
    $SG group select $OUTPUT '$ORIG_GROUP' 2>&1
    sleep 0.1"

run "Verify setup" 'true'

GA_EXISTS=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_A';")
if [ "$GA_EXISTS" = "1" ]; then pass "group '$GROUP_A' exists"; else fail "group '$GROUP_A' exists (count=$GA_EXISTS)"; fi

GB_EXISTS=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_B';")
if [ "$GB_EXISTS" = "1" ]; then pass "group '$GROUP_B' exists"; else fail "group '$GROUP_B' exists (count=$GB_EXISTS)"; fi

GC_EXISTS=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_C';")
if [ "$GC_EXISTS" = "1" ]; then pass "group '$GROUP_C' exists"; else fail "group '$GROUP_C' exists (count=$GC_EXISTS)"; fi

WS1_IN_GA=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS1' AND g.name = '$GROUP_A';")
if [ "$WS1_IN_GA" = "1" ]; then pass "'$WS1' in group '$GROUP_A'"; else fail "'$WS1' in group '$GROUP_A' (count=$WS1_IN_GA)"; fi

WS1_IN_GB=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS1' AND g.name = '$GROUP_B';")
if [ "$WS1_IN_GB" = "1" ]; then pass "'$WS1' in group '$GROUP_B'"; else fail "'$WS1' in group '$GROUP_B' (count=$WS1_IN_GB)"; fi

WS1_IN_GC=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS1' AND g.name = '$GROUP_C';")
if [ "$WS1_IN_GC" = "0" ]; then pass "'$WS1' NOT in group '$GROUP_C'"; else fail "'$WS1' NOT in group '$GROUP_C' (count=$WS1_IN_GC)"; fi

# --- 4. Test: workspace groups ---
run "workspace groups $WS1" \
    "$SG workspace groups '$WS1' 2>&1"

GROUPS_OUT=$($SG workspace groups "$WS1" 2>/dev/null)
echo "$GROUPS_OUT" | grep -q "$GROUP_A"
if [ $? -eq 0 ]; then pass "output contains '$GROUP_A'"; else fail "output contains '$GROUP_A'"; fi

echo "$GROUPS_OUT" | grep -q "$GROUP_B"
if [ $? -eq 0 ]; then pass "output contains '$GROUP_B'"; else fail "output contains '$GROUP_B'"; fi

echo "$GROUPS_OUT" | grep -q "$GROUP_C"
if [ $? -ne 0 ]; then pass "output does NOT contain '$GROUP_C'"; else fail "output does NOT contain '$GROUP_C'"; fi

# --- 5. Cleanup ---
run "Kill kitty" \
    "kill \$(cat $PID_DIR/$WS1.pid) 2>/dev/null; sleep 0.5"

GONE=$(swaymsg -t get_tree 2>/dev/null | jq -r '.. | objects | select(.app_id? != null) | .app_id' | grep -c "^${WS1}$")
if [ "$GONE" = "0" ]; then pass "kitty '$WS1' is gone"; else fail "kitty '$WS1' is gone (count=$GONE)"; fi

run "Switch back to original group '$ORIG_GROUP'" \
    "$SG group select $OUTPUT '$ORIG_GROUP' 2>&1"
FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$ORIG_WS" ]; then pass "focused on original workspace '$ORIG_WS'"; else fail "focused on original workspace '$ORIG_WS' (got '$FOCUSED')"; fi

for G in $GROUP_A $GROUP_B $GROUP_C; do
    run "Auto-delete $G" \
        "$SG group select $OUTPUT '$G' 2>&1"
    run "Switch back" \
        "$SG group select $OUTPUT '$ORIG_GROUP' 2>&1"
done

G_ALL_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name IN ('$GROUP_A', '$GROUP_B', '$GROUP_C');")
if [ "$G_ALL_GONE" = "0" ]; then pass "all test groups auto-deleted"; else fail "all test groups auto-deleted (count=$G_ALL_GONE)"; fi

# --- 6. Post-condition ---
run "Init to sync DB state" \
    '$SG init >/dev/null 2>&1'

run "Post-condition: no test data in DB" \
    'sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name IN ('"'"$GROUP_A"'"', '"'"$GROUP_B"'"', '"'"$GROUP_C"'"');"; sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '"'"$WS1"'"';"; sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id WHERE g.name IN ('"'"$GROUP_A"'"', '"'"$GROUP_B"'"', '"'"$GROUP_C"'"');"'

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
