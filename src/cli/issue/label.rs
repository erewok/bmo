use clap::{Args, Subcommand};

use crate::cli::parse_id;
use crate::config::find_bmo_dir;
use crate::db::{Repository, open_db};
use crate::output::{OutputMode, make_printer};

#[derive(Subcommand)]
pub enum LabelCommands {
    /// Add a label to an issue
    Add(AddArgs),
    /// Remove a label from an issue
    #[command(alias = "remove")]
    Rm(RmArgs),
    /// List labels on an issue
    List(ListArgs),
    /// Delete a label entirely
    Delete(DeleteArgs),
}

#[derive(Args)]
pub struct AddArgs {
    /// Issue ID
    pub id: String,
    /// Label name
    pub name: String,
    /// Label color (hex)
    #[arg(long)]
    pub color: Option<String>,
}

#[derive(Args)]
pub struct RmArgs {
    /// Issue ID
    pub id: String,
    /// Label name
    pub name: String,
}

#[derive(Args)]
pub struct ListArgs {
    /// Issue ID
    pub id: String,
}

#[derive(Args)]
pub struct DeleteArgs {
    /// Label name
    pub name: String,
}

pub fn run_add(args: &AddArgs, json: bool) -> anyhow::Result<()> {
    let bmo_dir = find_bmo_dir()?;
    let repo = open_db(&bmo_dir.join("issues.db"))?;

    let issue_id = parse_id(&args.id)?;
    let label = repo.get_or_create_label(&args.name, args.color.as_deref())?;
    repo.add_label_to_issue(issue_id, label.id)?;

    if json {
        let envelope = serde_json::json!({ "ok": true, "data": label, "message": format!("Label '{}' added", args.name) });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else {
        println!("Added label '{}' to {}", args.name, args.id);
    }
    Ok(())
}

pub fn run_rm(args: &RmArgs, json: bool) -> anyhow::Result<()> {
    let bmo_dir = find_bmo_dir()?;
    let repo = open_db(&bmo_dir.join("issues.db"))?;

    let issue_id = parse_id(&args.id)?;
    repo.remove_label_from_issue(issue_id, &args.name)?;

    if json {
        let envelope = serde_json::json!({ "ok": true, "data": null, "message": format!("Label '{}' removed", args.name) });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else {
        println!("Removed label '{}' from {}", args.name, args.id);
    }
    Ok(())
}

pub fn run_list(args: &ListArgs, json: bool) -> anyhow::Result<()> {
    let bmo_dir = find_bmo_dir()?;
    let repo = open_db(&bmo_dir.join("issues.db"))?;
    let printer = make_printer(if json {
        OutputMode::Json
    } else {
        OutputMode::Human
    });

    let issue_id = parse_id(&args.id)?;
    let labels = repo.list_issue_labels(issue_id)?;
    printer.print_labels(&labels);
    Ok(())
}

pub fn run_delete(args: &DeleteArgs, json: bool) -> anyhow::Result<()> {
    let bmo_dir = find_bmo_dir()?;
    let repo = open_db(&bmo_dir.join("issues.db"))?;

    repo.delete_label(&args.name)?;

    if json {
        let envelope = serde_json::json!({ "ok": true, "data": null, "message": format!("Deleted label '{}'", args.name) });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else {
        println!("Deleted label '{}'", args.name);
    }
    Ok(())
}
