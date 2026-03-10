use rusqlite::params;

use crate::model::Label;

use super::SqliteRepository;

impl SqliteRepository {
    pub(crate) fn get_or_create_label_impl(
        &self,
        name: &str,
        color: Option<&str>,
    ) -> anyhow::Result<Label> {
        // Try to find existing
        let existing = self.conn.query_row(
            "SELECT id, name, color FROM labels WHERE name = ?1",
            params![name],
            |r| {
                Ok(Label {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    color: r.get(2)?,
                })
            },
        );
        match existing {
            Ok(label) => Ok(label),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                self.conn.execute(
                    "INSERT INTO labels (name, color) VALUES (?1, ?2)",
                    params![name, color],
                )?;
                let id = self.conn.last_insert_rowid();
                Ok(Label {
                    id,
                    name: name.to_string(),
                    color: color.map(str::to_string),
                })
            }
            Err(e) => Err(e.into()),
        }
    }

    pub(crate) fn add_label_to_issue_impl(
        &self,
        issue_id: i64,
        label_id: i64,
    ) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO issue_labels (issue_id, label_id) VALUES (?1, ?2)",
            params![issue_id, label_id],
        )?;
        Ok(())
    }

    pub(crate) fn remove_label_from_issue_impl(
        &self,
        issue_id: i64,
        label_name: &str,
    ) -> anyhow::Result<()> {
        self.conn.execute(
            "DELETE FROM issue_labels WHERE issue_id = ?1 AND label_id = (SELECT id FROM labels WHERE name = ?2)",
            params![issue_id, label_name],
        )?;
        Ok(())
    }

    pub(crate) fn list_issue_labels_impl(&self, issue_id: i64) -> anyhow::Result<Vec<Label>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT l.id, l.name, l.color FROM labels l JOIN issue_labels il ON il.label_id = l.id WHERE il.issue_id = ?1 ORDER BY l.name",
        )?;
        let rows = stmt.query_map(params![issue_id], |r| {
            Ok(Label {
                id: r.get(0)?,
                name: r.get(1)?,
                color: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub(crate) fn list_all_labels_impl(&self) -> anyhow::Result<Vec<Label>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, color FROM labels ORDER BY name")?;
        let rows = stmt.query_map([], |r| {
            Ok(Label {
                id: r.get(0)?,
                name: r.get(1)?,
                color: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub(crate) fn delete_label_impl(&self, name: &str) -> anyhow::Result<()> {
        self.conn
            .execute("DELETE FROM labels WHERE name = ?1", params![name])?;
        Ok(())
    }
}
