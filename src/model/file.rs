use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueFile {
    pub id: i64,
    pub issue_id: i64,
    pub path: String,
    pub added_at: DateTime<Utc>,
}
