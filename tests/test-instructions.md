# Test Instructions

## General Rules

1. **Every assertion must have PASS/FAIL output.** Never check something without printing the result.
    - Use `pass()` for successful checks, `fail()` for failures.
    - If a precondition fails, ABORT immediately with `exit 1`.

2. **Always notify when work is done.** Use `notify-send "swayg" "<summary>"` when a task is finished (test written, test executed, plan presented, etc.). This applies to all work, not just tests.

2. **Check preconditions BEFORE init.** Things like "test group must not exist" must be checked against the current (real) DB, not the freshly initialized one. `init` comes AFTER preconditions.

3. **Always remember original state and restore it.**
   - Save the original workspace and group at the start.
   - At the end, switch back to the original workspace.
   - Verify the original workspace is focused again.
   - **CRITICAL: On ANY FAIL, switch back to original workspace immediately.** Use `trap` to ensure this happens even on unexpected exit. Never leave the user on a wrong workspace.

4. **NEVER switch workspaces/groups live without switching back immediately.** The user sees what happens on screen and gets annoyed.

5. **Use `swayg` commands, not `swaymsg`.** Tests should exercise `swayg` behavior. `swaymsg` is only acceptable for cleanup (killing windows, switching back).

6. **Cleanup must not re-import stale data.** Do NOT use `swayg init` for cleanup if test workspaces still exist in sway — `sync_from_sway` will re-import them. Instead, manually delete test data from DB with `sqlite3`.

7. **Test names:** Use `zz_` prefix for test groups/workspaces to sort after user's real groups ("moves", "sway-groups").

8. **Provide a summary at the end.** Always print "X passed, Y failed" at the end. Exit with non-zero if any failures.

9. **No premature cleanup.** Only clean up after all assertions are done. Never interleave cleanup with verification.

10. **Expand these rules proactively.** When discovering a new pattern, pitfall, or rule during testing, add it here without being asked.

11. **Reporting format.** After running a test, always summarize the results in this exact format:
    ```
    swayg init
    PASS - <description>
    swayg group select ...
    PASS - <description>
    PASS - <description>
    FAIL - <description>  (if any)
    ```
    One line with the essential command, then PASS/FAIL lines for the assertions that followed. No extra text, no grouping.

12. **On FAIL: stop and report.** Do not try to fix problems during test execution. Run the test, report results with PASS/FAIL summary, and wait for instructions.

## Kitty Handling

13. **In sway (Wayland), kitty uses `app_id` not `window_properties.class`.** The `--class` flag sets `app_id` on the sway node. `window_properties.class` is always `null` for Wayland apps.
    - jq queries: use `.app_id` not `.window_properties.class`
    - swaymsg kill: use `[app_id=X]` not `[class=X]`

14. **Kill kitties via PID, not swaymsg.** `swaymsg "[app_id=X] kill"` can fail on kitty's "Close OS window" dialog (zombie state). Instead:
    - Store PID when launching: `kitty --class X >/dev/null 2>&1 & echo $! > /tmp/sway-group-tests/X.pid`
    - Kill via PID: `kill $(cat /tmp/sway-group-tests/X.pid)`
    - Clean PID dir at test start: `rm -rf /tmp/sway-group-tests && mkdir -p /tmp/sway-group-tests`

15. **Do NOT use `-e sleep 300` in kitty.** This prevents kitty from being killed cleanly (sleep blocks SIGTERM). Just launch `kitty --class X` without `-e`.

16. **Verify kitty existence and workspace placement with jq:**
    - Kitty exists: `swaymsg -t get_tree | jq -r '.. | objects | select(.app_id? != null) | .app_id' | grep -c "^X$"`
    - Kitty on workspace Y: `swaymsg -t get_tree | jq -r '[.. | objects | select(.type? == "workspace")] | .[] | {name: .name, apps: [.. | objects | select(.app_id? != null) | .app_id]} | select(.apps | index("X")) | .name'`
    - Note: jq's `..` only traverses **downward**. To find a workspace containing a specific app_id, iterate over all workspaces and check descendants.

17. **Wait for kitty to appear in sway tree.** After launching kitty, use `sleep 0.5` before checking. Kitty needs a moment to register with sway.

## swayg Commands

26. **`swayg workspace add <name>`** creates the workspace in sway if it doesn't exist, adds it to DB, and assigns it to the active group. No need for `swaymsg workspace <name>` + `swayg sync`.

27. **`swayg container move <WORKSPACE> [--switch-to-workspace]`** moves the focused container to the target workspace. If the workspace doesn't exist in sway, sway creates it automatically on `move container`. The command also adds the workspace to the active group in DB.

28. **`swayg group select <OUTPUT> <GROUP> --create`** creates the group and switches to it. When switching to a group with no workspaces, workspace "0" is focused (temporary, auto-deleted by sway on switch away). Empty groups are auto-deleted when switching away.

## Visibility Checks

29. **`swayg workspace list --visible --plain --output <OUTPUT>`** lists all workspaces visible in the active group for that output (including global workspaces). Use this to verify workspace visibility:
    - Workspace visible: `echo "$OUT" | grep -q "$WS"` → expect `0` exit code
    - Workspace NOT visible: `echo "$OUT" | grep -q "$WS"` → expect non-zero exit code
    - **Note:** `--visible` is important — without it, all group workspaces are shown regardless of global status.

## sway Behavior

21. **`swaymsg workspace "0"`** creates a temporary workspace "0" but sway deletes it when switching away (empty workspace with no children).

22. **Auto-deletion of empty groups:** A group is deleted when:
    - Switching away from it
    - The currently focused workspace is empty (no children)
    - No other non-global workspaces from that group exist in sway
    - The old_group != "0" and old_group != new_group

23. **Empty workspace detection:** A workspace is empty when `representation` is `null` in `get_workspaces()` output. This is checked via `is_focused_workspace_empty()` using the `SwayIpcClient`.

24. **`move container to workspace` creates the workspace in sway.** No need to pre-create it. Sway automatically creates the target workspace when moving a container there.

25. **Sway doesn't delete empty workspaces immediately.** After `move container`, the old workspace persists as empty in sway until a focus switch happens. Use `swaymsg workspace "$OTHER_WS"; sleep 0.1` before checking the old workspace is gone.

## Precondition Checks

30. **Check preconditions for:** test group not in DB, test workspaces not in DB, test workspaces not in sway, test kitties not running. All checked BEFORE `init`.

## DB / Paths

31. **DB path:** Always use `~/.local/share/swayg/swayg.db`.
32. **Binary path:** Always use `$HOME/.cargo/bin/swayg`.

## Shell Script Pitfalls

33. **Shell quoting in `run()` helper:** The `run()` helper uses `eval "$@"`. Single-quoted strings don't expand variables. For variable expansion inside `eval`, use double quotes carefully.

34. **`run()` captures exit code of last command.** After a `run()` call, `$?` reflects the last command in the `eval` string, not the swayg command if it was followed by `>/dev/null 2>&1`. Check swayg exit code separately if needed.

## Presenting Test Results

35. **When asked to run tests, present results ONLY.** Run all tests sequentially. For each test, show a one-line summary (`test_name — X/Y PASS`). For the FAILING test, show the PASS/FAIL block using the format from rule 11. Stop after the first failing test — do NOT run subsequent tests.

36. **NO analysis, NO explanation.** Never explain WHY a test failed. Never diagnose the root cause. Never suggest fixes. The user will ask if they want analysis. Just present the raw results.

37. **Exact reporting format when a test fails:**
    ```
    test01_group_select.sh — 8/8 PASS
    test02_new_workspace.sh — 24/24 PASS
    test03_global_workspace.sh — 29/29 PASS
    test04_workspace_move.sh

    swayg init
    PASS - init succeeded
    swayg group select ...
    PASS - group created
    FAIL - description of failure

    ABORTED — 2 passed, 1 failed
    ```

38. **If all tests pass**, show each test on one line:
    ```
    test01_group_select.sh — 8/8 PASS
    test02_new_workspace.sh — 24/24 PASS
    test03_global_workspace.sh — 29/29 PASS
    test04_workspace_move.sh — 18/18 PASS
    ```

39. **If the first test fails**, do NOT continue to the next test. Show only the failing test's block.

## Presenting Test Plans

43. **Test plan format:** When asked to present a test plan, use this format — command line followed by dash-prefixed assertions. No PASS/FAIL, no explanation, no analysis. Separate test phases with `--- Section name ---` headers.

44. **Test plan section headers:**
    - `--- Precondition checks (BEFORE init) ---` — all precondition assertions
    - `--- Setup ---` — init, group creation, kitty launch, workspace creation
    - `--- Test ---` — the core assertions being tested
    - `--- Cleanup ---` — kill kitties, switch back, auto-delete verification
    - `--- Post-condition ---` — final DB verification

45. **Test plan completeness:** Include all steps from precondition checks through post-condition. Each `run()` command gets its own block with all assertions listed below it.

46. **Setup-Block:** All setup commands (init, group creation, kitty launch, workspace moves, group switches) go in a single `run()` block. After that, a single verification block checks the entire setup state. This applies only to the Setup section. Test, Cleanup, and Post-condition sections follow the normal pattern (command then assertions).

    Example:
    ```
    --- Setup ---
    [one run() block: init + create group + launch kitty + move container]

    Verify setup:
    - group exists
    - kitty running
    - workspace in group
    ```

    Example:
    ```
    --- Precondition checks (BEFORE init) ---
    - __test_group_a__ does not exist in DB
    - __tg_ws1__ does not exist in sway

    --- Setup ---
    swayg init
    - init succeeded

    swayg group select eDP-1 "__test_group_a__" --create
    - group created
    - active group set

    kitty --class "__tg_ws1__"; sleep 0.5
    - kitty running

    --- Test ---
    swayg workspace remove "__tg_ws1__"
    - removed from active group only
    - still in other group

    --- Cleanup ---
    swayg group select eDP-1 "$ORIG_GROUP"
    - focused on original workspace

    kill kitty
    - kitty gone

    --- Post-condition ---
    - no test data remains in DB
    ```

## Post-conditions

40. **Every test MUST have a post-condition** that verifies no test data remains after the test finishes. Check that all test groups, test workspaces, and test workspace_groups entries have been cleaned up from the DB.
41. **Post-condition format:** Use a single `run()` step with sqlite3 queries, followed by PASS/FAIL assertions for each category (groups, workspaces, workspace_groups).
42. **If test workspaces existed in sway during the test** but were killed (kitties), run `swayg init >/dev/null 2>&1` before the post-condition check to let `sync_from_sway` clean up stale workspace rows from the DB.

## Test File Naming

- `test01_*.sh`, `test02_*.sh`, etc. — numbered for ordering.
- Name should describe what is being tested (e.g., `test01_group_select.sh`).

## Test Structure Template

```bash
#!/bin/bash
# Test: <description>

SG="$HOME/.cargo/bin/swayg"
DB=~/.local/share/swayg/swayg.db
TEST_GROUP="__test_group__"
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

# Always switch back to original workspace on FAIL
trap '[ "$FAIL" -gt 0 ] && swaymsg workspace "$ORIG_WS" >/dev/null 2>&1' EXIT

echo -e "\033[1m=== Test: <name> ===\033[0m"

# Clean PID dir
rm -rf "$PID_DIR"
mkdir -p "$PID_DIR"

# 1. Precondition checks (BEFORE init)
# 2. Remember original state
# 3. Init
# 4. Test actions (with assertions after each)
# 5. Cleanup (kill kitties via PID)
# 6. Post-cleanup verification
# 7. Switch back to original workspace
# 8. Summary
```
