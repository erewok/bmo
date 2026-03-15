use chrono::Utc;
use sea_query::{Expr, ExprTrait, Order, Query, SqliteQueryBuilder};
use sea_query_rusqlite::RusqliteBinder;

use crate::model::{Comment, CommentIden};

use super::{AddCommentInput, SqliteRepository};

impl SqliteRepository {
    pub(crate) fn add_comment_impl(&self, input: &AddCommentInput) -> anyhow::Result<Comment> {
        let now = Utc::now().to_rfc3339();
        let (query, values) = Query::insert()
            .into_table(CommentIden::Table)
            .columns([
                CommentIden::IssueId,
                CommentIden::Body,
                CommentIden::Author,
                CommentIden::CreatedAt,
            ])
            .values_panic([
                input.issue_id.into(),
                input.body.clone().into(),
                input.author.clone().into(),
                now.clone().into(),
            ])
            .returning_col(CommentIden::Id)
            .build_rusqlite(SqliteQueryBuilder);

        let mut stmt = self.conn.prepare(query.as_str())?;
        let mut rows = stmt.query(&*values.as_params())?;
        let id: i64 = rows
            .next()?
            .ok_or_else(|| anyhow::anyhow!("INSERT returned no rows"))?
            .get(0)?;

        Ok(Comment {
            id,
            issue_id: input.issue_id,
            body: input.body.clone(),
            author: input.author.clone(),
            created_at: now.parse().unwrap_or_else(|_| Utc::now()),
        })
    }

    pub(crate) fn list_comments_impl(&self, issue_id: i64) -> anyhow::Result<Vec<Comment>> {
        let (query, values) = Query::select()
            .columns([
                CommentIden::Id,
                CommentIden::IssueId,
                CommentIden::Body,
                CommentIden::Author,
                CommentIden::CreatedAt,
            ])
            .from(CommentIden::Table)
            .and_where(Expr::col(CommentIden::IssueId).eq(issue_id))
            .order_by(CommentIden::CreatedAt, Order::Asc)
            .build_rusqlite(SqliteQueryBuilder);

        let mut stmt = self.conn.prepare_cached(query.as_str())?;
        let rows = stmt.query_map(&*values.as_params(), |r| {
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
