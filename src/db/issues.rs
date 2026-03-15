use std::collections::HashMap;

use chrono::Utc;
use sea_query::{Expr, ExprTrait, Func, Order, Query, SqliteQueryBuilder};
use sea_query_rusqlite::{RusqliteBinder, rusqlite};

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
    pub(crate) fn create_issue_impl(&self, input: &CreateIssueInput) -> anyhow::Result<Issue> {
        let now = Utc::now().to_rfc3339();
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
        let id: i64 = self
            .conn
            .query_row(sql.as_str(), &*values.as_params(), |r| r.get(0))?;

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
                Some(pid) => q.value(IssueIden::ParentId, *pid),
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
    use super::super::{CreateIssueInput, SqliteRepository};
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
}
