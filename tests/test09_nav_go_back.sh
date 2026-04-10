#!/bin/bash
# Test: nav go / nav back — navigate to specific workspace, go back in history.

SG="$HOME/.cargo/bin/swayg"
DB=~/.local/share/swayg/swayg.db
GROUP="__test_nav2__"
WS_A="__tg_one__"
WS_B="__tg_two__"
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

echo -e "\033[1m=== Test: nav go / nav back ===\033[0m"

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
run "Setup: init + create group + launch 2 kitties + move containers + switch back" \
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
    WS_SWAY=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[].name' | grep -c "^${WS}$")
    if [ "$WS_SWAY" = "1" ]; then pass "'$WS' exists in sway"; else fail "'$WS' exists in sway (count=$WS_SWAY)"; fi
done

FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$ORIG_WS" ]; then pass "focused on original workspace '$ORIG_WS'"; else fail "focused on original workspace '$ORIG_WS' (got '$FOCUSED')"; fi

# --- 4. Test: nav go __tg_one__ ---
run "nav go $WS_A" \
    "$SG nav go '$WS_A' 2>&1"

FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$WS_A" ]; then pass "focused on '$WS_A' after nav go"; else fail "focused on '$WS_A' after nav go (got '$FOCUSED')"; fi

# --- 5. Test: nav go __tg_two__ ---
run "nav go $WS_B" \
    "$SG nav go '$WS_B' 2>&1"

FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$WS_B" ]; then pass "focused on '$WS_B' after nav go"; else fail "focused on '$WS_B' after nav go (got '$FOCUSED')"; fi

# --- 6. Test: nav back (two → one) ---
run "nav back" \
    "$SG nav back 2>&1"

FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$WS_A" ]; then pass "focused on '$WS_A' after nav back"; else fail "focused on '$WS_A' after nav back (got '$FOCUSED')"; fi

# --- 7. Test: nav back (one → two, alternation) ---
run "nav back" \
    "$SG nav back 2>&1"

FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$WS_B" ]; then pass "focused on '$WS_B' after nav back (alternation)"; else fail "focused on '$WS_B' after nav back (alternation) (got '$FOCUSED')"; fi

# --- 8. Test: nav go original workspace ---
run "nav go $ORIG_WS" \
    "$SG nav go '$ORIG_WS' 2>&1"

FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$ORIG_WS" ]; then pass "focused on '$ORIG_WS' after nav go"; else fail "focused on '$ORIG_WS' after nav go (got '$FOCUSED')"; fi

# --- 9. Test: nav back (orig → two) ---
run "nav back" \
    "$SG nav back 2>&1"

FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$WS_B" ]; then pass "focused on '$WS_B' after nav back (from orig)"; else fail "focused on '$WS_B' after nav back (from orig) (got '$FOCUSED')"; fi

# --- 10. Test: nav back (two → orig) ---
run "nav back" \
    "$SG nav back 2>&1"

FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$ORIG_WS" ]; then pass "focused on '$ORIG_WS' after nav back (from two)"; else fail "focused on '$ORIG_WS' after nav back (from two) (got '$FOCUSED')"; fi

# --- 11. Cleanup ---
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

# --- 12. Post-condition ---
run "Init to sync DB state" \
    '$SG init >/dev/null 2>&1'

run "Post-condition: no test data in DB" \
    'sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '"'$GROUP'"';"; sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name IN ('"'"$WS_A"'"', '"'"$WS_B"'"');"; sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id WHERE g.name = '"'$GROUP'"';"'

GROUP_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP';")
WS_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name IN ('$WS_A', '$WS_B');")
WSGRP_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id WHERE g.name = '$GROUP';")
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
