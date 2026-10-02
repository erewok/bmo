use std::collections::{HashMap, HashSet, VecDeque};

use super::dag::Dag;

/// Topological sort using Kahn's algorithm.
/// Returns levels (phases) in order, where each level can run in parallel.
/// Returns Err if a cycle is detected, naming only the issues on the cycle
/// itself (not issues merely downstream of it).
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
                // `Dag::build` keeps `forward` free of ids that are not nodes, but
                // a malformed graph must degrade rather than abort the process.
                let Some(deg) = in_degree.get_mut(&fwd) else {
                    continue;
                };
                if *deg == 0 {
                    // Already unblocked and queued; never double-queue it.
                    continue;
                }
                *deg -= 1;
                if *deg == 0 {
                    queue.push_back(fwd);
                }
            }
        }

        levels.push(current_level);
    }

    if processed < dag.nodes.len() {
        let cycle_members = cycle_member_ids(dag);
        if cycle_members.is_empty() {
            // Only reachable when `forward` and `reverse` disagree, which
            // `Dag::build` never produces but the public fields allow.
            anyhow::bail!(
                "cycle detected in dependency graph, but no cycle could be traced; \
                 issues left unplanned: {}",
                display_ids(dag, &still_blocked_ids(&in_degree))
            );
        }
        anyhow::bail!(
            "cycle detected in dependency graph, involves issues: {}",
            display_ids(dag, &cycle_members)
        );
    }

    Ok(levels)
}

fn display_ids(dag: &Dag, ids: &[i64]) -> String {
    ids.iter()
        .map(|id| dag.nodes[id].issue.display_id())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Ids of the issues whose blockers were never all resolved, in ascending order.
fn still_blocked_ids(in_degree: &HashMap<i64, usize>) -> Vec<i64> {
    let mut ids: Vec<i64> = in_degree
        .iter()
        .filter(|(_, remaining)| **remaining > 0)
        .map(|(id, _)| *id)
        .collect();
    ids.sort_unstable();
    ids
}

/// Ids of the issues that lie on a dependency cycle, in ascending order.
fn cycle_member_ids(dag: &Dag) -> Vec<i64> {
    let mut ids: Vec<i64> = dag
        .nodes
        .keys()
        .copied()
        .filter(|&id| blocks_itself_transitively(dag, id))
        .collect();
    ids.sort_unstable();
    ids
}

fn blocks_itself_transitively(dag: &Dag, start: i64) -> bool {
    let mut seen: HashSet<i64> = HashSet::new();
    let mut pending = vec![start];

    while let Some(id) = pending.pop() {
        let Some(node) = dag.nodes.get(&id) else {
            continue;
        };
        for &blocked in &node.forward {
            if blocked == start {
                return true;
            }
            if seen.insert(blocked) {
                pending.push(blocked);
            }
        }
    }

    false
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

    fn rel_of_kind(from: i64, to: i64, kind: RelationKind) -> Relation {
        Relation {
            id: 0,
            from_id: from,
            to_id: to,
            kind,
        }
    }

    fn cycle_error(issue_count: i64, relations: &[Relation]) -> String {
        let issues: Vec<Issue> = (1..=issue_count).map(make_issue).collect();
        let dag = Dag::build(&issues, relations);
        topological_levels(&dag).unwrap_err().to_string()
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

    // Regression: a relation pointing at an issue that is not a node (because it
    // is `done` and was filtered out of the issue list) used to panic on the
    // forward side and produce a phantom "cycle detected" on the reverse side.

    #[test]
    fn dangling_forward_edge_does_not_panic() {
        // Live issue 1 blocks issue 2, which is done and so is not a node.
        let issues = vec![make_issue(1)];
        let relations = vec![rel(1, 2)];
        let dag = Dag::build(&issues, &relations);

        let levels = topological_levels(&dag).unwrap();
        assert_eq!(levels, vec![vec![1]]);
    }

    #[test]
    fn dangling_reverse_edge_is_not_a_cycle() {
        // Issue 1 is done and not a node; live issue 2 was blocked by it.
        // The prerequisite is satisfied, so 2 belongs in the first phase.
        let issues = vec![make_issue(2)];
        let relations = vec![rel(1, 2)];
        let dag = Dag::build(&issues, &relations);

        let levels = topological_levels(&dag).unwrap();
        assert_eq!(levels, vec![vec![2]]);
    }

    #[test]
    fn satisfied_prerequisite_does_not_shift_phases() {
        // 1 → 2 → 3 with 1 done: the remaining chain must plan in two phases,
        // not three, and 2 must be immediately actionable.
        let issues = vec![make_issue(2), make_issue(3)];
        let relations = vec![rel(1, 2), rel(2, 3)];
        let dag = Dag::build(&issues, &relations);

        let levels = topological_levels(&dag).unwrap();
        assert_eq!(levels, vec![vec![2], vec![3]]);
    }

    #[test]
    fn cycle_detection() {
        // 1 → 2 → 1 (cycle)
        let issues: Vec<Issue> = (1..=2).map(make_issue).collect();
        let relations = vec![rel(1, 2), rel(2, 1)];
        let dag = Dag::build(&issues, &relations);
        assert!(topological_levels(&dag).is_err());
    }

    #[test]
    fn cycle_detection_via_blocked_by() {
        // Same 2-cycle as `cycle_detection`, but declared entirely with the
        // "blocked-by" verb: 1 blocked-by 2, 2 blocked-by 1.
        let issues: Vec<Issue> = (1..=2).map(make_issue).collect();
        let relations = vec![
            Relation {
                id: 0,
                from_id: 1,
                to_id: 2,
                kind: RelationKind::BlockedBy,
            },
            Relation {
                id: 0,
                from_id: 2,
                to_id: 1,
                kind: RelationKind::BlockedBy,
            },
        ];
        let dag = Dag::build(&issues, &relations);
        assert!(topological_levels(&dag).is_err());
    }

    #[test]
    fn cycle_detection_via_mixed_dependency_kinds() {
        // 1 → 2 via "dependency-of", closed back to a cycle via "depends-on".
        let issues: Vec<Issue> = (1..=2).map(make_issue).collect();
        let relations = vec![
            Relation {
                id: 0,
                from_id: 1,
                to_id: 2,
                kind: RelationKind::DependencyOf,
            },
            Relation {
                id: 0,
                from_id: 1,
                to_id: 2,
                kind: RelationKind::DependsOn,
            },
        ];
        let dag = Dag::build(&issues, &relations);
        assert!(topological_levels(&dag).is_err());
    }

    #[test]
    fn linear_chain_via_blocked_by_matches_blocks() {
        // 1 → 2 → 3 declared with "blocked-by" must yield identical phases
        // as the same chain declared with "blocks".
        let issues: Vec<Issue> = (1..=3).map(make_issue).collect();
        let relations = vec![
            Relation {
                id: 0,
                from_id: 2,
                to_id: 1,
                kind: RelationKind::BlockedBy,
            },
            Relation {
                id: 0,
                from_id: 3,
                to_id: 2,
                kind: RelationKind::BlockedBy,
            },
        ];
        let dag = Dag::build(&issues, &relations);
        let levels = topological_levels(&dag).unwrap();
        assert_eq!(levels.len(), 3);
        assert_eq!(levels[0], vec![1]);
        assert_eq!(levels[1], vec![2]);
        assert_eq!(levels[2], vec![3]);
    }

    // 1 ⇄ 2 is the only cycle. 3 and 4 are merely downstream of it, 5 is
    // unrelated and 6 is upstream; none of those four lie on a cycle.
    const CYCLE_1_2_ONLY: &str =
        "cycle detected in dependency graph, involves issues: BMO-1, BMO-2";

    #[test]
    fn cycle_error_lists_only_cycle_members() {
        let relations = [rel(1, 2), rel(2, 1), rel(2, 3), rel(3, 4), rel(6, 1)];
        assert_eq!(cycle_error(6, &relations), CYCLE_1_2_ONLY);
    }

    #[test]
    fn cycle_error_lists_only_cycle_members_via_blocked_by() {
        let relations = [
            rel_of_kind(2, 1, RelationKind::BlockedBy),
            rel_of_kind(1, 2, RelationKind::BlockedBy),
            rel_of_kind(3, 2, RelationKind::BlockedBy),
            rel_of_kind(4, 3, RelationKind::BlockedBy),
            rel_of_kind(1, 6, RelationKind::BlockedBy),
        ];
        assert_eq!(cycle_error(6, &relations), CYCLE_1_2_ONLY);
    }

    #[test]
    fn cycle_error_lists_only_cycle_members_via_mixed_dependency_kinds() {
        let relations = [
            rel_of_kind(1, 2, RelationKind::DependencyOf),
            rel_of_kind(1, 2, RelationKind::DependsOn),
            rel_of_kind(3, 2, RelationKind::BlockedBy),
            rel_of_kind(3, 4, RelationKind::Blocks),
            rel_of_kind(1, 6, RelationKind::DependsOn),
        ];
        assert_eq!(cycle_error(6, &relations), CYCLE_1_2_ONLY);
    }

    #[test]
    fn cycle_error_omits_issue_between_two_cycles() {
        // 1 ⇄ 2 → 3 → 4 ⇄ 5: issue 3 is downstream of one cycle and upstream
        // of the other, but no path leads from 3 back to 3.
        let relations = [
            rel(1, 2),
            rel(2, 1),
            rel(2, 3),
            rel(3, 4),
            rel(4, 5),
            rel(5, 4),
        ];
        assert_eq!(
            cycle_error(5, &relations),
            "cycle detected in dependency graph, involves issues: BMO-1, BMO-2, BMO-4, BMO-5"
        );
    }

    #[test]
    fn cycle_error_lists_every_member_of_a_long_cycle_in_id_order() {
        // 7 → 3 → 5 → 7, with 9 downstream.
        let relations = [rel(7, 3), rel(3, 5), rel(5, 7), rel(5, 9)];
        assert_eq!(
            cycle_error(9, &relations),
            "cycle detected in dependency graph, involves issues: BMO-3, BMO-5, BMO-7"
        );
    }

    #[test]
    fn cycle_error_names_a_self_blocking_issue() {
        let relations = [rel(1, 1), rel(1, 2)];
        assert_eq!(
            cycle_error(2, &relations),
            "cycle detected in dependency graph, involves issues: BMO-1"
        );
    }

    #[test]
    fn inconsistent_graph_error_names_the_unplanned_issues() {
        // 2 and 3 each record 1 as a blocker, but 1 records no forward edge,
        // so neither is ever unblocked and no forward path forms a cycle.
        // 4 is unrelated and plans normally.
        let issues: Vec<Issue> = (1..=4).map(make_issue).collect();
        let mut dag = Dag::build(&issues, &[]);
        for blocked in [3, 2] {
            dag.nodes.get_mut(&blocked).unwrap().reverse.insert(1);
        }

        assert_eq!(
            topological_levels(&dag).unwrap_err().to_string(),
            "cycle detected in dependency graph, but no cycle could be traced; \
             issues left unplanned: BMO-2, BMO-3"
        );
    }

    #[test]
    fn cycle_error_is_identical_across_repeated_runs() {
        // Each `Dag::build` gets freshly seeded hash maps, so any dependence
        // on hash iteration order shows up as differing messages.
        let relations = [
            rel(4, 8),
            rel(8, 12),
            rel(12, 16),
            rel(16, 4),
            rel(16, 20),
            rel(20, 24),
        ];
        let first = cycle_error(24, &relations);
        assert_eq!(
            first,
            "cycle detected in dependency graph, involves issues: BMO-4, BMO-8, BMO-12, BMO-16"
        );
        for _ in 0..50 {
            assert_eq!(cycle_error(24, &relations), first);
        }
    }
}
