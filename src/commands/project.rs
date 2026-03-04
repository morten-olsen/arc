use anyhow::{bail, Result};
use clap::Subcommand;
use rayon::prelude::*;

use crate::global;

#[derive(Subcommand)]
pub enum ProjectCommand {
    /// Register a project in the global registry
    Add {
        /// Path to the project (defaults to current directory)
        path: Option<String>,

        /// Project name (defaults to directory name)
        #[arg(long)]
        name: Option<String>,

        /// Tags for the project (e.g. context:work, area:cli)
        #[arg(long)]
        tag: Vec<String>,
    },

    /// Remove a project from the registry
    Remove {
        /// Project name
        name: String,
    },

    /// Edit project metadata (tags, name)
    Edit {
        /// Project name
        name: String,

        /// Add tags
        #[arg(long = "add-tag")]
        add_tag: Vec<String>,

        /// Remove tags
        #[arg(long = "remove-tag")]
        remove_tag: Vec<String>,

        /// Rename the project
        #[arg(long = "name")]
        new_name: Option<String>,
    },

    /// List registered projects
    List {
        /// Filter by tags (must match all)
        #[arg(long)]
        tag: Vec<String>,
    },

    /// Show git and arc status for registered projects
    Status {
        /// Filter by tags
        #[arg(long)]
        tag: Vec<String>,

        /// Only show dirty projects
        #[arg(long)]
        dirty: bool,
    },

    /// Print project path (used by shell wrapper)
    #[command(hide = true)]
    SwitchPath {
        /// Project name
        name: String,
    },
}

pub fn run(cmd: ProjectCommand) -> Result<()> {
    match cmd {
        ProjectCommand::Add { path, name, tag } => run_add(path, name, tag),
        ProjectCommand::Remove { name } => run_remove(&name),
        ProjectCommand::Edit { name, add_tag, remove_tag, new_name } => {
            run_edit(&name, add_tag, remove_tag, new_name)
        }
        ProjectCommand::List { tag } => run_list(tag),
        ProjectCommand::Status { tag, dirty } => run_status(tag, dirty),
        ProjectCommand::SwitchPath { name } => run_switch_path(&name),
    }
}

fn run_add(path: Option<String>, name: Option<String>, tags: Vec<String>) -> Result<()> {
    let raw = match path {
        Some(p) => std::path::PathBuf::from(p),
        None => std::env::current_dir()?,
    };
    let canonical = std::fs::canonicalize(&raw)
        .map_err(|_| anyhow::anyhow!("path does not exist: {}", raw.display()))?;

    if !canonical.join(".arc").exists() {
        bail!(
            "Not an Arc repository: {}\nRun `arc init` there first.",
            canonical.display()
        );
    }

    let project_name = match name {
        Some(n) => n,
        None => canonical
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unnamed".into()),
    };

    let conn = global::open_registry()?;
    let path_str = canonical.display().to_string();

    if global::register_project(&conn, &path_str, &project_name, &tags)? {
        println!("Registered: {project_name}");
        println!("  Path: {path_str}");
        if !tags.is_empty() {
            println!("  Tags: {}", tags.join(", "));
        }
    } else {
        println!("Already registered: {path_str}");
    }

    Ok(())
}

fn run_remove(name: &str) -> Result<()> {
    let conn = global::open_registry()?;
    if global::remove_project(&conn, name)? {
        println!("Removed: {name}");
    } else {
        bail!("No project named '{name}'");
    }
    Ok(())
}

fn run_edit(
    name: &str,
    add_tag: Vec<String>,
    remove_tag: Vec<String>,
    new_name: Option<String>,
) -> Result<()> {
    let conn = global::open_registry()?;
    let project = global::find_project(&conn, name)?
        .ok_or_else(|| anyhow::anyhow!("No project named '{name}'"))?;

    if !add_tag.is_empty() {
        global::add_tags(&conn, project.id, &add_tag)?;
    }
    if !remove_tag.is_empty() {
        global::remove_tags(&conn, project.id, &remove_tag)?;
    }
    if let Some(ref nn) = new_name {
        global::rename_project(&conn, project.id, nn)?;
    }

    let display_name = new_name.as_deref().unwrap_or(name);
    println!("Updated: {display_name}");
    Ok(())
}

fn run_list(tags: Vec<String>) -> Result<()> {
    let conn = global::open_registry()?;
    let projects = global::list_projects(&conn, &tags)?;

    if projects.is_empty() {
        println!("No projects registered.");
        return Ok(());
    }

    for p in &projects {
        let tag_str = if p.tags.is_empty() {
            String::new()
        } else {
            format!("  [{}]", p.tags.join(", "))
        };
        println!("  {} {}{}", p.name, p.path.display(), tag_str);
    }

    Ok(())
}

struct ProjectStatus {
    name: String,
    path: String,
    branch: Option<String>,
    is_dirty: bool,
    unpushed_count: usize,
    in_progress_tasks: usize,
    abandoned_tasks: usize,
    error: Option<String>,
}

fn scan_project(project: &global::Project) -> ProjectStatus {
    let path_str = project.path.display().to_string();
    let mut ps = ProjectStatus {
        name: project.name.clone(),
        path: path_str.clone(),
        branch: None,
        is_dirty: false,
        unpushed_count: 0,
        in_progress_tasks: 0,
        abandoned_tasks: 0,
        error: None,
    };

    // Open git repo
    let repo = match git2::Repository::discover(&project.path) {
        Ok(r) => r,
        Err(e) => {
            ps.error = Some(format!("git: {e}"));
            return ps;
        }
    };

    // Branch
    if let Ok(head) = repo.head() {
        ps.branch = head.shorthand().map(String::from);
    }

    let null = std::process::Stdio::null;

    // Dirty (git status --porcelain)
    if let Ok(output) = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&project.path)
        .stdout(std::process::Stdio::piped())
        .stderr(null())
        .output()
    {
        ps.is_dirty = !output.stdout.is_empty();
    }

    // Unpushed
    if let Ok(output) = std::process::Command::new("git")
        .args(["rev-list", "--count", "@{upstream}..HEAD"])
        .current_dir(&project.path)
        .stdout(std::process::Stdio::piped())
        .stderr(null())
        .output()
    {
        if output.status.success() {
            ps.unpushed_count = String::from_utf8_lossy(&output.stdout)
                .trim()
                .parse()
                .unwrap_or(0);
        }
    }

    // Arc tasks
    let arc_dir = project.path.join(".arc");
    if let Ok(db) = crate::index::sqlite::open(&arc_dir) {
        if let Ok(count) = db.query_row(
            "SELECT COUNT(*) FROM tasks WHERE status = 'in_progress'",
            [],
            |row| row.get::<_, i64>(0),
        ) {
            ps.in_progress_tasks = count as usize;
        }
        if let Ok(count) = db.query_row(
            "SELECT COUNT(*) FROM tasks WHERE status = 'abandoned'",
            [],
            |row| row.get::<_, i64>(0),
        ) {
            ps.abandoned_tasks = count as usize;
        }
    }

    ps
}

fn run_status(tags: Vec<String>, dirty_only: bool) -> Result<()> {
    let conn = global::open_registry()?;
    let projects = global::list_projects(&conn, &tags)?;

    if projects.is_empty() {
        println!("No projects registered.");
        return Ok(());
    }

    let statuses: Vec<ProjectStatus> = projects.par_iter().map(scan_project).collect();

    let mut any_printed = false;
    for ps in &statuses {
        if dirty_only && !ps.is_dirty {
            continue;
        }

        any_printed = true;
        let branch = ps.branch.as_deref().unwrap_or("(detached)");

        let mut flags = Vec::new();
        if ps.is_dirty {
            flags.push("dirty".to_string());
        }
        if ps.unpushed_count > 0 {
            flags.push(format!("{}↑", ps.unpushed_count));
        }
        if ps.in_progress_tasks > 0 {
            flags.push(format!("{} task(s)", ps.in_progress_tasks));
        }
        if ps.abandoned_tasks > 0 {
            flags.push(format!("{} abandoned", ps.abandoned_tasks));
        }
        if let Some(ref e) = ps.error {
            flags.push(format!("error: {e}"));
        }

        let flag_str = if flags.is_empty() {
            "clean".to_string()
        } else {
            flags.join(", ")
        };

        println!("  {} ({branch}) [{flag_str}]", ps.name);
        println!("    {}", ps.path);
    }

    if dirty_only && !any_printed {
        println!("All projects are clean.");
    }

    Ok(())
}

fn run_switch_path(name: &str) -> Result<()> {
    let conn = global::open_registry()?;
    let project = global::find_project(&conn, name)?
        .ok_or_else(|| anyhow::anyhow!("No project named '{name}'"))?;
    print!("{}", project.path.display());
    Ok(())
}
