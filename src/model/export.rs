use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{ActivityEntry, Comment, Issue, IssueFile, Label, Relation};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportBundle {
    pub schema_version: u32,
    pub exported_at: DateTime<Utc>,
    pub project_name: String,
    pub issues: Vec<Issue>,
    pub comments: Vec<Comment>,
    pub labels: Vec<Label>,
    pub relations: Vec<Relation>,
    pub activity: Vec<ActivityEntry>,
    pub files: Vec<IssueFile>,
}
