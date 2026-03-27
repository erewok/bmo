use std::collections::{HashSet, VecDeque};

use sea_query::{Alias, Cond, Expr, ExprTrait, Query, SqliteQueryBuilder};
use sea_query_rusqlite::{RusqliteBinder, rusqlite};

use crate::errors::BmoError;
use crate::model::{Relation, RelationIden, RelationKind};

use super::SqliteRepository;

// The DB column for the relation kind is named "relation" — use Alias since
// the RelationIden::Kind variant maps to the struct field name, not the column name.
fn relation_col() -> Alias {
    Alias::new("relation")
}

impl SqliteRepository {
    /// Returns true if `target` is reachable from `start` by following DAG forward edges
    /// (Blocks: from→to, DependsOn: to→from) in the currently stored relations.
    ///
    /// Uses per-node DB queries during BFS so only traversed edges are loaded, keeping
    /// each `add_relation_impl` call efficient even as the graph grows.
    fn can_reach_impl(&self, start: i64, target: i64) -> anyhow::Result<bool> {
        if start == target {
            return Ok(true);
        }

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(start);

        while let Some(current) = queue.pop_front() {
            if !visited.insert(current) {
                continue;
            }

            // Query only DAG neighbors of `current`:
            //   Blocks(current, X)    → forward neighbor X  (from_id = current)
            //   DependsOn(X, current) → forward neighbor X  (to_id   = current)
            let (sql, values) = Query::select()
                .column(RelationIden::FromId)
                .column(RelationIden::ToId)
                .column(relation_col())
                .from(RelationIden::Table)
                .cond_where(
                    Cond::any()
                        .add(
                            Cond::all()
                                .add(Expr::col(relation_col()).eq(RelationKind::Blocks.label()))
                                .add(Expr::col(RelationIden::FromId).eq(current)),
                        )
                        .add(
                            Cond::all()
                                .add(Expr::col(relation_col()).eq(RelationKind::DependsOn.label()))
                                .add(Expr::col(RelationIden::ToId).eq(current)),
                        ),
                )
                .build_rusqlite(SqliteQueryBuilder);

            let mut stmt = self.conn.prepare_cached(sql.as_str())?;
            let rows = stmt.query_map(&*values.as_params(), |r| {
                let from_id: i64 = r.get(0)?;
                let to_id: i64 = r.get(1)?;
                let kind_str: String = r.get(2)?;
                Ok((from_id, to_id, kind_str))
            })?;

            for row in rows {
                let (from_id, to_id, kind_str) = row?;
                let next = match kind_str.as_str() {
                    k if k == RelationKind::Blocks.label() => to_id,
                    k if k == RelationKind::DependsOn.label() => from_id,
                    _ => continue,
                };
                if next == target {
                    return Ok(true);
                }
                if !visited.contains(&next) {
                    queue.push_back(next);
                }
            }
        }
        Ok(false)
    }

    pub(crate) fn add_relation_impl(
        &self,
        from_id: i64,
        kind: RelationKind,
        to_id: i64,
    ) -> anyhow::Result<Relation> {
        // Cycle check: reject any DAG edge that would create a cycle.
        // Blocks(A, B) adds DAG edge A→B; a cycle exists if B can already reach A.
        // DependsOn(A, B) adds DAG edge B→A; a cycle exists if A can already reach B.
        if kind.is_dag_edge() {
            let (dag_from, dag_to) = match kind {
                RelationKind::Blocks => (from_id, to_id),
                RelationKind::DependsOn => (to_id, from_id),
                _ => unreachable!(),
            };
            if self.can_reach_impl(dag_to, dag_from)? {
                return Err(BmoError::Validation(
                    "adding this link would create a cycle in the dependency graph".into(),
                )
                .into());
            }
        }

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
