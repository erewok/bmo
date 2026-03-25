use serde::Serialize;

/// Represents a file that is attached to more than one active issue,
/// along with the set of issues that share it.
#[derive(Debug, Serialize)]
pub struct FileConflict {
    pub file: String,
    pub conflicts_with: Vec<ConflictingIssue>,
}

/// A summary of an issue that conflicts (shares a file) with another issue.
#[derive(Debug, Serialize)]
pub struct ConflictingIssue {
    pub id: i64,
    pub title: String,
    pub status: String,
}
