use chrono::{DateTime, Utc};
use clap::ValueEnum;
use sea_query::{Cond, Expr, ExprTrait, Order, Query, SelectStatement, enum_def};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use super::LabelIden;

/// A single tracked work item.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[enum_def(table_name = "issues")] // Generate IssueIden for use in sea-query
pub struct Issue {
    /// Auto-assigned numeric identifier.
    pub id: i64,
    /// Parent issue id, if this is a sub-issue.
    pub parent_id: Option<i64>,
    pub title: String,
    pub description: String,
    pub status: Status,
    pub priority: Priority,
    pub kind: Kind,
    pub assignee: Option<String>,
    /// Labels attached to this issue (by name).
    pub labels: Vec<String>,
    /// File paths attached to this issue.
    pub files: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Issue {
    pub fn display_id(&self) -> String {
        format!("BMO-{}", self.id)
    }
}

/// A single tracked work item.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[enum_def(table_name = "issue_labels")] // Generate IssueLabelIden for use in sea-query
pub struct IssueLabel {
    pub issue_id: i64,
    pub label_id: i64,
}

// ── Status ────────────────────────────────────────────────────────────────────

/// Workflow state of an issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// Not yet scheduled for active work.
    Backlog,
    /// Scheduled and ready to start.
    Todo,
    /// Currently being worked on.
    #[value(name = "in-progress")]
    InProgress,
    /// Work complete, awaiting review.
    Review,
    /// Closed and resolved.
    Done,
}

impl Status {
    pub fn icon(self) -> &'static str {
        match self {
            Status::Backlog => "○",
            Status::Todo => "●",
            Status::InProgress => "◐",
            Status::Review => "◎",
            Status::Done => "✔",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Status::Backlog => "backlog",
            Status::Todo => "todo",
            Status::InProgress => "in-progress",
            Status::Review => "review",
            Status::Done => "done",
        }
    }

    pub fn all() -> &'static [Status] {
        &[
            Status::Backlog,
            Status::Todo,
            Status::InProgress,
            Status::Review,
            Status::Done,
        ]
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

impl FromStr for Status {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "backlog" => Ok(Status::Backlog),
            "todo" => Ok(Status::Todo),
            "in-progress" | "in_progress" | "inprogress" => Ok(Status::InProgress),
            "review" => Ok(Status::Review),
            "done" => Ok(Status::Done),
            _ => anyhow::bail!("unknown status: {s}"),
        }
    }
}

// ── Priority ──────────────────────────────────────────────────────────────────

/// Relative urgency of an issue. Variants are ordered from lowest to highest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Priority {
    /// No priority assigned.
    None,
    /// Low urgency.
    Low,
    /// Normal urgency.
    Medium,
    /// Elevated urgency.
    High,
    /// Requires immediate attention.
    Critical,
}

impl Priority {
    pub fn icon(self) -> &'static str {
        match self {
            Priority::Critical => "⏫",
            Priority::High => "↑",
            Priority::Medium => "↔",
            Priority::Low => "↓",
            Priority::None => "•",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Priority::Critical => "critical",
            Priority::High => "high",
            Priority::Medium => "medium",
            Priority::Low => "low",
            Priority::None => "none",
        }
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

impl FromStr for Priority {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "critical" => Ok(Priority::Critical),
            "high" => Ok(Priority::High),
            "medium" => Ok(Priority::Medium),
            "low" => Ok(Priority::Low),
            "none" => Ok(Priority::None),
            _ => anyhow::bail!("unknown priority: {s}"),
        }
    }
}

// ── Kind ──────────────────────────────────────────────────────────────────────

/// Classification of an issue by type of work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A defect or regression to fix.
    Bug,
    /// New functionality to add.
    Feature,
    /// A discrete unit of work.
    Task,
    /// A large body of work spanning multiple issues.
    Epic,
    /// Maintenance or non-functional work.
    Chore,
}

impl Kind {
    pub fn icon(self) -> &'static str {
        match self {
            Kind::Bug => "■",
            Kind::Feature => "✦",
            Kind::Task => "▶",
            Kind::Epic => "⬡",
            Kind::Chore => "⚒",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Bug => "bug",
            Kind::Feature => "feature",
            Kind::Task => "task",
            Kind::Epic => "epic",
            Kind::Chore => "chore",
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

impl FromStr for Kind {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "bug" => Ok(Kind::Bug),
            "feature" => Ok(Kind::Feature),
            "task" => Ok(Kind::Task),
            "epic" => Ok(Kind::Epic),
            "chore" => Ok(Kind::Chore),
            _ => anyhow::bail!("unknown kind: {s}"),
        }
    }
}

// ── IssueFilter ───────────────────────────────────────────────────────────────

/// Parameters for filtering issue queries. All fields are optional; unset fields are ignored.
#[derive(Debug, Default, Clone)]
pub struct IssueFilter {
    pub status: Option<Vec<Status>>,
    pub priority: Option<Vec<Priority>>,
    pub kind: Option<Vec<Kind>>,
    pub assignee: Option<String>,
    pub labels: Option<Vec<String>>,
    pub parent_id: Option<i64>,
    pub search: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    // By default filters above do not include `done` status`
    pub include_done: bool,
    // short-circuit everything above and select all issues.
    pub findall: bool,
}

impl IssueFilter {
    pub fn all() -> Self {
        Self {
            findall: true,
            ..Default::default()
        }
    }

    pub fn into_issue_query(&mut self) -> SelectStatement {
        // Build a dynamic SQL query based on which filters are set.
        let mut binding = Query::select();
        let mut query = binding
            .columns([
                IssueIden::Id,
                IssueIden::ParentId,
                IssueIden::Title,
                IssueIden::Description,
                IssueIden::Status,
                IssueIden::Priority,
                IssueIden::Kind,
                IssueIden::Assignee,
                IssueIden::CreatedAt,
                IssueIden::UpdatedAt,
            ])
            .from(IssueIden::Table);

        if self.findall { // '--all'
            return query.take();
            // No additional status filter needed, include all statuses
        } else if let Some(statuses) = &self.status {
            query = query.and_where(Expr::col(IssueIden::Status).is_in(statuses.iter().map(|s| s.label())));
        } else {
            // By default, exclude done issues
            query = query.and_where(Expr::col(IssueIden::Status).ne("done"));
        }

        // Apply filters if specified. Each filter is optional, and if provided should be applied as an AND condition.
        query.apply_if(self.priority.take(), |q, v| {
            q.and_where(Expr::col(IssueIden::Priority).is_in(v.iter().map(|p| p.label())));
        });
        query.apply_if(self.kind.take(), |q, v| {
            q.and_where(Expr::col(IssueIden::Kind).is_in(v.iter().map(|k| k.label())));
        });
        query.apply_if(self.assignee.take(), |q, v| {
            q.and_where(Expr::col(IssueIden::Assignee).eq(v.as_str()));
        });
        query.apply_if(self.parent_id.take(), |q, v| {
            q.and_where(Expr::col(IssueIden::ParentId).eq(v));
        });
        query.apply_if(self.search.take(), |q, v| {
            q.cond_where(
                Cond::any()
                    .add(
                        Expr::col(IssueIden::Title).like(format!("%{}%", v).as_str())
                    )
                    .add(
                        Expr::col(IssueIden::Description).like(format!("%{}%", v).as_str())
                    )
            );
        });
        if self.labels.is_some() {
            // Issues must have all specified labels.
            // When filtering by labels, we need to join the issue_labels to labels table.
            let labels = self.labels.as_ref().unwrap().iter().map(|s| s.as_str());
            let labels_len = self.labels.as_ref().unwrap().len();
            let mut binding = Query::select();
            let subselect = binding
                .expr(Expr::value(1))
                .from(IssueLabelIden::Table)
                .and_where(Expr::col(IssueLabelIden::IssueId).equals(IssueIden::Id))
                .and_where(
                    Expr::col(IssueLabelIden::LabelId).in_subquery(
                    Query::select()
                        .column(LabelIden::Id)
                        .from(LabelIden::Table)
                        .and_where(Expr::col(LabelIden::Name).is_in(labels))
                        .take()
                    )
                );
            query.expr_as(Expr::exists(subselect.take()), "label_match_count").take().gt(labels_len as i64 - 1);
        }

        query = query.order_by(IssueIden::Priority, Order::Desc).order_by(IssueIden::Id, Order::Asc);
        query.apply_if(self.limit, |q, v| { q.limit(v as u64); });
        query.apply_if(self.offset, |q, v| { q.offset(v as u64); });
        query.take()
        // query.build_rusqlite(SqliteQueryBuilder)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_round_trip() {
        for s in Status::all() {
            let label = s.label();
            let parsed: Status = label.parse().unwrap();
            assert_eq!(*s, parsed);
        }
    }

    #[test]
    fn status_in_progress_aliases() {
        assert_eq!("in-progress".parse::<Status>().unwrap(), Status::InProgress);
        assert_eq!("in_progress".parse::<Status>().unwrap(), Status::InProgress);
    }

    #[test]
    fn priority_ordering() {
        assert!(Priority::Critical > Priority::High);
        assert!(Priority::High > Priority::Medium);
        assert!(Priority::Medium > Priority::Low);
        assert!(Priority::Low > Priority::None);
    }

    #[test]
    fn kind_round_trip() {
        for k in [
            Kind::Bug,
            Kind::Feature,
            Kind::Task,
            Kind::Epic,
            Kind::Chore,
        ] {
            let label = k.label();
            let parsed: Kind = label.parse().unwrap();
            assert_eq!(k, parsed);
        }
    }

    #[test]
    fn display_id() {
        use chrono::Utc;
        let issue = Issue {
            id: 42,
            parent_id: None,
            title: "test".into(),
            description: "".into(),
            status: Status::Todo,
            priority: Priority::Medium,
            kind: Kind::Task,
            assignee: None,
            labels: vec![],
            files: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert_eq!(issue.display_id(), "BMO-42");
    }
}
