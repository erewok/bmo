use clap::Args;

use crate::config::find_bmo_dir;
use crate::db::{Repository, open_db};
use crate::model::export::ExportBundle;

#[derive(Args)]
pub struct ExportArgs {
    /// Output file path (default: stdout)
    #[arg(short, long)]
    pub output: Option<String>,
}

pub fn run(args: &ExportArgs, _json: bool) -> anyhow::Result<()> {
    let bmo_dir = find_bmo_dir()?;
    let repo = open_db(&bmo_dir.join("issues.db"))?;

    let all_issues = repo.list_issues(&mut crate::model::IssueFilter::all())?;
    let all_relations = repo.list_all_relations()?;
    let all_labels = repo.list_all_labels()?;
    let project_name = repo
        .get_meta("project_name")?
        .unwrap_or_else(|| "bmo".to_string());

    // Collect all comments and files for all issues
    let mut all_comments = Vec::new();
    let mut all_activity = Vec::new();
    let mut all_files = Vec::new();

    for issue in &all_issues {
        all_comments.extend(repo.list_comments(issue.id)?);
        all_activity.extend(repo.list_activity(issue.id, usize::MAX)?);
        all_files.extend(repo.list_files(issue.id)?);
    }

    let bundle = ExportBundle {
        schema_version: 1,
        exported_at: chrono::Utc::now(),
        project_name,
        issues: all_issues,
        comments: all_comments,
        labels: all_labels,
        relations: all_relations,
        activity: all_activity,
        files: all_files,
    };

    let json = serde_json::to_string_pretty(&bundle)?;

    match &args.output {
        Some(path) => std::fs::write(path, &json)?,
        None => println!("{json}"),
    }

    Ok(())
}
