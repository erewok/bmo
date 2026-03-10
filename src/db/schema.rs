use rusqlite::Connection;

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

const INITIAL_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS issues (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    parent_id   INTEGER REFERENCES issues(id) ON DELETE SET NULL,
    title       TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status      TEXT NOT NULL DEFAULT 'backlog',
    priority    TEXT NOT NULL DEFAULT 'medium',
    kind        TEXT NOT NULL DEFAULT 'task',
    assignee    TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS comments (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    issue_id   INTEGER NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    body       TEXT NOT NULL,
    author     TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS labels (
    id    INTEGER PRIMARY KEY AUTOINCREMENT,
    name  TEXT NOT NULL UNIQUE,
    color TEXT
);

CREATE TABLE IF NOT EXISTS issue_labels (
    issue_id INTEGER NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    label_id INTEGER NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
    PRIMARY KEY (issue_id, label_id)
);

CREATE TABLE IF NOT EXISTS issue_relations (
    id       INTEGER PRIMARY KEY AUTOINCREMENT,
    from_id  INTEGER NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    to_id    INTEGER NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    relation TEXT NOT NULL,
    UNIQUE(from_id, to_id, relation)
);

CREATE TABLE IF NOT EXISTS activity_log (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    issue_id   INTEGER NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    kind       TEXT NOT NULL,
    detail     TEXT,
    actor      TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS issue_files (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    issue_id   INTEGER NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    path       TEXT NOT NULL,
    added_at   TEXT NOT NULL,
    UNIQUE(issue_id, path)
);
";

/// Migrations keyed by the version they produce.
/// Each entry is (target_version, sql_to_apply).
static MIGRATIONS: &[(u32, &str)] = &[
    (1, INITIAL_SCHEMA),
    // Future migrations: (2, "ALTER TABLE ..."), ...
];

/// Initialize or migrate the database schema.
pub fn initialize(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch("PRAGMA journal_mode = WAL;")?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    conn.execute_batch("PRAGMA busy_timeout = 5000;")?;

    // Ensure the meta table exists before reading schema_version.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
    )?;

    let current_version: u32 = conn
        .query_row(
            "SELECT value FROM meta WHERE key = 'schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    for &(target, sql) in MIGRATIONS {
        if current_version < target {
            conn.execute_batch(sql)?;
            conn.execute(
                "INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', ?1)",
                rusqlite::params![target.to_string()],
            )?;
        }
    }

    // Seed created_at if not present.
    conn.execute(
        "INSERT OR IGNORE INTO meta (key, value) VALUES ('created_at', ?1)",
        rusqlite::params![chrono::Utc::now().to_rfc3339()],
    )?;

    Ok(())
}
