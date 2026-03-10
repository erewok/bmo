use std::path::Path;

use assert_cmd::Command;
use predicates::str::contains;
use tempfile::TempDir;

/// Initialize a fresh bmo project in a temp directory and return the dir handle.
fn setup() -> TempDir {
    let dir = TempDir::new().unwrap();
    Command::cargo_bin("bmo")
        .unwrap()
        .current_dir(dir.path())
        .arg("init")
        .assert()
        .success();
    dir
}

fn bmo(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("bmo").unwrap();
    cmd.current_dir(dir.path());
    cmd
}

// ── Import from fixture ───────────────────────────────────────────────────────

#[test]
fn import_fixture_creates_correct_issue_count() {
    let dir = setup();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample-export.json");

    bmo(&dir)
        .args(["import", fixture.to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("3 issue(s)"))
        .stdout(contains("1 comment(s)"));
}

#[test]
fn import_fixture_titles_are_preserved() {
    let dir = setup();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample-export.json");

    bmo(&dir)
        .args(["import", fixture.to_str().unwrap()])
        .assert()
        .success();

    // issue list excludes done by default; filter by status to check each one
    let output = bmo(&dir)
        .args(["issue", "list", "--status", "done", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let done_titles: Vec<&str> = json["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["title"].as_str().unwrap())
        .collect();
    assert!(done_titles.contains(&"Fix login bug"));

    let output = bmo(&dir)
        .args(["issue", "list", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let titles: Vec<&str> = json["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["title"].as_str().unwrap())
        .collect();
    assert!(titles.contains(&"Add dark mode"));
    assert!(titles.contains(&"Write onboarding docs"));
}

#[test]
fn import_fixture_with_from_docket_flag() {
    let dir = setup();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample-export.json");

    bmo(&dir)
        .args(["import", "--from-docket", fixture.to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("from docket format"));
}

// ── Export / import round-trip ────────────────────────────────────────────────

#[test]
fn export_import_roundtrip() {
    let dir = setup();

    // Create a couple of issues
    bmo(&dir)
        .args([
            "issue",
            "create",
            "--title",
            "Round-trip issue",
            "--priority",
            "high",
            "--kind",
            "bug",
        ])
        .assert()
        .success();

    bmo(&dir)
        .args([
            "issue",
            "create",
            "--title",
            "Second issue",
            "--priority",
            "low",
            "--kind",
            "task",
            "--assignee",
            "carol",
        ])
        .assert()
        .success();

    // Export
    let export_path = dir.path().join("export.json");
    bmo(&dir)
        .args(["export", "--output", export_path.to_str().unwrap()])
        .assert()
        .success();

    assert!(export_path.exists());

    let contents = std::fs::read_to_string(&export_path).unwrap();
    let bundle: serde_json::Value = serde_json::from_str(&contents).unwrap();
    assert_eq!(bundle["issues"].as_array().unwrap().len(), 2);
    assert_eq!(bundle["schema_version"], 1);

    // Import into a fresh project
    let dir2 = setup();
    bmo(&dir2)
        .args(["import", export_path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("2 issue(s)"));

    // Verify data integrity
    let output = bmo(&dir2)
        .args(["issue", "list", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let issues = json["data"].as_array().unwrap();
    assert_eq!(issues.len(), 2);

    let titles: Vec<&str> = issues
        .iter()
        .map(|i| i["title"].as_str().unwrap())
        .collect();
    assert!(titles.contains(&"Round-trip issue"));
    assert!(titles.contains(&"Second issue"));
}

#[test]
fn export_json_has_required_fields() {
    let dir = setup();
    bmo(&dir)
        .args(["issue", "create", "--title", "Schema check"])
        .assert()
        .success();

    let output = bmo(&dir).args(["export"]).output().unwrap();
    let bundle: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

    assert!(bundle["schema_version"].is_number());
    assert!(bundle["exported_at"].is_string());
    assert!(bundle["project_name"].is_string());
    assert!(bundle["issues"].is_array());
    assert!(bundle["comments"].is_array());
    assert!(bundle["labels"].is_array());
    assert!(bundle["relations"].is_array());
}

// ── Docket migration tests ────────────────────────────────────────────────────

#[test]
fn from_docket_import_exits_zero() {
    let dir = setup();
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/docket-export-sample.json");
    bmo(&dir)
        .args(["import", "--from-docket", fixture.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn from_docket_import_correct_issue_count() {
    let dir = setup();
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/docket-export-sample.json");
    bmo(&dir)
        .args(["import", "--from-docket", fixture.to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("4 issue(s)"))
        .stdout(contains("2 comment(s)"));
}

#[test]
fn from_docket_import_issues_use_bmo_prefix() {
    let dir = setup();
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/docket-export-sample.json");
    bmo(&dir)
        .args(["import", "--from-docket", fixture.to_str().unwrap()])
        .assert()
        .success();

    // All issues should be queryable; ids are sequential integers (BMO-1..BMO-4)
    let output = bmo(&dir)
        .args(["issue", "list", "--all", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let issues = json["data"].as_array().unwrap();
    assert_eq!(issues.len(), 4);
    // Verify each issue can be retrieved via BMO- prefixed ID
    for issue in issues {
        let numeric_id = issue["id"].as_i64().unwrap();
        let bmo_id = format!("BMO-{numeric_id}");
        bmo(&dir)
            .args(["issue", "show", &bmo_id])
            .assert()
            .success();
    }
}

#[test]
fn from_docket_import_titles_preserved() {
    let dir = setup();
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/docket-export-sample.json");
    bmo(&dir)
        .args(["import", "--from-docket", fixture.to_str().unwrap()])
        .assert()
        .success();

    let output = bmo(&dir)
        .args(["issue", "list", "--all", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let titles: Vec<&str> = json["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["title"].as_str().unwrap())
        .collect();
    assert!(titles.contains(&"Project Alpha"));
    assert!(titles.contains(&"Set up database"));
    assert!(titles.contains(&"Build API endpoints"));
    assert!(titles.contains(&"Write integration tests"));
}

#[test]
fn from_docket_import_relations_imported() {
    let dir = setup();
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/docket-export-sample.json");
    bmo(&dir)
        .args(["import", "--from-docket", fixture.to_str().unwrap()])
        .assert()
        .success();

    // BMO-2 should be the "Set up database" issue (second imported);
    // check its relations via issue show --json
    let output = bmo(&dir)
        .args(["issue", "show", "BMO-2", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    // The relations array should be non-empty
    let relations = json["data"]["relations"].as_array().unwrap();
    assert!(!relations.is_empty(), "Expected relations on BMO-2");
}
