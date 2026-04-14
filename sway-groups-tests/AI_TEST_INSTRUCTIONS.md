# AI Test Instructions — sway-groups-tests

Rules for writing Rust integration tests using `assert_cmd` + `DummyWindowHandle`.
Based on `tests/test-instructions.md`, adapted for Rust.

## General Rules

1. **Tests exercise the `swayg` CLI binary, not internal services.** Every test action is a `Command::cargo_bin("swayg")` call. Never call `GroupService`, `WorkspaceService`, etc. directly. This is an integration test, not a unit test.

2. **Use `Command::cargo_bin("swayg")` for all swayg invocations.** Never call internal Rust APIs. The whole point is testing the CLI end-to-end.

3. **Use `DummyWindowHandle` instead of kitty.** Spawn dummy windows via `sway-dummy-window` binary (200x100 pixel, visible in sway). Faster, no external dependencies.

4. **DB assertions via `sqlite3` CLI, not `rusqlite`.** `rusqlite` conflicts with `libsqlite3-sys` from `sqlx`/`sea-orm` in `sway-groups-core`. Use `Command::new("sqlite3")` for DB queries in tests.

5. **Test DB path: `/tmp/swayg-integration-test.db`.** Never touch the production DB. Configure swayg to use this DB via `--db` flag.

6. **Check preconditions BEFORE any swayg commands.** Verify test groups/workspaces don't exist in DB AND in sway before running setup.

7. **Always restore original state.** Save original workspace/output at start. Switch back in `Drop` or test cleanup. Never leave the user on a wrong workspace.

8. **NEVER switch workspaces/groups live without switching back immediately.** The user sees what happens on screen.

9. **`swaymsg` is only acceptable for:** cleanup (switching back), killing dummy windows, checking sway state for assertions.

10. **All tests run with `--test-threads=1`.** Sway state is global, tests must be sequential.

## Naming

11. **Test functions:** `test_<number>_<description>` — e.g., `test_01_group_select`, `test_02_workspace_with_containers`.

12. **Test names:** Use `zz_test_` prefix for test groups/workspaces to sort after user's real groups.

## Test Structure

Every test follows this pattern:

```rust
#[tokio::test]
async fn test_01_something() {
    // 1. Setup fixture
    let fixture = TestFixture::new().await.unwrap();

    // 2. Remember original state (orig_group, orig_ws) BEFORE preconditions
    let orig_group = swayg_output_without_db(&["group", "active", &fixture.orig_output]);

    // 3. Precondition checks (production DB + sway state)
    //    - Test groups not in production DB
    //    - Test workspaces not in production DB
    //    - Test workspaces not in sway

    // 4. Init + setup (swayg CLI calls with --db flag)
    fixture.init().success();
    fixture.swayg(&["group", "select", GROUP, "--output", &fixture.orig_output, "--create"]).success();

    // 5. Verify setup (DB + CLI assertions)
    //    - Group exists in DB (sqlite3 count)
    //    - Active group via CLI (swayg group active)

    // 6. Spawn dummy windows + verify in sway tree
    let _win = DummyWindowHandle::spawn(WS1).expect("spawn");
    sleep(500ms);
    assert!(workspace_of_window(WS1).is_some(), "window registered in sway");

    // 7. Test actions (swayg CLI calls)

    // 8. Assertions (DB + sway state via CLI)

    // 9. Cleanup (kill windows FIRST while on orig group, then auto-delete groups)
    drop(_win);
    assert!(workspace_of_window(WS1).is_none(), "window gone from sway");
    for g in &[GROUP_A, GROUP_B] {
        fixture.swayg(&["group", "select", g, "--output", &fixture.orig_output]).success();
        fixture.swayg(&["group", "select", &orig_group, "--output", &fixture.orig_output]).success();
    }
    assert_eq!(db_count("groups WHERE name = GROUP"), 0, "auto-deleted");

    // 10. Post-condition (init sync + verify no test data remains)
    fixture.init().success();
    assert_eq!(db_count("groups WHERE name IN (GROUP_A, GROUP_B)"), 0);
    assert_eq!(db_count("workspaces WHERE name IN (WS1, WS2)"), 0);
    assert_eq!(db_count("workspace_groups wg JOIN groups g ON ..."), 0);
}
```

## Sway Behavior

21. **`move container to workspace` creates the workspace in sway.** No need to pre-create it.

22. **Sway doesn't delete empty workspaces immediately.** After `move container`, the old workspace persists until a focus switch. Use `sleep(100ms)` + focus switch before checking.

23. **Workspace "0" is temporary.** Sway creates it when focusing an empty group, deletes it on switch away.

24. **Empty workspace detection:** `representation` is `null` in `get_workspaces()` output.

25. **Sway keeps focused workspace even if empty.** If you kill a window on the focused workspace, the workspace stays. Switch away FIRST before killing, otherwise the workspace won't be removed by sway.

26. **Auto-delete only triggers on the OLD active group.** When `group select` switches from group A to group B, only group A is checked for auto-deletion. To auto-delete multiple empty groups, you must iterate: select each group then switch away.

27. **`set_active_group` adds workspace "0" to empty groups.** When switching to an empty group, workspace "0" gets added. This prevents `group prune` from removing the group (since "0" is non-global and exists in sway). Don't rely on `prune` for cleanup after group switches.

## DB Queries

28. **Group exists:** `SELECT count(*) FROM groups WHERE name = ?` → assert 1
29. **Workspace exists:** `SELECT count(*) FROM workspaces WHERE name = ?` → assert 1
30. **Workspace in group:** JOIN `workspace_groups`, `groups`, `workspaces` on names
31. **Active group:** Use `swayg group active <output>` CLI, NOT direct SQL
32. **No test data:** No groups/workspaces with `zz_test_` prefix in their name

## Assertion Rules

33. **Active group MUST be checked via CLI**, not via direct SQL (`SELECT active_group FROM outputs`). Use `swayg_output(&fixture.db_path, &["group", "active", &fixture.orig_output])`.

34. **After every `DummyWindowHandle::spawn`, verify the window in sway.** Add `assert!(workspace_of_window(WS).is_some(), "...")` after the sleep. Process spawn success does NOT guarantee sway registered the window.

35. **After killing windows, verify they're gone from sway.** Add `assert!(workspace_of_window(WS).is_none(), "...")` after the sleep. Don't silently continue if the window failed to close.

36. **After `group select --create`, assert the group exists in DB.** Don't only check the active group — also verify `SELECT count(*) FROM groups WHERE name = ?` == 1.

37. **Every test MUST have a post-condition** verifying no test data remains. Check groups, workspaces, AND workspace_groups for test entity names.

38. **Auto-delete must be explicitly tested.** Don't rely solely on `fixture.init()` in post-condition to clean up. Exercise the auto-delete code path: select the empty group, switch away, then assert it's gone. Exception: if the group was already auto-deleted during a prior switch (e.g., `unglobal` made it effectively empty), verify via `init` sync instead.

39. **Post-condition checks MUST match the shell tests.** If the shell test checks `groups + workspaces + workspace_groups`, the Rust test must also check all three. If the shell only checks `groups + workspaces`, the Rust test should too.

40. **Preconditions check the PRODUCTION DB (no `--db` flag), not the test DB.** The test DB doesn't exist yet at precondition time. Guard with `if real_db.exists()` since the production DB may not exist on a fresh install.

## swayg Command Reference

These commands are used in tests (check latest syntax in CLI):

- `swayg init` — drop + recreate DB, sync from sway
- `swayg group select <group> [--output <output>] --create` — create + switch to group (output auto-resolved if omitted)
- `swayg group create <name>` — create group (error if exists)
- `swayg group delete <name> --force` — delete group
- `swayg group rename <old> <new>` — rename group
- `swayg group next [--output <output>] [--wrap]` — next group (output auto-resolved if omitted)
- `swayg group prev [--output <output>] [--wrap]` — prev group (output auto-resolved if omitted)
- `swayg group prune [--keep <n>]` — prune empty groups
- `swayg group list` — list groups
- `swayg group active <output>` — show active group
- `swayg workspace add <name>` — create workspace in sway + DB
- `swayg workspace remove <name>` — remove from active group
- `swayg workspace rename <old> <new>` — rename (simple or merge)
- `swayg workspace list [--visible] [--plain] [--output <output>]` — list workspaces
- `swayg workspace move <name> --groups <g1,g2>` — move to groups
- `swayg workspace global <name>` — set global
- `swayg workspace unglobal <name>` — unset global
- `swayg workspace groups <name>` — show groups
- `swayg container move <workspace> [--switch-to-workspace]` — move focused container; new workspaces auto-added to active group
- `swayg nav next [--output <output>] [--wrap]` — next workspace in group
- `swayg nav prev [--output <output>] [--wrap]` — prev workspace in group
- `swayg nav go <workspace>` — switch to workspace
- `swayg nav back` — go to last focused workspace
- `swayg repair` — full DB↔sway reconciliation
- `swayg sync [--workspaces] [--groups] [--outputs]` — sync from sway
- `swayg status` — show current status

## Live DB vs Test DB

51. **`swayg_live()` helper** exists in `common/mod.rs` — calls swayg WITHOUT `--db` flag, for live DB operations only. Used exclusively for:
    - Reading `orig_group` from live DB (before `fixture.init()`)
    - Final cleanup: restoring `orig_group` in live DB after test

52. **Tests must NOT operate on the live DB except at the very end for cleanup via `swayg_live()`.** All test-DB operations must use `fixture.swayg(...)`.

53. **`fixture.swayg(&["group", "select", &orig_group, ...])` is WRONG** — `orig_group` doesn't exist in the test DB. Use `fixture.swayg(&["group", "select", "0", ...])` instead.

54. **At the end of each test**, restore live state: `swayg_live(&["group", "select", &orig_group, "--output", ...])` + `swaymsg workspace <orig_ws>`.

## Daemon in Tests

55. **Production `swayg-daemon.service` MUST be stopped before tests** and restarted after. The daemon intercepts sway workspace events and adds test workspaces to the real DB, causing precondition failures.

56. **Test daemon** is started via `start_test_daemon()` in `common/mod.rs`. This spawns `swayg-daemon /tmp/swayg-integration-test.db /tmp/swayg-daemon-test.state`.

57. **Daemon startup order**: The test daemon MUST be started AFTER `fixture.init()`, because `init()` deletes and recreates the DB file, breaking any existing DB connection.

58. **Signal control**: `pause_test_daemon()` (SIGUSR1) blocks event processing, `resume_test_daemon()` (SIGUSR2) re-enables it. `daemon_state()` reads `/tmp/swayg-daemon-test.state`.

59. **Double pause check in daemon**: Flag checked before AND after `read_event()` to prevent race conditions.

## Sway Config Behavior

60. **`exec` lines in sway config are NOT re-executed on `swaymsg reload`.** Only `bindsym` definitions are re-loaded. Their `exec` commands only fire on key press.

61. **`swaymsg exec` does NOT accept `--no-startup-id`.** That's a sway-config-only directive. Use `swaymsg exec "sh -c '...'"` or just `swaymsg exec "command"`.

62. **`$mod+r` calls `~/.config/sway/swayg-reload.sh`** which does `swaymsg reload` then `swayg sync --init-bars`.

## Waybar Dynamic

63. **waybar-dynamic socket timing**: Socket file exists immediately but `connect()` returns ECONNREFUSED (error 111) for ~200ms after waybar starts. `WaybarClient::send_with_retry` catches ECONNREFUSED and socket-not-found.

64. **waybar-dynamic is fire-and-forget**: No response on the socket, no way to query state. WidgetSpec has 7 fields: id, label, classes, tooltip, on_click, on_right_click, on_middle_click.

65. **`swayg sync --init-bars`** uses retry logic (default 5x 200ms) for ECONNREFUSED errors. Configurable via `--init-bars-retries` and `--init-bars-delay-ms`.

## Cross-Output Testing

66. **Virtual outputs**: Use `swaymsg create_output HEADLESS-1` for cross-output tests. Sway auto-increments HEADLESS-N. Use `create_virtual_output()` helper.

67. **Virtual output cleanup**: `swaymsg output HEADLESS-N unplug` + `sleep` in `TestFixture::new()` for test isolation.

68. **`swaymsg focus output <name>`** is correct syntax (NOT `swaymsg output <name> focus`).

69. **Workspace "0" is global by name in sway**: Focus output first, remove "0" from other outputs before cross-output tests.

## Build & Test Commands

70. **`Command::cargo_bin("swayg")` uses the DEBUG build** — must `cargo build -p sway-groups-cli` (not `--release`) before tests.

71. **`assert_cmd` must be in `[dependencies]` not `[dev-dependencies]`** in sway-groups-tests Cargo.toml (used in common module).

72. **Test run**: `cargo test -p sway-groups-tests -- --test-threads=1`

73. **Install**: `cargo build --release && bash install.sh`

## Shell Test Equivalence

When writing or modifying Rust tests, they MUST match the corresponding shell test in `tests/test*.sh`:

41. **Same preconditions:** DB checks AND sway checks must match the shell test.
42. **Same assertions:** Every assertion in the shell test must have a Rust equivalent.
43. **Same cleanup ordering:** Kill windows before or after group switches should match the shell test unless there's a known sway behavior reason to differ.
44. **Same post-conditions:** Shell post-condition checks (groups, workspaces, workspace_groups) must all be present in the Rust test.
45. **Extra assertions in Rust are allowed** (e.g., `orig_group` non-empty check, init success check). Missing assertions from shell are NOT allowed.

## Helper Design

46. **`TestFixture`** — RAII guard:
    - Creates `/tmp/swayg-integration-test.db`, configures swayg env to use it
    - Saves original workspace + output
    - `Drop` switches back to original workspace
    - Fields: `db_path`, `orig_workspace`, `orig_output`

47. **`DummyWindowHandle`** — RAII wrapper:
    - Spawns `sway-dummy-window <app_id>` process
    - Waits until it appears in sway tree (up to 2s)
    - `Drop` kills the process (via PID)
    - Method: `spawn(app_id: &str) -> Result<Self>`
    - Method: `exists_in_tree() -> bool`

48. **`swayg()` helper** — shorthand for `Command::cargo_bin("swayg").unwrap().args(...)`:
    - Returns `assert_cmd::assert::Assert` for chaining
    - Always set `env("SWAYG_DB", &fixture.db_path)`

49. **`swayg_output()` helper** — captures stdout from a swayg CLI call:
    - Use for checking active group, workspace lists, status output
    - Returns `String`

50. **Sway state queries** — helper functions that call `swaymsg` and parse JSON:
    - `focused_workspace() -> String`
    - `workspace_exists(name) -> bool`
    - `window_in_tree(app_id) -> bool`
    - `workspace_of_window(app_id) -> Option<String>`
    - `workspaces_for_output(output) -> Vec<String>`

## Test Plan Format

When presenting a test plan to the user, use this format. Every command or action is followed by its assertions indented below:

```
`<command or action>`
  - assertion 1a
  - assertion 1b
`<command or action>`
  - assertion 2a
```

Commands are written as CLI invocations (e.g., `swayg group select <GROUP>`) or code actions (e.g., `TestFixture::new()`). Assertions describe what is checked after the command succeeds. Each command block must restore focus to the original workspace/output before the test ends.

Precondition checks (DB counts, sway state) are NOT separate commands — they are assertions indented under the last preceding command/action. Only actual commands or code actions (things that change state or execute something) appear at the top level.

Every command that changes state must have **full assertions** on the expected state changes — both DB and sway. `success` alone is NOT enough. Always verify what changed: group created (DB count), active group (CLI), workspace moved (sway), etc.
