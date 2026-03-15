use clap::Args;

use crate::config::find_bmo_dir;
use crate::db::{Repository, open_db};
use crate::model::IssueFilter;
use crate::output::{OutputMode, make_printer};
use crate::planner::dag::{Dag, find_ready};

#[derive(Args)]
pub struct NextArgs {
    /// Filter by assignee
    #[arg(short, long)]
    pub assignee: Option<String>,
    /// Maximum number of results
    #[arg(long, default_value = "10")]
    pub limit: usize,
}

pub fn run(args: &NextArgs, json: bool) -> anyhow::Result<()> {
    let bmo_dir = find_bmo_dir()?;
    let repo = open_db(&bmo_dir.join("issues.db"))?;
    let printer = make_printer(if json {
        OutputMode::Json
    } else {
        OutputMode::Human
    });

    let all_issues = repo.list_issues(&mut IssueFilter::all())?;
    let all_relations = repo.list_all_relations()?;

    let dag = Dag::build(&all_issues, &all_relations);
    let ready: Vec<_> = find_ready(&dag)
        .into_iter()
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
