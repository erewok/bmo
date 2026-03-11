use chrono::{DateTime, Utc};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub title: String,
    pub description: String,
    pub status: Status,
    pub priority: Priority,
    pub kind: Kind,
    pub assignee: Option<String>,
    pub labels: Vec<String>,
    pub files: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Issue {
    pub fn display_id(&self) -> String {
        format!("BMO-{}", self.id)
    }
}

// ── Status ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Backlog,
    Todo,
    #[value(name = "in-progress")]
    InProgress,
    Review,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Priority {
    None,
    Low,
    Medium,
    High,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Bug,
    Feature,
    Task,
    Epic,
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
    pub include_done: bool,
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
