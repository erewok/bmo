use std::collections::{HashSet, VecDeque};

use sea_query::{Alias, Cond, Expr, ExprTrait, Query, SqliteQueryBuilder};
use sea_query_rusqlite::rusqlite::{Transaction, TransactionBehavior};
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
    /// in the currently stored relations.
    ///
    /// All four directional relation kinds contribute a forward edge:
    ///   Blocks(current, X)        → forward neighbor X  (from_id = current)
    ///   DependencyOf(current, X)  → forward neighbor X  (from_id = current)
    ///   DependsOn(X, current)     → forward neighbor X  (to_id   = current)
    ///   BlockedBy(X, current)     → forward neighbor X  (to_id   = current)
    /// `RelatesTo`/`Duplicates`/`DuplicateOf` are informational only and are
    /// never DAG edges, so they are excluded from this traversal.
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

            // Query only DAG neighbors of `current`.
            let (sql, values) = Query::select()
                .column(RelationIden::FromId)
                .column(RelationIden::ToId)
                .column(relation_col())
                .from(RelationIden::Table)
                .cond_where(
                    Cond::any()
                        .add(
                            Cond::all()
                                .add(Expr::col(relation_col()).is_in([
                                    RelationKind::Blocks.label(),
                                    RelationKind::DependencyOf.label(),
                                ]))
                                .add(Expr::col(RelationIden::FromId).eq(current)),
                        )
                        .add(
                            Cond::all()
                                .add(Expr::col(relation_col()).is_in([
                                    RelationKind::DependsOn.label(),
                                    RelationKind::BlockedBy.label(),
                                ]))
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
                    k if k == RelationKind::Blocks.label()
                        || k == RelationKind::DependencyOf.label() =>
                    {
                        to_id
                    }
                    k if k == RelationKind::DependsOn.label()
                        || k == RelationKind::BlockedBy.label() =>
                    {
                        from_id
                    }
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
        if from_id == to_id {
            return Err(BmoError::Validation("Cannot link an issue to itself".into()).into());
        }

        // The cycle check and the INSERT share one IMMEDIATE transaction, which
        // takes the write lock before the check reads. A concurrent `link add`
        // waits for it (up to the busy timeout) and then checks against this
        // relation, so two links that each pass alone cannot together commit
        // a cycle. Returning before `commit()` rolls the transaction back.
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;

        // Cycle check: reject any DAG edge that would create a cycle.
        // Blocks(A, B) adds DAG edge A→B; a cycle exists if B can already reach A.
        // DependencyOf(A, B) adds DAG edge A→B (same direction as Blocks; it is
        //   the semantic inverse of DependsOn, not of Blocks).
        // DependsOn(A, B) adds DAG edge B→A; a cycle exists if A can already reach B.
        // BlockedBy(A, B) adds DAG edge B→A (same direction as DependsOn; it is
        //   the semantic inverse of Blocks).
        if kind.is_dag_edge() {
            let (dag_from, dag_to) = match kind {
                RelationKind::Blocks | RelationKind::DependencyOf => (from_id, to_id),
                RelationKind::DependsOn | RelationKind::BlockedBy => (to_id, from_id),
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

        let id: i64 = match tx.query_row(query.as_str(), &*values.as_params(), |r| r.get(0)) {
            Ok(id) => id,
            Err(rusqlite::Error::SqliteFailure(e, _))
                if e.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                anyhow::bail!("relation already exists");
            }
            Err(e) => return Err(e.into()),
        };
        tx.commit()?;
        Ok(Relation {
            id,
            from_id,
            to_id,
            kind,
        })
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

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use sea_query_rusqlite::rusqlite;

    use super::super::{CreateIssueInput, SqliteRepository, open_db};
    use crate::errors::BmoError;
    use crate::model::{Kind, Priority, RelationKind, Status};

    const ALL_KINDS: [RelationKind; 7] = [
        RelationKind::Blocks,
        RelationKind::BlockedBy,
        RelationKind::DependsOn,
        RelationKind::DependencyOf,
        RelationKind::RelatesTo,
        RelationKind::Duplicates,
        RelationKind::DuplicateOf,
    ];

    fn create_issue(repo: &SqliteRepository) -> i64 {
        repo.create_issue_impl(&CreateIssueInput {
            parent_id: None,
            title: "issue".to_string(),
            description: String::new(),
            status: Status::Todo,
            priority: Priority::None,
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
    fn add_relation_rejects_a_self_link_of_every_kind() {
        let repo = SqliteRepository::open_in_memory().unwrap();
        let id = create_issue(&repo);

        for kind in ALL_KINDS {
            let err = repo.add_relation_impl(id, kind, id).unwrap_err();
            assert!(
                matches!(
                    err.downcast_ref::<BmoError>(),
                    Some(BmoError::Validation(msg)) if msg == "Cannot link an issue to itself"
                ),
                "{kind}: {err}"
            );
        }
        assert!(repo.list_relations_impl(id).unwrap().is_empty());
    }

    // Two processes linking `1 blocks 2` and `2 blocks 1` at the same time must
    // not both succeed. The other writer holds the write lock with `2 blocks 1`
    // inserted but uncommitted while `add_relation` runs; `add_relation` has to
    // wait for that commit and then see the edge, instead of checking against
    // the older snapshot.
    #[test]
    fn add_relation_checks_for_a_cycle_after_a_concurrent_writer_commits() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("issues.db");
        let repo = open_db(&db_path).unwrap();
        let first = create_issue(&repo);
        let second = create_issue(&repo);

        let other_writer = open_db(&db_path).unwrap();
        other_writer.conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        other_writer
            .conn
            .execute(
                "INSERT INTO issue_relations (from_id, to_id, relation) VALUES (?1, ?2, 'blocks')",
                rusqlite::params![second, first],
            )
            .unwrap();

        let (sender, receiver) = mpsc::channel();
        let linker = thread::spawn(move || {
            let result = repo.add_relation_impl(first, RelationKind::Blocks, second);
            sender.send(()).unwrap();
            result.map(|_| ()).map_err(|e| e.to_string())
        });

        // `add_relation` must still be waiting on the write lock.
        assert!(
            receiver.recv_timeout(Duration::from_millis(300)).is_err(),
            "add_relation finished while another writer held the lock"
        );
        other_writer.conn.execute_batch("COMMIT").unwrap();

        assert_eq!(
            linker.join().unwrap(),
            Err(
                "validation error: adding this link would create a cycle in the dependency graph"
                    .to_string()
            )
        );
    }
}
