use std::collections::HashSet;

use clap::Args;

use crate::cli::parse_id;
use crate::db::{Repository, find_db, open_db};
use crate::model::{Relation, RelationKind};

#[derive(Args)]
pub struct GraphArgs {
    /// Issue ID
    pub id: String,
}

pub fn run(args: &GraphArgs, json: bool, db: Option<String>) -> anyhow::Result<()> {
    let db_path = find_db(db.as_deref())?;
    let repo = open_db(&db_path)?;

    let issue_id = parse_id(&args.id)?;

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

    println!("{} — {}", issue.display_id(), issue.title);

    let blockers = other_ids_with_role(&relations, issue_id, BlockingRole::BlockedByOther);
    let blocking = other_ids_with_role(&relations, issue_id, BlockingRole::BlocksOther);

    print_group(&repo, "← blocked by:", &blockers);
    print_group(&repo, "→ blocks:", &blocking);

    if blockers.is_empty() && blocking.is_empty() {
        println!("  (no blocking relations)");
    }

    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BlockingRole {
    BlockedByOther,
    BlocksOther,
}

impl BlockingRole {
    /// The role an issue plays when it has `kind` towards another issue.
    fn of_kind(kind: RelationKind) -> Option<Self> {
        match kind {
            RelationKind::BlockedBy | RelationKind::DependsOn => Some(Self::BlockedByOther),
            RelationKind::Blocks | RelationKind::DependencyOf => Some(Self::BlocksOther),
            RelationKind::RelatesTo | RelationKind::Duplicates | RelationKind::DuplicateOf => None,
        }
    }
}

/// Ids of the other issues `relations` names with `role` towards `issue_id`,
/// in stored order with duplicates (e.g. an edge declared from both ends) removed.
/// An issue with a directional relation to itself plays both roles.
fn other_ids_with_role(relations: &[Relation], issue_id: i64, role: BlockingRole) -> Vec<i64> {
    let mut other_ids = Vec::new();
    let mut seen_ids = HashSet::new();
    for relation in relations {
        let view = relation.viewed_from(issue_id);
        let Some(role_of_view) = BlockingRole::of_kind(view.kind) else {
            continue;
        };
        let is_self_relation = relation.from_id == relation.to_id;
        if (is_self_relation || role_of_view == role) && seen_ids.insert(view.other_id) {
            other_ids.push(view.other_id);
        }
    }
    other_ids
}

fn print_group(repo: &impl Repository, heading: &str, other_ids: &[i64]) {
    if other_ids.is_empty() {
        return;
    }
    println!("  {heading}");
    for &other_id in other_ids {
        if let Ok(Some(other)) = repo.get_issue(other_id) {
            println!("      BMO-{} — {}", other_id, other.title);
        }
    }
}
