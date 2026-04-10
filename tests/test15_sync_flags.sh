#!/bin/bash
# Test: sync flags — sync --workspaces, sync --groups, sync --outputs, sync (no flag).

SG="$HOME/.cargo/bin/swayg"
DB=~/.local/share/swayg/swayg.db
GROUP="__test_sync__"
GROUP2="__test_sync2__"
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

echo -e "\033[1m=== Test: sync flags ===\033[0m"

rm -rf "$PID_DIR"
mkdir -p "$PID_DIR"

# --- 1. Precondition checks (BEFORE init) ---

for G in $GROUP $GROUP2; do
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
    sqlite3 \$DB \"INSERT INTO workspaces (name, is_global, created_at, updated_at) VALUES ('$WS_STALE', 0, datetime('now'), datetime('now'));\"
    sqlite3 \$DB \"DELETE FROM workspace_groups WHERE workspace_id IN (SELECT id FROM workspaces WHERE name = '$WS1');\"
    sqlite3 \$DB \"DELETE FROM workspaces WHERE name = '$WS1';\"
    $SG group create '$GROUP2' 2>&1
    sleep 0.1"

run "Verify setup" 'true'

G_EXISTS=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP';")
if [ "$G_EXISTS" = "1" ]; then pass "group '$GROUP' exists"; else fail "group '$GROUP' exists (count=$G_EXISTS)"; fi

G2_EXISTS=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP2';")
if [ "$G2_EXISTS" = "1" ]; then pass "group '$GROUP2' exists"; else fail "group '$GROUP2' exists (count=$G2_EXISTS)"; fi

STALE_IN_DB=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS_STALE';")
if [ "$STALE_IN_DB" = "1" ]; then pass "'$WS_STALE' in DB (not in sway)"; else fail "'$WS_STALE' in DB (count=$STALE_IN_DB)"; fi

WS1_NOT_IN_DB=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS1';")
if [ "$WS1_NOT_IN_DB" = "0" ]; then pass "'$WS1' NOT in DB (removed)"; else fail "'$WS1' NOT in DB (count=$WS1_NOT_IN_DB)"; fi

WS1_SWAY=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[].name' | grep -c "^${WS1}$")
if [ "$WS1_SWAY" = "1" ]; then pass "'$WS1' still in sway"; else fail "'$WS1' still in sway (count=$WS1_SWAY)"; fi

FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$ORIG_WS" ]; then pass "focused on original workspace '$ORIG_WS'"; else fail "focused on original workspace '$ORIG_WS' (got '$FOCUSED')"; fi

# --- 4. Test: sync --workspaces ---
run "sync --workspaces" \
    "$SG sync --workspaces 2>&1"

SYNC_OUT=$($SG sync --workspaces 2>&1)
echo "$SYNC_OUT" | grep -q "workspaces"
if [ $? -eq 0 ]; then pass "output contains 'workspaces'"; else fail "output contains 'workspaces'"; fi

STALE_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS_STALE';")
if [ "$STALE_GONE" = "0" ]; then pass "'$WS_STALE' removed (was not in sway)"; else fail "'$WS_STALE' removed (count=$STALE_GONE)"; fi

WS1_RESTORED=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS1';")
if [ "$WS1_RESTORED" = "1" ]; then pass "'$WS1' re-added to DB (found in sway)"; else fail "'$WS1' re-added to DB (count=$WS1_RESTORED)"; fi

# --- 5. Test: sync --groups ---
run "sync --groups" \
    "$SG sync --groups 2>&1"

SYNC_GRP_OUT=$($SG sync --groups 2>&1)
echo "$SYNC_GRP_OUT" | grep -q "groups"
if [ $? -eq 0 ]; then pass "output contains 'groups'"; else fail "output contains 'groups'"; fi

# --- 6. Test: sync --outputs ---
run "sync --outputs" \
    "$SG sync --outputs 2>&1"

SYNC_OUT_OUT=$($SG sync --outputs 2>&1)
echo "$SYNC_OUT_OUT" | grep -q "outputs"
if [ $? -eq 0 ]; then pass "output contains 'outputs'"; else fail "output contains 'outputs'"; fi

# --- 7. Test: sync (no flag = all) ---
run "sync (no flag)" \
    "$SG sync 2>&1"

SYNC_ALL_OUT=$($SG sync 2>&1)
echo "$SYNC_ALL_OUT" | grep -q "workspaces"
if [ $? -eq 0 ]; then pass "output contains 'workspaces'"; else fail "output contains 'workspaces'"; fi
echo "$SYNC_ALL_OUT" | grep -q "groups"
if [ $? -eq 0 ]; then pass "output contains 'groups'"; else fail "output contains 'groups'"; fi
echo "$SYNC_ALL_OUT" | grep -q "outputs"
if [ $? -eq 0 ]; then pass "output contains 'outputs'"; else fail "output contains 'outputs'"; fi

# --- 8. Cleanup ---
run "Kill kitty" \
    "kill \$(cat $PID_DIR/$WS1.pid) 2>/dev/null; sleep 0.5"

GONE=$(swaymsg -t get_tree 2>/dev/null | jq -r '.. | objects | select(.app_id? != null) | .app_id' | grep -c "^${WS1}$")
if [ "$GONE" = "0" ]; then pass "kitty '$WS1' is gone"; else fail "kitty '$WS1' is gone (count=$GONE)"; fi

run "Auto-delete $GROUP" \
    "$SG group select $OUTPUT '$GROUP' 2>&1"
run "Switch back" \
    "$SG group select $OUTPUT '$ORIG_GROUP' 2>&1"
G_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP';")
if [ "$G_GONE" = "0" ]; then pass "'$GROUP' auto-deleted"; else fail "'$GROUP' auto-deleted (count=$G_GONE)"; fi

FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$ORIG_WS" ]; then pass "focused on original workspace '$ORIG_WS'"; else fail "focused on original workspace '$ORIG_WS' (got '$FOCUSED')"; fi

# --- 9. Post-condition ---
run "Init to sync DB state" \
    '$SG init >/dev/null 2>&1'

run "Post-condition: no test data in DB" \
    'sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name IN ('"'"$GROUP"'"', '"'"$GROUP2"'"');"; sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name IN ('"'"$WS1"'"', '"'"$WS_STALE"'"');"'

GROUP_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name IN ('$GROUP', '$GROUP2');")
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
