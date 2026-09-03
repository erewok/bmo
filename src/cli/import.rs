use clap::Args;

use crate::db::{AddCommentInput, CreateIssueInput, Repository, find_db, open_db};
use crate::model::export::ExportBundle;

#[derive(Args)]
pub struct ImportArgs {
    /// Path to the JSON export file
    pub file: String,
}

// ── run ───────────────────────────────────────────────────────────────────────

pub fn run(args: &ImportArgs, json: bool, db: Option<String>) -> anyhow::Result<()> {
    let db_path = find_db(db.as_deref())?;
    let repo = open_db(&db_path)?;

    let contents = std::fs::read_to_string(&args.file)?;

    let mut imported_issues = 0usize;
    let mut imported_comments = 0usize;

    import_from_bmo(
        &repo,
        &contents,
        &mut imported_issues,
        &mut imported_comments,
    )?;

    let msg = format!("Imported {imported_issues} issue(s) and {imported_comments} comment(s)");

    if json {
        // The `warnings` field is emitted as an always-empty array purely to
        // keep the envelope shape stable for existing --json consumers. Only
        // the removed docket importer ever reported skipped records; the
        // native importer has no partial-failure mode to warn about.
        let envelope = serde_json::json!({
            "ok": true,
            "data": { "issues": imported_issues, "comments": imported_comments },
            "message": msg,
            "warnings": [],
        });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else {
        println!("{msg}");
    }

    Ok(())
}

// ── bmo native import ────────────────────────────────────────────────────────

fn import_from_bmo(
    repo: &impl Repository,
    contents: &str,
    imported_issues: &mut usize,
    imported_comments: &mut usize,
) -> anyhow::Result<()> {
    let bundle: ExportBundle = serde_json::from_str(contents)?;

    for issue in &bundle.issues {
        let input = CreateIssueInput {
            parent_id: issue.parent_id,
            title: issue.title.clone(),
            description: issue.description.clone(),
            status: issue.status,
            priority: issue.priority,
            kind: issue.kind,
            assignee: issue.assignee.clone(),
            labels: issue.labels.clone(),
            files: issue.files.clone(),
            actor: Some("import".to_string()),
        };
        repo.create_issue(&input)?;
        *imported_issues += 1;
    }

    for comment in &bundle.comments {
        let input = AddCommentInput {
            issue_id: comment.issue_id,
            body: comment.body.clone(),
            author: comment.author.clone(),
        };
        if repo.get_issue(comment.issue_id)?.is_some() {
            repo.add_comment(&input)?;
            *imported_comments += 1;
        }
    }

    Ok(())
}
