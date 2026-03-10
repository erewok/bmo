use clap::Args;

use crate::cli::parse_id;
use crate::config::find_bmo_dir;
use crate::db::{Repository, UpdateIssueInput, open_db};
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

pub fn run(args: &MoveArgs, json: bool) -> anyhow::Result<()> {
    run_move(&args.id, &args.status, json)
}

pub fn run_move(id_str: &str, status_str: &str, json: bool) -> anyhow::Result<()> {
    let bmo_dir = find_bmo_dir()?;
    let repo = open_db(&bmo_dir.join("issues.db"))?;
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
