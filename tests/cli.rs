use assert_cmd::cargo;
use assert_cmd::prelude::*;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use std::process::Command;
use tempfile::TempDir;

fn setup() -> TempDir {
    let dir = TempDir::new().unwrap();
    Command::new(cargo::cargo_bin!("bmo"))
        .current_dir(dir.path())
        .arg("init")
        .assert()
        .success();
    dir
}

fn bmo(dir: &TempDir) -> Command {
    let mut cmd = Command::new(cargo::cargo_bin!("bmo"));
    cmd.current_dir(dir.path());
    cmd
}

// ── Version ───────────────────────────────────────────────────────────────────

#[test]
fn version_prints_version() {
    let version = env!("CARGO_PKG_VERSION");
    Command::new(cargo::cargo_bin!("bmo"))
        .arg("version")
        .assert()
        .success()
        .stdout(contains(version));
}

// ── Init ──────────────────────────────────────────────────────────────────────

#[test]
fn init_creates_bmo_dir() {
    let dir = TempDir::new().unwrap();
    Command::new(cargo::cargo_bin!("bmo"))
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
    assert_cmd::Command::cargo_bin("bmo")
        .unwrap()
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
