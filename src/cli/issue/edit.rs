use clap::Args;

use crate::cli::parse_id;
use crate::config::find_bmo_dir;
use crate::db::{Repository, UpdateIssueInput, open_db};
use crate::model::{Kind, Priority, Status};
use crate::output::{OutputMode, make_printer};

#[derive(Args)]
pub struct EditArgs {
    /// Issue ID
    pub id: String,
    #[arg(short, long)]
    pub title: Option<String>,
    #[arg(short, long)]
    pub description: Option<String>,
    #[arg(short, long)]
    pub status: Option<String>,
    #[arg(short, long)]
    pub priority: Option<String>,
    #[arg(short = 'T', long = "kind")]
    pub kind: Option<String>,
    #[arg(short, long)]
    pub assignee: Option<String>,
    #[arg(long)]
    pub parent: Option<String>,
}

pub fn run(args: &EditArgs, json: bool) -> anyhow::Result<()> {
    let bmo_dir = find_bmo_dir()?;
    let repo = open_db(&bmo_dir.join("issues.db"))?;
    let printer = make_printer(if json {
        OutputMode::Json
    } else {
        OutputMode::Human
    });

    let id = parse_id(&args.id)?;

    let status = args
        .status
        .as_deref()
        .map(|s| s.parse::<Status>())
        .transpose()?;
    let priority = args
        .priority
        .as_deref()
        .map(|p| p.parse::<Priority>())
        .transpose()?;
    let kind = args
        .kind
        .as_deref()
        .map(|k| k.parse::<Kind>())
        .transpose()?;
    let parent_id = args.parent.as_deref().map(parse_id).transpose()?.map(Some);

    let input = UpdateIssueInput {
        title: args.title.clone(),
        description: args.description.clone(),
        status,
        priority,
        kind,
        assignee: args.assignee.clone(),
        parent_id,
        actor: None,
    };

    let issue = repo.update_issue(id, &input)?;

    if json {
        printer.print_issue(&issue);
    } else {
        println!("Updated {}", issue.display_id());
    }
    Ok(())
}
