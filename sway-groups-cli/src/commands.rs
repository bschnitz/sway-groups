//! CLI commands for swayg.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use sway_groups_core::services::{GroupService, NavigationService, WaybarSyncService, WorkspaceService};
use sway_groups_core::sway::SwayIpcClient;

#[derive(Parser)]
#[command(name = "swayg")]
#[command(author, version, about = "Sway workspace groups management CLI")]
pub struct Cli {
    #[arg(short, long)]
    pub verbose: bool,

    /// Path to the database file. Overrides the default location.
    #[arg(short, long, env = "SWAYG_DB")]
    pub db: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Group {
        #[command(subcommand)]
        action: GroupAction,
    },
    Workspace {
        #[command(subcommand)]
        action: WorkspaceAction,
    },
    Nav {
        #[command(subcommand)]
        action: NavAction,
    },
    Container {
        #[command(subcommand)]
        action: ContainerAction,
    },
    Sync {
        #[arg(short, long)]
        all: bool,

        #[arg(short, long)]
        workspaces: bool,

        #[arg(short, long)]
        groups: bool,

        #[arg(short, long)]
        outputs: bool,
    },
    Init,
    Repair,
    Status,
}

#[derive(Subcommand)]
enum GroupAction {
    List {
        #[arg(short, long)]
        output: Option<String>,
    },
    Create {
        name: String,
    },
    Delete {
        name: String,
        #[arg(short, long)]
        force: bool,
    },
    Rename {
        old_name: String,
        new_name: String,
    },
    Select {
        output: String,
        group: String,
        #[arg(short, long)]
        create: bool,
    },
    Active {
        output: String,
    },
    Next {
        #[arg(short, long)]
        output: Option<String>,
        #[arg(short, long)]
        wrap: bool,
    },
    NextOnOutput {
        #[arg(short, long)]
        output: Option<String>,
        #[arg(short, long)]
        wrap: bool,
    },
    Prev {
        #[arg(short, long)]
        output: Option<String>,
        #[arg(short, long)]
        wrap: bool,
    },
    PrevOnOutput {
        #[arg(short, long)]
        output: Option<String>,
        #[arg(short, long)]
        wrap: bool,
    },
    Prune {
        #[arg(long)]
        keep: Vec<String>,
    },
}

#[derive(Subcommand)]
enum WorkspaceAction {
    List {
        #[arg(short, long)]
        output: Option<String>,
        #[arg(short, long)]
        group: Option<String>,
        #[arg(long)]
        visible: bool,
        #[arg(long)]
        plain: bool,
    },
    Add {
        workspace: String,
        #[arg(short, long)]
        group: Option<String>,
        #[arg(short, long)]
        output: Option<String>,
    },
    Move {
        workspace: String,
        #[arg(short, long)]
        groups: String,
    },
    Remove {
        workspace: String,
        #[arg(short, long)]
        group: Option<String>,
    },
    Rename {
        old_name: String,
        new_name: String,
    },
    Global {
        workspace: String,
    },
    Unglobal {
        workspace: String,
    },
    Groups {
        workspace: String,
    },
}

#[derive(Subcommand)]
enum NavAction {
    Next {
        #[arg(short, long)]
        output: Option<String>,
        #[arg(short, long)]
        wrap: bool,
    },
    NextOnOutput {
        #[arg(short, long)]
        wrap: bool,
    },
    Prev {
        #[arg(short, long)]
        output: Option<String>,
        #[arg(short, long)]
        wrap: bool,
    },
    PrevOnOutput {
        #[arg(short, long)]
        wrap: bool,
    },
    Go {
        workspace: String,
        #[arg(short, long)]
        output: Option<String>,
    },
    MoveTo {
        workspace: String,
    },
    Back,
}

#[derive(Subcommand)]
enum ContainerAction {
    Move {
        workspace: String,
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
        Command::Group { action } => run_group(action, group_service, waybar_sync, ipc_client).await?,
        Command::Workspace { action } => run_workspace(action, workspace_service, group_service, waybar_sync, ipc_client).await?,
        Command::Nav { action } => run_nav(action, nav_service, waybar_sync, ipc_client).await?,
        Command::Container { action } => run_container(action, workspace_service, group_service, nav_service, waybar_sync, ipc_client).await?,
        Command::Sync { all, workspaces, groups, outputs } => {
            run_sync(all, workspaces, groups, outputs, workspace_service, waybar_sync).await?;
        }
        Command::Init => {
            run_init(db_path, workspace_service, group_service, waybar_sync).await?;
        }
        Command::Repair => {
            run_repair(workspace_service, group_service, waybar_sync, ipc_client).await?;
        }
        Command::Status => {
            run_status(group_service, workspace_service, waybar_sync, ipc_client).await?;
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
        GroupAction::Select { output, group, create } => {
            if create {
                let groups = group_service.list_all_group_names().await?;
                if !groups.iter().any(|g| g == &group) {
                    group_service.create_group(&group).await?;
                    println!("Created group \"{}\"", group);
                }
            }
            group_service.set_active_group(&output, &group).await?;
            waybar_sync.update_waybar().await?;
            println!("Set active group for \"{}\" to \"{}\"", output, group);
        }
        GroupAction::Active { output } => {
            let active = group_service.get_active_group(&output).await.unwrap_or_else(|_| "0".to_string());
            println!("{}", active);
        }
        GroupAction::Next { output, wrap } => {
            let output = resolve_output(output.as_deref(), ipc_client)?;
            if let Some(next) = group_service.next_group(&output, wrap).await? {
                waybar_sync.update_waybar().await?;
                println!("Switched from active group to \"{}\"", next);
            }
        }
        GroupAction::NextOnOutput { output, wrap } => {
            let output = resolve_output(output.as_deref(), ipc_client)?;
            if let Some(next) = group_service.next_group_on_output(&output, wrap).await? {
                waybar_sync.update_waybar().await?;
                println!("Switched from active group to \"{}\"", next);
            }
        }
        GroupAction::Prev { output, wrap } => {
            let output = resolve_output(output.as_deref(), ipc_client)?;
            if let Some(prev) = group_service.prev_group(&output, wrap).await? {
                waybar_sync.update_waybar().await?;
                println!("Switched from active group to \"{}\"", prev);
            }
        }
        GroupAction::PrevOnOutput { output, wrap } => {
            let output = resolve_output(output.as_deref(), ipc_client)?;
            if let Some(prev) = group_service.prev_group_on_output(&output, wrap).await? {
                waybar_sync.update_waybar().await?;
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
    waybar_sync: &WaybarSyncService,
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
                        Some(ref out) => group_service.get_active_group(out).await.unwrap_or_else(|_| "0".to_string()),
                        None => "0".to_string(),
                    }
                }
            };
            workspace_service.remove_from_group(&workspace, &source_group).await?;
            waybar_sync.update_waybar().await?;
            println!("Removed workspace \"{}\" from group \"{}\"", workspace, source_group);
        }
        WorkspaceAction::Rename { old_name, new_name } => {
            let merged = workspace_service.rename_workspace(&old_name, &new_name).await?;
            waybar_sync.update_waybar().await?;
            if merged {
                println!("Merged workspace \"{}\" into \"{}\"", old_name, new_name);
            } else {
                println!("Renamed workspace \"{}\" to \"{}\"", old_name, new_name);
            }
        }
        WorkspaceAction::Global { workspace } => {
            workspace_service.set_global(&workspace, true).await?;
            waybar_sync.update_waybar().await?;
            println!("Marked workspace \"{}\" as global", workspace);
        }
        WorkspaceAction::Unglobal { workspace } => {
            workspace_service.set_global(&workspace, false).await?;
            waybar_sync.update_waybar().await?;
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
    waybar_sync: &WaybarSyncService,
    ipc_client: &SwayIpcClient,
) -> anyhow::Result<()> {
    match action {
        NavAction::Next { output, wrap } => {
            let output = resolve_output(output.as_deref(), ipc_client)?;
            if let Some(target) = nav_service.next_workspace(&output, wrap).await? {
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
        NavAction::Prev { output, wrap } => {
            let output = resolve_output(output.as_deref(), ipc_client)?;
            if let Some(target) = nav_service.prev_workspace(&output, wrap).await? {
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
            nav_service.go_workspace(&workspace).await?;
            waybar_sync.update_waybar().await?;
            println!("Navigated to \"{}\"", workspace);
        }
        NavAction::MoveTo { workspace } => {
            nav_service.move_to_workspace(&workspace).await?;
            waybar_sync.update_waybar().await?;
            println!("Moved container to \"{}\"", workspace);
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
    workspace_service: &WorkspaceService,
    group_service: &GroupService,
    nav_service: &NavigationService,
    waybar_sync: &WaybarSyncService,
    ipc_client: &SwayIpcClient,
) -> anyhow::Result<()> {
    match action {
        ContainerAction::Move { workspace, switch_to_workspace } => {
            let output = ipc_client.get_primary_output().ok();
            let active_group = match output {
                Some(ref out) => group_service.get_active_group(out).await.unwrap_or_else(|_| "0".to_string()),
                None => "0".to_string(),
            };

            nav_service.move_to_workspace(&workspace).await?;
            workspace_service.add_to_group(&workspace, &active_group).await.ok();

            if switch_to_workspace {
                nav_service.go_workspace(&workspace).await?;
            }

            waybar_sync.update_waybar().await?;
            println!("Moved container to \"{}\"", workspace);
        }
    }
    Ok(())
}

async fn run_sync(
    all: bool,
    workspaces: bool,
    groups: bool,
    outputs: bool,
    workspace_service: &WorkspaceService,
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

    if !all && !workspaces && !groups && !outputs {
        workspace_service.sync_from_sway().await?;
        synced_ws = true;
        synced_gr = true;
        synced_out = true;
    }

    waybar_sync.update_waybar().await?;

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

async fn run_init(
    db_path: PathBuf,
    _workspace_service: &WorkspaceService,
    _group_service: &GroupService,
    _waybar_sync: &WaybarSyncService,
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

    group_svc.ensure_default_group().await?;
    workspace_svc.sync_from_sway().await?;
    waybar_sync_svc.update_waybar().await?;

    println!("Initialized: created database, synced workspaces and outputs.");
    Ok(())
}

async fn run_repair(
    workspace_service: &WorkspaceService,
    group_service: &GroupService,
    waybar_sync: &WaybarSyncService,
    _ipc_client: &SwayIpcClient,
) -> anyhow::Result<()> {
    let (removed_ws, added_ws, removed_groups) = workspace_service.repair(group_service).await?;

    group_service.ensure_default_group().await?;
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

    for output in &outputs {
        let active_group = group_service.get_active_group(&output.name).await
            .unwrap_or_else(|_| "0".to_string());
        println!("{}: active group = \"{}\"", output.name, active_group);

        let visible = workspace_service.list_visible_workspaces(&output.name).await?;

        let all_ws = workspace_service.list_workspaces(Some(&output.name), None).await?;

        let mut hidden = Vec::new();
        let mut global_ws = Vec::new();
        let mut visible_names = Vec::new();

        for ws in &all_ws {
            if ws.is_global {
                global_ws.push(ws.name.clone());
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

        for ws in &all_ws {
            if ws.is_global {
                continue;
            }
            if visible_names.iter().any(|v| v == &ws.name) {
                continue;
            }
            if sway_names.contains(&ws.name) {
                hidden.push(ws.name.clone());
            }
        }

        visible_names.sort();
        hidden.sort();
        global_ws.sort();
        global_ws.dedup();

        println!("  Visible: {}", if visible_names.is_empty() { "(none)".to_string() } else { visible_names.join(", ") });
        println!("  Hidden:  {}", if hidden.is_empty() { "(none)".to_string() } else { hidden.join(", ") });
        if !global_ws.is_empty() {
            println!("  Global:  {}", global_ws.join(", "));
        }
    }

    Ok(())
}
