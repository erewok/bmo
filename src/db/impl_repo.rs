use crate::model::activity::NewActivityEntry;
use crate::model::{
    ActivityEntry, Comment, Issue, IssueFile, IssueFilter, Label, Relation, RelationKind, Status,
};

use super::{
    AddCommentInput, CreateIssueInput, Repository, SqliteRepository, Stats, UpdateIssueInput,
};

impl Repository for SqliteRepository {
    fn create_issue(&self, input: &CreateIssueInput) -> anyhow::Result<Issue> {
        self.create_issue_impl(input)
    }

    fn get_issue(&self, id: i64) -> anyhow::Result<Option<Issue>> {
        self.get_issue_impl(id)
    }

    fn list_issues(&self, filter: &IssueFilter) -> anyhow::Result<Vec<Issue>> {
        self.list_issues_impl(filter)
    }

    fn count_issues(&self, filter: &IssueFilter) -> anyhow::Result<i64> {
        self.count_issues_impl(filter)
    }

    fn update_issue(&self, id: i64, input: &UpdateIssueInput) -> anyhow::Result<Issue> {
        self.update_issue_impl(id, input)
    }

    fn delete_issue(&self, id: i64) -> anyhow::Result<()> {
        self.delete_issue_impl(id)
    }

    fn truncate_issues(&self, statuses: &[Status]) -> anyhow::Result<u64> {
        self.truncate_issues_impl(statuses)
    }

    fn truncate_all_issues(&self) -> anyhow::Result<u64> {
        self.truncate_all_issues_impl()
    }

    fn get_sub_issues(&self, parent_id: i64) -> anyhow::Result<Vec<Issue>> {
        self.get_sub_issues_impl(parent_id)
    }

    fn add_comment(&self, input: &AddCommentInput) -> anyhow::Result<Comment> {
        self.add_comment_impl(input)
    }

    fn list_comments(&self, issue_id: i64) -> anyhow::Result<Vec<Comment>> {
        self.list_comments_impl(issue_id)
    }

    fn get_or_create_label(&self, name: &str, color: Option<&str>) -> anyhow::Result<Label> {
        self.get_or_create_label_impl(name, color)
    }

    fn add_label_to_issue(&self, issue_id: i64, label_id: i64) -> anyhow::Result<()> {
        self.add_label_to_issue_impl(issue_id, label_id)
    }

    fn remove_label_from_issue(&self, issue_id: i64, label_name: &str) -> anyhow::Result<()> {
        self.remove_label_from_issue_impl(issue_id, label_name)
    }

    fn list_issue_labels(&self, issue_id: i64) -> anyhow::Result<Vec<Label>> {
        self.list_issue_labels_impl(issue_id)
    }

    fn list_all_labels(&self) -> anyhow::Result<Vec<Label>> {
        self.list_all_labels_impl()
    }

    fn delete_label(&self, name: &str) -> anyhow::Result<()> {
        self.delete_label_impl(name)
    }

    fn add_relation(
        &self,
        from_id: i64,
        kind: RelationKind,
        to_id: i64,
    ) -> anyhow::Result<Relation> {
        self.add_relation_impl(from_id, kind, to_id)
    }

    fn remove_relation(&self, relation_id: i64) -> anyhow::Result<()> {
        self.remove_relation_impl(relation_id)
    }

    fn list_relations(&self, issue_id: i64) -> anyhow::Result<Vec<Relation>> {
        self.list_relations_impl(issue_id)
    }

    fn list_all_relations(&self) -> anyhow::Result<Vec<Relation>> {
        self.list_all_relations_impl()
    }

    fn log_activity(&self, entry: &NewActivityEntry) -> anyhow::Result<()> {
        self.log_activity_impl(entry)
    }

    fn list_activity(&self, issue_id: i64, limit: usize) -> anyhow::Result<Vec<ActivityEntry>> {
        self.list_activity_impl(issue_id, limit)
    }

    fn add_file(&self, issue_id: i64, path: &str) -> anyhow::Result<IssueFile> {
        self.add_file_impl(issue_id, path)
    }

    fn remove_file(&self, issue_id: i64, path: &str) -> anyhow::Result<()> {
        self.remove_file_impl(issue_id, path)
    }

    fn list_files(&self, issue_id: i64) -> anyhow::Result<Vec<IssueFile>> {
        self.list_files_impl(issue_id)
    }

    fn get_meta(&self, key: &str) -> anyhow::Result<Option<String>> {
        let result = self.conn.query_row(
            "SELECT value FROM meta WHERE key = ?1",
            rusqlite::params![key],
            |r| r.get(0),
        );
        match result {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn set_meta(&self, key: &str, value: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
            rusqlite::params![key, value],
        )?;
        Ok(())
    }

    fn get_stats(&self) -> anyhow::Result<Stats> {
        self.get_stats_impl()
    }

    fn board_snapshot_stats(&self) -> anyhow::Result<(i64, Option<chrono::DateTime<chrono::Utc>>)> {
        self.board_snapshot_stats_impl()
    }

    fn list_issues_by_status(
        &self,
        limit_per_status: usize,
    ) -> anyhow::Result<std::collections::HashMap<crate::model::Status, Vec<Issue>>> {
        self.list_issues_by_status_impl(limit_per_status)
    }
}
