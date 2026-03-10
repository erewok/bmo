use clap::Args;

use crate::cli::parse_id;
use crate::config::find_bmo_dir;
use crate::db::{CreateIssueInput, Repository, open_db};
use crate::model::{Kind, Priority, Status};
use crate::output::{OutputMode, make_printer};

#[derive(Args)]
pub struct CreateArgs {
    /// Issue title
    #[arg(short, long)]
    pub title: String,
    /// Issue description
    #[arg(short, long, default_value = "")]
    pub description: String,
    /// Status
    #[arg(short, long, default_value = "backlog")]
    pub status: String,
    /// Priority
    #[arg(short, long, default_value = "medium")]
    pub priority: String,
    /// Issue kind/type
    #[arg(short = 'T', long = "kind", default_value = "task")]
    pub kind: String,
    /// Assignee
    #[arg(short, long)]
    pub assignee: Option<String>,
    /// Parent issue ID
    #[arg(long)]
    pub parent: Option<String>,
    /// Labels (repeatable)
    #[arg(short, long)]
    pub label: Vec<String>,
    /// File attachments (repeatable)
    #[arg(short, long)]
    pub file: Vec<String>,
}

pub fn run(args: &CreateArgs, json: bool) -> anyhow::Result<()> {
    let bmo_dir = find_bmo_dir()?;
    let repo = open_db(&bmo_dir.join("issues.db"))?;
    let printer = make_printer(if json {
        OutputMode::Json
    } else {
        OutputMode::Human
    });

    let status: Status = args.status.parse()?;
    let priority: Priority = args.priority.parse()?;
    let kind: Kind = args.kind.parse()?;
    let parent_id = args.parent.as_deref().map(parse_id).transpose()?;

    let input = CreateIssueInput {
        parent_id,
        title: args.title.clone(),
        description: args.description.clone(),
        status,
        priority,
        kind,
        assignee: args.assignee.clone(),
        labels: args.label.clone(),
        files: args.file.clone(),
        actor: None,
    };

    let issue = repo.create_issue(&input)?;

    if json {
        printer.print_issue(&issue);
    } else {
        println!("Created {}: {}", issue.display_id(), issue.title);
    }
    Ok(())
}
