use clap::Args;

use crate::cli::parse_id;
use crate::db::{Repository, find_db, open_db};

#[derive(Args)]
pub struct DeleteArgs {
    /// Issue ID
    pub id: String,

    // Todo: support multiple IDs in one command
    // /// List of IDs to delete (alternative to --id)
    // #[arg(long)]
    // pub ids: Vec<String>,
    /// Skip confirmation prompt
    #[arg(long)]
    pub yes: bool,
}

pub fn run(args: &DeleteArgs, json: bool, db: Option<String>) -> anyhow::Result<()> {
    let db_path = find_db(db.as_deref())?;
    let repo = open_db(&db_path)?;

    let id = parse_id(&args.id)?;

    // Confirm unless --yes
    if !args.yes {
        eprint!("Delete {} permanently? [y/N] ", args.id);
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    repo.delete_issue(id)?;

    if json {
        let envelope = serde_json::json!({ "ok": true, "data": null, "message": format!("Deleted {}", args.id) });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else {
        println!("Deleted {}", args.id);
    }
    Ok(())
}
