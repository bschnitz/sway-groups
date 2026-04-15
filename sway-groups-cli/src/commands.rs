//! CLI commands for swayg.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use sway_groups_core::services::{GroupService, NavigationService, WaybarSyncService, WorkspaceService};
use sway_groups_core::sway::SwayIpcClient;

#[derive(Parser)]
#[command(name = "swayg")]
#[command(author, version, about = "Sway workspace groups management CLI")]
pub struct Cli {
    /// Enable verbose logging (info/debug to stderr).
    #[arg(short, long)]
    pub verbose: bool,

    /// Path to the database file. Overrides the default location.
    #[arg(short, long, env = "SWAYG_DB")]
    pub db: Option<PathBuf>,

    /// Path to the config file. Overrides the default location.
    #[arg(short, long, env = "SWAYG_CONFIG")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Manage groups (named workspace collections).
    Group {
        #[command(subcommand)]
        action: GroupAction,
    },
    /// Manage workspaces and their group membership.
    Workspace {
        #[command(subcommand)]
        action: WorkspaceAction,
    },
    /// Navigate between workspaces.
    Nav {
        #[command(subcommand)]
        action: NavAction,
    },
    /// Move the focused container between workspaces.
    Container {
        #[command(subcommand)]
        action: ContainerAction,
    },
    /// Synchronize state between sway, the DB, and the waybar bars.
    Sync {
        /// Sync workspaces, groups, and outputs from sway.
        #[arg(short, long)]
        all: bool,

        /// Sync only workspaces from sway into the DB.
        #[arg(short, long)]
        workspaces: bool,

        /// Sync only groups.
        #[arg(short, long)]
        groups: bool,

        /// Sync only outputs from sway.
        #[arg(short, long)]
        outputs: bool,

        /// Run repair pass (reconcile DB with sway, prune stale entries).
        #[arg(long)]
        repair: bool,

        /// Force a full waybar bar refresh; use with retries/delay to wait for
        /// waybar startup.
        #[arg(long)]
        init_bars: bool,

        /// Max retry attempts when the waybar socket isn't available yet.
        #[arg(long, default_value = "5")]
        init_bars_retries: u32,

        /// Delay in milliseconds between waybar-socket retry attempts.
        #[arg(long, default_value = "200")]
        init_bars_delay_ms: u64,
    },
    /// Initialize the database and sync current sway state.
    Init {
        /// Restart the swayg-daemon systemd user service after init.
        #[arg(long)]
        restart_daemon_service: bool,
    },
    /// Repair DB state by reconciling with sway (removes stale entries).
    Repair,
    /// Show current group/workspace status per output.
    Status,
    /// Configuration file operations.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Write the default configuration to stdout or a file.
    Dump {
        /// Write to this file instead of stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum GroupAction {
    /// List all groups.
    List {
        /// Only list groups that have workspaces on this output.
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Create a new group.
    Create {
        /// Name of the new group.
        name: String,
    },
    /// Delete a group.
    Delete {
        /// Group name to delete.
        name: String,
        /// Delete even if the group has workspaces; orphans move to default_group.
        #[arg(short, long)]
        force: bool,
    },
    /// Rename an existing group.
    Rename {
        /// Current group name.
        old_name: String,
        /// New group name.
        new_name: String,
    },
    /// Make a group the active group for an output.
    Select {
        /// Group to activate.
        group: String,
        /// Target output (default: focused output).
        #[arg(short, long)]
        output: Option<String>,
        /// Create the group if it does not exist yet.
        #[arg(short, long)]
        create: bool,
    },
    /// Print the active group of an output.
    Active {
        /// Output to query.
        output: String,
    },
    /// Switch to the next group (alphabetical).
    Next {
        /// Target output (default: focused output).
        #[arg(short, long)]
        output: Option<String>,
        /// Wrap around past the last group.
        #[arg(short, long)]
        wrap: bool,
    },
    /// Switch to the next group in the current output's group list.
    NextOnOutput {
        /// Target output (default: focused output).
        #[arg(short, long)]
        output: Option<String>,
        /// Wrap around past the last group.
        #[arg(short, long)]
        wrap: bool,
    },
    /// Switch to the previous group (alphabetical).
    Prev {
        /// Target output (default: focused output).
        #[arg(short, long)]
        output: Option<String>,
        /// Wrap around past the first group.
        #[arg(short, long)]
        wrap: bool,
    },
    /// Switch to the previous group in the current output's group list.
    PrevOnOutput {
        /// Target output (default: focused output).
        #[arg(short, long)]
        output: Option<String>,
        /// Wrap around past the first group.
        #[arg(short, long)]
        wrap: bool,
    },
    /// Delete all empty groups.
    Prune {
        /// Do not delete these groups even if they are empty.
        #[arg(long)]
        keep: Vec<String>,
    },
    /// Unhide all hidden workspaces in a group.
    UnhideAll {
        /// Group (default: active group on focused output).
        group: Option<String>,
    },
}

#[derive(Subcommand)]
enum WorkspaceAction {
    /// List workspaces.
    List {
        /// Filter by output.
        #[arg(short, long)]
        output: Option<String>,
        /// Filter by group.
        #[arg(short, long)]
        group: Option<String>,
        /// Only show workspaces visible in the active group.
        #[arg(long)]
        visible: bool,
        /// Plain output (space-separated names) instead of formatted list.
        #[arg(long)]
        plain: bool,
        /// Include group membership per workspace.
        #[arg(long)]
        groups: bool,
        /// One workspace per line.
        #[arg(long)]
        flatten: bool,
    },
    /// Add a workspace to a group.
    Add {
        /// Workspace name.
        workspace: String,
        /// Group (default: active group on focused output).
        #[arg(short, long)]
        group: Option<String>,
    },
    /// Move a workspace to a set of groups (removing from all others).
    Move {
        /// Workspace name.
        workspace: String,
        /// Comma-separated list of target groups.
        #[arg(short, long)]
        groups: String,
    },
    /// Remove a workspace from a group.
    Remove {
        /// Workspace name.
        workspace: String,
        /// Group (default: active group on focused output).
        #[arg(short, long)]
        group: Option<String>,
    },
    /// Rename a workspace. If the target exists, merges source into target.
    Rename {
        /// Current workspace name.
        old_name: String,
        /// New workspace name.
        new_name: String,
    },
    /// List groups a workspace belongs to.
    Groups {
        /// Workspace name.
        workspace: String,
    },
    /// Mark a workspace as global (visible in all groups).
    Global {
        /// Workspace name (default: focused workspace).
        workspace: Option<String>,
        /// Toggle current state instead of always setting true.
        #[arg(short, long)]
        toggle: bool,
    },
    /// Remove global status from a workspace.
    Unglobal {
        /// Workspace name (default: focused workspace).
        workspace: Option<String>,
    },
    /// Mark a workspace as hidden in a specific group.
    Hide {
        /// Workspace name (default: focused workspace).
        workspace: Option<String>,
        /// Group (default: active group on focused output).
        #[arg(short, long)]
        group: Option<String>,
        /// Toggle current hidden state instead of always setting true.
        #[arg(short, long)]
        toggle: bool,
    },
    /// Remove the hidden flag from a workspace in a group.
    Unhide {
        /// Workspace name (default: focused workspace).
        workspace: Option<String>,
        /// Group (default: active group on focused output).
        #[arg(short, long)]
        group: Option<String>,
    },
    /// Toggle or enable the global show_hidden_workspaces DB flag.
    ///
    /// When true, hidden workspaces are emitted to waybar with the CSS class
    /// "hidden" and included in navigation. Default: false.
    ShowHidden {
        /// Toggle current state; otherwise sets true.
        #[arg(short, long)]
        toggle: bool,
    },
}

#[derive(Subcommand)]
enum NavAction {
    /// Focus the next visible workspace.
    Next {
        /// Target output (default: focused output).
        #[arg(short, long)]
        output: Option<String>,
        /// Wrap around past the last workspace.
        #[arg(short, long)]
        wrap: bool,
        /// Include workspaces from all outputs.
        #[arg(long)]
        all_outputs: bool,
    },
    /// Focus the next visible workspace on the same output.
    NextOnOutput {
        /// Wrap around past the last workspace on this output.
        #[arg(short, long)]
        wrap: bool,
    },
    /// Focus the previous visible workspace.
    Prev {
        /// Target output (default: focused output).
        #[arg(short, long)]
        output: Option<String>,
        /// Wrap around past the first workspace.
        #[arg(short, long)]
        wrap: bool,
        /// Include workspaces from all outputs.
        #[arg(long)]
        all_outputs: bool,
    },
    /// Focus the previous visible workspace on the same output.
    PrevOnOutput {
        /// Wrap around past the first workspace on this output.
        #[arg(short, long)]
        wrap: bool,
    },
    /// Focus a specific workspace (works even for hidden workspaces).
    Go {
        /// Target workspace name.
        workspace: String,
        /// Move workspace to this output if it doesn't exist yet.
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Focus the previously focused workspace.
    Back,
}

#[derive(Subcommand)]
enum ContainerAction {
    /// Move the focused container to a workspace.
    Move {
        /// Target workspace name.
        workspace: String,
        /// After moving, follow the container by switching to that workspace.
        #[arg(long)]
        switch_to_workspace: bool,
    },
}

pub async fn run(
    cli: Cli,
    group_service: &GroupService,
    workspace_service: &WorkspaceService,
    waybar_sync: &WaybarSyncService,
    nav_service: &NavigationService,
    ipc_client: &SwayIpcClient,
    db_path: PathBuf,
) -> anyhow::Result<()> {
    match cli.command {
        Command::Group { action } => run_group(action, group_service, workspace_service, waybar_sync, ipc_client).await?,
        Command::Workspace { action } => run_workspace(action, workspace_service, group_service, waybar_sync, ipc_client).await?,
        Command::Nav { action } => run_nav(action, nav_service, workspace_service, waybar_sync, ipc_client).await?,
        Command::Container { action } => run_container(action, nav_service, group_service, workspace_service, waybar_sync, ipc_client).await?,
        Command::Sync { all, workspaces, groups, outputs, repair, init_bars, init_bars_retries, init_bars_delay_ms } => {
            run_sync(all, workspaces, groups, outputs, repair, init_bars, init_bars_retries, init_bars_delay_ms, workspace_service, group_service, waybar_sync).await?;
        }
        Command::Init { restart_daemon_service } => {
            run_init(db_path, workspace_service, group_service, waybar_sync, restart_daemon_service).await?;
        }
        Command::Repair => {
            run_repair(workspace_service, group_service, waybar_sync, ipc_client).await?;
        }
        Command::Status => {
            run_status(group_service, workspace_service, waybar_sync, ipc_client).await?;
        }
        Command::Config { action } => {
            run_config(action)?;
        }
    }
    Ok(())
}

fn resolve_output(output: Option<&str>, ipc_client: &SwayIpcClient) -> anyhow::Result<String> {
    match output {
        Some(o) => Ok(o.to_string()),
        None => {
            let primary = ipc_client.get_primary_output()?;
            Ok(primary)
        }
    }
}

async fn resolve_group_output(
    explicit_output: Option<&str>,
    group: &str,
    group_service: &GroupService,
    ipc_client: &SwayIpcClient,
) -> anyhow::Result<String> {
    if let Some(output) = explicit_output {
        return Ok(output.to_string());
    }
    if let Some(output) = group_service.find_last_visited_output(group).await? {
        return Ok(output);
    }
    Ok(ipc_client.get_primary_output()?)
}

async fn run_group(
    action: GroupAction,
    group_service: &GroupService,
    workspace_service: &WorkspaceService,
    waybar_sync: &WaybarSyncService,
    ipc_client: &SwayIpcClient,
) -> anyhow::Result<()> {
    match action {
        GroupAction::List { output } => {
            let groups = group_service.list_groups(output.as_deref()).await?;
            if groups.is_empty() {
                println!("No groups found.");
            } else {
                for group in &groups {
                    println!("Group \"{}\":", group.name);
                    if group.workspaces.is_empty() {
                        println!("  (empty)");
                    } else {
                        for ws in &group.workspaces {
                            println!("  - {}", ws);
                        }
                    }
                }
            }
        }
        GroupAction::Create { name } => {
            group_service.create_group(&name).await?;
            waybar_sync.update_waybar_groups().await?;
            println!("Created group \"{}\"", name);
        }
        GroupAction::Delete { name, force } => {
            group_service.delete_group(&name, force).await?;
            waybar_sync.update_waybar_groups().await?;
            println!("Deleted group \"{}\"", name);
        }
        GroupAction::Rename { old_name, new_name } => {
            group_service.rename_group(&old_name, &new_name).await?;
            waybar_sync.update_waybar_groups().await?;
            println!("Renamed group \"{}\" to \"{}\"", old_name, new_name);
        }
        GroupAction::Select { output, group, create } => {
            if create {
                let groups = group_service.list_all_group_names().await?;
                if !groups.iter().any(|g| g == &group) {
                    group_service.create_group(&group).await?;
                    println!("Created group \"{}\"", group);
                }
            }
            let resolved_output = resolve_group_output(output.as_deref(), &group, group_service, ipc_client).await?;
            group_service.set_active_group(&resolved_output, &group).await?;
            waybar_sync.update_waybar().await?;
            waybar_sync.update_waybar_groups().await?;
            println!("Set active group for \"{}\" to \"{}\"", resolved_output, group);
        }
        GroupAction::Active { output } => {
            let active = group_service.get_active_group(&output).await.unwrap_or(None);
            println!("{}", active.unwrap_or_default());
        }
        GroupAction::Next { output, wrap } => {
            let current_output = resolve_output(output.as_deref(), ipc_client)?;
            if let Some(next_name) = group_service.next_group_name(&current_output, wrap).await? {
                let resolved_output = resolve_group_output(None, &next_name, group_service, ipc_client).await?;
                group_service.set_active_group(&resolved_output, &next_name).await?;
                waybar_sync.update_waybar().await?;
                waybar_sync.update_waybar_groups().await?;
                println!("Switched from active group to \"{}\"", next_name);
            }
        }
        GroupAction::NextOnOutput { output, wrap } => {
            let output = resolve_output(output.as_deref(), ipc_client)?;
            if let Some(next) = group_service.next_group_on_output(&output, wrap).await? {
                waybar_sync.update_waybar().await?;
                waybar_sync.update_waybar_groups().await?;
                println!("Switched from active group to \"{}\"", next);
            }
        }
        GroupAction::Prev { output, wrap } => {
            let current_output = resolve_output(output.as_deref(), ipc_client)?;
            if let Some(prev_name) = group_service.prev_group_name(&current_output, wrap).await? {
                let resolved_output = resolve_group_output(None, &prev_name, group_service, ipc_client).await?;
                group_service.set_active_group(&resolved_output, &prev_name).await?;
                waybar_sync.update_waybar().await?;
                waybar_sync.update_waybar_groups().await?;
                println!("Switched from active group to \"{}\"", prev_name);
            }
        }
        GroupAction::PrevOnOutput { output, wrap } => {
            let output = resolve_output(output.as_deref(), ipc_client)?;
            if let Some(prev) = group_service.prev_group_on_output(&output, wrap).await? {
                waybar_sync.update_waybar().await?;
                waybar_sync.update_waybar_groups().await?;
                println!("Switched from active group to \"{}\"", prev);
            }
        }
        GroupAction::Prune { keep } => {
            let removed = group_service.prune_groups(&keep).await?;
            if removed == 0 {
                println!("No empty groups to prune.");
            } else {
                waybar_sync.update_waybar_groups().await?;
                println!("Pruned {} empty group(s)", removed);
            }
        }
        GroupAction::UnhideAll { group } => {
            let group_name = match resolve_group(group.as_deref(), group_service, ipc_client).await? {
                Some(g) => g,
                None => return Ok(()),
            };
            let removed = workspace_service.unhide_all_in_group(&group_name).await?;
            waybar_sync.update_waybar().await?;
            if removed == 0 {
                println!("No hidden workspaces in group \"{}\"", group_name);
            } else {
                println!("Unhid {} workspace(s) in group \"{}\"", removed, group_name);
            }
        }
    }
    Ok(())
}

async fn run_workspace(
    action: WorkspaceAction,
    workspace_service: &WorkspaceService,
    group_service: &GroupService,
    waybar_sync: &WaybarSyncService,
    ipc_client: &SwayIpcClient,
) -> anyhow::Result<()> {
    match action {
        WorkspaceAction::List { output, group, visible, plain, groups, flatten } => {
            if visible {
                let output_name = output.as_deref()
                    .map(|s| s.to_string())
                    .or_else(|| ipc_client.get_primary_output().ok())
                    .unwrap_or_default();
                let workspaces = workspace_service.list_visible_workspaces(&output_name).await?;
                if workspaces.is_empty() {
                    if !plain {
                        println!("No visible workspaces found.");
                    }
                } else {
                    for ws in &workspaces {
                        println!("{}", ws);
                    }
                }
            } else {
                let workspaces = workspace_service.list_workspaces(output.as_deref(), group.as_deref()).await?;
                if workspaces.is_empty() {
                    if !plain {
                        println!("No workspaces found.");
                    }
                } else {
                    let active_group_name = if group.is_none() {
                        let output_name = output.as_deref()
                            .map(|s| s.to_string())
                            .or_else(|| ipc_client.get_primary_output().ok());
                        match output_name {
                            Some(ref out) => group_service.get_active_group(out).await.ok(),
                            None => None,
                        }
                    } else {
                        None
                    };

                    if !plain {
                        let group_label = group.as_deref().unwrap_or("active");
                        let output_label = output.as_deref().unwrap_or("all");
                        println!("Workspaces in group \"{}\" on \"{}\":", group_label, output_label);
                    }
                    for ws in &workspaces {
                        if plain && groups && flatten {
                            let mut sorted_groups: Vec<&String> = ws.groups.iter().collect();
                            sorted_groups.sort_by(|a, b| {
                                if let Some(ref active) = active_group_name {
                                    if active.as_deref() == Some(a.as_str()) { return std::cmp::Ordering::Less; }
                                    if active.as_deref() == Some(b.as_str()) { return std::cmp::Ordering::Greater; }
                                }
                                a.cmp(b)
                            });
                            for g in &sorted_groups {
                                println!("{}│{}", ws.name, g);
                            }
                        } else if plain && groups {
                            let groups_str = ws.groups.join(",");
                            if groups_str.is_empty() {
                                println!("{}│", ws.name);
                            } else {
                                println!("{}│{}", ws.name, groups_str);
                            }
                        } else if plain {
                            println!("{}", ws.name);
                        } else {
                            let status = if ws.is_global {
                                "(global)"
                            } else if let Some(ref active) = active_group_name {
                                if ws.groups.iter().any(|g| Some(g.as_str()) == active.as_deref()) {
                                    "(visible)"
                                } else if !ws.groups.is_empty() {
                                    "(hidden)"
                                } else {
                                    "(visible)"
                                }
                            } else {
                                ""
                            };
                            println!("  {:20} {}", ws.name, status);
                        }
                    }
                }
            }
        }
        WorkspaceAction::Add { workspace, group } => {
            let target_group = match &group {
                Some(g) => g.clone(),
                None => {
                    let output_name = ipc_client.get_primary_output().ok();
                    match output_name {
                        Some(ref out) => {
                            match group_service.get_active_group(out).await.unwrap_or(None) {
                                Some(g) => g,
                                None => {
                                    eprintln!("No active group for output '{}'. Specify a group explicitly.", out);
                                    return Ok(());
                                }
                            }
                        }
                        None => {
                            eprintln!("Cannot determine output. Specify a group explicitly.");
                            return Ok(());
                        }
                    }
                }
            };
            workspace_service.add_to_group(&workspace, &target_group).await?;
            waybar_sync.update_waybar().await?;
            println!("Added workspace \"{}\" to group \"{}\"", workspace, target_group);
        }
        WorkspaceAction::Move { workspace, groups } => {
            let target_groups: Vec<&str> = groups.split(',').map(|g| g.trim()).filter(|g| !g.is_empty()).collect();
            if target_groups.is_empty() {
                anyhow::bail!("No groups specified for move. Use --groups <group1,group2,...>");
            }
            workspace_service.move_to_groups(&workspace, &target_groups).await?;
            waybar_sync.update_waybar().await?;
            println!("Moved workspace \"{}\" to group(s): {}", workspace, target_groups.join(", "));
        }
        WorkspaceAction::Remove { workspace, group } => {
            let source_group = match &group {
                Some(g) => g.clone(),
                None => {
                    let output_name = ipc_client.get_primary_output().ok();
                    match output_name {
                        Some(ref out) => {
                            match group_service.get_active_group(out).await.unwrap_or(None) {
                                Some(g) => g,
                                None => {
                                    eprintln!("No active group for output '{}'. Specify a group explicitly.", out);
                                    return Ok(());
                                }
                            }
                        }
                        None => {
                            eprintln!("Cannot determine output. Specify a group explicitly.");
                            return Ok(());
                        }
                    }
                }
            };
            workspace_service.remove_from_group(&workspace, &source_group).await?;
            waybar_sync.update_waybar().await?;
            println!("Removed workspace \"{}\" from group \"{}\"", workspace, source_group);
        }
        WorkspaceAction::Rename { old_name, new_name } => {
            let pending_id = workspace_service.register_pending_event(&new_name, "rename").await.ok();
            let result: anyhow::Result<()> = async {
                let merged = workspace_service.rename_workspace(&old_name, &new_name).await?;
                waybar_sync.update_waybar().await?;
                if merged {
                    println!("Merged workspace \"{}\" into \"{}\"", old_name, new_name);
                } else {
                    println!("Renamed workspace \"{}\" to \"{}\"", old_name, new_name);
                }
                Ok(())
            }.await;
            if let Some(id) = pending_id {
                workspace_service.remove_pending_event(id).await.ok();
            }
            result?;
        }
        WorkspaceAction::Global { workspace, toggle } => {
            let ws = match workspace {
                Some(w) => w,
                None => ipc_client.get_focused_workspace().map(|ws| ws.name).unwrap_or_default(),
            };
            let make_global = if toggle {
                !workspace_service.is_global(&ws).await.unwrap_or(false)
            } else {
                true
            };
            workspace_service.set_global(&ws, make_global).await?;
            waybar_sync.update_waybar().await?;
            if make_global {
                println!("Marked workspace \"{}\" as global", ws);
            } else {
                println!("Removed global status from workspace \"{}\"", ws);
            }
        }
        WorkspaceAction::Unglobal { workspace } => {
            let ws = match workspace {
                Some(w) => w,
                None => ipc_client.get_focused_workspace().map(|ws| ws.name).unwrap_or_default(),
            };
            workspace_service.set_global(&ws, false).await?;
            waybar_sync.update_waybar().await?;
            println!("Removed global status from workspace \"{}\"", ws);
        }
        WorkspaceAction::Groups { workspace } => {
            let groups = workspace_service.get_groups_for_workspace(&workspace).await?;
            if groups.is_empty() {
                println!("Workspace \"{}\" is not in any group.", workspace);
            } else {
                println!("Workspace \"{}\" is in groups: {}", workspace,
                    groups.iter().map(|g| format!("\"{}\"", g)).collect::<Vec<_>>().join(", "));
            }
        }
        WorkspaceAction::Hide { workspace, group, toggle } => {
            let ws = match workspace {
                Some(w) => w,
                None => ipc_client.get_focused_workspace().map(|w| w.name).unwrap_or_default(),
            };
            let group_name = match resolve_group(group.as_deref(), group_service, ipc_client).await? {
                Some(g) => g,
                None => return Ok(()),
            };
            let new_state = if toggle {
                !workspace_service.is_hidden(&ws, &group_name).await.unwrap_or(false)
            } else {
                true
            };
            workspace_service.set_hidden(&ws, &group_name, new_state).await?;
            waybar_sync.update_waybar().await?;
            if new_state {
                println!("Hid workspace \"{}\" in group \"{}\"", ws, group_name);
            } else {
                println!("Unhid workspace \"{}\" in group \"{}\"", ws, group_name);
            }
        }
        WorkspaceAction::Unhide { workspace, group } => {
            let ws = match workspace {
                Some(w) => w,
                None => ipc_client.get_focused_workspace().map(|w| w.name).unwrap_or_default(),
            };
            let group_name = match resolve_group(group.as_deref(), group_service, ipc_client).await? {
                Some(g) => g,
                None => return Ok(()),
            };
            workspace_service.set_hidden(&ws, &group_name, false).await?;
            waybar_sync.update_waybar().await?;
            println!("Unhid workspace \"{}\" in group \"{}\"", ws, group_name);
        }
        WorkspaceAction::ShowHidden { toggle } => {
            let new_value = if toggle {
                !workspace_service.get_show_hidden().await?
            } else {
                true
            };
            workspace_service.set_show_hidden(new_value).await?;
            waybar_sync.update_waybar().await?;
            println!("show_hidden_workspaces = {}", new_value);
        }
    }
    Ok(())
}

/// Resolve a group argument: if `Some`, returns it as-is; otherwise resolves to
/// the active group of the focused output. Prints an error and returns `None`
/// if resolution fails.
async fn resolve_group(
    explicit: Option<&str>,
    group_service: &GroupService,
    ipc_client: &SwayIpcClient,
) -> anyhow::Result<Option<String>> {
    if let Some(g) = explicit {
        return Ok(Some(g.to_string()));
    }
    let output = match ipc_client.get_primary_output().ok() {
        Some(o) => o,
        None => {
            eprintln!("Cannot determine output. Specify a group explicitly.");
            return Ok(None);
        }
    };
    match group_service.get_active_group(&output).await.unwrap_or(None) {
        Some(g) => Ok(Some(g)),
        None => {
            eprintln!("No active group for output '{}'. Specify a group explicitly.", output);
            Ok(None)
        }
    }
}

async fn run_nav(
    action: NavAction,
    nav_service: &NavigationService,
    workspace_service: &WorkspaceService,
    waybar_sync: &WaybarSyncService,
    ipc_client: &SwayIpcClient,
) -> anyhow::Result<()> {
    match action {
        NavAction::Next { output, wrap, all_outputs } => {
            let output = resolve_output(output.as_deref(), ipc_client)?;
            if all_outputs {
                if let Some(target) = nav_service.next_workspace_all_outputs(&output, wrap).await? {
                    waybar_sync.update_waybar().await?;
                    println!("Navigated to \"{}\"", target);
                }
            } else if let Some(target) = nav_service.next_workspace(&output, wrap).await? {
                waybar_sync.update_waybar().await?;
                println!("Navigated to \"{}\"", target);
            }
        }
        NavAction::NextOnOutput { wrap } => {
            if let Some(target) = nav_service.next_workspace_global(wrap).await? {
                waybar_sync.update_waybar().await?;
                println!("Navigated to \"{}\"", target);
            }
        }
        NavAction::Prev { output, wrap, all_outputs } => {
            let output = resolve_output(output.as_deref(), ipc_client)?;
            if all_outputs {
                if let Some(target) = nav_service.prev_workspace_all_outputs(&output, wrap).await? {
                    waybar_sync.update_waybar().await?;
                    println!("Navigated to \"{}\"", target);
                }
            } else if let Some(target) = nav_service.prev_workspace(&output, wrap).await? {
                waybar_sync.update_waybar().await?;
                println!("Navigated to \"{}\"", target);
            }
        }
        NavAction::PrevOnOutput { wrap } => {
            if let Some(target) = nav_service.prev_workspace_global(wrap).await? {
                waybar_sync.update_waybar().await?;
                println!("Navigated to \"{}\"", target);
            }
        }
        NavAction::Go { workspace, output: _ } => {
            let is_new_workspace = ipc_client.get_workspaces()
                .map(|ws| !ws.iter().any(|w| w.name == workspace))
                .unwrap_or(true);

            let pending_id = if is_new_workspace {
                workspace_service.register_pending_event(&workspace, "add").await.ok()
            } else {
                None
            };

            let result: anyhow::Result<()> = async {
                nav_service.go_workspace(&workspace).await?;
                waybar_sync.update_waybar().await?;
                println!("Navigated to \"{}\"", workspace);
                Ok(())
            }.await;

            if let Some(id) = pending_id {
                workspace_service.remove_pending_event(id).await.ok();
            }

            result?;
        }
        NavAction::Back => {
            if let Some(target) = nav_service.go_back().await? {
                waybar_sync.update_waybar().await?;
                println!("Navigated back to \"{}\"", target);
            } else {
                println!("No previous workspace found.");
            }
        }
    }
    Ok(())
}

async fn run_container(
    action: ContainerAction,
    nav_service: &NavigationService,
    group_service: &GroupService,
    workspace_service: &WorkspaceService,
    waybar_sync: &WaybarSyncService,
    ipc_client: &SwayIpcClient,
) -> anyhow::Result<()> {
    match action {
        ContainerAction::Move { workspace, switch_to_workspace } => {
            let is_new_workspace = ipc_client.get_workspaces()
                .map(|ws| !ws.iter().any(|w| w.name == workspace))
                .unwrap_or(true);

            let pending_id = if is_new_workspace {
                workspace_service.register_pending_event(&workspace, "add").await.ok()
            } else {
                None
            };

            let result: anyhow::Result<()> = async {
                nav_service.move_to_workspace(&workspace).await?;

                if switch_to_workspace {
                    nav_service.focus_workspace(&workspace).await?;

                    if let Ok(groups) = workspace_service.get_groups_for_workspace(&workspace).await
                        && !groups.is_empty()
                            && let Ok(output) = ipc_client.get_primary_output() {
                                let current = group_service.get_active_group(&output).await.unwrap_or(None);
                                if !groups.iter().any(|g| current.as_deref() == Some(g.as_str())) {
                                    group_service.set_active_group_db_only(&output, &groups[0]).await?;
                                }
                            }
                }

                waybar_sync.update_waybar().await?;
                println!("Moved container to \"{}\"", workspace);
                Ok(())
            }.await;

            if let Some(id) = pending_id {
                workspace_service.remove_pending_event(id).await.ok();
            }

            result?;
        }
    }
    Ok(())
}

// Args mirror the `Command::Sync` clap variant 1:1; bundling them into a
// struct would just add an extra layer of packing/unpacking.
#[allow(clippy::too_many_arguments)]
async fn run_sync(
    all: bool,
    workspaces: bool,
    groups: bool,
    outputs: bool,
    repair: bool,
    init_bars: bool,
    init_bars_retries: u32,
    init_bars_delay_ms: u64,
    workspace_service: &WorkspaceService,
    group_service: &GroupService,
    waybar_sync: &WaybarSyncService,
) -> anyhow::Result<()> {
    let mut synced_ws = false;
    let mut synced_gr = false;
    let mut synced_out = false;

    if all || workspaces {
        workspace_service.sync_from_sway().await?;
        synced_ws = true;
    }
    if all || groups {
        synced_gr = true;
    }
    if all || outputs {
        synced_out = true;
    }

    if !all && !workspaces && !groups && !outputs && !repair && !init_bars {
        workspace_service.sync_from_sway().await?;
        synced_ws = true;
        synced_gr = true;
        synced_out = true;
    }

    if repair {
        let (removed_ws, added_ws, removed_groups) = workspace_service.repair(group_service).await?;
        println!("Repair complete:");
        println!("  Workspaces removed from DB: {}", removed_ws);
        if added_ws > 0 {
            let dg = group_service.default_group().to_string();
            if group_service.list_groups(None).await?.iter().all(|g| g.name != dg) {
                group_service.create_group(&dg).await?;
            }
            println!("  Workspaces added to group '{}': {}", dg, added_ws);
        }
        println!("  Empty groups removed: {}", removed_groups);
    }

    if init_bars {
        let delay = std::time::Duration::from_millis(init_bars_delay_ms);
        waybar_sync.update_waybar_with_retry(init_bars_retries, delay).await?;
        waybar_sync.update_waybar_groups_with_retry(init_bars_retries, delay).await?;
        println!("Bars initialized (retries={}, delay={}ms).", init_bars_retries, init_bars_delay_ms);
    } else {
        waybar_sync.update_waybar().await?;
        waybar_sync.update_waybar_groups().await?;
    }

    let mut parts = Vec::new();
    if synced_ws {
        parts.push("workspaces");
    }
    if synced_gr {
        parts.push("groups");
    }
    if synced_out {
        parts.push("outputs");
    }
    if repair {
        parts.push("repair");
    }
    if init_bars {
        parts.push("bars");
    }
    println!("Synced: {}", parts.join(", "));

    Ok(())
}

async fn run_init(
    db_path: PathBuf,
    _workspace_service: &WorkspaceService,
    _group_service: &GroupService,
    _waybar_sync: &WaybarSyncService,
    restart_daemon_service: bool,
) -> anyhow::Result<()> {
    if db_path.exists() {
        std::fs::remove_file(&db_path)?;
        println!("Removed existing database.");
    }

    let db = sway_groups_core::db::DatabaseManager::new(db_path).await?;
    let ipc = sway_groups_core::sway::SwayIpcClient::new()?;
    let group_svc = GroupService::new(db.clone(), ipc.clone());
    let workspace_svc = WorkspaceService::new(db.clone(), ipc.clone());
    let waybar_sync_svc = WaybarSyncService::new(db.clone(), ipc.clone(), sway_groups_core::sway::WaybarClient::new());

    workspace_svc.sync_from_sway().await?;
    waybar_sync_svc.update_waybar().await?;
    waybar_sync_svc.update_waybar_groups().await?;

    if group_svc.list_groups(None).await?.is_empty() {
        let dg = group_svc.default_group().to_string();
        group_svc.create_group(&dg).await?;
        println!("Created default group '{}' (no groups found after sync).", dg);
    }

    println!("Initialized: created database, synced workspaces and outputs.");

    if restart_daemon_service {
        let result = std::process::Command::new("systemctl")
            .args(["--user", "restart", "swayg-daemon.service"])
            .output()?;
        if result.status.success() {
            println!("Restarted swayg-daemon service.");
        } else {
            eprintln!("Failed to restart swayg-daemon service: {}",
                String::from_utf8_lossy(&result.stderr));
        }
    }

    Ok(())
}

async fn run_repair(
    workspace_service: &WorkspaceService,
    group_service: &GroupService,
    waybar_sync: &WaybarSyncService,
    _ipc_client: &SwayIpcClient,
) -> anyhow::Result<()> {
    let (removed_ws, added_ws, removed_groups) = workspace_service.repair(group_service).await?;

    if added_ws > 0 {
        let dg = group_service.default_group().to_string();
        if group_service.list_groups(None).await?.iter().all(|g| g.name != dg) {
            group_service.create_group(&dg).await?;
        }
    }
    waybar_sync.update_waybar().await?;

    println!("Repair complete:");
    println!("  Workspaces removed from DB: {}", removed_ws);
    println!("  Workspaces added to group '0': {}", added_ws);
    println!("  Empty groups removed: {}", removed_groups);

    Ok(())
}

async fn run_status(
    group_service: &GroupService,
    workspace_service: &WorkspaceService,
    _waybar_sync: &WaybarSyncService,
    ipc_client: &SwayIpcClient,
) -> anyhow::Result<()> {
    let outputs = ipc_client.get_outputs()?;

    let show_hidden = workspace_service.get_show_hidden().await.unwrap_or(false);
    println!("show_hidden_workspaces = {}", show_hidden);

    for output in &outputs {
        let active_group = group_service.get_active_group(&output.name).await
            .unwrap_or(None);
        println!("{}: active group = \"{}\"", output.name, active_group.as_deref().unwrap_or("(none)"));

        let visible = workspace_service.list_visible_workspaces(&output.name).await?;

        let all_ws = workspace_service.list_workspaces(Some(&output.name), None).await?;

        let mut inactive = Vec::new();
        let mut global_ws = Vec::new();
        let mut visible_names = Vec::new();
        let mut hidden_names: Vec<String> = Vec::new();

        for ws in &all_ws {
            if ws.is_global {
                global_ws.push(ws.name.clone());
            }
        }

        // Determine user-hidden workspaces for the active group (if any).
        if let Some(ref ag) = active_group {
            for ws in &all_ws {
                if workspace_service.is_hidden(&ws.name, ag).await.unwrap_or(false) {
                    hidden_names.push(ws.name.clone());
                }
            }
        }

        for ws in &visible {
            if all_ws.iter().any(|w| w.name == *ws) {
                if all_ws.iter().any(|w| w.name == *ws && w.is_global) {
                    global_ws.push(ws.clone());
                } else {
                    visible_names.push(ws.clone());
                }
            }
        }

        let sway_workspaces = ipc_client.get_workspaces()?;
        let sway_names: std::collections::HashSet<String> = sway_workspaces.iter()
            .filter(|w| w.output == output.name)
            .map(|w| w.name.clone())
            .collect();

        // "Inactive" = workspaces on this output that belong to other groups
        // (formerly called "Hidden"). Excludes user-hidden and globals.
        for ws in &all_ws {
            if ws.is_global {
                continue;
            }
            if visible_names.iter().any(|v| v == &ws.name) {
                continue;
            }
            if hidden_names.iter().any(|h| h == &ws.name) {
                continue;
            }
            if sway_names.contains(&ws.name) {
                inactive.push(ws.name.clone());
            }
        }

        visible_names.sort();
        inactive.sort();
        hidden_names.sort();
        global_ws.sort();
        global_ws.dedup();

        println!("  Visible:  {}", if visible_names.is_empty() { "(none)".to_string() } else { visible_names.join(", ") });
        println!("  Inactive: {}", if inactive.is_empty() { "(none)".to_string() } else { inactive.join(", ") });
        println!("  Hidden:   {}", if hidden_names.is_empty() { "(none)".to_string() } else { hidden_names.join(", ") });
        if !global_ws.is_empty() {
            println!("  Global:   {}", global_ws.join(", "));
        }
    }

    Ok(())
}

fn run_config(action: ConfigAction) -> anyhow::Result<()> {
    let config = sway_groups_config::SwaygConfig::default();
    match action {
        ConfigAction::Dump { output } => {
            match output {
                Some(path) => {
                    config.dump_to(&path)?;
                    println!("Wrote default config to {}", path.display());
                }
                None => {
                    print!("{}", config.dump()?);
                }
            }
        }
    }
    Ok(())
}
