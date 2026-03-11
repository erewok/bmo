use chrono::Utc;
use rusqlite::params;

use crate::model::{Issue, IssueFilter, Kind, Priority, Status};

use super::{CreateIssueInput, SqliteRepository, UpdateIssueInput};

fn row_to_issue(row: &rusqlite::Row<'_>) -> rusqlite::Result<Issue> {
    Ok(Issue {
        id: row.get(0)?,
        parent_id: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        status: row.get::<_, String>(4)?.parse().unwrap_or(Status::Backlog),
        priority: row.get::<_, String>(5)?.parse().unwrap_or(Priority::None),
        kind: row.get::<_, String>(6)?.parse().unwrap_or(Kind::Task),
        assignee: row.get(7)?,
        labels: vec![],
        files: vec![],
        created_at: row
            .get::<_, String>(8)?
            .parse()
            .unwrap_or_else(|_| Utc::now()),
        updated_at: row
            .get::<_, String>(9)?
            .parse()
            .unwrap_or_else(|_| Utc::now()),
    })
}

impl SqliteRepository {
    pub(crate) fn create_issue_impl(&self, input: &CreateIssueInput) -> anyhow::Result<Issue> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO issues (parent_id, title, description, status, priority, kind, assignee, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                input.parent_id,
                input.title,
                input.description,
                input.status.label(),
                input.priority.label(),
                input.kind.label(),
                input.assignee,
                now,
                now,
            ],
        )?;
        let id = self.conn.last_insert_rowid();

        // Add labels
        for label_name in &input.labels {
            let label = self.get_or_create_label_impl(label_name, None)?;
            self.add_label_to_issue_impl(id, label.id)?;
        }

        // Add files
        for path in &input.files {
            self.add_file_impl(id, path)?;
        }

        self.get_issue_impl(id)
            .map(|opt| opt.expect("issue must exist after insert"))
    }

    pub(crate) fn get_issue_impl(&self, id: i64) -> anyhow::Result<Option<Issue>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, parent_id, title, description, status, priority, kind, assignee, created_at, updated_at
             FROM issues WHERE id = ?1",
        )?;
        let result = stmt.query_row(params![id], row_to_issue);
        match result {
            Ok(mut issue) => {
                issue.labels = self.get_issue_label_names(id)?;
                issue.files = self.get_issue_file_paths(id)?;
                Ok(Some(issue))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub(crate) fn list_issues_impl(&self, filter: &IssueFilter) -> anyhow::Result<Vec<Issue>> {
        let mut sql = String::from(
            "SELECT id, parent_id, title, description, status, priority, kind, assignee, created_at, updated_at FROM issues WHERE 1=1",
        );
        let mut bind: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if !filter.include_done {
            if let Some(statuses) = &filter.status {
                if !statuses.is_empty() {
                    let placeholders = statuses
                        .iter()
                        .enumerate()
                        .map(|(i, _)| format!("?{}", bind.len() + i + 1))
                        .collect::<Vec<_>>()
                        .join(", ");
                    sql.push_str(&format!(" AND status IN ({placeholders})"));
                    for s in statuses {
                        bind.push(Box::new(s.label().to_string()));
                    }
                }
            } else {
                // Default: exclude done
                let idx = bind.len() + 1;
                sql.push_str(&format!(" AND status != ?{idx}"));
                bind.push(Box::new("done".to_string()));
            }
        }

        if let Some(priorities) = &filter.priority
            && !priorities.is_empty()
        {
            let placeholders = priorities
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", bind.len() + i + 1))
                .collect::<Vec<_>>()
                .join(", ");
            sql.push_str(&format!(" AND priority IN ({placeholders})"));
            for p in priorities {
                bind.push(Box::new(p.label().to_string()));
            }
        }

        if let Some(kinds) = &filter.kind
            && !kinds.is_empty()
        {
            let placeholders = kinds
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", bind.len() + i + 1))
                .collect::<Vec<_>>()
                .join(", ");
            sql.push_str(&format!(" AND kind IN ({placeholders})"));
            for k in kinds {
                bind.push(Box::new(k.label().to_string()));
            }
        }

        if let Some(assignee) = &filter.assignee {
            let idx = bind.len() + 1;
            sql.push_str(&format!(" AND assignee = ?{idx}"));
            bind.push(Box::new(assignee.clone()));
        }

        if let Some(parent_id) = filter.parent_id {
            let idx = bind.len() + 1;
            sql.push_str(&format!(" AND parent_id = ?{idx}"));
            bind.push(Box::new(parent_id));
        }

        if let Some(search) = &filter.search {
            let idx = bind.len() + 1;
            sql.push_str(&format!(
                " AND (title LIKE ?{idx} OR description LIKE ?{idx})"
            ));
            bind.push(Box::new(format!("%{search}%")));
        }

        // Label filter: require all specified labels via EXISTS subqueries (AND semantics).
        if let Some(label_filter) = &filter.labels
            && !label_filter.is_empty()
        {
            for label_name in label_filter {
                let idx = bind.len() + 1;
                sql.push_str(&format!(
                    " AND EXISTS (SELECT 1 FROM issue_labels il JOIN labels l ON l.id = il.label_id WHERE il.issue_id = issues.id AND l.name = ?{idx})"
                ));
                bind.push(Box::new(label_name.clone()));
            }
        }

        sql.push_str(" ORDER BY priority DESC, id ASC");

        if let Some(limit) = filter.limit {
            let idx = bind.len() + 1;
            sql.push_str(&format!(" LIMIT ?{idx}"));
            bind.push(Box::new(limit as i64));
        }

        if let Some(offset) = filter.offset {
            let idx = bind.len() + 1;
            sql.push_str(&format!(" OFFSET ?{idx}"));
            bind.push(Box::new(offset as i64));
        }

        let mut stmt = self.conn.prepare(&sql)?;
        let refs: Vec<&dyn rusqlite::ToSql> = bind.iter().map(|b| b.as_ref()).collect();
        let rows = stmt.query_map(refs.as_slice(), row_to_issue)?;
        let mut issues = Vec::new();
        for r in rows {
            let mut issue = r?;
            issue.labels = self.get_issue_label_names(issue.id)?;
            issue.files = self.get_issue_file_paths(issue.id)?;
            issues.push(issue);
        }

        Ok(issues)
    }

    pub(crate) fn count_issues_impl(&self, filter: &IssueFilter) -> anyhow::Result<i64> {
        let mut sql = String::from("SELECT COUNT(*) FROM issues WHERE 1=1");
        let mut bind: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if !filter.include_done {
            if let Some(statuses) = &filter.status {
                if !statuses.is_empty() {
                    let placeholders = statuses
                        .iter()
                        .enumerate()
                        .map(|(i, _)| format!("?{}", bind.len() + i + 1))
                        .collect::<Vec<_>>()
                        .join(", ");
                    sql.push_str(&format!(" AND status IN ({placeholders})"));
                    for s in statuses {
                        bind.push(Box::new(s.label().to_string()));
                    }
                }
            } else {
                let idx = bind.len() + 1;
                sql.push_str(&format!(" AND status != ?{idx}"));
                bind.push(Box::new("done".to_string()));
            }
        }

        if let Some(priorities) = &filter.priority
            && !priorities.is_empty()
        {
            let placeholders = priorities
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", bind.len() + i + 1))
                .collect::<Vec<_>>()
                .join(", ");
            sql.push_str(&format!(" AND priority IN ({placeholders})"));
            for p in priorities {
                bind.push(Box::new(p.label().to_string()));
            }
        }

        if let Some(kinds) = &filter.kind
            && !kinds.is_empty()
        {
            let placeholders = kinds
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", bind.len() + i + 1))
                .collect::<Vec<_>>()
                .join(", ");
            sql.push_str(&format!(" AND kind IN ({placeholders})"));
            for k in kinds {
                bind.push(Box::new(k.label().to_string()));
            }
        }

        if let Some(assignee) = &filter.assignee {
            let idx = bind.len() + 1;
            sql.push_str(&format!(" AND assignee = ?{idx}"));
            bind.push(Box::new(assignee.clone()));
        }

        if let Some(parent_id) = filter.parent_id {
            let idx = bind.len() + 1;
            sql.push_str(&format!(" AND parent_id = ?{idx}"));
            bind.push(Box::new(parent_id));
        }

        if let Some(search) = &filter.search {
            let idx = bind.len() + 1;
            sql.push_str(&format!(
                " AND (title LIKE ?{idx} OR description LIKE ?{idx})"
            ));
            bind.push(Box::new(format!("%{search}%")));
        }

        // Label filter: require all specified labels via EXISTS subqueries.
        if let Some(label_filter) = &filter.labels
            && !label_filter.is_empty()
        {
            for label_name in label_filter {
                let idx = bind.len() + 1;
                sql.push_str(&format!(
                    " AND EXISTS (SELECT 1 FROM issue_labels il JOIN labels l ON l.id = il.label_id WHERE il.issue_id = issues.id AND l.name = ?{idx})"
                ));
                bind.push(Box::new(label_name.clone()));
            }
        }

        let mut stmt = self.conn.prepare(&sql)?;
        let refs: Vec<&dyn rusqlite::ToSql> = bind.iter().map(|b| b.as_ref()).collect();
        let count: i64 = stmt.query_row(refs.as_slice(), |r| r.get(0))?;
        Ok(count)
    }

    pub(crate) fn update_issue_impl(
        &self,
        id: i64,
        input: &UpdateIssueInput,
    ) -> anyhow::Result<Issue> {
        let now = Utc::now().to_rfc3339();
        let mut sets = vec!["updated_at = ?1".to_string()];
        let mut bind: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now)];

        macro_rules! push {
            ($field:expr, $val:expr) => {{
                let idx = bind.len() + 1;
                sets.push(format!("{} = ?{idx}", $field));
                bind.push(Box::new($val));
            }};
        }

        if let Some(v) = &input.title {
            push!("title", v.clone());
        }
        if let Some(v) = &input.description {
            push!("description", v.clone());
        }
        if let Some(v) = input.status {
            push!("status", v.label().to_string());
        }
        if let Some(v) = input.priority {
            push!("priority", v.label().to_string());
        }
        if let Some(v) = input.kind {
            push!("kind", v.label().to_string());
        }
        if let Some(v) = &input.assignee {
            push!("assignee", v.clone());
        }
        if let Some(parent) = &input.parent_id {
            match parent {
                Some(pid) => push!("parent_id", *pid),
                None => {
                    let idx = bind.len() + 1;
                    sets.push(format!("parent_id = ?{idx}"));
                    bind.push(Box::new(Option::<i64>::None));
                }
            }
        }

        let id_idx = bind.len() + 1;
        let sql = format!("UPDATE issues SET {} WHERE id = ?{id_idx}", sets.join(", "));
        bind.push(Box::new(id));

        let refs: Vec<&dyn rusqlite::ToSql> = bind.iter().map(|b| b.as_ref()).collect();
        let changed = self.conn.execute(&sql, refs.as_slice())?;
        if changed == 0 {
            anyhow::bail!("issue {} not found", id);
        }

        self.get_issue_impl(id)
            .map(|opt| opt.expect("issue must exist after update"))
    }

    pub(crate) fn delete_issue_impl(&self, id: i64) -> anyhow::Result<()> {
        let changed = self
            .conn
            .execute("DELETE FROM issues WHERE id = ?1", params![id])?;
        if changed == 0 {
            anyhow::bail!("issue {} not found", id);
        }
        Ok(())
    }

    pub(crate) fn get_sub_issues_impl(&self, parent_id: i64) -> anyhow::Result<Vec<Issue>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, parent_id, title, description, status, priority, kind, assignee, created_at, updated_at
             FROM issues WHERE parent_id = ?1 ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![parent_id], row_to_issue)?;
        let mut issues = Vec::new();
        for r in rows {
            let mut issue = r?;
            issue.labels = self.get_issue_label_names(issue.id)?;
            issue.files = self.get_issue_file_paths(issue.id)?;
            issues.push(issue);
        }
        Ok(issues)
    }

    pub(crate) fn get_issue_label_names(&self, issue_id: i64) -> anyhow::Result<Vec<String>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT l.name FROM labels l JOIN issue_labels il ON il.label_id = l.id WHERE il.issue_id = ?1 ORDER BY l.name",
        )?;
        let rows = stmt.query_map(params![issue_id], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<String>>>()?)
    }

    pub(crate) fn get_issue_file_paths(&self, issue_id: i64) -> anyhow::Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT path FROM issue_files WHERE issue_id = ?1 ORDER BY path")?;
        let rows = stmt.query_map(params![issue_id], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<String>>>()?)
    }

    pub(crate) fn list_issues_by_status_impl(
        &self,
        limit_per_status: usize,
    ) -> anyhow::Result<std::collections::HashMap<Status, Vec<Issue>>> {
        use std::collections::HashMap;

        // Canonical column order for the board.
        let all_statuses = [
            Status::Backlog,
            Status::Todo,
            Status::InProgress,
            Status::Review,
            Status::Done,
        ];

        // Pre-populate every key so callers always get a full map even when a
        // column is empty.
        let mut map: HashMap<Status, Vec<Issue>> = all_statuses
            .iter()
            .map(|s| (*s, Vec::new()))
            .collect();

        // Single query: fetch up to limit_per_status * 5 rows across all
        // statuses, sorted by priority DESC then id ASC (matches list_issues).
        let ceiling = (limit_per_status * all_statuses.len()) as i64;
        let mut stmt = self.conn.prepare(
            "SELECT id, parent_id, title, description, status, priority, kind, assignee, \
             created_at, updated_at \
             FROM issues \
             ORDER BY priority DESC, id ASC \
             LIMIT ?1",
        )?;

        let rows = stmt.query_map(rusqlite::params![ceiling], row_to_issue)?;
        for r in rows {
            let mut issue = r?;
            issue.labels = self.get_issue_label_names(issue.id)?;
            issue.files = self.get_issue_file_paths(issue.id)?;

            if let Some(bucket) = map.get_mut(&issue.status) {
                if bucket.len() < limit_per_status {
                    bucket.push(issue);
                }
            }
            // Issues whose status doesn't map to a canonical column are silently
            // ignored (defensive; in practice all statuses are canonical).
        }

        Ok(map)
    }

    pub(crate) fn board_snapshot_stats_impl(
        &self,
    ) -> anyhow::Result<(i64, Option<chrono::DateTime<chrono::Utc>>)> {
        let (count, max_updated): (i64, Option<String>) =
            self.conn
                .query_row("SELECT COUNT(*), MAX(updated_at) FROM issues", [], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })?;
        let max_dt = max_updated
            .as_deref()
            .and_then(|s| s.parse::<chrono::DateTime<chrono::Utc>>().ok());
        Ok((count, max_dt))
    }

    pub(crate) fn get_stats_impl(&self) -> anyhow::Result<super::Stats> {
        let total: u64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM issues", [], |r| r.get(0))?;

        let mut by_status = std::collections::HashMap::new();
        let mut stmt = self
            .conn
            .prepare("SELECT status, COUNT(*) FROM issues GROUP BY status")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, u64>(1)?)))?;
        for r in rows {
            let (k, v) = r?;
            by_status.insert(k, v);
        }

        let mut by_priority = std::collections::HashMap::new();
        let mut stmt = self
            .conn
            .prepare("SELECT priority, COUNT(*) FROM issues GROUP BY priority")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, u64>(1)?)))?;
        for r in rows {
            let (k, v) = r?;
            by_priority.insert(k, v);
        }

        let mut by_kind = std::collections::HashMap::new();
        let mut stmt = self
            .conn
            .prepare("SELECT kind, COUNT(*) FROM issues GROUP BY kind")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, u64>(1)?)))?;
        for r in rows {
            let (k, v) = r?;
            by_kind.insert(k, v);
        }

        Ok(super::Stats {
            total,
            by_status,
            by_priority,
            by_kind,
        })
    }
}
