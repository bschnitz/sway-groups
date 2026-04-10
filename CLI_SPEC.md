# swayg - Sway Workspace Groups CLI

## Overview

`swayg` is a CLI tool for managing sway workspace groups. It wraps sway IPC commands to provide group-aware workspace navigation and management. Workspace visibility in waybar is handled via [waybar-dynamic](https://github.com/AriaSeitia/waybar-dynamic) IPC.

## Key Concepts

- **Workspace**: A sway workspace (e.g., "1", "2", "3:Firefox")
- **Group**: A named collection of workspaces (e.g., "0", "dev", "work")
- **Active Group**: The currently selected group per output -- only workspaces in this group are shown in waybar
- **Global Workspace**: A workspace visible in ALL groups

## Database Schema

### Tables

#### `groups`
| Column | Type | Constraints |
|--------|------|-------------|
| id | INTEGER | PRIMARY KEY, AUTO_INCREMENT |
| name | TEXT | UNIQUE, NOT NULL |
| created_at | DATETIME | NOT NULL |
| updated_at | DATETIME | NOT NULL |

#### `workspaces`
| Column | Type | Constraints |
|--------|------|-------------|
| id | INTEGER | PRIMARY KEY, AUTO_INCREMENT |
| name | TEXT | UNIQUE, NOT NULL |
| number | INTEGER | NULLABLE |
| output | TEXT | NULLABLE |
| is_global | BOOLEAN | DEFAULT FALSE |
| created_at | DATETIME | NOT NULL |
| updated_at | DATETIME | NOT NULL |

#### `workspace_groups`
| Column | Type | Constraints |
|--------|------|-------------|
| id | INTEGER | PRIMARY KEY, AUTO_INCREMENT |
| workspace_id | INTEGER | FOREIGN KEY -> workspaces.id |
| group_id | INTEGER | FOREIGN KEY -> groups.id |
| created_at | DATETIME | NOT NULL |

#### `outputs`
| Column | Type | Constraints |
|--------|------|-------------|
| id | INTEGER | PRIMARY KEY, AUTO_INCREMENT |
| name | TEXT | UNIQUE, NOT NULL |
| active_group | TEXT | NOT NULL |
| created_at | DATETIME | NOT NULL |
| updated_at | DATETIME | NOT NULL |

#### `group_state`
| Column | Type | Constraints |
|--------|------|-------------|
| id | INTEGER | PRIMARY KEY, AUTO_INCREMENT |
| output | TEXT | NOT NULL |
| group_name | TEXT | NOT NULL |
| last_focused_workspace | TEXT | NULLABLE |
| last_visited | DATETIME | NULLABLE |

Tracks the last focused workspace per group per output for workspace restoration on group switch.

#### `focus_history`
| Column | Type | Constraints |
|--------|------|-------------|
| id | INTEGER | PRIMARY KEY, AUTO_INCREMENT |
| workspace_name | TEXT | NOT NULL |
| focused_at | DATETIME | NOT NULL |

Stack of workspace focus events for `nav back`. Entries older than 10 minutes are automatically pruned.

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
List all groups and their workspaces.

```sh
swayg group list
swayg group list --output DP-1
```

#### `swayg group create <NAME>`
Create a new group.

```sh
$ swayg group create dev
Created group "dev"
```

#### `swayg group delete <NAME> [-f|--force]`
Delete a group. Requires `--force` if workspaces are assigned. The default group "0" cannot be deleted.

```sh
$ swayg group delete old-project
Error: Group "old-project" has 1 workspaces. Use --force to delete anyway.

$ swayg group delete old-project --force
Deleted group "old-project"
```

#### `swayg group rename <OLD_NAME> <NEW_NAME>`
Rename a group. Cannot rename the default group "0".

```sh
$ swayg group rename work project
Renamed group "work" to "project"
```

#### `swayg group select <OUTPUT> <GROUP>`
Set the active group for an output. Automatically switches workspace focus:
- **Empty group**: Focuses workspace "0"
- **First visit**: Focuses alphabetically first workspace in the group
- **Previously visited**: Restores last focused workspace in the group

```sh
$ swayg group select eDP-1 dev
Set active group for "eDP-1" to "dev"
```

#### `swayg group active <OUTPUT>`
Show the currently active group for an output. Falls back to "0" if the output is not yet registered.

```sh
$ swayg group active eDP-1
dev
```

#### `swayg group next [-o|--output <OUTPUT>] [-w|--wrap]`
Switch to the next group alphabetically (all groups).

```sh
swayg group next --output eDP-1 --wrap
```

#### `swayg group next-on-output [-o|--output <OUTPUT>] [-w|--wrap]`
Switch to the next non-empty group on the output. Only considers groups that have workspaces on the specified output.

```sh
swayg group next-on-output --output eDP-1 --wrap
```

#### `swayg group prev [-o|--output <OUTPUT>] [-w|--wrap]`
Switch to the previous group alphabetically (all groups).

```sh
swayg group prev --output eDP-1 --wrap
```

#### `swayg group prev-on-output [-o|--output <OUTPUT>] [-w|--wrap]`
Switch to the previous non-empty group on the output.

```sh
swayg group prev-on-output --output eDP-1 --wrap
```

#### `swayg group prune [--keep <NAME>...]`
Remove empty groups (except default "0"). A group is considered empty if it contains no non-global workspaces.

```sh
$ swayg group prune --keep 0 --keep default
Pruned 2 empty group(s)
```

### `swayg workspace` - Workspace Management

#### `swayg workspace list [-o|--output <OUTPUT>] [-g|--group <GROUP>] [--visible] [--plain]`
List workspaces, optionally filtered by output and/or group.

`--visible` shows only workspaces visible in the active group on the output.
`--plain` outputs workspace names only, one per line (useful for scripting/piping).

```sh
$ swayg workspace list --group dev
Workspaces in group "dev" on "eDP-1":
  1:Firefox    (visible)
  2:Terminal   (hidden)
  3            (global)
```

```sh
$ swayg workspace list --visible --plain
1
28_www
```

#### `swayg workspace add <WORKSPACE> [-g|--group <GROUP>]`
Add a workspace to a group. The workspace must exist in sway. If `--group` is omitted, defaults to the active group for the output. A workspace can belong to multiple groups.

```sh
$ swayg workspace add 4 --group dev
Added workspace "4" to group "dev"
```

#### `swayg workspace rename <OLD_NAME> <NEW_NAME>`
Rename a workspace in sway and update the database. If the target name already exists, the source workspace is merged into the target (containers are moved, group memberships are unioned).

```sh
$ swayg workspace rename old_name new_name
Renamed workspace "old_name" to "new_name"
```

#### `swayg workspace move <WORKSPACE> -g|--groups <GROUPS>`
Move a workspace to one or more groups (comma-separated), removing it from all other groups. The groups must exist.

```sh
$ swayg workspace move 4 --groups dev
Moved workspace "4" to group(s): dev

$ swayg workspace move 4 --groups dev,work
Moved workspace "4" to group(s): dev, work
```

#### `swayg workspace remove <WORKSPACE> [-g|--group <GROUP>]`
Remove a workspace from a group. Defaults to the active group.

```sh
$ swayg workspace remove 4 --group dev
Removed workspace "4" from group "dev"
```

#### `swayg workspace global <WORKSPACE>`
Mark a workspace as global (visible in all groups).

```sh
$ swayg workspace global 1
Marked workspace "1" as global
```

#### `swayg workspace unglobal <WORKSPACE>`
Remove global status from a workspace.

```sh
$ swayg workspace unglobal 1
Removed global status from workspace "1"
```

#### `swayg workspace groups <WORKSPACE>`
List all groups a workspace belongs to.

```sh
$ swayg workspace groups 2
Workspace "2" is in groups: "0", "dev"
```

### `swayg nav` - Navigation Commands

Group-aware workspace navigation. Only considers workspaces in the active group plus global workspaces.

#### `swayg nav next [-o|--output <OUTPUT>] [-w|--wrap]`
Navigate to the next workspace in the active group on the output.

```sh
$ swayg nav next --output eDP-1
Navigated to "2"
```

#### `swayg nav next-on-output [-w|--wrap]`
Navigate to the next workspace globally across all outputs.

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
$ swayg nav go 3
Navigated to "3"
```

#### `swayg nav move-to <WORKSPACE>`
Move the currently focused container to a specific workspace. The target workspace is automatically added to the active group.

```sh
$ swayg nav move-to 3
Moved container to "3"
```

#### `swayg nav back`
Navigate back to the previously focused workspace. Maintains a focus history with entries pruned after 10 minutes.

```sh
$ swayg nav back
Navigated back to "1"
```

### `swayg sync`
Manually synchronize the database with the current sway state. Useful for initial setup or recovery.

```sh
$ swayg sync --all
Synced: workspaces, groups, outputs
```

### `swayg status`
Show current status of all outputs and their active groups.

```sh
$ swayg status
eDP-1: active group = "dev"
  Visible: 1, 3
  Hidden: 2, 4
HDMI-A-0: active group = "0"
  Visible: 5
  Hidden: (none)
```

## Implementation Notes

- Uses SeaORM 2.0 with entity-first approach
- SQLite database for persistence
- Sway IPC over Unix socket
- waybar-dynamic IPC for workspace widget updates
- Async runtime with Tokio
- Rust Edition 2024
- Log files: `~/.local/share/swayg/swayg.YYYY-MM-DD` (rolling daily)
