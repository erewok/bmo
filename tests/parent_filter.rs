//! `--parent` scoping for `list`, `next`, `plan` and `board`.
//!
//! `--parent` matches every descendant of the parent, at any depth.
//!
//! Tests start with this issue tree:
//!
//! ```text
//! BMO-1  epic
//! ├── BMO-2  child, ready
//! ├── BMO-3  child, blocked by BMO-4
//! └── BMO-6  child with a sub-issue of its own
//!     └── BMO-7  grandchild
//! BMO-4  outside the epic, open, blocks BMO-3
//! BMO-5  outside the epic, unrelated
//! ```

use std::collections::BTreeSet;
use std::process::Command;

use assert_cmd::prelude::*;
use tempfile::TempDir;

mod common;
use common::bmo_command;

fn bmo(dir: &TempDir) -> Command {
    let mut cmd = bmo_command();
    cmd.current_dir(dir.path());
    cmd
}

fn create(dir: &TempDir, title: &str, extra: &[&str]) {
    bmo(dir)
        .args(["create", "--title", title])
        .args(extra)
        .assert()
        .success();
}

fn setup_epic_tree() -> TempDir {
    let dir = TempDir::new().unwrap();
    bmo(&dir).arg("init").assert().success();

    create(&dir, "Epic", &["--kind", "epic"]);
    create(&dir, "Ready child", &["--parent", "BMO-1"]);
    create(&dir, "Blocked child", &["--parent", "BMO-1"]);
    create(&dir, "Outside blocker", &[]);
    create(&dir, "Unrelated", &[]);
    create(&dir, "Child with sub-issue", &["--parent", "1"]);
    create(&dir, "Grandchild", &["--parent", "BMO-6"]);
    bmo(&dir)
        .args(["link", "add", "BMO-4", "blocks", "BMO-3"])
        .assert()
        .success();
    dir
}

fn json_data(dir: &TempDir, args: &[&str]) -> serde_json::Value {
    let output = bmo(dir).args(args).arg("--json").output().unwrap();
    assert!(
        output.status.success(),
        "{args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    envelope["data"].clone()
}

fn ids_of(issues: &serde_json::Value) -> BTreeSet<i64> {
    issues
        .as_array()
        .unwrap()
        .iter()
        .map(|issue| issue["id"].as_i64().unwrap())
        .collect()
}

#[test]
fn list_parent_returns_all_descendants_in_either_id_form() {
    let dir = setup_epic_tree();
    for parent in ["BMO-1", "bmo-1", "1"] {
        let issues = json_data(&dir, &["list", "--parent", parent]);
        assert_eq!(
            ids_of(&issues),
            BTreeSet::from([2, 3, 6, 7]),
            "--parent {parent}"
        );
    }
}

#[test]
fn parent_filters_accept_single_comma_separated_and_repeated_values() {
    let dir = setup_epic_tree();
    create(&dir, "Second tree child", &["--parent", "4"]);
    create(&dir, "Third tree child", &["--parent", "5"]);
    create(&dir, "Second tree grandchild", &["--parent", "8"]);

    for command in ["list", "ls", "next", "plan", "board"] {
        for (parents, multiple) in [
            (vec!["--parent", "1"], false),
            (vec!["--parent", "BMO-1"], false),
            (vec!["--parent", "1,4,5"], true),
            (vec!["--parent", "BMO-1,bmo-4,5"], true),
            (vec!["--parent", "1", "--parent", "4,5"], true),
            (vec!["--parent", "1,4,5,8,1,999"], true),
        ] {
            let mut args = vec![command];
            args.extend(parents);
            let data = json_data(&dir, &args);
            let issues = match command {
                "plan" => data["phases"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .flat_map(|phase| phase["issues"].as_array().unwrap().iter())
                    .cloned()
                    .collect::<Vec<_>>(),
                "board" => ["backlog", "todo", "in_progress", "review", "done"]
                    .iter()
                    .flat_map(|column| data[column].as_array().unwrap().iter())
                    .cloned()
                    .collect(),
                _ => data.as_array().unwrap().clone(),
            };
            let ids: Vec<i64> = issues
                .iter()
                .map(|issue| issue["id"].as_i64().unwrap())
                .collect();
            let unique: BTreeSet<i64> = ids.iter().copied().collect();
            assert_eq!(ids.len(), unique.len(), "duplicate issues for {args:?}");
            let expected = match (command, multiple) {
                ("next", false) => BTreeSet::from([2, 7]),
                ("next", true) => BTreeSet::from([2, 7, 9, 10]),
                ("plan", false) => BTreeSet::from([2, 3, 4, 6, 7]),
                ("plan", true) => BTreeSet::from([2, 3, 4, 6, 7, 8, 9, 10]),
                (_, false) => BTreeSet::from([2, 3, 6, 7]),
                (_, true) => BTreeSet::from([2, 3, 6, 7, 8, 9, 10]),
            };
            assert_eq!(unique, expected, "{args:?}");
        }
    }
}

#[test]
fn list_parent_reaches_descendants_through_a_done_sub_issue() {
    let dir = setup_epic_tree();
    bmo(&dir).args(["close", "BMO-6"]).assert().success();

    let issues = json_data(&dir, &["list", "--parent", "BMO-1"]);
    assert_eq!(ids_of(&issues), BTreeSet::from([2, 3, 7]));
}

#[test]
fn next_parent_returns_only_ready_descendants() {
    let dir = setup_epic_tree();

    // BMO-3 waits on the open BMO-4 outside the epic, and BMO-6 has a
    // sub-issue, so only BMO-2 and the grandchild BMO-7 are ready.
    let ready = json_data(&dir, &["next", "--parent", "BMO-1"]);
    assert_eq!(ids_of(&ready), BTreeSet::from([2, 7]));

    let ready_everywhere = json_data(&dir, &["next"]);
    assert_eq!(ids_of(&ready_everywhere), BTreeSet::from([2, 4, 5, 7]));
}

#[test]
fn next_parent_reports_a_child_once_its_outside_blocker_is_done() {
    let dir = setup_epic_tree();
    bmo(&dir).args(["close", "BMO-4"]).assert().success();

    let ready = json_data(&dir, &["next", "--parent", "BMO-1"]);
    assert_eq!(ids_of(&ready), BTreeSet::from([2, 3, 7]));
}

#[test]
fn plan_parent_orders_descendants_after_their_outside_blockers() {
    let dir = setup_epic_tree();
    let plan = json_data(&dir, &["plan", "--parent", "BMO-1"]);

    let phase_of = |id: i64| -> u64 {
        plan["phases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|phase| ids_of(&phase["issues"]).contains(&id))
            .unwrap_or_else(|| panic!("BMO-{id} is missing from the plan: {plan}"))["number"]
            .as_u64()
            .unwrap()
    };

    let planned: BTreeSet<i64> = plan["phases"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|phase| ids_of(&phase["issues"]))
        .collect();
    assert_eq!(planned, BTreeSet::from([2, 3, 4, 6, 7]));
    assert!(phase_of(4) < phase_of(3));
}

#[test]
fn plan_parent_includes_the_whole_chain_of_outside_blockers() {
    let dir = setup_epic_tree();
    create(&dir, "Blocker of the blocker", &[]);
    bmo(&dir)
        .args(["link", "add", "BMO-8", "blocks", "BMO-4"])
        .assert()
        .success();

    let plan = json_data(&dir, &["plan", "--parent", "BMO-1"]);
    let phases: Vec<BTreeSet<i64>> = plan["phases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|phase| ids_of(&phase["issues"]))
        .collect();
    let phase_of = |id: i64| phases.iter().position(|ids| ids.contains(&id)).unwrap();
    assert!(phase_of(8) < phase_of(4));
    assert!(phase_of(4) < phase_of(3));
    assert!(!phases.iter().any(|ids| ids.contains(&5)));
}

#[test]
fn board_parent_shows_descendants_across_all_columns() {
    let dir = setup_epic_tree();
    bmo(&dir).args(["close", "BMO-2"]).assert().success();

    let board = json_data(&dir, &["board", "--parent", "BMO-1"]);
    let shown: BTreeSet<i64> = ["backlog", "todo", "in_progress", "review", "done"]
        .iter()
        .flat_map(|column| ids_of(&board[column]))
        .collect();
    assert_eq!(shown, BTreeSet::from([2, 3, 6, 7]));
    assert_eq!(ids_of(&board["done"]), BTreeSet::from([2]));
}

#[test]
fn board_label_filter_applies() {
    let dir = setup_epic_tree();
    bmo(&dir)
        .args(["label", "add", "BMO-5", "ui"])
        .assert()
        .success();

    let board = json_data(&dir, &["board", "--label", "ui"]);
    let shown: BTreeSet<i64> = ["backlog", "todo", "in_progress", "review", "done"]
        .iter()
        .flat_map(|column| ids_of(&board[column]))
        .collect();
    assert_eq!(shown, BTreeSet::from([5]));
}

#[test]
fn parent_rejects_a_malformed_id() {
    let dir = setup_epic_tree();
    for command in ["list", "next", "plan", "board"] {
        for parent in ["BMO-x", "1,BMO-x", "BMO-x,1", "", "1,", ",1"] {
            bmo(&dir)
                .args([command, "--parent", parent])
                .assert()
                .failure()
                .stderr(predicates::str::contains("invalid value"));
        }
    }
}
