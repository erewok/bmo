use assert_cmd::prelude::*;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use std::process::Command;
use tempfile::TempDir;

mod common;
use common::bmo_command;

fn setup() -> TempDir {
    let dir = TempDir::new().unwrap();
    bmo_command()
        .current_dir(dir.path())
        .arg("init")
        .assert()
        .success();
    dir
}

fn bmo(dir: &TempDir) -> Command {
    let mut cmd = bmo_command();
    cmd.current_dir(dir.path());
    cmd
}

// ── Version ───────────────────────────────────────────────────────────────────

#[test]
fn version_prints_version() {
    let version = env!("CARGO_PKG_VERSION");
    bmo_command()
        .arg("version")
        .assert()
        .success()
        .stdout(contains(version));
}

#[test]
fn version_flags_print_the_same_line_as_the_version_subcommand() {
    let dir = TempDir::new().unwrap();
    let expected = format!("bmo {}\n", env!("CARGO_PKG_VERSION"));
    for invocation in ["version", "--version", "-V"] {
        bmo_command()
            .current_dir(dir.path())
            .arg(invocation)
            .assert()
            .success()
            .stdout(expected.clone());
    }
}

#[test]
fn version_subcommand_json_emits_envelope() {
    let version = env!("CARGO_PKG_VERSION");
    let output = bmo_command().args(["version", "--json"]).output().unwrap();
    assert!(output.status.success());
    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        envelope,
        serde_json::json!({ "ok": true, "data": { "version": version }, "message": version })
    );
}

#[test]
fn version_flag_prints_plain_text_even_with_json_flag() {
    let dir = TempDir::new().unwrap();
    let expected = format!("bmo {}\n", env!("CARGO_PKG_VERSION"));
    for invocation in [["--json", "--version"], ["--version", "--json"]] {
        bmo_command()
            .current_dir(dir.path())
            .args(invocation)
            .assert()
            .success()
            .stdout(expected.clone());
    }
}

#[test]
fn version_flag_before_a_subcommand_prints_version_without_running_it() {
    let dir = setup();
    bmo(&dir)
        .args(["create", "--title", "Listed issue"])
        .assert()
        .success();
    bmo(&dir)
        .arg("list")
        .assert()
        .success()
        .stdout(contains("Listed issue"));

    bmo(&dir)
        .args(["-V", "list"])
        .assert()
        .success()
        .stdout(format!("bmo {}\n", env!("CARGO_PKG_VERSION")));
}

// `-v` is conventionally `--verbose`, so it is not an alias for `--version`.
#[test]
fn lowercase_v_is_not_a_version_flag() {
    let dir = TempDir::new().unwrap();
    bmo_command()
        .current_dir(dir.path())
        .arg("-v")
        .assert()
        .code(2)
        .stdout("")
        .stderr(contains("unexpected argument"));
}

#[test]
fn version_flag_after_a_subcommand_is_rejected() {
    let dir = setup();
    for flag in ["-V", "--version"] {
        bmo(&dir)
            .args(["list", flag])
            .assert()
            .code(2)
            .stdout("")
            .stderr(contains("unexpected argument"));
    }
}

// ── Init ──────────────────────────────────────────────────────────────────────

#[test]
fn init_creates_bmo_dir() {
    let dir = TempDir::new().unwrap();
    bmo_command()
        .current_dir(dir.path())
        .arg("init")
        .assert()
        .success();
    assert!(dir.path().join(".bmo").is_dir());
    assert!(dir.path().join(".bmo").join("issues.db").exists());
}

#[test]
fn init_is_idempotent() {
    let dir = setup();
    // Second init should not fail
    bmo(&dir).arg("init").assert().success();
}

// ── Issue CRUD ────────────────────────────────────────────────────────────────

#[test]
fn create_and_list_issue() {
    let dir = setup();
    bmo(&dir)
        .args([
            "issue",
            "create",
            "--title",
            "My first issue",
            "--priority",
            "high",
            "--kind",
            "bug",
        ])
        .assert()
        .success()
        .stdout(contains("BMO-1"));

    bmo(&dir)
        .args(["issue", "list"])
        .assert()
        .success()
        .stdout(contains("My first issue"));
}

#[test]
fn list_filters_accept_single_comma_separated_and_repeated_values() {
    let dir = setup();
    for (title, status, priority, kind, labels) in [
        ("First", "todo", "high", "bug", vec!["red"]),
        ("Second", "review", "low", "task", vec!["blue"]),
        ("Third", "todo", "low", "bug", vec!["red", "blue"]),
        ("Fourth", "done", "high", "chore", vec!["red", "blue"]),
    ] {
        let output = bmo(&dir)
            .args([
                "create",
                "--title",
                title,
                "--status",
                status,
                "--priority",
                priority,
                "--kind",
                kind,
                "--json",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let created: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let id = created["data"]["id"].as_i64().unwrap().to_string();
        for label in labels {
            bmo(&dir)
                .args(["label", "add", &id, label])
                .assert()
                .success();
        }
    }

    let cases = [
        (vec!["--status", "todo"], vec![1, 3]),
        (vec!["--status", "todo,review"], vec![1, 2, 3]),
        (
            vec!["--status", "todo", "--status", "review"],
            vec![1, 2, 3],
        ),
        (vec!["--status", "todo,done"], vec![1, 3, 4]),
        (vec!["--priority", "high"], vec![1]),
        (vec!["--priority", "high,low"], vec![1, 2, 3]),
        (
            vec!["--priority", "high", "--priority", "low"],
            vec![1, 2, 3],
        ),
        (vec!["--kind", "bug"], vec![1, 3]),
        (vec!["--kind", "bug,task"], vec![1, 2, 3]),
        (vec!["--kind", "bug", "--kind", "task"], vec![1, 2, 3]),
        (vec!["--label", "red"], vec![1, 3]),
        (vec!["--label", "red,blue"], vec![3]),
        (vec!["--label", "red", "--label", "blue"], vec![3]),
        (vec!["--label", "red,blue", "--include-done"], vec![3, 4]),
        (
            vec![
                "--status",
                "todo,review",
                "--priority",
                "low",
                "--kind",
                "bug,task",
            ],
            vec![2, 3],
        ),
        (vec!["--status", "review", "--kind", "bug"], vec![]),
    ];
    for command in [vec!["list"], vec!["ls"], vec!["issue", "list"]] {
        for (filters, expected) in &cases {
            let output = bmo(&dir)
                .args(&command)
                .args(filters)
                .arg("--json")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{command:?} {filters:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(envelope["ok"], true);
            let mut ids: Vec<i64> = envelope["data"]
                .as_array()
                .unwrap()
                .iter()
                .map(|issue| issue["id"].as_i64().unwrap())
                .collect();
            ids.sort_unstable();
            assert_eq!(&ids, expected, "{command:?} {filters:?}");
        }
    }
}

#[test]
fn create_labels_and_board_filters_accept_single_lists_and_repeated_flags() {
    let dir = setup();
    for (title, priority, labels) in [
        ("First", "high", vec!["--label", "red"]),
        ("Second", "low", vec!["--label", "blue"]),
        ("Third", "high", vec!["--label", "red,blue"]),
        ("Fourth", "low", vec!["--label", "red", "--label", "blue"]),
        (
            "Fifth",
            "medium",
            vec!["--label", "red,blue", "--label", "green"],
        ),
    ] {
        bmo(&dir)
            .args(["create", "--title", title, "--priority", priority])
            .args(labels)
            .assert()
            .success();
    }
    bmo(&dir).args(["close", "4"]).assert().success();

    let output = bmo(&dir)
        .args(["label", "list", "5", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let labels: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let mut names: Vec<&str> = labels["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|label| label["name"].as_str().unwrap())
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["blue", "green", "red"]);

    for (filters, expected) in [
        (vec!["--priority", "high"], vec![1, 3]),
        (vec!["--priority", "high,low"], vec![1, 2, 3, 4]),
        (
            vec!["--priority", "high", "--priority", "low"],
            vec![1, 2, 3, 4],
        ),
        (
            vec!["--priority", "high,low", "--priority", "medium"],
            vec![1, 2, 3, 4, 5],
        ),
        (vec!["--label", "red"], vec![1, 3, 4, 5]),
        (vec!["--label", "red,blue"], vec![3, 4, 5]),
        (vec!["--label", "red", "--label", "blue"], vec![3, 4, 5]),
        (vec!["--label", "red,blue", "--label", "green"], vec![5]),
        (
            vec!["--priority", "high,low", "--label", "red,blue"],
            vec![3, 4],
        ),
    ] {
        let output = bmo(&dir)
            .arg("board")
            .args(&filters)
            .arg("--json")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{filters:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let board: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let mut ids: Vec<i64> = ["backlog", "todo", "in_progress", "review", "done"]
            .iter()
            .flat_map(|column| board["data"][column].as_array().unwrap())
            .map(|issue| issue["id"].as_i64().unwrap())
            .collect();
        ids.sort_unstable();
        assert_eq!(ids, expected, "{filters:?}");
    }
}

#[test]
fn truncate_status_accepts_single_lists_and_repeated_flags() {
    for (statuses, expected_deleted, expected_remaining) in [
        (vec!["--status", "done"], 1, vec![2, 3, 4]),
        (vec!["--status", "done,backlog"], 2, vec![3, 4]),
        (
            vec!["--status", "done", "--status", "backlog"],
            2,
            vec![3, 4],
        ),
        (
            vec!["--status", "done,backlog", "--status", "review"],
            3,
            vec![4],
        ),
    ] {
        let dir = setup();
        for status in ["done", "backlog", "review", "todo"] {
            bmo(&dir)
                .args(["create", "--title", status, "--status", status])
                .assert()
                .success();
        }
        let output = bmo(&dir)
            .arg("truncate")
            .args(&statuses)
            .args(["--yes", "--json"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{statuses:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["data"]["deleted"], expected_deleted, "{statuses:?}");

        let output = bmo(&dir)
            .args(["list", "--all", "--sort", "id", "--json"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let remaining: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let mut ids: Vec<i64> = remaining["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|issue| issue["id"].as_i64().unwrap())
            .collect();
        ids.sort_unstable();
        assert_eq!(ids, expected_remaining, "{statuses:?}");
    }
}

#[test]
fn multi_value_help_describes_supported_syntax() {
    for command in ["list", "board", "next", "plan", "truncate", "create"] {
        bmo_command()
            .args([command, "--help"])
            .assert()
            .success()
            .stdout(contains(
                "comma-separated values; alternatively, repeat the flag for each value",
            ));
    }
}

#[test]
fn list_filters_reject_invalid_values_anywhere_in_a_list() {
    let dir = setup();
    for (flag, valid) in [
        ("--status", "todo"),
        ("--priority", "high"),
        ("--kind", "bug"),
    ] {
        for value in [
            "invalid".to_string(),
            format!("{valid},invalid"),
            format!("invalid,{valid}"),
            format!("{valid},"),
        ] {
            bmo(&dir)
                .args(["list", flag, &value])
                .assert()
                .failure()
                .stderr(contains("unknown"));
        }
    }
}

#[test]
fn create_issue_json_envelope() {
    let dir = setup();
    let output = bmo(&dir)
        .args(["issue", "create", "--title", "JSON test", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["title"], "JSON test");
    assert!(json["data"]["id"].is_number());
}

#[test]
fn show_issue() {
    let dir = setup();
    bmo(&dir)
        .args(["issue", "create", "--title", "Show me"])
        .assert()
        .success();

    bmo(&dir)
        .args(["issue", "show", "BMO-1", "--json"])
        .assert()
        .success()
        .stdout(contains("\"title\""))
        .stdout(contains("Show me"));
}

#[test]
fn show_nonexistent_issue_fails() {
    let dir = setup();
    bmo(&dir)
        .args(["issue", "show", "BMO-999", "--json"])
        .assert()
        .failure()
        .stdout(contains("\"ok\":false").or(contains("\"ok\": false")));
}

#[test]
fn move_issue_status() {
    let dir = setup();
    bmo(&dir)
        .args(["issue", "create", "--title", "Move me"])
        .assert()
        .success();

    bmo(&dir)
        .args(["issue", "move", "BMO-1", "--status", "in-progress"])
        .assert()
        .success();

    let output = bmo(&dir)
        .args(["issue", "show", "BMO-1", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["data"]["issue"]["status"], "in-progress");
}

#[test]
fn close_and_reopen_issue() {
    let dir = setup();
    bmo(&dir)
        .args(["issue", "create", "--title", "Close me"])
        .assert()
        .success();

    bmo(&dir)
        .args(["issue", "close", "BMO-1"])
        .assert()
        .success();

    let output = bmo(&dir)
        .args(["issue", "show", "BMO-1", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["data"]["issue"]["status"], "done");

    bmo(&dir)
        .args(["issue", "reopen", "BMO-1"])
        .assert()
        .success();

    let output = bmo(&dir)
        .args(["issue", "show", "BMO-1", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["data"]["issue"]["status"], "todo");
}

// ── Issue list --oneline ──────────────────────────────────────────────────────

#[test]
fn issue_list_oneline() {
    let dir = setup();
    bmo(&dir)
        .args(["issue", "create", "--title", "Oneline test issue"])
        .assert()
        .success();

    // --oneline produces a single line containing the ID and title
    let output = bmo(&dir)
        .args(["issue", "list", "--oneline"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("BMO-1"));
    assert!(stdout.contains("Oneline test issue"));
    // One line per issue — no table borders or padding
    assert!(!stdout.contains("─"));
}

#[test]
fn issue_list_oneline_json_takes_precedence() {
    let dir = setup();
    bmo(&dir)
        .args(["issue", "create", "--title", "JSON wins"])
        .assert()
        .success();

    // --json beats --oneline: output must be valid JSON with the envelope
    let output = bmo(&dir)
        .args(["issue", "list", "--oneline", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["ok"], true);
    assert!(json["data"].is_array());
}

// ── Board ─────────────────────────────────────────────────────────────────────

#[test]
fn board_shows_issues() {
    let dir = setup();
    bmo(&dir)
        .args(["issue", "create", "--title", "Board issue"])
        .assert()
        .success();

    bmo(&dir).args(["board"]).assert().success();
}

// ── Stats ─────────────────────────────────────────────────────────────────────

#[test]
fn stats_json_output() {
    let dir = setup();
    bmo(&dir)
        .args(["issue", "create", "--title", "Stat me"])
        .assert()
        .success();

    let output = bmo(&dir).args(["stats", "--json"]).output().unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["total"], 1);
}

// ── Next (DAG) ────────────────────────────────────────────────────────────────

#[test]
fn next_returns_unblocked_issues() {
    let dir = setup();
    bmo(&dir)
        .args(["issue", "create", "--title", "Ready to go"])
        .assert()
        .success();

    let output = bmo(&dir).args(["next", "--json"]).output().unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["ok"], true);
    let issues = json["data"].as_array().unwrap();
    assert!(!issues.is_empty());
}

// ── Comments ──────────────────────────────────────────────────────────────────

#[test]
fn add_and_list_comment() {
    let dir = setup();
    bmo(&dir)
        .args(["issue", "create", "--title", "Comment target"])
        .assert()
        .success();

    bmo(&dir)
        .args(["issue", "comment", "add", "BMO-1", "--body", "Great work!"])
        .assert()
        .success();

    bmo(&dir)
        .args(["issue", "comment", "list", "BMO-1"])
        .assert()
        .success()
        .stdout(contains("Great work!"));
}

// ── Links ─────────────────────────────────────────────────────────────────────

#[test]
fn link_blocks_relation() {
    let dir = setup();
    bmo(&dir)
        .args(["issue", "create", "--title", "Blocker"])
        .assert()
        .success();
    bmo(&dir)
        .args(["issue", "create", "--title", "Blocked"])
        .assert()
        .success();

    bmo(&dir)
        .args(["issue", "link", "add", "BMO-1", "blocks", "BMO-2"])
        .assert()
        .success();

    // BMO-2 should not appear in next since it's blocked
    let output = bmo(&dir).args(["next", "--json"]).output().unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ids: Vec<i64> = json["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_i64().unwrap())
        .collect();
    assert!(!ids.contains(&2));
}

// ── Truncate ──────────────────────────────────────────────────────────────────

#[test]
fn truncate_empty_db_prints_nothing_to_delete() {
    let dir = setup();
    bmo(&dir)
        .args(["truncate", "--yes"])
        .assert()
        .success()
        .stdout(contains("Nothing to delete"));
}

#[test]
fn truncate_deletes_done_issues_by_default() {
    let dir = setup();
    // Create one done and one todo issue
    bmo(&dir)
        .args(["issue", "create", "--title", "Done issue"])
        .assert()
        .success();
    bmo(&dir)
        .args(["issue", "create", "--title", "Todo issue"])
        .assert()
        .success();
    bmo(&dir)
        .args(["issue", "close", "BMO-1"])
        .assert()
        .success();

    bmo(&dir)
        .args(["truncate", "--yes"])
        .assert()
        .success()
        .stdout(contains("Deleted 1"));

    // Done issue is gone, todo issue survives
    bmo(&dir)
        .args(["issue", "show", "BMO-1", "--json"])
        .assert()
        .failure();
    bmo(&dir)
        .args(["issue", "show", "BMO-2", "--json"])
        .assert()
        .success()
        .stdout(contains("Todo issue"));
}

#[test]
fn truncate_status_flag_targets_specified_status() {
    let dir = setup();
    bmo(&dir)
        .args(["issue", "create", "--title", "In progress issue"])
        .assert()
        .success();
    bmo(&dir)
        .args(["issue", "create", "--title", "Todo issue"])
        .assert()
        .success();
    bmo(&dir)
        .args(["issue", "move", "BMO-1", "--status", "in-progress"])
        .assert()
        .success();

    bmo(&dir)
        .args(["truncate", "--status", "in-progress", "--yes"])
        .assert()
        .success()
        .stdout(contains("Deleted 1"));

    bmo(&dir)
        .args(["issue", "show", "BMO-1", "--json"])
        .assert()
        .failure();
    bmo(&dir)
        .args(["issue", "show", "BMO-2", "--json"])
        .assert()
        .success();
}

#[test]
fn truncate_all_deletes_every_issue() {
    let dir = setup();
    bmo(&dir)
        .args(["issue", "create", "--title", "Issue A"])
        .assert()
        .success();
    bmo(&dir)
        .args(["issue", "create", "--title", "Issue B"])
        .assert()
        .success();
    bmo(&dir)
        .args(["issue", "close", "BMO-1"])
        .assert()
        .success();

    bmo(&dir)
        .args(["truncate", "--all", "--yes"])
        .assert()
        .success()
        .stdout(contains("Deleted 2"));

    let output = bmo(&dir).args(["stats", "--json"]).output().unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["data"]["total"], 0);
}

#[test]
fn truncate_json_output_envelope() {
    let dir = setup();
    bmo(&dir)
        .args(["issue", "create", "--title", "To delete"])
        .assert()
        .success();
    bmo(&dir)
        .args(["issue", "close", "BMO-1"])
        .assert()
        .success();

    let output = bmo(&dir)
        .args(["truncate", "--yes", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["deleted"], 1);
}

#[test]
fn truncate_nothing_to_delete_when_no_status_match() {
    let dir = setup();
    // Only todo issues exist; truncate (default: done) should find nothing
    bmo(&dir)
        .args(["issue", "create", "--title", "Todo only"])
        .assert()
        .success();

    bmo(&dir)
        .args(["truncate", "--yes"])
        .assert()
        .success()
        .stdout(contains("Nothing to delete"));

    // Issue was not touched
    bmo(&dir)
        .args(["issue", "show", "BMO-1", "--json"])
        .assert()
        .success();
}

#[test]
fn truncate_json_nothing_to_delete_returns_envelope() {
    let dir = setup();
    bmo(&dir)
        .args(["issue", "create", "--title", "Not done"])
        .assert()
        .success();

    let output = bmo(&dir)
        .args(["truncate", "--yes", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["deleted"], 0);
}

#[test]
fn truncate_all_conflicts_with_status() {
    let dir = setup();
    bmo(&dir)
        .args(["truncate", "--all", "--status", "done", "--yes"])
        .assert()
        .failure();
}

#[test]
fn truncate_confirmation_prompt_aborts_on_no() {
    let dir = setup();
    bmo(&dir)
        .args(["issue", "create", "--title", "Should survive"])
        .assert()
        .success();
    bmo(&dir)
        .args(["issue", "close", "BMO-1"])
        .assert()
        .success();

    // Send "n" to the confirmation prompt via assert_cmd::Command which supports write_stdin
    assert_cmd::Command::from_std(bmo_command())
        .current_dir(dir.path())
        .args(["truncate"])
        .write_stdin("n\n")
        .assert()
        .success();

    // Issue should still exist
    bmo(&dir)
        .args(["issue", "show", "BMO-1", "--json"])
        .assert()
        .success()
        .stdout(contains("Should survive"));
}

#[test]
fn truncate_status_flag_nothing_to_delete_when_no_match() {
    let dir = setup();
    // Only a backlog issue exists (default status); truncate --status done should find nothing
    bmo(&dir)
        .args(["issue", "create", "--title", "active work"])
        .assert()
        .success();

    bmo(&dir)
        .args(["truncate", "--status", "done", "--yes"])
        .assert()
        .success()
        .stdout(contains("Nothing to delete"));

    // The backlog issue was not touched
    bmo(&dir)
        .args(["issue", "show", "BMO-1", "--json"])
        .assert()
        .success();
}

#[test]
fn truncate_backlog_status_nothing_to_delete() {
    let dir = setup();
    // Create a todo issue explicitly; truncate --status backlog should find nothing
    bmo(&dir)
        .args([
            "issue",
            "create",
            "--title",
            "active work",
            "--status",
            "todo",
        ])
        .assert()
        .success();

    bmo(&dir)
        .args(["truncate", "--status", "backlog", "--yes"])
        .assert()
        .success()
        .stdout(contains("Nothing to delete"));

    // The todo issue was not touched
    bmo(&dir)
        .args(["issue", "show", "BMO-1", "--json"])
        .assert()
        .success();
}

// ── Plan ─────────────────────────────────────────────────────────────────────

#[test]
fn plan_outputs_phases() {
    let dir = setup();
    bmo(&dir)
        .args(["issue", "create", "--title", "Phase issue"])
        .assert()
        .success();

    bmo(&dir).args(["plan"]).assert().success();
}
