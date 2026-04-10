#!/bin/bash
# Test: status — show DB state, active groups, visible/hidden/global workspaces.

SG="$HOME/.cargo/bin/swayg"
DB=~/.local/share/swayg/swayg.db
OUTPUT="eDP-1"

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

echo -e "\033[1m=== Test: status ===\033[0m"

# --- 1. Setup ---
run "init" \
    "$SG init >/dev/null 2>&1"

# --- 2. Test: status (clean state) ---
run "status" \
    "$SG status 2>&1"

STATUS_OUT=$($SG status 2>/dev/null)
echo "  Status output:"
echo "$STATUS_OUT" | sed 's/^/    /'

echo "$STATUS_OUT" | grep -q "eDP-1"
if [ $? -eq 0 ]; then pass "output contains output name 'eDP-1'"; else fail "output contains output name 'eDP-1'"; fi

echo "$STATUS_OUT" | grep -q "active group"
if [ $? -eq 0 ]; then pass "output contains 'active group'"; else fail "output contains 'active group'"; fi

echo "$STATUS_OUT" | grep -q "Visible:"
if [ $? -eq 0 ]; then pass "output contains 'Visible:'"; else fail "output contains 'Visible:'"; fi

echo "$STATUS_OUT" | grep -q "Hidden:"
if [ $? -eq 0 ]; then pass "output contains 'Hidden:'"; else fail "output contains 'Hidden:'"; fi

# --- 3. Test: status with kitty in test group (non-default active group) ---
GROUP="__test_status__"
WS1="__tg_ws1__"
PID_DIR="/tmp/sway-group-tests"

rm -rf "$PID_DIR"
mkdir -p "$PID_DIR"

run "Setup: create group + kitty" \
    "$SG group select $OUTPUT '$GROUP' --create 2>&1
    kitty --class '$WS1' >/dev/null 2>&1 & echo \$! > $PID_DIR/$WS1.pid
    sleep 0.5
    $SG container move '$WS1' --switch-to-workspace 2>&1
    $SG group select $OUTPUT '0' 2>&1
    sleep 0.1"

run "status (with test group + hidden workspace)" \
    "$SG status 2>&1"

STATUS_OUT2=$($SG status 2>/dev/null)
echo "  Status output (with test group):"
echo "$STATUS_OUT2" | sed 's/^/    /'

echo "$STATUS_OUT2" | grep -q "active group = \"0\""
if [ $? -eq 0 ]; then pass "active group = '0' (not test group)"; else fail "active group = '0' (not test group)"; fi

echo "$STATUS_OUT2" | grep -q "active group = \"0\""
if [ $? -eq 0 ]; then pass "active group = '0'"; else fail "active group = '0'"; fi

echo "$STATUS_OUT2" | grep -q "__tg_ws1__"
if [ $? -eq 0 ]; then pass "output mentions '$WS1' (hidden workspace)"; else fail "output mentions '$WS1' (hidden workspace)"; fi

# --- 4. Test: status with global workspace ---
run "Set $WS1 global" \
    "$SG workspace global '$WS1' 2>&1"

run "status (with global workspace)" \
    "$SG status 2>&1"

STATUS_GLOBAL=$($SG status 2>/dev/null)
echo "  Status output (global):"
echo "$STATUS_GLOBAL" | sed 's/^/    /'

echo "$STATUS_GLOBAL" | grep -q "Global:"
if [ $? -eq 0 ]; then pass "output contains 'Global:' section"; else fail "output contains 'Global:' section"; fi

echo "$STATUS_GLOBAL" | grep -q "$WS1"
if [ $? -eq 0 ]; then pass "'$WS1' listed in Global section"; else fail "'$WS1' listed in Global section"; fi

# --- 5. Cleanup ---
run "unglobal $WS1" \
    "$SG workspace unglobal '$WS1' 2>&1"

run "Kill kitty" \
    "kill \$(cat $PID_DIR/$WS1.pid) 2>/dev/null; sleep 0.5"

GONE=$(swaymsg -t get_tree 2>/dev/null | jq -r '.. | objects | select(.app_id? != null) | .app_id' | grep -c "^${WS1}$")
if [ "$GONE" = "0" ]; then pass "kitty '$WS1' is gone"; else fail "kitty '$WS1' is gone (count=$GONE)"; fi

run "Auto-delete $GROUP" \
    "$SG group select $OUTPUT '$GROUP' 2>&1"
run "Switch back" \
    "$SG group select $OUTPUT '0' 2>&1"

# --- 6. Post-condition ---
run "Init to clean DB" \
    '$SG init >/dev/null 2>&1'

run "Post-condition: no test data" \
    'sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '"'$GROUP'"';"; sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '"'"$WS1"'"';"'

GROUP_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP';")
WS_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS1';")
if [ "$GROUP_GONE" = "0" ] && [ "$WS_GONE" = "0" ]; then
    pass "no test data remains"
else
    fail "no test data remains (group=$GROUP_GONE, ws=$WS_GONE)"
fi

echo ""
echo -e "\033[1m=== Summary ===\033[0m"
echo "  $PASS passed, $FAIL failed"
if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
