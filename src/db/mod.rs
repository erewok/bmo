//! Database layer: the [`Repository`] trait and its SQLite-backed implementation.
//!
//! The primary entry points for callers are [`find_db`] to locate the database
//! file and [`open_db`] to open a [`SqliteRepository`].

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

/// Defines all database operations used by bmo.
///
/// The only provided implementation is [`SqliteRepository`]. The trait exists
/// to allow alternative backends and test doubles.
pub trait Repository {
    // Issues

    /// Insert a new issue and return it with its assigned `id`.
    fn create_issue(&self, input: &CreateIssueInput) -> anyhow::Result<Issue>;

    /// Fetch a single issue by `id`. Returns `None` if no such issue exists.
    fn get_issue(&self, id: i64) -> anyhow::Result<Option<Issue>>;

    /// Return all issues matching `filter`, in priority-descending, id-ascending order.
    fn list_issues(&self, filter: IssueFilter) -> anyhow::Result<Vec<Issue>>;

    /// Return the count of issues matching `filter` without loading the rows.
    fn count_issues(&self, filter: IssueFilter) -> anyhow::Result<i64>;

    /// Apply `input` fields to the issue with `id` and return the updated issue.
    fn update_issue(&self, id: i64, input: &UpdateIssueInput) -> anyhow::Result<Issue>;

    /// Permanently delete the issue with `id`.
    fn delete_issue(&self, id: i64) -> anyhow::Result<()>;

    /// Delete all issues whose status is in `statuses` and return the count of deleted rows.
    /// Sub-issues referencing deleted issues have their `parent_id` set to `NULL` (`ON DELETE SET NULL`); they are not deleted.
    /// The entire deletion is issued as a single `DELETE ... WHERE status IN (...)` statement.
    fn truncate_issues(&self, statuses: &[Status]) -> anyhow::Result<u64>;

    /// Delete all issues regardless of status and return the count of deleted rows.
    /// Sub-issues referencing deleted issues have their `parent_id` set to `NULL` (`ON DELETE SET NULL`); they are not deleted.
    /// `truncate_all_issues` uses an unconditional `DELETE FROM issues`.
    fn truncate_all_issues(&self) -> anyhow::Result<u64>;

    /// Return all direct children of `parent_id`.
    fn get_sub_issues(&self, parent_id: i64) -> anyhow::Result<Vec<Issue>>;

    // Comments

    /// Append a comment to an issue and return the stored comment.
    fn add_comment(&self, input: &AddCommentInput) -> anyhow::Result<Comment>;

    /// Return all comments for `issue_id` in ascending creation order.
    fn list_comments(&self, issue_id: i64) -> anyhow::Result<Vec<Comment>>;

    // Labels

    /// Look up a label by `name`, creating it with the optional `color` if absent.
    fn get_or_create_label(&self, name: &str, color: Option<&str>) -> anyhow::Result<Label>;

    /// Attach label `label_id` to issue `issue_id`.
    fn add_label_to_issue(&self, issue_id: i64, label_id: i64) -> anyhow::Result<()>;

    /// Remove the label named `label_name` from issue `issue_id`.
    fn remove_label_from_issue(&self, issue_id: i64, label_name: &str) -> anyhow::Result<()>;

    /// Return all labels attached to `issue_id`.
    fn list_issue_labels(&self, issue_id: i64) -> anyhow::Result<Vec<Label>>;

    /// Return every label defined in the repository.
    fn list_all_labels(&self) -> anyhow::Result<Vec<Label>>;

    /// Delete the label named `name` and remove it from all issues.
    fn delete_label(&self, name: &str) -> anyhow::Result<()>;

    // Relations

    /// Create a directed relation of `kind` from `from_id` to `to_id`.
    fn add_relation(
        &self,
        from_id: i64,
        kind: RelationKind,
        to_id: i64,
    ) -> anyhow::Result<Relation>;

    /// Delete the relation with `relation_id`.
    fn remove_relation(&self, relation_id: i64) -> anyhow::Result<()>;

    /// Return all relations where `issue_id` appears as either endpoint.
    fn list_relations(&self, issue_id: i64) -> anyhow::Result<Vec<Relation>>;

    /// Return every relation in the repository.
    fn list_all_relations(&self) -> anyhow::Result<Vec<Relation>>;

    // Activity

    /// Append an activity log entry for an issue.
    fn log_activity(&self, entry: &NewActivityEntry) -> anyhow::Result<()>;

    /// Return up to `limit` activity entries for `issue_id`, newest first.
    fn list_activity(&self, issue_id: i64, limit: usize) -> anyhow::Result<Vec<ActivityEntry>>;

    // Files

    /// Attach a file path to an issue and return the stored record.
    fn add_file(&self, issue_id: i64, path: &str) -> anyhow::Result<IssueFile>;

    /// Remove the file with `path` from `issue_id`.
    fn remove_file(&self, issue_id: i64, path: &str) -> anyhow::Result<()>;

    /// Return all files attached to `issue_id`.
    fn list_files(&self, issue_id: i64) -> anyhow::Result<Vec<IssueFile>>;

    // Meta

    /// Retrieve the string value stored under `key`, if any.
    fn get_meta(&self, key: &str) -> anyhow::Result<Option<String>>;

    /// Store `value` under `key`, replacing any existing value.
    fn set_meta(&self, key: &str, value: &str) -> anyhow::Result<()>;

    // Stats

    /// Return aggregate counts of issues grouped by status, priority, and kind.
    fn get_stats(&self) -> anyhow::Result<Stats>;

    /// Return `(total_issue_count, timestamp_of_last_update)` for SSE change detection.
    fn board_snapshot_stats(&self) -> anyhow::Result<(i64, Option<chrono::DateTime<chrono::Utc>>)>;

    // Board

    /// Fetch issues for all board columns, running one query per status (5 queries total).
    ///
    /// Returns a map of `Status` -> `Vec<Issue>`, where each vec contains at most
    /// `limit_per_status` issues ordered by priority DESC, id ASC.  All five
    /// canonical statuses (backlog, todo, in_progress, review, done) are always
    /// present as keys, even if the corresponding vec is empty.
    fn list_issues_by_status(
        &self,
        limit_per_status: usize,
    ) -> anyhow::Result<std::collections::HashMap<Status, Vec<Issue>>>;
}

// ── Input types ───────────────────────────────────────────────────────────────

/// Input for creating a new issue.
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

/// Input for a partial update to an existing issue. `None` fields are left unchanged.
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

/// Input for adding a comment to an issue.
#[derive(Debug)]
pub struct AddCommentInput {
    pub issue_id: i64,
    pub body: String,
    pub author: Option<String>,
}

// ── Stats ─────────────────────────────────────────────────────────────────────

/// Aggregate issue counts returned by [`Repository::get_stats`].
#[derive(Debug, Default, serde::Serialize)]
pub struct Stats {
    pub total: u64,
    pub by_status: std::collections::HashMap<String, u64>,
    pub by_priority: std::collections::HashMap<String, u64>,
    pub by_kind: std::collections::HashMap<String, u64>,
}

// ── SqliteRepository ──────────────────────────────────────────────────────────

/// SQLite-backed implementation of [`Repository`].
pub struct SqliteRepository {
    pub(crate) conn: Connection,
}

impl SqliteRepository {
    /// Open the database at `path`. The schema is not initialized; call [`open_db`] instead.
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode = WAL;")?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        conn.execute_batch("PRAGMA busy_timeout = 5000;")?;
        conn.set_prepared_statement_cache_capacity(32);
        Ok(Self { conn })
    }

    /// Open an in-memory SQLite database with the schema pre-initialized. Intended for tests.
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
