#!/bin/bash
# Test: group prune — remove effectively empty groups, --keep flag.

SG="$HOME/.cargo/bin/swayg"
DB=~/.local/share/swayg/swayg.db
GROUP_A="__test_pa__"
GROUP_B="__test_pb__"
GROUP_C="__test_pc__"
GROUP_D="__test_pd__"
GROUP_E="__test_pe__"
GROUP_F="__test_pf__"
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

echo -e "\033[1m=== Test: group prune ===\033[0m"

rm -rf "$PID_DIR"
mkdir -p "$PID_DIR"

# --- 1. Precondition checks (BEFORE init) ---

for G in $GROUP_A $GROUP_B $GROUP_C $GROUP_D $GROUP_E $GROUP_F; do
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
run "Setup: init + create 4 groups + kitty in group A + switch back" \
    "$SG init >/dev/null 2>&1
    $SG group create '$GROUP_A' 2>&1
    $SG group create '$GROUP_B' 2>&1
    $SG group create '$GROUP_C' 2>&1
    $SG group create '$GROUP_D' 2>&1
    $SG group select $OUTPUT '$GROUP_A' --create 2>&1
    kitty --class '$WS1' >/dev/null 2>&1 & echo \$! > $PID_DIR/$WS1.pid
    sleep 0.5
    $SG container move '$WS1' --switch-to-workspace 2>&1
    $SG group select $OUTPUT '$ORIG_GROUP' 2>&1
    sleep 0.1"

run "Verify setup" 'true'

for G in $GROUP_A $GROUP_B $GROUP_C $GROUP_D; do
    G_EXISTS=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$G';")
    if [ "$G_EXISTS" = "1" ]; then pass "group '$G' exists"; else fail "group '$G' exists (count=$G_EXISTS)"; fi
done

APPID_COUNT=$(swaymsg -t get_tree 2>/dev/null | jq -r '.. | objects | select(.app_id? != null) | .app_id' | grep -c "^${WS1}$")
if [ "$APPID_COUNT" -ge 1 ]; then pass "kitty '$WS1' is running"; else fail "kitty '$WS1' is running (count=$APPID_COUNT)"; fi

WS1_IN_GA=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS1' AND g.name = '$GROUP_A';")
if [ "$WS1_IN_GA" = "1" ]; then pass "'$WS1' in group '$GROUP_A'"; else fail "'$WS1' in group '$GROUP_A' (count=$WS1_IN_GA)"; fi

for G in $GROUP_B $GROUP_C $GROUP_D; do
    WS1_NOT=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS1' AND g.name = '$G';")
    if [ "$WS1_NOT" = "0" ]; then pass "'$WS1' NOT in group '$G'"; else fail "'$WS1' NOT in group '$G' (count=$WS1_NOT)"; fi
done

GROUP0_EXISTS=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '0';")
if [ "$GROUP0_EXISTS" = "1" ]; then pass "group '0' exists"; else fail "group '0' exists (count=$GROUP0_EXISTS)"; fi

# --- 4. Test: group prune (no --keep) ---
run "group prune" \
    "$SG group prune 2>&1; PRUNE_OUT=\"\$?\""

PRUNE_OUT=${PRUNE_OUT:-0}
pass "prune command executed"

GA_STILL=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_A';")
if [ "$GA_STILL" = "1" ]; then pass "group '$GROUP_A' still exists (has WS1)"; else fail "group '$GROUP_A' still exists (count=$GA_STILL)"; fi

GB_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_B';")
if [ "$GB_GONE" = "0" ]; then pass "group '$GROUP_B' pruned"; else fail "group '$GROUP_B' pruned (count=$GB_GONE)"; fi

GC_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_C';")
if [ "$GC_GONE" = "0" ]; then pass "group '$GROUP_C' pruned"; else fail "group '$GROUP_C' pruned (count=$GC_GONE)"; fi

GD_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_D';")
if [ "$GD_GONE" = "0" ]; then pass "group '$GROUP_D' pruned"; else fail "group '$GROUP_D' pruned (count=$GD_GONE)"; fi

GROUP0_STILL=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '0';")
if [ "$GROUP0_STILL" = "1" ]; then pass "group '0' NOT pruned (default group)"; else fail "group '0' NOT pruned (count=$GROUP0_STILL)"; fi

# --- 5. Test: group prune with --keep ---
run "Setup: create groups E and F via sqlite3" \
    'sqlite3 "$DB" "INSERT INTO groups (name, created_at, updated_at) VALUES ('"'"$GROUP_E"'"', datetime('"'"'now'"'"'), datetime('"'"'now'"'"'));"
    sqlite3 "$DB" "INSERT INTO groups (name, created_at, updated_at) VALUES ('"'"$GROUP_F"'"', datetime('"'"'now'"'"'), datetime('"'"'now'"'"'));"'

run "Verify: E and F exist" 'true'
GE_EXISTS=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_E';")
if [ "$GE_EXISTS" = "1" ]; then pass "group '$GROUP_E' exists"; else fail "group '$GROUP_E' exists (count=$GE_EXISTS)"; fi

GF_EXISTS=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_F';")
if [ "$GF_EXISTS" = "1" ]; then pass "group '$GROUP_F' exists"; else fail "group '$GROUP_F' exists (count=$GF_EXISTS)"; fi

run "group prune --keep $GROUP_E" \
    "$SG group prune --keep '$GROUP_E' 2>&1"
pass "prune --keep command executed"

GE_STILL=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_E';")
if [ "$GE_STILL" = "1" ]; then pass "group '$GROUP_E' kept (--keep)"; else fail "group '$GROUP_E' kept (count=$GE_STILL)"; fi

GF_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_F';")
if [ "$GF_GONE" = "0" ]; then pass "group '$GROUP_F' pruned (not in --keep)"; else fail "group '$GROUP_F' pruned (count=$GF_GONE)"; fi

# --- 6. Cleanup ---
run "Kill kitty" \
    "kill \$(cat $PID_DIR/$WS1.pid) 2>/dev/null; sleep 0.5"

GONE=$(swaymsg -t get_tree 2>/dev/null | jq -r '.. | objects | select(.app_id? != null) | .app_id' | grep -c "^${WS1}$")
if [ "$GONE" = "0" ]; then pass "kitty '$WS1' is gone"; else fail "kitty '$WS1' is gone (count=$GONE)"; fi

run "Auto-delete $GROUP_A" \
    "$SG group select $OUTPUT '$GROUP_A' 2>&1"
run "Switch back" \
    "$SG group select $OUTPUT '$ORIG_GROUP' 2>&1"
G_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_A';")
if [ "$G_GONE" = "0" ]; then pass "'$GROUP_A' auto-deleted"; else fail "'$GROUP_A' auto-deleted (count=$G_GONE)"; fi

FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$ORIG_WS" ]; then pass "focused on original workspace '$ORIG_WS'"; else fail "focused on original workspace '$ORIG_WS' (got '$FOCUSED')"; fi

# --- 7. Post-condition ---
run "Init to sync DB state" \
    '$SG init >/dev/null 2>&1'

run "Post-condition: no test data in DB" \
    'sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name IN ('"'"$GROUP_A"'"', '"'"$GROUP_B"'"', '"'"$GROUP_C"'"', '"'"$GROUP_D"'"', '"'"$GROUP_E"'"', '"'"$GROUP_F"'"');"; sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '"'"$WS1"'"';"'

GROUP_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name IN ('$GROUP_A', '$GROUP_B', '$GROUP_C', '$GROUP_D', '$GROUP_E', '$GROUP_F');")
WS_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS1';")
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
