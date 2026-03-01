use anyhow::Result;
use std::path::Path;

use crate::git;
use crate::index::sqlite;

pub fn run() -> Result<()> {
    let cwd = std::env::current_dir()?;

    // Initialize git if needed
    let repo = match git::repo::open(&cwd) {
        Ok(repo) => {
            println!("Found existing Git repository.");
            repo
        }
        Err(_) => {
            let repo = git::repo::init(&cwd)?;
            println!("Initialized Git repository.");
            repo
        }
    };

    let repo_root = repo.workdir().unwrap_or(&cwd);
    let arc_dir = repo_root.join(".arc");

    if arc_dir.exists() {
        println!("Arc is already initialized.");
        return Ok(());
    }

    // Create .arc directory structure
    std::fs::create_dir_all(arc_dir.join("worktrees"))?;

    // Initialize SQLite
    let db = sqlite::open(&arc_dir)?;
    sqlite::migrate(&db)?;

    // Install git hooks
    git::hooks::install(repo.path())?;

    // Add .arc to .gitignore if not already present
    ensure_gitignore(repo_root)?;

    // Write initial config to refs/arc/config.json
    let config = serde_json::json!({
        "version": 1,
    });
    git::refs::write_ref(&repo, "config.json", &config.to_string())?;

    println!("Initialized Arc.");
    println!();
    println!("Add this to your shell profile for task switching:");
    println!("  eval \"$(arc shell-init)\"");

    Ok(())
}

pub fn run_shell_init() -> Result<()> {
    print!("{}", shell_wrapper());
    Ok(())
}

fn ensure_gitignore(repo_root: &Path) -> Result<()> {
    let gitignore = repo_root.join(".gitignore");
    if gitignore.exists() {
        let content = std::fs::read_to_string(&gitignore)?;
        if content.lines().any(|l| l.trim() == ".arc" || l.trim() == ".arc/") {
            return Ok(());
        }
        let mut content = content;
        if !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(".arc/\n");
        std::fs::write(&gitignore, content)?;
    } else {
        std::fs::write(&gitignore, ".arc/\n")?;
    }
    Ok(())
}

fn shell_wrapper() -> &'static str {
    r#"arc() {
    if [ "$1" = "task" ] && [ "$2" = "switch" ]; then
        local dir
        dir="$(command arc task switch-path "$3")" && cd "$dir"
    else
        command arc "$@"
    fi
}
"#
}
