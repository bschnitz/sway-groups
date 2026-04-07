#!/usr/bin/env bash
# Integration tests for swayg
#
# Self-contained: uses existing sway workspaces, creates its own groups,
# cleans up after itself. Run from project root:
#
#   ./tests/integration_test.sh
#
# Prerequisites: sway running, swayg installed, jq, python3

set -uo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
NC='\033[0m'

PASS=0
FAIL=0
SKIP=0

pass() { echo -e "  ${GREEN}PASS${NC} $1"; PASS=$((PASS + 1)); }
fail() { echo -e "  ${RED}FAIL${NC} $1"; echo "       $2"; FAIL=$((FAIL + 1)); }
skip() { echo -e "  ${YELLOW}SKIP${NC} $1"; SKIP=$((SKIP + 1)); }

# Run swayg, strip info log lines from stdout
sg() { command swayg "$@" 2>&1 | sed '/^\x1b\[[0-9;]*m.*INFO/d'; }

# Get the base name of the currently focused workspace (strips suffixes)
focused_ws() {
    swaymsg -t get_workspaces 2>/dev/null | python3 -c "
import json, sys
for w in json.load(sys.stdin):
    if w['focused']:
        n = w['name']
        for s in ('_class_hidden', '_class_global'):
            n = n.removesuffix(s)
        print(n)
        break
"
}

# Get the output of the currently focused workspace
focused_output() {
    swaymsg -t get_workspaces 2>/dev/null | python3 -c "
import json, sys
for w in json.load(sys.stdin):
    if w['focused']:
        print(w['output'])
        break
"
}

# ============ Pre-flight ============
echo -e "${BOLD}=== swayg Integration Tests ===${NC}"
echo ""

if ! command -v swaymsg &>/dev/null; then
    echo "SKIP: sway not running (swaymsg not found)"
    exit 0
fi
if ! command -v swayg &>/dev/null; then
    echo "SKIP: swayg not installed"
    exit 0
fi

ORIG_WS=$(focused_ws)
ORIG_OUT=$(focused_output)
if [ -z "$ORIG_OUT" ]; then
    echo "SKIP: Cannot determine focused output"
    exit 0
fi

# Clean DB, sync fresh
rm -f ~/.local/share/swayg/swayg.db
sg sync --all >/dev/null

# Collect available workspace names (sorted)
WS_ALL=($(sg group list 2>/dev/null | awk '/^Group/{g=1; next} /^  - /{print $2}'))
WS_A="${WS_ALL[0]:-}"
WS_B="${WS_ALL[1]:-}"
WS_C="${WS_ALL[2]:-}"
WS_D="${WS_ALL[3]:-}"

if [ -z "$WS_A" ] || [ -z "$WS_B" ]; then
    echo "SKIP: Need at least 2 workspaces (found ${#WS_ALL[@]})"
    exit 0
fi

# Test group names (prefixed to avoid collisions)
T_GRP=("T_empty" "T_one" "T_two" "T_nav" "T_a" "T_b" "T_c" "T_dup" "T_prune_me" "T_keep_me" "T_move_x" "T_move_y" "T_renamed")

cleanup() {
    echo ""
    echo "Cleanup..."
    sg group select "$ORIG_OUT" 0 >/dev/null
    for g in "${T_GRP[@]}"; do
        sg group delete "$g" --force >/dev/null 2>/dev/null || true
    done
    sg workspace unglobal "$WS_A" >/dev/null 2>/dev/null || true
    sg workspace unglobal "$WS_B" >/dev/null 2>/dev/null || true
    swaymsg workspace "$ORIG_WS" >/dev/null 2>/dev/null || true
    sleep 0.5
    echo ""
    local total=$((PASS + FAIL + SKIP))
    echo -e "${BOLD}Results: ${GREEN}$PASS passed${NC}, ${RED}$FAIL failed${NC}, ${YELLOW}$SKIP skipped${NC} ($total total)"
    [ $FAIL -eq 0 ]
}

trap cleanup EXIT

echo "Workspaces: ${WS_ALL[*]}"
echo "Output:    $ORIG_OUT (focused: $ORIG_WS)"
echo "Test groups: ${T_GRP[*]}"
echo ""

# ============ 1. Group CRUD ============
echo -e "${BOLD}--- 1. Group CRUD ---${NC}"

OUT=$(sg group create T_one)
echo "$OUT" | grep -q 'Created group "T_one"' && pass "create group" || fail "create group" "$OUT"

OUT=$(sg group list)
echo "$OUT" | grep -q 'T_one' && pass "list shows created group" || fail "list shows created group"

OUT=$(sg group create T_one 2>&1)
echo "$OUT" | grep -qi 'already exists' && pass "reject duplicate group" || fail "reject duplicate group" "$OUT"

OUT=$(sg group rename T_one T_renamed 2>&1)
echo "$OUT" | grep -q 'Renamed group "T_one" to "T_renamed"' && pass "rename group" || fail "rename group" "$OUT"

OUT=$(sg group list)
echo "$OUT" | grep -q 'T_renamed' && pass "list shows renamed group" || fail "list shows renamed group" "$OUT"

OUT=$(sg group delete 0 2>&1)
echo "$OUT" | grep -qi "cannot delete" && pass "reject deleting default group" || fail "reject deleting default group" "$OUT"

OUT=$(sg group rename 0 badname 2>&1)
echo "$OUT" | grep -qi "cannot rename" && pass "reject renaming default group" || fail "reject renaming default group" "$OUT"

sg group create T_dup >/dev/null
sg workspace add "$WS_A" -g T_dup >/dev/null
OUT=$(sg group delete T_dup 2>&1)
echo "$OUT" | grep -q 'workspaces' && pass "reject delete non-empty group" || fail "reject delete non-empty group" "$OUT"

OUT=$(sg group delete T_dup --force)
echo "$OUT" | grep -q 'Deleted group "T_dup"' && pass "force delete non-empty group" || fail "force delete non-empty group" "$OUT"

OUT=$(sg group delete T_renamed)
echo "$OUT" | grep -q 'Deleted group "T_renamed"' && pass "delete empty group" || fail "delete empty group" "$OUT"

echo ""

# ============ 2. Workspace Management ============
echo -e "${BOLD}--- 2. Workspace Management ---${NC}"

sg group create T_one >/dev/null

OUT=$(sg workspace add "$WS_A" -g T_one)
echo "$OUT" | grep -q "Added workspace \"$WS_A\" to group \"T_one\"" && pass "add workspace to group" || fail "add workspace to group" "$OUT"

OUT=$(sg workspace groups "$WS_A")
echo "$OUT" | grep -q '"T_one"' && pass "groups shows membership" || fail "groups shows membership" "$OUT"

OUT=$(sg workspace add "$WS_A" -g T_one 2>&1)
echo "$OUT" | grep -qi 'already in group' && pass "reject duplicate membership" || fail "reject duplicate membership" "$OUT"

OUT=$(sg workspace remove "$WS_A" -g T_one)
echo "$OUT" | grep -q "Removed workspace \"$WS_A\" from group \"T_one\"" && pass "remove workspace from group" || fail "remove workspace from group" "$OUT"

OUT=$(sg workspace remove "$WS_A" -g T_one 2>&1)
echo "$OUT" | grep -qi 'not in group' && pass "reject remove from wrong group" || fail "reject remove from wrong group" "$OUT"

OUT=$(sg workspace add __nonexistent__ 2>&1)
echo "$OUT" | grep -qi 'not found' && pass "reject adding non-existent workspace" || fail "reject adding non-existent workspace" "$OUT"

OUT=$(sg workspace global "$WS_A")
echo "$OUT" | grep -q "global" && pass "mark workspace global" || fail "mark workspace global" "$OUT"

OUT=$(sg status)
echo "$OUT" | grep -q "Global" && echo "$OUT" | grep -q "$WS_A" && pass "status shows global workspace" || fail "status shows global workspace" "$OUT"

OUT=$(sg workspace unglobal "$WS_A")
echo "$OUT" | grep -q "global" && pass "unglobal workspace" || fail "unglobal workspace" "$OUT"

sg group delete T_one --force >/dev/null

echo ""

# ============ 3. Group Switch: Fall 2 (First Visit) ============
echo -e "${BOLD}--- 3. Group Switch: Fall 2 (First Visit) ---${NC}"

sg group create T_a >/dev/null
sg group create T_b >/dev/null
sg workspace add "$WS_B" -g T_a >/dev/null
sg workspace add "$WS_A" -g T_a >/dev/null
sg workspace add "$WS_C" -g T_b >/dev/null

sg group select "$ORIG_OUT" 0 >/dev/null
sleep 0.3

# First visit to T_a: should focus alphabetically first workspace in group
# WS_A < WS_B alphabetically (both in T_a), so expect WS_A
sg group select "$ORIG_OUT" T_a >/dev/null
sleep 0.5
FW=$(focused_ws)
[ "$FW" = "$WS_A" ] && pass "first visit focuses first alphabetical (T_a -> $WS_A)" || fail "first visit focuses first alphabetical (T_a)" "expected $WS_A, got $FW"

echo ""

# ============ 4. Group Switch: Fall 3 (Revisit) ============
echo -e "${BOLD}--- 4. Group Switch: Fall 3 (Revisit) ---${NC}"

# Navigate to second workspace (WS_B) within T_a
sg nav go "$WS_B" >/dev/null
sleep 0.3

# Switch to T_b
sg group select "$ORIG_OUT" T_b >/dev/null
sleep 0.5
[ "$(focused_ws)" = "$WS_C" ] && pass "first visit focuses first in T_b" || fail "first visit focuses first in T_b" "expected $WS_C, got $(focused_ws)"

# Switch back to T_a -> should restore WS_B
sg group select "$ORIG_OUT" T_a >/dev/null
sleep 0.5
FW=$(focused_ws)
[ "$FW" = "$WS_B" ] && pass "revisit restores last focused (T_a -> $WS_B)" || fail "revisit restores last focused (T_a)" "expected $WS_B, got $FW"

echo ""

# ============ 5. Group Switch: Fall 1 (Empty Group) ============
echo -e "${BOLD}--- 5. Group Switch: Fall 1 (Empty Group) ---${NC}"

sg group create T_empty >/dev/null
sg group select "$ORIG_OUT" T_empty >/dev/null
sleep 0.5
FW=$(focused_ws)
[ "$FW" = "0" ] && pass "empty group focuses workspace 0" || fail "empty group focuses workspace 0" "expected 0, got $FW"

echo ""

# ============ 6. Group next/prev (all groups) ============
echo -e "${BOLD}--- 6. Group next/prev (all groups) ---${NC}"

# Alphabetical order: 0 < T_a < T_b < T_empty
# Start from 0 (first)
sg group select "$ORIG_OUT" 0 >/dev/null

OUT=$(sg group next -o "$ORIG_OUT")
echo "$OUT" | grep -q 'T_a' && pass "group next from 0 -> T_a" || fail "group next from 0" "$OUT"

OUT=$(sg group next -o "$ORIG_OUT")
echo "$OUT" | grep -q 'T_b' && pass "group next from T_a -> T_b" || fail "group next from T_a" "$OUT"

# prev from T_a -> 0
sg group select "$ORIG_OUT" T_a >/dev/null
OUT=$(sg group prev -o "$ORIG_OUT")
echo "$OUT" | grep -q '0' && pass "group prev from T_a -> 0" || fail "group prev from T_a" "$OUT"

# no-wrap: at end (T_empty), next should not switch
sg group select "$ORIG_OUT" T_empty >/dev/null
OUT=$(sg group next -o "$ORIG_OUT" 2>&1)
[ -z "$OUT" ] && pass "group next at end without wrap does nothing" || fail "group next at end" "$OUT"

# no-wrap: at start (0), prev should not switch
sg group select "$ORIG_OUT" 0 >/dev/null
OUT=$(sg group prev -o "$ORIG_OUT" 2>&1)
[ -z "$OUT" ] && pass "group prev at start without wrap does nothing" || fail "group prev at start" "$OUT"

# wrap: from T_empty (last), next wraps to 0
sg group select "$ORIG_OUT" T_empty >/dev/null
OUT=$(sg group next -o "$ORIG_OUT" -w 2>&1)
echo "$OUT" | grep -q '0' && pass "group next with wrap cycles to first" || fail "group next with wrap" "$OUT"

# wrap: from 0 (first), prev wraps to T_empty
OUT=$(sg group prev -o "$ORIG_OUT" -w 2>&1)
echo "$OUT" | grep -q 'T_empty' && pass "group prev with wrap cycles to last" || fail "group prev with wrap" "$OUT"

echo ""

# ============ 7. Group next-on-output / prev-on-output ============
echo -e "${BOLD}--- 7. Group next-on-output / prev-on-output ---${NC}"

# Non-empty groups on output: 0, T_a, T_b. T_empty is empty.
# From 0 (first non-empty), next-on-output -> T_a
sg group select "$ORIG_OUT" 0 >/dev/null

OUT=$(sg group next-on-output -o "$ORIG_OUT")
echo "$OUT" | grep -q 'T_a' && pass "next-on-output from 0 -> T_a" || fail "next-on-output from 0" "$OUT"

OUT=$(sg group next-on-output -o "$ORIG_OUT")
echo "$OUT" | grep -q 'T_b' && pass "next-on-output from T_a -> T_b" || fail "next-on-output from T_a" "$OUT"

# From T_b, next-on-output without wrap -> nothing (T_empty skipped, at end)
OUT=$(sg group next-on-output -o "$ORIG_OUT" 2>&1)
[ -z "$OUT" ] && pass "next-on-output at end without wrap stops" || fail "next-on-output at end" "$OUT"

# From T_b, next-on-output with wrap -> 0 (skip T_empty)
OUT=$(sg group next-on-output -o "$ORIG_OUT" -w)
echo "$OUT" | grep -q '0' && pass "next-on-output with wrap skips empty, goes to 0" || fail "next-on-output with wrap" "$OUT"

# prev-on-output from 0 without wrap: 0 is first, does nothing
OUT=$(sg group prev-on-output -o "$ORIG_OUT" 2>&1)
[ -z "$OUT" ] && pass "prev-on-output at start without wrap stops" || fail "prev-on-output at start" "$OUT"

# prev-on-output from 0 with wrap -> T_b (last non-empty alphabetically)
OUT=$(sg group prev-on-output -o "$ORIG_OUT" -w)
echo "$OUT" | grep -q 'T_b' && pass "prev-on-output with wrap goes to T_b" || fail "prev-on-output with wrap" "$OUT"

echo ""

# ============ 8. Workspace Navigation ============
echo -e "${BOLD}--- 8. Workspace Navigation ---${NC}"

# Reset to T_a, first visit restores WS_B (revisit from test 4)
sg group select "$ORIG_OUT" T_a >/dev/null
sleep 0.5

# Navigate to WS_A first, so nav next can go to WS_B
OUT=$(sg nav go "$WS_A" 2>&1)
echo "$OUT" | grep -q "Navigated to \"$WS_A\"" && pass "nav go resets position in T_a" || fail "nav go reset" "$OUT"

OUT=$(sg nav next -o "$ORIG_OUT" 2>&1)
echo "$OUT" | grep -q "Navigated to \"$WS_B\"" && pass "nav next within group (T_a: $WS_A -> $WS_B)" || fail "nav next" "$OUT"

OUT=$(sg nav prev -o "$ORIG_OUT" 2>&1)
echo "$OUT" | grep -q "Navigated to \"$WS_A\"" && pass "nav prev within group (T_a: $WS_B -> $WS_A)" || fail "nav prev" "$OUT"

OUT=$(sg nav back 2>&1)
echo "$OUT" | grep -q "Navigated back to \"$WS_B\"" && pass "nav back ($WS_A -> $WS_B)" || fail "nav back" "$OUT"

echo ""

# ============ 9. Sync ============
echo -e "${BOLD}--- 9. Sync ---${NC}"

rm -f ~/.local/share/swayg/swayg.db
OUT=$(sg sync --all 2>&1)
echo "$OUT" | grep -q 'Synced' && pass "sync from clean state" || fail "sync from clean state" "$OUT"

OUT=$(sg group list)
echo "$OUT" | grep -q 'Group "0"' && pass "sync creates default group" || fail "sync creates default group" "$OUT"

echo ""

# ============ 10. Status ============
echo -e "${BOLD}--- 10. Status ---${NC}"

OUT=$(sg status 2>&1)
echo "$OUT" | grep -q "$ORIG_OUT" && pass "status shows output name" || fail "status shows output" "$OUT"
echo "$OUT" | grep -q 'active group' && pass "status shows active group" || fail "status shows active group" "$OUT"
echo "$OUT" | grep -q 'Visible:' && pass "status shows visible workspaces" || fail "status shows visible" "$OUT"
echo "$OUT" | grep -q 'Hidden:' && pass "status shows hidden workspaces" || fail "status shows hidden" "$OUT"

echo ""

# ============ 11. Prune ============
echo -e "${BOLD}--- 11. Prune ---${NC}"

sg group create T_prune_me >/dev/null
OUT=$(sg group prune 2>&1)
echo "$OUT" | grep -q 'Pruned' && pass "prune removes empty groups" || fail "prune" "$OUT"

# T_prune_me should be gone
OUT=$(sg group list 2>&1)
echo "$OUT" | grep -q 'T_prune_me' && fail "prune did not remove T_prune_me" "" || pass "pruned group no longer listed"

# --keep should preserve
sg group create T_keep_me >/dev/null
OUT=$(sg group prune --keep T_keep_me 2>&1)
OUT=$(sg group list 2>&1)
echo "$OUT" | grep -q 'T_keep_me' && pass "prune --keep preserves group" || fail "prune --keep" "$OUT"
sg group delete T_keep_me --force >/dev/null

echo ""

# ============ 12. Workspace List ============
echo -e "${BOLD}--- 12. Workspace List ---${NC}"

OUT=$(sg workspace list -o "$ORIG_OUT" 2>&1)
echo "$OUT" | grep -q "$ORIG_OUT" && pass "workspace list shows output filter" || fail "workspace list output" "$OUT"

OUT=$(sg workspace list -g 0 2>&1)
echo "$OUT" | grep -q '"0"' && pass "workspace list shows group filter" || fail "workspace list group" "$OUT"

echo ""

# ============ 13. Group Active / List by Output ============
echo -e "${BOLD}--- 13. Group Active / List by Output ---${NC}"

OUT=$(sg group active "$ORIG_OUT" 2>&1)
echo "$OUT" | grep -q '0' && pass "group active returns current group" || fail "group active" "$OUT"

sg group create T_move_active_test >/dev/null
sg group select "$ORIG_OUT" T_move_active_test >/dev/null
OUT=$(sg group active "$ORIG_OUT" 2>&1)
echo "$OUT" | grep -q 'T_move_active_test' && pass "group active reflects group switch" || fail "group active after switch" "$OUT"
sg group select "$ORIG_OUT" 0 >/dev/null
sg group delete T_move_active_test --force >/dev/null

OUT=$(sg group list --output "$ORIG_OUT" 2>&1)
echo "$OUT" | grep -q "Group" && pass "group list --output returns groups" || fail "group list --output" "$OUT"

echo ""

# ============ 14. Workspace Move ============
echo -e "${BOLD}--- 14. Workspace Move ---${NC}"

sg group create T_move_x >/dev/null
sg group create T_move_y >/dev/null
sg workspace add "$WS_A" -g 0 >/dev/null

OUT=$(sg workspace move "$WS_A" --groups T_move_x 2>&1)
echo "$OUT" | grep -q "Moved workspace \"$WS_A\" to group(s): T_move_x" && pass "workspace move to single group" || fail "workspace move single" "$OUT"

OUT=$(sg workspace groups "$WS_A" 2>&1)
echo "$OUT" | grep -q 'T_move_x' && pass "move removed from old group (0), now only in T_move_x" || fail "move removed old" "$OUT"
! echo "$OUT" | grep -q '"0"' && pass "move no longer in group 0" || fail "move still in 0" "$OUT"

OUT=$(sg workspace move "$WS_A" --groups T_move_x,T_move_y 2>&1)
echo "$OUT" | grep -q "Moved workspace" && pass "workspace move to multiple groups" || fail "workspace move multiple" "$OUT"

OUT=$(sg workspace groups "$WS_A" 2>&1)
echo "$OUT" | grep -q 'T_move_x' && echo "$OUT" | grep -q 'T_move_y' && pass "workspace is in both target groups after move" || fail "move both groups" "$OUT"

OUT=$(sg workspace move __nonexistent__ --groups T_move_x 2>&1)
echo "$OUT" | grep -qi 'not found' && pass "workspace move rejects non-existent workspace" || fail "move nonexistent ws" "$OUT"

OUT=$(sg workspace move "$WS_A" --groups __no_such_group__ 2>&1)
echo "$OUT" | grep -qi 'not found' && pass "workspace move rejects non-existent group" || fail "move nonexistent group" "$OUT"

# Move back to 0 for cleanup
sg workspace move "$WS_A" --groups 0 >/dev/null
sg group delete T_move_x --force >/dev/null
sg group delete T_move_y --force >/dev/null

echo ""

# ============ 15. Nav next-on-output / prev-on-output ============
echo -e "${BOLD}--- 15. Nav next-on-output / prev-on-output ---${NC}"

sg group select "$ORIG_OUT" 0 >/dev/null
sleep 0.3

FW_ORIG=$(focused_ws)
OUT=$(sg nav next-on-output -w 2>&1)
echo "$OUT" | grep -q 'Navigated' && pass "nav next-on-output navigates" || fail "nav next-on-output" "$OUT"

OUT=$(sg nav prev-on-output -w 2>&1)
echo "$OUT" | grep -q 'Navigated' && pass "nav prev-on-output navigates" || fail "nav prev-on-output" "$OUT"

echo ""

# ============ 16. Daemon ====
echo -e "${BOLD}--- 16. Daemon ---${NC}"

sg daemon stop >/dev/null 2>&1 || true
sleep 1

OUT=$(sg daemon status 2>&1)
echo "$OUT" | grep -q 'not running' && pass "daemon not running after stop" || fail "daemon not running" "$OUT"

OUT=$(sg daemon start 2>&1)
echo "$OUT" | grep -q 'Started' && pass "daemon start" || fail "daemon start" "$OUT"
sleep 2

OUT=$(sg daemon status 2>&1)
echo "$OUT" | grep -q 'running' && pass "daemon running after start" || fail "daemon running" "$OUT"

sleep 1

OUT=$(sg daemon start 2>&1 || true)
echo "$OUT" | grep -qi 'already running' && pass "daemon start rejects duplicate" || fail "daemon start duplicate" "output: $OUT"

echo ""
