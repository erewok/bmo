use chrono::Utc;
use rusqlite::params;

use crate::model::Comment;

use super::{AddCommentInput, SqliteRepository};

impl SqliteRepository {
    pub(crate) fn add_comment_impl(&self, input: &AddCommentInput) -> anyhow::Result<Comment> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO comments (issue_id, body, author, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![input.issue_id, input.body, input.author, now],
        )?;
        let id = self.conn.last_insert_rowid();
        Ok(Comment {
            id,
            issue_id: input.issue_id,
            body: input.body.clone(),
            author: input.author.clone(),
            created_at: now.parse().unwrap_or_else(|_| Utc::now()),
        })
    }

    pub(crate) fn list_comments_impl(&self, issue_id: i64) -> anyhow::Result<Vec<Comment>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, issue_id, body, author, created_at FROM comments WHERE issue_id = ?1 ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![issue_id], |r| {
            let created_at_str: String = r.get(4)?;
            Ok(Comment {
                id: r.get(0)?,
                issue_id: r.get(1)?,
                body: r.get(2)?,
                author: r.get(3)?,
                created_at: created_at_str.parse().unwrap_or_else(|_| Utc::now()),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}
