use clap::Args;

use crate::config::find_bmo_dir;
use crate::db::{Repository, open_db};
use crate::model::IssueFilter;
use crate::output::{ExecutionPlan, OutputMode, Phase, make_printer};
use crate::planner::dag::Dag;
use crate::planner::plan::generate_plan;

#[derive(Args)]
pub struct PlanArgs {
    /// Filter by assignee
    #[arg(short, long)]
    pub assignee: Option<String>,
}

pub fn run(_args: &PlanArgs, json: bool) -> anyhow::Result<()> {
    let bmo_dir = find_bmo_dir()?;
    let repo = open_db(&bmo_dir.join("issues.db"))?;
    let printer = make_printer(if json {
        OutputMode::Json
    } else {
        OutputMode::Human
    });

    let all_issues = repo.list_issues(&IssueFilter {
        include_done: false,
        ..Default::default()
    })?;
    let all_relations = repo.list_all_relations()?;

    let dag = Dag::build(&all_issues, &all_relations);
    let internal_plan = generate_plan(&dag)?;

    // Convert planner::plan::Phase to output::Phase
    let output_plan = ExecutionPlan {
        total_issues: internal_plan.total_issues,
        total_phases: internal_plan.total_phases,
        max_parallelism: internal_plan.max_parallelism,
        phases: internal_plan
            .phases
            .into_iter()
            .map(|p| Phase {
                number: p.number,
                issues: p.issues,
            })
            .collect(),
    };

    printer.print_plan(&output_plan);
    Ok(())
}
