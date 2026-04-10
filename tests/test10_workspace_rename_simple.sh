#!/bin/bash
# Test: workspace rename — simple rename (no merge).

SG="$HOME/.cargo/bin/swayg"
DB=~/.local/share/swayg/swayg.db
GROUP="__test_rn__"
WS_SRC="__tg_src__"
WS_DST="__tg_dst__"
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

echo -e "\033[1m=== Test: workspace rename (simple) ===\033[0m"

rm -rf "$PID_DIR"
mkdir -p "$PID_DIR"

# --- 1. Precondition checks (BEFORE init) ---

run "Precondition: $GROUP does not exist" \
    'sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '"'$GROUP'"';"'
G_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP';")
if [ "$G_COUNT" = "0" ]; then pass "$GROUP does not exist in DB"; else fail "$GROUP must not exist in DB (count=$G_COUNT)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi

for WS in $WS_SRC $WS_DST; do
    run "Precondition: $WS does not exist in DB" \
        'sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '"'$WS'"';"'
    WS_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS';")
    if [ "$WS_COUNT" = "0" ]; then pass "$WS does not exist in DB"; else fail "$WS must not exist in DB (count=$WS_COUNT)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi
done

for WS in $WS_SRC $WS_DST; do
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
run "Setup: init + create group + launch kitty + move to src" \
    "$SG init >/dev/null 2>&1
    $SG group select $OUTPUT '$GROUP' --create 2>&1
    kitty --class '$WS_SRC' >/dev/null 2>&1 & echo \$! > $PID_DIR/$WS_SRC.pid
    sleep 0.5
    $SG container move '$WS_SRC' --switch-to-workspace 2>&1
    sleep 0.1"

run "Verify setup" 'true'

G_EXISTS=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP';")
if [ "$G_EXISTS" = "1" ]; then pass "group '$GROUP' exists"; else fail "group '$GROUP' exists (count=$G_EXISTS)"; fi

APPID_COUNT=$(swaymsg -t get_tree 2>/dev/null | jq -r '.. | objects | select(.app_id? != null) | .app_id' | grep -c "^${WS_SRC}$")
if [ "$APPID_COUNT" -ge 1 ]; then pass "kitty '$WS_SRC' is running"; else fail "kitty '$WS_SRC' is running (count=$APPID_COUNT)"; fi

WS_SRC_SWAY=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[].name' | grep -c "^${WS_SRC}$")
if [ "$WS_SRC_SWAY" = "1" ]; then pass "'$WS_SRC' exists in sway"; else fail "'$WS_SRC' exists in sway (count=$WS_SRC_SWAY)"; fi

WS_SRC_IN_G=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS_SRC' AND g.name = '$GROUP';")
if [ "$WS_SRC_IN_G" = "1" ]; then pass "'$WS_SRC' in group '$GROUP'"; else fail "'$WS_SRC' in group '$GROUP' (count=$WS_SRC_IN_G)"; fi

# --- 4. Test: rename src → dst (simple, no merge) ---
run "Rename $WS_SRC to $WS_DST" \
    "$SG workspace rename '$WS_SRC' '$WS_DST' 2>&1; RENAME_EXIT=\$?"

RENAME_EXIT=${RENAME_EXIT:-0}
if [ "$RENAME_EXIT" -eq 0 ]; then pass "rename command succeeded (exit 0)"; else fail "rename command succeeded (exit=$RENAME_EXIT)"; fi

WS_SRC_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS_SRC';")
if [ "$WS_SRC_GONE" = "0" ]; then pass "'$WS_SRC' gone from DB"; else fail "'$WS_SRC' gone from DB (count=$WS_SRC_GONE)"; fi

WS_DST_IN_DB=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS_DST';")
if [ "$WS_DST_IN_DB" = "1" ]; then pass "'$WS_DST' in DB"; else fail "'$WS_DST' in DB (count=$WS_DST_IN_DB)"; fi

WS_DST_IN_G=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS_DST' AND g.name = '$GROUP';")
if [ "$WS_DST_IN_G" = "1" ]; then pass "'$WS_DST' in group '$GROUP'"; else fail "'$WS_DST' in group '$GROUP' (count=$WS_DST_IN_G)"; fi

FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$WS_DST" ]; then pass "focused on '$WS_DST' after rename"; else fail "focused on '$WS_DST' after rename (got '$FOCUSED')"; fi

WS_SRC_KITTY_WS=$(swaymsg -t get_tree 2>/dev/null | jq -r '[.. | objects | select(.type? == "workspace")] | .[] | {name: .name, apps: [.. | objects | select(.app_id? != null) | .app_id]} | select(.apps | index("'$WS_SRC'")) | .name')
if [ "$WS_SRC_KITTY_WS" = "$WS_DST" ]; then pass "kitty '$WS_SRC' still on workspace '$WS_DST'"; else fail "kitty '$WS_SRC' still on workspace '$WS_DST' (got '$WS_SRC_KITTY_WS')"; fi

# --- 5. Test: workspace list shows renamed workspace ---
run "workspace list --plain --group $GROUP" \
    "$SG workspace list --plain --group '$GROUP' 2>/dev/null"

OUT=$($SG workspace list --plain --group "$GROUP" 2>/dev/null)
echo "$OUT" | grep -q "$WS_DST"
if [ $? -eq 0 ]; then pass "'$WS_DST' listed in group via workspace list"; else fail "'$WS_DST' listed in group via workspace list"; fi
echo "$OUT" | grep -q "$WS_SRC"
if [ $? -ne 0 ]; then pass "'$WS_SRC' NOT listed"; else fail "'$WS_SRC' NOT listed"; fi

# --- 6. Cleanup ---
run "Kill kitty" \
    "kill \$(cat $PID_DIR/$WS_SRC.pid) 2>/dev/null; sleep 0.5"

GONE=$(swaymsg -t get_tree 2>/dev/null | jq -r '.. | objects | select(.app_id? != null) | .app_id' | grep -c "^${WS_SRC}$")
if [ "$GONE" = "0" ]; then pass "kitty '$WS_SRC' is gone"; else fail "kitty '$WS_SRC' is gone (count=$GONE)"; fi

run "Switch back to original group '$ORIG_GROUP'" \
    "$SG group select $OUTPUT '$ORIG_GROUP' 2>&1"
FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$ORIG_WS" ]; then pass "focused on original workspace '$ORIG_WS'"; else fail "focused on original workspace '$ORIG_WS' (got '$FOCUSED')"; fi

run "Auto-delete $GROUP" \
    "$SG group select $OUTPUT '$GROUP' 2>&1"
run "Switch back" \
    "$SG group select $OUTPUT '$ORIG_GROUP' 2>&1"
G_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP';")
if [ "$G_GONE" = "0" ]; then pass "'$GROUP' auto-deleted"; else fail "'$GROUP' auto-deleted (count=$G_GONE)"; fi

# --- 7. Post-condition ---
run "Init to sync DB state" \
    '$SG init >/dev/null 2>&1'

run "Post-condition: no test data in DB" \
    'sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '"'$GROUP'"';"; sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name IN ('"'"$WS_SRC"'"', '"'"$WS_DST"'"');"; sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id WHERE g.name = '"'$GROUP'"';"'

GROUP_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP';")
WS_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name IN ('$WS_SRC', '$WS_DST');")
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
