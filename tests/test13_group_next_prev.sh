#!/bin/bash
# Test: group next / group prev — switch between groups, wrap behavior, boundary.

SG="$HOME/.cargo/bin/swayg"
DB=~/.local/share/swayg/swayg.db
GROUP_A="__test_ga__"
GROUP_B="__test_gb__"
GROUP_C="__test_gc__"
WS_A="__tg_a__"
WS_B="__tg_b__"
WS_C="__tg_c__"
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

echo -e "\033[1m=== Test: group next / group prev ===\033[0m"

rm -rf "$PID_DIR"
mkdir -p "$PID_DIR"

# --- 1. Precondition checks (BEFORE init) ---

for G in $GROUP_A $GROUP_B $GROUP_C; do
    run "Precondition: $G does not exist" \
        'sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '"'$G'"';"'
    G_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$G';")
    if [ "$G_COUNT" = "0" ]; then pass "$G does not exist in DB"; else fail "$G must not exist in DB (count=$G_COUNT)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi
done

for WS in $WS_A $WS_B $WS_C; do
    run "Precondition: $WS does not exist in DB" \
        'sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '"'$WS'"';"'
    WS_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS';")
    if [ "$WS_COUNT" = "0" ]; then pass "$WS does not exist in DB"; else fail "$WS must not exist in DB (count=$WS_COUNT)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi
done

for WS in $WS_A $WS_B $WS_C; do
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
run "Setup: init + create 3 groups + launch 3 kitties + move containers" \
    "$SG init >/dev/null 2>&1
    $SG group select $OUTPUT '$GROUP_A' --create 2>&1
    kitty --class '$WS_A' >/dev/null 2>&1 & echo \$! > $PID_DIR/$WS_A.pid
    sleep 0.5
    $SG container move '$WS_A' --switch-to-workspace 2>&1
    $SG group select $OUTPUT '$GROUP_B' --create 2>&1
    kitty --class '$WS_B' >/dev/null 2>&1 & echo \$! > $PID_DIR/$WS_B.pid
    sleep 0.5
    $SG container move '$WS_B' --switch-to-workspace 2>&1
    $SG group select $OUTPUT '$GROUP_C' --create 2>&1
    kitty --class '$WS_C' >/dev/null 2>&1 & echo \$! > $PID_DIR/$WS_C.pid
    sleep 0.5
    $SG container move '$WS_C' --switch-to-workspace 2>&1
    $SG group select $OUTPUT '$GROUP_A' 2>&1
    sleep 0.1"

run "Verify setup" 'true'

for G in $GROUP_A $GROUP_B $GROUP_C; do
    G_EXISTS=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$G';")
    if [ "$G_EXISTS" = "1" ]; then pass "group '$G' exists"; else fail "group '$G' exists (count=$G_EXISTS)"; fi
done

for WS in $WS_A $WS_B $WS_C; do
    APPID_COUNT=$(swaymsg -t get_tree 2>/dev/null | jq -r '.. | objects | select(.app_id? != null) | .app_id' | grep -c "^${WS}$")
    if [ "$APPID_COUNT" -ge 1 ]; then pass "kitty '$WS' is running"; else fail "kitty '$WS' is running (count=$APPID_COUNT)"; fi
done

WS_A_IN_GA=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS_A' AND g.name = '$GROUP_A';")
if [ "$WS_A_IN_GA" = "1" ]; then pass "'$WS_A' in group '$GROUP_A'"; else fail "'$WS_A' in group '$GROUP_A' (count=$WS_A_IN_GA)"; fi

WS_B_IN_GB=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS_B' AND g.name = '$GROUP_B';")
if [ "$WS_B_IN_GB" = "1" ]; then pass "'$WS_B' in group '$GROUP_B'"; else fail "'$WS_B' in group '$GROUP_B' (count=$WS_B_IN_GB)"; fi

WS_C_IN_GC=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS_C' AND g.name = '$GROUP_C';")
if [ "$WS_C_IN_GC" = "1" ]; then pass "'$WS_C' in group '$GROUP_C'"; else fail "'$WS_C' in group '$GROUP_C' (count=$WS_C_IN_GC)"; fi

ACTIVE=$($SG group active $OUTPUT 2>/dev/null)
if [ "$ACTIVE" = "$GROUP_A" ]; then pass "active group = '$GROUP_A'"; else fail "active group = '$GROUP_A' (got '$ACTIVE')"; fi

# Note: group next/prev iterate ALL groups alphabetically.
# Order: "0", "__test_ga__", "__test_gb__", "__test_gc__"
# Indexes: 0=0, 1=A, 2=B, 3=C

# --- 4. Test: group next (A → B) ---
run "group next --output $OUTPUT" \
    "$SG group next --output $OUTPUT 2>&1"
ACTIVE=$($SG group active $OUTPUT 2>/dev/null)
if [ "$ACTIVE" = "$GROUP_B" ]; then pass "active group = '$GROUP_B'"; else fail "active group = '$GROUP_B' (got '$ACTIVE')"; fi

# --- 5. Test: group next (B → C) ---
run "group next --output $OUTPUT" \
    "$SG group next --output $OUTPUT 2>&1"
ACTIVE=$($SG group active $OUTPUT 2>/dev/null)
if [ "$ACTIVE" = "$GROUP_C" ]; then pass "active group = '$GROUP_C'"; else fail "active group = '$GROUP_C' (got '$ACTIVE')"; fi

# --- 6. Test: group next without wrap at boundary (C → stays) ---
run "group next --output $OUTPUT (no wrap, at boundary)" \
    "$SG group next --output $OUTPUT 2>&1"
ACTIVE=$($SG group active $OUTPUT 2>/dev/null)
if [ "$ACTIVE" = "$GROUP_C" ]; then pass "still '$GROUP_C' (no wrap, at boundary)"; else fail "still '$GROUP_C' (no wrap, at boundary) (got '$ACTIVE')"; fi

# --- 7. Test: group next with wrap (C → "0") ---
run "group next --output $OUTPUT --wrap" \
    "$SG group next --output $OUTPUT --wrap 2>&1"
ACTIVE=$($SG group active $OUTPUT 2>/dev/null)
if [ "$ACTIVE" = "0" ]; then pass "active group = '0' after wrap (past end to first)"; else fail "active group = '0' after wrap (got '$ACTIVE')"; fi

# --- 8. Test: group prev from "0" without wrap (boundary → stays) ---
run "group prev --output $OUTPUT (no wrap, '0' is first)" \
    "$SG group prev --output $OUTPUT 2>&1"
ACTIVE=$($SG group active $OUTPUT 2>/dev/null)
if [ "$ACTIVE" = "0" ]; then pass "still '0' (no wrap prev, at start)"; else fail "still '0' (no wrap prev, at start) (got '$ACTIVE')"; fi

# --- 9. Test: group prev with wrap ("0" → C, last alphabetically) ---
run "group prev --output $OUTPUT --wrap" \
    "$SG group prev --output $OUTPUT --wrap 2>&1"
ACTIVE=$($SG group active $OUTPUT 2>/dev/null)
if [ "$ACTIVE" = "$GROUP_C" ]; then pass "active group = '$GROUP_C' after wrap prev"; else fail "active group = '$GROUP_C' after wrap prev (got '$ACTIVE')"; fi

# --- 10. Test: group prev (C → B) ---
run "group prev --output $OUTPUT" \
    "$SG group prev --output $OUTPUT 2>&1"
ACTIVE=$($SG group active $OUTPUT 2>/dev/null)
if [ "$ACTIVE" = "$GROUP_B" ]; then pass "active group = '$GROUP_B' after prev"; else fail "active group = '$GROUP_B' after prev (got '$ACTIVE')"; fi

# --- 11. Test: group prev (B → A) ---
run "group prev --output $OUTPUT" \
    "$SG group prev --output $OUTPUT 2>&1"
ACTIVE=$($SG group active $OUTPUT 2>/dev/null)
if [ "$ACTIVE" = "$GROUP_A" ]; then pass "active group = '$GROUP_A' after prev"; else fail "active group = '$GROUP_A' after prev (got '$ACTIVE')"; fi

# --- 12. Test: group next-on-output --wrap (A → next non-empty group on output) ---
# Switch back to A first for clean state
run "Switch to $GROUP_A for next-on-output test" \
    "$SG group select $OUTPUT '$GROUP_A' 2>&1"
run "group next-on-output --wrap" \
    "$SG group next-on-output --wrap 2>&1"
ACTIVE=$($SG group active $OUTPUT 2>/dev/null)
if [ "$ACTIVE" = "$GROUP_B" ]; then pass "active group = '$GROUP_B' after next-on-output"; else fail "active group = '$GROUP_B' after next-on-output (got '$ACTIVE')"; fi

# --- 13. Test: group prev-on-output --wrap (B → A) ---
run "group prev-on-output --wrap" \
    "$SG group prev-on-output --wrap 2>&1"
ACTIVE=$($SG group active $OUTPUT 2>/dev/null)
if [ "$ACTIVE" = "$GROUP_A" ]; then pass "active group = '$GROUP_A' after prev-on-output"; else fail "active group = '$GROUP_A' after prev-on-output (got '$ACTIVE')"; fi

# --- 13. Cleanup ---
run "Kill kitties" \
    "kill \$(cat $PID_DIR/$WS_A.pid) 2>/dev/null; kill \$(cat $PID_DIR/$WS_B.pid) 2>/dev/null; kill \$(cat $PID_DIR/$WS_C.pid) 2>/dev/null; sleep 0.5"

for WS in $WS_A $WS_B $WS_C; do
    GONE=$(swaymsg -t get_tree 2>/dev/null | jq -r '.. | objects | select(.app_id? != null) | .app_id' | grep -c "^${WS}$")
    if [ "$GONE" = "0" ]; then pass "kitty '$WS' is gone"; else fail "kitty '$WS' is gone (count=$GONE)"; fi
done

run "Switch back to original group '$ORIG_GROUP'" \
    "$SG group select $OUTPUT '$ORIG_GROUP' 2>&1"
FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$ORIG_WS" ]; then pass "focused on original workspace '$ORIG_WS'"; else fail "focused on original workspace '$ORIG_WS' (got '$FOCUSED')"; fi

for G in $GROUP_A $GROUP_B $GROUP_C; do
    run "Auto-delete $G" \
        "$SG group select $OUTPUT '$G' 2>&1"
    run "Switch back" \
        "$SG group select $OUTPUT '$ORIG_GROUP' 2>&1"
done

G_ALL_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name IN ('$GROUP_A', '$GROUP_B', '$GROUP_C');")
if [ "$G_ALL_GONE" = "0" ]; then pass "all test groups auto-deleted"; else fail "all test groups auto-deleted (count=$G_ALL_GONE)"; fi

# --- 14. Post-condition ---
run "Init to sync DB state" \
    '$SG init >/dev/null 2>&1'

run "Post-condition: no test data in DB" \
    'sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name IN ('"'"$GROUP_A"'"', '"'"$GROUP_B"'"', '"'"$GROUP_C"'"');"; sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name IN ('"'"$WS_A"'"', '"'"$WS_B"'"', '"'"$WS_C"'"');"'

GROUP_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name IN ('$GROUP_A', '$GROUP_B', '$GROUP_C');")
WS_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name IN ('$WS_A', '$WS_B', '$WS_C');")
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
