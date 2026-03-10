use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEntry {
    pub id: i64,
    pub issue_id: i64,
    pub kind: String,
    pub detail: Option<String>,
    pub actor: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct NewActivityEntry {
    pub issue_id: i64,
    pub kind: String,
    pub detail: Option<String>,
    pub actor: Option<String>,
}
