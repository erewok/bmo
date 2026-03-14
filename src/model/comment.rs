use chrono::{DateTime, Utc};
use sea_query::enum_def;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[enum_def] // Generate CommentIden for use in sea-query
pub struct Comment {
    pub id: i64,
    pub issue_id: i64,
    pub body: String,
    pub author: Option<String>,
    pub created_at: DateTime<Utc>,
}
