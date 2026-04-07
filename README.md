# swayg - Sway Workspace Groups

## Overview

`swayg` is a CLI tool for managing sway workspace groups. It wraps sway IPC commands to provide group-aware workspace navigation and management. Workspaces are organized into named groups, and switching between groups automatically hides/shows the appropriate workspaces via waybar-compatible suffixes.

## Installation

### Prerequisites

- Rust toolchain (stable, edition 2024)
- Sway window manager
- systemd (user instance) for daemon management

### Build and Install

```sh
# Clone and install
cargo install --path sway-groups-cli

# Install systemd service and start daemon
cp swaygd.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now swaygd
```

Or use the convenience script:

```sh
./install.sh
```

### Verify Installation

```sh
swayg --help
swayg daemon status
```

## Initial Setup

After installation, sync your existing sway workspaces into the database:

```sh
swayg sync --all
```

This creates the default group "0" containing all existing workspaces.

## Key Concepts

- **Workspace**: A sway workspace (e.g., "1", "2", "3:Firefox")
- **Group**: A named collection of workspaces (e.g., "0", "dev", "work")
- **Active Group**: The currently selected group per output -- only workspaces in this group are visible
- **Global Workspace**: A workspace visible in ALL groups (marked with `_class_global`)
- **Hidden Suffix**: Non-active workspaces get `_class_hidden` suffix to indicate they should be hidden in waybar

## Database

swayg uses SQLite for persistence. The database is located at:

```
~/.local/share/swayg/swayg.db
```

To reset everything (delete all groups, workspaces, and state):

```sh
rm ~/.local/share/swayg/swayg.db
swayg sync --all
```

## Daemon (swaygd)

The daemon `swaygd` runs in the background and automatically syncs workspace suffixes when sway events occur (workspace changes, output changes, shutdown).

### Daemon Management

```sh
swayg daemon status     # Check if daemon is running
swayg daemon start      # Start the daemon
swayg daemon stop       # Stop the daemon
```

The daemon is typically managed via systemd:

```sh
systemctl --user status swaygd
systemctl --user restart swaygd
```

The daemon writes a PID file to `~/.local/share/swayg/swaygd.pid`.

## Group Switching Behavior

When switching to a group, swayg automatically focuses a workspace in the new group:

1. **Empty group**: Creates and focuses workspace "0" on the output
2. **First visit**: Focuses the alphabetically first workspace in the group
3. **Previously visited**: Restores the last focused workspace in that group

The last focused workspace per group/output is persisted in the database.

## CLI Commands

### Global Options
```
swayg [OPTIONS] <COMMAND>
    -h, --help     Show help
    -V, --version  Show version
    -v, --verbose  Enable verbose output
```

### `swayg group` - Group Management

#### `swayg group list [-o|--output <OUTPUT>]`
List all groups and their workspaces. Optional filter by output.

```sh
swayg group list
swayg group list --output DP-1
```

#### `swayg group create <NAME>`
Create a new group.

```sh
swayg group create dev
```

#### `swayg group delete <NAME> [-f|--force]`
Delete a group. Requires `--force` if the group has workspaces assigned.

```sh
swayg group delete old-project
swayg group delete old-project --force
```

The default group "0" cannot be deleted.

#### `swayg group rename <OLD_NAME> <NEW_NAME>`
Rename a group. The default group "0" cannot be renamed.

```sh
swayg group rename work project
```

#### `swayg group select <OUTPUT> <GROUP>`
Set the active group for an output. This automatically:
- Saves the currently focused workspace for the old group
- Switches sway focus to an appropriate workspace in the new group
- Syncs all workspace suffixes

```sh
swayg group select eDP-1 dev
```

#### `swayg group active <OUTPUT>`
Show the currently active group for an output.

```sh
swayg group active eDP-1
```

#### `swayg group next [-o|--output <OUTPUT>] [-w|--wrap]`
Switch to the next group alphabetically (all groups). Without `--wrap`, stops at the last group.

```sh
swayg group next --output eDP-1 --wrap
```

#### `swayg group next-on-output [-o|--output <OUTPUT>] [-w|--wrap]`
Switch to the next **non-empty** group on the output. Skips groups that have no workspaces on the specified output.

```sh
swayg group next-on-output --output eDP-1 --wrap
```

#### `swayg group prev [-o|--output <OUTPUT>] [-w|--wrap]`
Switch to the previous group alphabetically (all groups).

```sh
swayg group prev --output eDP-1 --wrap
```

#### `swayg group prev-on-output [-o|--output <OUTPUT>] [-w|--wrap]`
Switch to the previous **non-empty** group on the output.

```sh
swayg group prev-on-output --output eDP-1 --wrap
```

#### `swayg group prune [--keep <NAME>...]`
Remove empty groups (except default "0"). Specify groups to keep with `--keep`.

```sh
swayg group prune
swayg group prune --keep 0 --keep default
```

### `swayg workspace` - Workspace Management

#### `swayg workspace list [-o|--output <OUTPUT>] [-g|--group <GROUP>] [--visible] [--plain]`
List workspaces, optionally filtered by output and/or group. `--visible` shows only workspaces in the active group. `--plain` outputs names only (useful for piping).

```sh
swayg workspace list
swayg workspace list --group dev
swayg workspace list --visible --plain
```

#### `swayg workspace add <WORKSPACE> [-g|--group <GROUP>]`
Add a workspace to a group. The workspace must exist in sway. If `--group` is omitted, the active group for the output is used.

```sh
swayg workspace add 4 --group dev
```

A workspace can belong to multiple groups simultaneously.

#### `swayg workspace move <WORKSPACE> -g|--groups <GROUPS>`
Move a workspace to specific groups (comma-separated), removing it from all other groups.

```sh
swayg workspace move 4 --groups dev
swayg workspace move 4 --groups dev,work
```

#### `swayg workspace remove <WORKSPACE> [-g|--group <GROUP>]`
Remove a workspace from a group. If `--group` is omitted, the active group is used.

```sh
swayg workspace remove 4 --group dev
```

#### `swayg workspace global <WORKSPACE>`
Mark a workspace as global (visible in all groups).

```sh
swayg workspace global 1
```

#### `swayg workspace unglobal <WORKSPACE>`
Remove global status from a workspace.

```sh
swayg workspace unglobal 1
```

#### `swayg workspace groups <WORKSPACE>`
List all groups a workspace belongs to.

```sh
swayg workspace groups 2
```

### `swayg nav` - Navigation Commands

Group-aware workspace navigation. Only considers workspaces in the active group (plus global workspaces).

#### `swayg nav next [-o|--output <OUTPUT>] [-w|--wrap]`
Navigate to the next workspace in the active group on the output.

```sh
swayg nav next --output eDP-1 --wrap
```

#### `swayg nav next-on-output [-w|--wrap]`
Navigate to the next workspace globally, considering all visible workspaces across all outputs.

```sh
swayg nav next-on-output --wrap
```

#### `swayg nav prev [-o|--output <OUTPUT>] [-w|--wrap]`
Navigate to the previous workspace in the active group on the output.

```sh
swayg nav prev --output eDP-1 --wrap
```

#### `swayg nav prev-on-output [-w|--wrap]`
Navigate to the previous workspace globally across all outputs.

```sh
swayg nav prev-on-output --wrap
```

#### `swayg nav go <WORKSPACE>`
Navigate to a specific workspace.

```sh
swayg nav go 3
```

#### `swayg nav move-to <WORKSPACE>`
Move the currently focused container to a specific workspace.

```sh
swayg nav move-to 3
```

#### `swayg nav back`
Navigate back to the previously focused workspace. Maintains a focus history (entries older than 10 minutes are pruned automatically).

```sh
swayg nav back
```

### `swayg sync`
Manually synchronize the database with the current sway state. Normally handled automatically by the daemon, but useful for initial setup or recovery.

```sh
swayg sync --all          # Sync everything
swayg sync --workspaces   # Sync only workspaces
```

### `swayg status`
Show current status of all outputs and their active groups.

```sh
swayg status
```

Example output:
```
eDP-1: active group = "dev"
  Visible: 1, 3
  Hidden: 2, 4
HDMI-A-0: active group = "0"
  Visible: 5
  Hidden: (none)
```

### `swayg daemon` - Daemon Management

#### `swayg daemon start`
Start the swayg daemon. Checks if already running.

#### `swayg daemon stop`
Stop the running daemon via SIGTERM.

#### `swayg daemon status`
Check if the daemon is running by checking the PID file.

## Suffix Management

### Suffix Rules

1. **Global workspaces**: Always get `_class_global` suffix (never hidden)
2. **Active group workspaces**: No suffix (visible)
3. **Other group workspaces**: Get `_class_hidden` suffix
4. **Workspaces not in any group**: Treated as in group "0" (default)

### Suffix Sync

Suffixes are automatically synced when:
- Active group changes (`swayg group select`, `swayg group next`, etc.)
- Workspace added/removed from group
- Workspace global status changes
- Daemon receives sway workspace/output events

### Workspace Naming Convention

Workspaces in sway use a naming convention with optional suffixes:
- Basic: `"1"`, `"2"`, `"3"`
- Named: `"1:Firefox"`, `"2:Terminal"`
- Hidden: `"2_class_hidden"`, `"3:Code_class_hidden"`
- Global: `"1_class_global"`

The CLI handles the suffix manipulation transparently.

### waybar Integration

To hide workspaces with `_class_hidden` suffix in waybar, configure the sway/workspaces module:

```json
"sway/workspaces": {
    "format": "{icon} {name}",
    "name_map": {
        "_class_hidden": ""
    }
}
```

Or use a custom `rewrite` rule to hide them.

## Typical Workflow

```sh
# Initial setup after install
swayg sync --all

# Create groups for different projects
swayg group create dev
swayg group create work

# Assign workspaces to groups (a workspace can be in multiple groups)
swayg workspace add 1 --group dev
swayg workspace add 2 --group dev
swayg workspace add 3 --group work

# Make a workspace global (visible in all groups)
swayg workspace global 1

# Switch between groups
swayg group select eDP-1 dev
swayg group select eDP-1 work

# Use next/prev to cycle through groups
swayg group next --output eDP-1 --wrap

# Bind group switching in sway config:
# bindsym $mod+bracketright exec swayg group next -o eDP-1 -w
# bindsym $mod+bracketleft exec swayg group prev -o eDP-1 -w
```

## Troubleshooting

### Workspaces stuck in hidden state
Run `swayg sync --all` to resync suffixes, or restart the daemon:

```sh
systemctl --user restart swaygd
```

### Reset everything
```sh
rm ~/.local/share/swayg/swayg.db
swayg sync --all
```

### Enable verbose logging
```sh
RUST_LOG=debug swayg status
RUST_LOG=debug swaygd
```
