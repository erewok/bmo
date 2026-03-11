pub mod schema;

mod activity;
mod comments;
mod files;
mod impl_repo;
mod issues;
mod labels;
mod relations;

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::model::activity::NewActivityEntry;
use crate::model::{
    ActivityEntry, Comment, Issue, IssueFile, IssueFilter, Kind, Label, Priority, Relation,
    RelationKind, Status,
};

// ── Repository trait ──────────────────────────────────────────────────────────

pub trait Repository {
    // Issues
    fn create_issue(&self, input: &CreateIssueInput) -> anyhow::Result<Issue>;
    fn get_issue(&self, id: i64) -> anyhow::Result<Option<Issue>>;
    fn list_issues(&self, filter: &IssueFilter) -> anyhow::Result<Vec<Issue>>;
    fn count_issues(&self, filter: &IssueFilter) -> anyhow::Result<i64>;
    fn update_issue(&self, id: i64, input: &UpdateIssueInput) -> anyhow::Result<Issue>;
    fn delete_issue(&self, id: i64) -> anyhow::Result<()>;
    fn get_sub_issues(&self, parent_id: i64) -> anyhow::Result<Vec<Issue>>;

    // Comments
    fn add_comment(&self, input: &AddCommentInput) -> anyhow::Result<Comment>;
    fn list_comments(&self, issue_id: i64) -> anyhow::Result<Vec<Comment>>;

    // Labels
    fn get_or_create_label(&self, name: &str, color: Option<&str>) -> anyhow::Result<Label>;
    fn add_label_to_issue(&self, issue_id: i64, label_id: i64) -> anyhow::Result<()>;
    fn remove_label_from_issue(&self, issue_id: i64, label_name: &str) -> anyhow::Result<()>;
    fn list_issue_labels(&self, issue_id: i64) -> anyhow::Result<Vec<Label>>;
    fn list_all_labels(&self) -> anyhow::Result<Vec<Label>>;
    fn delete_label(&self, name: &str) -> anyhow::Result<()>;

    // Relations
    fn add_relation(
        &self,
        from_id: i64,
        kind: RelationKind,
        to_id: i64,
    ) -> anyhow::Result<Relation>;
    fn remove_relation(&self, relation_id: i64) -> anyhow::Result<()>;
    fn list_relations(&self, issue_id: i64) -> anyhow::Result<Vec<Relation>>;
    fn list_all_relations(&self) -> anyhow::Result<Vec<Relation>>;

    // Activity
    fn log_activity(&self, entry: &NewActivityEntry) -> anyhow::Result<()>;
    fn list_activity(&self, issue_id: i64, limit: usize) -> anyhow::Result<Vec<ActivityEntry>>;

    // Files
    fn add_file(&self, issue_id: i64, path: &str) -> anyhow::Result<IssueFile>;
    fn remove_file(&self, issue_id: i64, path: &str) -> anyhow::Result<()>;
    fn list_files(&self, issue_id: i64) -> anyhow::Result<Vec<IssueFile>>;

    // Meta
    fn get_meta(&self, key: &str) -> anyhow::Result<Option<String>>;
    fn set_meta(&self, key: &str, value: &str) -> anyhow::Result<()>;

    // Stats
    fn get_stats(&self) -> anyhow::Result<Stats>;
    fn board_snapshot_stats(&self) -> anyhow::Result<(i64, Option<chrono::DateTime<chrono::Utc>>)>;

    // Board
    /// Fetch issues for all board columns in a single DB round-trip.
    ///
    /// Returns a map of Status -> Vec<Issue>, where each vec contains at most
    /// `limit_per_status` issues ordered by priority DESC, id ASC.  All five
    /// canonical statuses (backlog, todo, in_progress, review, done) are always
    /// present as keys, even if the corresponding vec is empty.
    fn list_issues_by_status(
        &self,
        limit_per_status: usize,
    ) -> anyhow::Result<std::collections::HashMap<Status, Vec<Issue>>>;
}

// ── Input types ───────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct CreateIssueInput {
    pub parent_id: Option<i64>,
    pub title: String,
    pub description: String,
    pub status: Status,
    pub priority: Priority,
    pub kind: Kind,
    pub assignee: Option<String>,
    pub labels: Vec<String>,
    pub files: Vec<String>,
    pub actor: Option<String>,
}

#[derive(Debug, Default)]
pub struct UpdateIssueInput {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<Status>,
    pub priority: Option<Priority>,
    pub kind: Option<Kind>,
    pub assignee: Option<String>,
    pub parent_id: Option<Option<i64>>, // Some(Some(id)) = set, Some(None) = clear, None = no change
    pub actor: Option<String>,
}

#[derive(Debug)]
pub struct AddCommentInput {
    pub issue_id: i64,
    pub body: String,
    pub author: Option<String>,
}

// ── Stats ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Default, serde::Serialize)]
pub struct Stats {
    pub total: u64,
    pub by_status: std::collections::HashMap<String, u64>,
    pub by_priority: std::collections::HashMap<String, u64>,
    pub by_kind: std::collections::HashMap<String, u64>,
}

// ── SqliteRepository ──────────────────────────────────────────────────────────

pub struct SqliteRepository {
    pub(crate) conn: Connection,
}

impl SqliteRepository {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode = WAL;")?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        conn.execute_batch("PRAGMA busy_timeout = 5000;")?;
        conn.set_prepared_statement_cache_capacity(32);
        Ok(Self { conn })
    }

    pub fn open_in_memory() -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory()?;
        schema::initialize(&conn)?;
        conn.set_prepared_statement_cache_capacity(32);
        Ok(Self { conn })
    }
}

// ── DB path resolution ────────────────────────────────────────────────────────

/// Resolve the database path using priority order:
/// 1. Explicit path argument
/// 2. BMO_DB environment variable (handled by clap via `env`)
/// 3. Walk up from CWD to find .bmo/issues.db
pub fn find_db(explicit: Option<&str>) -> anyhow::Result<PathBuf> {
    if let Some(p) = explicit {
        return Ok(PathBuf::from(p));
    }
    if let Ok(p) = std::env::var("BMO_DB") {
        return Ok(PathBuf::from(p));
    }
    let mut dir = std::env::current_dir()?;
    loop {
        let candidate = dir.join(".bmo").join("issues.db");
        if candidate.exists() {
            return Ok(candidate);
        }
        if !dir.pop() {
            break;
        }
    }
    anyhow::bail!("not in a bmo project — run `bmo init` first")
}

/// Open and return a SqliteRepository, running schema initialization.
pub fn open_db(path: &Path) -> anyhow::Result<SqliteRepository> {
    let repo = SqliteRepository::open(path)?;
    schema::initialize(&repo.conn)?;
    Ok(repo)
}
