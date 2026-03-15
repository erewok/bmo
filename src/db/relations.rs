use sea_query::{Alias, Cond, Expr, ExprTrait, Query, SqliteQueryBuilder};
use sea_query_rusqlite::{RusqliteBinder, rusqlite};

use crate::model::{Relation, RelationIden, RelationKind};

use super::SqliteRepository;

// The DB column for the relation kind is named "relation" — use Alias since
// the RelationIden::Kind variant maps to the struct field name, not the column name.
fn relation_col() -> Alias {
    Alias::new("relation")
}

impl SqliteRepository {
    pub(crate) fn add_relation_impl(
        &self,
        from_id: i64,
        kind: RelationKind,
        to_id: i64,
    ) -> anyhow::Result<Relation> {
        let (query, values) = Query::insert()
            .into_table(RelationIden::Table)
            .columns([
                Alias::new("from_id"),
                Alias::new("to_id"),
                Alias::new("relation"),
            ])
            .values_panic([from_id.into(), to_id.into(), kind.label().into()])
            .returning_col(RelationIden::Id)
            .build_rusqlite(SqliteQueryBuilder);

        let mut stmt = self.conn.prepare(query.as_str())?;
        let result = stmt.query(&*values.as_params());
        match result {
            Ok(mut rows) => {
                let id: i64 = rows
                    .next()?
                    .ok_or_else(|| anyhow::anyhow!("INSERT returned no rows"))?
                    .get(0)?;
                Ok(Relation {
                    id,
                    from_id,
                    to_id,
                    kind,
                })
            }
            Err(rusqlite::Error::SqliteFailure(e, _))
                if e.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                anyhow::bail!("relation already exists");
            }
            Err(e) => Err(e.into()),
        }
    }

    pub(crate) fn remove_relation_impl(&self, relation_id: i64) -> anyhow::Result<()> {
        let (query, values) = Query::delete()
            .from_table(RelationIden::Table)
            .and_where(Expr::col(RelationIden::Id).eq(relation_id))
            .returning_col(RelationIden::Id)
            .build_rusqlite(SqliteQueryBuilder);

        let mut stmt = self.conn.prepare(query.as_str())?;
        let mut rows = stmt.query(&*values.as_params())?;
        if rows.next()?.is_none() {
            anyhow::bail!("relation {} not found", relation_id);
        }
        Ok(())
    }

    pub(crate) fn list_relations_impl(&self, issue_id: i64) -> anyhow::Result<Vec<Relation>> {
        let (query, values) = Query::select()
            .columns([RelationIden::Id, RelationIden::FromId, RelationIden::ToId])
            .column(relation_col())
            .from(RelationIden::Table)
            .cond_where(
                Cond::any()
                    .add(Expr::col(RelationIden::FromId).eq(issue_id))
                    .add(Expr::col(RelationIden::ToId).eq(issue_id)),
            )
            .build_rusqlite(SqliteQueryBuilder);

        let mut stmt = self.conn.prepare_cached(query.as_str())?;
        let rows = stmt.query_map(&*values.as_params(), |r| {
            let kind_str: String = r.get(3)?;
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                kind_str,
            ))
        })?;

        let mut relations = Vec::new();
        for r in rows {
            let (id, from_id, to_id, kind_str) = r?;
            let kind: RelationKind = kind_str.parse().unwrap_or(RelationKind::RelatesTo);
            relations.push(Relation {
                id,
                from_id,
                to_id,
                kind,
            });
        }
        Ok(relations)
    }

    pub(crate) fn list_all_relations_impl(&self) -> anyhow::Result<Vec<Relation>> {
        let (query, values) = Query::select()
            .columns([RelationIden::Id, RelationIden::FromId, RelationIden::ToId])
            .column(relation_col())
            .from(RelationIden::Table)
            .build_rusqlite(SqliteQueryBuilder);

        let mut stmt = self.conn.prepare_cached(query.as_str())?;
        let rows = stmt.query_map(&*values.as_params(), |r| {
            let kind_str: String = r.get(3)?;
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                kind_str,
            ))
        })?;
        let mut relations = Vec::new();
        for r in rows {
            let (id, from_id, to_id, kind_str) = r?;
            let kind: RelationKind = kind_str.parse().unwrap_or(RelationKind::RelatesTo);
            relations.push(Relation {
                id,
                from_id,
                to_id,
                kind,
            });
        }
        Ok(relations)
    }
}
