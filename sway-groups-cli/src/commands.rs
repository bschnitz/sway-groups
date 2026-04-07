//! CLI commands for swayg.

use clap::{Parser, Subcommand};
use sway_groups_core::services::{GroupService, SuffixService, WorkspaceService};

/// Sway workspace groups management CLI.
#[derive(Parser)]
#[command(name = "swayg")]
#[command(author, version, about = "Sway workspace groups management CLI")]
pub struct Cli {
    /// Enable verbose output.
    #[arg(short, long)]
    verbose: bool,

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
    /// Switch to next group.
    Next {
        /// Output name.
        #[arg(short, long)]
        output: Option<String>,

        /// Wrap around.
        #[arg(short, long)]
        wrap: bool,
    },
    /// Switch to previous group.
    Prev {
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
    /// Go to next workspace in active group.
    Next {
        /// Output name.
        #[arg(short, long)]
        output: Option<String>,

        /// Wrap around.
        #[arg(short, long)]
        wrap: bool,
    },
    /// Go to previous workspace in active group.
    Prev {
        /// Output name.
        #[arg(short, long)]
        output: Option<String>,

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
    /// Go back to previous workspace.
    Back {
        /// Output name.
        #[arg(short, long)]
        output: Option<String>,
    },
}

/// Run the CLI commands.
pub async fn run(
    cli: Cli,
    _group_service: &GroupService,
    _workspace_service: &WorkspaceService,
    _suffix_service: &SuffixService,
) -> anyhow::Result<()> {
    match cli.command {
        Command::Group { action } => run_group(action).await?,
        Command::Workspace { action } => run_workspace(action).await?,
        Command::Nav { action } => run_nav(action).await?,
        Command::Sync { all, workspaces, groups, outputs } => {
            if all || (!workspaces && !groups && !outputs) {
                println!("Syncing everything...");
            } else {
                if workspaces {
                    println!("Syncing workspaces...");
                }
                if groups {
                    println!("Syncing groups...");
                }
                if outputs {
                    println!("Syncing outputs...");
                }
            }
        }
        Command::Status => {
            println!("Status: OK");
        }
    }
    Ok(())
}

/// Run group commands.
async fn run_group(action: GroupAction) -> anyhow::Result<()> {
    match action {
        GroupAction::List { output } => {
            println!("Listing groups{}...",
                output.as_ref().map(|o| format!(" for output '{}'", o)).unwrap_or_default()
            );
        }
        GroupAction::Create { name } => {
            println!("Creating group '{}'...", name);
        }
        GroupAction::Delete { name, force } => {
            println!("Deleting group '{}' (force={})...", name, force);
        }
        GroupAction::Rename { old_name, new_name } => {
            println!("Renaming group '{}' to '{}'...", old_name, new_name);
        }
        GroupAction::Select { output, group } => {
            println!("Setting active group for '{}' to '{}'...", output, group);
        }
        GroupAction::Active { output } => {
            println!("Showing active group for '{}'...", output);
        }
        GroupAction::Next { output, wrap } => {
            println!("Next group (output={}, wrap={})...", output.as_deref().unwrap_or("default"), wrap);
        }
        GroupAction::Prev { output, wrap } => {
            println!("Previous group (output={}, wrap={})...", output.as_deref().unwrap_or("default"), wrap);
        }
        GroupAction::Prune { keep } => {
            println!("Pruning empty groups (keeping: {:?})...", keep);
        }
    }
    Ok(())
}

/// Run workspace commands.
async fn run_workspace(action: WorkspaceAction) -> anyhow::Result<()> {
    match action {
        WorkspaceAction::List { output, group } => {
            println!("Listing workspaces (output={}, group={})...",
                output.as_deref().unwrap_or("all"),
                group.as_deref().unwrap_or("all")
            );
        }
        WorkspaceAction::Add { workspace, group, output } => {
            println!("Adding workspace '{}' to group '{}'...",
                workspace,
                group.as_deref().unwrap_or("active")
            );
        }
        WorkspaceAction::Remove { workspace, group } => {
            println!("Removing workspace '{}' from group '{}'...",
                workspace,
                group.as_deref().unwrap_or("active")
            );
        }
        WorkspaceAction::Global { workspace } => {
            println!("Marking workspace '{}' as global...", workspace);
        }
        WorkspaceAction::Unglobal { workspace } => {
            println!("Removing global status from workspace '{}'...", workspace);
        }
        WorkspaceAction::Groups { workspace } => {
            println!("Showing groups for workspace '{}'...", workspace);
        }
    }
    Ok(())
}

/// Run navigation commands.
async fn run_nav(action: NavAction) -> anyhow::Result<()> {
    match action {
        NavAction::Next { output, wrap } => {
            println!("Navigating to next workspace (output={}, wrap={})...",
                output.as_deref().unwrap_or("default"), wrap
            );
        }
        NavAction::Prev { output, wrap } => {
            println!("Navigating to previous workspace (output={}, wrap={})...",
                output.as_deref().unwrap_or("default"), wrap
            );
        }
        NavAction::Go { workspace, output } => {
            println!("Navigating to workspace '{}' on '{}'...",
                workspace,
                output.as_deref().unwrap_or("default")
            );
        }
        NavAction::Back { output } => {
            println!("Navigating back on '{}'...",
                output.as_deref().unwrap_or("default")
            );
        }
    }
    Ok(())
}
