use anyhow::{bail, Result};

use crate::context::ArcContext;

pub fn run() -> Result<()> {
    let ctx = ArcContext::open()?;

    // Pull arc metadata refs
    let _ = std::process::Command::new("git")
        .args(["fetch", "origin", "refs/arc/*:refs/arc/*"])
        .current_dir(&ctx.repo_root)
        .status();

    // Pull current branch
    let status = std::process::Command::new("git")
        .args(["pull"])
        .current_dir(&ctx.repo_root)
        .status()?;

    if !status.success() {
        bail!("git pull failed");
    }

    println!("Pulled.");
    Ok(())
}
