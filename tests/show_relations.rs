use assert_cmd::cargo;
use assert_cmd::prelude::*;
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

fn setup_with_issues(titles: &[&str]) -> TempDir {
    let dir = setup();
    for title in titles {
        bmo(&dir)
            .args(["issue", "create", "--title", title])
            .assert()
            .success();
    }
    dir
}

fn link(dir: &TempDir, from: &str, kind: &str, to: &str) {
    bmo(dir)
        .args(["issue", "link", "add", from, kind, to])
        .assert()
        .success();
}

fn stdout_of(dir: &TempDir, args: &[&str]) -> String {
    let output = bmo(dir).args(args).output().unwrap();
    assert!(output.status.success(), "bmo {args:?} failed: {output:?}");
    String::from_utf8(output.stdout).unwrap()
}

/// The lines printed under the `Relations:` heading of `bmo show <issue>`.
fn shown_relation_lines(dir: &TempDir, issue: &str) -> Vec<String> {
    stdout_of(dir, &["issue", "show", issue])
        .lines()
        .skip_while(|line| line.trim() != "Relations:")
        .skip(1)
        .take_while(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
        .collect()
}

/// The header and body rows of the table printed by `bmo issue link list <issue>`.
fn link_list_table_rows(dir: &TempDir, issue: &str) -> Vec<Vec<String>> {
    stdout_of(dir, &["issue", "link", "list", issue])
        .lines()
        .map(|line| {
            line.split(['│', '┆'])
                .map(str::trim)
                .filter(|cell| !cell.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|cells| cells.len() == 4)
        .collect()
}

#[test]
fn show_on_the_to_side_names_the_other_issue_with_the_inverted_verb() {
    let cases = [
        ("blocks", "← blocked by BMO-1"),
        ("blocked-by", "→ blocks BMO-1"),
        ("depends-on", "← dependency of BMO-1"),
        ("dependency-of", "→ depends on BMO-1"),
        ("relates-to", "↔ relates to BMO-1"),
        ("duplicates", "← duplicate of BMO-1"),
        ("duplicate-of", "→ duplicates BMO-1"),
    ];
    for (stored_kind, expected_line) in cases {
        let dir = setup_with_issues(&["first", "second"]);
        link(&dir, "BMO-1", stored_kind, "BMO-2");

        let lines = shown_relation_lines(&dir, "BMO-2");

        assert_eq!(lines, [expected_line], "stored kind {stored_kind}");
        assert!(
            lines.iter().all(|line| !line.contains("BMO-2")),
            "stored kind {stored_kind}: shown issue named in its own relations: {lines:?}"
        );
    }
}

#[test]
fn show_on_the_from_side_keeps_the_stored_verb_and_names_the_to_issue() {
    let cases = [
        ("blocks", "→ blocks BMO-2"),
        ("blocked-by", "← blocked by BMO-2"),
        ("depends-on", "→ depends on BMO-2"),
        ("dependency-of", "← dependency of BMO-2"),
        ("relates-to", "↔ relates to BMO-2"),
        ("duplicates", "→ duplicates BMO-2"),
        ("duplicate-of", "← duplicate of BMO-2"),
    ];
    for (stored_kind, expected_line) in cases {
        let dir = setup_with_issues(&["first", "second"]);
        link(&dir, "BMO-1", stored_kind, "BMO-2");

        assert_eq!(
            shown_relation_lines(&dir, "BMO-1"),
            [expected_line],
            "stored kind {stored_kind}"
        );
    }
}

#[test]
fn show_renders_each_relation_from_the_shown_issue() {
    let dir = setup_with_issues(&["first", "second", "third"]);
    link(&dir, "BMO-1", "blocks", "BMO-2");
    link(&dir, "BMO-3", "blocked-by", "BMO-2");

    assert_eq!(
        shown_relation_lines(&dir, "BMO-2"),
        ["← blocked by BMO-1", "→ blocks BMO-3"]
    );
    assert_eq!(shown_relation_lines(&dir, "BMO-1"), ["→ blocks BMO-2"]);
    assert_eq!(shown_relation_lines(&dir, "BMO-3"), ["← blocked by BMO-2"]);
}

#[test]
fn show_json_returns_relations_as_stored() {
    let dir = setup_with_issues(&["first", "second", "third"]);
    link(&dir, "BMO-1", "blocks", "BMO-2");
    link(&dir, "BMO-3", "blocked-by", "BMO-2");

    let json: serde_json::Value =
        serde_json::from_str(&stdout_of(&dir, &["issue", "show", "BMO-2", "--json"])).unwrap();

    assert_eq!(
        json["data"]["relations"],
        serde_json::json!([
            {"id": 1, "from_id": 1, "to_id": 2, "kind": "blocks"},
            {"id": 2, "from_id": 3, "to_id": 2, "kind": "blocked-by"},
        ])
    );
}

#[test]
fn link_list_table_shows_relations_as_stored_whichever_side_is_listed() {
    let dir = setup_with_issues(&["first", "second", "third"]);
    link(&dir, "BMO-1", "blocks", "BMO-2");
    link(&dir, "BMO-3", "blocked-by", "BMO-2");

    let header = ["ID", "From", "Relation", "To"];
    let first_blocks_second = ["1", "BMO-1", "blocks", "BMO-2"];
    let third_blocked_by_second = ["2", "BMO-3", "blocked-by", "BMO-2"];

    assert_eq!(
        link_list_table_rows(&dir, "BMO-2"),
        [header, first_blocks_second, third_blocked_by_second]
    );
    assert_eq!(
        link_list_table_rows(&dir, "BMO-1"),
        [header, first_blocks_second]
    );
    assert_eq!(
        link_list_table_rows(&dir, "BMO-3"),
        [header, third_blocked_by_second]
    );
}

#[test]
fn link_list_json_returns_relations_as_stored() {
    let dir = setup_with_issues(&["first", "second"]);
    link(&dir, "BMO-1", "blocks", "BMO-2");

    let json: serde_json::Value = serde_json::from_str(&stdout_of(
        &dir,
        &["issue", "link", "list", "BMO-2", "--json"],
    ))
    .unwrap();

    assert_eq!(
        json["data"],
        serde_json::json!([{"id": 1, "from_id": 1, "to_id": 2, "kind": "blocks"}])
    );
}
