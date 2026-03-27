use clap::{Args, Subcommand};

use crate::cli::parse_id;
use crate::config::find_bmo_dir;
use crate::db::{Repository, open_db};
use crate::errors::{BmoError, ErrorCode};
use crate::model::RelationKind;
use crate::output::{OutputMode, make_printer};

#[derive(Subcommand)]
pub enum LinkCommands {
    /// Add a relation between issues
    Add(AddArgs),
    /// Remove a relation by ID
    Remove(RemoveArgs),
    /// List relations for an issue
    List(ListArgs),
}

#[derive(Args)]
pub struct AddArgs {
    /// Source issue ID
    pub from_id: String,
    /// Relation type (blocks, blocked-by, depends-on, dependency-of, relates-to, duplicates, duplicate-of)
    pub relation: String,
    /// Target issue ID
    pub to_id: String,
}

#[derive(Args)]
pub struct RemoveArgs {
    /// Relation ID
    pub id: i64,
}

#[derive(Args)]
pub struct ListArgs {
    /// Issue ID
    pub id: String,
}

pub fn run_add(args: &AddArgs, json: bool) -> anyhow::Result<()> {
    let bmo_dir = find_bmo_dir()?;
    let repo = open_db(&bmo_dir.join("issues.db"))?;

    let from_id = parse_id(&args.from_id)?;
    let to_id = parse_id(&args.to_id)?;
    let kind: RelationKind = args.relation.parse()?;

    let relation = match repo.add_relation(from_id, kind, to_id) {
        Ok(r) => r,
        Err(e) => {
            if let Some(BmoError::Validation(msg)) = e.downcast_ref::<BmoError>() {
                let printer = make_printer(if json {
                    OutputMode::Json
                } else {
                    OutputMode::Human
                });
                printer.print_error(msg, ErrorCode::Validation);
                std::process::exit(ErrorCode::Validation.exit_code());
            }
            return Err(e);
        }
    };

    if json {
        let envelope = serde_json::json!({ "ok": true, "data": relation, "message": format!("Linked BMO-{from_id} {kind} BMO-{to_id}") });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else {
        println!("Linked BMO-{from_id} {kind} BMO-{to_id}");
    }
    Ok(())
}

pub fn run_remove(args: &RemoveArgs, json: bool) -> anyhow::Result<()> {
    let bmo_dir = find_bmo_dir()?;
    let repo = open_db(&bmo_dir.join("issues.db"))?;

    repo.remove_relation(args.id)?;

    if json {
        let envelope = serde_json::json!({ "ok": true, "data": null, "message": format!("Removed relation {}", args.id) });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else {
        println!("Removed relation {}", args.id);
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
    let relations = repo.list_relations(issue_id)?;
    printer.print_relations(&relations);
    Ok(())
}
