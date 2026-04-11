# AI Test Instructions — sway-groups-tests

Rules for writing Rust integration tests using `assert_cmd` + `DummyWindowHandle`.
Based on `tests/test-instructions.md`, adapted for Rust.

## General Rules

1. **Tests exercise the `swayg` CLI binary, not internal services.** Every test action is a `Command::cargo_bin("swayg")` call. Never call `GroupService`, `WorkspaceService`, etc. directly. This is an integration test, not a unit test.

2. **Use `Command::cargo_bin("swayg")` for all swayg invocations.** Never call internal Rust APIs. The whole point is testing the CLI end-to-end.

3. **Use `DummyWindowHandle` instead of kitty.** Spawn dummy windows via `sway-dummy-window` binary (200x100 pixel, visible in sway). Faster, no external dependencies.

4. **DB assertions via `rusqlite`, not `sea-orm`.** Read the test DB directly with SQLite queries, not through entity abstractions.

5. **Test DB path: `/tmp/swayg-integration-test.db`.** Never touch the production DB. Configure swayg to use this DB via environment variable or config before each test.

6. **Check preconditions BEFORE any swayg commands.** Verify test groups/workspaces don't exist in DB before running setup.

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
    
    // 2. Precondition checks (sqlite3 queries)
    assert!(!group_exists(&fixture.db, "zz_test_x"));
    
    // 3. Setup (swayg CLI calls)
    swayg(&["init"]).success();
    swayg(&["group", "select", &fixture.output, "zz_test_x", "--create"]).success();
    
    // 4. Spawn dummy windows if needed
    let win = fixture.spawn_window("zz_test_win1").await;
    
    // 5. Test actions (swayg CLI calls)
    swayg(&["container", "move", "zz_target", "--switch-to-workspace"]).success();
    
    // 6. Assertions (DB + sway state)
    assert_workspace_exists(&fixture.db, "zz_target");
    assert_focused_workspace(&fixture, "zz_target");
    
    // 7. Cleanup
    drop(win);  // kills dummy window
    
    // 8. Post-condition
    assert_no_test_data(&fixture.db);
}
```

## Sway Behavior

21. **`move container to workspace` creates the workspace in sway.** No need to pre-create it.

22. **Sway doesn't delete empty workspaces immediately.** After `move container`, the old workspace persists until a focus switch. Use `sleep(100ms)` + focus switch before checking.

23. **Workspace "0" is temporary.** Sway creates it when focusing an empty group, deletes it on switch away.

24. **Empty workspace detection:** `representation` is `null` in `get_workspaces()` output.

## DB Queries

25. **Group exists:** `SELECT count(*) FROM groups WHERE name = ?` → assert 1
26. **Workspace exists:** `SELECT count(*) FROM workspaces WHERE name = ?` → assert 1
27. **Workspace in group:** JOIN `workspace_groups`, `groups`, `workspaces` on names
28. **Active group:** `SELECT active_group FROM outputs WHERE name = ?`
29. **No test data:** No groups/workspaces with `zz_test_` prefix in their name

## swayg Command Reference

These commands are used in tests (check latest syntax in CLI):

- `swayg init` — drop + recreate DB, sync from sway
- `swayg group select <output> <group> --create` — create + switch to group
- `swayg group create <name>` — create group (error if exists)
- `swayg group delete <name> --force` — delete group
- `swayg group rename <old> <new>` — rename group
- `swayg group next --output <output> [--wrap]` — next group
- `swayg group prev --output <output> [--wrap]` — prev group
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
- `swayg container move <workspace> [--switch-to-workspace]` — move focused container
- `swayg nav next [--output <output>] [--wrap]` — next workspace in group
- `swayg nav prev [--output <output>] [--wrap]` — prev workspace in group
- `swayg nav go <workspace>` — switch to workspace
- `swayg nav move-to <workspace>` — move focused container + add to group
- `swayg nav back` — go to last focused workspace
- `swayg repair` — full DB↔sway reconciliation
- `swayg sync [--workspaces] [--groups] [--outputs]` — sync from sway
- `swayg status` — show current status

## Post-conditions

30. **Every test MUST have a post-condition** verifying no test data remains. Check groups, workspaces, workspace_groups for `zz_test_` prefix.

31. **If test workspaces existed in sway** during the test (dummy windows were spawned), kill them first, then run `swayg init` before post-condition to clean stale rows.

## Helper Design

32. **`TestFixture`** — RAII guard:
    - Creates `/tmp/swayg-integration-test.db`, configures swayg env to use it
    - Saves original workspace + output
    - `Drop` switches back to original workspace
    - Fields: `db_path`, `orig_workspace`, `orig_output`

33. **`DummyWindowHandle`** — RAII wrapper:
    - Spawns `sway-dummy-window <app_id>` process
    - Waits until it appears in sway tree (up to 2s)
    - `Drop` kills the process (via PID)
    - Method: `spawn(app_id: &str) -> Result<Self>`
    - Method: `exists_in_tree() -> bool`

34. **`swayg()` helper** — shorthand for `Command::cargo_bin("swayg").unwrap().args(...)`:
    - Returns `assert_cmd::assert::Assert` for chaining
    - Always set `env("SWAYG_DB", &fixture.db_path)`

35. **Sway state queries** — helper functions that call `swaymsg` and parse JSON:
    - `focused_workspace() -> String`
    - `workspace_exists(name) -> bool`
    - `window_in_tree(app_id) -> bool`
    - `workspace_of_window(app_id) -> Option<String>`
    - `workspaces_for_output(output) -> Vec<String>`
