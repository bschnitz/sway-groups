# Test05: Multi-Group Workspaces

## Overview

Tests for workspaces that exist in multiple groups simultaneously.

## Test Plans

### test05a.sh — `workspace add` for existing workspace in another group
- [ ] Implement
- [ ] Pass

**Setup:** Create Group A, add WS1 with kitty to Group A, switch to Group B
**Action:** `swayg workspace add WS1`
**Expectation:** WS1 gets a second membership in Group B
**Checks:** WS1 in both groups in DB, WS1 exists once in sway

### test05b.sh — `container move` to workspace in another group
- [ ] Implement
- [ ] Pass

**Setup:** Create Group A + B, WS1 in Group A with kitty, switch to Group B
**Action:** Launch kitty WS2, `swayg container move WS1 --switch-to-workspace`
**Expectation:** Container moved to existing WS1, WS1 added to Group B
**Checks:** Focused on WS1, kitty WS2 on WS1, WS1 in both groups in DB

### test05c.sh — `workspace rename` to workspace in another group (merge)
- [ ] Implement
- [ ] Pass

**Setup:** WS1 in Group A, WS2 in Group B (both with kitties)
**Action:** `swayg workspace rename WS2 WS1`
**Expectation:** Merge — containers from WS2 moved to WS1, WS1 gets union of group memberships, WS2 deleted from DB
**Checks:** WS2 gone from DB, WS1 in both groups, kitty WS2 on WS1, one WS1 in sway

### test05d.sh — `workspace global` on multi-group workspace
- [ ] Implement
- [ ] Pass

**Setup:** WS1 in Group A and Group B
**Action:** `swayg workspace global WS1`
**Expectation:** WS1 removed from ALL groups
**Checks:** is_global=1, no workspace_groups entries for WS1

### test05e.sh — `workspace unglobal` on previously multi-group workspace
- [ ] Implement
- [ ] Pass

**Setup:** WS1 was global (no group memberships)
**Action:** `swayg workspace unglobal WS1`
**Expectation:** WS1 added to active group only
**Checks:** is_global=0, WS1 in exactly one group (active)

### test05f.sh — `workspace remove` from one group keeps other membership
- [ ] Implement
- [ ] Pass

**Setup:** WS1 in Group A and Group B
**Action:** `swayg workspace remove WS1` (from active group)
**Expectation:** WS1 removed from active group only
**Checks:** WS1 not in active group, WS1 in other group, WS1 still in sway

### test05g.sh — Auto-delete with multi-group workspace
- [ ] Implement
- [ ] Pass

**Setup:** Group A has only WS1 (WS1 also in B), Group B has WS1 + other workspaces
**Action:** Switch away from Group A
**Expectation:** Group A NOT auto-deleted (WS1 still in sway)
**Then:** Kill kitty on WS1, activate Group A and switch away
**Expectation:** Group A NOW auto-deleted (WS1 gone from sway)
