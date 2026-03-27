// Raw SQL is intentional here — consistent with get_stats_impl and
// board_snapshot_stats_impl.  sea-query does not support self-JOINs with
// table aliases cleanly, so we drop down to a prepared statement.

use crate::model::{ConflictingIssue, FileConflict, Status};

use super::SqliteRepository;

impl SqliteRepository {
    pub(crate) fn list_file_conflicts_impl(
        &self,
        issue_id: i64,
    ) -> anyhow::Result<Vec<FileConflict>> {
        let sql = "
            SELECT
                f1.path        AS file,
                i.id           AS conflict_id,
                i.title        AS conflict_title,
                i.status       AS conflict_status
            FROM issue_files f1
            JOIN issue_files f2
                ON  f2.path     = f1.path
                AND f2.issue_id != f1.issue_id
            JOIN issues i
                ON  i.id        = f2.issue_id
                AND i.status    = ?
            WHERE f1.issue_id = ?
            ORDER BY f1.path, i.id
        ";

        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(
            rusqlite::params![Status::InProgress.label(), issue_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?, // file
                    r.get::<_, i64>(1)?,    // conflict_id
                    r.get::<_, String>(2)?, // conflict_title
                    r.get::<_, String>(3)?, // conflict_status
                ))
            },
        )?;

        // Collect flat rows and group by file path.
        let mut result: Vec<FileConflict> = Vec::new();
        for row in rows {
            let (file, id, title, status_str) = row?;
            let status = status_str.parse::<Status>().unwrap_or(Status::InProgress);
            let issue = ConflictingIssue { id, title, status };
            if let Some(last) = result.last_mut()
                && last.file == file
            {
                last.conflicts_with.push(issue);
                continue;
            }

            result.push(FileConflict {
                file,
                conflicts_with: vec![issue],
            });
        }

        Ok(result)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::db::{CreateIssueInput, Repository, SqliteRepository};
    use crate::model::{Kind, Priority, Status};

    fn make_issue(repo: &SqliteRepository, title: &str, status: Status) -> i64 {
        repo.create_issue(&CreateIssueInput {
            parent_id: None,
            title: title.to_string(),
            description: String::new(),
            status,
            priority: Priority::Medium,
            kind: Kind::Task,
            assignee: None,
            labels: vec![],
            files: vec![],
            actor: None,
        })
        .unwrap()
        .id
    }

    #[test]
    fn no_file_attachments_returns_empty() {
        let repo = SqliteRepository::open_in_memory().unwrap();
        let id = make_issue(&repo, "A", Status::InProgress);
        let conflicts = repo.list_file_conflicts(id).unwrap();
        assert!(conflicts.is_empty());
    }

    #[test]
    fn attachments_but_no_other_in_progress_returns_empty() {
        let repo = SqliteRepository::open_in_memory().unwrap();
        let id = make_issue(&repo, "A", Status::InProgress);
        repo.add_file(id, "src/lib.rs").unwrap();

        // Another issue with the same file but NOT in-progress
        let other = make_issue(&repo, "B", Status::Todo);
        repo.add_file(other, "src/lib.rs").unwrap();

        let conflicts = repo.list_file_conflicts(id).unwrap();
        assert!(conflicts.is_empty());
    }

    #[test]
    fn one_file_shared_with_one_in_progress_issue() {
        let repo = SqliteRepository::open_in_memory().unwrap();
        let id = make_issue(&repo, "A", Status::InProgress);
        repo.add_file(id, "src/lib.rs").unwrap();

        let other = make_issue(&repo, "B", Status::InProgress);
        repo.add_file(other, "src/lib.rs").unwrap();

        let conflicts = repo.list_file_conflicts(id).unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].file, "src/lib.rs");
        assert_eq!(conflicts[0].conflicts_with.len(), 1);
        assert_eq!(conflicts[0].conflicts_with[0].id, other);
    }

    #[test]
    fn two_files_shared_with_two_different_in_progress_issues() {
        let repo = SqliteRepository::open_in_memory().unwrap();
        let id = make_issue(&repo, "A", Status::InProgress);
        repo.add_file(id, "src/foo.rs").unwrap();
        repo.add_file(id, "src/bar.rs").unwrap();

        let other1 = make_issue(&repo, "B", Status::InProgress);
        repo.add_file(other1, "src/foo.rs").unwrap();

        let other2 = make_issue(&repo, "C", Status::InProgress);
        repo.add_file(other2, "src/bar.rs").unwrap();

        let conflicts = repo.list_file_conflicts(id).unwrap();
        assert_eq!(conflicts.len(), 2);

        // Results are ORDER BY f1.path, so bar.rs < foo.rs
        assert_eq!(conflicts[0].file, "src/bar.rs");
        assert_eq!(conflicts[0].conflicts_with[0].id, other2);
        assert_eq!(conflicts[1].file, "src/foo.rs");
        assert_eq!(conflicts[1].conflicts_with[0].id, other1);
    }

    #[test]
    fn conflicts_with_done_or_todo_not_returned() {
        let repo = SqliteRepository::open_in_memory().unwrap();
        let id = make_issue(&repo, "A", Status::InProgress);
        repo.add_file(id, "src/lib.rs").unwrap();

        for status in [Status::Done, Status::Todo, Status::Backlog, Status::Review] {
            let other = make_issue(&repo, &format!("Other {:?}", status), status);
            repo.add_file(other, "src/lib.rs").unwrap();
        }

        let conflicts = repo.list_file_conflicts(id).unwrap();
        assert!(
            conflicts.is_empty(),
            "expected no conflicts, got: {:?}",
            conflicts
        );
    }
}
