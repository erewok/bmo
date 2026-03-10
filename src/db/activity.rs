use chrono::Utc;
use rusqlite::params;

use crate::model::ActivityEntry;
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
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, issue_id, kind, detail, actor, created_at FROM activity_log WHERE issue_id = ?1 ORDER BY created_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![issue_id, limit as i64], |r| {
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
