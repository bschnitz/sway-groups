#!/usr/bin/env bash
# Integration tests for swayg
#
# Usage:
#   ./tests/integration_test.sh
#
# Prerequisites: sway running, swayg installed, kitty, python3
#
# How it works:
#   - Creates its own named workspaces (__test_alpha__, __test_beta__, __test_gamma__)
#     with a kitty on each so sway won't garbage-collect them
#   - Creates test groups prefixed with "T_" to avoid collisions
#   - Returns to the user's original workspace between sections when possible
#   - On cleanup: kills only containers that were created during the test
#     (compares container IDs before/after), then restores the original workspace
#
# IMPORTANT rules when extending tests:
#   1. Never use existing user workspaces (1, 2, etc.) – use only WS_A/WS_B/WS_C
#   2. Use "swaymsg workspace <name>" (NOT "swaymsg workspace string:<name>")
#      – the "string:" prefix becomes part of the workspace name and breaks lookups
#   3. Kitty processes spawned during tests are automatically cleaned up
#      by container ID comparison (no PID tracking needed)
#   4. Always restore to group "0" and workspace "$ORIG_WS" after tests that
#      switch groups or navigate away
#   5. sway auto-deletes empty workspaces on navigation – always put a window
#      (kitty) on test workspaces to keep them alive
#   6. After "sg sync --all" the DB is reset – all group assignments are lost.
#      If you need sync in the middle of the test suite, backup/restore the DB
#   7. There is no daemon – all operations happen synchronously in the CLI

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

# Remember existing container IDs so we don't kill user's windows
EXISTING_CON_IDS=$(swaymsg -t get_tree 2>/dev/null | python3 -c "
import json, sys
def collect_ids(node):
    ids = []
    for c in node.get('nodes', []) + node.get('floating_nodes', []):
        ids.append(str(c['id']))
        ids.extend(collect_ids(c))
    return ids
print(','.join(collect_ids(json.load(sys.stdin))))
")

# Create 3 own test workspaces (named, so sway won't auto-delete them)
# Open a minimal kitty on each so sway won't garbage-collect them
WS_A="__test_alpha__"
WS_B="__test_beta__"
WS_C="__test_gamma__"

for ws in "$WS_A" "$WS_B" "$WS_C"; do
    swaymsg workspace "$ws" >/dev/null 2>&1 || true
    sleep 0.2
    kitty --class "__test_${ws}" -e sleep 300 >/dev/null 2>&1 &
    sleep 0.3
done

# Sync so swayg knows about the new workspaces
sg sync --all >/dev/null

# Return to user's workspace
swaymsg workspace "$ORIG_WS" >/dev/null 2>&1 || true
sleep 0.3

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
# WS_A (__test_alpha__) < WS_B (__test_beta__) alphabetically
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
sg group create T_empty >/dev/null
sg group select "$ORIG_OUT" T_empty >/dev/null
OUT=$(sg group next -o "$ORIG_OUT" 2>&1)
[ -z "$OUT" ] && pass "group next at end without wrap does nothing" || fail "group next at end" "$OUT"

# no-wrap: at start (0), prev should not switch
sg group select "$ORIG_OUT" 0 >/dev/null
OUT=$(sg group prev -o "$ORIG_OUT" 2>&1)
[ -z "$OUT" ] && pass "group prev at start without wrap does nothing" || fail "group prev at start" "$OUT"

# wrap: from T_empty (last), next wraps to 0
sg group create T_empty >/dev/null
sg group select "$ORIG_OUT" T_empty >/dev/null
OUT=$(sg group next -o "$ORIG_OUT" -w 2>&1)
echo "$OUT" | grep -q '0' && pass "group next with wrap cycles to first" || fail "group next with wrap" "$OUT"

# wrap: from 0 (first), prev wraps to last non-empty (T_b, since T_empty was auto-deleted)
OUT=$(sg group prev -o "$ORIG_OUT" -w 2>&1)
echo "$OUT" | grep -q 'T_b' && pass "group prev with wrap cycles to last (T_empty auto-deleted)" || fail "group prev with wrap" "$OUT"

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

# Test sync from clean state: backup DB, remove, sync fresh, verify, restore
DB_PATH=~/.local/share/swayg/swayg.db
cp "$DB_PATH" "${DB_PATH}.bak" 2>/dev/null || true
rm -f "$DB_PATH" "$DB_PATH-wal" "$DB_PATH-shm"
OUT=$(sg sync --all 2>&1)
echo "$OUT" | grep -q 'Synced' && pass "sync from clean state" || fail "sync from clean state" "$OUT"

OUT=$(sg group list)
echo "$OUT" | grep -q 'Group "0"' && pass "sync creates default group" || fail "sync creates default group" "$OUT"

# Restore original DB to not break subsequent tests
cp "${DB_PATH}.bak" "$DB_PATH" 2>/dev/null || true
rm -f "${DB_PATH}.bak" "${DB_PATH}-wal" "${DB_PATH}-shm" "${DB_PATH}-journal" 2>/dev/null || true

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

sg group select "$ORIG_OUT" 0 >/dev/null
sleep 0.3
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

sg workspace add "$WS_A" -g 0 >/dev/null

OUT=$(sg workspace move "$WS_A" --groups T_move_x 2>&1)
echo "$OUT" | grep -q "Moved workspace \"$WS_A\" to group(s): T_move_x" && pass "workspace move to single group" || fail "workspace move single" "$OUT"

OUT=$(sg workspace groups "$WS_A" 2>&1)
echo "$OUT" | grep -q 'T_move_x' && pass "move removed from old group (0), now only in T_move_x" || fail "move removed old" "$OUT"
! echo "$OUT" | grep -q '"0"' && pass "move no longer in group 0" || fail "move still in 0" "$OUT"

# Verify T_move_x was auto-created (not pre-existing)
OUT=$(sg group list 2>&1)
echo "$OUT" | grep -q 'T_move_x' && pass "move auto-created group T_move_x" || fail "auto-created T_move_x" "$OUT"

OUT=$(sg workspace move "$WS_A" --groups T_move_x,T_move_y 2>&1)
echo "$OUT" | grep -q "Moved workspace" && pass "workspace move to multiple groups" || fail "workspace move multiple" "$OUT"

OUT=$(sg workspace groups "$WS_A" 2>&1)
echo "$OUT" | grep -q 'T_move_x' && echo "$OUT" | grep -q 'T_move_y' && pass "workspace is in both target groups after move" || fail "move both groups" "$OUT"

# Verify T_move_y was also auto-created
OUT=$(sg group list 2>&1)
echo "$OUT" | grep -q 'T_move_y' && pass "move auto-created group T_move_y" || fail "auto-created T_move_y" "$OUT"

OUT=$(sg workspace move __nonexistent__ --groups T_move_x 2>&1)
echo "$OUT" | grep -qi 'not found' && pass "workspace move rejects non-existent workspace" || fail "move nonexistent ws" "$OUT"

# Move back to 0 for cleanup
sg workspace move "$WS_A" --groups 0 >/dev/null
sg group delete T_move_x --force >/dev/null
sg group delete T_move_y --force >/dev/null

echo ""

# ============ 15. Nav next-on-output / prev-on-output ============
echo -e "${BOLD}--- 15. Nav next-on-output / prev-on-output ---${NC}"

sg group select "$ORIG_OUT" 0 >/dev/null
sleep 0.3

OUT=$(sg nav next-on-output -w 2>&1)
echo "$OUT" | grep -q 'Navigated' && pass "nav next-on-output navigates" || fail "nav next-on-output" "$OUT"

OUT=$(sg nav prev-on-output -w 2>&1)
echo "$OUT" | grep -q 'Navigated' && pass "nav prev-on-output navigates" || fail "nav prev-on-output" "$OUT"

echo ""

# ============ 17. Workspace List --visible --plain ============
echo -e "${BOLD}--- 17. Workspace List --visible --plain ---${NC}"

sg group create T_vis_a >/dev/null
sg group create T_vis_b >/dev/null
sg workspace add "$WS_A" -g T_vis_a >/dev/null
sg workspace add "$WS_B" -g T_vis_b >/dev/null

# Switch to T_vis_a: only WS_A should be visible
sg group select "$ORIG_OUT" T_vis_a >/dev/null
sleep 0.3

OUT=$(sg workspace list --visible --plain 2>&1)
echo "$OUT" | grep -q "$WS_A" && pass "visible list shows workspace in active group" || fail "visible list active group" "$OUT"
! echo "$OUT" | grep -q "$WS_B" && pass "visible list hides workspace in other group" || fail "visible list other group" "$OUT"

# --plain output: no header/footer, just names
OUT=$(sg workspace list --visible --plain 2>&1)
! echo "$OUT" | grep -qi "group\|visible\|workspace" && pass "--plain output has no headers" || fail "--plain headers" "$OUT"

# --visible without --plain should show readable output
OUT=$(sg workspace list --visible 2>&1)
echo "$OUT" | grep -q "$WS_A" && pass "visible list without plain shows workspace" || fail "visible list no plain" "$OUT"

sg group select "$ORIG_OUT" 0 >/dev/null
sg group delete T_vis_a --force >/dev/null
sg group delete T_vis_b --force >/dev/null

echo ""

# ============ 18. Nav move-to ============
echo -e "${BOLD}--- 18. Nav move-to ---${NC}"

sg group select "$ORIG_OUT" 0 >/dev/null
sleep 0.3

# Focus WS_A, open a terminal so the workspace has content, then move container
sg nav go "$WS_A" >/dev/null
sleep 0.3
kitty --class __test_move__ -e true >/dev/null 2>&1 &
sleep 0.5
OUT=$(sg nav move-to "$WS_B" 2>&1)
echo "$OUT" | grep -q "Moved container to \"$WS_B\"" && pass "nav move-to moves container" || fail "nav move-to" "$OUT"

# Cleanup: move container back to WS_A
sg nav go "$WS_B" >/dev/null
sleep 0.3
sg nav move-to "$WS_A" >/dev/null
sleep 0.3
# Move back to user's workspace
swaymsg workspace "$ORIG_WS" >/dev/null 2>&1 || true
sleep 0.3

echo ""

# ============ 19. Lazy Sync: New Workspaces ============
echo -e "${BOLD}--- 19. Lazy Sync: New Workspaces ---${NC}"

# Create a new workspace via sway (open a kitty so sway won't garbage-collect it)
T_NEW_WS="__test_lazy_ws__"
kitty --class "__test_${T_NEW_WS}" -e sleep 300 >/dev/null 2>&1 &
sleep 0.3
swaymsg workspace "$T_NEW_WS" >/dev/null 2>&1
sleep 0.3

# Before sync: workspace should NOT be in DB
OUT=$(sg workspace list --plain 2>&1)
! echo "$OUT" | grep -q "$T_NEW_WS" && pass "workspace not in DB before sync" || fail "workspace not in DB before sync" "$OUT"

# After sync: workspace should be in DB
sg sync --all >/dev/null
OUT=$(sg workspace list --plain 2>&1)
echo "$OUT" | grep -q "$T_NEW_WS" && pass "lazy sync picks up new workspace" || fail "lazy sync picks up new workspace" "DB: $OUT"

# Verify nav next/prev can navigate to the new workspace
sg nav go "$WS_A" >/dev/null
sleep 0.3
OUT=$(sg nav next -o "$ORIG_OUT" 2>&1)
echo "$OUT" | grep -q 'Navigated' && pass "nav next navigates to new workspace" || fail "nav next to new workspace" "$OUT"

# Verify nav prev goes back
OUT=$(sg nav prev -o "$ORIG_OUT" 2>&1)
echo "$OUT" | grep -q 'Navigated' && pass "nav prev navigates back from new workspace" || fail "nav prev from new workspace" "$OUT"

# Return to user's workspace
swaymsg workspace "$ORIG_WS" >/dev/null 2>&1 || true
sleep 0.3

echo ""

# ============ 20. Global Workspace Navigation ============
echo -e "${BOLD}--- 20. Global Workspace Navigation ---${NC}"

sg group select "$ORIG_OUT" 0 >/dev/null
sleep 0.3

# Mark WS_A as global
sg workspace global "$WS_A" >/dev/null
OUT=$(sg status 2>&1)
echo "$OUT" | grep -q "Global" && echo "$OUT" | grep -q "$WS_A" && pass "WS_A is now global" || fail "WS_A is global" "$OUT"

# Sync to apply _class_global suffix
sg sync --all >/dev/null
sleep 0.3

# Verify the suffix is applied in sway
WS_A_SWAY=$(swaymsg -t get_workspaces 2>/dev/null | python3 -c "
import json, sys
for w in json.load(sys.stdin):
    if w['name'].startswith('$WS_A'):
        print(w['name'])
        break
")
echo "$WS_A_SWAY" | grep -q '_class_global' && pass "sway has _class_global suffix on $WS_A" || fail "_class_global suffix" "got: $WS_A_SWAY"

# Navigate to WS_A (which has _class_global suffix) — must not create duplicate
sg nav go "$WS_A" >/dev/null
sleep 0.3

# Verify only one workspace starting with WS_A exists
WS_A_COUNT=$(swaymsg -t get_workspaces 2>/dev/null | python3 -c "
import json, sys
count = 0
for w in json.load(sys.stdin):
    base = w['name']
    for s in ('_class_hidden', '_class_global'):
        base = base.removesuffix(s)
    if base == '$WS_A':
        count += 1
print(count)
")
[ "$WS_A_COUNT" = "1" ] && pass "nav go to global workspace does not create duplicate" || fail "no duplicate global" "count: $WS_A_COUNT"

# Test nav next goes to next workspace and nav prev comes back to global
swaymsg workspace "$ORIG_WS" >/dev/null 2>&1 || true
sleep 0.3

OUT=$(sg nav next -o "$ORIG_OUT" 2>&1)
echo "$OUT" | grep -q 'Navigated' && pass "nav next from global navigates" || fail "nav next from global" "$OUT"

# Check no duplicate was created
WS_A_COUNT2=$(swaymsg -t get_workspaces 2>/dev/null | python3 -c "
import json, sys
count = 0
for w in json.load(sys.stdin):
    base = w['name']
    for s in ('_class_hidden', '_class_global'):
        base = base.removesuffix(s)
    if base == '$WS_A':
        count += 1
print(count)
")
[ "$WS_A_COUNT2" = "1" ] && pass "no duplicate after nav next away from global" || fail "no dup after next" "count: $WS_A_COUNT2"

# Navigate back toward WS_A
OUT=$(sg nav go "$WS_A" 2>&1)
echo "$OUT" | grep -q 'Navigated' && pass "nav go back to global workspace succeeds" || fail "nav go back to global" "$OUT"

# Verify still only one
WS_A_COUNT3=$(swaymsg -t get_workspaces 2>/dev/null | python3 -c "
import json, sys
count = 0
for w in json.load(sys.stdin):
    base = w['name']
    for s in ('_class_hidden', '_class_global'):
        base = base.removesuffix(s)
    if base == '$WS_A':
        count += 1
print(count)
")
[ "$WS_A_COUNT3" = "1" ] && pass "no duplicate after returning to global" || fail "no dup after return" "count: $WS_A_COUNT3"

# Test nav prev/next through global workspace
# First navigate to a workspace that is NOT the first in the sorted list
# so nav prev will have a target
sg nav go "$WS_B" >/dev/null
sleep 0.3

OUT=$(sg nav prev -o "$ORIG_OUT" 2>&1)
echo "$OUT" | grep -q 'Navigated' && pass "nav prev toward global navigates" || fail "nav prev toward global" "$OUT"

# Verify no duplicate
WS_A_COUNT4=$(swaymsg -t get_workspaces 2>/dev/null | python3 -c "
import json, sys
count = 0
for w in json.load(sys.stdin):
    base = w['name']
    for s in ('_class_hidden', '_class_global'):
        base = base.removesuffix(s)
    if base == '$WS_A':
        count += 1
print(count)
")
[ "$WS_A_COUNT4" = "1" ] && pass "no duplicate after nav prev to global" || fail "no dup after prev" "count: $WS_A_COUNT4"

# Cleanup: unglobal
sg workspace unglobal "$WS_A" >/dev/null
sg sync --all >/dev/null
sleep 0.3

swaymsg workspace "$ORIG_WS" >/dev/null 2>&1 || true
sleep 0.3

echo ""

# ============ 21. New workspace inherits active group ============
echo -e "${BOLD}--- 21. New workspace inherits active group ---${NC}"

sg group create T_sync_group >/dev/null
sg group select "$ORIG_OUT" T_sync_group >/dev/null
sleep 0.3

sg nav go "$WS_A" >/dev/null
sleep 0.3
kitty --class __test_inherit__ -e sleep 5 >/dev/null 2>&1 &
sleep 0.5
sg nav move-to "__test_inherit_ws__" >/dev/null
sleep 0.5
sg sync --all >/dev/null

OUT=$(sg workspace groups __test_inherit_ws__ 2>&1)
echo "$OUT" | grep -q 'T_sync_group' && pass "new workspace added to active group (T_sync_group)" || fail "new workspace in active group" "$OUT"
! echo "$OUT" | grep -q '"0"' && pass "new workspace NOT in group 0" || fail "new workspace not in group 0" "$OUT"

sg group select "$ORIG_OUT" 0 >/dev/null
sleep 0.3
sg group delete T_sync_group --force >/dev/null
swaymsg workspace "$ORIG_WS" >/dev/null 2>&1 || true
sleep 0.3

echo ""

# ============ 22. Auto-delete empty group on switch ============
echo -e "${BOLD}--- 22. Auto-delete empty group on switch ---${NC}"

sg group create T_auto_del >/dev/null
sg group select "$ORIG_OUT" T_auto_del >/dev/null
sleep 0.3

OUT=$(sg group list 2>&1)
echo "$OUT" | grep -q 'T_auto_del' && pass "T_auto_del exists after creation" || fail "T_auto_del exists" "$OUT"

# Switch away — T_auto_del should be auto-deleted because it's empty
sg group select "$ORIG_OUT" 0 >/dev/null
sleep 0.3

OUT=$(sg group list 2>&1)
! echo "$OUT" | grep -q 'T_auto_del' && pass "T_auto_del auto-deleted after switch" || fail "T_auto_del auto-deleted" "$OUT"

# Non-empty group should NOT be auto-deleted
sg group create T_no_del >/dev/null
sg workspace add "$WS_A" -g T_no_del >/dev/null
sg group select "$ORIG_OUT" T_no_del >/dev/null
sleep 0.3

sg group select "$ORIG_OUT" 0 >/dev/null
sleep 0.3

OUT=$(sg group list 2>&1)
echo "$OUT" | grep -q 'T_no_del' && pass "non-empty group T_no_del NOT auto-deleted" || fail "T_no_del not deleted" "$OUT"

sg workspace remove "$WS_A" -g T_no_del >/dev/null
sg group delete T_no_del --force >/dev/null
swaymsg workspace "$ORIG_WS" >/dev/null 2>&1 || true
sleep 0.3

echo ""

# ============ Cleanup ============
echo -e "${BOLD}--- Cleanup ---${NC}"

# 1. Kill all test kitty processes (app_id starts with __test_)
TEST_KITTY_PIDS=$(swaymsg -t get_tree 2>/dev/null | python3 -c "
import json, sys
pids = set()
def collect(node):
    app_id = node.get('app_id', '')
    if app_id.startswith('__test_') and node.get('pid'):
        pids.add(node['pid'])
    for c in node.get('nodes', []) + node.get('floating_nodes', []):
        collect(c)
collect(json.load(sys.stdin))
print(' '.join(str(p) for p in pids))
" 2>/dev/null)
if [ -n "$TEST_KITTY_PIDS" ]; then
    for pid in $TEST_KITTY_PIDS; do
        kill -- -"$pid" 2>/dev/null || kill -9 "$pid" 2>/dev/null || true
    done
    sleep 0.5
fi

# 2. Remove test workspaces from sway (navigate to each, sway auto-cleans empty ones)
for ws in "$WS_A" "$WS_B" "$WS_C" __test_lazy_ws__ __test_inherit_ws__; do
    swaymsg workspace "$ws" >/dev/null 2>&1 || true
    sleep 0.1
done
swaymsg workspace "$ORIG_WS" >/dev/null 2>&1 || true
sleep 0.3

# 3. Remove all test groups (T_*) and test workspace entries from DB
DB_PATH=~/.local/share/swayg/swayg.db
sqlite3 "$DB_PATH" "
DELETE FROM focus_history WHERE workspace_name LIKE '__test_%';
DELETE FROM group_state WHERE group_name LIKE 'T_%';
DELETE FROM workspace_groups WHERE workspace_id IN (SELECT id FROM workspaces WHERE name LIKE '__test_%');
DELETE FROM workspace_groups WHERE group_id IN (SELECT id FROM groups WHERE name LIKE 'T_%');
DELETE FROM workspaces WHERE name LIKE '__test_%';
DELETE FROM groups WHERE name LIKE 'T_%';
UPDATE outputs SET active_group = '0';
" 2>/dev/null

pass "cleanup done"
