use sea_query::{Expr, ExprTrait, OnConflict, Order, Query, SqliteQueryBuilder};
use sea_query_rusqlite::{RusqliteBinder, rusqlite};

use crate::model::issue::IssueLabelIden;
use crate::model::{Label, LabelIden};

use super::SqliteRepository;

impl SqliteRepository {
    pub(crate) fn get_or_create_label_impl(
        &self,
        name: &str,
        color: Option<&str>,
    ) -> anyhow::Result<Label> {
        // Try to find existing label by name
        let (sel_sql, sel_values) = Query::select()
            .columns([LabelIden::Id, LabelIden::Name, LabelIden::Color])
            .from(LabelIden::Table)
            .and_where(Expr::col(LabelIden::Name).eq(name))
            .build_rusqlite(SqliteQueryBuilder);

        let mut stmt = self.conn.prepare_cached(sel_sql.as_str())?;
        let mut rows = stmt.query(&*sel_values.as_params())?;

        if let Some(row) = rows.next()? {
            return Ok(Label {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
            });
        }

        // Not found — insert with RETURNING id
        let (ins_sql, ins_values) = Query::insert()
            .into_table(LabelIden::Table)
            .columns([LabelIden::Name, LabelIden::Color])
            .values_panic([name.into(), color.map(str::to_string).into()])
            .returning_col(LabelIden::Id)
            .build_rusqlite(SqliteQueryBuilder);

        let mut stmt = self.conn.prepare(ins_sql.as_str())?;
        let mut rows = stmt.query(&*ins_values.as_params())?;
        let id: i64 = rows
            .next()?
            .ok_or_else(|| anyhow::anyhow!("INSERT into labels returned no rows"))?
            .get(0)?;

        Ok(Label {
            id,
            name: name.to_string(),
            color: color.map(str::to_string),
        })
    }

    pub(crate) fn add_label_to_issue_impl(
        &self,
        issue_id: i64,
        label_id: i64,
    ) -> anyhow::Result<()> {
        let (sql, values) = Query::insert()
            .into_table(IssueLabelIden::Table)
            .columns([IssueLabelIden::IssueId, IssueLabelIden::LabelId])
            .values_panic([issue_id.into(), label_id.into()])
            .on_conflict(OnConflict::new().do_nothing().to_owned())
            .build_rusqlite(SqliteQueryBuilder);

        let mut stmt = self.conn.prepare(sql.as_str())?;
        stmt.execute(&*values.as_params())?;
        Ok(())
    }

    pub(crate) fn remove_label_from_issue_impl(
        &self,
        issue_id: i64,
        label_name: &str,
    ) -> anyhow::Result<()> {
        // Subquery: SELECT id FROM labels WHERE name = ?
        let subquery = Query::select()
            .column(LabelIden::Id)
            .from(LabelIden::Table)
            .and_where(Expr::col(LabelIden::Name).eq(label_name))
            .to_owned();

        let (sql, values) = Query::delete()
            .from_table(IssueLabelIden::Table)
            .and_where(Expr::col(IssueLabelIden::IssueId).eq(issue_id))
            .and_where(Expr::col(IssueLabelIden::LabelId).in_subquery(subquery))
            .build_rusqlite(SqliteQueryBuilder);

        let mut stmt = self.conn.prepare(sql.as_str())?;
        stmt.execute(&*values.as_params())?;
        Ok(())
    }

    pub(crate) fn list_issue_labels_impl(&self, issue_id: i64) -> anyhow::Result<Vec<Label>> {
        // SELECT l.id, l.name, l.color
        // FROM labels l
        // JOIN issue_labels il ON il.label_id = l.id
        // WHERE il.issue_id = ?
        // ORDER BY l.name
        let (sql, values) = Query::select()
            .column((LabelIden::Table, LabelIden::Id))
            .column((LabelIden::Table, LabelIden::Name))
            .column((LabelIden::Table, LabelIden::Color))
            .from(LabelIden::Table)
            .inner_join(
                IssueLabelIden::Table,
                Expr::col((IssueLabelIden::Table, IssueLabelIden::LabelId))
                    .equals((LabelIden::Table, LabelIden::Id)),
            )
            .and_where(Expr::col((IssueLabelIden::Table, IssueLabelIden::IssueId)).eq(issue_id))
            .order_by((LabelIden::Table, LabelIden::Name), Order::Asc)
            .build_rusqlite(SqliteQueryBuilder);

        let mut stmt = self.conn.prepare_cached(sql.as_str())?;
        let rows = stmt.query_map(&*values.as_params(), |r| {
            Ok(Label {
                id: r.get(0)?,
                name: r.get(1)?,
                color: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub(crate) fn list_all_labels_impl(&self) -> anyhow::Result<Vec<Label>> {
        let (sql, values) = Query::select()
            .columns([LabelIden::Id, LabelIden::Name, LabelIden::Color])
            .from(LabelIden::Table)
            .order_by(LabelIden::Name, Order::Asc)
            .build_rusqlite(SqliteQueryBuilder);

        let mut stmt = self.conn.prepare_cached(sql.as_str())?;
        let rows = stmt.query_map(&*values.as_params(), |r| {
            Ok(Label {
                id: r.get(0)?,
                name: r.get(1)?,
                color: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub(crate) fn delete_label_impl(&self, name: &str) -> anyhow::Result<()> {
        let (sql, values) = Query::delete()
            .from_table(LabelIden::Table)
            .and_where(Expr::col(LabelIden::Name).eq(name))
            .build_rusqlite(SqliteQueryBuilder);

        let mut stmt = self.conn.prepare(sql.as_str())?;
        stmt.execute(&*values.as_params())?;
        Ok(())
    }
}
