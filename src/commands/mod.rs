pub mod change;
pub mod eject;
pub mod hook;
pub mod init;
pub mod intent;
pub mod log;
pub mod pull;
pub mod push;
pub mod task;
pub mod undo;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "arc",
    about = "Version control for codebases that didn't write themselves",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Initialize Arc in the current repository
    Init,

    /// Print shell wrapper function (add to your shell profile)
    ShellInit,

    /// Manage tasks
    #[command(subcommand)]
    Task(task::TaskCommand),

    /// Declare a new change (creates a commit, optionally empty)
    Change {
        /// Short summary of the change
        summary: String,

        /// Why this change is being made
        #[arg(long)]
        intent: Option<String>,

        /// Mark as agent-authored
        #[arg(long)]
        agent: bool,

        /// Model name (implies --agent)
        #[arg(long)]
        model: Option<String>,

        /// Amend the most recent commit instead of creating a new one
        #[arg(long)]
        amend: bool,
    },

    /// Save a lightweight checkpoint
    Checkpoint {
        /// Optional checkpoint message
        message: Option<String>,

        /// Mark as agent-authored
        #[arg(long)]
        agent: bool,

        /// Model name (implies --agent)
        #[arg(long)]
        model: Option<String>,
    },

    /// Apply a fix to a specific change
    Fix {
        /// Change ID to fix (prefix match)
        change_id: String,

        /// Fix description
        message: Option<String>,

        /// Mark as agent-authored
        #[arg(long)]
        agent: bool,

        /// Model name (implies --agent)
        #[arg(long)]
        model: Option<String>,
    },

    /// Undo one or more changes
    Undo {
        /// Revert back to a specific change ID
        #[arg(long)]
        to: Option<String>,
    },

    /// Show change history
    Log {
        /// Show all changes including checkpoints, fixes, and undone
        #[arg(long)]
        all: bool,

        /// Filter to a specific task
        #[arg(long)]
        task: Option<String>,
    },

    /// Push code and metadata to remote
    Push {
        /// Force push (use with caution)
        #[arg(long)]
        force: bool,
    },

    /// Pull code and metadata from remote
    Pull,

    /// Show the Arc intent behind each line of a file
    Intent {
        /// File to show intents for
        file: String,
        /// Line or line range (e.g. "10" or "10,20")
        #[arg(long)]
        line: Option<String>,
    },

    /// Remove Arc from the repository
    Eject,

    /// Handle Git hook events (internal)
    #[command(hide = true)]
    Hook {
        /// Hook event name
        event: String,
    },
}

pub fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Init => init::run(),
        Command::ShellInit => init::run_shell_init(),
        Command::Task(cmd) => task::run(cmd),
        Command::Change { summary, intent, agent, model, amend } => {
            if amend {
                change::run_amend(summary, intent, agent, model)
            } else {
                change::run(summary, intent, agent, model)
            }
        }
        Command::Checkpoint { message, agent, model } => {
            change::run_checkpoint(message, agent, model)
        }
        Command::Fix { change_id, message, agent, model } => {
            change::run_fix(change_id, message, agent, model)
        }
        Command::Undo { to } => undo::run(to),
        Command::Log { all, task } => log::run(all, task),
        Command::Push { force } => push::run(force),
        Command::Pull => pull::run(),
        Command::Intent { file, line } => intent::run(file, line),
        Command::Eject => eject::run(),
        Command::Hook { event } => hook::run(&event),
    }
}
