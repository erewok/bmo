use clap::Args;

use crate::config::find_bmo_dir;
use crate::db::{Repository, open_db};
use crate::filter::FilterBuilder;
use crate::output::{OutputMode, make_printer};

#[derive(Args)]
pub struct ListArgs {
    /// Find everything: short-circuit all other filters and select all issues.
    #[arg(long)]
    pub all: bool,
    /// Filter by status (repeatable)
    #[arg(short, long)]
    pub status: Vec<String>,
    /// Filter by priority (repeatable)
    #[arg(short, long)]
    pub priority: Vec<String>,
    /// Filter by kind (repeatable)
    #[arg(short = 'T', long = "kind")]
    pub kind: Vec<String>,
    /// Filter by assignee
    #[arg(short, long)]
    pub assignee: Option<String>,
    /// Filter by label (AND semantics, repeatable)
    #[arg(short, long)]
    pub label: Vec<String>,
    /// Filter by parent ID
    #[arg(long)]
    pub parent: Option<String>,
    /// Search in title and description
    #[arg(long)]
    pub search: Option<String>,
    /// Maximum number of results
    #[arg(long, default_value = "50")]
    pub limit: usize,
    /// Sort field
    #[arg(long)]
    pub sort: Option<String>,
    /// Include done issues
    #[arg(long)]
    pub include_done: bool,
    /// Compact one-line-per-issue output
    #[arg(long)]
    pub oneline: bool,
}

impl ListArgs {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.all
            && [
                !self.status.is_empty(),
                !self.priority.is_empty(),
                !self.kind.is_empty(),
                self.assignee.is_some(),
                !self.label.is_empty(),
                self.parent.is_some(),
                self.search.is_some(),
            ]
            .iter()
            .any(|&x| x)
        {
            println!(
                "--all overrides all other filters: ignoring all filters and selecting all issues"
            );
        }
        Ok(())
    }
}

pub fn run(args: &ListArgs, json: bool) -> anyhow::Result<()> {
    args.validate()?;
    let bmo_dir = find_bmo_dir()?;
    let repo = open_db(&bmo_dir.join("issues.db"))?;
    let printer = make_printer(if json {
        OutputMode::Json
    } else if args.oneline {
        OutputMode::Oneline
    } else {
        OutputMode::Human
    });

    let filter = FilterBuilder {
        statuses: args.status.clone(),
        priorities: args.priority.clone(),
        kinds: args.kind.clone(),
        assignee: args.assignee.clone(),
        labels: args.label.clone(),
        parent_id: args.parent.clone(),
        search: args.search.clone(),
        limit: args.limit,
        offset: 0,
        findall: args.all,
        include_done: args.include_done,
    }
    .build()?;

    let issues = repo.list_issues(filter)?;
    printer.print_issue_list(&issues);
    Ok(())
}
