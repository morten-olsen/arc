use anyhow::Result;

use crate::context::ArcContext;
use crate::git;

pub fn run() -> Result<()> {
    let ctx = ArcContext::open()?;

    // Remove worktrees
    let slugs = git::worktree::list(&ctx.repo_root)?;
    for slug in &slugs {
        println!("  Removing worktree: {slug}");
        git::worktree::remove(&ctx.repo_root, slug)?;
    }

    // Delete all refs/arc/* refs
    let refs = git::refs::list_refs(&ctx.repo, "")?;
    for ref_name in &refs {
        let short = ref_name.strip_prefix("refs/arc/").unwrap_or(ref_name);
        println!("  Deleted {short}");
        git::refs::delete_ref(&ctx.repo, short)?;
    }

    // Remove git hooks
    git::hooks::uninstall(ctx.repo.path())?;
    println!("  Removed Git hooks");

    // Remove .arc directory
    if ctx.arc_dir.exists() {
        std::fs::remove_dir_all(&ctx.arc_dir)?;
        println!("  Deleted .arc/");
    }

    println!();
    println!("Done. This is now a plain Git repository.");
    println!("Your code, branches, and commit history are unchanged.");

    Ok(())
}
