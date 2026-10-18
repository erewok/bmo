use chrono::{DateTime, Utc};
use sea_query::enum_def;
use serde::{Deserialize, Serialize};

use super::IssueId;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[enum_def(table_name = "comments")] // Generate CommentIden for use in sea-query
pub struct Comment {
    pub id: i64,
    pub issue_id: IssueId,
    pub body: String,
    pub author: Option<String>,
    pub created_at: DateTime<Utc>,
}
