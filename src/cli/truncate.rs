use std::str::FromStr;

use clap::Args;

use crate::config::find_bmo_dir;
use crate::db::{Repository, open_db};
use crate::model::{IssueFilter, Status};

#[derive(Args)]
pub struct TruncateArgs {
    /// Delete issues with these statuses (repeatable; default: done)
    #[arg(short, long, value_name = "STATUS")]
    pub status: Vec<String>,

    /// Delete ALL issues regardless of status (mutually exclusive with --status)
    #[arg(long, conflicts_with = "status")]
    pub all: bool,

    /// Skip confirmation prompt (for non-interactive use)
    #[arg(long)]
    pub yes: bool,
}

impl TruncateArgs {
    pub fn get_statuses(&self) -> Vec<Status> {
        if self.all {
            Status::all().to_vec()
        } else if !self.status.is_empty() {
            self.status
                .iter()
                .filter_map(|s| {
                    Status::from_str(s)
                        .map_err(|e| anyhow::anyhow!("invalid status {:?}: {}", s, e))
                        .ok()
                })
                .collect()
        } else {
            vec![Status::Done]
        }
    }
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
        args.get_statuses()
    } else {
        vec![Status::Done]
    };

    // Count matching issues before deletion.
    // NOTE: include_done must be false when a status filter is provided — the
    // query builder only applies the status IN (...) clause when include_done
    // is false. For --all we skip the status filter entirely and just count
    // every row.
    let count = if args.all {
        repo.count_issues(&IssueFilter {
            include_done: true,
            ..Default::default()
        })?
    } else {
        repo.count_issues(&IssueFilter {
            status: Some(statuses.clone()),
            include_done: false,
            ..Default::default()
        })?
    };

    if count == 0 {
        if json {
            let envelope = serde_json::json!({
                "ok": true,
                "data": { "deleted": 0 },
                "message": "Nothing to delete.",
            });
            println!("{}", serde_json::to_string_pretty(&envelope)?);
        } else {
            println!("Nothing to delete.");
        }
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
    let deleted = if args.all {
        repo.truncate_all_issues()?
    } else {
        repo.truncate_issues(&statuses)?
    };

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
