use chrono::Utc;
use rusqlite::params;
use sea_query::{Expr, ExprTrait, Order, Query, SqliteQueryBuilder};
use sea_query_rusqlite::{RusqliteBinder, rusqlite};

use crate::model::{ActivityEntry, ActivityEntryIden};
use crate::model::activity::NewActivityEntry;

use super::SqliteRepository;

impl SqliteRepository {
    pub(crate) fn log_activity_impl(&self, entry: &NewActivityEntry) -> anyhow::Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO activity_log (issue_id, kind, detail, actor, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![entry.issue_id, entry.kind, entry.detail, entry.actor, now],
        )?;
        Ok(())
    }

    pub(crate) fn list_activity_impl(
        &self,
        issue_id: i64,
        limit: usize,
    ) -> anyhow::Result<Vec<ActivityEntry>> {
        let (sql, values) = Query::select()
            .columns([
                ActivityEntryIden::Id,
                ActivityEntryIden::IssueId,
                ActivityEntryIden::Kind,
                ActivityEntryIden::Detail,
                ActivityEntryIden::Actor,
                ActivityEntryIden::CreatedAt,
            ])
            .from(ActivityEntryIden::Table)
            .order_by(ActivityEntryIden::CreatedAt, Order::Desc)
            .and_where(Expr::col(ActivityEntryIden::IssueId).eq(issue_id))
            .limit(limit as u64)
            .build_rusqlite(SqliteQueryBuilder);
        let mut stmt = self.conn.prepare_cached(sql.as_str())?;
        let rows = stmt.query_map(&*values.as_params(), |r| {
            let created_at_str: String = r.get(5)?;
            Ok(ActivityEntry {
                id: r.get(0)?,
                issue_id: r.get(1)?,
                kind: r.get(2)?,
                detail: r.get(3)?,
                actor: r.get(4)?,
                created_at: created_at_str.parse().unwrap_or_else(|_| Utc::now()),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}
