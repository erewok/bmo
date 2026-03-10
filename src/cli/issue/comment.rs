use clap::{Args, Subcommand};

use crate::cli::parse_id;
use crate::config::find_bmo_dir;
use crate::db::{AddCommentInput, Repository, open_db};
use crate::output::{OutputMode, make_printer};

#[derive(Subcommand)]
pub enum CommentCommands {
    /// Add a comment to an issue
    Add(AddArgs),
    /// List comments on an issue
    List(ListArgs),
}

#[derive(Args)]
pub struct AddArgs {
    /// Issue ID
    pub id: String,
    /// Comment body
    #[arg(short, long)]
    pub body: String,
    /// Comment author
    #[arg(short, long)]
    pub author: Option<String>,
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
    let comment = repo.add_comment(&AddCommentInput {
        issue_id,
        body: args.body.clone(),
        author: args.author.clone(),
    })?;

    if json {
        let envelope =
            serde_json::json!({ "ok": true, "data": comment, "message": "Comment added" });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else {
        println!("Comment added to {}", args.id);
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
    let comments = repo.list_comments(issue_id)?;
    printer.print_comments(&comments);
    Ok(())
}
