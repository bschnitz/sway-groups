#!/bin/bash
# Test: auto-delete with multi-group workspace.

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

echo -e "\033[1m=== Test: auto-delete with multi-group workspace ===\033[0m"

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

# --- 3. Setup (one run() block) ---
run "Setup: init + create groups + workspaces + kitties" \
    '$SG init >/dev/null 2>&1
    $SG group select $OUTPUT '"'$GROUP_A'"' --create 2>&1
    kitty --class "'$WS1'" >/dev/null 2>&1 & echo $! > '"$PID_DIR/$WS1.pid"'
    sleep 0.5
    $SG container move "'$WS1'" --switch-to-workspace 2>&1
    $SG group select $OUTPUT '"'$GROUP_B'"' --create 2>&1
    $SG workspace add "'$WS1'" 2>&1
    kitty --class "'$WS2'" >/dev/null 2>&1 & echo $! > '"$PID_DIR/$WS2.pid"'
    sleep 0.5
    $SG container move "'$WS2'" --switch-to-workspace 2>&1
    $SG group select $OUTPUT '"'$ORIG_GROUP'"' 2>&1'

# --- 4. Verify setup ---
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

WS2_IN_GB=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS2' AND g.name = '$GROUP_B';")
if [ "$WS2_IN_GB" = "1" ]; then pass "$WS2 is in group '$GROUP_B'"; else fail "$WS2 is in group '$GROUP_B' (count=$WS2_IN_GB)"; fi

WS1_SWAY=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[].name' | grep -c "^${WS1}$")
if [ "$WS1_SWAY" = "1" ]; then pass "$WS1 is in sway"; else fail "$WS1 is in sway (count=$WS1_SWAY)"; fi

WS2_SWAY=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[].name' | grep -c "^${WS2}$")
if [ "$WS2_SWAY" = "1" ]; then pass "$WS2 is in sway"; else fail "$WS2 is in sway (count=$WS2_SWAY)"; fi

# --- 5. Test: switch to Group A, back — Group A should NOT auto-delete ---
run "Switch to Group A" \
    '$SG group select $OUTPUT '"'$GROUP_A'"' 2>&1'
ACTIVE=$($SG group active $OUTPUT 2>/dev/null)
if [ "$ACTIVE" = "$GROUP_A" ]; then pass "active group = '$GROUP_A'"; else fail "active group = '$GROUP_A' (got '$ACTIVE')"; fi

run "Switch back to original group '$ORIG_GROUP'" \
    '$SG group select $OUTPUT '"'$ORIG_GROUP'"' 2>&1'
FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$ORIG_WS" ]; then pass "focused on original workspace '$ORIG_WS'"; else fail "focused on original workspace '$ORIG_WS' (got '$FOCUSED')"; fi
GA_STILL=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_A';")
if [ "$GA_STILL" = "1" ]; then pass "$GROUP_A NOT auto-deleted (WS1 still in sway)"; else fail "$GROUP_A NOT auto-deleted (count=$GA_STILL)"; fi

# --- 6. Kill kitty WS1, verify WS1 gone from sway ---
run "Kill kitty $WS1" \
    'kill $(cat '"$PID_DIR/$WS1.pid"') 2>/dev/null; sleep 0.5'
WS1_GONE=$(swaymsg -t get_tree 2>/dev/null | jq -r '.. | objects | select(.app_id? != null) | .app_id' | grep -c "^${WS1}$")
if [ "$WS1_GONE" = "0" ]; then pass "kitty '$WS1' is gone"; else fail "kitty '$WS1' is gone (count=$WS1_GONE)"; fi

run "Switch to orig workspace (let sway clean up empty $WS1)" \
    'swaymsg workspace "'$ORIG_WS'" >/dev/null 2>&1; sleep 0.5'
WS1_SWAY_GONE=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[].name' | grep -c "^${WS1}$")
if [ "$WS1_SWAY_GONE" = "0" ]; then pass "$WS1 gone from sway"; else fail "$WS1 gone from sway (count=$WS1_SWAY_GONE)"; fi

# --- 7. Test: switch to Group A, back — Group A NOW auto-deleted ---
run "Switch to Group A (WS1 in DB but gone from sway)" \
    '$SG group select $OUTPUT '"'$GROUP_A'"' 2>&1'
ACTIVE=$($SG group active $OUTPUT 2>/dev/null)
if [ "$ACTIVE" = "$GROUP_A" ]; then pass "active group = '$GROUP_A'"; else fail "active group = '$GROUP_A' (got '$ACTIVE')"; fi

run "Switch back to original group '$ORIG_GROUP'" \
    '$SG group select $OUTPUT '"'$ORIG_GROUP'"' 2>&1'
FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$ORIG_WS" ]; then pass "focused on original workspace '$ORIG_WS'"; else fail "focused on original workspace '$ORIG_WS' (got '$FOCUSED')"; fi
GA_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_A';")
if [ "$GA_GONE" = "0" ]; then pass "$GROUP_A auto-deleted (WS1 gone from sway, no non-global workspaces)"; else fail "$GROUP_A auto-deleted (count=$GA_GONE)"; fi

# --- 8. Cleanup: Group B should NOT auto-delete (still has WS2) ---
run "Switch to Group B then back (Group B should survive)" \
    '$SG group select $OUTPUT '"'$GROUP_B'"' 2>&1'
run "Switch back" \
    '$SG group select $OUTPUT '"'$ORIG_GROUP'"' 2>&1'
FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$ORIG_WS" ]; then pass "focused on '$ORIG_WS'"; else fail "focused on '$ORIG_WS' (got '$FOCUSED')"; fi
GB_STILL=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_B';")
if [ "$GB_STILL" = "1" ]; then pass "$GROUP_B NOT auto-deleted (still has WS2)"; else fail "$GROUP_B NOT auto-deleted (count=$GB_STILL)"; fi

run "Kill kitty $WS2" \
    'kill $(cat '"$PID_DIR/$WS2.pid"') 2>/dev/null; sleep 0.5'
WS2_GONE=$(swaymsg -t get_tree 2>/dev/null | jq -r '.. | objects | select(.app_id? != null) | .app_id' | grep -c "^${WS2}$")
if [ "$WS2_GONE" = "0" ]; then pass "kitty '$WS2' is gone"; else fail "kitty '$WS2' is gone (count=$WS2_GONE)"; fi

run "Switch to Group B then back (NOW auto-delete Group B)" \
    '$SG group select $OUTPUT '"'$GROUP_B'"' 2>&1'
run "Switch back" \
    '$SG group select $OUTPUT '"'$ORIG_GROUP'"' 2>&1'
FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$ORIG_WS" ]; then pass "focused on '$ORIG_WS'"; else fail "focused on '$ORIG_WS' (got '$FOCUSED')"; fi
GB_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_B';")
if [ "$GB_GONE" = "0" ]; then pass "$GROUP_B auto-deleted"; else fail "$GROUP_B auto-deleted (count=$GB_GONE)"; fi

# --- 9. Post-condition ---
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
