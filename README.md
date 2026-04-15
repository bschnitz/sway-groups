# sway-groups (`swayg`)

Group-aware workspace management for [sway](https://swaywm.org/), with
optional [waybar](https://github.com/Alexays/Waybar) integration via
[waybar-dynamic](https://github.com/AriaSeitia/waybar-dynamic).

Workspaces are organised into named **groups**. Each output has an **active
group**, and only workspaces that belong to the active group (plus globals
and user-unhidden ones) are shown to waybar and included in group-aware
navigation. Workspace state is persisted in a small SQLite DB so switching
back to a group restores its last focus.

## Key concepts

- **Workspace** — a sway workspace (`1`, `2`, `3:Firefox`, …).
- **Group** — a named collection of workspaces. Each output has one *active*
  group at a time.
- **Global workspace** — visible in all groups (e.g. a persistent notes
  workspace).
- **Hidden workspace** — a workspace marked as hidden in a specific group.
  By default hidden workspaces are invisible to waybar and skipped by
  navigation, so you can declutter the bar during presentations or deep
  work. Toggle `show_hidden_workspaces` to reveal them with a `.hidden`
  CSS class applied (combinable with `.global`, `.focused`, …).

## Requirements

- Rust toolchain (stable, edition 2024)
- sway
- (Optional, for bar integration) waybar + [waybar-dynamic](https://github.com/AriaSeitia/waybar-dynamic)

## Installation

### `cargo install --git` (recommended right now)

No crates.io publishing needed:

```sh
cargo install --git https://github.com/bschnitz/sway-groups swayg
cargo install --git https://github.com/bschnitz/sway-groups swayg-daemon
```

Both binaries land in `~/.cargo/bin/`. Make sure that's in your `PATH`.

### `cargo install --path` (from a local clone)

```sh
git clone https://github.com/bschnitz/sway-groups
cd sway-groups
cargo install --path sway-groups-cli
cargo install --path sway-groups-daemon
```

### Later: `cargo install` from crates.io

Once the crates are published (in order: `sway-groups-config` →
`sway-groups-core` → `sway-groups-cli` / `sway-groups-daemon`) it becomes:

```sh
cargo install sway-groups-cli
cargo install sway-groups-daemon
```

### systemd user unit for the daemon

`cargo install` cannot install non-binary files. Copy the unit once:

```sh
mkdir -p ~/.config/systemd/user
cp swayg-daemon.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now swayg-daemon.service
```

The unit is `WantedBy=graphical-session.target`. For sway users, make sure
the target actually gets activated — the common recipe is to add a small
session target and start it from the sway config. Create
`~/.config/systemd/user/sway-session.target`:

```
[Unit]
Description=sway compositor session
BindsTo=graphical-session.target
```

…and in your sway `config`:

```
exec systemctl --user --no-block start sway-session.target
```

### waybar-dynamic integration (optional)

Install [waybar-dynamic](https://github.com/AriaSeitia/waybar-dynamic),
then add two modules to your waybar config:

```jsonc
"cffi/swayg_groups": {
    "module_path": "/path/to/libwaybar_dynamic.so",
    "name": "swayg_groups"
},
"cffi/swayg_workspaces": {
    "module_path": "/path/to/libwaybar_dynamic.so",
    "name": "swayg_workspaces"
}
```

`swayg` pushes widget updates to these modules automatically after every
state-changing command. For per-state CSS classes see [Bar styling](#bar-styling)
below.

## First-time setup

```sh
swayg init             # creates the DB and imports current sway state
```

This seeds the DB from sway's current workspaces, creates the default
group (`0`), and pushes initial bar widgets.

## CLI overview

Every command is documented under `--help`:

```sh
swayg --help
swayg workspace --help
swayg workspace hide --help
```

High-level tour:

```sh
# Groups
swayg group create dev
swayg group select dev               # make dev the active group on current output
swayg group next -w                  # next group (alphabetical, wrap)
swayg group prune                    # delete empty groups

# Workspace membership
swayg workspace add 3 -g dev         # add workspace "3" to dev
swayg workspace move 3 -g dev,work   # set exactly these groups
swayg workspace global 1             # workspace 1 visible in all groups
swayg workspace rename old new       # rename (merges if target exists)

# Hiding
swayg workspace hide                 # hide currently focused workspace in active group
swayg workspace hide 4 -g dev -t     # toggle "4" hidden in group dev
swayg workspace unhide 4 -g dev
swayg group unhide-all               # unhide everything in active group
swayg workspace show-hidden -t       # toggle the global show_hidden flag

# Navigation (group-aware — skips hidden unless show_hidden=true)
swayg nav next -w                    # next visible workspace, wrap
swayg nav go 3                       # focus workspace 3 (works even if hidden)
swayg nav back                       # previous focus

# Container moves
swayg container move 3 --switch-to-workspace

# State
swayg status
swayg sync --all --repair
swayg config dump                    # print the default config TOML

# Global flags
swayg -v ...                         # verbose
swayg --db /tmp/test.db ...          # alternate DB file
swayg --config ~/my.toml ...         # alternate config file
```

`swayg status` sample:

```
show_hidden_workspaces = false
eDP-1: active group = "dev"
  Visible:  1, 3
  Inactive: 2, 4
  Hidden:   5
  Global:   0
```

- **Visible** — in the active group (plus globals) and not user-hidden
- **Inactive** — belongs to other groups; exists in sway on this output
- **Hidden** — user-hidden in the active group (only shown if
  `show_hidden_workspaces = true`)
- **Global** — `is_global = true` workspaces

## Configuration

`swayg config dump` prints the default TOML. Save to
`~/.config/swayg/config.toml` (or any path passed via `--config` or
`SWAYG_CONFIG=`) and edit.

Current sections:

- `[defaults]` — `default_group`, `default_workspace` (used when orphan
  workspaces need a home, e.g. after `group delete --force`)
- `[bar.workspaces]` / `[bar.groups]` — per-bar tuning: socket instance
  name, display mode (`all` | `active` | `none`), `show_global`,
  `show_empty`

Runtime DB flags (separate from the config file):

- `show_hidden_workspaces` — toggled via `swayg workspace show-hidden`

## Bar styling

Widgets emitted by `swayg` carry CSS classes you can style. For a
waybar-dynamic module named `swayg_workspaces`:

```css
#waybar-dynamic.swayg_workspaces label { /* base */ }
#waybar-dynamic.swayg_workspaces label.focused  { /* this output's focused ws */ }
#waybar-dynamic.swayg_workspaces label.visible  { /* visible on another output */ }
#waybar-dynamic.swayg_workspaces label.urgent   { /* urgency hint from sway */ }
#waybar-dynamic.swayg_workspaces label.global   { /* is_global */ }
#waybar-dynamic.swayg_workspaces label.hidden   { /* only when show_hidden=true */ }

/* Classes combine: .focused.global, .hidden.global.focused, etc. */
```

For the groups bar (`swayg_groups`):

```css
#waybar-dynamic.swayg_groups label        { /* inactive */ }
#waybar-dynamic.swayg_groups label.active { /* active on focused output */ }
#waybar-dynamic.swayg_groups label.urgent { /* a workspace in the group is urgent */ }
```

## Storage locations

- SQLite DB: `~/.local/share/swayg/swayg.db`
- Log files: `~/.local/share/swayg/swayg.YYYY-MM-DD` (daily rotation)
- Config (optional): `~/.config/swayg/config.toml`
- Daemon state: `/tmp/swayg-daemon-test.state` (test daemon only)

Reset all state:

```sh
rm ~/.local/share/swayg/swayg.db
swayg init
```

## Architecture

Workspace crates:

| Crate | Role |
|---|---|
| `sway-groups-config` | TOML config schema + loader |
| `sway-groups-core` | DB entities, services, sway/waybar IPC |
| `sway-groups-cli` → `swayg` | User-facing CLI |
| `sway-groups-daemon` → `swayg-daemon` | Catches sway IPC events (new/empty workspace, etc.), keeps DB + bars in sync |
| `sway-groups-dummy-window` | Wayland dummy window for tests (`publish = false`) |
| `sway-groups-tests` | Integration tests against a live sway session (`publish = false`) |

### Tables

- `workspaces`, `groups` — main entities
- `workspace_groups` — many-to-many membership
- `hidden_workspaces` — presence-based `(workspace_id, group_id)` pairs
- `outputs` — per-output state (including active group)
- `settings` — global runtime flags (key/value)
- `focus_history`, `group_state`, `pending_workspace_events` — internal
  state for nav-back and daemon coordination

## Troubleshooting

- `RUST_LOG=debug swayg <cmd>` — verbose tracing to stderr
- Log files under `~/.local/share/swayg/`
- `swayg repair` — reconcile DB with sway (removes stale workspaces etc.)
- `swayg sync --all --init-bars --init-bars-retries 20 --init-bars-delay-ms 500`
  — after `swaymsg reload`, retry pushing to waybar until its socket is
  back up

## Development

```sh
cargo build --workspace
cargo test -- --test-threads=1    # integration tests need a serialised sway session
cargo clippy --workspace --all-targets
```

The integration test suite spawns a test-mode daemon, temporarily stops
the production daemon, and tears everything down in `Drop`. All tests
must be able to run against a real sway socket.

## License

MIT
