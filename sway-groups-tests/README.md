# sway-groups-tests

Integration tests for `sway-groups`. The tests run against a **live Sway
session** — they manipulate real workspaces and groups via IPC and verify both
Sway state and the database.

---

## Prerequisites

- A running Sway session (`SWAYSOCK` must be set)
- The workspace where you run the tests must be focused and ideally free of
  important work — the tests switch groups and workspaces during their run
- The `sway-dummy-window` binary must be buildable (it gets built automatically
  by `cargo test`)

---

## Running the tests

### All integration tests

```sh
cargo test -p sway-groups-tests -- --test-threads=1
```

`--test-threads=1` is **mandatory**. Sway state is global — running tests
concurrently would cause them to interfere with each other.

### A single test file

```sh
cargo test -p sway-groups-tests --test test01_group_select -- --test-threads=1
```

### With output (recommended while developing)

```sh
cargo test -p sway-groups-tests -- --test-threads=1 --nocapture
```

### Build the dummy-window binary first (optional, cargo test does this automatically)

```sh
cargo build -p sway-groups-dummy-window
```

---

## What the tests do NOT do

- They do **not** touch the production database at
  `~/.local/share/swayg/swayg.db`. Every test creates a fresh isolated
  database at `/tmp/swayg-integration-test.db`.
- They do **not** require a headless Sway instance. Tests run against your
  real session. The fixture restores your original workspace even when a test
  panics.
- They do **not** invoke the `swayg` CLI binary. Tests call the Rust service
  layer directly (`GroupService`, `WorkspaceService`, etc.), which is faster
  and gives cleaner failure messages.

---

## Test database

Each test creates a fresh SQLite database at:

```
/tmp/swayg-integration-test.db
```

The file is removed at the start of each `SwayTestFixture::new()` call, so
every test starts from a clean state. If a test fails, the file is left on disk
for post-mortem inspection with `sqlite3`.

---

## Test structure

### File layout

```
sway-groups-tests/
├── src/
│   ├── lib.rs               # re-exports common module
│   └── common/
│       └── mod.rs           # SwayTestFixture, DummyWindowHandle, assertions
└── tests/
    ├── test01_group_select.rs
    ├── test02_*.rs
    └── ...
```

Each file in `tests/` is a separate test binary. Every new test file must be
declared in `Cargo.toml`:

```toml
[[test]]
name = "test02_my_feature"
path = "tests/test02_my_feature.rs"
harness = true
```

### Anatomy of a test

```rust
use sway_groups_tests::common::{
    assert_active_group, assert_focused_workspace, assert_group_exists,
    assert_group_not_exists, assert_no_test_data, assert_workspace_in_group,
    DummyWindowHandle, SwayTestFixture, TEST_PREFIX,
};

// All test names start with the prefix so they sort after real groups/workspaces.
const TEST_GROUP: &str = "zz_test_my_feature";
const TEST_WS: &str    = "zz_test_my_ws";

#[tokio::test]
async fn test_my_feature() {
    // 1. Setup — creates fresh DB, syncs from Sway, remembers current state.
    let fixture = SwayTestFixture::new()
        .await
        .expect("Failed to set up test fixture");

    let output   = fixture.orig_output.clone();
    let orig_ws  = fixture.orig_workspace.clone();

    // 2. Preconditions
    assert_group_not_exists(&fixture.db, TEST_GROUP).await;

    // 3. Actions via service layer
    fixture.group_service.get_or_create_group(TEST_GROUP).await.unwrap();
    fixture.group_service.set_active_group(&output, TEST_GROUP).await.unwrap();

    // 4. Optionally spawn a window on a workspace
    let _win = DummyWindowHandle::spawn(&fixture, TEST_WS)
        .expect("Failed to spawn dummy window");
    // _win is dropped (killed) at end of scope automatically.

    // 5. Assertions
    assert_group_exists(&fixture.db, TEST_GROUP).await;
    assert_active_group(&fixture, &output, TEST_GROUP).await;

    // 6. Cleanup — switch back to original group
    fixture.group_service.set_active_group(&output, "0").await.unwrap();

    // 7. Post-conditions — no test data remains
    assert_focused_workspace(&fixture, &orig_ws);
    assert_no_test_data(&fixture.db).await;

    // SwayTestFixture::drop() switches back to orig_ws automatically if the
    // test panics before step 6.
}
```

---

## Common module reference

### `SwayTestFixture`

The central RAII guard. Constructed with `SwayTestFixture::new().await`.

| Field / Method | Description |
|---|---|
| `ipc: SwayIpcClient` | Direct access to Sway IPC |
| `db: DatabaseManager` | Connection to the test DB |
| `orig_workspace: String` | Workspace focused when the test started |
| `orig_output: String` | Output focused when the test started |
| `group_service: GroupService` | Group operations |
| `workspace_service: WorkspaceService` | Workspace operations |
| `waybar_sync: WaybarSyncService` | Waybar sync (usually not needed in tests) |
| `active_group().await` | Active group on `orig_output` |
| `focus_workspace(name)` | Switch Sway focus to a workspace |
| `focused_workspace()` | Name of the currently focused workspace |
| `wait_until(timeout, condition)` | Poll until condition is true or timeout |

`Drop` switches Sway back to `orig_workspace` automatically, even on panic.

### `DummyWindowHandle`

A lightweight Wayland window (`sway-dummy-window` binary) with a configurable
`app_id`. Sway sees it as a real window on a workspace. The process is killed
in `Drop`.

```rust
// Spawn a window with app_id "zz_test_myapp", wait until Sway sees it.
let win = DummyWindowHandle::spawn(&fixture, "zz_test_myapp")?;

// Check it is still in the tree.
assert!(win.exists_in_tree(&fixture));

// Drop kills the process.
drop(win);
```

Use `DummyWindowHandle` whenever a test needs containers on a workspace —
for example to verify that switching groups doesn't destroy windows, or that
`container move` works correctly.

### Assertion helpers

All assertions panic with a descriptive message on failure.

| Function | Description |
|---|---|
| `assert_group_exists(db, name).await` | Group exists in test DB |
| `assert_group_not_exists(db, name).await` | Group absent from test DB |
| `assert_active_group(fixture, output, expected).await` | Active group on output |
| `assert_focused_workspace(fixture, expected)` | Sway focused workspace |
| `assert_workspace_exists(db, name).await` | Workspace exists in test DB |
| `assert_workspace_not_exists(db, name).await` | Workspace absent from test DB |
| `assert_workspace_in_group(db, workspace, group).await` | Workspace is member of group |
| `assert_no_test_data(db).await` | No `zz_test_` prefixed rows in DB |

### Constants

| Constant | Value | Purpose |
|---|---|---|
| `TEST_DB_PATH` | `/tmp/swayg-integration-test.db` | Isolated test database |
| `TEST_PREFIX` | `zz_test_` | Prefix for all test group/workspace names |

---

## Writing a new test

### 1. Create the test file

```
sway-groups-tests/tests/test19_my_feature.rs
```

Name it after the next free number and a short description of what it tests.

### 2. Register it in `Cargo.toml`

```toml
[[test]]
name = "test19_my_feature"
path = "tests/test19_my_feature.rs"
harness = true
```

### 3. Follow these rules

**Naming:** All test groups and workspaces must use the `zz_test_` prefix.
This keeps them sorted after real names in Sway and makes `assert_no_test_data`
work correctly.

**Isolation:** Never hardcode an output name. Always use
`fixture.orig_output.clone()`. The user may run tests on any output.

**Cleanup:** Every test must end with `assert_no_test_data(&fixture.db).await`.
If test workspaces existed in Sway during the test (via `DummyWindowHandle`),
all handles must be dropped before this assertion — dropping a handle kills the
process, which causes Sway to remove the workspace.

**Timing:** After killing a dummy window or switching groups, Sway needs a
brief moment to update its state. Use `fixture.wait_until(...)` instead of
`std::thread::sleep` where possible — it polls and returns as soon as the
condition is met, rather than sleeping a fixed amount.

```rust
// Good — returns as soon as the window disappears, up to 2 seconds.
fixture.wait_until(Duration::from_secs(2), || {
    !win.exists_in_tree(&fixture)
}).expect("Window did not disappear");

// Acceptable for Sway settle after group switch.
std::thread::sleep(Duration::from_millis(100));
```

**Preconditions:** Because `SwayTestFixture::new()` always starts from a fresh
DB, most preconditions are implicit. Only add explicit precondition assertions
when checking Sway state (e.g., that a test workspace does not already exist in
Sway from a previous crashed run).

**Error propagation:** Use `.expect("descriptive message")` rather than `?` in
tests — panic messages are more readable in test output than propagated errors.

**One logical scenario per test function:** If a test covers multiple
sub-scenarios, split them into separate `#[tokio::test]` functions within the
same file. Each function gets its own `SwayTestFixture`.

---

## Adding assertions

If you need an assertion that does not exist yet, add it to
`src/common/mod.rs`. Follow the existing pattern:

- `async` functions take `&DatabaseManager` for DB checks
- sync functions take `&SwayTestFixture` for Sway IPC checks
- Always `panic!` with a message that includes both expected and actual values

---

## Troubleshooting

**`SWAYSOCK not set`** — The test process does not inherit `SWAYSOCK`. Run
`cargo test` from within a Sway session, not over SSH without forwarding.

**Test leaves wrong workspace focused** — The `Drop` implementation on
`SwayTestFixture` should have restored it. If it did not, a panic happened
before the fixture was constructed. Check that `SwayTestFixture::new()` did not
itself panic.

**`/tmp/swayg-integration-test.db` is stale** — Delete it manually with
`rm /tmp/swayg-integration-test.db`. The next test run will recreate it.

**`sway-dummy-window` not found** — Run `cargo build -p sway-groups-dummy-window`
first, or let `cargo test` build the whole workspace.

**Timing failures** — If assertions about Sway state fail intermittently,
increase the `wait_until` timeout or add a short `sleep` after the action.
Sway processes IPC commands asynchronously and workspace deletion can be
slightly delayed.
