use clap::Args;

use crate::cli::open_descendant_ids;
use crate::db::{Repository, find_db, open_db};
use crate::model::{IssueFilter, IssueId};
use crate::output::{OutputMode, make_printer};
use crate::planner::dag::{Dag, find_ready};
use crate::planner::topo::topological_levels;

#[derive(Args)]
pub struct NextArgs {
    /// Filter by assignee
    #[arg(short, long)]
    pub assignee: Option<String>,

    /// Show descendants at any depth of any listed parent (IDs like 5 or BMO-5; comma-separated values; alternatively, repeat the flag for each value)
    #[arg(long, value_delimiter = ',')]
    pub parent: Option<Vec<IssueId>>,

    /// Maximum number of results
    #[arg(long, default_value = "10")]
    pub limit: usize,
}

pub fn run(args: &NextArgs, json: bool, db: Option<String>) -> anyhow::Result<()> {
    let db_path = find_db(db.as_deref())?;
    let repo = open_db(&db_path)?;
    let printer = make_printer(if json {
        OutputMode::Json
    } else {
        OutputMode::Human
    });

    // Build the graph from every open issue, even if `--parent` narrows the
    // output. A descendant's blocker may live outside the parent, and
    // readiness depends on it.
    let all_issues = repo.list_issues(IssueFilter::default())?;
    let all_relations = repo.list_all_relations()?;

    let dag = Dag::build(&all_issues, &all_relations);
    topological_levels(&dag)?;
    let scope = args
        .parent
        .as_ref()
        .map(|parents| open_descendant_ids(&repo, parents.clone()))
        .transpose()?;
    let ready: Vec<_> = find_ready(&dag)
        .into_iter()
        .filter(|i| scope.as_ref().is_none_or(|ids| ids.contains(&i.id)))
        .filter(|i| {
            args.assignee
                .as_ref()
                .map(|a| i.assignee.as_deref() == Some(a.as_str()))
                .unwrap_or(true)
        })
        .take(args.limit)
        .cloned()
        .collect();

    printer.print_issue_list(&ready);
    Ok(())
}
