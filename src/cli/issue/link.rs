use clap::{Args, Subcommand};

use crate::db::{Repository, find_db, open_db};
use crate::errors::{BmoError, ErrorCode};
use crate::model::{IssueId, RelationKind};
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
    pub from_id: IssueId,
    /// Relation type (blocks, blocked-by, depends-on, dependency-of, relates-to, duplicates, duplicate-of)
    pub relation: String,
    /// Target issue ID
    pub to_id: IssueId,
}

#[derive(Args)]
pub struct RemoveArgs {
    /// Relation ID
    pub id: i64,
}

#[derive(Args)]
pub struct ListArgs {
    /// Issue ID
    pub id: IssueId,
}

pub fn run_add(args: &AddArgs, json: bool, db: Option<String>) -> anyhow::Result<()> {
    let db_path = find_db(db.as_deref())?;
    let repo = open_db(&db_path)?;

    let from_id = args.from_id;
    let to_id = args.to_id;
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
        let envelope = serde_json::json!({ "ok": true, "data": relation, "message": format!("Linked {from_id} {kind} {to_id}") });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else {
        println!("Linked {from_id} {kind} {to_id}");
    }
    Ok(())
}

pub fn run_remove(args: &RemoveArgs, json: bool, db: Option<String>) -> anyhow::Result<()> {
    let db_path = find_db(db.as_deref())?;
    let repo = open_db(&db_path)?;

    repo.remove_relation(args.id)?;

    if json {
        let envelope = serde_json::json!({ "ok": true, "data": null, "message": format!("Removed relation {}", args.id) });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else {
        println!("Removed relation {}", args.id);
    }
    Ok(())
}

pub fn run_list(args: &ListArgs, json: bool, db: Option<String>) -> anyhow::Result<()> {
    let db_path = find_db(db.as_deref())?;
    let repo = open_db(&db_path)?;
    let printer = make_printer(if json {
        OutputMode::Json
    } else {
        OutputMode::Human
    });

    let issue_id = args.id;
    let relations = repo.list_relations(issue_id)?;
    printer.print_relations(&relations);
    Ok(())
}
