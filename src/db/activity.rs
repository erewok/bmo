use chrono::Utc;
use sea_query::{Expr, ExprTrait, Order, Query, SqliteQueryBuilder};
use sea_query_rusqlite::{RusqliteBinder, rusqlite};

use crate::model::activity::NewActivityEntry;
use crate::model::{ActivityEntry, ActivityEntryIden};

use super::SqliteRepository;

impl SqliteRepository {
    pub(crate) fn log_activity_impl(&self, entry: &NewActivityEntry) -> anyhow::Result<()> {
        let now = Utc::now().to_rfc3339();
        let (sql, values) = Query::insert()
            .into_table(ActivityEntryIden::Table)
            .columns([
                ActivityEntryIden::IssueId,
                ActivityEntryIden::Kind,
                ActivityEntryIden::Detail,
                ActivityEntryIden::Actor,
                ActivityEntryIden::CreatedAt,
            ])
            .values_panic([
                entry.issue_id.into(),
                entry.kind.clone().into(),
                entry.detail.clone().into(),
                entry.actor.clone().into(),
                now.into(),
            ])
            .build_rusqlite(SqliteQueryBuilder);
        self.conn.execute(sql.as_str(), &*values.as_params())?;
        Ok(())
    }

    pub(crate) fn list_activity_impl(
        &self,
        issue_id: i64,
        limit: usize,
    ) -> anyhow::Result<Vec<ActivityEntry>> {
        let mut q = Query::select();
        q.columns([
            ActivityEntryIden::Id,
            ActivityEntryIden::IssueId,
            ActivityEntryIden::Kind,
            ActivityEntryIden::Detail,
            ActivityEntryIden::Actor,
            ActivityEntryIden::CreatedAt,
        ])
        .from(ActivityEntryIden::Table)
        .order_by(ActivityEntryIden::CreatedAt, Order::Desc)
        .and_where(Expr::col(ActivityEntryIden::IssueId).eq(issue_id));

        // usize::MAX is the sentinel meaning "no limit". Passing it as u64
        // to SQLite would overflow i64 and cause a rusqlite runtime error.
        if limit != usize::MAX {
            q.limit(limit as u64);
        }

        let (sql, values) = q.build_rusqlite(SqliteQueryBuilder);
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
