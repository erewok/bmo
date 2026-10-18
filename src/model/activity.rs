use chrono::{DateTime, Utc};
use sea_query::enum_def;
use serde::{Deserialize, Serialize};

use super::IssueId;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[enum_def(table_name = "activity_log")] // Generate ActivityEntryIden for use in sea-query
pub struct ActivityEntry {
    pub id: i64,
    pub issue_id: IssueId,
    pub kind: String,
    pub detail: Option<String>,
    pub actor: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct NewActivityEntry {
    pub issue_id: IssueId,
    pub kind: String,
    pub detail: Option<String>,
    pub actor: Option<String>,
}
