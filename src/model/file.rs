use chrono::{DateTime, Utc};
use sea_query::enum_def;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[enum_def] // Generate IssueFileIden for use in sea-query
pub struct IssueFile {
    pub id: i64,
    pub issue_id: i64,
    pub path: String,
    pub added_at: DateTime<Utc>,
}
