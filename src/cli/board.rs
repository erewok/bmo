use clap::Args;

use crate::db::{Repository, find_db, open_db};
use crate::filter::FilterBuilder;
use crate::model::Status;
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

pub fn run(args: &BoardArgs, json: bool, db: Option<String>) -> anyhow::Result<()> {
    let db_path = find_db(db.as_deref())?;
    let repo = open_db(&db_path)?;
    let printer = make_printer(if json {
        OutputMode::Json
    } else {
        OutputMode::Human
    });

    let filter = FilterBuilder {
        priorities: args.priority.clone(),
        labels: args.label.clone(),
        assignee: args.assignee.clone(),
        findall: true,
        limit: 500,
        ..Default::default()
    }
    .build()?;

    let all_issues = repo.list_issues(filter)?;

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
