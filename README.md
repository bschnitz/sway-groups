# swayg - Sway Workspace Groups

## Overview

`swayg` is a CLI tool for managing sway workspace groups. It wraps sway IPC commands to provide group-aware workspace navigation and management. Workspaces are organized into named groups, and switching between groups shows only the relevant workspaces in waybar via [waybar-dynamic](https://github.com/AriaSeitia/waybar-dynamic) IPC.

## Installation

### Prerequisites

- Rust toolchain (stable, edition 2024)
- Sway window manager
- [waybar-dynamic](https://github.com/AriaSeitia/waybar-dynamic)

### Build and Install

```sh
cargo install --path sway-groups-cli
```

Or use the convenience script:

```sh
./install.sh
```

### Verify Installation

```sh
swayg --help
```

### waybar-dynamic Setup

1. Install [waybar-dynamic](https://github.com/AriaSeitia/waybar-dynamic)
2. Add a custom widget to your waybar config:

```json
"custom/swayg_workspaces": {
    "format": "{}",
    "exec": "",
    "instance": "swayg_workspaces",
    "separator": false,
    "interval": 0
}
```

`swayg` communicates with waybar-dynamic via Unix socket IPC. Each `swayg` command automatically updates the waybar widget.

## Key Concepts

- **Workspace**: A sway workspace (e.g., "1", "2", "3:Firefox")
- **Group**: A named collection of workspaces (e.g., "0", "dev", "work")
- **Active Group**: The currently selected group per output -- only workspaces in this group are shown in waybar
- **Global Workspace**: A workspace visible in ALL groups

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

## Group Switching Behavior

When switching to a group, swayg automatically focuses a workspace in the new group:

1. **Empty group**: Creates and focuses workspace "0" on the output
2. **First visit**: Focuses the alphabetically first workspace in the group
3. **Previously visited**: Restores the last focused workspace in that group

The last focused workspace per group/output is persisted in the database.

Empty groups (containing no non-global workspaces) are automatically deleted when switching away from them.

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
- Updates the waybar widget

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
Remove empty groups (except default "0"). A group is considered empty if it contains no non-global workspaces. Specify groups to keep with `--keep`.

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

#### `swayg workspace rename <OLD_NAME> <NEW_NAME>`
Rename a workspace in sway and update the database. If the target name already exists, the source workspace is merged into the target (containers are moved, group memberships are unioned).

```sh
swayg workspace rename old_name new_name
```

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
Move the currently focused container to a specific workspace. The target workspace is automatically added to the active group.

```sh
swayg nav move-to 3
```

#### `swayg nav back`
Navigate back to the previously focused workspace. Maintains a focus history (entries older than 10 minutes are pruned automatically).

```sh
swayg nav back
```

### `swayg sync`
Manually synchronize the database with the current sway state. Useful for initial setup or recovery.

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

### Reset everything
```sh
rm ~/.local/share/swayg/swayg.db
swayg sync --all
```

### Enable verbose logging
```sh
RUST_LOG=debug swayg status
```

Log files are written to `~/.local/share/swayg/swayg.YYYY-MM-DD` (rolling daily).
