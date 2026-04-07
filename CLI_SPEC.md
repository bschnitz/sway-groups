# swayg - Sway Workspace Groups CLI

## Overview

`swayg` is a CLI tool for managing sway workspace groups. It wraps sway IPC commands to provide group-aware workspace navigation and management.

## Key Concepts

- **Workspace**: A sway workspace (e.g., "1", "2", "3:Firefox")
- **Group**: A named collection of workspaces (e.g., "0", "dev", "work")
- **Active Group**: The currently selected group per output - only workspaces in this group are visible
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
| workspace_id | INTEGER | FOREIGN KEY → groups.id |
| group_id | INTEGER | FOREIGN KEY → workspaces.id |
| created_at | DATETIME | NOT NULL |

#### `outputs`
| Column | Type | Constraints |
|--------|------|-------------|
| id | INTEGER | PRIMARY KEY, AUTO_INCREMENT |
| name | TEXT | UNIQUE, NOT NULL |
| active_group | TEXT | NOT NULL |
| created_at | DATETIME | NOT NULL |
| updated_at | DATETIME | NOT NULL |

## CLI Commands

### Global Options
```
swayg [OPTIONS] <COMMAND>
    -h, --help     Show help
    -V, --version  Show version
    -v, --verbose  Enable verbose output
```

### `swayg group` - Group Management

#### `swayg group list [OPTIONS]`
List all groups and their workspaces.

**Options:**
```
-o, --output <OUTPUT>  Filter by output name
```

**Example Output:**
```
Group "0":
  - 1
  - 2
  - 3
Group "dev":
  - 1:Firefox
  - 2:Terminal
```

#### `swayg group create <NAME>`
Create a new group.

**Arguments:**
```
<NAME>              Name of the group to create
```

**Example:**
```
$ swayg group create dev
Created group "dev"
```

#### `swayg group delete <NAME> [-f|--force]`
Delete a group.

**Arguments:**
```
<NAME>              Name of the group to delete
```

**Options:**
```
-f, --force         Force delete even if workspaces are assigned
```

**Example:**
```
$ swayg group delete old-project
Error: Group "old-project" has 3 workspaces. Use --force to delete anyway.
```

#### `swayg group rename <OLD_NAME> <NEW_NAME>`
Rename a group.

**Arguments:**
```
<OLD_NAME>          Current name of the group
<NEW_NAME>          New name for the group
```

**Constraints:**
- Cannot rename the default group "0"

**Example:**
```
$ swayg group rename work project
Renamed group "work" to "project"
```

#### `swayg group select <OUTPUT> <GROUP>`
Set the active group for an output.

**Arguments:**
```
<OUTPUT>             Output name (e.g., "DP-1", "HDMI-A-0")
<GROUP>             Group name to make active
```

**Behavior:**
- Creates the output entry if it doesn't exist
- Syncs workspace suffixes after changing active group

**Example:**
```
$ swayg group select DP-1 dev
Set active group for "DP-1" to "dev"
```

#### `swayg group active <OUTPUT>`
Show the currently active group for an output.

**Arguments:**
```
<OUTPUT>             Output name
```

**Example:**
```
$ swayg group active DP-1
dev
```

#### `swayg group next [-o|--output <OUTPUT>] [-w|--wrap]`
Switch to the next group alphabetically.

**Options:**
```
-o, --output <OUTPUT>  Output name (default: primary output)
-w, --wrap            Wrap around to first group
```

**Example:**
```
$ swayg group next --output DP-1 --wrap
Switched from "dev" to "project"
```

#### `swayg group prev [-o|--output <OUTPUT>] [-w|--wrap]`
Switch to the previous group alphabetically.

**Options:**
```
-o, --output <OUTPUT>  Output name (default: primary output)
-w, --wrap            Wrap around to last group
```

#### `swayg group prune [--keep <NAME>...]`
Remove empty groups (except default "0").

**Options:**
```
--keep <NAME>        Groups to keep even if empty (can be repeated)
```

**Example:**
```
$ swayg group prune --keep 0 --keep default
Pruned 2 empty groups
```

### `swayg workspace` - Workspace Management

#### `swayg workspace list [-o|--output <OUTPUT>] [-g|--group <GROUP>]`
List workspaces in a group.

**Options:**
```
-o, --output <OUTPUT>  Filter by output
-g, --group <GROUP>    Filter by group (default: active group for output)
```

**Example Output:**
```
Workspaces in group "dev" on "DP-1":
  1:Firefox    (visible)
  2:Terminal   (hidden)
  3            (visible)
```

#### `swayg workspace add <WORKSPACE> [-g|--group <GROUP>] [-o|--output <OUTPUT>]`
Add a workspace to a group.

**Arguments:**
```
<WORKSPACE>          Workspace name or number
```

**Options:**
```
-g, --group <GROUP>  Target group (default: active group for output)
-o, --output <OUTPUT>  Output for the workspace
```

**Example:**
```
$ swayg workspace add 4 --group dev
Added workspace "4" to group "dev"
```

#### `swayg workspace remove <WORKSPACE> [-g|--group <GROUP>]`
Remove a workspace from a group.

**Arguments:**
```
<WORKSPACE>          Workspace name
```

**Options:**
```
-g, --group <GROUP>  Source group (default: active group)
```

**Example:**
```
$ swayg workspace remove 4 --group dev
Removed workspace "4" from group "dev"
```

#### `swayg workspace global <WORKSPACE>`
Mark a workspace as global (visible in all groups).

**Arguments:**
```
<WORKSPACE>          Workspace name
```

**Example:**
```
$ swayg workspace global 1
Marked workspace "1" as global
```

#### `swayg workspace unglobal <WORKSPACE>`
Remove global status from a workspace.

**Arguments:**
```
<WORKSPACE>          Workspace name
```

#### `swayg workspace groups <WORKSPACE>`
List all groups a workspace belongs to.

**Arguments:**
```
<WORKSPACE>          Workspace name
```

**Example:**
```
$ swayg workspace groups 2
Workspace "2" is in groups: "0", "dev"
```

### `swayg nav` - Navigation Commands

These are group-aware wrappers around sway workspace commands.

#### `swayg nav next [-o|--output <OUTPUT>] [-w|--wrap]`
Navigate to next workspace in the active group.

**Options:**
```
-o, --output <OUTPUT>  Output name
-w, --wrap            Wrap around
```

**Behavior:**
- Only considers workspaces in the active group
- Global workspaces are always included
- Hides current workspace, reveals next one

**Example:**
```
$ swayg nav next --output DP-1
Navigated from "1" to "2"
```

#### `swayg nav prev [-o|--output <OUTPUT>] [-w|--wrap]`
Navigate to previous workspace in the active group.

**Options:**
```
-o, --output <OUTPUT>  Output name
-w, --wrap            Wrap around
```

#### `swayg nav go <WORKSPACE> [-o|--output <OUTPUT>]`
Navigate to a specific workspace.

**Arguments:**
```
<WORKSPACE>          Target workspace name or number
```

**Options:**
```
-o, --output <OUTPUT>  Output name
```

**Example:**
```
$ swayg nav go 3 --output DP-1
Navigated to "3"
```

#### `swayg nav back [-o|--output <OUTPUT>]`
Navigate back to the previously focused workspace.

**Options:**
```
-o, --output <OUTPUT>  Output name
```

### `swayg sync`
Synchronize database with current sway state.

**Options:**
```
-a, --all            Sync everything
-w, --workspaces     Sync only workspaces
-g, --groups         Sync only groups
-o, --outputs        Sync only outputs
```

**Example:**
```
$ swayg sync --all
Synced:
  - 5 workspaces
  - 3 groups
  - 2 outputs
```

### `swayg status`
Show current status of all outputs and their active groups.

**Example Output:**
```
DP-1: active group = "dev"
  Visible: 1, 3
  Hidden: 2, 4
HDMI-A-0: active group = "0"
  Visible: 5
  Hidden: (none)
```

### `swayg daemon` - Background Service

#### `swayg daemon start`
Start the swayg daemon for automatic suffix synchronization.

**Options:**
```
--socket <PATH>      Unix socket path for daemon communication
```

#### `swayg daemon stop`
Stop the running daemon.

#### `swayg daemon status`
Check if daemon is running.

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

## Workspace Naming Convention

Workspaces in sway use a naming convention:
- Basic: `"1"`, `"2"`, `"3"`
- Named: `"1:Firefox"`, `"2:Terminal"`
- Hidden: `"2_class_hidden"`, `"3:Code_class_hidden"`

The CLI handles the suffix manipulation transparently.

## Implementation Notes

- Uses SeaORM 2.0 with entity-first approach
- SQLite database for persistence
- Sway IPC over Unix socket
- Async runtime with Tokio
- Rust Edition 2024
