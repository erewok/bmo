use std::collections::{HashMap, HashSet};

use crate::model::{Issue, Relation, RelationKind, Status};

/// A node in the directed acyclic graph.
#[derive(Debug)]
pub struct DagNode {
    pub issue: Issue,
    /// IDs of issues that this issue blocks (i.e., this → those)
    pub forward: HashSet<i64>,
    /// IDs of issues that block this issue (i.e., those → this)
    pub reverse: HashSet<i64>,
}

/// The full dependency DAG.
#[derive(Debug)]
pub struct Dag {
    pub nodes: HashMap<i64, DagNode>,
}

impl Dag {
    /// Build the DAG from a list of issues and their relations.
    /// Only `blocks` and `depends_on` edges are used; others are ignored.
    pub fn build(issues: &[Issue], relations: &[Relation]) -> Self {
        let mut nodes: HashMap<i64, DagNode> = issues
            .iter()
            .map(|i| {
                (
                    i.id,
                    DagNode {
                        issue: i.clone(),
                        forward: HashSet::new(),
                        reverse: HashSet::new(),
                    },
                )
            })
            .collect();

        for rel in relations {
            match rel.kind {
                RelationKind::Blocks => {
                    // from blocks to: from → to
                    if let Some(node) = nodes.get_mut(&rel.from_id) {
                        node.forward.insert(rel.to_id);
                    }
                    if let Some(node) = nodes.get_mut(&rel.to_id) {
                        node.reverse.insert(rel.from_id);
                    }
                }
                RelationKind::DependsOn => {
                    // from depends_on to: to blocks from → to → from
                    if let Some(node) = nodes.get_mut(&rel.to_id) {
                        node.forward.insert(rel.from_id);
                    }
                    if let Some(node) = nodes.get_mut(&rel.from_id) {
                        node.reverse.insert(rel.to_id);
                    }
                }
                _ => {} // relates_to, duplicates, etc. ignored
            }
        }

        Dag { nodes }
    }

    /// True if the issue has no children in the parent-child hierarchy.
    /// We approximate this by checking if any other issue has this as parent_id.
    pub fn is_leaf(&self, id: i64) -> bool {
        // Check if any issue in the DAG has this issue as parent
        !self.nodes.values().any(|n| n.issue.parent_id == Some(id))
    }
}

/// Find all work-ready issues: those in backlog/todo status that are leaf
/// nodes (no children) and have all blockers completed.
pub fn find_ready(dag: &Dag) -> Vec<&Issue> {
    let allowed_statuses = [Status::Backlog, Status::Todo];
    let done = Status::Done;

    let mut ready: Vec<&Issue> = dag
        .nodes
        .values()
        .filter(|node| {
            // Must be in an actionable status
            if !allowed_statuses.contains(&node.issue.status) {
                return false;
            }
            // Must be a leaf (no children)
            if !dag.is_leaf(node.issue.id) {
                return false;
            }
            // All blockers must be done
            node.reverse.iter().all(|blocker_id| {
                dag.nodes
                    .get(blocker_id)
                    .map(|n| n.issue.status == done)
                    .unwrap_or(true) // If not in DAG, assume done
            })
        })
        .map(|n| &n.issue)
        .collect();

    // Sort by priority (highest first), then by id (oldest first)
    ready.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));

    ready
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Kind, Priority, Status};
    use chrono::Utc;

    fn make_issue(id: i64, status: Status, priority: Priority) -> Issue {
        Issue {
            id,
            parent_id: None,
            title: format!("Issue {id}"),
            description: String::new(),
            status,
            priority,
            kind: Kind::Task,
            assignee: None,
            labels: vec![],
            files: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn make_relation(from_id: i64, to_id: i64, kind: RelationKind) -> Relation {
        Relation {
            id: 0,
            from_id,
            to_id,
            kind,
        }
    }

    #[test]
    fn dag_construction_blocks() {
        let issues = vec![
            make_issue(1, Status::Todo, Priority::High),
            make_issue(2, Status::Backlog, Priority::Medium),
        ];
        let relations = vec![make_relation(1, 2, RelationKind::Blocks)];
        let dag = Dag::build(&issues, &relations);

        assert!(dag.nodes[&1].forward.contains(&2));
        assert!(dag.nodes[&2].reverse.contains(&1));
    }

    #[test]
    fn find_ready_unblocked() {
        let issues = vec![
            make_issue(1, Status::Done, Priority::High),
            make_issue(2, Status::Todo, Priority::Medium),
        ];
        let relations = vec![make_relation(1, 2, RelationKind::Blocks)];
        let dag = Dag::build(&issues, &relations);
        let ready = find_ready(&dag);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, 2);
    }

    #[test]
    fn find_ready_blocked() {
        let issues = vec![
            make_issue(1, Status::InProgress, Priority::High),
            make_issue(2, Status::Todo, Priority::Medium),
        ];
        let relations = vec![make_relation(1, 2, RelationKind::Blocks)];
        let dag = Dag::build(&issues, &relations);
        let ready = find_ready(&dag);
        // Issue 2 is blocked by issue 1 which is not done
        assert!(ready.iter().all(|i| i.id != 2));
    }
}
