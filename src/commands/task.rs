use anyhow::{bail, Context, Result};
use chrono::Utc;
use clap::Subcommand;

use crate::commands::change;
use crate::context::ArcContext;
use crate::format::commit_message;
use crate::git;
use crate::metadata::task::{Task, TaskStatus};

#[derive(Subcommand)]
pub enum TaskCommand {
    /// Create a new task (creates a worktree)
    New {
        /// Task goal (short description)
        goal: String,

        /// External ticket reference (e.g. PROJ-123)
        #[arg(long = "ref")]
        ticket_ref: Option<String>,
    },

    /// List all tasks
    List,

    /// Show current task status
    Status,

    /// Print worktree path for a task (used by shell wrapper)
    #[command(hide = true)]
    SwitchPath {
        /// Task name (fuzzy matched)
        name: String,
    },

    /// Complete the current task (cleanup after external merge)
    Complete,

    /// Abandon the current task
    Abandon {
        /// Reason for abandoning
        #[arg(long)]
        reason: Option<String>,
    },

    /// Sync the current task branch with upstream
    Sync {
        /// Continue a paused rebase after conflict resolution
        #[arg(long, name = "continue")]
        cont: bool,

        /// Abort a paused rebase
        #[arg(long)]
        abort: bool,
    },

    /// Squash checkpoints and fixes into their parent changes
    Finalize,

    /// Adopt existing commits into a new task
    Adopt {
        /// Task goal (short description)
        goal: String,

        /// Adopt the last N commits
        #[arg(long)]
        last: Option<usize>,

        /// Adopt all commits after this ref (exclusive)
        #[arg(long)]
        since: Option<String>,

        /// External ticket reference (e.g. PROJ-123)
        #[arg(long = "ref")]
        ticket_ref: Option<String>,
    },
}

pub fn run(cmd: TaskCommand) -> Result<()> {
    match cmd {
        TaskCommand::New { goal, ticket_ref } => run_new(&goal, ticket_ref),
        TaskCommand::List => run_list(),
        TaskCommand::Status => run_status(),
        TaskCommand::SwitchPath { name } => run_switch_path(&name),
        TaskCommand::Complete => run_complete(),
        TaskCommand::Abandon { reason } => run_abandon(reason),
        TaskCommand::Sync { cont, abort } => run_sync(cont, abort),
        TaskCommand::Finalize => run_finalize(),
        TaskCommand::Adopt { goal, last, since, ticket_ref } => {
            run_adopt(&goal, last, since, ticket_ref)
        }
    }
}

fn run_new(goal: &str, ticket_ref: Option<String>) -> Result<()> {
    let ctx = ArcContext::open()?;

    let task_id = uuid::Uuid::new_v4().to_string();
    let short_id = &task_id[..8];
    let slug = slug::slugify(goal);
    let slug = if slug.len() > 40 { &slug[..40] } else { &slug };
    let branch = format!("task/{short_id}-{slug}");

    let wt_path = git::worktree::create(&ctx.repo_root, slug, &branch, None)?;
    let wt_path_str = wt_path.display().to_string();

    let base_ref = git::repo::current_branch(&ctx.repo)?
        .unwrap_or_else(|| "HEAD".to_string());

    let task = Task {
        id: task_id.clone(),
        name: goal.to_string(),
        goal: goal.to_string(),
        status: TaskStatus::InProgress,
        branch: branch.clone(),
        worktree_path: Some(wt_path_str.clone()),
        base_ref,
        changes: Vec::new(),
        created_at: Utc::now(),
        completed_at: None,
        ticket_ref: ticket_ref.clone(),
        abandoned_reason: None,
    };

    ctx.db.execute(
        "INSERT INTO tasks (id, name, goal, status, branch, worktree_path, base_ref, created_at, ticket_ref)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            task.id, task.name, task.goal, task.status.to_string(),
            task.branch, task.worktree_path, task.base_ref,
            task.created_at.to_rfc3339(),
            ticket_ref,
        ],
    )?;

    let json = serde_json::to_string_pretty(&task)?;
    git::refs::write_ref(&ctx.repo, &format!("tasks/{task_id}.json"), &json)?;

    println!("Created task: {goal}");
    println!("  Branch:   {branch}");
    println!("  Worktree: {wt_path_str}");
    if let Some(ref tr) = ticket_ref {
        println!("  Ref:      {tr}");
    }
    println!();
    println!("Switch to it: arc task switch {slug}");

    Ok(())
}

fn run_list() -> Result<()> {
    let ctx = ArcContext::open()?;

    let mut stmt = ctx.db.prepare(
        "SELECT id, name, status, branch, worktree_path FROM tasks ORDER BY created_at DESC"
    )?;

    let tasks: Vec<(String, String, String, String, Option<String>)> = stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?
        .collect::<Result<_, _>>()?;

    if tasks.is_empty() {
        println!("No tasks. Create one with: arc task new \"description\"");
        return Ok(());
    }

    let current_task = ctx.current_task_id().ok().flatten();

    for (id, name, status, _branch, wt_path) in &tasks {
        let marker = if current_task.as_deref() == Some(id.as_str()) { "* " } else { "  " };
        let short_id = &id[..8.min(id.len())];
        let path = wt_path.as_deref().unwrap_or("-");
        println!("{marker}[{short_id}] {name}  ({status})  {path}");
    }

    Ok(())
}

fn run_status() -> Result<()> {
    let ctx = ArcContext::open()?;

    let task_id = ctx.current_task_id()?
        .ok_or_else(|| anyhow::anyhow!("Not in a task worktree. Use `arc task switch <name>` first."))?;

    let mut stmt = ctx.db.prepare(
        "SELECT name, goal, status, branch, ticket_ref FROM tasks WHERE id = ?1"
    )?;
    let (name, goal, status, branch, ticket_ref): (String, String, String, String, Option<String>) = stmt
        .query_row([&task_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
        })?;

    let short_id = &task_id[..8.min(task_id.len())];

    let change_count: i64 = ctx.db.query_row(
        "SELECT COUNT(*) FROM changes WHERE task_id = ?1 AND status = 'active'",
        [&task_id],
        |row| row.get(0),
    )?;

    println!("Task: {name}");
    println!("  ID:      {short_id}");
    println!("  Status:  {status}");
    println!("  Branch:  {branch}");
    println!("  Changes: {change_count}");
    if let Some(ref tr) = ticket_ref {
        println!("  Ref:     {tr}");
    }
    if name != goal {
        println!("  Goal:    {goal}");
    }

    Ok(())
}

fn run_switch_path(name: &str) -> Result<()> {
    let ctx = ArcContext::open()?;

    let mut stmt = ctx.db.prepare(
        "SELECT worktree_path FROM tasks WHERE status = 'in_progress'
         AND (name LIKE '%' || ?1 || '%' OR branch LIKE '%' || ?1 || '%' OR id LIKE ?1 || '%')"
    )?;

    let paths: Vec<String> = stmt
        .query_map([name], |row| row.get::<_, Option<String>>(0))?
        .filter_map(|r| r.ok().flatten())
        .collect();

    match paths.len() {
        0 => bail!("No matching task found for '{name}'"),
        1 => {
            print!("{}", paths[0]);
            Ok(())
        }
        _ => bail!("Multiple tasks match '{name}'. Be more specific."),
    }
}

/// Complete: cleanup after external merge (e.g. GitHub PR).
/// Removes worktree and deletes branch (soft delete).
fn run_complete() -> Result<()> {
    let ctx = ArcContext::open()?;

    let task_id = ctx.current_task_id()?
        .ok_or_else(|| anyhow::anyhow!("Not in a task worktree."))?;

    let mut stmt = ctx.db.prepare(
        "SELECT name, branch FROM tasks WHERE id = ?1"
    )?;
    let (name, branch): (String, String) = stmt
        .query_row([&task_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;

    let slug = branch
        .strip_prefix("task/")
        .and_then(|s| s.split_once('-'))
        .map(|(_, slug)| slug)
        .unwrap_or(&branch);

    println!("Completing task: {name}");

    let null = std::process::Stdio::null;

    // Remove worktree
    git::worktree::remove(&ctx.repo_root, slug)?;

    // Delete task branch (soft: -d, will fail if unmerged which is fine)
    let _ = std::process::Command::new("git")
        .args(["branch", "-d", &branch])
        .current_dir(&ctx.repo_root)
        .stdout(null()).stderr(null())
        .status();

    // Update task status in SQLite
    ctx.db.execute(
        "UPDATE tasks SET status = 'completed', completed_at = ?1, worktree_path = NULL WHERE id = ?2",
        rusqlite::params![Utc::now().to_rfc3339(), task_id],
    )?;

    // Update ref
    let ref_path = format!("tasks/{task_id}.json");
    if let Some(json) = git::refs::read_ref(&ctx.repo, &ref_path)? {
        let mut task: Task = serde_json::from_str(&json)?;
        task.status = TaskStatus::Completed;
        task.completed_at = Some(Utc::now());
        task.worktree_path = None;
        let updated = serde_json::to_string_pretty(&task)?;
        git::refs::write_ref(&ctx.repo, &ref_path, &updated)?;
    }

    println!("Task completed. Worktree and branch cleaned up.");

    Ok(())
}

/// Abandon: force-remove worktree and branch.
fn run_abandon(reason: Option<String>) -> Result<()> {
    let ctx = ArcContext::open()?;

    let task_id = ctx.current_task_id()?
        .ok_or_else(|| anyhow::anyhow!("Not in a task worktree."))?;

    let mut stmt = ctx.db.prepare(
        "SELECT name, branch FROM tasks WHERE id = ?1"
    )?;
    let (name, branch): (String, String) = stmt
        .query_row([&task_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;

    let slug = branch
        .strip_prefix("task/")
        .and_then(|s| s.split_once('-'))
        .map(|(_, slug)| slug)
        .unwrap_or(&branch);

    let reason_display = reason.as_deref().unwrap_or("no reason given");
    println!("Abandoning task: {name} ({reason_display})");

    let null = std::process::Stdio::null;

    // Remove worktree
    git::worktree::remove(&ctx.repo_root, slug)?;

    // Force-delete branch (-D)
    let _ = std::process::Command::new("git")
        .args(["branch", "-D", &branch])
        .current_dir(&ctx.repo_root)
        .stdout(null()).stderr(null())
        .status();

    // Update task status in SQLite
    ctx.db.execute(
        "UPDATE tasks SET status = 'abandoned', completed_at = ?1, worktree_path = NULL, abandoned_reason = ?2 WHERE id = ?3",
        rusqlite::params![Utc::now().to_rfc3339(), reason, task_id],
    )?;

    // Update ref
    let ref_path = format!("tasks/{task_id}.json");
    if let Some(json) = git::refs::read_ref(&ctx.repo, &ref_path)? {
        let mut task: Task = serde_json::from_str(&json)?;
        task.status = TaskStatus::Abandoned;
        task.completed_at = Some(Utc::now());
        task.worktree_path = None;
        task.abandoned_reason = reason;
        let updated = serde_json::to_string_pretty(&task)?;
        git::refs::write_ref(&ctx.repo, &ref_path, &updated)?;
    }

    println!("Task abandoned.");

    Ok(())
}

/// Sync: rebase current task branch onto upstream base.
fn run_sync(cont: bool, abort: bool) -> Result<()> {
    let ctx = ArcContext::open()?;

    let task_id = ctx.current_task_id()?
        .ok_or_else(|| anyhow::anyhow!("Not in a task worktree."))?;

    let base_ref: String = ctx.db.query_row(
        "SELECT base_ref FROM tasks WHERE id = ?1",
        [&task_id],
        |row| row.get(0),
    )?;

    let workdir = ctx.repo.workdir()
        .context("bare repo not supported")?
        .to_path_buf();

    let null = std::process::Stdio::null;

    if abort {
        let status = std::process::Command::new("git")
            .args(["rebase", "--abort"])
            .current_dir(&workdir)
            .stdout(null()).stderr(std::process::Stdio::piped())
            .status()?;
        if !status.success() {
            bail!("git rebase --abort failed");
        }
        println!("Rebase aborted.");
        return Ok(());
    }

    if cont {
        let status = std::process::Command::new("git")
            .args(["rebase", "--continue"])
            .current_dir(&workdir)
            .status()?;
        if !status.success() {
            bail!("Rebase continue failed. Resolve conflicts and run `arc task sync --continue` again.");
        }
        println!("Rebase continued successfully.");
        return Ok(());
    }

    // Auto-checkpoint dirty work before sync
    let has_changes = repo_has_changes(&workdir)?;
    if has_changes {
        println!("Auto-checkpointing dirty work before sync...");
        change::run_checkpoint(Some("auto-checkpoint before sync".into()), false, None)?;
    }

    // Fetch from origin
    println!("Fetching from origin...");
    let status = std::process::Command::new("git")
        .args(["fetch", "origin"])
        .current_dir(&workdir)
        .stdout(null()).stderr(std::process::Stdio::piped())
        .status()?;
    if !status.success() {
        bail!("git fetch origin failed");
    }

    // Rebase onto origin/<base_ref>
    let upstream = format!("origin/{base_ref}");
    println!("Rebasing onto {upstream}...");
    let output = std::process::Command::new("git")
        .args(["rebase", &upstream])
        .current_dir(&workdir)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("CONFLICT") || stderr.contains("conflict") {
            println!("Rebase paused due to conflicts.");
            println!("Resolve conflicts, then run: arc task sync --continue");
            println!("Or abort with: arc task sync --abort");
            return Ok(());
        }
        bail!("Rebase failed: {stderr}");
    }

    println!("Sync complete.");
    Ok(())
}

/// Finalize: squash checkpoints and fixes into their parent changes via interactive rebase.
fn run_finalize() -> Result<()> {
    let ctx = ArcContext::open()?;

    let task_id = ctx.current_task_id()?
        .ok_or_else(|| anyhow::anyhow!("Not in a task worktree."))?;

    let base_ref: String = ctx.db.query_row(
        "SELECT base_ref FROM tasks WHERE id = ?1",
        [&task_id],
        |row| row.get(0),
    )?;

    let workdir = ctx.repo.workdir()
        .context("bare repo not supported")?
        .to_path_buf();

    // Find merge-base
    let merge_base_output = std::process::Command::new("git")
        .args(["merge-base", &base_ref, "HEAD"])
        .current_dir(&workdir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .context("failed to run git merge-base")?;

    if !merge_base_output.status.success() {
        bail!("Could not find merge-base between {base_ref} and HEAD");
    }

    let merge_base = String::from_utf8_lossy(&merge_base_output.stdout).trim().to_string();

    // Walk commits from merge-base to HEAD
    let log_output = std::process::Command::new("git")
        .args(["log", "--reverse", "--format=%H", &format!("{merge_base}..HEAD")])
        .current_dir(&workdir)
        .stdout(std::process::Stdio::piped())
        .output()
        .context("failed to run git log")?;

    let commit_shas: Vec<String> = String::from_utf8_lossy(&log_output.stdout)
        .lines()
        .map(String::from)
        .filter(|s| !s.is_empty())
        .collect();

    if commit_shas.is_empty() {
        println!("No commits to finalize.");
        return Ok(());
    }

    // Look up each commit's metadata from SQLite to determine parent_change_id and change_type
    #[derive(Debug)]
    struct CommitInfo {
        sha: String,
        change_id: Option<String>,
        change_type: String,
        parent_change_id: Option<String>,
    }

    let mut commits: Vec<CommitInfo> = Vec::new();
    for sha in &commit_shas {
        let info = ctx.db.query_row(
            "SELECT id, change_type, parent_change_id FROM changes WHERE git_sha = ?1",
            [sha],
            |row| Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            )),
        );

        match info {
            Ok((id, ct, parent)) => commits.push(CommitInfo {
                sha: sha.clone(),
                change_id: Some(id),
                change_type: ct,
                parent_change_id: parent,
            }),
            Err(_) => commits.push(CommitInfo {
                sha: sha.clone(),
                change_id: None,
                change_type: "unknown".into(),
                parent_change_id: None,
            }),
        }
    }

    // Build rebase todo: group checkpoints/fixes under their parent changes
    // Strategy: collect "change" commits as pick targets, then fixup their children
    let mut todo_lines: Vec<String> = Vec::new();
    let mut handled: std::collections::HashSet<String> = std::collections::HashSet::new();

    for commit in &commits {
        if handled.contains(&commit.sha) {
            continue;
        }

        match commit.change_type.as_str() {
            "change" | "unknown" => {
                // This is a primary commit — pick it
                todo_lines.push(format!("pick {}", commit.sha));
                handled.insert(commit.sha.clone());

                // Find all checkpoints and fixes that belong to this change
                if let Some(ref cid) = commit.change_id {
                    for child in &commits {
                        if handled.contains(&child.sha) {
                            continue;
                        }
                        if child.parent_change_id.as_deref() == Some(cid) {
                            todo_lines.push(format!("fixup {}", child.sha));
                            handled.insert(child.sha.clone());
                        }
                    }
                }
            }
            _ => {
                // Orphan checkpoint/fix without a known parent — pick as standalone
                if !handled.contains(&commit.sha) {
                    todo_lines.push(format!("pick {}", commit.sha));
                    handled.insert(commit.sha.clone());
                }
            }
        }
    }

    if todo_lines.is_empty() {
        println!("Nothing to finalize.");
        return Ok(());
    }

    let todo_content = todo_lines.join("\n") + "\n";

    // Write todo to a temp file for GIT_SEQUENCE_EDITOR
    let tmp_dir = std::env::temp_dir();
    let todo_file = tmp_dir.join(format!("arc-finalize-{}.txt", &task_id[..8]));
    std::fs::write(&todo_file, &todo_content)?;

    // GIT_SEQUENCE_EDITOR="cp <todo_file>" — git invokes as: cp <todo_file> <rebase_todo_path>
    let editor_cmd = format!("cp {}", todo_file.display());

    println!("Finalizing: squashing checkpoints/fixes into their parent changes...");

    let status = std::process::Command::new("git")
        .args(["rebase", "-i", &merge_base])
        .env("GIT_SEQUENCE_EDITOR", &editor_cmd)
        .current_dir(&workdir)
        .status()?;

    // Clean up temp file
    let _ = std::fs::remove_file(&todo_file);

    if !status.success() {
        bail!("Finalize rebase failed. You may need to resolve conflicts manually.");
    }

    // After rebase: walk new commits, update SHA mappings in SQLite and change-map
    let new_log_output = std::process::Command::new("git")
        .args(["log", "--reverse", "--format=%H", &format!("{merge_base}..HEAD")])
        .current_dir(&workdir)
        .stdout(std::process::Stdio::piped())
        .output()?;

    let new_shas: Vec<String> = String::from_utf8_lossy(&new_log_output.stdout)
        .lines()
        .map(String::from)
        .filter(|s| !s.is_empty())
        .collect();

    // For each new commit, parse arc:change:id from the message and update the DB
    for sha in &new_shas {
        let msg_output = std::process::Command::new("git")
            .args(["log", "-1", "--format=%B", sha])
            .current_dir(&workdir)
            .stdout(std::process::Stdio::piped())
            .output()?;

        let message = String::from_utf8_lossy(&msg_output.stdout);
        if let Some(meta) = commit_message::parse(&message) {
            if let Some(ref change_id) = meta.change_id {
                ctx.db.execute(
                    "UPDATE changes SET git_sha = ?1 WHERE id = ?2",
                    rusqlite::params![sha, change_id],
                )?;
                change::update_change_map(&ctx, change_id, sha)?;
            }
        }
    }

    // Mark all checkpoints and fixes for this task as squashed
    ctx.db.execute(
        "UPDATE changes SET status = 'squashed' WHERE task_id = ?1 AND change_type IN ('checkpoint', 'fix') AND status = 'active'",
        [&task_id],
    )?;

    println!("Finalize complete. {} clean commits remain.", new_shas.len());

    Ok(())
}

/// Adopt: promote existing commits into a new task.
fn run_adopt(
    goal: &str,
    last: Option<usize>,
    since: Option<String>,
    ticket_ref: Option<String>,
) -> Result<()> {
    let ctx = ArcContext::open()?;

    // Must not be inside a task worktree
    if ctx.current_task_id()?.is_some() {
        bail!("Already inside a task worktree. Run this from the main repo.");
    }

    let workdir = ctx.repo.workdir()
        .context("bare repo not supported")?
        .to_path_buf();

    let null = std::process::Stdio::null;

    // Stash all working state (staged + unstaged + untracked) so we can
    // restore it in the task worktree after adopting.
    let dirty = repo_has_changes(&workdir)?;
    let has_stash = if dirty {
        let stash_status = std::process::Command::new("git")
            .args(["stash", "push", "--include-untracked", "-m", "arc-adopt-wip"])
            .current_dir(&workdir)
            .stdout(null()).stderr(null())
            .status()
            .context("Failed to stash working changes")?;
        if !stash_status.success() {
            bail!("Failed to stash working changes before adopt.");
        }
        true
    } else {
        false
    };

    // Determine the base commit (parent of oldest adopted commit)
    let (base_sha, commit_shas) = select_commits(&workdir, last, since.as_deref())?;

    if commit_shas.is_empty() {
        bail!("No commits to adopt.");
    }

    // Warn if commits are on the remote tracking branch
    let current_branch = git::repo::current_branch(&ctx.repo)?
        .unwrap_or_else(|| "HEAD".to_string());

    let tracking_output = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", &format!("{current_branch}@{{upstream}}")])
        .current_dir(&workdir)
        .stdout(std::process::Stdio::piped())
        .stderr(null())
        .output()?;

    if tracking_output.status.success() {
        let upstream = String::from_utf8_lossy(&tracking_output.stdout).trim().to_string();
        // Check if the oldest adopted commit is reachable from upstream
        let oldest = &commit_shas[0];
        let merge_base = std::process::Command::new("git")
            .args(["merge-base", "--is-ancestor", oldest, &upstream])
            .current_dir(&workdir)
            .stdout(null()).stderr(null())
            .status()?;
        if merge_base.success() {
            eprintln!("Warning: adopted commits are already on remote tracking branch '{upstream}'.");
            eprintln!("Resetting will cause divergence — you may need to force push.");
        }
    }

    // Create the task
    let task_id = uuid::Uuid::new_v4().to_string();
    let short_id = &task_id[..8];
    let slug = slug::slugify(goal);
    let slug = if slug.len() > 40 { &slug[..40] } else { &slug };
    let branch = format!("task/{short_id}-{slug}");

    let wt_path = git::worktree::create(&ctx.repo_root, slug, &branch, Some(&base_sha))?;
    let wt_path_str = wt_path.display().to_string();

    let task = crate::metadata::task::Task {
        id: task_id.clone(),
        name: goal.to_string(),
        goal: goal.to_string(),
        status: crate::metadata::task::TaskStatus::InProgress,
        branch: branch.clone(),
        worktree_path: Some(wt_path_str.clone()),
        base_ref: current_branch.clone(),
        changes: Vec::new(),
        created_at: Utc::now(),
        completed_at: None,
        ticket_ref: ticket_ref.clone(),
        abandoned_reason: None,
    };

    ctx.db.execute(
        "INSERT INTO tasks (id, name, goal, status, branch, worktree_path, base_ref, created_at, ticket_ref)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            task.id, task.name, task.goal, task.status.to_string(),
            task.branch, task.worktree_path, task.base_ref,
            task.created_at.to_rfc3339(),
            ticket_ref,
        ],
    )?;

    let json = serde_json::to_string_pretty(&task)?;
    git::refs::write_ref(&ctx.repo, &format!("tasks/{task_id}.json"), &json)?;

    // Cherry-pick each commit into the worktree, rewriting trailers
    let mut adopted_change_ids: Vec<String> = Vec::new();

    for original_sha in &commit_shas {
        // Cherry-pick into the worktree
        let cp_status = std::process::Command::new("git")
            .args(["cherry-pick", original_sha])
            .current_dir(&wt_path)
            .stdout(null()).stderr(null())
            .status()
            .context("Failed to run git cherry-pick")?;

        if !cp_status.success() {
            bail!(
                "Cherry-pick of {original_sha} failed. Aborting adopt.\n\
                 The task branch and worktree were created but may be incomplete."
            );
        }

        // Read the commit message
        let msg_output = std::process::Command::new("git")
            .args(["log", "-1", "--format=%B"])
            .current_dir(&wt_path)
            .stdout(std::process::Stdio::piped())
            .output()?;
        let original_message = String::from_utf8_lossy(&msg_output.stdout).to_string();

        // Parse existing arc metadata (if any)
        let existing_meta = commit_message::parse(&original_message);
        let (summary, intent) = commit_message::extract_summary_and_intent(&original_message);

        let change_id;
        let author_type;
        let change_type;
        let author_model;

        if let Some(ref meta) = existing_meta {
            // Preserve existing arc metadata, just add task
            change_id = meta.change_id.clone()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            author_type = meta.author_type.clone()
                .unwrap_or_else(|| "human".to_string());
            change_type = meta.change_type.clone()
                .unwrap_or_else(|| "change".to_string());
            author_model = meta.author_model.clone();
        } else {
            // Plain git commit — generate full arc metadata
            change_id = uuid::Uuid::new_v4().to_string();
            author_type = "human".to_string();
            change_type = "change".to_string();
            author_model = None;
        }

        let new_metadata = commit_message::CommitMetadata {
            change_id: Some(change_id.clone()),
            author_type: Some(author_type.clone()),
            author_model: author_model.clone(),
            task_id: Some(task_id.clone()),
            change_type: Some(change_type.clone()),
            task_ref: ticket_ref.clone(),
            session_id: existing_meta.as_ref().and_then(|m| m.session_id.clone()),
            confidence: existing_meta.as_ref().and_then(|m| m.confidence),
            prompt_hash: existing_meta.as_ref().and_then(|m| m.prompt_hash.clone()),
            derived_from: existing_meta.as_ref().map(|m| m.derived_from.clone()).unwrap_or_default(),
            parent_change_summary: existing_meta.as_ref().and_then(|m| m.parent_change_summary.clone()),
        };

        let new_message = commit_message::format(&summary, intent.as_deref(), &new_metadata);

        // Amend the cherry-picked commit with the new message
        let amend_status = std::process::Command::new("git")
            .args(["commit", "--amend", "-m", &new_message])
            .current_dir(&wt_path)
            .stdout(null()).stderr(null())
            .status()
            .context("Failed to amend cherry-picked commit")?;

        if !amend_status.success() {
            bail!("Failed to amend commit with arc trailers.");
        }

        // Get new SHA after amend
        let new_sha_output = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&wt_path)
            .stdout(std::process::Stdio::piped())
            .output()?;
        let new_sha = String::from_utf8_lossy(&new_sha_output.stdout).trim().to_string();

        // Update SQLite: insert or update the change record
        let existing_change: bool = ctx.db.query_row(
            "SELECT COUNT(*) FROM changes WHERE id = ?1",
            [&change_id],
            |row| row.get::<_, i64>(0),
        ).map(|c| c > 0).unwrap_or(false);

        if existing_change {
            ctx.db.execute(
                "UPDATE changes SET git_sha = ?1, task_id = ?2 WHERE id = ?3",
                rusqlite::params![new_sha, task_id, change_id],
            )?;
        } else {
            ctx.db.execute(
                "INSERT INTO changes (id, git_sha, summary, intent, author_type, author_name, task_id, change_type, status, created_at, author_model)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'active', ?9, ?10)",
                rusqlite::params![
                    change_id, new_sha, summary, intent,
                    author_type, whoami(), task_id,
                    change_type,
                    Utc::now().to_rfc3339(),
                    author_model,
                ],
            )?;
        }

        // Update change map
        change::update_change_map(&ctx, &change_id, &new_sha)?;
        adopted_change_ids.push(change_id);
    }

    // Update task ref JSON with change IDs
    let ref_path = format!("tasks/{task_id}.json");
    if let Some(json) = git::refs::read_ref(&ctx.repo, &ref_path)? {
        let mut task: crate::metadata::task::Task = serde_json::from_str(&json)?;
        task.changes = adopted_change_ids.clone();
        let updated = serde_json::to_string_pretty(&task)?;
        git::refs::write_ref(&ctx.repo, &ref_path, &updated)?;
    }

    // Reset the original branch to the base commit
    let reset_status = std::process::Command::new("git")
        .args(["reset", "--hard", &base_sha])
        .current_dir(&workdir)
        .stdout(null()).stderr(null())
        .status()
        .context("Failed to reset original branch")?;

    if !reset_status.success() {
        bail!("Failed to reset original branch to base commit.");
    }

    // Restore stashed working state in the task worktree
    if has_stash {
        // git stash is shared across worktrees, so we can pop in the task worktree
        let pop_status = std::process::Command::new("git")
            .args(["stash", "pop"])
            .current_dir(&wt_path)
            .stdout(null()).stderr(null())
            .status()
            .context("Failed to restore working state in task worktree")?;

        if !pop_status.success() {
            eprintln!("Warning: could not restore working state in task worktree.");
            eprintln!("Your changes are still in `git stash`. Run `git stash pop` in the worktree.");
        }
    }

    println!("Created task: {goal}");
    println!("  Branch:   {branch}");
    println!("  Worktree: {wt_path_str}");
    println!("  Adopted:  {} commit(s)", commit_shas.len());
    if let Some(ref tr) = ticket_ref {
        println!("  Ref:      {tr}");
    }
    println!();
    println!("Switch to it: arc task switch {slug}");

    Ok(())
}

/// Select commits to adopt based on --last, --since, or default (ahead of upstream/merge-base).
/// Returns (base_sha, vec_of_commit_shas) where base_sha is the parent of the oldest commit.
fn select_commits(
    workdir: &std::path::Path,
    last: Option<usize>,
    since: Option<&str>,
) -> Result<(String, Vec<String>)> {
    let null = std::process::Stdio::null;

    if let Some(n) = last {
        // --last N: take the last N commits
        let log_output = std::process::Command::new("git")
            .args(["log", "--reverse", "--format=%H", &format!("-{n}")])
            .current_dir(workdir)
            .stdout(std::process::Stdio::piped())
            .stderr(null())
            .output()
            .context("failed to run git log")?;

        let shas: Vec<String> = String::from_utf8_lossy(&log_output.stdout)
            .lines()
            .map(String::from)
            .filter(|s| !s.is_empty())
            .collect();

        if shas.is_empty() {
            return Ok((String::new(), vec![]));
        }

        let base = get_parent_sha(workdir, &shas[0])?;
        Ok((base, shas))
    } else if let Some(ref_name) = since {
        // --since <ref>: all commits after ref
        let log_output = std::process::Command::new("git")
            .args(["log", "--reverse", "--format=%H", &format!("{ref_name}..HEAD")])
            .current_dir(workdir)
            .stdout(std::process::Stdio::piped())
            .stderr(null())
            .output()
            .context("failed to run git log")?;

        let shas: Vec<String> = String::from_utf8_lossy(&log_output.stdout)
            .lines()
            .map(String::from)
            .filter(|s| !s.is_empty())
            .collect();

        if shas.is_empty() {
            return Ok((String::new(), vec![]));
        }

        // Base is the ref itself (resolved to SHA)
        let base_output = std::process::Command::new("git")
            .args(["rev-parse", ref_name])
            .current_dir(workdir)
            .stdout(std::process::Stdio::piped())
            .stderr(null())
            .output()?;
        let base = String::from_utf8_lossy(&base_output.stdout).trim().to_string();

        Ok((base, shas))
    } else {
        // Default: ahead of upstream tracking branch, or since merge-base with main/master
        let upstream_output = std::process::Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD@{upstream}"])
            .current_dir(workdir)
            .stdout(std::process::Stdio::piped())
            .stderr(null())
            .output()?;

        let base_ref = if upstream_output.status.success() {
            String::from_utf8_lossy(&upstream_output.stdout).trim().to_string()
        } else {
            // No upstream — try main, then master
            let main_exists = std::process::Command::new("git")
                .args(["rev-parse", "--verify", "main"])
                .current_dir(workdir)
                .stdout(null()).stderr(null())
                .status()?;

            if main_exists.success() {
                "main".to_string()
            } else {
                "master".to_string()
            }
        };

        // Find merge-base
        let mb_output = std::process::Command::new("git")
            .args(["merge-base", &base_ref, "HEAD"])
            .current_dir(workdir)
            .stdout(std::process::Stdio::piped())
            .stderr(null())
            .output()
            .context("failed to find merge-base")?;

        if !mb_output.status.success() {
            bail!("Could not find merge-base between '{base_ref}' and HEAD.");
        }

        let merge_base = String::from_utf8_lossy(&mb_output.stdout).trim().to_string();

        let log_output = std::process::Command::new("git")
            .args(["log", "--reverse", "--format=%H", &format!("{merge_base}..HEAD")])
            .current_dir(workdir)
            .stdout(std::process::Stdio::piped())
            .stderr(null())
            .output()?;

        let shas: Vec<String> = String::from_utf8_lossy(&log_output.stdout)
            .lines()
            .map(String::from)
            .filter(|s| !s.is_empty())
            .collect();

        Ok((merge_base, shas))
    }
}

/// Get the parent SHA of a commit. Returns the commit itself if it has no parent (root commit).
fn get_parent_sha(workdir: &std::path::Path, sha: &str) -> Result<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", &format!("{sha}^")])
        .current_dir(workdir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        // Root commit — no parent, use the commit itself as base
        Ok(sha.to_string())
    }
}

fn whoami() -> String {
    let config = git2::Config::open_default().ok();
    config
        .and_then(|c| c.get_string("user.name").ok())
        .unwrap_or_else(|| "unknown".into())
}

fn repo_has_changes(workdir: &std::path::Path) -> Result<bool> {
    let output = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(workdir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .context("failed to run git status")?;
    Ok(!output.stdout.is_empty())
}
