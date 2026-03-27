use chrono::Utc;
use sea_query::{Expr, ExprTrait, Query, SqliteQueryBuilder};
use sea_query_rusqlite::RusqliteBinder;

use crate::errors::BmoError;
use crate::model::{Issue, IssueIden, Status};

use super::{ClaimIssueInput, SqliteRepository};

impl SqliteRepository {
    pub(crate) fn claim_issue_impl(&self, input: &ClaimIssueInput) -> anyhow::Result<Issue> {
        let now = Utc::now().to_rfc3339();

        let mut query = Query::update();
        query
            .table(IssueIden::Table)
            .value(IssueIden::Status, Status::InProgress.label())
            .value(IssueIden::UpdatedAt, now)
            .and_where(Expr::col(IssueIden::Id).eq(input.issue_id))
            .and_where(Expr::col(IssueIden::Status).is_not_in([Status::InProgress.label()]));

        if let Some(assignee) = &input.assignee {
            query.value(IssueIden::Assignee, Some(assignee.clone()));
        }

        let (sql, values) = query.build_rusqlite(SqliteQueryBuilder);
        let rows_affected = self.conn.execute(sql.as_str(), &*values.as_params())?;

        if rows_affected == 1 {
            let issue = self
                .get_issue_impl(input.issue_id)?
                .ok_or_else(|| {
                    BmoError::Db(format!(
                        "unexpected state: issue {} missing after successful UPDATE",
                        input.issue_id
                    ))
                })?;
            return Ok(issue);
        }

        // rows_affected == 0: either not found or already in-progress
        match self.get_issue_impl(input.issue_id)? {
            None => Err(BmoError::NotFound(format!("issue {} not found", input.issue_id)).into()),
            Some(issue) if issue.status == Status::InProgress => Err(BmoError::Conflict(format!(
                "issue {} is already in-progress",
                input.issue_id
            ))
            .into()),
            Some(_) => Err(BmoError::Db(format!(
                "unexpected state: issue {} was not updated but status is not in-progress",
                input.issue_id
            ))
            .into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tempfile::TempDir;

    use crate::db::{ClaimIssueInput, CreateIssueInput, SqliteRepository, open_db};
    use crate::errors::BmoError;
    use crate::model::{Kind, Priority, Status};

    fn make_repo_in_memory() -> SqliteRepository {
        SqliteRepository::open_in_memory().expect("in-memory db")
    }

    fn create_todo_issue(repo: &SqliteRepository) -> i64 {
        let issue = repo
            .create_issue_impl(&CreateIssueInput {
                parent_id: None,
                title: "Test issue".into(),
                description: "desc".into(),
                status: Status::Todo,
                priority: Priority::Medium,
                kind: Kind::Task,
                assignee: None,
                labels: vec![],
                files: vec![],
                actor: None,
            })
            .expect("create issue");
        issue.id
    }

    #[test]
    fn claim_todo_issue_succeeds() {
        let repo = make_repo_in_memory();
        let id = create_todo_issue(&repo);

        let result = repo
            .claim_issue_impl(&ClaimIssueInput {
                issue_id: id,
                assignee: Some("alice".into()),
            })
            .expect("claim should succeed");

        assert_eq!(result.status, Status::InProgress);
        assert_eq!(result.assignee.as_deref(), Some("alice"));
    }

    #[test]
    fn claim_already_in_progress_returns_conflict() {
        let repo = make_repo_in_memory();
        let id = create_todo_issue(&repo);

        // First claim succeeds
        repo.claim_issue_impl(&ClaimIssueInput {
            issue_id: id,
            assignee: Some("alice".into()),
        })
        .expect("first claim should succeed");

        // Second claim should return Conflict
        let err = repo
            .claim_issue_impl(&ClaimIssueInput {
                issue_id: id,
                assignee: Some("bob".into()),
            })
            .expect_err("second claim should fail");

        let bmo_err = err.downcast::<BmoError>().expect("should be BmoError");
        assert!(
            matches!(bmo_err, BmoError::Conflict(_)),
            "expected Conflict, got {:?}",
            bmo_err
        );
    }

    #[test]
    fn claim_nonexistent_issue_returns_not_found() {
        let repo = make_repo_in_memory();

        let err = repo
            .claim_issue_impl(&ClaimIssueInput {
                issue_id: 99999,
                assignee: None,
            })
            .expect_err("claim of nonexistent issue should fail");

        let bmo_err = err.downcast::<BmoError>().expect("should be BmoError");
        assert!(
            matches!(bmo_err, BmoError::NotFound(_)),
            "expected NotFound, got {:?}",
            bmo_err
        );
    }

    #[test]
    fn claim_without_assignee_preserves_existing_assignee() {
        let repo = make_repo_in_memory();

        // Create a todo issue with a pre-set assignee
        let issue = repo
            .create_issue_impl(&CreateIssueInput {
                parent_id: None,
                title: "Test issue with assignee".into(),
                description: "desc".into(),
                status: Status::Todo,
                priority: Priority::Medium,
                kind: Kind::Task,
                assignee: Some("alice".into()),
                labels: vec![],
                files: vec![],
                actor: None,
            })
            .expect("create issue");

        // Claim without providing an assignee — existing assignee should be preserved
        let claimed = repo
            .claim_issue_impl(&ClaimIssueInput {
                issue_id: issue.id,
                assignee: None,
            })
            .expect("claim should succeed");

        assert_eq!(claimed.status, Status::InProgress);
        assert_eq!(
            claimed.assignee.as_deref(),
            Some("alice"),
            "existing assignee should not be cleared when None is passed"
        );
    }

    #[test]
    fn claim_concurrency_exactly_one_succeeds() {
        // Use a real on-disk file so two Connection objects share state.
        let dir = TempDir::new().expect("tempdir");
        let db_path = dir.path().join("issues.db");

        // Initialize schema by opening once and creating the issue
        let setup_repo = open_db(&db_path).expect("setup repo");
        let issue_id = create_todo_issue(&setup_repo);
        // Drop setup_repo to close its connection before spawning threads
        drop(setup_repo);

        // Share the path across threads
        let db_path = Arc::new(db_path);

        let path1 = Arc::clone(&db_path);
        let path2 = Arc::clone(&db_path);

        let t1 = std::thread::spawn(move || {
            let repo = open_db(&path1).expect("repo1");
            repo.claim_issue_impl(&ClaimIssueInput {
                issue_id,
                assignee: Some("thread-1".into()),
            })
        });

        let t2 = std::thread::spawn(move || {
            let repo = open_db(&path2).expect("repo2");
            repo.claim_issue_impl(&ClaimIssueInput {
                issue_id,
                assignee: Some("thread-2".into()),
            })
        });

        let r1 = t1.join().expect("thread 1 panicked");
        let r2 = t2.join().expect("thread 2 panicked");

        let successes = [&r1, &r2].iter().filter(|r| r.is_ok()).count();
        let conflicts = [r1, r2]
            .into_iter()
            .filter_map(|r| r.err())
            .filter(|e| {
                e.downcast_ref::<BmoError>()
                    .map(|b| matches!(b, BmoError::Conflict(_)))
                    .unwrap_or(false)
            })
            .count();

        assert_eq!(successes, 1, "exactly one thread should succeed");
        assert_eq!(conflicts, 1, "exactly one thread should get Conflict");
    }
}
