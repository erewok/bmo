use std::collections::{HashMap, HashSet, VecDeque};

use super::dag::Dag;

/// Topological sort using Kahn's algorithm.
/// Returns levels (phases) in order, where each level can run in parallel.
/// Returns Err if a cycle is detected, naming each cycle separately and only
/// the issues on a cycle (not issues merely downstream of one).
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
        let cycles = cycle_groups(dag);
        let Some(first_cycle) = cycles.first() else {
            // Only reachable when `forward` and `reverse` disagree, which
            // `Dag::build` never produces but the public fields allow.
            anyhow::bail!(
                "cycle detected in dependency graph, but no cycle could be traced; \
                 issues left unplanned: {}",
                display_ids(dag, &still_blocked_ids(&in_degree), ", ")
            );
        };
        anyhow::bail!(
            "cycle detected in dependency graph, involves issues: {}\n\
             hint: `A → B` means A blocks B. Pick an edge on a cycle and run `bmo link list {}` \
             to find its relation ids, then `bmo link remove <relation id>` for every relation \
             that creates that edge (an edge can be stored more than once, e.g. as both \
             `A blocks B` and `B depends-on A`). Repeat until no cycle remains.",
            cycles
                .iter()
                .map(|cycle| cycle.display(dag))
                .collect::<Vec<_>>()
                .join("; "),
            dag.nodes[&first_cycle.lowest_id()].issue.display_id()
        );
    }

    Ok(levels)
}

fn display_ids(dag: &Dag, ids: &[i64], separator: &str) -> String {
    ids.iter()
        .map(|id| dag.nodes[id].issue.display_id())
        .collect::<Vec<_>>()
        .join(separator)
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

/// The issues of one strongly connected component that contains a cycle.
enum CycleGroup {
    /// A single cycle, in blocking order starting at its lowest id.
    Simple(Vec<i64>),
    /// Several cycles sharing issues, as every `(blocker, blocked)` edge
    /// between members of the component, in ascending order.
    Overlapping(Vec<(i64, i64)>),
}

impl CycleGroup {
    fn lowest_id(&self) -> i64 {
        match self {
            CycleGroup::Simple(ids) => ids[0],
            // Every member of a cycle blocks some member, so the lowest member
            // is the blocker of the first edge.
            CycleGroup::Overlapping(edges) => edges[0].0,
        }
    }

    fn display(&self, dag: &Dag) -> String {
        match self {
            CycleGroup::Simple(cycle_order) => {
                let mut closed_cycle = cycle_order.clone();
                closed_cycle.push(cycle_order[0]);
                display_ids(dag, &closed_cycle, " → ")
            }
            CycleGroup::Overlapping(edges) => {
                let edges = edges
                    .iter()
                    .map(|&(blocker, blocked)| display_ids(dag, &[blocker, blocked], " → "))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{edges} (overlapping cycles)")
            }
        }
    }
}

/// Every dependency cycle in the graph, ordered by lowest member id.
fn cycle_groups(dag: &Dag) -> Vec<CycleGroup> {
    let mut groups: Vec<CycleGroup> = strongly_connected_components(dag)
        .into_iter()
        .filter_map(|members| cycle_group(dag, members))
        .collect();
    groups.sort_unstable_by_key(CycleGroup::lowest_id);
    groups
}

/// Classifies one strongly connected component, or `None` if it is a lone
/// issue that does not block itself.
fn cycle_group(dag: &Dag, mut members: Vec<i64>) -> Option<CycleGroup> {
    members.sort_unstable();
    let lowest = members[0];
    if members.len() == 1 && !dag.nodes[&lowest].forward.contains(&lowest) {
        return None;
    }

    // A component prints as one simple cycle only if every member blocks
    // exactly one other member of the same component; that single successor
    // is the next issue in the printed cycle order.
    let mut next_in_cycle: HashMap<i64, i64> = HashMap::new();
    for &member in &members {
        let mut successors_in_component = dag.nodes[&member]
            .forward
            .iter()
            .filter(|blocked_id| members.binary_search(blocked_id).is_ok());
        match (
            successors_in_component.next(),
            successors_in_component.next(),
        ) {
            (Some(&successor), None) => next_in_cycle.insert(member, successor),
            _ => return Some(CycleGroup::Overlapping(edges_within(dag, &members))),
        };
    }

    let cycle_order = std::iter::successors(Some(lowest), |id| next_in_cycle.get(id).copied())
        .take(members.len())
        .collect();
    Some(CycleGroup::Simple(cycle_order))
}

/// Every `(blocker, blocked)` edge between two of `sorted_members`, in ascending order.
fn edges_within(dag: &Dag, sorted_members: &[i64]) -> Vec<(i64, i64)> {
    let mut edges: Vec<(i64, i64)> = sorted_members
        .iter()
        .flat_map(|&blocker| {
            dag.nodes[&blocker]
                .forward
                .iter()
                .filter(|blocked_id| sorted_members.binary_search(blocked_id).is_ok())
                .map(move |&blocked| (blocker, blocked))
        })
        .collect();
    edges.sort_unstable();
    edges
}

fn strongly_connected_components(dag: &Dag) -> Vec<Vec<i64>> {
    let mut search = ComponentSearch::default();
    for &root in dag.nodes.keys() {
        if !search.discovery_index.contains_key(&root) {
            search.explore_from(dag, root);
        }
    }
    search.components
}

/// Tarjan's strongly-connected-components search over `forward` edges.
#[derive(Default)]
struct ComponentSearch {
    discovery_index: HashMap<i64, usize>,
    lowest_reachable_index: HashMap<i64, usize>,
    unassigned: Vec<i64>,
    is_unassigned: HashSet<i64>,
    components: Vec<Vec<i64>>,
}

impl ComponentSearch {
    // Iterative rather than recursive: a recursive walk overflows the call
    // stack on a dependency chain a few thousand issues long.
    fn explore_from(&mut self, dag: &Dag, root: i64) {
        self.discover(root);
        let mut path = vec![(root, dag.nodes[&root].forward.iter())];

        while let Some((id, unexplored)) = path.last_mut() {
            let id = *id;
            let Some(&blocked) = unexplored.next() else {
                path.pop();
                let parent = path.last().map(|(parent, _)| *parent);
                self.finish(id, parent);
                continue;
            };
            let Some(blocked_node) = dag.nodes.get(&blocked) else {
                continue;
            };
            if !self.discovery_index.contains_key(&blocked) {
                self.discover(blocked);
                path.push((blocked, blocked_node.forward.iter()));
            } else if self.is_unassigned.contains(&blocked) {
                let blocked_index = self.discovery_index[&blocked];
                self.lower_reachable_index(id, blocked_index);
            }
        }
    }

    fn discover(&mut self, id: i64) {
        let index = self.discovery_index.len();
        self.discovery_index.insert(id, index);
        self.lowest_reachable_index.insert(id, index);
        self.unassigned.push(id);
        self.is_unassigned.insert(id);
    }

    fn lower_reachable_index(&mut self, id: i64, candidate: usize) {
        self.lowest_reachable_index
            .entry(id)
            .and_modify(|lowest| *lowest = (*lowest).min(candidate));
    }

    fn finish(&mut self, id: i64, parent: Option<i64>) {
        let lowest_reachable = self.lowest_reachable_index[&id];
        if let Some(parent) = parent {
            self.lower_reachable_index(parent, lowest_reachable);
        }
        if lowest_reachable != self.discovery_index[&id] {
            return;
        }

        let mut component = Vec::new();
        while let Some(member) = self.unassigned.pop() {
            self.is_unassigned.remove(&member);
            component.push(member);
            if member == id {
                break;
            }
        }
        self.components.push(component);
    }
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

    fn cycle_error_naming(groups: &str, first_issue: &str) -> String {
        format!(
            "cycle detected in dependency graph, involves issues: {groups}\n\
             hint: `A → B` means A blocks B. Pick an edge on a cycle and run \
             `bmo link list {first_issue}` to find its relation ids, then \
             `bmo link remove <relation id>` for every relation that creates that edge \
             (an edge can be stored more than once, e.g. as both `A blocks B` and \
             `B depends-on A`). Repeat until no cycle remains."
        )
    }

    // 1 ⇄ 2 is the only cycle. 3 and 4 are merely downstream of it, 5 is
    // unrelated and 6 is upstream; none of those four lie on a cycle.
    fn cycle_1_2_only() -> String {
        cycle_error_naming("BMO-1 → BMO-2 → BMO-1", "BMO-1")
    }

    #[test]
    fn cycle_error_lists_only_cycle_members() {
        let relations = [rel(1, 2), rel(2, 1), rel(2, 3), rel(3, 4), rel(6, 1)];
        assert_eq!(cycle_error(6, &relations), cycle_1_2_only());
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
        assert_eq!(cycle_error(6, &relations), cycle_1_2_only());
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
        assert_eq!(cycle_error(6, &relations), cycle_1_2_only());
    }

    #[test]
    fn cycle_error_prints_independent_cycles_as_separate_groups() {
        // 1 ⇄ 2 and 3 → 4 → 5 → 3 share no issue and no edge.
        let relations = [rel(1, 2), rel(2, 1), rel(3, 4), rel(4, 5), rel(5, 3)];
        assert_eq!(
            cycle_error(5, &relations),
            cycle_error_naming(
                "BMO-1 → BMO-2 → BMO-1; BMO-3 → BMO-4 → BMO-5 → BMO-3",
                "BMO-1"
            )
        );
    }

    #[test]
    fn cycle_error_orders_groups_by_lowest_member_id() {
        // 6 → 2 → 6 is declared after 9 → 4 → 9 and 3 → 3, but holds the lowest id.
        let relations = [rel(9, 4), rel(4, 9), rel(3, 3), rel(6, 2), rel(2, 6)];
        assert_eq!(
            cycle_error(9, &relations),
            cycle_error_naming(
                "BMO-2 → BMO-6 → BMO-2; BMO-3 → BMO-3; BMO-4 → BMO-9 → BMO-4",
                "BMO-2"
            )
        );
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
            cycle_error_naming("BMO-1 → BMO-2 → BMO-1; BMO-4 → BMO-5 → BMO-4", "BMO-1")
        );
    }

    #[test]
    fn cycle_error_prints_a_long_cycle_in_blocking_order_from_its_lowest_id() {
        // 7 → 3 → 5 → 7, with 9 downstream.
        let relations = [rel(7, 3), rel(3, 5), rel(5, 7), rel(5, 9)];
        assert_eq!(
            cycle_error(9, &relations),
            cycle_error_naming("BMO-3 → BMO-5 → BMO-7 → BMO-3", "BMO-3")
        );
    }

    #[test]
    fn cycle_error_follows_blocking_direction_not_id_order() {
        // 1 → 3 → 2 → 1: ascending id order would misstate who blocks whom.
        let relations = [rel(1, 3), rel(3, 2), rel(2, 1)];
        assert_eq!(
            cycle_error(3, &relations),
            cycle_error_naming("BMO-1 → BMO-3 → BMO-2 → BMO-1", "BMO-1")
        );
    }

    #[test]
    fn cycle_error_names_a_self_blocking_issue() {
        let relations = [rel(1, 1), rel(1, 2)];
        assert_eq!(
            cycle_error(2, &relations),
            cycle_error_naming("BMO-1 → BMO-1", "BMO-1")
        );
    }

    #[test]
    fn cycle_error_lists_the_edges_of_cycles_sharing_an_issue() {
        // 1 ⇄ 2 and 2 ⇄ 3 share issue 2; 4 is downstream, so 3 → 4 is omitted.
        let relations = [rel(1, 2), rel(2, 1), rel(2, 3), rel(3, 2), rel(3, 4)];
        assert_eq!(
            cycle_error(4, &relations),
            cycle_error_naming(
                "BMO-1 → BMO-2, BMO-2 → BMO-1, BMO-2 → BMO-3, BMO-3 → BMO-2 (overlapping cycles)",
                "BMO-1"
            )
        );
    }

    #[test]
    fn cycle_error_lists_a_self_loop_inside_a_larger_cycle_as_an_edge() {
        let relations = [rel(1, 2), rel(2, 1), rel(2, 2)];
        assert_eq!(
            cycle_error(2, &relations),
            cycle_error_naming(
                "BMO-1 → BMO-2, BMO-2 → BMO-1, BMO-2 → BMO-2 (overlapping cycles)",
                "BMO-1"
            )
        );
    }

    #[test]
    fn cycle_error_mixes_simple_and_overlapping_groups() {
        // 2 ⇄ 3 ⇄ 4 overlap; 5 → 6 → 5 is a separate simple cycle; 1 is upstream.
        let relations = [
            rel(1, 2),
            rel(2, 3),
            rel(3, 2),
            rel(3, 4),
            rel(4, 3),
            rel(6, 5),
            rel(5, 6),
        ];
        assert_eq!(
            cycle_error(6, &relations),
            cycle_error_naming(
                "BMO-2 → BMO-3, BMO-3 → BMO-2, BMO-3 → BMO-4, BMO-4 → BMO-3 \
                 (overlapping cycles); BMO-5 → BMO-6 → BMO-5",
                "BMO-2"
            )
        );
    }

    #[test]
    fn cycle_error_hint_names_both_remediation_commands() {
        // The cycle is 3 ⇄ 4; 1 and 2 are upstream and must not be suggested.
        let relations = [rel(1, 2), rel(2, 3), rel(3, 4), rel(4, 3)];
        let message = cycle_error(4, &relations);
        let hint = message.lines().nth(1).expect("hint line missing");

        assert!(hint.starts_with("hint: "), "{hint}");
        assert!(hint.contains("`bmo link list BMO-3`"), "{hint}");
        assert!(hint.contains("`bmo link remove <relation id>`"), "{hint}");
        assert_eq!(message.lines().count(), 2);
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
    fn inconsistent_graph_error_has_no_hint() {
        let issues: Vec<Issue> = (1..=2).map(make_issue).collect();
        let mut dag = Dag::build(&issues, &[]);
        dag.nodes.get_mut(&2).unwrap().reverse.insert(1);

        let message = topological_levels(&dag).unwrap_err().to_string();
        assert!(!message.contains("hint"), "{message}");
        assert!(!message.contains("bmo link"), "{message}");
        assert_eq!(message.lines().count(), 1);
    }

    #[test]
    fn graph_with_a_cycle_and_an_inconsistency_reports_only_the_cycle() {
        // 1 ⇄ 2 is a real cycle; 4 records 3 as a blocker with no matching
        // forward edge.
        let issues: Vec<Issue> = (1..=4).map(make_issue).collect();
        let mut dag = Dag::build(&issues, &[rel(1, 2), rel(2, 1)]);
        dag.nodes.get_mut(&4).unwrap().reverse.insert(3);

        assert_eq!(
            topological_levels(&dag).unwrap_err().to_string(),
            cycle_error_naming("BMO-1 → BMO-2 → BMO-1", "BMO-1")
        );
    }

    #[test]
    fn cycle_error_tolerates_forward_edges_to_missing_issues() {
        // 1 ⇄ 2, with 2 also recording a blocked issue that is not a node.
        let issues: Vec<Issue> = (1..=2).map(make_issue).collect();
        let mut dag = Dag::build(&issues, &[rel(1, 2), rel(2, 1)]);
        dag.nodes.get_mut(&2).unwrap().forward.insert(99);

        assert_eq!(
            topological_levels(&dag).unwrap_err().to_string(),
            cycle_error_naming("BMO-1 → BMO-2 → BMO-1", "BMO-1")
        );
    }

    #[test]
    fn cycle_error_survives_a_cycle_thousands_of_issues_long() {
        let issue_count = 20_000;
        let relations: Vec<Relation> = (1..=issue_count)
            .map(|id| rel(id, id % issue_count + 1))
            .collect();
        let message = cycle_error(issue_count, &relations);

        assert!(
            message.starts_with(
                "cycle detected in dependency graph, involves issues: BMO-1 → BMO-2 → BMO-3 → "
            ),
            "{}",
            &message[..120]
        );
        assert!(message.contains(" → BMO-20000 → BMO-1\nhint: "));
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
            rel(3, 2),
            rel(2, 3),
            rel(2, 1),
            rel(1, 2),
            rel(22, 22),
        ];
        let first = cycle_error(24, &relations);
        assert_eq!(
            first,
            cycle_error_naming(
                "BMO-1 → BMO-2, BMO-2 → BMO-1, BMO-2 → BMO-3, BMO-3 → BMO-2 \
                 (overlapping cycles); \
                 BMO-4 → BMO-8 → BMO-12 → BMO-16 → BMO-4; BMO-22 → BMO-22",
                "BMO-1"
            )
        );
        for _ in 0..50 {
            assert_eq!(cycle_error(24, &relations), first);
        }
    }
}
