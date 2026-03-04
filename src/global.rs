use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::PathBuf;

pub struct Project {
    pub id: i64,
    pub path: PathBuf,
    pub name: String,
    pub tags: Vec<String>,
    pub registered_at: String,
}

/// Resolve the Arc global data directory.
/// Uses `$XDG_DATA_HOME/arc/` with `$HOME/.local/share/arc/` fallback.
pub fn data_dir() -> Result<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        if !xdg.is_empty() {
            return Ok(PathBuf::from(xdg).join("arc"));
        }
    }
    let home = std::env::var("HOME").context("$HOME not set")?;
    Ok(PathBuf::from(home).join(".local/share/arc"))
}

/// Open (or create) the global registry database with WAL mode and migrations.
pub fn open_registry() -> Result<Connection> {
    let dir = data_dir()?;
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("creating {}", dir.display()))?;
    let db_path = dir.join("registry.db");
    let conn = Connection::open(db_path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(SCHEMA)?;
    Ok(())
}

const SCHEMA: &str = r#"
    CREATE TABLE IF NOT EXISTS projects (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        path            TEXT NOT NULL UNIQUE,
        name            TEXT NOT NULL UNIQUE,
        registered_at   TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS project_tags (
        project_id  INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        tag         TEXT NOT NULL,
        PRIMARY KEY (project_id, tag)
    );

    CREATE INDEX IF NOT EXISTS idx_project_tags_tag ON project_tags(tag);
"#;

/// Register a project. Returns false if the path is already registered.
pub fn register_project(
    conn: &Connection,
    path: &str,
    name: &str,
    tags: &[String],
) -> Result<bool> {
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM projects WHERE path = ?1",
            [path],
            |row| row.get(0),
        )
        .ok();

    if existing.is_some() {
        return Ok(false);
    }

    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO projects (path, name, registered_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![path, name, now],
    )?;

    if !tags.is_empty() {
        let id = conn.last_insert_rowid();
        add_tags(conn, id, tags)?;
    }

    Ok(true)
}

pub fn remove_project(conn: &Connection, name: &str) -> Result<bool> {
    let changed = conn.execute("DELETE FROM projects WHERE name = ?1", [name])?;
    Ok(changed > 0)
}

pub fn find_project(conn: &Connection, name: &str) -> Result<Option<Project>> {
    let row = conn.query_row(
        "SELECT id, path, name, registered_at FROM projects WHERE name = ?1",
        [name],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        },
    );

    match row {
        Ok((id, path, name, registered_at)) => {
            let tags = get_tags(conn, id)?;
            Ok(Some(Project {
                id,
                path: PathBuf::from(path),
                name,
                tags,
                registered_at,
            }))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// List projects, optionally filtered by tags (ALL-of semantics).
pub fn list_projects(conn: &Connection, tags: &[String]) -> Result<Vec<Project>> {
    let mut projects = Vec::new();

    if tags.is_empty() {
        let mut stmt =
            conn.prepare("SELECT id, path, name, registered_at FROM projects ORDER BY name")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        for r in rows {
            let (id, path, name, registered_at) = r?;
            let t = get_tags(conn, id)?;
            projects.push(Project {
                id,
                path: PathBuf::from(path),
                name,
                tags: t,
                registered_at,
            });
        }
    } else {
        // ALL-of: project must have every specified tag
        let placeholders: Vec<String> = (0..tags.len()).map(|i| format!("?{}", i + 1)).collect();
        let sql = format!(
            "SELECT p.id, p.path, p.name, p.registered_at
             FROM projects p
             JOIN project_tags pt ON pt.project_id = p.id
             WHERE pt.tag IN ({})
             GROUP BY p.id
             HAVING COUNT(DISTINCT pt.tag) = ?{}
             ORDER BY p.name",
            placeholders.join(", "),
            tags.len() + 1,
        );
        let mut stmt = conn.prepare(&sql)?;

        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = tags
            .iter()
            .map(|t| Box::new(t.clone()) as Box<dyn rusqlite::types::ToSql>)
            .collect();
        params.push(Box::new(tags.len() as i64));

        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        for r in rows {
            let (id, path, name, registered_at) = r?;
            let t = get_tags(conn, id)?;
            projects.push(Project {
                id,
                path: PathBuf::from(path),
                name,
                tags: t,
                registered_at,
            });
        }
    }

    Ok(projects)
}

pub fn add_tags(conn: &Connection, project_id: i64, tags: &[String]) -> Result<()> {
    for tag in tags {
        conn.execute(
            "INSERT OR IGNORE INTO project_tags (project_id, tag) VALUES (?1, ?2)",
            rusqlite::params![project_id, tag],
        )?;
    }
    Ok(())
}

pub fn remove_tags(conn: &Connection, project_id: i64, tags: &[String]) -> Result<()> {
    for tag in tags {
        conn.execute(
            "DELETE FROM project_tags WHERE project_id = ?1 AND tag = ?2",
            rusqlite::params![project_id, tag],
        )?;
    }
    Ok(())
}

pub fn rename_project(conn: &Connection, project_id: i64, new_name: &str) -> Result<()> {
    conn.execute(
        "UPDATE projects SET name = ?1 WHERE id = ?2",
        rusqlite::params![new_name, project_id],
    )?;
    Ok(())
}

fn get_tags(conn: &Connection, project_id: i64) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT tag FROM project_tags WHERE project_id = ?1 ORDER BY tag")?;
    let tags: Vec<String> = stmt
        .query_map([project_id], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(tags)
}
