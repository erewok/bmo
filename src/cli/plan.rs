use clap::Args;

use crate::config::find_bmo_dir;
use crate::db::{Repository, open_db};
use crate::errors::ErrorCode;
use crate::model::IssueFilter;
use crate::output::{ExecutionPlan, OutputMode, Phase, make_printer};
use crate::planner::dag::Dag;
use crate::planner::plan::generate_plan;

#[derive(Args)]
pub struct PlanArgs {
    /// Filter by assignee (requires --phase)
    #[arg(short, long)]
    pub assignee: Option<String>,

    /// Show only the issues in phase N. In --json mode returns a flat array, not the full plan
    /// envelope; in human mode prints a header plus a formatted list.
    #[arg(long)]
    pub phase: Option<u32>,
}

pub fn run(args: &PlanArgs, json: bool) -> anyhow::Result<()> {
    let bmo_dir = find_bmo_dir()?;
    let repo = open_db(&bmo_dir.join("issues.db"))?;
    let printer = make_printer(if json {
        OutputMode::Json
    } else {
        OutputMode::Human
    });

    // Validate: --assignee requires --phase
    if args.assignee.is_some() && args.phase.is_none() {
        printer.print_error("--assignee requires --phase", ErrorCode::Validation);
        std::process::exit(ErrorCode::Validation.exit_code());
    }

    let all_issues = repo.list_issues(IssueFilter::default())?;
    let all_relations = repo.list_all_relations()?;

    let dag = Dag::build(&all_issues, &all_relations);

    // NOTE: The --assignee flag is declared in PlanArgs but generate_plan() operates on a
    // pre-built Dag and has no assignee parameter. As of this implementation, assignee
    // filtering is NOT yet wired into generate_plan(). When --phase and --assignee are both
    // given, we apply assignee filtering post-generation by retaining only issues whose
    // assignee matches. Validation of the phase number (out-of-range check) always uses the
    // unfiltered plan's total_phases so that --phase N with an assignee that empties a phase
    // returns an empty array rather than a validation error.
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

    if let Some(phase_n) = args.phase {
        // Validate phase number against the unfiltered plan's total_phases.
        // We use total_phases from the full plan so that an assignee filter that empties a
        // phase does not convert an in-range phase number into a validation error.
        let total = output_plan.total_phases as u32;
        if phase_n == 0 || phase_n > total {
            let msg = format!(
                "phase {} does not exist (plan has {} phase{})",
                phase_n,
                total,
                if total == 1 { "" } else { "s" }
            );
            printer.print_error(&msg, ErrorCode::Validation);
            std::process::exit(ErrorCode::Validation.exit_code());
        }

        // Find the matching phase and extract its issues.
        let mut issues: Vec<_> = output_plan
            .phases
            .into_iter()
            .find(|p| p.number == phase_n as usize)
            .map(|p| p.issues)
            .unwrap_or_default();

        // Apply optional assignee filter post-generation (see note above).
        if let Some(ref assignee) = args.assignee {
            issues.retain(|i| i.assignee.as_deref() == Some(assignee.as_str()));
        }

        // Human output: compact list matching the documented format.
        if json {
            let msg = format!("Phase {}: {} issue(s).", phase_n, issues.len());
            let out = serde_json::json!({
                "ok": true,
                "data": issues,
                "message": msg,
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
        } else {
            println!(
                "Phase {} ({} issue{}):",
                phase_n,
                issues.len(),
                if issues.len() == 1 { "" } else { "s" }
            );
            for issue in &issues {
                println!(
                    "  {}  {}  {}  {}  {}",
                    issue.display_id(),
                    issue.status,
                    issue.priority,
                    issue.kind,
                    issue.title,
                );
            }
        }
        return Ok(());
    }

    printer.print_plan(&output_plan);
    Ok(())
}
