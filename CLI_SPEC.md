# swayg - Sway Workspace Groups CLI

## Overview

`swayg` is a CLI tool for managing sway workspace groups. It wraps sway IPC commands to provide group-aware workspace navigation and management.

## Key Concepts

- **Workspace**: A sway workspace (e.g., "1", "2", "3:Firefox")
- **Group**: A named collection of workspaces (e.g., "0", "dev", "work")
- **Active Group**: The currently selected group per output -- only workspaces in this group are visible
- **Global Workspace**: A workspace visible in ALL groups (marked with `_class_global`)
- **Hidden Suffix**: Non-active workspaces get `_class_hidden` suffix to indicate they should be hidden in waybar

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
Remove empty groups (except default "0").

```sh
$ swayg group prune --keep 0 --keep default
Pruned 2 empty group(s)
```

### `swayg workspace` - Workspace Management

#### `swayg workspace list [-o|--output <OUTPUT>] [-g|--group <GROUP>]`
List workspaces with visibility status.

```sh
$ swayg workspace list --group dev
Workspaces in group "dev" on "eDP-1":
  1:Firefox    (visible)
  2:Terminal   (hidden)
  3            (global)
```

#### `swayg workspace add <WORKSPACE> [-g|--group <GROUP>]`
Add a workspace to a group. The workspace must exist in sway. If `--group` is omitted, defaults to the active group for the output. A workspace can belong to multiple groups.

```sh
$ swayg workspace add 4 --group dev
Added workspace "4" to group "dev"
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

#### `swayg nav back`
Navigate back to the previously focused workspace. Maintains a focus history with entries pruned after 10 minutes.

```sh
$ swayg nav back
Navigated back to "1"
```

### `swayg sync`
Manually synchronize the database with the current sway state. Normally handled automatically by the daemon. Useful for initial setup or recovery.

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

### `swayg daemon` - Background Service

#### `swayg daemon start`
Start the swayg daemon for automatic suffix synchronization. Refuses to start if already running.

#### `swayg daemon stop`
Stop the running daemon via SIGTERM.

#### `swayg daemon status`
Check if the daemon is running.

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
- Daemon receives sway IPC events

## Workspace Naming Convention

Workspaces in sway use a naming convention:
- Basic: `"1"`, `"2"`, `"3"`
- Named: `"1:Firefox"`, `"2:Terminal"`
- Hidden: `"2_class_hidden"`, `"3:Code_class_hidden"`
- Global: `"1_class_global"`

The CLI handles the suffix manipulation transparently.

## Implementation Notes

- Uses SeaORM 2.0 with entity-first approach
- SQLite database for persistence
- Sway IPC over Unix socket
- Async runtime with Tokio
- Rust Edition 2024
