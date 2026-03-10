use std::collections::{HashMap, VecDeque};

use super::dag::Dag;

/// Topological sort using Kahn's algorithm.
/// Returns levels (phases) in order, where each level can run in parallel.
/// Returns Err if a cycle is detected, with the IDs involved.
pub fn topological_levels(dag: &Dag) -> anyhow::Result<Vec<Vec<i64>>> {
    // in_degree = number of unresolved blockers for each node
    let mut in_degree: HashMap<i64, usize> = dag
        .nodes
        .keys()
        .map(|&id| (id, dag.nodes[&id].reverse.len()))
        .collect();

    // Start with nodes that have no blockers
    let mut queue: VecDeque<i64> = in_degree
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(id, _)| *id)
        .collect();

    let mut levels: Vec<Vec<i64>> = Vec::new();
    let mut processed = 0usize;

    while !queue.is_empty() {
        // Collect current level — all nodes currently unblocked
        let mut current_level: Vec<i64> = queue.drain(..).collect();
        current_level.sort(); // deterministic ordering within a phase
        processed += current_level.len();

        // Reduce in-degree of forward neighbors
        for &id in &current_level {
            for &fwd in &dag.nodes[&id].forward {
                let deg = in_degree.get_mut(&fwd).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    queue.push_back(fwd);
                }
            }
        }

        levels.push(current_level);
    }

    if processed < dag.nodes.len() {
        // Some nodes were never processed — cycle exists
        let cycle_ids: Vec<i64> = in_degree
            .iter()
            .filter(|(_, d)| **d > 0)
            .map(|(id, _)| *id)
            .collect();
        anyhow::bail!(
            "cycle detected in dependency graph, involves issues: {:?}",
            cycle_ids
        );
    }

    Ok(levels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Issue, Kind, Priority, Relation, RelationKind, Status};
    use crate::planner::dag::Dag;
    use chrono::Utc;

    fn make_issue(id: i64) -> Issue {
        Issue {
            id,
            parent_id: None,
            title: format!("Issue {id}"),
            description: String::new(),
            status: Status::Todo,
            priority: Priority::Medium,
            kind: Kind::Task,
            assignee: None,
            labels: vec![],
            files: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn rel(from: i64, to: i64) -> Relation {
        Relation {
            id: 0,
            from_id: from,
            to_id: to,
            kind: RelationKind::Blocks,
        }
    }

    #[test]
    fn linear_chain() {
        // 1 → 2 → 3
        let issues: Vec<Issue> = (1..=3).map(make_issue).collect();
        let relations = vec![rel(1, 2), rel(2, 3)];
        let dag = Dag::build(&issues, &relations);
        let levels = topological_levels(&dag).unwrap();
        assert_eq!(levels.len(), 3);
        assert_eq!(levels[0], vec![1]);
        assert_eq!(levels[1], vec![2]);
        assert_eq!(levels[2], vec![3]);
    }

    #[test]
    fn parallel_phase() {
        // 1 → 2, 1 → 3 (2 and 3 can run in parallel after 1)
        let issues: Vec<Issue> = (1..=3).map(make_issue).collect();
        let relations = vec![rel(1, 2), rel(1, 3)];
        let dag = Dag::build(&issues, &relations);
        let levels = topological_levels(&dag).unwrap();
        assert_eq!(levels.len(), 2);
        assert_eq!(levels[0], vec![1]);
        let mut second = levels[1].clone();
        second.sort();
        assert_eq!(second, vec![2, 3]);
    }

    #[test]
    fn cycle_detection() {
        // 1 → 2 → 1 (cycle)
        let issues: Vec<Issue> = (1..=2).map(make_issue).collect();
        let relations = vec![rel(1, 2), rel(2, 1)];
        let dag = Dag::build(&issues, &relations);
        assert!(topological_levels(&dag).is_err());
    }
}
