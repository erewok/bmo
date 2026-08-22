use clap::Args;

use crate::cli::parse_id;
use crate::db::{Repository, UpdateIssueInput, find_db, open_db};
use crate::model::Status;
use crate::output::{OutputMode, make_printer};

#[derive(Args)]
pub struct MoveArgs {
    /// Issue ID
    pub id: String,
    /// New status
    #[arg(short, long)]
    pub status: String,
}

pub fn run(args: &MoveArgs, json: bool, db: Option<String>) -> anyhow::Result<()> {
    run_move(&args.id, &args.status, json, db)
}

pub fn run_move(
    id_str: &str,
    status_str: &str,
    json: bool,
    db: Option<String>,
) -> anyhow::Result<()> {
    let db_path = find_db(db.as_deref())?;
    let repo = open_db(&db_path)?;
    let printer = make_printer(if json {
        OutputMode::Json
    } else {
        OutputMode::Human
    });

    let id = parse_id(id_str)?;
    let status: Status = status_str.parse()?;

    let input = UpdateIssueInput {
        status: Some(status),
        ..Default::default()
    };

    let issue = repo.update_issue(id, &input)?;

    if json {
        printer.print_issue(&issue);
    } else {
        println!("Moved {} → {}", issue.display_id(), status);
    }
    Ok(())
}
