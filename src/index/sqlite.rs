use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

/// Open or create the Arc SQLite database.
pub fn open(arc_dir: &Path) -> Result<Connection> {
    let db_path = arc_dir.join("index.db");
    let conn = Connection::open(db_path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    Ok(conn)
}

/// Create the schema tables if they don't exist, and run migrations.
pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(SCHEMA_V1)?;
    run_migrations(conn)?;
    Ok(())
}

/// Initial schema (v1).
const SCHEMA_V1: &str = r#"
    CREATE TABLE IF NOT EXISTS changes (
        id          TEXT PRIMARY KEY,
        git_sha     TEXT,
        summary     TEXT NOT NULL,
        intent      TEXT,
        author_type TEXT NOT NULL DEFAULT 'human',
        author_name TEXT NOT NULL,
        task_id     TEXT,
        change_type TEXT NOT NULL DEFAULT 'change',
        status      TEXT NOT NULL DEFAULT 'active',
        created_at  TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS tasks (
        id            TEXT PRIMARY KEY,
        name          TEXT NOT NULL,
        goal          TEXT NOT NULL,
        status        TEXT NOT NULL DEFAULT 'in_progress',
        branch        TEXT NOT NULL,
        worktree_path TEXT,
        base_ref      TEXT NOT NULL,
        created_at    TEXT NOT NULL,
        completed_at  TEXT
    );

    CREATE INDEX IF NOT EXISTS idx_changes_task ON changes(task_id);
    CREATE INDEX IF NOT EXISTS idx_changes_git_sha ON changes(git_sha);

    CREATE TABLE IF NOT EXISTS schema_version (
        version INTEGER NOT NULL
    );
"#;

/// Run versioned migrations.
fn run_migrations(conn: &Connection) -> Result<()> {
    let current = current_version(conn);

    if current < 2 {
        conn.execute_batch(
            r#"
            ALTER TABLE changes ADD COLUMN parent_change_id TEXT;
            ALTER TABLE changes ADD COLUMN author_model TEXT;
            ALTER TABLE tasks ADD COLUMN ticket_ref TEXT;
            ALTER TABLE tasks ADD COLUMN abandoned_reason TEXT;
            CREATE INDEX IF NOT EXISTS idx_changes_parent ON changes(parent_change_id);
            "#,
        )?;
        set_version(conn, 2)?;
    }

    Ok(())
}

fn current_version(conn: &Connection) -> i64 {
    conn.query_row("SELECT COALESCE(MAX(version), 1) FROM schema_version", [], |row| row.get(0))
        .unwrap_or(1)
}

fn set_version(conn: &Connection, version: i64) -> Result<()> {
    conn.execute("DELETE FROM schema_version", [])?;
    conn.execute("INSERT INTO schema_version (version) VALUES (?1)", [version])?;
    Ok(())
}
