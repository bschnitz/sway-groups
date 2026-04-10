#!/bin/bash
# Test: workspace remove from one group keeps other membership.

SG="$HOME/.cargo/bin/swayg"
DB=~/.local/share/swayg/swayg.db
GROUP_A="__test_group_a__"
GROUP_B="__test_group_b__"
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

echo -e "\033[1m=== Test: workspace remove from one group keeps other membership ===\033[0m"

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

# --- 3. Init ---
run "Init fresh DB" \
    '$SG init >/dev/null 2>&1'
if [ $? -eq 0 ]; then pass "init succeeded"; else fail "init failed"; fi

# --- 4. Create Group A ---
run "Create Group A" \
    '$SG group select $OUTPUT '"'$GROUP_A'"' --create 2>&1'
GA_EXISTS=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_A';")
if [ "$GA_EXISTS" = "1" ]; then pass "group '$GROUP_A' was created"; else fail "group '$GROUP_A' was created (count=$GA_EXISTS)"; fi
ACTIVE=$($SG group active $OUTPUT 2>/dev/null)
if [ "$ACTIVE" = "$GROUP_A" ]; then pass "active group = '$GROUP_A'"; else fail "active group = '$GROUP_A' (got '$ACTIVE')"; fi

# --- 5. Launch kitty WS1, move to WS1 ---
run "Launch kitty with app_id $WS1" \
    'kitty --class "'$WS1'" >/dev/null 2>&1 & echo $! > '"$PID_DIR/$WS1.pid"'; sleep 0.5'
WS1_APPID=$(swaymsg -t get_tree 2>/dev/null | jq -r '.. | objects | select(.app_id? != null) | .app_id' | grep -c "^${WS1}$")
if [ "$WS1_APPID" -ge 1 ]; then pass "kitty with app_id '$WS1' is running"; else fail "kitty with app_id '$WS1' is running (count=$WS1_APPID)"; fi

run "Move container to $WS1" \
    '$SG container move "'$WS1'" --switch-to-workspace 2>&1'
FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$WS1" ]; then pass "focused on '$WS1'"; else fail "focused on '$WS1' (got '$FOCUSED')"; fi
WS1_IN_GA=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS1' AND g.name = '$GROUP_A';")
if [ "$WS1_IN_GA" = "1" ]; then pass "$WS1 is in group '$GROUP_A'"; else fail "$WS1 is in group '$GROUP_A' (count=$WS1_IN_GA)"; fi

# --- 6. Switch to Group B ---
run "Switch to Group B" \
    '$SG group select $OUTPUT '"'$GROUP_B'"' --create 2>&1'
ACTIVE=$($SG group active $OUTPUT 2>/dev/null)
if [ "$ACTIVE" = "$GROUP_B" ]; then pass "active group = '$GROUP_B'"; else fail "active group = '$GROUP_B' (got '$ACTIVE')"; fi
GA_STILL=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_A';")
if [ "$GA_STILL" = "1" ]; then pass "$GROUP_A NOT auto-deleted (still has WS1)"; else fail "$GROUP_A NOT auto-deleted (count=$GA_STILL)"; fi

# --- 7. Add WS1 to Group B (multi-group) ---
run "workspace add $WS1 (add to Group B)" \
    '$SG workspace add "'$WS1'" 2>&1'
WS1_IN_GA=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS1' AND g.name = '$GROUP_A';")
if [ "$WS1_IN_GA" = "1" ]; then pass "$WS1 still in group '$GROUP_A'"; else fail "$WS1 still in group '$GROUP_A' (count=$WS1_IN_GA)"; fi
WS1_IN_GB=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS1' AND g.name = '$GROUP_B';")
if [ "$WS1_IN_GB" = "1" ]; then pass "$WS1 is in group '$GROUP_B'"; else fail "$WS1 is in group '$GROUP_B' (count=$WS1_IN_GB)"; fi

# --- 8. Remove WS1 from Group B (active) ---
run "Remove $WS1 from Group B" \
    '$SG workspace remove "'$WS1'" 2>&1'

WS1_NOT_IN_GB=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS1' AND g.name = '$GROUP_B';")
if [ "$WS1_NOT_IN_GB" = "0" ]; then pass "$WS1 NOT in group '$GROUP_B'"; else fail "$WS1 NOT in group '$GROUP_B' (count=$WS1_NOT_IN_GB)"; fi

WS1_IN_GA=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS1' AND g.name = '$GROUP_A';")
if [ "$WS1_IN_GA" = "1" ]; then pass "$WS1 still in group '$GROUP_A'"; else fail "$WS1 still in group '$GROUP_A' (count=$WS1_IN_GA)"; fi

WS1_IN_DB=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS1';")
if [ "$WS1_IN_DB" = "1" ]; then pass "$WS1 still in DB"; else fail "$WS1 still in DB (count=$WS1_IN_DB)"; fi

WS1_SWAY=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[].name' | grep -c "^${WS1}$")
if [ "$WS1_SWAY" = "1" ]; then pass "$WS1 still in sway"; else fail "$WS1 still in sway (count=$WS1_SWAY)"; fi

GA_STILL=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_A';")
if [ "$GA_STILL" = "1" ]; then pass "$GROUP_A still exists"; else fail "$GROUP_A still exists (count=$GA_STILL)"; fi

# --- 9. Verify: WS1 NOT visible in Group B ---
OUT_VISIBLE=$($SG workspace list --visible --plain --output $OUTPUT 2>/dev/null)
echo "$OUT_VISIBLE" | grep -q "$WS1"
if [ $? -ne 0 ]; then pass "$WS1 is NOT visible in Group B"; else fail "$WS1 is NOT visible in Group B"; fi

# --- 10. Switch to Group A, verify WS1 visible ---
run "Switch to Group A" \
    '$SG group select $OUTPUT '"'$GROUP_A'"' 2>&1'
ACTIVE=$($SG group active $OUTPUT 2>/dev/null)
if [ "$ACTIVE" = "$GROUP_A" ]; then pass "active group = '$GROUP_A'"; else fail "active group = '$GROUP_A' (got '$ACTIVE')"; fi

OUT_VISIBLE_A=$($SG workspace list --visible --plain --output $OUTPUT 2>/dev/null)
echo "$OUT_VISIBLE_A" | grep -q "$WS1"
if [ $? -eq 0 ]; then pass "$WS1 is visible in Group A"; else fail "$WS1 is visible in Group A"; fi

# --- 11. Switch back to original group (Group A should NOT auto-delete) ---
run "Switch back to original group '$ORIG_GROUP'" \
    '$SG group select $OUTPUT '"'$ORIG_GROUP'"' 2>&1'
FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$ORIG_WS" ]; then pass "focused on original workspace '$ORIG_WS'"; else fail "focused on original workspace '$ORIG_WS' (got '$FOCUSED')"; fi
GA_STILL=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_A';")
if [ "$GA_STILL" = "1" ]; then pass "$GROUP_A NOT auto-deleted (WS1 still in sway)"; else fail "$GROUP_A NOT auto-deleted (count=$GA_STILL)"; fi

# --- 12. Kill kitty ---
run "Kill kitty $WS1" \
    'kill $(cat '"$PID_DIR/$WS1.pid"') 2>/dev/null; sleep 0.5'
WS1_GONE=$(swaymsg -t get_tree 2>/dev/null | jq -r '.. | objects | select(.app_id? != null) | .app_id' | grep -c "^${WS1}$")
if [ "$WS1_GONE" = "0" ]; then pass "kitty '$WS1' is gone"; else fail "kitty '$WS1' is gone (count=$WS1_GONE)"; fi

# --- 13. Switch to Group A then back (NOW auto-delete Group A) ---
run "Switch to Group A (should still exist)" \
    '$SG group select $OUTPUT '"'$GROUP_A'"' 2>&1'
run "Switch back" \
    '$SG group select $OUTPUT '"'$ORIG_GROUP'"' 2>&1'
FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$ORIG_WS" ]; then pass "focused on '$ORIG_WS'"; else fail "focused on '$ORIG_WS' (got '$FOCUSED')"; fi
GA_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_A';")
if [ "$GA_GONE" = "0" ]; then pass "$GROUP_A auto-deleted (WS1 gone from sway)"; else fail "$GROUP_A auto-deleted (count=$GA_GONE)"; fi

# --- 14. Post-condition ---
run "Init to sync DB state" \
    '$SG init >/dev/null 2>&1'

run "Post-condition: no test data in DB" \
    'sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name IN ('"'"$GROUP_A"'"', '"'"$GROUP_B"'"');"; sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '"'"$WS1"'"';"; sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id WHERE g.name IN ('"'"$GROUP_A"'"', '"'"$GROUP_B"'"');"'

GROUP_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name IN ('$GROUP_A', '$GROUP_B');")
WS_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS1';")
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
