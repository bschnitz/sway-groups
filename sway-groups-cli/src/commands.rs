//! CLI commands for swayg.

use clap::{Parser, Subcommand};
use sway_groups_core::services::{GroupService, NavigationService, SuffixService, WorkspaceService};
use sway_groups_core::sway::SwayIpcClient;

/// Sway workspace groups management CLI.
#[derive(Parser)]
#[command(name = "swayg")]
#[command(author, version, about = "Sway workspace groups management CLI")]
pub struct Cli {
    /// Enable verbose output.
    #[arg(short, long)]
    pub verbose: bool,

    #[command(subcommand)]
    command: Command,
}

/// Available commands.
#[derive(Subcommand)]
enum Command {
    /// Group management commands.
    Group {
        #[command(subcommand)]
        action: GroupAction,
    },
    /// Workspace management commands.
    Workspace {
        #[command(subcommand)]
        action: WorkspaceAction,
    },
    /// Navigation commands.
    Nav {
        #[command(subcommand)]
        action: NavAction,
    },
    /// Sync database with sway state.
    Sync {
        /// Sync everything.
        #[arg(short, long)]
        all: bool,

        /// Sync only workspaces.
        #[arg(short, long)]
        workspaces: bool,

        /// Sync only groups.
        #[arg(short, long)]
        groups: bool,

        /// Sync only outputs.
        #[arg(short, long)]
        outputs: bool,
    },
    /// Show current status.
    Status,
}

/// Group subcommands.
#[derive(Subcommand)]
enum GroupAction {
    /// List all groups.
    List {
        /// Filter by output name.
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Create a new group.
    Create {
        /// Group name.
        name: String,
    },
    /// Delete a group.
    Delete {
        /// Group name.
        name: String,

        /// Force delete even with workspaces.
        #[arg(short, long)]
        force: bool,
    },
    /// Rename a group.
    Rename {
        /// Current name.
        old_name: String,

        /// New name.
        new_name: String,
    },
    /// Set active group for an output.
    Select {
        /// Output name.
        output: String,

        /// Group name.
        group: String,
    },
    /// Show active group for an output.
    Active {
        /// Output name.
        output: String,
    },
    /// Switch to next group (all groups).
    Next {
        /// Output name.
        #[arg(short, long)]
        output: Option<String>,

        /// Wrap around.
        #[arg(short, long)]
        wrap: bool,
    },
    /// Switch to next non-empty group on the output.
    NextOnOutput {
        /// Output name.
        #[arg(short, long)]
        output: Option<String>,

        /// Wrap around.
        #[arg(short, long)]
        wrap: bool,
    },
    /// Switch to previous group (all groups).
    Prev {
        /// Output name.
        #[arg(short, long)]
        output: Option<String>,

        /// Wrap around.
        #[arg(short, long)]
        wrap: bool,
    },
    /// Switch to previous non-empty group on the output.
    PrevOnOutput {
        /// Output name.
        #[arg(short, long)]
        output: Option<String>,

        /// Wrap around.
        #[arg(short, long)]
        wrap: bool,
    },
    /// Remove empty groups.
    Prune {
        /// Groups to keep.
        #[arg(long)]
        keep: Vec<String>,
    },
}

/// Workspace subcommands.
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

        /// Show only workspaces visible in the active group.
        #[arg(long)]
        visible: bool,

        /// Plain output: workspace names only, one per line.
        #[arg(long)]
        plain: bool,
    },
    /// Add workspace to group.
    Add {
        /// Workspace name or number.
        workspace: String,

        /// Target group.
        #[arg(short, long)]
        group: Option<String>,

        /// Output.
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Move workspace to groups (comma-separated), removing from all other groups.
    /// Groups are created automatically if they don't exist.
    Move {
        /// Workspace name or number.
        workspace: String,

        /// Target groups (comma-separated).
        #[arg(short, long)]
        groups: String,
    },
    /// Remove workspace from group.
    Remove {
        /// Workspace name.
        workspace: String,

        /// Source group.
        #[arg(short, long)]
        group: Option<String>,
    },
    /// Mark workspace as global.
    Global {
        /// Workspace name.
        workspace: String,
    },
    /// Remove global status.
    Unglobal {
        /// Workspace name.
        workspace: String,
    },
    /// Show groups for workspace.
    Groups {
        /// Workspace name.
        workspace: String,
    },
}

/// Navigation subcommands.
#[derive(Subcommand)]
enum NavAction {
    /// Go to next workspace in active group on output.
    Next {
        /// Output name.
        #[arg(short, long)]
        output: Option<String>,

        /// Wrap around.
        #[arg(short, long)]
        wrap: bool,
    },
    /// Go to next workspace globally (across all outputs).
    NextOnOutput {
        /// Wrap around.
        #[arg(short, long)]
        wrap: bool,
    },
    /// Go to previous workspace in active group on output.
    Prev {
        /// Output name.
        #[arg(short, long)]
        output: Option<String>,

        /// Wrap around.
        #[arg(short, long)]
        wrap: bool,
    },
    /// Go to previous workspace globally (across all outputs).
    PrevOnOutput {
        /// Wrap around.
        #[arg(short, long)]
        wrap: bool,
    },
    /// Go to specific workspace.
    Go {
        /// Workspace name or number.
        workspace: String,

        /// Output name.
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Move focused container to a specific workspace.
    MoveTo {
        /// Workspace name or number.
        workspace: String,
    },
    /// Go back to previous workspace.
    Back,
}

/// Run the CLI commands.
pub async fn run(
    cli: Cli,
    group_service: &GroupService,
    workspace_service: &WorkspaceService,
    suffix_service: &SuffixService,
    nav_service: &NavigationService,
    ipc_client: &SwayIpcClient,
) -> anyhow::Result<()> {
    match cli.command {
        Command::Group { action } => run_group(action, group_service, ipc_client).await?,
        Command::Workspace { action } => run_workspace(action, workspace_service, group_service, suffix_service, ipc_client).await?,
        Command::Nav { action } => run_nav(action, nav_service, suffix_service, ipc_client).await?,
        Command::Sync { all, workspaces, groups, outputs } => {
            run_sync(all, workspaces, groups, outputs, workspace_service, suffix_service).await?;
        }
        Command::Status => {
            run_status(group_service, suffix_service, ipc_client).await?;
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

async fn run_group(
    action: GroupAction,
    group_service: &GroupService,
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
            println!("Created group \"{}\"", name);
        }
        GroupAction::Delete { name, force } => {
            group_service.delete_group(&name, force).await?;
            println!("Deleted group \"{}\"", name);
        }
        GroupAction::Rename { old_name, new_name } => {
            group_service.rename_group(&old_name, &new_name).await?;
            println!("Renamed group \"{}\" to \"{}\"", old_name, new_name);
        }
        GroupAction::Select { output, group } => {
            group_service.set_active_group(&output, &group).await?;
            println!("Set active group for \"{}\" to \"{}\"", output, group);
        }
        GroupAction::Active { output } => {
            let active = group_service.get_active_group(&output).await.unwrap_or_else(|_| "0".to_string());
            println!("{}", active);
        }
        GroupAction::Next { output, wrap } => {
            let output = resolve_output(output.as_deref(), ipc_client)?;
            if let Some(next) = group_service.next_group(&output, wrap).await? {
                println!("Switched from active group to \"{}\"", next);
            }
        }
        GroupAction::NextOnOutput { output, wrap } => {
            let output = resolve_output(output.as_deref(), ipc_client)?;
            if let Some(next) = group_service.next_group_on_output(&output, wrap).await? {
                println!("Switched from active group to \"{}\"", next);
            }
        }
        GroupAction::Prev { output, wrap } => {
            let output = resolve_output(output.as_deref(), ipc_client)?;
            if let Some(prev) = group_service.prev_group(&output, wrap).await? {
                println!("Switched from active group to \"{}\"", prev);
            }
        }
        GroupAction::PrevOnOutput { output, wrap } => {
            let output = resolve_output(output.as_deref(), ipc_client)?;
            if let Some(prev) = group_service.prev_group_on_output(&output, wrap).await? {
                println!("Switched from active group to \"{}\"", prev);
            }
        }
        GroupAction::Prune { keep } => {
            let removed = group_service.prune_groups(&keep).await?;
            if removed == 0 {
                println!("No empty groups to prune.");
            } else {
                println!("Pruned {} empty group(s)", removed);
            }
        }
    }
    Ok(())
}

async fn run_workspace(
    action: WorkspaceAction,
    workspace_service: &WorkspaceService,
    group_service: &GroupService,
    suffix_service: &SuffixService,
    ipc_client: &SwayIpcClient,
) -> anyhow::Result<()> {
    match action {
        WorkspaceAction::List { output, group, visible, plain } => {
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
                    let active_group_name = if !plain && group.is_none() {
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
                        if plain {
                            println!("{}", ws.name);
                        } else {
                            let status = if ws.is_global {
                                "(global)"
                            } else if let Some(ref active) = active_group_name {
                                if ws.groups.iter().any(|g| g == active) {
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
        WorkspaceAction::Add { workspace, group, output } => {
            let target_group = match &group {
                Some(g) => g.clone(),
                None => {
                    let output_name = output.as_deref()
                        .map(|o| o.to_string())
                        .or_else(|| ipc_client.get_primary_output().ok());
                    match output_name {
                        Some(ref out) => group_service.get_active_group(out).await.unwrap_or_else(|_| "0".to_string()),
                        None => "0".to_string(),
                    }
                }
            };
            workspace_service.add_to_group(&workspace, &target_group).await?;
            suffix_service.sync_all_suffixes().await?;
            println!("Added workspace \"{}\" to group \"{}\"", workspace, target_group);
        }
        WorkspaceAction::Move { workspace, groups } => {
            let target_groups: Vec<&str> = groups.split(',').map(|g| g.trim()).filter(|g| !g.is_empty()).collect();
            if target_groups.is_empty() {
                anyhow::bail!("No groups specified for move. Use --groups <group1,group2,...>");
            }
            workspace_service.move_to_groups(&workspace, &target_groups).await?;
            suffix_service.sync_all_suffixes().await?;
            println!("Moved workspace \"{}\" to group(s): {}", workspace, target_groups.join(", "));
        }
        WorkspaceAction::Remove { workspace, group } => {
            let source_group = match &group {
                Some(g) => g.clone(),
                None => {
                    let output_name = ipc_client.get_primary_output().ok();
                    match output_name {
                        Some(ref out) => group_service.get_active_group(out).await.unwrap_or_else(|_| "0".to_string()),
                        None => "0".to_string(),
                    }
                }
            };
            workspace_service.remove_from_group(&workspace, &source_group).await?;
            suffix_service.sync_all_suffixes().await?;
            println!("Removed workspace \"{}\" from group \"{}\"", workspace, source_group);
        }
        WorkspaceAction::Global { workspace } => {
            workspace_service.set_global(&workspace, true).await?;
            suffix_service.sync_all_suffixes().await?;
            println!("Marked workspace \"{}\" as global", workspace);
        }
        WorkspaceAction::Unglobal { workspace } => {
            workspace_service.set_global(&workspace, false).await?;
            suffix_service.sync_all_suffixes().await?;
            println!("Removed global status from workspace \"{}\"", workspace);
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
    }
    Ok(())
}

async fn run_nav(
    action: NavAction,
    nav_service: &NavigationService,
    suffix_service: &SuffixService,
    ipc_client: &SwayIpcClient,
) -> anyhow::Result<()> {
    suffix_service.sync_all_suffixes().await?;
    match action {
        NavAction::Next { output, wrap } => {
            let output = resolve_output(output.as_deref(), ipc_client)?;
            if let Some(target) = nav_service.next_workspace(&output, wrap).await? {
                println!("Navigated to \"{}\"", target);
            }
        }
        NavAction::NextOnOutput { wrap } => {
            if let Some(target) = nav_service.next_workspace_global(wrap).await? {
                println!("Navigated to \"{}\"", target);
            }
        }
        NavAction::Prev { output, wrap } => {
            let output = resolve_output(output.as_deref(), ipc_client)?;
            if let Some(target) = nav_service.prev_workspace(&output, wrap).await? {
                println!("Navigated to \"{}\"", target);
            }
        }
        NavAction::PrevOnOutput { wrap } => {
            if let Some(target) = nav_service.prev_workspace_global(wrap).await? {
                println!("Navigated to \"{}\"", target);
            }
        }
        NavAction::Go { workspace, output: _ } => {
            nav_service.go_workspace(&workspace).await?;
            println!("Navigated to \"{}\"", workspace);
        }
        NavAction::MoveTo { workspace } => {
            nav_service.move_to_workspace(&workspace).await?;
            println!("Moved container to \"{}\"", workspace);
        }
        NavAction::Back => {
            if let Some(target) = nav_service.go_back().await? {
                println!("Navigated back to \"{}\"", target);
            } else {
                println!("No previous workspace found.");
            }
        }
    }
    suffix_service.sync_all_suffixes().await?;
    Ok(())
}

async fn run_sync(
    all: bool,
    workspaces: bool,
    groups: bool,
    outputs: bool,
    workspace_service: &WorkspaceService,
    suffix_service: &SuffixService,
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

    if all || (!workspaces && !groups && !outputs) {
        workspace_service.sync_from_sway().await?;
        synced_ws = true;
        synced_gr = true;
        synced_out = true;
    }

    suffix_service.sync_all_suffixes().await?;

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
    println!("Synced: {}", parts.join(", "));

    Ok(())
}

async fn run_status(
    group_service: &GroupService,
    suffix_service: &SuffixService,
    ipc_client: &SwayIpcClient,
) -> anyhow::Result<()> {
    let outputs = ipc_client.get_outputs()?;
    let sway_workspaces = ipc_client.get_workspaces()?;

    for output in &outputs {
        let active_group = group_service.get_active_group(&output.name).await
            .unwrap_or_else(|_| "0".to_string());
        println!("{}: active group = \"{}\"", output.name, active_group);

        let output_workspaces: Vec<_> = sway_workspaces.iter()
            .filter(|w| w.output == output.name)
            .collect();

        let mut visible = Vec::new();
        let mut hidden = Vec::new();
        let mut global_ws = Vec::new();

        for ws in &output_workspaces {
            let base_name = suffix_service.get_base_name(&ws.name);
            if suffix_service.is_global(&ws.name) {
                global_ws.push(base_name);
            } else if suffix_service.is_hidden(&ws.name) {
                hidden.push(base_name);
            } else {
                visible.push(base_name);
            }
        }

        visible.sort();
        hidden.sort();
        global_ws.sort();

        println!("  Visible: {}", if visible.is_empty() { "(none)".to_string() } else { visible.join(", ") });
        println!("  Hidden:  {}", if hidden.is_empty() { "(none)".to_string() } else { hidden.join(", ") });
        if !global_ws.is_empty() {
            println!("  Global:  {}", global_ws.join(", "));
        }
    }

    Ok(())
}
