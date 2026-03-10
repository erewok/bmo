use clap::{Args, Subcommand};

use crate::cli::parse_id;
use crate::config::find_bmo_dir;
use crate::db::{Repository, open_db};

#[derive(Subcommand)]
pub enum FileCommands {
    /// Attach a file to an issue
    Add(AddArgs),
    /// Remove a file attachment
    #[command(alias = "remove")]
    Rm(RmArgs),
    /// List file attachments
    List(ListArgs),
}

#[derive(Args)]
pub struct AddArgs {
    /// Issue ID
    pub id: String,
    /// File path
    pub path: String,
}

#[derive(Args)]
pub struct RmArgs {
    /// Issue ID
    pub id: String,
    /// File path
    pub path: String,
}

#[derive(Args)]
pub struct ListArgs {
    /// Issue ID
    pub id: String,
}

pub fn run_add(args: &AddArgs, json: bool) -> anyhow::Result<()> {
    let bmo_dir = find_bmo_dir()?;
    let repo = open_db(&bmo_dir.join("issues.db"))?;

    let issue_id = parse_id(&args.id)?;
    let file = repo.add_file(issue_id, &args.path)?;

    if json {
        let envelope = serde_json::json!({ "ok": true, "data": file, "message": format!("Attached {}", args.path) });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else {
        println!("Attached '{}' to {}", args.path, args.id);
    }
    Ok(())
}

pub fn run_rm(args: &RmArgs, json: bool) -> anyhow::Result<()> {
    let bmo_dir = find_bmo_dir()?;
    let repo = open_db(&bmo_dir.join("issues.db"))?;

    let issue_id = parse_id(&args.id)?;
    repo.remove_file(issue_id, &args.path)?;

    if json {
        let envelope = serde_json::json!({ "ok": true, "data": null, "message": format!("Removed {}", args.path) });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else {
        println!("Removed '{}' from {}", args.path, args.id);
    }
    Ok(())
}

pub fn run_list(args: &ListArgs, json: bool) -> anyhow::Result<()> {
    let bmo_dir = find_bmo_dir()?;
    let repo = open_db(&bmo_dir.join("issues.db"))?;

    let issue_id = parse_id(&args.id)?;
    let files = repo.list_files(issue_id)?;

    if json {
        let envelope = serde_json::json!({ "ok": true, "data": files, "message": format!("{} file(s)", files.len()) });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else if files.is_empty() {
        println!("No files attached.");
    } else {
        for f in &files {
            println!("{}", f.path);
        }
    }
    Ok(())
}
