#!/bin/bash
# Test: workspace list — output format, status markers (visible), (hidden), (global), (plain).

SG="$HOME/.cargo/bin/swayg"
DB=~/.local/share/swayg/swayg.db
GROUP="__test_vis__"
WS_A="__tg_vis__"
WS_B="__tg_hid__"
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

echo -e "\033[1m=== Test: workspace list output format ===\033[0m"

rm -rf "$PID_DIR"
mkdir -p "$PID_DIR"

# --- 1. Precondition checks (BEFORE init) ---

run "Precondition: $GROUP does not exist" \
    'sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '"'$GROUP'"';"'
G_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP';")
if [ "$G_COUNT" = "0" ]; then pass "$GROUP does not exist in DB"; else fail "$GROUP must not exist in DB (count=$G_COUNT)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi

for WS in $WS_A $WS_B; do
    run "Precondition: $WS does not exist in DB" \
        'sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '"'$WS'"';"'
    WS_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS';")
    if [ "$WS_COUNT" = "0" ]; then pass "$WS does not exist in DB"; else fail "$WS must not exist in DB (count=$WS_COUNT)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi
done

for WS in $WS_A $WS_B; do
    run "Precondition: $WS does not exist in sway" \
        'swaymsg -t get_workspaces 2>/dev/null | jq -r ".[].name" | grep -c "^'"$WS"'$"'
    WS_SWAY=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[].name' | grep -c "^${WS}$")
    if [ "$WS_SWAY" = "0" ]; then pass "$WS does not exist in sway"; else fail "$WS must not exist in sway (count=$WS_SWAY)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi
done

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
run "Setup: init + group + 2 kitties + move + switch back" \
    "$SG init >/dev/null 2>&1
    $SG group select $OUTPUT '$GROUP' --create 2>&1
    kitty --class '$WS_A' >/dev/null 2>&1 & echo \$! > $PID_DIR/$WS_A.pid
    sleep 0.5
    $SG container move '$WS_A' --switch-to-workspace 2>&1
    kitty --class '$WS_B' >/dev/null 2>&1 & echo \$! > $PID_DIR/$WS_B.pid
    sleep 0.5
    $SG container move '$WS_B' --switch-to-workspace 2>&1
    $SG group select $OUTPUT '$ORIG_GROUP' 2>&1
    sleep 0.1"

run "Verify setup" 'true'

G_EXISTS=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP';")
if [ "$G_EXISTS" = "1" ]; then pass "group '$GROUP' exists"; else fail "group '$GROUP' exists (count=$G_EXISTS)"; fi

for WS in $WS_A $WS_B; do
    APPID_COUNT=$(swaymsg -t get_tree 2>/dev/null | jq -r '.. | objects | select(.app_id? != null) | .app_id' | grep -c "^${WS}$")
    if [ "$APPID_COUNT" -ge 1 ]; then pass "kitty '$WS' is running"; else fail "kitty '$WS' is running (count=$APPID_COUNT)"; fi
done

for WS in $WS_A $WS_B; do
    WS_IN_G=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS' AND g.name = '$GROUP';")
    if [ "$WS_IN_G" = "1" ]; then pass "'$WS' in group '$GROUP'"; else fail "'$WS' in group '$GROUP' (count=$WS_IN_G)"; fi
done

FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$ORIG_WS" ]; then pass "focused on original workspace '$ORIG_WS'"; else fail "focused on original workspace '$ORIG_WS' (got '$FOCUSED')"; fi

# --- 4. Test: workspace list --visible (only active group's workspaces) ---
run "workspace list --visible --output $OUTPUT" \
    "$SG workspace list --visible --output $OUTPUT 2>/dev/null"

VIS_OUT=$($SG workspace list --visible --output $OUTPUT 2>/dev/null)
echo "  Visible output: $(echo "$VIS_OUT" | head -5)"
echo "$VIS_OUT" | grep -q "$WS_A"
if [ $? -ne 0 ]; then pass "'$WS_A' NOT in visible list (different active group)"; else fail "'$WS_A' NOT in visible list (different active group)"; fi
echo "$VIS_OUT" | grep -q "$WS_B"
if [ $? -ne 0 ]; then pass "'$WS_B' NOT in visible list"; else fail "'$WS_B' NOT in visible list"; fi

# --- 5. Test: workspace list --output (all with status markers) ---
run "workspace list --output $OUTPUT" \
    "$SG workspace list --output $OUTPUT 2>/dev/null"

ALL_OUT=$($SG workspace list --output $OUTPUT 2>/dev/null)
echo "  All output: $(echo "$ALL_OUT" | head -10)"
echo "$ALL_OUT" | grep -q "$WS_A"
if [ $? -eq 0 ]; then pass "'$WS_A' in full list"; else fail "'$WS_A' in full list"; fi
echo "$ALL_OUT" | grep -q "$WS_B"
if [ $? -eq 0 ]; then pass "'$WS_B' in full list"; else fail "'$WS_B' in full list"; fi
echo "$ALL_OUT" | grep -q "$WS_A.*hidden"
if [ $? -eq 0 ]; then pass "'$WS_A' marked as (hidden)"; else fail "'$WS_A' marked as (hidden)"; fi
echo "$ALL_OUT" | grep -q "$WS_B.*hidden"
if [ $? -eq 0 ]; then pass "'$WS_B' marked as (hidden)"; else fail "'$WS_B' marked as (hidden)"; fi
echo "$ALL_OUT" | grep -q "$ORIG_WS.*visible"
if [ $? -eq 0 ]; then pass "'$ORIG_WS' marked as (visible)"; else fail "'$ORIG_WS' marked as (visible)"; fi

# --- 6. Test: make WS_A global, check (global) marker ---
run "workspace global $WS_A" \
    "$SG workspace global '$WS_A' 2>&1"

IS_GLOBAL=$(sqlite3 "$DB" "SELECT is_global FROM workspaces WHERE name = '$WS_A';")
if [ "$IS_GLOBAL" = "1" ]; then pass "'$WS_A' is global in DB"; else fail "'$WS_A' is global in DB (got '$IS_GLOBAL')"; fi

run "workspace list --output $OUTPUT (after global)" \
    "$SG workspace list --output $OUTPUT 2>/dev/null"

GLOBAL_OUT=$($SG workspace list --output $OUTPUT 2>/dev/null)
echo "  Global output: $(echo "$GLOBAL_OUT" | head -10)"
echo "$GLOBAL_OUT" | grep -q "$WS_A.*global"
if [ $? -eq 0 ]; then pass "'$WS_A' marked as (global)"; else fail "'$WS_A' marked as (global)"; fi

# --- 7. Test: workspace list --plain (no status markers) ---
run "workspace list --plain --output $OUTPUT" \
    "$SG workspace list --plain --output $OUTPUT 2>/dev/null"

PLAIN_OUT=$($SG workspace list --plain --output $OUTPUT 2>/dev/null)
echo "  Plain output: $(echo "$PLAIN_OUT" | head -10)"
echo "$PLAIN_OUT" | grep -q "$WS_A"
if [ $? -eq 0 ]; then pass "'$WS_A' in plain list"; else fail "'$WS_A' in plain list"; fi
echo "$PLAIN_OUT" | grep -q "(global)"
if [ $? -ne 0 ]; then pass "no status markers in plain output"; else fail "no status markers in plain output"; fi
echo "$PLAIN_OUT" | grep -q "(hidden)"
if [ $? -ne 0 ]; then pass "no (hidden) in plain output"; else fail "no (hidden) in plain output"; fi
echo "$PLAIN_OUT" | grep -q "(visible)"
if [ $? -ne 0 ]; then pass "no (visible) in plain output"; else fail "no (visible) in plain output"; fi

# --- 8. Test: workspace list --group (filtered by group) ---
run "workspace list --group $GROUP --output $OUTPUT" \
    "$SG workspace list --group '$GROUP' --output $OUTPUT 2>/dev/null"

GRP_OUT=$($SG workspace list --group "$GROUP" --output $OUTPUT 2>/dev/null)
echo "  Group output: $(echo "$GRP_OUT" | head -10)"
echo "$GRP_OUT" | grep -q "$WS_B"
if [ $? -eq 0 ]; then pass "'$WS_B' in group list"; else fail "'$WS_B' in group list"; fi
echo "$GRP_OUT" | grep -q "^ *$ORIG_WS "
if [ $? -ne 0 ]; then pass "'$ORIG_WS' NOT in group list (different group)"; else fail "'$ORIG_WS' NOT in group list (different group)"; fi

# --- 9. Cleanup ---
run "unglobal $WS_A" \
    "$SG workspace unglobal '$WS_A' 2>&1"

run "Kill kitties" \
    "kill \$(cat $PID_DIR/$WS_A.pid) 2>/dev/null; kill \$(cat $PID_DIR/$WS_B.pid) 2>/dev/null; sleep 0.5"

for WS in $WS_A $WS_B; do
    GONE=$(swaymsg -t get_tree 2>/dev/null | jq -r '.. | objects | select(.app_id? != null) | .app_id' | grep -c "^${WS}$")
    if [ "$GONE" = "0" ]; then pass "kitty '$WS' is gone"; else fail "kitty '$WS' is gone (count=$GONE)"; fi
done

run "Auto-delete $GROUP" \
    "$SG group select $OUTPUT '$GROUP' 2>&1"
run "Switch back" \
    "$SG group select $OUTPUT '$ORIG_GROUP' 2>&1"
G_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP';")
if [ "$G_GONE" = "0" ]; then pass "'$GROUP' auto-deleted"; else fail "'$GROUP' auto-deleted (count=$G_GONE)"; fi

FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$ORIG_WS" ]; then pass "focused on original workspace '$ORIG_WS'"; else fail "focused on original workspace '$ORIG_WS' (got '$FOCUSED')"; fi

# --- 10. Post-condition ---
run "Init to sync DB state" \
    '$SG init >/dev/null 2>&1'

run "Post-condition: no test data in DB" \
    'sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '"'$GROUP'"';"; sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name IN ('"'"$WS_A"'"', '"'"$WS_B"'"');"'

GROUP_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP';")
WS_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name IN ('$WS_A', '$WS_B');")
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
