# Writing tests for `sway-groups-tests`

These are the rules for the integration suite: what a test may assume, what the
harness already does for it, and which sway behaviour trips tests up. Read this
before adding or changing a test.

The suite drives the `swayg` CLI end to end. A test never calls `GroupService`,
`WorkspaceService` or any other internal API — if a behaviour cannot be reached
through the binary, it belongs in a unit test next to the code instead.

## How a test is isolated

One file is one test is one process, and every process gets a sway of its own:
a headless compositor (`WLR_BACKENDS=headless`) started by
`common::sway_instance`, with nothing on it but sway's workspace `1`.

Everything the test could otherwise share with the developer's session is
private to that process, and all of it lives in one directory that is deleted
when the compositor stops:

| Variable          | Points at                                                                 | Why it matters                                                                                                            |
| ----------------- | ------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| `XDG_RUNTIME_DIR` | `<runtime>/swayg-test-<pid>`                                              | Where the waybar sockets are looked up — this is what keeps a run out of the developer's bar                              |
| `SWAYSOCK`        | `<instance dir>/sway.sock`                                                | Every `swaymsg` in the test talks to the private sway                                                                     |
| `WAYLAND_DISPLAY` | the display name sway reports, whose socket sits in the runtime dir above | Dummy windows land in the private sway                                                                                    |
| `SWAYG_CONFIG`    | `<instance dir>/config.toml`                                              | Deliberately absent, so the binaries use their built-in defaults instead of the developer's `~/.config/swayg/config.toml` |
| `--db`            | `<instance dir>/swayg-test.db`                                            | Passed explicitly by every helper; the production DB is never opened                                                      |

Two things follow from this, and they are the difference to how the suite used
to be written:

- **Nothing has to be restored.** Whatever a test does to workspaces, groups or
  focus dies with its compositor. Do not save and re-select the original group,
  do not `swaymsg workspace` back at the end, and do not assert that focus
  returned — an assertion that only re-checks the test's own cleanup proves
  nothing.
- **Preconditions are about the test's own database**, which the fixture has
  just created. There is no production DB to guard against and no production
  daemon to stop.

`test_38_bar_isolation` and `test_39_config_isolation` guard the isolation
itself. If either starts failing, the suite has begun reaching into the
developer's session again — fix that before anything else.

## The shape of a test

```rust
use sway_groups_tests::common::{TestFixture, db_count, swayg_output};

const TEST_GROUP: &str = "zz_test_group_select";

#[tokio::test]
async fn test_01_group_select() {
    // Starts the compositor, seeds the DB, spawns the anchor window.
    let fixture = TestFixture::new().await.expect("fixture setup");

    // Precondition, against the fixture's own database.
    fixture.init().success();
    assert_eq!(db_count(&fixture.db_path, "SELECT count(*) FROM groups WHERE name = 'zz_test_group_select'"), 0);

    // Action.
    fixture
        .swayg(&["group", "select", TEST_GROUP, "--output", &fixture.orig_output, "--create"])
        .success();

    // Assertions: the database and sway, not just the exit status.
    assert_eq!(db_count(&fixture.db_path, "SELECT count(*) FROM groups WHERE name = 'zz_test_group_select'"), 1);
    assert_eq!(
        swayg_output(&fixture.db_path, &["group", "active", &fixture.orig_output]),
        TEST_GROUP,
    );

    // Post-condition: no test data left in the test DB.
}
```

Conventions:

- **File and function share the name**: `test_<nn>_<description>.rs` contains
  `async fn test_<nn>_<description>()`. The fixture derives the badge label from
  the binary name, so a mismatch shows up on the bar.
- **One test per file.** The compositor is per process; a second test in the
  same file would share it.
- **Prefix every group and workspace a test creates with `zz_test_`** so a stray
  entity is recognisable at a glance.
- **`success()` is never an assertion on its own.** Every command that changes
  state gets assertions on what changed — the DB row, the active group via the
  CLI, the workspace in sway.
- **After spawning a dummy window, assert it is in the tree**; after dropping
  one, assert it is gone. A spawned process is not a mapped window.
- **Exercise auto-delete explicitly** where it is the subject: select the empty
  group, switch away, then assert it is gone. Do not let the post-condition's
  `init()` sync hide it.
- **Every test ends with a post-condition** that no test entity remains:
  `groups`, `workspaces` and `workspace_groups`.
- **Read the active group through the CLI** (`swayg group active <output>`), not
  with `SELECT active_group FROM outputs`.
- **DB assertions go through the `sqlite3` CLI**, via the helpers below.
  `rusqlite` conflicts with the `libsqlite3-sys` that `sea-orm` pulls in.

## What the harness gives you

`TestFixture` (RAII, `common/mod.rs`):

- Fields: `db_path`, `orig_workspace`, `orig_output`, `sway`.
- `TestFixture::new()` starts sway, deletes a stale test DB, spawns the anchor
  window that keeps the starting workspace from evaporating, then runs `init`,
  `repair` and `group select 0 --create` so the test starts from the state a
  used session is in.
- `TestFixture::with_sway_config(extra)` does the same with extra lines appended
  to the compositor's config, for tests that need sway itself to react to a rule.
- `fixture.swayg(&[...])` runs the CLI against the test DB and returns an
  `assert_cmd` `Assert`; `fixture.init()` is the shorthand for `init`.
- `Drop` stops the test daemon, updates the progress badge and shuts the
  compositor down.

`DummyWindowHandle::spawn(app_id)` starts `sway-dummy-window`, waits until the
window is in the tree (up to 2 s) and kills it on drop. Use it instead of a real
terminal.

Helpers, all in `common`:

| Group      | Functions                                                                                                                                                                                     |
| ---------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Database   | `db_query`, `db_count`, `db_exec`, `ws_in_group_count`                                                                                                                                        |
| CLI        | `swayg`, `swayg_output`, `swayg_stderr`, `orig_active_group`, `output_contains`, `line_starts_with`                                                                                           |
| Sway state | `workspace_exists_in_sway`, `workspace_count_in_sway`, `window_count_in_tree`, `workspace_of_window`, `con_id_of_window`, `get_focused_workspace`, `get_focused_output`, `get_primary_output` |
| Outputs    | `create_virtual_output`, `unplug_output`                                                                                                                                                      |
| Daemon     | `start_test_daemon`, `start_test_daemon_with_config`, `pause_test_daemon`, `resume_test_daemon`, `stop_test_daemon`, `daemon_state`                                                           |

The three binaries under test are built by the harness itself
(`common/binaries.rs`), which takes their paths from cargo's JSON output. A test
therefore cannot run against a stale `target/debug/swayg`, and it must not
construct binary paths of its own.

`assert_cmd` is a normal dependency, not a dev-dependency, because the common
module uses it.

## The daemon in tests

- `start_test_daemon()` spawns `swayg-daemon` against the fixture's DB and a
  state file in the instance directory. Call it after the fixture exists — and
  after any explicit `fixture.init()`, because `init` recreates the DB file and
  breaks an open connection.
- `start_test_daemon_with_config(path)` does the same with `SWAYG_CONFIG`
  pointing at a config of the test's own, which overrides the fixture's default.
- `pause_test_daemon()` (SIGUSR1) and `resume_test_daemon()` (SIGUSR2) gate
  event processing; the daemon checks the flag before and after `read_event()`
  so a pause cannot be raced. `daemon_state()` reads the state file.
- `TestFixture::drop()` stops the test daemon, so a test that panics leaves no
  daemon behind.

## sway behaviour worth knowing

- `move container to workspace` creates the workspace in sway; it does not need
  to exist first.
- An empty workspace is destroyed when it loses focus, not when it becomes
  empty. To make sway drop one, switch away and give it ~100 ms.
- A focused workspace survives even when its last window dies, which is why the
  fixture pins its starting workspace with an anchor window.
- Auto-delete only inspects the group that was active _before_ a `group select`.
  Emptying several groups takes one select-and-leave per group.
- Selecting an empty group adds the default workspace (`default_workspace`, `0`)
  to it, which also keeps `group prune` from removing it.
- Workspace `0` is an ordinary group name: it can be renamed, pruned and
  auto-deleted, so `group select 0` still needs `--create`.
- An empty workspace reports `representation: null` in `get_workspaces`.
- Cross-output tests use `create_virtual_output()` and `unplug_output()`. The
  helper asks sway for `HEADLESS-1` and returns whichever name sway actually
  created, because sway numbers the outputs itself. Focus an output with
  `swaymsg focus output <name>`, not `swaymsg output <name> focus`.
- waybar's dynamic socket exists before it accepts connections: `connect()`
  returns `ECONNREFUSED` for roughly 200 ms after waybar starts, which is what
  `sync --init-bars` retries (`--init-bars-retries`, `--init-bars-delay-ms`).
  Pushing a widget is fire-and-forget — nothing can be read back.

## The `swayg` CLI

`swayg <command> --help` is the reference; do not keep a copy of the command
list here, it only rots. Two flags matter for tests, and both are also
environment variables: `--db` (`SWAYG_DB`) and `--config` (`SWAYG_CONFIG`). The
helpers always pass `--db`, and the fixture sets `SWAYG_CONFIG` for the process.

A test must not write into `~/.config/swayg/`. A test that needs a config writes
it into the instance directory and points the binary at it.

## Running the suite

```sh
cargo test -p sway-groups-tests                  # every test, its own compositor each
cargo test -p sway-groups-tests --no-fail-fast   # keep going past the first failing target
cargo test --test test_12_repair                 # one test
```

No `--test-threads=1`: the sessions are private, so nothing is shared to
serialise. While a run is going, the fixture writes
`/tmp/swayg-test-progress.json` for the waybar badge; the module ignores the
file once it stops being touched, so the badge clears itself however the run
ends. `reset_test_counter()` clears the counter by hand if it is ever needed.

## Presenting a test plan

When proposing a test to the user, write each command or action at the top
level and its assertions indented under it:

```
`<command or action>`
  - assertion 1a
  - assertion 1b
`<command or action>`
  - assertion 2a
```

Preconditions are not commands — they are assertions under the action that
precedes them. Only things that execute or change state appear at the top
level, and every one of them carries full assertions on the resulting state in
both the database and sway.
