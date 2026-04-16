# sway-groups (`swayg`)

Group-aware workspace management for [sway](https://swaywm.org/), with
[waybar](https://github.com/Alexays/Waybar) integration via
[waybar-dynamic](https://github.com/bschnitz/waybar-dynamic).

(Theoretically waybar is an optional dependency, but there are not yet any other
adapters for other bars.)

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

![swayg bars in waybar](screenshot.png)

## Setup overview

1. [Install the CLI](#1-install-the-cli) (`swayg`)
2. [Install and start the daemon](#2-install-and-start-the-daemon) (`swayg-daemon`)
3. [Install waybar-dynamic](#3-install-waybar-dynamic)
4. [Configure waybar](#4-configure-waybar)
5. [Style the bar](#5-style-the-bar)
6. [Use the CLI and bind keys](#6-use-the-cli-and-bind-keys)

### 1. Install the CLI

Requires a Rust toolchain (stable, edition 2024).

**From crates.io:**

```sh
cargo install sway-groups-cli
```

**From git (latest development version):**

```sh
cargo install --git https://github.com/bschnitz/sway-groups sway-groups-cli
```

**From a local clone:**

```sh
git clone https://github.com/bschnitz/sway-groups
cd sway-groups
cargo install --path sway-groups-cli
```

The binary `swayg` lands in `~/.cargo/bin/`. Make sure that's in your `PATH`.

### 2. Install and start the daemon

The daemon watches sway IPC events (workspace creation/deletion, urgency
changes) and keeps the DB and bar in sync.

**Install:**

```sh
cargo install sway-groups-daemon        # from crates.io
# or
cargo install --git https://github.com/bschnitz/sway-groups sway-groups-daemon
```

**Option A: systemd user service (recommended)**

Create `~/.config/systemd/user/swayg-daemon.service`:

```ini
[Unit]
Description=swayg daemon - track external sway workspace events
After=graphical-session.target
PartOf=graphical-session.target

[Service]
Type=simple
ExecStart=%h/.cargo/bin/swayg-daemon
Restart=on-failure
RestartSec=5
Environment=RUST_LOG=sway_groups_daemon=info

[Install]
WantedBy=graphical-session.target
```

```sh
systemctl --user daemon-reload
systemctl --user enable --now swayg-daemon.service
```

The unit is `WantedBy=graphical-session.target`. For sway users, make sure
the target actually gets activated. Create
`~/.config/systemd/user/sway-session.target`:

```ini
[Unit]
Description=sway compositor session
BindsTo=graphical-session.target
```

…and in your sway `config`:

```
exec systemctl --user --no-block start sway-session.target
```

**Option B: start directly from sway config (no systemd)**

Add to your sway `config`:

```
exec swayg-daemon
```

The daemon runs in the foreground and exits when sway exits. Logs go to
stderr (visible in sway's journal or log file). Set `RUST_LOG=info` for
verbose output:

```
exec RUST_LOG=sway_groups_daemon=info swayg-daemon
```

### 3. Install waybar-dynamic

[waybar-dynamic](https://github.com/bschnitz/waybar-dynamic) is the CFFI
module that renders swayg's widgets in waybar. Follow its
[installation instructions](https://github.com/bschnitz/waybar-dynamic#installation)
— in short:

```sh
git clone https://github.com/bschnitz/waybar-dynamic
cd waybar-dynamic
cargo build --release
mkdir -p ~/.config/waybar/modules
cp target/release/libwaybar_dynamic.so ~/.config/waybar/modules/
```

### 4. Configure waybar

Add two waybar-dynamic modules to your `~/.config/waybar/config.jsonc` — one
for groups, one for workspaces:

```jsonc
{
    "modules-left": [
        "cffi/swayg_groups",
        "cffi/swayg_workspaces"
    ],

    "cffi/swayg_groups": {
        "module_path": "~/.config/waybar/modules/libwaybar_dynamic.so",
        "name": "swayg_groups"
    },
    "cffi/swayg_workspaces": {
        "module_path": "~/.config/waybar/modules/libwaybar_dynamic.so",
        "name": "swayg_workspaces"
    }
}
```

`swayg` pushes widget updates to these modules automatically after every
state-changing command.

### 5. Style the bar

Widgets carry CSS classes you can style in `~/.config/waybar/style.css`:

- **`swayg_workspaces`**: `focused`, `visible`, `urgent`, `global`,
  `hidden` (only when `show_hidden_workspaces = true`). Classes combine,
  e.g. `.focused.global`, `.hidden.global.focused`.
- **`swayg_groups`**: `active`, `urgent` (a workspace in the group is
  urgent).

**Example theme** (lavender workspaces, blue groups — as in the screenshot):

```css
/* ── swayg workspaces — lavender, lime accent for globals ───────── */
#waybar-dynamic.swayg_workspaces label {
    padding: 0 5px;
    background: transparent;
    color: #C9A0F8;
    border-bottom: 3px solid rgba(184, 133, 255, 0.7);
    border-radius: 0;
    transition: background 0.15s, color 0.15s;
}
#waybar-dynamic.swayg_workspaces label.focused {
    background: rgba(184, 133, 255, 0.35);
    color: #ffffff;
    border-bottom: 3px solid #D4AAFF;
}
#waybar-dynamic.swayg_workspaces label.visible {
    color: rgba(184, 133, 255, 0.75);
}
#waybar-dynamic.swayg_workspaces label.urgent {
    background-image: linear-gradient(to top, transparent, rgba(232, 69, 60, 0.7));
    color: #ffffff;
}
#waybar-dynamic.swayg_workspaces label.global {
    color: #b8f060;
    border-bottom: 3px solid rgba(184, 240, 96, 0.75);
}
#waybar-dynamic.swayg_workspaces label.focused.global {
    background: rgba(184, 133, 255, 0.3);
    color: #b8f060;
    border-bottom: 3px solid #b8f060;
}
#waybar-dynamic.swayg_workspaces label.hover {
    background: rgba(184, 133, 255, 0.2);
}

/* Hidden workspaces: faded + italic + dashed border */
#waybar-dynamic.swayg_workspaces label.hidden {
    opacity: 0.45;
    border-bottom: 3px dashed rgba(184, 133, 255, 0.7);
    font-style: italic;
}
#waybar-dynamic.swayg_workspaces label.hidden.focused {
    opacity: 0.8;
    background: rgba(184, 133, 255, 0.25);
    color: #ffffff;
    border-bottom: 3px dashed #D4AAFF;
}
#waybar-dynamic.swayg_workspaces label.hidden.urgent {
    opacity: 1.0;
    background-image: linear-gradient(to top, transparent, rgba(232, 69, 60, 0.7));
    color: #ffffff;
    font-style: normal;
}

/* ── swayg groups — blue accent ─────────────────────────────────── */
#waybar-dynamic.swayg_groups label {
    padding: 0 5px;
    background: transparent;
    color: rgba(255, 255, 255, 0.5);
    border-bottom: 3px solid rgba(137, 180, 250, 0.3);
    border-radius: 0;
}
#waybar-dynamic.swayg_groups label.active {
    color: #ffffff;
    background: rgba(137, 180, 250, 0.15);
    border-bottom: 3px solid #89b4fa;
}
#waybar-dynamic.swayg_groups label.urgent {
    background-image: linear-gradient(to top, transparent, rgba(235, 77, 75, 0.7));
    color: #ffffff;
}
#waybar-dynamic.swayg_groups label.hover {
    background: rgba(100, 114, 125, 0.3);
}
#waybar-dynamic.swayg_groups label.active.hover {
    background: rgba(137, 180, 250, 0.3);
}
```

### 6. Use the CLI and bind keys

**First-time setup:**

```sh
swayg init             # creates the DB and imports current sway state
```

This seeds the DB from sway's current workspaces, creates the default
group (`0`), and pushes initial bar widgets.

**Example sway keybindings** (add to your sway `config`):

```
# Switch groups
bindsym $mod+a exec swayg group next -w
bindsym $mod+d exec swayg group prev -w

# Navigate workspaces within active group
bindsym $mod+n exec swayg nav next -w
bindsym $mod+p exec swayg nav prev -w

# Move container to workspace
bindsym $mod+Shift+n exec swayg container move next --switch-to-workspace

# Re-sync after swaymsg reload
bindsym $mod+r exec sh -c 'swaymsg reload && sleep 0.3 && swayg sync --init-bars --init-bars-retries 20 --init-bars-delay-ms 500'
```

**CLI overview:**

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

# Hiding (auto-focuses away when the focused workspace becomes invisible)
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
- `[[assign]]` — workspace assignment rules (see below)

### Assignment rules

When the daemon sees a new workspace, it normally adds it to the active
group. Assignment rules let you override this per workspace name — useful
together with sway's `assign` / `for_window` rules:

```toml
# Exact match: put "music" in media + bg, mark global
[[assign]]
match = "music"
groups = ["media", "bg"]
global = true

# Regex match: any workspace starting with "dev_" goes to dev group
[[assign]]
match = "^dev_"
match_type = "regex"
groups = ["dev"]
```

- `match` — pattern to match against the workspace name.
- `match_type` — `"exact"` (default) or `"regex"`.
- `groups` — groups to add the workspace to. When set, replaces the
  default "add to active group" behaviour.
- `global` — mark the workspace as global (`true`/`false`).

If a rule sets `global = true` but specifies no `groups`, the workspace
is still added to the active group (in addition to being global).
Multiple rules can match the same workspace — their groups are merged.

Runtime DB flags (separate from the config file):

- `show_hidden_workspaces` — toggled via `swayg workspace show-hidden`

## Storage locations

- SQLite DB: `~/.local/share/swayg/swayg.db`
- Log files: `~/.local/share/swayg/swayg.YYYY-MM-DD` (daily rotation)
- Config (optional): `~/.config/swayg/config.toml`

Reset all state:

```sh
rm ~/.local/share/swayg/swayg.db
swayg init
```

## Architecture

| Crate | Role |
|---|---|
| `sway-groups-config` | TOML config schema + loader |
| `sway-groups-core` | DB entities, services, sway/waybar IPC |
| `sway-groups-cli` → `swayg` | User-facing CLI |
| `sway-groups-daemon` → `swayg-daemon` | Catches sway IPC events, keeps DB + bars in sync |
| `sway-groups-dummy-window` | Wayland dummy window for tests (`publish = false`) |
| `sway-groups-tests` | Integration tests against a live sway session (`publish = false`) |

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
cargo test -p sway-groups-tests -- --test-threads=1   # integration tests need a serialised sway session
cargo clippy --workspace --all-targets
```

The integration test suite spawns a test-mode daemon, temporarily stops
the production daemon, and tears everything down in `Drop`. All tests
must be able to run against a real sway socket.

### Waybar test progress

During test runs a waybar `custom` module shows which test is running
and overall progress (n/m). The test fixture writes JSON to
`/tmp/swayg-test-progress.json` which waybar polls every second.

Add the module to your waybar config (e.g. in `modules-center`):

```jsonc
"custom/swayg_tests": {
    "exec": "cat /tmp/swayg-test-progress.json 2>/dev/null || echo '{}'",
    "return-type": "json",
    "interval": 1,
    "tooltip": true
}
```

Suggested CSS (pill badge, yellow while running, green when done):

```css
#custom-swayg_tests {
    padding: 2px 12px;
    margin: 4px 0;
    background: rgba(80, 80, 100, 0.4);
    color: rgba(255, 255, 255, 0.5);
    border-radius: 12px;
    font-size: 12px;
}
#custom-swayg_tests.running {
    color: #1e1e2e;
    background: #fac850;
    font-weight: bold;
}
#custom-swayg_tests.done {
    color: #1e1e2e;
    background: #a6e3a1;
    font-weight: bold;
}
```

## License

MIT
