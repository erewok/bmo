use clap::Args;

use crate::cli::parse_id;
use crate::config::find_bmo_dir;
use crate::db::{Repository, open_db};
use crate::model::RelationKind;

#[derive(Args)]
pub struct GraphArgs {
    /// Issue ID
    pub id: String,
}

pub fn run(args: &GraphArgs, json: bool) -> anyhow::Result<()> {
    let bmo_dir = find_bmo_dir()?;
    let repo = open_db(&bmo_dir.join("issues.db"))?;

    let issue_id = parse_id(&args.id)?;

    // Get the root issue
    let issue = repo
        .get_issue(issue_id)?
        .ok_or_else(|| anyhow::anyhow!("issue {} not found", args.id))?;

    let relations = repo.list_relations(issue_id)?;

    if json {
        let envelope = serde_json::json!({
            "ok": true,
            "data": { "issue": issue, "relations": relations },
            "message": format!("Graph for {}", issue.display_id())
        });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
        return Ok(());
    }

    // ASCII tree output
    println!("{} — {}", issue.display_id(), issue.title);

    let blockers: Vec<_> = relations
        .iter()
        .filter(|r| {
            (r.kind == RelationKind::BlockedBy && r.from_id == issue_id)
                || (r.kind == RelationKind::Blocks && r.to_id == issue_id)
        })
        .collect();

    let blocking: Vec<_> = relations
        .iter()
        .filter(|r| {
            (r.kind == RelationKind::Blocks && r.from_id == issue_id)
                || (r.kind == RelationKind::BlockedBy && r.to_id == issue_id)
        })
        .collect();

    let no_relations = blockers.is_empty() && blocking.is_empty();

    if !blockers.is_empty() {
        println!("  ← blocked by:");
        for r in &blockers {
            let other_id = if r.from_id == issue_id {
                r.to_id
            } else {
                r.from_id
            };
            if let Ok(Some(other)) = repo.get_issue(other_id) {
                println!("      BMO-{} — {}", other_id, other.title);
            }
        }
    }

    if !blocking.is_empty() {
        println!("  → blocks:");
        for r in &blocking {
            let other_id = if r.from_id == issue_id {
                r.to_id
            } else {
                r.from_id
            };
            if let Ok(Some(other)) = repo.get_issue(other_id) {
                println!("      BMO-{} — {}", other_id, other.title);
            }
        }
    }

    if no_relations {
        println!("  (no blocking relations)");
    }

    Ok(())
}
