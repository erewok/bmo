use std::collections::{HashMap, HashSet};

use chrono::Utc;
use sea_query::{Expr, ExprTrait, Func, Order, Query, SqliteQueryBuilder};
use sea_query_rusqlite::{RusqliteBinder, rusqlite};

use crate::errors::BmoError;
use crate::model::issue::IssueLabelIden;
use crate::model::{
    Issue, IssueFileIden, IssueFilter, IssueIden, Kind, LabelIden, Priority, Status,
};

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
    /// Returns the direct `parent_id` of `id`, or `None` if the issue has no
    /// parent (or does not exist — callers only invoke this on ids already
    /// known to exist, via the ancestor-chain walk below).
    fn parent_id_of_impl(&self, id: i64) -> anyhow::Result<Option<i64>> {
        let (sql, values) = Query::select()
            .column(IssueIden::ParentId)
            .from(IssueIden::Table)
            .and_where(Expr::col(IssueIden::Id).eq(id))
            .build_rusqlite(SqliteQueryBuilder);
        let mut stmt = self.conn.prepare_cached(sql.as_str())?;
        let result = stmt.query_row(&*values.as_params(), |r| r.get::<_, Option<i64>>(0));
        match result {
            Ok(parent_id) => Ok(parent_id),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Returns true if setting `issue_id`'s parent to `candidate_parent` would
    /// create a cycle in the `parent_id` hierarchy — i.e. `issue_id` appears
    /// somewhere in `candidate_parent`'s ancestor chain (or is
    /// `candidate_parent` itself). Mirrors `can_reach_impl` in
    /// `src/db/relations.rs`, but walks `issues.parent_id` instead of the
    /// `relations` table. A `HashSet` visited-guard prevents infinite loops
    /// should the existing data already contain a cycle.
    fn would_create_parent_cycle_impl(
        &self,
        issue_id: i64,
        candidate_parent: i64,
    ) -> anyhow::Result<bool> {
        if issue_id == candidate_parent {
            return Ok(true);
        }

        let mut visited = HashSet::new();
        let mut current = candidate_parent;
        loop {
            if !visited.insert(current) {
                // Already-corrupt data forming a cycle that doesn't involve
                // issue_id; stop walking rather than looping forever.
                return Ok(false);
            }
            match self.parent_id_of_impl(current)? {
                Some(next) if next == issue_id => return Ok(true),
                Some(next) => current = next,
                None => return Ok(false),
            }
        }
    }

    pub(crate) fn create_issue_impl(&self, input: &CreateIssueInput) -> anyhow::Result<Issue> {
        let now = Utc::now().to_rfc3339();

        // The INSERT and the self-parent check are wrapped in a single
        // transaction so a self-parented row is never durably committed or
        // visible to any other connection (e.g. the web server's SSE
        // poller). Either the insert commits cleanly, or — when a
        // self-parent is detected — the entire transaction, including the
        // insert, is rolled back atomically by dropping it without calling
        // `commit()`. This replaces a prior insert-then-compensating-DELETE
        // approach, which left a window (and a crash-durability gap)
        // between the two separately-autocommitted statements.
        let tx = self.conn.unchecked_transaction()?;
        let (sql, values) = Query::insert()
            .into_table(IssueIden::Table)
            .columns([
                IssueIden::ParentId,
                IssueIden::Title,
                IssueIden::Description,
                IssueIden::Status,
                IssueIden::Priority,
                IssueIden::Kind,
                IssueIden::Assignee,
                IssueIden::CreatedAt,
                IssueIden::UpdatedAt,
            ])
            .values_panic([
                input.parent_id.into(),
                input.title.clone().into(),
                input.description.clone().into(),
                input.status.label().into(),
                input.priority.label().into(),
                input.kind.label().into(),
                input.assignee.clone().into(),
                now.clone().into(),
                now.into(),
            ])
            .returning_col(IssueIden::Id)
            .build_rusqlite(SqliteQueryBuilder);
        let id: i64 = tx.query_row(sql.as_str(), &*values.as_params(), |r| r.get(0))?;

        // A new issue's id is only known after INSERT (SQLite AUTOINCREMENT),
        // so a self-parent can only be detected after the fact. No existing
        // issue's ancestor chain can reference an id that didn't exist before
        // this insert, so a direct self-reference is the only possible
        // violation at create time.
        if input.parent_id == Some(id) {
            // Drop without committing: SQLite rolls back the transaction,
            // so the self-parented row never becomes visible to any other
            // reader and never persists past this call, even on crash.
            drop(tx);
            return Err(
                BmoError::Validation("cannot set an issue as its own parent".into()).into(),
            );
        }

        tx.commit()?;

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
        let (sql, values) = Query::select()
            .columns([
                IssueIden::Id,
                IssueIden::ParentId,
                IssueIden::Title,
                IssueIden::Description,
                IssueIden::Status,
                IssueIden::Priority,
                IssueIden::Kind,
                IssueIden::Assignee,
                IssueIden::CreatedAt,
                IssueIden::UpdatedAt,
            ])
            .from(IssueIden::Table)
            .and_where(Expr::col(IssueIden::Id).eq(id))
            .build_rusqlite(SqliteQueryBuilder);
        let mut stmt = self.conn.prepare_cached(sql.as_str())?;
        let result = stmt.query_row(&*values.as_params(), row_to_issue);
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

    pub(crate) fn list_issues_impl(&self, filter: IssueFilter) -> anyhow::Result<Vec<Issue>> {
        let sql = filter.into_issue_query();
        let (query, values) = sql.build_rusqlite(SqliteQueryBuilder);

        let mut stmt = self.conn.prepare(query.as_str())?;
        let rows = stmt.query_map(&*values.as_params(), row_to_issue)?;
        let mut issues = Vec::new();
        for r in rows {
            issues.push(r?);
        }

        // Early-exit: nothing to hydrate.
        if issues.is_empty() {
            return Ok(issues);
        }

        let ids: Vec<i64> = issues.iter().map(|i| i.id).collect();

        // Batch label query — one round-trip for all issues.
        let (sql, values) = Query::select()
            .column((IssueLabelIden::Table, IssueLabelIden::IssueId))
            .column((LabelIden::Table, LabelIden::Name))
            .from(IssueLabelIden::Table)
            .inner_join(
                LabelIden::Table,
                Expr::col((LabelIden::Table, LabelIden::Id))
                    .equals((IssueLabelIden::Table, IssueLabelIden::LabelId)),
            )
            .and_where(
                Expr::col((IssueLabelIden::Table, IssueLabelIden::IssueId)).is_in(ids.clone()),
            )
            .order_by((LabelIden::Table, LabelIden::Name), Order::Asc)
            .build_rusqlite(SqliteQueryBuilder);

        let mut label_map: HashMap<i64, Vec<String>> = HashMap::new();
        let mut stmt = self.conn.prepare(sql.as_str())?;
        let rows = stmt.query_map(&*values.as_params(), |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (issue_id, name) = row?;
            label_map.entry(issue_id).or_default().push(name);
        }

        // Batch file query — one round-trip for all issues.
        let (sql, values) = Query::select()
            .column(IssueFileIden::IssueId)
            .column(IssueFileIden::Path)
            .from(IssueFileIden::Table)
            .and_where(Expr::col(IssueFileIden::IssueId).is_in(ids))
            .order_by(IssueFileIden::Path, Order::Asc)
            .build_rusqlite(SqliteQueryBuilder);

        let mut file_map: HashMap<i64, Vec<String>> = HashMap::new();
        let mut stmt = self.conn.prepare(sql.as_str())?;
        let rows = stmt.query_map(&*values.as_params(), |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (issue_id, path) = row?;
            file_map.entry(issue_id).or_default().push(path);
        }

        // Attach labels and files to each issue from the maps.
        for issue in &mut issues {
            issue.labels = label_map.remove(&issue.id).unwrap_or_default();
            issue.files = file_map.remove(&issue.id).unwrap_or_default();
        }

        Ok(issues)
    }

    pub(crate) fn count_issues_impl(&self, filter: IssueFilter) -> anyhow::Result<i64> {
        let inner = filter.into_issue_query();
        let mut binding = Query::select();
        let sql = binding
            .expr(Func::count(Expr::col((IssueIden::Table, IssueIden::Id))))
            .from_subquery(inner, "issues");
        let (query, values) = sql.build_rusqlite(SqliteQueryBuilder);

        let mut stmt = self.conn.prepare(query.as_str())?;
        let count = stmt
            .query_map(&*values.as_params(), |row| row.get(0))?
            .next()
            .unwrap_or(Ok(0))?;
        Ok(count)
    }

    pub(crate) fn update_issue_impl(
        &self,
        id: i64,
        input: &UpdateIssueInput,
    ) -> anyhow::Result<Issue> {
        let now = Utc::now().to_rfc3339();
        let mut q = Query::update();
        q.table(IssueIden::Table).value(IssueIden::UpdatedAt, now);

        if let Some(v) = &input.title {
            q.value(IssueIden::Title, v.clone());
        }
        if let Some(v) = &input.description {
            q.value(IssueIden::Description, v.clone());
        }
        if let Some(v) = input.status {
            q.value(IssueIden::Status, v.label().to_string());
        }
        if let Some(v) = input.priority {
            q.value(IssueIden::Priority, v.label().to_string());
        }
        if let Some(v) = input.kind {
            q.value(IssueIden::Kind, v.label().to_string());
        }
        if let Some(v) = &input.assignee {
            q.value(IssueIden::Assignee, v.clone());
        }
        // parent_id: outer None = don't touch, Some(None) = set NULL, Some(Some(x)) = set x
        if let Some(parent) = &input.parent_id {
            match parent {
                Some(pid) => {
                    if *pid == id {
                        return Err(BmoError::Validation(
                            "cannot set an issue as its own parent".into(),
                        )
                        .into());
                    }
                    if self.would_create_parent_cycle_impl(id, *pid)? {
                        return Err(BmoError::Validation(
                            "setting this parent would create a cycle in the issue hierarchy"
                                .into(),
                        )
                        .into());
                    }
                    q.value(IssueIden::ParentId, *pid)
                }
                None => q.value(IssueIden::ParentId, Option::<i64>::None),
            };
        }

        q.and_where(Expr::col(IssueIden::Id).eq(id))
            .returning_col(IssueIden::Id);

        let (sql, values) = q.build_rusqlite(SqliteQueryBuilder);
        let result = self
            .conn
            .query_row(sql.as_str(), &*values.as_params(), |r| r.get::<_, i64>(0));
        match result {
            Ok(_) => {}
            Err(rusqlite::Error::QueryReturnedNoRows) => anyhow::bail!("issue {} not found", id),
            Err(e) => return Err(e.into()),
        }

        self.get_issue_impl(id)
            .map(|opt| opt.expect("issue must exist after update"))
    }

    pub(crate) fn delete_issue_impl(&self, id: i64) -> anyhow::Result<()> {
        let (sql, values) = Query::delete()
            .from_table(IssueIden::Table)
            .and_where(Expr::col(IssueIden::Id).eq(id))
            .returning_col(IssueIden::Id)
            .build_rusqlite(SqliteQueryBuilder);
        let result = self
            .conn
            .query_row(sql.as_str(), &*values.as_params(), |r| r.get::<_, i64>(0));
        match result {
            Ok(_) => Ok(()),
            Err(rusqlite::Error::QueryReturnedNoRows) => anyhow::bail!("issue {} not found", id),
            Err(e) => Err(e.into()),
        }
    }

    pub(crate) fn truncate_issues_impl(&self, statuses: &[Status]) -> anyhow::Result<u64> {
        if statuses.is_empty() {
            // sea-query `is_in([])` generates invalid SQL; guard here.
            return Ok(0);
        }
        let (sql, values) = Query::delete()
            .from_table(IssueIden::Table)
            .and_where(Expr::col(IssueIden::Status).is_in(statuses.iter().map(|s| s.label())))
            .build_rusqlite(SqliteQueryBuilder);
        let changed = self.conn.execute(sql.as_str(), &*values.as_params())?;
        Ok(changed as u64)
    }

    pub(crate) fn truncate_all_issues_impl(&self) -> anyhow::Result<u64> {
        let (sql, values) = Query::delete()
            .from_table(IssueIden::Table)
            .build_rusqlite(SqliteQueryBuilder);
        let changed = self.conn.execute(sql.as_str(), &*values.as_params())?;
        Ok(changed as u64)
    }

    pub(crate) fn get_sub_issues_impl(&self, parent_id: i64) -> anyhow::Result<Vec<Issue>> {
        let (sql, values) = Query::select()
            .columns([
                IssueIden::Id,
                IssueIden::ParentId,
                IssueIden::Title,
                IssueIden::Description,
                IssueIden::Status,
                IssueIden::Priority,
                IssueIden::Kind,
                IssueIden::Assignee,
                IssueIden::CreatedAt,
                IssueIden::UpdatedAt,
            ])
            .from(IssueIden::Table)
            .and_where(Expr::col(IssueIden::ParentId).eq(parent_id))
            .order_by(IssueIden::Id, Order::Asc)
            .build_rusqlite(SqliteQueryBuilder);
        let mut stmt = self.conn.prepare_cached(sql.as_str())?;
        let rows = stmt.query_map(&*values.as_params(), row_to_issue)?;
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
        let (sql, values) = Query::select()
            .column(LabelIden::Name)
            .from(LabelIden::Table)
            .inner_join(
                IssueLabelIden::Table,
                Expr::col((LabelIden::Table, LabelIden::Id))
                    .equals((IssueLabelIden::Table, IssueLabelIden::LabelId)),
            )
            .and_where(Expr::col((IssueLabelIden::Table, IssueLabelIden::IssueId)).eq(issue_id))
            .order_by(LabelIden::Name, Order::Asc)
            .build_rusqlite(SqliteQueryBuilder);
        let mut stmt = self.conn.prepare_cached(sql.as_str())?;
        let rows = stmt.query_map(&*values.as_params(), |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<String>>>()?)
    }

    pub(crate) fn get_issue_file_paths(&self, issue_id: i64) -> anyhow::Result<Vec<String>> {
        let (sql, values) = Query::select()
            .column(IssueFileIden::Path)
            .from(IssueFileIden::Table)
            .and_where(Expr::col(IssueFileIden::IssueId).eq(issue_id))
            .order_by(IssueFileIden::Path, Order::Asc)
            .build_rusqlite(SqliteQueryBuilder);
        let mut stmt = self.conn.prepare_cached(sql.as_str())?;
        let rows = stmt.query_map(&*values.as_params(), |r| r.get(0))?;
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

        // Issue one query per status so each column always returns up to
        // limit_per_status items regardless of how the data is distributed
        // across statuses. All queries run on the same connection (self.conn
        // via list_issues_impl), no additional DB opens needed.

        let mut map: HashMap<Status, Vec<Issue>> = HashMap::with_capacity(all_statuses.len());
        for status in &all_statuses {
            let filter = crate::model::IssueFilter {
                status: Some(vec![*status]),
                limit: Some(limit_per_status),
                ..Default::default()
            };
            let issues = self.list_issues_impl(filter)?;
            map.insert(*status, issues);
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

#[cfg(test)]
mod tests {
    use super::super::{CreateIssueInput, SqliteRepository, UpdateIssueInput};
    use crate::errors::BmoError;
    use crate::model::{IssueFilter, Kind, Priority, Status};

    fn make_repo() -> SqliteRepository {
        SqliteRepository::open_in_memory().expect("in-memory db")
    }

    fn create_input(title: &str, status: Status) -> CreateIssueInput {
        CreateIssueInput {
            parent_id: None,
            title: title.to_string(),
            description: String::new(),
            status,
            priority: Priority::None,
            kind: Kind::Task,
            assignee: None,
            labels: vec![],
            files: vec![],
            actor: None,
        }
    }

    fn create_input_with_priority(
        title: &str,
        status: Status,
        priority: Priority,
    ) -> CreateIssueInput {
        CreateIssueInput {
            priority,
            ..create_input(title, status)
        }
    }

    /// `include_done: true` combined with a priority filter must:
    /// - Return done issues that match the priority.
    /// - Exclude issues of a different priority regardless of status.
    /// - NOT apply the default `status != 'done'` exclusion.
    #[test]
    fn include_done_with_priority_filter() {
        let repo = make_repo();

        // High-priority done — should appear.
        repo.create_issue_impl(&create_input_with_priority(
            "done-high",
            Status::Done,
            Priority::High,
        ))
        .unwrap();
        // High-priority todo — should also appear (include_done does not restrict non-done).
        repo.create_issue_impl(&create_input_with_priority(
            "todo-high",
            Status::Todo,
            Priority::High,
        ))
        .unwrap();
        // Medium-priority done — must NOT appear (wrong priority).
        repo.create_issue_impl(&create_input_with_priority(
            "done-medium",
            Status::Done,
            Priority::Medium,
        ))
        .unwrap();
        // Medium-priority todo — must NOT appear (wrong priority).
        repo.create_issue_impl(&create_input_with_priority(
            "todo-medium",
            Status::Todo,
            Priority::Medium,
        ))
        .unwrap();

        let filter = IssueFilter {
            include_done: true,
            priority: Some(vec![Priority::High]),
            ..Default::default()
        };
        let results = repo.list_issues_impl(filter).unwrap();

        let titles: Vec<&str> = results.iter().map(|i| i.title.as_str()).collect();

        assert_eq!(
            results.len(),
            2,
            "expected exactly 2 high-priority issues; got: {:?}",
            titles
        );

        // The done high-priority issue must be present (include_done is in effect).
        assert!(
            results
                .iter()
                .any(|i| i.title == "done-high" && i.status == Status::Done),
            "done-high (Status::Done) should be included when include_done=true; got: {:?}",
            titles
        );
        // The non-done high-priority issue must also be present.
        assert!(
            results.iter().any(|i| i.title == "todo-high"),
            "todo-high should be included; got: {:?}",
            titles
        );
        // No medium-priority issues should leak through.
        assert!(
            results.iter().all(|i| i.priority == Priority::High),
            "all results must have High priority; got: {:?}",
            titles
        );
    }

    #[test]
    fn truncate_empty_db_returns_zero() {
        let repo = make_repo();
        let deleted = repo.truncate_issues_impl(&[Status::Done]).unwrap();
        assert_eq!(deleted, 0, "empty DB should delete 0 rows");
    }

    #[test]
    fn truncate_with_status_done_deletes_done_leaves_others() {
        let repo = make_repo();

        repo.create_issue_impl(&create_input("done-1", Status::Done))
            .unwrap();
        repo.create_issue_impl(&create_input("done-2", Status::Done))
            .unwrap();
        repo.create_issue_impl(&create_input("open-1", Status::Todo))
            .unwrap();
        repo.create_issue_impl(&create_input("open-2", Status::InProgress))
            .unwrap();
        repo.create_issue_impl(&create_input("backlog-1", Status::Backlog))
            .unwrap();

        let deleted = repo.truncate_issues_impl(&[Status::Done]).unwrap();
        assert_eq!(deleted, 2, "should delete exactly the 2 done issues");

        // Verify surviving issues have non-done status
        let all = repo
            .list_issues_impl(crate::model::IssueFilter {
                findall: true,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(all.len(), 3, "3 non-done issues should remain");
        assert!(
            all.iter().all(|i| i.status != Status::Done),
            "no done issues should remain"
        );
    }

    #[test]
    fn truncate_empty_slice_returns_zero() {
        let repo = make_repo();

        repo.create_issue_impl(&create_input("done-a", Status::Done))
            .unwrap();
        repo.create_issue_impl(&create_input("todo-a", Status::Todo))
            .unwrap();

        let deleted = repo.truncate_issues_impl(&[]).unwrap();
        assert_eq!(deleted, 0, "empty slice should delete nothing");

        let remaining = repo
            .list_issues_impl(crate::model::IssueFilter::all())
            .unwrap();
        assert_eq!(remaining.len(), 2, "all issues should remain");
    }

    #[test]
    fn truncate_returns_correct_count() {
        let repo = make_repo();

        for i in 0..5 {
            repo.create_issue_impl(&create_input(&format!("done-{i}"), Status::Done))
                .unwrap();
        }
        repo.create_issue_impl(&create_input("review-1", Status::Review))
            .unwrap();

        let deleted = repo.truncate_issues_impl(&[Status::Done]).unwrap();
        assert_eq!(deleted, 5, "should return exact count of deleted rows");

        let filter = crate::model::IssueFilter {
            findall: true,
            ..Default::default()
        };
        let remaining = repo.list_issues_impl(filter).unwrap();
        assert_eq!(remaining.len(), 1, "only the review issue should remain");
    }

    fn create_input_with_parent(title: &str, parent_id: Option<i64>) -> CreateIssueInput {
        CreateIssueInput {
            parent_id,
            ..create_input(title, Status::Backlog)
        }
    }

    fn update_parent_input(parent_id: Option<i64>) -> UpdateIssueInput {
        UpdateIssueInput {
            title: None,
            description: None,
            status: None,
            priority: None,
            kind: None,
            assignee: None,
            parent_id: Some(parent_id),
            actor: None,
        }
    }

    fn assert_validation_err(err: anyhow::Error, expected_substring: &str) {
        match err.downcast_ref::<BmoError>() {
            Some(BmoError::Validation(msg)) => {
                assert!(
                    msg.contains(expected_substring),
                    "expected validation message to contain {expected_substring:?}, got {msg:?}"
                );
            }
            other => panic!("expected BmoError::Validation, got {other:?}"),
        }
    }

    #[test]
    fn create_issue_rejects_self_parent() {
        let repo = make_repo();

        // The next assigned id in a fresh in-memory DB is 1.
        let err = repo
            .create_issue_impl(&create_input_with_parent("self-parent", Some(1)))
            .expect_err("self-parent create must be rejected");
        assert_validation_err(err, "own parent");

        // The rejected insert must not have left a row behind.
        let all = repo.list_issues_impl(IssueFilter::all()).unwrap();
        assert!(
            all.is_empty(),
            "no issue should persist after a rejected self-parent create; got: {all:?}"
        );
    }

    #[test]
    fn create_issue_with_valid_parent_succeeds() {
        let repo = make_repo();
        let parent = repo
            .create_issue_impl(&create_input("parent", Status::Backlog))
            .unwrap();

        let child = repo
            .create_issue_impl(&create_input_with_parent("child", Some(parent.id)))
            .unwrap();
        assert_eq!(child.parent_id, Some(parent.id));
    }

    #[test]
    fn create_issue_rejects_self_parent_atomically_rolls_back_id_sequence() {
        let repo = make_repo();

        // The next assigned id in a fresh in-memory DB is 1.
        let err = repo
            .create_issue_impl(&create_input_with_parent("self-parent", Some(1)))
            .expect_err("self-parent create must be rejected");
        assert_validation_err(err, "own parent");

        // If the insert-then-check-then-delete sequence were not
        // transactional, the rejected insert's autoincrement counter would
        // still have advanced (SQLite AUTOINCREMENT never reuses ids after
        // a DELETE), so the next successfully created issue would be
        // assigned id 2, not 1. Because the whole insert + self-parent
        // check is now wrapped in a single transaction that gets rolled
        // back (not committed then compensating-deleted), the aborted
        // insert leaves no trace at all -- including in the sequence
        // counter -- and id 1 is reused.
        let next = repo
            .create_issue_impl(&create_input("first-real-issue", Status::Backlog))
            .unwrap();
        assert_eq!(
            next.id, 1,
            "rejected self-parent insert must roll back completely, including the autoincrement counter"
        );
    }

    #[test]
    fn update_issue_rejects_direct_self_parent() {
        let repo = make_repo();
        let issue = repo
            .create_issue_impl(&create_input("solo", Status::Backlog))
            .unwrap();

        let err = repo
            .update_issue_impl(issue.id, &update_parent_input(Some(issue.id)))
            .expect_err("direct self-parent edit must be rejected");
        assert_validation_err(err, "own parent");

        // parent_id must remain unchanged after the rejected edit.
        let unchanged = repo.get_issue_impl(issue.id).unwrap().unwrap();
        assert_eq!(unchanged.parent_id, None);
    }

    #[test]
    fn update_issue_rejects_two_cycle() {
        let repo = make_repo();
        let a = repo
            .create_issue_impl(&create_input("a", Status::Backlog))
            .unwrap();
        let b = repo
            .create_issue_impl(&create_input("b", Status::Backlog))
            .unwrap();

        // A's parent = B: fine.
        repo.update_issue_impl(a.id, &update_parent_input(Some(b.id)))
            .unwrap();

        // B's parent = A would close a 2-cycle: must be rejected.
        let err = repo
            .update_issue_impl(b.id, &update_parent_input(Some(a.id)))
            .expect_err("2-cycle edit must be rejected");
        assert_validation_err(err, "cycle");

        let b_after = repo.get_issue_impl(b.id).unwrap().unwrap();
        assert_eq!(
            b_after.parent_id, None,
            "B's parent_id must remain unchanged after the rejected edit"
        );
    }

    #[test]
    fn update_issue_rejects_longer_chain_cycle() {
        let repo = make_repo();
        let a = repo
            .create_issue_impl(&create_input("a", Status::Backlog))
            .unwrap();
        let b = repo
            .create_issue_impl(&create_input("b", Status::Backlog))
            .unwrap();
        let c = repo
            .create_issue_impl(&create_input("c", Status::Backlog))
            .unwrap();

        // A parent=B, B parent=C: both fine (A -> B -> C chain).
        repo.update_issue_impl(a.id, &update_parent_input(Some(b.id)))
            .unwrap();
        repo.update_issue_impl(b.id, &update_parent_input(Some(c.id)))
            .unwrap();

        // C parent=A would close the chain into a cycle: must be rejected.
        let err = repo
            .update_issue_impl(c.id, &update_parent_input(Some(a.id)))
            .expect_err("longer chain cycle edit must be rejected");
        assert_validation_err(err, "cycle");

        let c_after = repo.get_issue_impl(c.id).unwrap().unwrap();
        assert_eq!(
            c_after.parent_id, None,
            "C's parent_id must remain unchanged after the rejected edit"
        );
    }

    #[test]
    fn update_issue_valid_reparenting_succeeds() {
        let repo = make_repo();
        let epic = repo
            .create_issue_impl(&create_input("epic", Status::Backlog))
            .unwrap();
        let other_epic = repo
            .create_issue_impl(&create_input("other-epic", Status::Backlog))
            .unwrap();
        let child = repo
            .create_issue_impl(&create_input("child", Status::Backlog))
            .unwrap();

        // Legitimate, non-cyclic re-parenting must continue to work.
        let updated = repo
            .update_issue_impl(child.id, &update_parent_input(Some(epic.id)))
            .unwrap();
        assert_eq!(updated.parent_id, Some(epic.id));

        let reparented = repo
            .update_issue_impl(child.id, &update_parent_input(Some(other_epic.id)))
            .unwrap();
        assert_eq!(reparented.parent_id, Some(other_epic.id));
    }
}
