use chrono::Utc;
use sea_query::{Expr, ExprTrait, Order, Query, SqliteQueryBuilder};
use sea_query_rusqlite::{RusqliteBinder, rusqlite};

use crate::model::{IssueFile, IssueFileIden};

use super::SqliteRepository;

impl SqliteRepository {
    pub(crate) fn add_file_impl(&self, issue_id: i64, path: &str) -> anyhow::Result<IssueFile> {
        let now = Utc::now().to_rfc3339();

        let (ins_sql, ins_values) = Query::insert()
            .into_table(IssueFileIden::Table)
            .columns([
                IssueFileIden::IssueId,
                IssueFileIden::Path,
                IssueFileIden::AddedAt,
            ])
            .values_panic([issue_id.into(), path.into(), now.clone().into()])
            .returning_all()
            .build_rusqlite(SqliteQueryBuilder);

        let mut stmt = self.conn.prepare(ins_sql.as_str())?;
        let result = stmt.query(&*ins_values.as_params());

        match result {
            Ok(mut rows) => {
                let row = rows
                    .next()?
                    .ok_or_else(|| anyhow::anyhow!("INSERT into issue_files returned no rows"))?;
                let added_at_str: String = row.get(3)?;
                Ok(IssueFile {
                    id: row.get(0)?,
                    issue_id: row.get(1)?,
                    path: row.get(2)?,
                    added_at: added_at_str.parse().unwrap_or_else(|_| Utc::now()),
                })
            }
            Err(rusqlite::Error::SqliteFailure(e, _))
                if e.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                // Already attached — return the existing record via SELECT
                let (sel_sql, sel_values) = Query::select()
                    .columns([
                        IssueFileIden::Id,
                        IssueFileIden::IssueId,
                        IssueFileIden::Path,
                        IssueFileIden::AddedAt,
                    ])
                    .from(IssueFileIden::Table)
                    .and_where(Expr::col(IssueFileIden::IssueId).eq(issue_id))
                    .and_where(Expr::col(IssueFileIden::Path).eq(path))
                    .build_rusqlite(SqliteQueryBuilder);

                let mut stmt = self.conn.prepare_cached(sel_sql.as_str())?;
                let mut rows = stmt.query(&*sel_values.as_params())?;
                let row = rows
                    .next()?
                    .ok_or_else(|| anyhow::anyhow!("existing issue_file record not found"))?;
                let added_at_str: String = row.get(3)?;
                Ok(IssueFile {
                    id: row.get(0)?,
                    issue_id: row.get(1)?,
                    path: row.get(2)?,
                    added_at: added_at_str.parse().unwrap_or_else(|_| Utc::now()),
                })
            }
            Err(e) => Err(e.into()),
        }
    }

    pub(crate) fn remove_file_impl(&self, issue_id: i64, path: &str) -> anyhow::Result<()> {
        let (sql, values) = Query::delete()
            .from_table(IssueFileIden::Table)
            .and_where(Expr::col(IssueFileIden::IssueId).eq(issue_id))
            .and_where(Expr::col(IssueFileIden::Path).eq(path))
            .build_rusqlite(SqliteQueryBuilder);

        let mut stmt = self.conn.prepare(sql.as_str())?;
        stmt.execute(&*values.as_params())?;
        Ok(())
    }

    pub(crate) fn list_files_impl(&self, issue_id: i64) -> anyhow::Result<Vec<IssueFile>> {
        let (sql, values) = Query::select()
            .columns([
                IssueFileIden::Id,
                IssueFileIden::IssueId,
                IssueFileIden::Path,
                IssueFileIden::AddedAt,
            ])
            .from(IssueFileIden::Table)
            .and_where(Expr::col(IssueFileIden::IssueId).eq(issue_id))
            .order_by(IssueFileIden::Path, Order::Asc)
            .build_rusqlite(SqliteQueryBuilder);

        let mut stmt = self.conn.prepare_cached(sql.as_str())?;
        let rows = stmt.query_map(&*values.as_params(), |r| {
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
