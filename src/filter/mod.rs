use crate::model::{IssueFilter, Kind, Priority, Status};

/// Build an IssueFilter from CLI string arguments.
#[derive(Debug, Default)]
pub struct FilterBuilder {
    pub statuses: Vec<String>,
    pub priorities: Vec<String>,
    pub kinds: Vec<String>,
    pub assignee: Option<String>,
    pub labels: Vec<String>,
    pub parent_id: Option<String>,
    pub search: Option<String>,
    pub limit: usize,
    pub offset: usize,
    /// When `true`, removes the default `status != 'done'` exclusion.
    /// All other active filters (priority, kind, assignee, labels, etc.) remain in effect.
    /// Distinct from `findall` which bypasses ALL predicates.
    pub include_done: bool,
    // --all is a short-circuit that ignores all other filters and selects all issues.
    pub findall: bool,
}

impl FilterBuilder {
    pub fn build(self) -> anyhow::Result<IssueFilter> {
        let status = if self.statuses.is_empty() {
            None
        } else {
            Some(
                self.statuses
                    .iter()
                    .map(|s| s.parse::<Status>())
                    .collect::<anyhow::Result<Vec<_>>>()?,
            )
        };

        let priority = if self.priorities.is_empty() {
            None
        } else {
            Some(
                self.priorities
                    .iter()
                    .map(|p| p.parse::<Priority>())
                    .collect::<anyhow::Result<Vec<_>>>()?,
            )
        };

        let kind = if self.kinds.is_empty() {
            None
        } else {
            Some(
                self.kinds
                    .iter()
                    .map(|k| k.parse::<Kind>())
                    .collect::<anyhow::Result<Vec<_>>>()?,
            )
        };

        let parent_id = self
            .parent_id
            .as_deref()
            .map(crate::cli::parse_id)
            .transpose()?;

        let labels = if self.labels.is_empty() {
            None
        } else {
            Some(self.labels)
        };

        Ok(IssueFilter {
            status,
            priority,
            kind,
            assignee: self.assignee,
            labels,
            parent_id,
            search: self.search,
            limit: if self.limit == 0 {
                None
            } else {
                Some(self.limit)
            },
            offset: if self.offset == 0 {
                None
            } else {
                Some(self.offset)
            },
            include_done: self.include_done,
            findall: self.findall,
        })
    }
}
