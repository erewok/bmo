use crate::model::Issue;

use super::dag::Dag;
use super::topo::topological_levels;

#[derive(Debug, serde::Serialize, Clone)]
pub struct Phase {
    pub number: usize,
    pub issues: Vec<Issue>,
}

#[derive(Debug, serde::Serialize)]
pub struct ExecutionPlan {
    pub phases: Vec<Phase>,
    pub total_issues: usize,
    pub total_phases: usize,
    pub max_parallelism: usize,
}

/// Generate a phased execution plan from the DAG.
/// Returns Err if the DAG has a cycle.
pub fn generate_plan(dag: &Dag) -> anyhow::Result<ExecutionPlan> {
    let levels = topological_levels(dag)?;

    let mut phases: Vec<Phase> = Vec::new();
    let mut phase_num = 1usize;

    for level_ids in levels {
        if level_ids.is_empty() {
            continue;
        }
        let issues: Vec<Issue> = level_ids
            .iter()
            .filter_map(|id| dag.nodes.get(id).map(|n| n.issue.clone()))
            .collect();

        // File collision detection: split into sub-phases if issues in the same
        // phase touch the same files
        let sub_phases = split_by_file_collisions(issues);
        for sub in sub_phases {
            phases.push(Phase {
                number: phase_num,
                issues: sub,
            });
            phase_num += 1;
        }
    }

    let total_issues = phases.iter().map(|p| p.issues.len()).sum();
    let max_parallelism = phases.iter().map(|p| p.issues.len()).max().unwrap_or(0);
    let total_phases = phases.len();

    Ok(ExecutionPlan {
        phases,
        total_issues,
        total_phases,
        max_parallelism,
    })
}

/// Split issues in a single topological level into sub-phases to avoid
/// file collisions (two issues touching the same file shouldn't run in parallel).
fn split_by_file_collisions(issues: Vec<Issue>) -> Vec<Vec<Issue>> {
    if issues.is_empty() {
        return vec![];
    }

    let mut sub_phases: Vec<Vec<Issue>> = vec![];
    let mut used_files: Vec<std::collections::HashSet<String>> = vec![];

    'outer: for issue in issues {
        let issue_files: std::collections::HashSet<String> = issue.files.iter().cloned().collect();

        // Try to fit into an existing sub-phase that has no file conflict
        for (i, phase_files) in used_files.iter_mut().enumerate() {
            if issue_files.is_disjoint(phase_files) {
                phase_files.extend(issue_files.iter().cloned());
                sub_phases[i].push(issue);
                continue 'outer;
            }
        }

        // Start a new sub-phase
        used_files.push(issue_files);
        sub_phases.push(vec![issue]);
    }

    sub_phases
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Issue, Kind, Priority, Relation, RelationKind, Status};
    use crate::planner::dag::Dag;
    use chrono::Utc;

    fn make_issue(id: i64, files: Vec<&str>) -> Issue {
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
            files: files.iter().map(|s| s.to_string()).collect(),
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
    fn linear_three_phase_plan() {
        let issues = vec![
            make_issue(1, vec![]),
            make_issue(2, vec![]),
            make_issue(3, vec![]),
        ];
        let relations = vec![rel(1, 2), rel(2, 3)];
        let dag = Dag::build(&issues, &relations);
        let plan = generate_plan(&dag).unwrap();
        assert_eq!(plan.total_phases, 3);
        assert_eq!(plan.phases[0].issues[0].id, 1);
        assert_eq!(plan.phases[1].issues[0].id, 2);
        assert_eq!(plan.phases[2].issues[0].id, 3);
    }

    #[test]
    fn file_collision_splits_phase() {
        // Issues 2 and 3 are both unblocked by 1, but share a file
        let issues = vec![
            make_issue(1, vec![]),
            make_issue(2, vec!["src/lib.rs"]),
            make_issue(3, vec!["src/lib.rs"]),
        ];
        let relations = vec![rel(1, 2), rel(1, 3)];
        let dag = Dag::build(&issues, &relations);
        let plan = generate_plan(&dag).unwrap();
        // Phase 1: issue 1; then 2 and 3 must be in separate phases due to collision
        assert!(plan.total_phases >= 3);
    }

    #[test]
    fn cycle_returns_error() {
        let issues = vec![make_issue(1, vec![]), make_issue(2, vec![])];
        let relations = vec![rel(1, 2), rel(2, 1)];
        let dag = Dag::build(&issues, &relations);
        assert!(generate_plan(&dag).is_err());
    }
}
