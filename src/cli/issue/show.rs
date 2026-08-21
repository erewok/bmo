use clap::Args;

use crate::cli::parse_id;
use crate::db::{Repository, find_db, open_db};
use crate::errors::ErrorCode;
use crate::output::{IssueDetail, OutputMode, make_printer};

#[derive(Args)]
pub struct ShowArgs {
    /// Issue ID (e.g. 1 or BMO-1)
    pub id: String,
}

pub fn run(args: &ShowArgs, json: bool, db: Option<String>) -> anyhow::Result<()> {
    let db_path = find_db(db.as_deref())?;
    let repo = open_db(&db_path)?;
    let printer = make_printer(if json {
        OutputMode::Json
    } else {
        OutputMode::Human
    });

    let id = parse_id(&args.id)?;
    let issue = repo
        .get_issue(id)?
        .ok_or_else(|| anyhow::anyhow!("issue {} not found", args.id));

    match issue {
        Err(e) => {
            printer.print_error(&e.to_string(), ErrorCode::NotFound);
            std::process::exit(ErrorCode::NotFound.exit_code());
        }
        Ok(issue) => {
            let sub_issues = repo.get_sub_issues(id)?;
            let relations = repo.list_relations(id)?;
            let comments = repo.list_comments(id)?;
            let labels = repo.list_issue_labels(id)?;

            let detail = IssueDetail {
                issue,
                sub_issues,
                relations,
                comments,
                labels,
            };
            printer.print_issue_detail(&detail);
        }
    }
    Ok(())
}
