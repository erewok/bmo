use chrono::Utc;
use rusqlite::params;

use crate::model::IssueFile;

use super::SqliteRepository;

impl SqliteRepository {
    pub(crate) fn add_file_impl(&self, issue_id: i64, path: &str) -> anyhow::Result<IssueFile> {
        let now = Utc::now().to_rfc3339();
        let result = self.conn.execute(
            "INSERT INTO issue_files (issue_id, path, added_at) VALUES (?1, ?2, ?3)",
            params![issue_id, path, now],
        );
        match result {
            Ok(_) => {}
            Err(rusqlite::Error::SqliteFailure(e, _))
                if e.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                // Already attached — return the existing record
                return self.conn.query_row(
                    "SELECT id, issue_id, path, added_at FROM issue_files WHERE issue_id = ?1 AND path = ?2",
                    params![issue_id, path],
                    |r| {
                        let added_at_str: String = r.get(3)?;
                        Ok(IssueFile {
                            id: r.get(0)?,
                            issue_id: r.get(1)?,
                            path: r.get(2)?,
                            added_at: added_at_str.parse().unwrap_or_else(|_| Utc::now()),
                        })
                    },
                ).map_err(Into::into);
            }
            Err(e) => return Err(e.into()),
        }
        let id = self.conn.last_insert_rowid();
        Ok(IssueFile {
            id,
            issue_id,
            path: path.to_string(),
            added_at: now.parse().unwrap_or_else(|_| Utc::now()),
        })
    }

    pub(crate) fn remove_file_impl(&self, issue_id: i64, path: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "DELETE FROM issue_files WHERE issue_id = ?1 AND path = ?2",
            params![issue_id, path],
        )?;
        Ok(())
    }

    pub(crate) fn list_files_impl(&self, issue_id: i64) -> anyhow::Result<Vec<IssueFile>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, issue_id, path, added_at FROM issue_files WHERE issue_id = ?1 ORDER BY path",
        )?;
        let rows = stmt.query_map(params![issue_id], |r| {
            let added_at_str: String = r.get(3)?;
            Ok(IssueFile {
                id: r.get(0)?,
                issue_id: r.get(1)?,
                path: r.get(2)?,
                added_at: added_at_str.parse().unwrap_or_else(|_| Utc::now()),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}
