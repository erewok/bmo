use rusqlite::params;

use crate::model::{Relation, RelationKind};

use super::SqliteRepository;

impl SqliteRepository {
    pub(crate) fn add_relation_impl(
        &self,
        from_id: i64,
        kind: RelationKind,
        to_id: i64,
    ) -> anyhow::Result<Relation> {
        let result = self.conn.execute(
            "INSERT INTO issue_relations (from_id, to_id, relation) VALUES (?1, ?2, ?3)",
            params![from_id, to_id, kind.label()],
        );
        match result {
            Ok(_) => {}
            Err(rusqlite::Error::SqliteFailure(e, _))
                if e.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                anyhow::bail!("relation already exists");
            }
            Err(e) => return Err(e.into()),
        }
        let id = self.conn.last_insert_rowid();
        Ok(Relation {
            id,
            from_id,
            to_id,
            kind,
        })
    }

    pub(crate) fn remove_relation_impl(&self, relation_id: i64) -> anyhow::Result<()> {
        let changed = self.conn.execute(
            "DELETE FROM issue_relations WHERE id = ?1",
            params![relation_id],
        )?;
        if changed == 0 {
            anyhow::bail!("relation {} not found", relation_id);
        }
        Ok(())
    }

    pub(crate) fn list_relations_impl(&self, issue_id: i64) -> anyhow::Result<Vec<Relation>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, from_id, to_id, relation FROM issue_relations WHERE from_id = ?1 OR to_id = ?1",
        )?;
        let rows = stmt.query_map(params![issue_id], |r| {
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
        let mut stmt = self
            .conn
            .prepare("SELECT id, from_id, to_id, relation FROM issue_relations")?;
        let rows = stmt.query_map([], |r| {
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
