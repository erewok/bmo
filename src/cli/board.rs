use clap::Args;

use crate::config::find_bmo_dir;
use crate::db::{Repository, open_db};
use crate::filter::FilterBuilder;
use crate::model::{IssueFilter, Status};
use crate::output::{BoardColumns, OutputMode, make_printer};

#[derive(Args)]
pub struct BoardArgs {
    /// Filter by label
    #[arg(short, long)]
    pub label: Vec<String>,
    /// Filter by priority
    #[arg(short, long)]
    pub priority: Vec<String>,
    /// Filter by assignee
    #[arg(short, long)]
    pub assignee: Option<String>,
}

pub fn run(args: &BoardArgs, json: bool) -> anyhow::Result<()> {
    let bmo_dir = find_bmo_dir()?;
    let repo = open_db(&bmo_dir.join("issues.db"))?;
    let printer = make_printer(if json {
        OutputMode::Json
    } else {
        OutputMode::Human
    });

    let mut filter = FilterBuilder {
        priorities: args.priority.clone(),
        labels: args.label.clone(),
        assignee: args.assignee.clone(),
        findall: true,
        limit: 500,
        ..Default::default()
    }
    .build()?;

    let all_issues = repo.list_issues(&mut filter)?;

    let board = BoardColumns {
        backlog: all_issues
            .iter()
            .filter(|i| i.status == Status::Backlog)
            .cloned()
            .collect(),
        todo: all_issues
            .iter()
            .filter(|i| i.status == Status::Todo)
            .cloned()
            .collect(),
        in_progress: all_issues
            .iter()
            .filter(|i| i.status == Status::InProgress)
            .cloned()
            .collect(),
        review: all_issues
            .iter()
            .filter(|i| i.status == Status::Review)
            .cloned()
            .collect(),
        done: all_issues
            .iter()
            .filter(|i| i.status == Status::Done)
            .cloned()
            .collect(),
    };

    printer.print_board(&board);
    Ok(())
}
