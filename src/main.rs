mod commands;
mod context;
mod format;
mod git;
mod index;
mod metadata;

use clap::Parser;
use commands::Cli;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    commands::run(cli)
}
