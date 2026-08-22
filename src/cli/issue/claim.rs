use clap::Args;

use crate::cli::parse_id;
use crate::db::{ClaimIssueInput, Repository, find_db, open_db};
use crate::errors::{BmoError, ErrorCode};

#[derive(Args)]
pub struct ClaimArgs {
    /// Issue ID (e.g. 7 or BMO-7)
    pub id: String,
    /// Assignee name to record on the issue
    #[arg(short, long)]
    pub assignee: Option<String>,
}

pub fn run(args: &ClaimArgs, json: bool, db: Option<String>) -> anyhow::Result<()> {
    let db_path = find_db(db.as_deref())?;
    let repo = open_db(&db_path)?;

    let issue_id = parse_id(&args.id)?;

    match repo.claim_issue(&ClaimIssueInput {
        issue_id,
        assignee: args.assignee.clone(),
    }) {
        Ok(issue) => {
            let display = issue.display_id();
            let file_conflicts = repo.list_file_conflicts(issue_id)?;

            if json {
                let mut envelope = serde_json::json!({
                    "ok": true,
                    "data": issue,
                    "message": format!("Claimed {}.", display),
                });
                if !file_conflicts.is_empty() {
                    envelope["file_conflicts"] =
                        serde_json::to_value(&file_conflicts).unwrap_or_default();
                }
                println!("{}", serde_json::to_string_pretty(&envelope)?);
            } else {
                println!("Claimed {}.", display);
                for conflict in &file_conflicts {
                    for ci in &conflict.conflicts_with {
                        println!(
                            "Warning: file conflict on {} with BMO-{} ({})",
                            conflict.file, ci.id, ci.title
                        );
                    }
                }
            }
            Ok(())
        }
        Err(e) => {
            if let Some(BmoError::Conflict(_)) = e.downcast_ref::<BmoError>() {
                let msg = format!("BMO-{} is already in-progress", issue_id);
                if json {
                    let out = serde_json::json!({
                        "ok": false,
                        "code": ErrorCode::Conflict.as_str(),
                        "error": msg,
                    });
                    println!("{}", serde_json::to_string_pretty(&out)?);
                } else {
                    eprintln!("error: {}", msg);
                }
                std::process::exit(ErrorCode::Conflict.exit_code());
            }

            if let Some(BmoError::NotFound(_)) = e.downcast_ref::<BmoError>() {
                let msg = e.to_string();
                if json {
                    let out = serde_json::json!({
                        "ok": false,
                        "code": ErrorCode::NotFound.as_str(),
                        "error": msg,
                    });
                    println!("{}", serde_json::to_string_pretty(&out)?);
                } else {
                    eprintln!("error: {}", msg);
                }
                std::process::exit(ErrorCode::NotFound.exit_code());
            }

            Err(e)
        }
    }
}
