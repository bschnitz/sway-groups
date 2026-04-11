# Plan: Shell-Tests → Rust Integration Tests

## Ziel

Alle 26 Shell-Tests (`tests/test*.sh`) als echte CLI-Integrationstests in Rust neu schreiben. Die Shell-Tests bleiben als Referenz bestehen.

## Crate: `sway-groups-tests`

### Dependencies (Cargo.toml)

```toml
[package]
name = "sway-groups-tests"
version.workspace = true
edition.workspace = true
publish = false

[lib]
name = "sway_groups_tests"
path = "src/lib.rs"

[dependencies]
tokio = { workspace = true }
anyhow = { workspace = true }
serde_json = { workspace = true }
rusqlite = { version = "0.31", features = ["bundled"] }

[dev-dependencies]
assert_cmd = "2.2"
predicates = "3.1"
sway-groups-cli = { path = "../sway-groups-cli" }
sway-groups-dummy-window = { path = "../sway-groups-dummy-window" }
tokio = { workspace = true }
serde_json = { workspace = true }
```

**Entfallen:**
- `sway-groups-core` — nicht mehr direkt genutzt (CLI-Tests!)
- `sea-orm` — DB-Assertions via `rusqlite`
- `chrono` — nicht mehr benötigt

**Neu:**
- `assert_cmd` = "2.2" — CLI-Test-Framework (`Command::cargo_bin()`)
- `predicates` = "3.1" — Komponierbare Assertions für stdout/stderr
- `sway-groups-cli` — damit `Command::cargo_bin("swayg")` funktioniert
- `rusqlite` = "0.31" (bundled) — SQLite-Client für DB-Assertions

### Warum `rusqlite` mit `bundled` Feature?

- Keine System-Dependency (`libsqlite3-dev`)
- Cross-Plattform konsistent
- Klein genug (embedded SQLite)

## sway-dummy-window

**Bereits erledigt.** Das Dummy-Window ist jetzt 200x100 Pixel, sichtbar (dunkelblauer Rechteck), nutzt sctk's `Shm`/`SlotPool` statt raw `wl_shm`. Keine neuen C-Dependencies nötig.

Text-Rendering ("Test Dummy") kommt später wenn `libfreetype6-dev` installiert ist (braucht `raqote` + `font-kit`).

## Test Infrastructure

### `src/common/mod.rs`

```rust
pub struct TestFixture {
    pub db_path: PathBuf,         // /tmp/swayg-integration-test.db
    pub orig_workspace: String,
    pub orig_output: String,
}

impl TestFixture {
    pub async fn new() -> Result<Self> { ... }
}

impl Drop for TestFixture {
    fn drop(&mut self) { /* switch back to orig workspace */ }
}

pub struct DummyWindowHandle { child: Child, app_id: String }

impl DummyWindowHandle {
    pub fn spawn(app_id: &str) -> Result<Self> { ... }
    pub fn exists_in_tree(&self) -> bool { ... }
}

impl Drop for DummyWindowHandle {
    fn drop(&mut self) { /* kill child process */ }
}

// Helper: swayg CLI shorthand
pub fn swayg(args: &[&str]) -> assert_cmd::assert::Assert { ... }

// DB assertion helpers
pub fn group_exists(db: &Connection, name: &str) -> bool { ... }
pub fn workspace_exists(db: &Connection, name: &str) -> bool { ... }
pub fn workspace_in_group(db: &Connection, ws: &str, group: &str) -> bool { ... }
pub fn active_group(db: &Connection, output: &str) -> String { ... }
pub fn no_test_data(db: &Connection) -> bool { ... }

// Sway state helpers
pub fn focused_workspace() -> String { ... }
pub fn workspace_of_window(app_id: &str) -> Option<String> { ... }
pub fn window_in_tree(app_id: &str) -> bool { ... }
```

### DB Path Konfiguration

`swayg` nutzt standardmäßig `~/.local/share/swayg/swayg.db`. Für Tests müssen wir:
1. Eine Test-DB bei `/tmp/swayg-integration-test.db` erstellen
2. `swayg` via Environment-Variable oder `--db` Flag auf die Test-DB zeigen

**Prüfen:** Unterstützt `swayg` bereits ein `--db` Flag oder `SWAYG_DB` Env-Var? Falls nein, muss das zuerst im CLI hinzugefügt werden.

### `swayg` DB-Konfiguration

Falls kein `--db` Flag existiert:
- `swayg init` mit `SWAYG_DB=/tmp/swayg-integration-test.db` aufrufen
- Das init erstellt die DB am angegebenen Pfad
- Alle weiteren `swayg` Aufrufe in dem Test müssen dieselbe Env-Var setzen

## Test-Migration: 26 Shell-Tests

| # | Shell-Test | Rust-Test | Assertions |
|---|-----------|-----------|------------|
| 01 | test01_group_select.sh | test_01_group_select | ~9 |
| 02 | test02_new_workspace_with_containers_and_workspaces.sh | test_02_workspace_with_containers | ~25 |
| 03 | test03_global_workspace.sh | test_03_global_workspace | ~30 |
| 04 | test04_workspace_move.sh | test_04_workspace_move | ~19 |
| 05a | test05a_multi_group_workspace_add.sh | test_05a_multi_group_workspace_add | ~26 |
| 05b | test05b_multi_group_container_move.sh | test_05b_multi_group_container_move | ~32 |
| 05c | test05c_multi_group_workspace_rename_merge.sh | test_05c_multi_group_workspace_rename_merge | ~44 |
| 05d | test05d_multi_group_global.sh | test_05d_multi_group_global | ~22 |
| 05e | test05e_multi_group_unglobal.sh | test_05e_multi_group_unglobal | ~29 |
| 05f | test05f_multi_group_workspace_remove.sh | test_05f_multi_group_workspace_remove | ~28 |
| 05g | test05g_multi_group_auto_delete.sh | test_05g_multi_group_auto_delete | ~27 |
| 06a | test06a_group_delete_multi_group_workspace.sh | test_06a_group_delete_orphan | ~30 |
| 06b | test06b_workspace_move_to_groups.sh | test_06b_workspace_move_to_groups | ~22 |
| 07 | test07_group_rename.sh | test_07_group_rename | ~29 |
| 08 | test08_nav_next_prev.sh | test_08_nav_next_prev | ~37 |
| 09 | test09_nav_go_back.sh | test_09_nav_go_back | ~25 |
| 10 | test10_workspace_rename_simple.sh | test_10_workspace_rename_simple | ~23 |
| 11 | test11_workspace_groups.sh | test_11_workspace_groups | ~20 |
| 12 | test12_repair.sh | test_12_repair | ~22 |
| 13 | test13_group_next_prev.sh | test_13_group_next_prev | ~37 |
| 14 | test14_workspace_list_output_format.sh | test_14_workspace_list_format | ~33 |
| 15 | test15_sync_flags.sh | test_15_sync_flags | ~26 |
| 16 | test16_group_prune.sh | test_16_group_prune | ~31 |
| 17 | test17_status.sh | test_17_status | ~15 |
| 18 | test18_group_create.sh | test_18_group_create | ~13 |
| 19 | test19_nav_move_to.sh | test_19_nav_move_to | ~16 |
| 20 | run_all.sh | (Runner-Script, wird nicht portiert) | — |

## Offene Fragen

1. **`swayg` DB-Pfad konfigurierbar?** Muss ein `--db` Flag oder `SWAYG_DB` Env-Var hinzugefügt werden, damit Tests eine isolierte DB nutzen können. Ohne das können Tests nicht isoliert von der Produktions-DB laufen.

2. **shell-tests behalten?** Die Shell-Tests in `tests/` bleiben bestehen. Die neuen Rust-Tests leben in `sway-groups-tests/tests/`. Soll `tests/run_all.sh` auch die Rust-Tests aufrufen?

3. **Test-Runner:** Sollen wir einen eigenen Runner schreiben oder reicht `cargo test -p sway-groups-tests -- --test-threads=1`?

## Ausführungsreihenfolge

1. ~~sway-dummy-window aufrüsten~~ ✅
2. Prüfen ob `swayg` `--db` Flag oder `SWAYG_DB` Env-Var unterstützt
3. Falls nein: `--db` Flag zu swayg hinzufügen
4. `sway-groups-tests/Cargo.toml` aktualisieren (Dependencies)
5. `src/common/mod.rs` neu schreiben (TestFixture, DummyWindowHandle, Helpers)
6. Bestehende Rust-Tests löschen (19 Dateien unter `sway-groups-tests/tests/`)
7. Test 01 als PoC neu schreiben
8. Test 01 ausführen und validieren
9. Restliche Tests (02-19) nacheinander schreiben
10. `run_all.sh` aktualisieren
