#!/bin/bash
# Test: group rename — basic rename, output reference update, group_state update, error cases.

SG="$HOME/.cargo/bin/swayg"
DB=~/.local/share/swayg/swayg.db
GROUP_A="__test_group_a__"
GROUP_B="__test_group_b__"
GROUP_RENAMED="__test_group_a_renamed__"
WS1="__tg_ws1__"
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

trap '[ "$FAIL" -gt 0 ] && swaymsg workspace "$ORIG_WS" >/dev/null 2>&1' EXIT

echo -e "\033[1m=== Test: group rename ===\033[0m"

# --- 1. Precondition checks (BEFORE init) ---

run "Precondition: $GROUP_A does not exist" \
    'sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '"'$GROUP_A'"';"'
GA_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_A';")
if [ "$GA_COUNT" = "0" ]; then pass "$GROUP_A does not exist in DB"; else fail "$GROUP_A must not exist in DB (count=$GA_COUNT)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi

run "Precondition: $GROUP_B does not exist" \
    'sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '"'$GROUP_B'"';"'
GB_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_B';")
if [ "$GB_COUNT" = "0" ]; then pass "$GROUP_B does not exist in DB"; else fail "$GROUP_B must not exist in DB (count=$GB_COUNT)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi

run "Precondition: $GROUP_RENAMED does not exist" \
    'sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '"'$GROUP_RENAMED'"';"'
GR_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_RENAMED';")
if [ "$GR_COUNT" = "0" ]; then pass "$GROUP_RENAMED does not exist in DB"; else fail "$GROUP_RENAMED must not exist in DB (count=$GR_COUNT)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi

run "Precondition: $WS1 does not exist in DB" \
    'sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '"'$WS1'"';"'
WS1_COUNT=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS1';")
if [ "$WS1_COUNT" = "0" ]; then pass "$WS1 does not exist in DB"; else fail "$WS1 must not exist in DB (count=$WS1_COUNT)"; echo "Results: $PASS passed, $FAIL failed (ABORTED)"; exit 1; fi

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
run "Setup: init + create test groups + workspace + memberships + group_state" \
    "$SG init >/dev/null 2>&1
    sqlite3 \$DB \"INSERT INTO groups (name, created_at, updated_at) VALUES ('$GROUP_A', datetime('now'), datetime('now'));\"
    sqlite3 \$DB \"INSERT INTO groups (name, created_at, updated_at) VALUES ('$GROUP_B', datetime('now'), datetime('now'));\"
    sqlite3 \$DB \"UPDATE outputs SET active_group = '$GROUP_A' WHERE name = '$OUTPUT';\"
    sqlite3 \$DB \"INSERT INTO workspaces (name, is_global, created_at, updated_at) VALUES ('$WS1', 0, datetime('now'), datetime('now'));\"
    sqlite3 \$DB \"INSERT INTO workspace_groups (workspace_id, group_id, created_at) SELECT w.id, g.id, datetime('now') FROM workspaces w, groups g WHERE w.name = '$WS1' AND g.name = '$GROUP_A';\"
    sqlite3 \$DB \"INSERT INTO group_state (output, group_name, last_focused_workspace, last_visited) VALUES ('$OUTPUT', '$GROUP_A', '$WS1', datetime('now'));\""

run "Verify setup" 'true'

GA_EXISTS=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_A';")
if [ "$GA_EXISTS" = "1" ]; then pass "group '$GROUP_A' exists"; else fail "group '$GROUP_A' exists (count=$GA_EXISTS)"; fi

GB_EXISTS=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_B';")
if [ "$GB_EXISTS" = "1" ]; then pass "group '$GROUP_B' exists"; else fail "group '$GROUP_B' exists (count=$GB_EXISTS)"; fi

WS1_IN_DB=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS1';")
if [ "$WS1_IN_DB" = "1" ]; then pass "'$WS1' in DB"; else fail "'$WS1' in DB (count=$WS1_IN_DB)"; fi

WS1_IN_GA=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS1' AND g.name = '$GROUP_A';")
if [ "$WS1_IN_GA" = "1" ]; then pass "'$WS1' in group '$GROUP_A'"; else fail "'$WS1' in group '$GROUP_A' (count=$WS1_IN_GA)"; fi

ACTIVE=$(sqlite3 "$DB" "SELECT active_group FROM outputs WHERE name = '$OUTPUT';")
if [ "$ACTIVE" = "$GROUP_A" ]; then pass "output active_group = '$GROUP_A'"; else fail "output active_group = '$GROUP_A' (got '$ACTIVE')"; fi

GS_EXISTS=$(sqlite3 "$DB" "SELECT count(*) FROM group_state WHERE output = '$OUTPUT' AND group_name = '$GROUP_A';")
if [ "$GS_EXISTS" = "1" ]; then pass "group_state entry for '$GROUP_A' exists"; else fail "group_state entry for '$GROUP_A' exists (count=$GS_EXISTS)"; fi

# --- 4. Test: rename A → A_renamed (success) ---
run "Rename $GROUP_A to $GROUP_RENAMED" \
    "$SG group rename '$GROUP_A' '$GROUP_RENAMED' 2>&1"

RENAME_EXIT=$?
if [ "$RENAME_EXIT" -eq 0 ]; then pass "rename command succeeded (exit 0)"; else fail "rename command succeeded (exit=$RENAME_EXIT)"; fi

GA_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_A';")
if [ "$GA_GONE" = "0" ]; then pass "'$GROUP_A' gone from DB"; else fail "'$GROUP_A' gone from DB (count=$GA_GONE)"; fi

GR_EXISTS=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_RENAMED';")
if [ "$GR_EXISTS" = "1" ]; then pass "'$GROUP_RENAMED' exists in DB"; else fail "'$GROUP_RENAMED' exists in DB (count=$GR_EXISTS)"; fi

WS1_IN_GR=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS1' AND g.name = '$GROUP_RENAMED';")
if [ "$WS1_IN_GR" = "1" ]; then pass "'$WS1' membership updated to '$GROUP_RENAMED'"; else fail "'$WS1' membership updated to '$GROUP_RENAMED' (count=$WS1_IN_GR)"; fi

WS1_NOT_IN_GA=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id JOIN workspaces w ON w.id = wg.workspace_id WHERE w.name = '$WS1' AND g.name = '$GROUP_A';")
if [ "$WS1_NOT_IN_GA" = "0" ]; then pass "'$WS1' NOT in old group name '$GROUP_A'"; else fail "'$WS1' NOT in old group name '$GROUP_A' (count=$WS1_NOT_IN_GA)"; fi

ACTIVE_AFTER=$(sqlite3 "$DB" "SELECT active_group FROM outputs WHERE name = '$OUTPUT';")
if [ "$ACTIVE_AFTER" = "$GROUP_RENAMED" ]; then pass "output active_group updated to '$GROUP_RENAMED'"; else fail "output active_group updated to '$GROUP_RENAMED' (got '$ACTIVE_AFTER')"; fi

GS_UPDATED=$(sqlite3 "$DB" "SELECT count(*) FROM group_state WHERE output = '$OUTPUT' AND group_name = '$GROUP_RENAMED';")
if [ "$GS_UPDATED" = "1" ]; then pass "group_state updated to '$GROUP_RENAMED'"; else fail "group_state updated to '$GROUP_RENAMED' (count=$GS_UPDATED)"; fi

GS_OLD=$(sqlite3 "$DB" "SELECT count(*) FROM group_state WHERE output = '$OUTPUT' AND group_name = '$GROUP_A';")
if [ "$GS_OLD" = "0" ]; then pass "group_state old entry for '$GROUP_A' gone"; else fail "group_state old entry for '$GROUP_A' gone (count=$GS_OLD)"; fi

OUT=$($SG workspace list --plain --group "$GROUP_RENAMED" 2>/dev/null)
echo "$OUT" | grep -q "$WS1"
if [ $? -eq 0 ]; then pass "'$WS1' listed in renamed group via workspace list"; else fail "'$WS1' listed in renamed group via workspace list"; fi

# --- 5. Test: rename to existing name (error) ---
run "Rename $GROUP_RENAMED to $GROUP_B (should fail — target exists)" \
    "$SG group rename '$GROUP_RENAMED' '$GROUP_B' 2>&1; true"

GR_STILL=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_RENAMED';")
if [ "$GR_STILL" = "1" ]; then pass "'$GROUP_RENAMED' NOT renamed (target exists)"; else fail "'$GROUP_RENAMED' NOT renamed (target exists) (count=$GR_STILL)"; fi

GB_UNCHANGED=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_B';")
if [ "$GB_UNCHANGED" = "1" ]; then pass "'$GROUP_B' unchanged"; else fail "'$GROUP_B' unchanged (count=$GB_UNCHANGED)"; fi

# --- 6. Test: rename nonexistent group (error) ---
run "Rename nonexistent group (should fail — source not found)" \
    "$SG group rename 'nonexistent__test__' '$GROUP_RENAMED' 2>&1; true"

GR_UNCHANGED=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '$GROUP_RENAMED';")
if [ "$GR_UNCHANGED" = "1" ]; then pass "'$GROUP_RENAMED' unchanged (nonexistent source)"; else fail "'$GROUP_RENAMED' unchanged (nonexistent source) (count=$GR_UNCHANGED)"; fi

NO_CREATE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = 'nonexistent__test__';")
if [ "$NO_CREATE" = "0" ]; then pass "no group created for nonexistent source"; else fail "no group created for nonexistent source (count=$NO_CREATE)"; fi

# --- 7. Test: rename group "0" (error) ---
run "Rename group '0' (should fail — cannot rename default)" \
    "$SG group rename '0' 'should_not_work__' 2>&1; true"

GROUP_0=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = '0';")
if [ "$GROUP_0" = "1" ]; then pass "group '0' NOT renamed"; else fail "group '0' NOT renamed (count=$GROUP_0)"; fi

NO_TARGET=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name = 'should_not_work__';")
if [ "$NO_TARGET" = "0" ]; then pass "'should_not_work__' NOT created"; else fail "'should_not_work__' NOT created (count=$NO_TARGET)"; fi

# --- 8. Cleanup ---
run "Switch back to original workspace" \
    "swaymsg workspace \"$ORIG_WS\" >/dev/null 2>&1"

FOCUSED=$(swaymsg -t get_workspaces 2>/dev/null | jq -r '.[] | select(.focused) | .name')
if [ "$FOCUSED" = "$ORIG_WS" ]; then pass "focused on original workspace '$ORIG_WS'"; else fail "focused on original workspace '$ORIG_WS' (got '$FOCUSED')"; fi

# --- 9. Post-condition ---
run "Post-condition: init to reset DB" \
    '$SG init >/dev/null 2>&1'

run "Post-condition: no test data in DB" \
    'sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name IN ('"'"$GROUP_A"'"', '"'"$GROUP_B"'"', '"'"$GROUP_RENAMED"'"', '"'"'should_not_work__'"'"');"; sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '"'"$WS1"'"';"; sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id WHERE g.name IN ('"'"$GROUP_A"'"', '"'"$GROUP_B"'"', '"'"$GROUP_RENAMED"'"');"'

GROUP_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM groups WHERE name IN ('$GROUP_A', '$GROUP_B', '$GROUP_RENAMED', 'should_not_work__');")
WS_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM workspaces WHERE name = '$WS1';")
WSGRP_GONE=$(sqlite3 "$DB" "SELECT count(*) FROM workspace_groups wg JOIN groups g ON g.id = wg.group_id WHERE g.name IN ('$GROUP_A', '$GROUP_B', '$GROUP_RENAMED');")
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
