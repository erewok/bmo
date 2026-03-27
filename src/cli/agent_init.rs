use clap::Args;

use crate::config::{Config, init_bmo_dir};
use crate::db::{Repository, open_db};
use crate::model::{IssueFilter, Status};
use crate::output::{BoardColumns, OutputMode, make_printer};
use crate::planner::dag::{Dag, find_ready};
use crate::planner::topo::topological_levels;

#[derive(Args)]
pub struct AgentInitArgs {}

pub const CHEAT_SHEET: &str = r#"## BMO Quick Reference

### Claiming & Working Issues
  bmo issue claim BMO-N [--assignee <name>]  # atomically claim a ticket (exits 4 if already claimed)
  bmo issue show  BMO-N --json               # full details + comments
  bmo issue file conflicts BMO-N --json      # check for file overlaps with other in-progress work
  bmo issue comment add BMO-N --body "..."   # record findings, decisions, handoffs
  bmo issue move  BMO-N --status review      # advance status
  bmo issue close BMO-N                      # mark done

### Planning & Discovery
  bmo agent-init --json                      # refresh board state (run once per session)
  bmo next --json                            # work-ready issues (no unresolved blockers)
  bmo plan --phase 1 --json                  # all issues in phase 1 (iterate phases 1..N)
  bmo board --json                           # full kanban overview

### JSON Parsing
  bmo next --json | jq '.data[] | {id: .id, title: .title}'
  bmo issue comment list BMO-N --json | jq '.data[] | select(.body | startswith("HANDOFF:")) | .body'

### Comment Tags (prefix every agent comment with the appropriate tag)
  BLOCKER:    — work cannot proceed without resolution (any agent)
  CONCERN:    — should be addressed, not a hard stop (staff-engineer, ux-designer)
  SUGGESTION: — optional improvement (staff-engineer)
  APPROVED:   — review complete, change accepted (staff-engineer)
  BUG:        — defect found with reproduction steps (qa-engineer)
  VERIFIED:   — acceptance criteria confirmed passing (qa-engineer)
  FINDING:    — information discovered during implementation (senior-engineer)
  DECISION:   — approach chosen and rationale (senior-engineer)
  HANDOFF:    — work complete, context for the next agent (any agent)"#;

pub fn run(_args: &AgentInitArgs, json: bool) -> anyhow::Result<()> {
    // ── Collect phase: run all sub-operations, fail fast on error ────────────

    // 1. init
    let bmo_dir = init_bmo_dir()?;
    let db_path = bmo_dir.join("issues.db");
    let already_existed = db_path.exists();
    let repo = open_db(&db_path)?;

    // 2. config
    let config = Config::load(&bmo_dir)?;

    // 3. board
    let board_filter = crate::filter::FilterBuilder {
        findall: true,
        limit: 500,
        ..Default::default()
    }
    .build()?;
    let all_issues_for_board = repo.list_issues(board_filter)?;
    let board = BoardColumns {
        backlog: all_issues_for_board
            .iter()
            .filter(|i| i.status == Status::Backlog)
            .cloned()
            .collect(),
        todo: all_issues_for_board
            .iter()
            .filter(|i| i.status == Status::Todo)
            .cloned()
            .collect(),
        in_progress: all_issues_for_board
            .iter()
            .filter(|i| i.status == Status::InProgress)
            .cloned()
            .collect(),
        review: all_issues_for_board
            .iter()
            .filter(|i| i.status == Status::Review)
            .cloned()
            .collect(),
        done: all_issues_for_board
            .iter()
            .filter(|i| i.status == Status::Done)
            .cloned()
            .collect(),
    };

    // 4. next (unblocked, work-ready issues)
    let all_issues_for_next = repo.list_issues(IssueFilter::default())?;
    let all_relations = repo.list_all_relations()?;
    let dag = Dag::build(&all_issues_for_next, &all_relations);
    topological_levels(&dag)?;
    let next: Vec<_> = find_ready(&dag).into_iter().take(10).cloned().collect();

    // 5. stats
    let stats = repo.get_stats()?;

    // ── Emit phase: all data collected, now produce output ───────────────────

    if json {
        let data = serde_json::json!({
            "init": {
                "db_path": db_path.to_string_lossy(),
                "already_existed": already_existed,
            },
            "config": {
                "project_name": config.project_name,
                "default_assignee": config.default_assignee,
                "web_port": config.web_port(),
                "web_host": config.web_host(),
            },
            "board": board,
            "next": next,
            "stats": stats,
        });
        let envelope = serde_json::json!({
            "ok": true,
            "data": data,
            "message": "Session initialized.",
            "cheat_sheet": CHEAT_SHEET,
        });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else {
        let printer = make_printer(OutputMode::Human);

        // init section
        println!("=== init ===");
        if already_existed {
            println!("Already initialized — database at {}", db_path.display());
        } else {
            println!("Initialized bmo project at {}", db_path.display());
        }
        println!();

        // config section
        println!("=== config ===");
        println!(
            "project_name     = {}",
            config.project_name.as_deref().unwrap_or("(not set)")
        );
        println!(
            "default_assignee = {}",
            config.default_assignee.as_deref().unwrap_or("(not set)")
        );
        println!("web_port         = {}", config.web_port());
        println!("web_host         = {}", config.web_host());
        println!();

        // board section
        println!("=== board ===");
        printer.print_board(&board);
        println!();

        // next section
        println!("=== next ===");
        printer.print_issue_list(&next);
        println!();

        // stats section
        println!("=== stats ===");
        printer.print_stats(&stats);
        println!();

        // cheat sheet
        println!("=== cheat sheet ===");
        println!("{CHEAT_SHEET}");
    }

    Ok(())
}

/// Run agent-init against an explicit bmo_dir path.
/// Used in tests to avoid mutating CWD and in the test helper below.
fn run_with_dir_inner(bmo_dir: &std::path::Path, json: bool) -> anyhow::Result<()> {
    let db_path = bmo_dir.join("issues.db");
    let already_existed = db_path.exists();
    let repo = open_db(&db_path)?;

    let config = Config::load(bmo_dir)?;

    let board_filter = crate::filter::FilterBuilder {
        findall: true,
        limit: 500,
        ..Default::default()
    }
    .build()?;
    let all_issues_for_board = repo.list_issues(board_filter)?;
    let board = BoardColumns {
        backlog: all_issues_for_board
            .iter()
            .filter(|i| i.status == Status::Backlog)
            .cloned()
            .collect(),
        todo: all_issues_for_board
            .iter()
            .filter(|i| i.status == Status::Todo)
            .cloned()
            .collect(),
        in_progress: all_issues_for_board
            .iter()
            .filter(|i| i.status == Status::InProgress)
            .cloned()
            .collect(),
        review: all_issues_for_board
            .iter()
            .filter(|i| i.status == Status::Review)
            .cloned()
            .collect(),
        done: all_issues_for_board
            .iter()
            .filter(|i| i.status == Status::Done)
            .cloned()
            .collect(),
    };

    let all_issues_for_next = repo.list_issues(IssueFilter::default())?;
    let all_relations = repo.list_all_relations()?;
    let dag = Dag::build(&all_issues_for_next, &all_relations);
    topological_levels(&dag)?;
    let next: Vec<_> = find_ready(&dag).into_iter().take(10).cloned().collect();

    let stats = repo.get_stats()?;

    if json {
        let data = serde_json::json!({
            "init": {
                "db_path": db_path.to_string_lossy(),
                "already_existed": already_existed,
            },
            "config": {
                "project_name": config.project_name,
                "default_assignee": config.default_assignee,
                "web_port": config.web_port(),
                "web_host": config.web_host(),
            },
            "board": board,
            "next": next,
            "stats": stats,
        });
        let envelope = serde_json::json!({
            "ok": true,
            "data": data,
            "message": "Session initialized.",
            "cheat_sheet": CHEAT_SHEET,
        });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else {
        let printer = make_printer(OutputMode::Human);

        println!("=== init ===");
        if already_existed {
            println!("Already initialized — database at {}", db_path.display());
        } else {
            println!("Initialized bmo project at {}", db_path.display());
        }
        println!();

        println!("=== config ===");
        println!(
            "project_name     = {}",
            config.project_name.as_deref().unwrap_or("(not set)")
        );
        println!(
            "default_assignee = {}",
            config.default_assignee.as_deref().unwrap_or("(not set)")
        );
        println!("web_port         = {}", config.web_port());
        println!("web_host         = {}", config.web_host());
        println!();

        println!("=== board ===");
        printer.print_board(&board);
        println!();

        println!("=== next ===");
        printer.print_issue_list(&next);
        println!();

        println!("=== stats ===");
        printer.print_stats(&stats);
        println!();

        println!("=== cheat sheet ===");
        println!("{CHEAT_SHEET}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_bmo_dir() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let bmo_dir = tmp.path().join(".bmo");
        std::fs::create_dir_all(&bmo_dir).unwrap();
        // Initialize the database
        let db_path = bmo_dir.join("issues.db");
        open_db(&db_path).unwrap();
        tmp
    }

    #[test]
    fn cheat_sheet_is_non_empty() {
        assert!(!CHEAT_SHEET.is_empty());
        assert!(CHEAT_SHEET.contains("bmo agent-init"));
        assert!(CHEAT_SHEET.contains("HANDOFF:"));
    }

    #[test]
    fn json_output_does_not_error() {
        let tmp = setup_bmo_dir();
        let bmo_dir = tmp.path().join(".bmo");
        run_with_dir_inner(&bmo_dir, true).unwrap();
    }

    #[test]
    fn human_output_does_not_error() {
        let tmp = setup_bmo_dir();
        let bmo_dir = tmp.path().join(".bmo");
        run_with_dir_inner(&bmo_dir, false).unwrap();
    }
}
