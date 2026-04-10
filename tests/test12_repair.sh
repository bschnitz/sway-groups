#!/bin/bash
# Test: repair — DB↔sway reconciliation (stale WS, missing WS, empty groups).

SG="$HOME/.cargo/bin/swayg"
DB=~/.local/share/swayg/swayg.db
GROUP="__test_repair__"
GROUP_EMPTY="__test_empty__"
WS1="__tg_ws1__"
WS_STALE="__tg_stale__"
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

echo -e "\033[1m=== Test: repair ===\033[0m"

rm -rf "$PID_DIR"
mkdir -p "$PID_DIR"

# --- 1. Precondition checks (BEFORE init) ---

for G in $GROUP $GROUP_EMPTY; do
    run "Precondition: $G does not exist" \
        'sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '"'$G'"';"'
    G_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$G';")
    if [ "$G_COUNT" = "0" ]; then pass "$G does not exist in DB"; else fail "$G must not exist in DB (count=$G_COUNT)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi
done

for WS in $WS1 $WS_STALE; do
    run "Precondition: $WS does not exist in DB" \
        'sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '"'$WS'"';"'
    WS_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS';")
    if [ "$WS_COUNT" = "0" ]; then pass "$WS does not exist in DB"; else fail "$WS must not exist in DB (count=$WS_COUNT)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi
done

for WS in $WS1 $WS_STALE; do
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
run "Setup: init + group + kitty + move + switch back + DB manipulation" \
    "$SG init >/dev/null 2>&1
    $SG group select $OUTPUT '$GROUP' --create 2>&1
    kitty --class '$WS1' >/dev/null 2>&1 & echo \$! > $PID_DIR/$WS1.pid
    sleep 0.5
    $SG container move '$WS1' --switch-to-workspace 2>&1
    $SG group select $OUTPUT '$ORIG_GROUP' 2>&1
    $SG group create '$GROUP_EMPTY' 2>&1
    sqlite3 \$DB \"INSERT INTO workspaces (name, is_global, created_at, updated_at) VALUES ('$WS_STALE', 0, datetime('now'), datetime('now'));\"
    sqlite3 \$DB \"INSERT INTO workspace_groups (workspace_id, group_id, created_at) SELECT w.id, g.id, datetime('now') FROM workspaces w, groups g WHERE w.name = '$WS_STALE' AND g.name = '$GROUP';\"
    sqlite3 \$DB \"DELETE FROM workspace_groups WHERE workspace_id IN (SELECT id FROM workspaces WHERE name = '$WS1');\"
    sqlite3 \$DB \"DELETE FROM workspaces WHERE name = '$WS1';\"
    sleep 0.1"

run "Verify setup" 'true'

G_EXISTS=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP';")
if [ "$G_EXISTS" = "1" ]; then pass "group '$GROUP' exists"; else fail "group '$GROUP' exists (count=$G_EXISTS)"; fi

GE_EXISTS=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_EMPTY';")
if [ "$GE_EXISTS" = "1" ]; then pass "group '$GROUP_EMPTY' exists"; else fail "group '$GROUP_EMPTY' exists (count=$GE_EXISTS)"; fi

STALE_IN_DB=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS_STALE';")
if [ "$STALE_IN_DB" = "1" ]; then pass "'$WS_STALE' in DB (not in sway)"; else fail "'$WS_STALE' in DB (count=$STALE_IN_DB)"; fi

WS1_NOT_IN_DB=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS1';")
if [ "$WS1_NOT_IN_DB" = "0" ]; then pass "'$WS1' NOT in DB (removed)"; else fail "'$WS1' NOT in DB (count=$WS1_NOT_IN_DB)"; fi

WS1_SWAY=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[].name' | grep -c "^${WS1}$")
if [ "$WS1_SWAY" = "1" ]; then pass "'$WS1' still in sway"; else fail "'$WS1' still in sway (count=$WS1_SWAY)"; fi

FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$ORIG_WS" ]; then pass "focused on original workspace '$ORIG_WS'"; else fail "focused on original workspace '$ORIG_WS' (got '$FOCUSED')"; fi

# --- 4. Test: repair ---
run "swayg repair" \
    "$SG repair 2>&1"

STALE_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS_STALE';")
if [ "$STALE_GONE" = "0" ]; then pass "'$WS_STALE' removed from DB (was not in sway)"; else fail "'$WS_STALE' removed from DB (count=$STALE_GONE)"; fi

WS1_RESTORED=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS1';")
if [ "$WS1_RESTORED" = "1" ]; then pass "'$WS1' re-added to DB (found in sway)"; else fail "'$WS1' re-added to DB (count=$WS1_RESTORED)"; fi

WS1_IN_GROUP0=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS1' AND g.name = '0';")
if [ "$WS1_IN_GROUP0" = "1" ]; then pass "'$WS1' added to default group '0'"; else fail "'$WS1' added to default group '0' (count=$WS1_IN_GROUP0)"; fi

GE_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_EMPTY';")
if [ "$GE_GONE" = "0" ]; then pass "'$GROUP_EMPTY' pruned (was effectively empty)"; else fail "'$GROUP_EMPTY' pruned (count=$GE_GONE)"; fi

G_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP';")
if [ "$G_GONE" = "0" ]; then pass "'$GROUP' pruned (WS1 was removed from DB, group effectively empty)"; else fail "'$GROUP' pruned (count=$G_GONE)"; fi

# --- 5. Test: workspace list shows repaired workspace ---
run "workspace list --visible --plain --output $OUTPUT" \
    "$SG workspace list --visible --plain --output $OUTPUT 2>/dev/null"

OUT=$($SG workspace list --visible --plain --output $OUTPUT 2>/dev/null)
echo "$OUT" | grep -q "$WS1"
if [ $? -eq 0 ]; then pass "'$WS1' visible after repair"; else fail "'$WS1' visible after repair"; fi

# --- 6. Cleanup ---
run "Kill kitty" \
    "kill \$(cat $PID_DIR/$WS1.pid) 2>/dev/null; sleep 0.5"

GONE=$(swaymsg -t get_tree 2>/dev/null | jq -r '.. | objects | select(.app_id? != null) | .app_id' | grep -c "^${WS1}$")
if [ "$GONE" = "0" ]; then pass "kitty '$WS1' is gone"; else fail "kitty '$WS1' is gone (count=$GONE)"; fi

# --- 7. Post-condition ---
run "Init to sync DB state" \
    '$SG init >/dev/null 2>&1'

run "Post-condition: no test data in DB" \
    'sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name IN ('"'"$GROUP"'"', '"'"$GROUP_EMPTY"'"');"; sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name IN ('"'"$WS1"'"', '"'"$WS_STALE"'"');"'

GROUP_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name IN ('$GROUP', '$GROUP_EMPTY');")
WS_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name IN ('$WS1', '$WS_STALE');")
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
