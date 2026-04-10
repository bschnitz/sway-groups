#!/bin/bash
# Test: group delete with multi-group workspace — orphaned workspaces move to group 0.

SG="$HOME/.cargo/bin/swayg"
DB=~/.local/share/swayg/swayg.db
GROUP_A="__test_group_a__"
GROUP_B="__test_group_b__"
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

echo -e "\033[1m=== Test: group delete — orphaned workspaces move to group 0 ===\033[0m"

rm -rf "$PID_DIR"
mkdir -p "$PID_DIR"

# --- 1. Precondition checks ---

run "Precondition: $GROUP_A does not exist" \
    'sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '"'$GROUP_A'"';"'
GA_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_A';")
if [ "$GA_COUNT" = "0" ]; then pass "$GROUP_A does not exist in DB"; else fail "$GROUP_A must not exist in DB (count=$GA_COUNT)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi

run "Precondition: $GROUP_B does not exist" \
    'sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '"'$GROUP_B'"';"'
GB_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_B';")
if [ "$GB_COUNT" = "0" ]; then pass "$GROUP_B does not exist in DB"; else fail "$GROUP_B must not exist in DB (count=$GB_COUNT)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi

run "Precondition: $WS1 does not exist in DB" \
    'sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '"'$WS1'"';"'
WS1_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS1';")
if [ "$WS1_COUNT" = "0" ]; then pass "$WS1 does not exist in DB"; else fail "$WS1 must not exist in DB (count=$WS1_COUNT)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi

run "Precondition: $WS2 does not exist in DB" \
    'sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '"'$WS2'"';"'
WS2_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS2';")
if [ "$WS2_COUNT" = "0" ]; then pass "$WS2 does not exist in DB"; else fail "$WS2 must not exist in DB (count=$WS2_COUNT)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi

run "Precondition: $WS1 does not exist in sway" \
    'swaymsg -t get_workspaces 2>/dev/null | jq -r ".[].name" | grep -c "^'"$WS1"'$"'
WS1_SWAY=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[].name' | grep -c "^${WS1}$")
if [ "$WS1_SWAY" = "0" ]; then pass "$WS1 does not exist in sway"; else fail "$WS1 must not exist in sway (count=$WS1_SWAY)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi

run "Precondition: $WS2 does not exist in sway" \
    'swaymsg -t get_workspaces 2>/dev/null | jq -r ".[].name" | grep -c "^'"$WS2"'$"'
WS2_SWAY=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[].name' | grep -c "^${WS2}$")
if [ "$WS2_SWAY" = "0" ]; then pass "$WS2 does not exist in sway"; else fail "$WS2 must not exist in sway (count=$WS2_SWAY)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi

# --- 2. Remember original state ---
ORIG_GROUP=$($SG group active $OUTPUT 2>/dev/null)
ORIG_WS=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')

echo ""
echo "  Original group: '$ORIG_GROUP'"
echo "  Original workspace: '$ORIG_WS'"

# --- 3. Setup ---
run "Setup: init + create groups + workspaces + kitties" \
    '$SG init >/dev/null 2>&1
    $SG group select $OUTPUT '"'$GROUP_A'"' --create 2>&1
    kitty --class "'$WS1'" >/dev/null 2>&1 & echo $! > '"$PID_DIR/$WS1.pid"'
    sleep 0.5
    $SG container move "'$WS1'" --switch-to-workspace 2>&1
    kitty --class "'$WS2'" >/dev/null 2>&1 & echo $! > '"$PID_DIR/$WS2.pid"'
    sleep 0.5
    $SG container move "'$WS2'" --switch-to-workspace 2>&1
    $SG group select $OUTPUT '"'$GROUP_B'"' --create 2>&1
    $SG workspace add "'$WS1'" 2>&1
    $SG group select $OUTPUT '"'$ORIG_GROUP'"' 2>&1'

run "Verify setup" \
    'true'

GA_EXISTS=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_A';")
if [ "$GA_EXISTS" = "1" ]; then pass "group '$GROUP_A' exists"; else fail "group '$GROUP_A' exists (count=$GA_EXISTS)"; fi

GB_EXISTS=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_B';")
if [ "$GB_EXISTS" = "1" ]; then pass "group '$GROUP_B' exists"; else fail "group '$GROUP_B' exists (count=$GB_EXISTS)"; fi

WS1_IN_GA=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS1' AND g.name = '$GROUP_A';")
if [ "$WS1_IN_GA" = "1" ]; then pass "$WS1 is in group '$GROUP_A'"; else fail "$WS1 is in group '$GROUP_A' (count=$WS1_IN_GA)"; fi

WS1_IN_GB=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS1' AND g.name = '$GROUP_B';")
if [ "$WS1_IN_GB" = "1" ]; then pass "$WS1 is in group '$GROUP_B'"; else fail "$WS1 is in group '$GROUP_B' (count=$WS1_IN_GB)"; fi

WS2_IN_GA=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS2' AND g.name = '$GROUP_A';")
if [ "$WS2_IN_GA" = "1" ]; then pass "$WS2 is in group '$GROUP_A'"; else fail "$WS2 is in group '$GROUP_A' (count=$WS2_IN_GA)"; fi

WS1_SWAY=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[].name' | grep -c "^${WS1}$")
if [ "$WS1_SWAY" = "1" ]; then pass "$WS1 is in sway"; else fail "$WS1 is in sway (count=$WS1_SWAY)"; fi

WS2_SWAY=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[].name' | grep -c "^${WS2}$")
if [ "$WS2_SWAY" = "1" ]; then pass "$WS2 is in sway"; else fail "$WS2 is in sway (count=$WS2_SWAY)"; fi

# --- 4. Test: delete without --force should fail ---
run "Delete $GROUP_A without --force (should fail)" \
    '$SG group delete "'$GROUP_A'" 2>&1; true'
# Check that group still exists (command should have failed)
GA_STILL=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_A';")
if [ "$GA_STILL" = "1" ]; then pass "$GROUP_A NOT deleted (no --force)"; else fail "$GROUP_A NOT deleted (no --force) (count=$GA_STILL)"; fi

# --- 5. Test: delete with --force ---
run "Delete $GROUP_A with --force" \
    '$SG group delete "'$GROUP_A'" --force 2>&1'

GA_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_A';")
if [ "$GA_GONE" = "0" ]; then pass "$GROUP_A deleted"; else fail "$GROUP_A deleted (count=$GA_GONE)"; fi

# --- 6. Verify: WS1 (multi-group) still in Group B, WS2 (single-group) moved to group 0 ---
WS1_STILL_IN_GB=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS1' AND g.name = '$GROUP_B';")
if [ "$WS1_STILL_IN_GB" = "1" ]; then pass "$WS1 still in group '$GROUP_B' (had other membership)"; else fail "$WS1 still in group '$GROUP_B' (count=$WS1_STILL_IN_GB)"; fi

WS1_IN_0=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS1' AND g.name = '0';")
if [ "$WS1_IN_0" = "0" ]; then pass "$WS1 NOT in group '0' (still in Group B)"; else fail "$WS1 NOT in group '0' (count=$WS1_IN_0)"; fi

WS2_IN_0=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS2' AND g.name = '0';")
if [ "$WS2_IN_0" = "1" ]; then pass "$WS2 moved to group '0' (was only in Group A)"; else fail "$WS2 moved to group '0' (count=$WS2_IN_0)"; fi

WS1_IN_DB=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS1';")
if [ "$WS1_IN_DB" = "1" ]; then pass "$WS1 still in DB"; else fail "$WS1 still in DB (count=$WS1_IN_DB)"; fi

WS2_IN_DB=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS2';")
if [ "$WS2_IN_DB" = "1" ]; then pass "$WS2 still in DB"; else fail "$WS2 still in DB (count=$WS2_IN_DB)"; fi

WS1_SWAY=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[].name' | grep -c "^${WS1}$")
if [ "$WS1_SWAY" = "1" ]; then pass "$WS1 still in sway"; else fail "$WS1 still in sway (count=$WS1_SWAY)"; fi

WS2_SWAY=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[].name' | grep -c "^${WS2}$")
if [ "$WS2_SWAY" = "1" ]; then pass "$WS2 still in sway"; else fail "$WS2 still in sway (count=$WS2_SWAY)"; fi

# --- 7. Verify visibility in Group B ---
swayg group select $OUTPUT "$GROUP_B" >/dev/null 2>&1
OUT_GB=$($SG workspace list --visible --plain --output $OUTPUT 2>/dev/null)
echo "$OUT_GB" | grep -q "$WS1"
if [ $? -eq 0 ]; then pass "$WS1 visible in Group B"; else fail "$WS1 visible in Group B"; fi
echo "$OUT_GB" | grep -q "$WS2"
if [ $? -ne 0 ]; then pass "$WS2 NOT visible in Group B (moved to group 0)"; else fail "$WS2 NOT visible in Group B"; fi

# --- 8. Verify visibility in group 0 ---
$SG group select $OUTPUT "0" >/dev/null 2>&1
OUT_0=$($SG workspace list --visible --plain --output $OUTPUT 2>/dev/null)
echo "$OUT_0" | grep -q "$WS2"
if [ $? -eq 0 ]; then pass "$WS2 visible in group '0'"; else fail "$WS2 visible in group '0'"; fi

# --- 9. Switch back to original group ---
run "Switch back to original group '$ORIG_GROUP'" \
    '$SG group select $OUTPUT '"'$ORIG_GROUP'"' 2>&1'
FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$ORIG_WS" ]; then pass "focused on original workspace '$ORIG_WS'"; else fail "focused on original workspace '$ORIG_WS' (got '$FOCUSED')"; fi

# --- 10. Cleanup: kill kitties, clean up Group B ---
run "Kill kitties" \
    'kill $(cat '"$PID_DIR/$WS1.pid"') 2>/dev/null; kill $(cat '"$PID_DIR/$WS2.pid"') 2>/dev/null; sleep 0.5'
WS1_GONE=$(swaymsg -t get_tree 2>/dev/null | jq -r '.. | objects | select(.app_id? != null) | .app_id' | grep -c "^${WS1}$")
if [ "$WS1_GONE" = "0" ]; then pass "kitty '$WS1' is gone"; else fail "kitty '$WS1' is gone (count=$WS1_GONE)"; fi
WS2_GONE=$(swaymsg -t get_tree 2>/dev/null | jq -r '.. | objects | select(.app_id? != null) | .app_id' | grep -c "^${WS2}$")
if [ "$WS2_GONE" = "0" ]; then pass "kitty '$WS2' is gone"; else fail "kitty '$WS2' is gone (count=$WS2_GONE)"; fi

run "Switch to Group B then back (auto-delete Group B)" \
    '$SG group select $OUTPUT '"'$GROUP_B'"' 2>&1'
run "Switch back" \
    '$SG group select $OUTPUT '"'$ORIG_GROUP'"' 2>&1'
GB_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_B';")
if [ "$GB_GONE" = "0" ]; then pass "$GROUP_B auto-deleted"; else fail "$GROUP_B auto-deleted (count=$GB_GONE)"; fi

# --- 11. Post-condition ---
run "Init to sync DB state" \
    '$SG init >/dev/null 2>&1'

run "Post-condition: no test data in DB" \
    'sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name IN ('"'"$GROUP_A"'"', '"'"$GROUP_B"'"');"; sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name IN ('"'"$WS1"'"', '"'"$WS2"'"');"; sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id WHERE g.name IN ('"'"$GROUP_A"'"', '"'"$GROUP_B"'"');"'

GROUP_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name IN ('$GROUP_A', '$GROUP_B');")
WS_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name IN ('$WS1', '$WS2');")
WSGRP_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id WHERE g.name IN ('$GROUP_A', '$GROUP_B');")
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
