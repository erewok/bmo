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
    ///
    /// All four directional relation kinds contribute an edge: `Blocks` and
    /// `DependencyOf` add a forward edge `from_id → to_id`; `DependsOn` and
    /// `BlockedBy` are their semantic inverses and add the mirrored edge
    /// `to_id → from_id`. This way the same real ordering constraint is
    /// recorded regardless of which of the two equivalent verbs was used to
    /// declare the link. `RelatesTo`, `Duplicates`, and `DuplicateOf` are
    /// informational only and contribute no edge.
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
            // Normalise all four directional kinds to one (blocker, blocked)
            // pair, so the same ordering constraint is recorded regardless of
            // which of the two equivalent verbs declared the link.
            let (blocker, blocked) = match rel.kind {
                // Blocks: from blocks to → from → to
                // DependencyOf: from is a dependency of to → from → to
                RelationKind::Blocks | RelationKind::DependencyOf => (rel.from_id, rel.to_id),
                // DependsOn: from depends_on to → to blocks from → to → from
                // BlockedBy: from is blocked_by to → to blocks from → to → from
                RelationKind::DependsOn | RelationKind::BlockedBy => (rel.to_id, rel.from_id),
                // relates_to, duplicates, duplicate_of are informational only
                _ => continue,
            };

            // Both endpoints must be nodes. Callers build the graph from a
            // status-filtered issue list, so an absent endpoint is a `done`
            // issue: a prerequisite already satisfied, imposing no ordering.
            // Inserting its id anyway would leave `forward`/`reverse` holding
            // ids with no entry in `nodes`, which corrupts the topological sort.
            if !nodes.contains_key(&blocker) || !nodes.contains_key(&blocked) {
                continue;
            }

            if let Some(node) = nodes.get_mut(&blocker) {
                node.forward.insert(blocked);
            }
            if let Some(node) = nodes.get_mut(&blocked) {
                node.reverse.insert(blocker);
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

    // A relation whose other endpoint was filtered out of the issue list (a
    // `done` issue) must not leave a dangling id in `forward`/`reverse`.
    // Regression: dangling ids crashed or deadlocked the topological sort.
    #[test]
    fn dag_drops_edges_with_a_missing_endpoint() {
        let issues = vec![make_issue(1, Status::Todo, Priority::High)];

        // Every directional kind, in both directions, with the other endpoint
        // absent from the node set. None may contribute an edge.
        let relations = vec![
            make_relation(1, 2, RelationKind::Blocks),
            make_relation(3, 1, RelationKind::Blocks),
            make_relation(1, 4, RelationKind::DependsOn),
            make_relation(5, 1, RelationKind::DependsOn),
            make_relation(1, 6, RelationKind::BlockedBy),
            make_relation(7, 1, RelationKind::BlockedBy),
            make_relation(1, 8, RelationKind::DependencyOf),
            make_relation(9, 1, RelationKind::DependencyOf),
        ];
        let dag = Dag::build(&issues, &relations);

        assert!(dag.nodes[&1].forward.is_empty());
        assert!(dag.nodes[&1].reverse.is_empty());
    }

    // Every id reachable through `forward` or `reverse` must be a key in `nodes`.
    #[test]
    fn dag_edge_ids_are_always_nodes() {
        let issues = vec![
            make_issue(1, Status::Todo, Priority::High),
            make_issue(2, Status::Todo, Priority::Medium),
        ];
        let relations = vec![
            make_relation(1, 2, RelationKind::Blocks),
            make_relation(2, 99, RelationKind::Blocks),
        ];
        let dag = Dag::build(&issues, &relations);

        for node in dag.nodes.values() {
            for id in node.forward.iter().chain(node.reverse.iter()) {
                assert!(dag.nodes.contains_key(id), "dangling edge id {id}");
            }
        }
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

    #[test]
    fn dag_construction_blocked_by_mirrors_blocks() {
        // Issue 2 declares "blocked-by" issue 1 — the natural inverse verb of
        // `bmo link add 1 blocks 2`. Must produce the identical DAG edge.
        let issues = vec![
            make_issue(1, Status::Todo, Priority::High),
            make_issue(2, Status::Backlog, Priority::Medium),
        ];
        let relations = vec![make_relation(2, 1, RelationKind::BlockedBy)];
        let dag = Dag::build(&issues, &relations);

        assert!(dag.nodes[&1].forward.contains(&2));
        assert!(dag.nodes[&2].reverse.contains(&1));
    }

    #[test]
    fn dag_construction_dependency_of_mirrors_depends_on() {
        // Issue 2 "depends-on" issue 1 is equivalent to issue 1 being
        // "dependency-of" issue 2 — both must produce edge 1 → 2.
        let issues = vec![
            make_issue(1, Status::Todo, Priority::High),
            make_issue(2, Status::Backlog, Priority::Medium),
        ];
        let depends_on_dag = Dag::build(&issues, &[make_relation(2, 1, RelationKind::DependsOn)]);
        let dependency_of_dag =
            Dag::build(&issues, &[make_relation(1, 2, RelationKind::DependencyOf)]);

        assert!(depends_on_dag.nodes[&1].forward.contains(&2));
        assert!(depends_on_dag.nodes[&2].reverse.contains(&1));
        assert!(dependency_of_dag.nodes[&1].forward.contains(&2));
        assert!(dependency_of_dag.nodes[&2].reverse.contains(&1));
    }

    #[test]
    fn find_ready_blocked_via_blocked_by_verb() {
        // Same scenario as find_ready_blocked but declared with the
        // "blocked-by" verb instead of "blocks" — must be equally enforced.
        let issues = vec![
            make_issue(1, Status::InProgress, Priority::High),
            make_issue(2, Status::Todo, Priority::Medium),
        ];
        let relations = vec![make_relation(2, 1, RelationKind::BlockedBy)];
        let dag = Dag::build(&issues, &relations);
        let ready = find_ready(&dag);
        // Issue 2 is blocked-by issue 1 which is not done
        assert!(ready.iter().all(|i| i.id != 2));
    }

    #[test]
    fn informational_relations_ignored() {
        let issues = vec![
            make_issue(1, Status::Todo, Priority::High),
            make_issue(2, Status::Backlog, Priority::Medium),
        ];
        let relations = vec![
            make_relation(1, 2, RelationKind::RelatesTo),
            make_relation(1, 2, RelationKind::Duplicates),
            make_relation(1, 2, RelationKind::DuplicateOf),
        ];
        let dag = Dag::build(&issues, &relations);
        assert!(dag.nodes[&1].forward.is_empty());
        assert!(dag.nodes[&1].reverse.is_empty());
        assert!(dag.nodes[&2].forward.is_empty());
        assert!(dag.nodes[&2].reverse.is_empty());
    }
}
