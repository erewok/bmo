use clap::Args;

use crate::config::find_bmo_dir;
use crate::db::{Repository, open_db};
use crate::model::{IssueFilter, Status};

#[derive(Args)]
pub struct TruncateArgs {
    /// Delete issues with these statuses (repeatable; default: done)
    #[arg(short, long, value_name = "STATUS")]
    pub status: Vec<Status>,

    /// Delete ALL issues regardless of status (mutually exclusive with --status)
    #[arg(long, conflicts_with = "status")]
    pub all: bool,

    /// Skip confirmation prompt (for non-interactive use)
    #[arg(long)]
    pub yes: bool,
}

pub fn run(args: &TruncateArgs, json: bool, db: Option<String>) -> anyhow::Result<()> {
    let db_path = if let Some(path) = db {
        std::path::PathBuf::from(path)
    } else {
        find_bmo_dir()?.join("issues.db")
    };
    let repo = open_db(&db_path)?;

    // Resolve the effective set of statuses to delete.
    let statuses: Vec<Status> = if args.all {
        Status::all().to_vec()
    } else if !args.status.is_empty() {
        args.status.clone()
    } else {
        vec![Status::Done]
    };

    // Count matching issues before deletion.
    let count = if args.all {
        // Count all issues regardless of status.
        repo.count_issues(&IssueFilter {
            include_done: true,
            ..Default::default()
        })?
    } else {
        repo.count_issues(&IssueFilter {
            status: Some(statuses.clone()),
            include_done: true,
            ..Default::default()
        })?
    };

    if count == 0 {
        println!("Nothing to delete.");
        return Ok(());
    }

    // Confirmation prompt (unless --yes).
    if !args.yes {
        if args.all {
            eprint!(
                "Delete ALL {} issue(s) permanently? This cannot be undone. [y/N] ",
                count
            );
        } else if statuses == [Status::Done] {
            eprint!("Delete {} done issue(s) permanently? [y/N] ", count);
        } else {
            let labels: Vec<&str> = statuses.iter().map(|s| s.label()).collect();
            eprint!(
                "Delete {} issue(s) with status [{}]? [y/N] ",
                count,
                labels.join(", ")
            );
        }
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            eprintln!("Aborted.");
            return Ok(());
        }
    }

    // Perform deletion in a single atomic statement.
    let deleted = repo.truncate_issues(&statuses)?;

    let msg = format!("Deleted {} issue(s).", deleted);
    if json {
        let envelope = serde_json::json!({
            "ok": true,
            "data": { "deleted": deleted },
            "message": msg,
        });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else {
        println!("{}", msg);
    }

    Ok(())
}
