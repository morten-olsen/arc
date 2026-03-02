use anyhow::{bail, Context, Result};
use chrono::Utc;

use crate::context::ArcContext;
use crate::format::commit_message::CommitMetadata;
use crate::git;
use crate::index::change_map::ChangeMap;

/// Declare a new change — creates an (optionally empty) commit marking intent.
pub fn run(
    summary: String,
    intent: Option<String>,
    agent: bool,
    model: Option<String>,
) -> Result<()> {
    let ctx = ArcContext::open()?;
    let task_id = ctx.current_task_id()?;

    let change_id = uuid::Uuid::new_v4().to_string();
    let is_agent = agent || model.is_some();
    let author_type_str = if is_agent { "agent" } else { "human" };
    let task_ref = find_task_ref(&ctx, task_id.as_deref())?;

    let metadata = CommitMetadata {
        change_id: Some(change_id.clone()),
        author_type: Some(author_type_str.into()),
        author_model: model.clone(),
        task_id: task_id.clone(),
        change_type: Some("change".into()),
        task_ref,
        ..Default::default()
    };

    let has_changes = repo_has_changes(&ctx.repo)?;

    let oid = if has_changes {
        git::commit::create_commit(&ctx.repo, &summary, intent.as_deref(), &metadata)?
    } else {
        git::commit::create_empty_commit(&ctx.repo, &summary, intent.as_deref(), &metadata)?
    };

    let sha = oid.to_string();

    ctx.db.execute(
        "INSERT INTO changes (id, git_sha, summary, intent, author_type, author_name, task_id, change_type, status, created_at, parent_change_id, author_model)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'change', 'active', ?8, NULL, ?9)",
        rusqlite::params![
            change_id, sha, summary, intent,
            author_type_str, whoami(), task_id,
            Utc::now().to_rfc3339(),
            model,
        ],
    )?;

    update_change_map(&ctx, &change_id, &sha)?;

    if let Some(ref tid) = task_id {
        update_task_changes(&ctx, tid, &change_id)?;
    }

    let short_id = &change_id[..8];
    if has_changes {
        println!("Change [{short_id}]: {summary}");
    } else {
        println!("Change [{short_id}]: {summary}  (empty \u{2014} use `arc checkpoint` to add work)");
    }

    Ok(())
}

/// Amend the most recent change — stages changes, rewrites the commit, updates the change-map.
pub fn run_amend(
    summary: String,
    intent: Option<String>,
    agent: bool,
    model: Option<String>,
) -> Result<()> {
    let ctx = ArcContext::open()?;
    let task_id = ctx.current_task_id()?;

    // Find the most recent active change to amend
    let (change_id, old_summary) = find_current_change(&ctx, task_id.as_deref())?
        .context("No active change to amend")?;

    let is_agent = agent || model.is_some();
    let author_type_str = if is_agent { "agent" } else { "human" };
    let task_ref = find_task_ref(&ctx, task_id.as_deref())?;

    let metadata = CommitMetadata {
        change_id: Some(change_id.clone()),
        author_type: Some(author_type_str.into()),
        author_model: model.clone(),
        task_id: task_id.clone(),
        change_type: Some("change".into()),
        task_ref,
        ..Default::default()
    };

    let oid = git::commit::amend_commit(&ctx.repo, &summary, intent.as_deref(), &metadata)?;
    let sha = oid.to_string();

    ctx.db.execute(
        "UPDATE changes SET git_sha = ?1, summary = ?2, intent = ?3, author_model = ?4 WHERE id = ?5",
        rusqlite::params![sha, summary, intent, model, change_id],
    )?;

    update_change_map(&ctx, &change_id, &sha)?;

    let short_id = &change_id[..8];
    println!("Amended [{short_id}]: {old_summary} → {summary}");

    Ok(())
}

/// Checkpoint: create a new lightweight commit for recovery.
pub fn run_checkpoint(
    message: Option<String>,
    agent: bool,
    model: Option<String>,
) -> Result<()> {
    let ctx = ArcContext::open()?;
    let task_id = ctx.current_task_id()?;

    let change_id = uuid::Uuid::new_v4().to_string();
    let summary = message.unwrap_or_else(|| "checkpoint".into());
    let is_agent = agent || model.is_some();
    let author_type_str = if is_agent { "agent" } else { "human" };

    let parent = find_current_change(&ctx, task_id.as_deref())?;
    let parent_summary = parent.as_ref().map(|(_, s)| s.clone());
    let parent_id = parent.map(|(id, _)| id);

    let task_ref = find_task_ref(&ctx, task_id.as_deref())?;

    let metadata = CommitMetadata {
        change_id: Some(change_id.clone()),
        author_type: Some(author_type_str.into()),
        author_model: model.clone(),
        task_id: task_id.clone(),
        change_type: Some("checkpoint".into()),
        parent_change_summary: parent_summary,
        task_ref,
        ..Default::default()
    };

    let oid = git::commit::create_commit(&ctx.repo, &summary, None, &metadata)?;
    let sha = oid.to_string();

    ctx.db.execute(
        "INSERT INTO changes (id, git_sha, summary, author_type, author_name, task_id, change_type, status, created_at, parent_change_id, author_model)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'checkpoint', 'active', ?7, ?8, ?9)",
        rusqlite::params![
            change_id, sha, summary,
            author_type_str, whoami(), task_id,
            Utc::now().to_rfc3339(),
            parent_id,
            model,
        ],
    )?;

    update_change_map(&ctx, &change_id, &sha)?;

    let short_id = &change_id[..8];
    println!("Checkpoint [{short_id}]: {summary}");

    Ok(())
}

/// Fix: create a commit that fixes a specific change.
pub fn run_fix(
    target_change_id: String,
    message: Option<String>,
    agent: bool,
    model: Option<String>,
) -> Result<()> {
    let ctx = ArcContext::open()?;
    let task_id = ctx.current_task_id()?;

    // Resolve target change by prefix match
    let (resolved_id, target_summary) = resolve_change_id(&ctx, &target_change_id)?;

    let change_id = uuid::Uuid::new_v4().to_string();
    let summary = message.unwrap_or_else(|| format!("fix {}", &resolved_id[..8]));
    let is_agent = agent || model.is_some();
    let author_type_str = if is_agent { "agent" } else { "human" };
    let task_ref = find_task_ref(&ctx, task_id.as_deref())?;

    let metadata = CommitMetadata {
        change_id: Some(change_id.clone()),
        author_type: Some(author_type_str.into()),
        author_model: model.clone(),
        task_id: task_id.clone(),
        change_type: Some("fix".into()),
        parent_change_summary: Some(target_summary),
        task_ref,
        ..Default::default()
    };

    let oid = git::commit::create_commit(&ctx.repo, &summary, None, &metadata)?;
    let sha = oid.to_string();

    ctx.db.execute(
        "INSERT INTO changes (id, git_sha, summary, author_type, author_name, task_id, change_type, status, created_at, parent_change_id, author_model)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'fix', 'active', ?7, ?8, ?9)",
        rusqlite::params![
            change_id, sha, summary,
            author_type_str, whoami(), task_id,
            Utc::now().to_rfc3339(),
            resolved_id,
            model,
        ],
    )?;

    update_change_map(&ctx, &change_id, &sha)?;

    let short_id = &change_id[..8];
    println!("Fix [{short_id}]: {summary}");

    Ok(())
}

/// Find the most recent change (change_type='change') for the current task.
/// Returns (change_id, summary) or None if no changes exist.
fn find_current_change(ctx: &ArcContext, task_id: Option<&str>) -> Result<Option<(String, String)>> {
    let result = if let Some(tid) = task_id {
        ctx.db.query_row(
            "SELECT id, summary FROM changes WHERE task_id = ?1 AND change_type = 'change' AND status = 'active'
             ORDER BY created_at DESC LIMIT 1",
            [tid],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
    } else {
        ctx.db.query_row(
            "SELECT id, summary FROM changes WHERE task_id IS NULL AND change_type = 'change' AND status = 'active'
             ORDER BY created_at DESC LIMIT 1",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
    };

    match result {
        Ok(pair) => Ok(Some(pair)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Look up the ticket_ref for the current task.
fn find_task_ref(ctx: &ArcContext, task_id: Option<&str>) -> Result<Option<String>> {
    let Some(tid) = task_id else { return Ok(None) };
    let result = ctx.db.query_row(
        "SELECT ticket_ref FROM tasks WHERE id = ?1",
        [tid],
        |row| row.get::<_, Option<String>>(0),
    );
    match result {
        Ok(v) => Ok(v),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Resolve a change ID by prefix match.
fn resolve_change_id(ctx: &ArcContext, prefix: &str) -> Result<(String, String)> {
    let mut stmt = ctx.db.prepare(
        "SELECT id, summary FROM changes WHERE id LIKE ?1 || '%' AND status = 'active'"
    )?;
    let matches: Vec<(String, String)> = stmt
        .query_map([prefix], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .filter_map(|r| r.ok())
        .collect();

    match matches.len() {
        0 => bail!("No change found matching '{prefix}'"),
        1 => Ok(matches.into_iter().next().unwrap()),
        _ => bail!("Multiple changes match '{prefix}'. Be more specific."),
    }
}

pub fn update_change_map(ctx: &ArcContext, change_id: &str, sha: &str) -> Result<()> {
    let mut map = match git::refs::read_ref(&ctx.repo, "index/change-map.json")? {
        Some(json) => ChangeMap::from_json(&json)?,
        None => ChangeMap::new(),
    };
    map.insert(change_id.to_string(), sha.to_string());
    git::refs::write_ref(&ctx.repo, "index/change-map.json", &map.to_json()?)?;
    Ok(())
}

fn update_task_changes(ctx: &ArcContext, task_id: &str, change_id: &str) -> Result<()> {
    let ref_path = format!("tasks/{task_id}.json");
    if let Some(json) = git::refs::read_ref(&ctx.repo, &ref_path)? {
        let mut task: crate::metadata::task::Task = serde_json::from_str(&json)?;
        task.changes.push(change_id.to_string());
        let updated = serde_json::to_string_pretty(&task)?;
        git::refs::write_ref(&ctx.repo, &ref_path, &updated)?;
    }
    Ok(())
}

fn repo_has_changes(repo: &git2::Repository) -> Result<bool> {
    let workdir = repo.workdir().context("bare repo not supported")?;
    let output = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(workdir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .context("failed to run git status")?;
    Ok(!output.stdout.is_empty())
}

fn whoami() -> String {
    let config = git2::Config::open_default().ok();
    config
        .and_then(|c| c.get_string("user.name").ok())
        .unwrap_or_else(|| "unknown".into())
}
