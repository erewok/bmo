use clap::Args;

use crate::cli::parse_id;
use crate::config::find_bmo_dir;
use crate::db::{Repository, open_db};

#[derive(Args)]
pub struct LogArgs {
    /// Issue ID
    pub id: String,
    /// Number of entries to show
    #[arg(long, default_value = "10")]
    pub limit: usize,
}

pub fn run(args: &LogArgs, json: bool) -> anyhow::Result<()> {
    let bmo_dir = find_bmo_dir()?;
    let repo = open_db(&bmo_dir.join("issues.db"))?;

    let issue_id = parse_id(&args.id)?;
    let entries = repo.list_activity(issue_id, args.limit)?;

    if json {
        let envelope = serde_json::json!({ "ok": true, "data": entries, "message": format!("{} entries", entries.len()) });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else if entries.is_empty() {
        println!("No activity.");
    } else {
        for e in &entries {
            let actor = e.actor.as_deref().unwrap_or("unknown");
            let detail = e.detail.as_deref().unwrap_or("");
            println!(
                "[{}] {} — {} {}",
                e.created_at.format("%Y-%m-%d %H:%M"),
                actor,
                e.kind,
                detail
            );
        }
    }
    Ok(())
}
