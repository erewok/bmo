use assert_cmd::cargo;
use assert_cmd::prelude::*;
use rusqlite::Connection;
use std::process::Command;
use tempfile::TempDir;

const BLOCKED_BY_HEADING: &str = "  ← blocked by:";
const BLOCKS_HEADING: &str = "  → blocks:";

fn setup() -> TempDir {
    let dir = TempDir::new().unwrap();
    bmo(&dir).arg("init").assert().success();
    dir
}

fn bmo(dir: &TempDir) -> Command {
    let mut cmd = Command::new(cargo::cargo_bin!("bmo"));
    cmd.current_dir(dir.path()).env_remove("BMO_DB");
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

/// Writes a relation of an issue to itself, which `link add` rejects.
fn inject_self_relation(dir: &TempDir, issue_id: i64, kind: &str) {
    let conn = Connection::open(dir.path().join(".bmo/issues.db")).unwrap();
    conn.execute(
        "INSERT INTO issue_relations (from_id, to_id, relation) VALUES (?1, ?1, ?2)",
        rusqlite::params![issue_id, kind],
    )
    .unwrap();
}

fn stdout_of(dir: &TempDir, args: &[&str]) -> String {
    let output = bmo(dir).args(args).output().unwrap();
    assert!(output.status.success(), "bmo {args:?} failed: {output:?}");
    String::from_utf8(output.stdout).unwrap()
}

fn graph_lines(dir: &TempDir, issue: &str) -> Vec<String> {
    stdout_of(dir, &["graph", issue])
        .lines()
        .map(str::to_string)
        .collect()
}

fn graph_json(dir: &TempDir, issue: &str) -> serde_json::Value {
    serde_json::from_str(&stdout_of(dir, &["graph", issue, "--json"])).unwrap()
}

/// Links `BMO-1 <kind> BMO-2` and returns the graph output of both endpoints.
fn graphs_of_both_endpoints(kind: &str) -> (Vec<String>, Vec<String>) {
    let dir = setup_with_issues(&["First", "Second"]);
    link(&dir, "BMO-1", kind, "BMO-2");
    (graph_lines(&dir, "BMO-1"), graph_lines(&dir, "BMO-2"))
}

fn first_blocked_by_second() -> Vec<&'static str> {
    vec!["BMO-1 — First", BLOCKED_BY_HEADING, "      BMO-2 — Second"]
}

fn first_blocks_second() -> Vec<&'static str> {
    vec!["BMO-1 — First", BLOCKS_HEADING, "      BMO-2 — Second"]
}

fn second_blocked_by_first() -> Vec<&'static str> {
    vec!["BMO-2 — Second", BLOCKED_BY_HEADING, "      BMO-1 — First"]
}

fn second_blocks_first() -> Vec<&'static str> {
    vec!["BMO-2 — Second", BLOCKS_HEADING, "      BMO-1 — First"]
}

#[test]
fn blocks_link_shows_target_under_blocks_from_the_source() {
    let (first, _) = graphs_of_both_endpoints("blocks");
    assert_eq!(first, first_blocks_second());
}

#[test]
fn blocks_link_shows_source_under_blocked_by_from_the_target() {
    let (_, second) = graphs_of_both_endpoints("blocks");
    assert_eq!(second, second_blocked_by_first());
}

#[test]
fn blocked_by_link_shows_target_under_blocked_by_from_the_source() {
    let (first, _) = graphs_of_both_endpoints("blocked-by");
    assert_eq!(first, first_blocked_by_second());
}

#[test]
fn blocked_by_link_shows_source_under_blocks_from_the_target() {
    let (_, second) = graphs_of_both_endpoints("blocked-by");
    assert_eq!(second, second_blocks_first());
}

#[test]
fn depends_on_link_shows_target_under_blocked_by_from_the_source() {
    let (first, _) = graphs_of_both_endpoints("depends-on");
    assert_eq!(first, first_blocked_by_second());
}

#[test]
fn depends_on_link_shows_source_under_blocks_from_the_target() {
    let (_, second) = graphs_of_both_endpoints("depends-on");
    assert_eq!(second, second_blocks_first());
}

#[test]
fn dependency_of_link_shows_target_under_blocks_from_the_source() {
    let (first, _) = graphs_of_both_endpoints("dependency-of");
    assert_eq!(first, first_blocks_second());
}

#[test]
fn dependency_of_link_shows_source_under_blocked_by_from_the_target() {
    let (_, second) = graphs_of_both_endpoints("dependency-of");
    assert_eq!(second, second_blocked_by_first());
}

#[test]
fn issue_with_only_informational_links_has_no_blocking_relations() {
    let dir = setup_with_issues(&["First", "Second", "Third", "Fourth"]);
    link(&dir, "BMO-1", "relates-to", "BMO-2");
    link(&dir, "BMO-1", "duplicates", "BMO-3");
    link(&dir, "BMO-1", "duplicate-of", "BMO-4");

    assert_eq!(
        graph_lines(&dir, "BMO-1"),
        ["BMO-1 — First", "  (no blocking relations)"]
    );
    for (issue, title) in [("BMO-2", "Second"), ("BMO-3", "Third"), ("BMO-4", "Fourth")] {
        assert_eq!(
            graph_lines(&dir, issue),
            [
                format!("{issue} — {title}"),
                "  (no blocking relations)".to_string()
            ]
        );
    }
}

#[test]
fn informational_links_are_left_out_alongside_directional_ones() {
    let dir = setup_with_issues(&["First", "Second", "Third"]);
    link(&dir, "BMO-1", "depends-on", "BMO-2");
    link(&dir, "BMO-1", "relates-to", "BMO-3");

    assert_eq!(graph_lines(&dir, "BMO-1"), first_blocked_by_second());
}

#[test]
fn edge_declared_by_two_equivalent_links_is_listed_once() {
    let dir = setup_with_issues(&["First", "Second"]);
    link(&dir, "BMO-1", "blocks", "BMO-2");
    link(&dir, "BMO-2", "depends-on", "BMO-1");

    assert_eq!(graph_lines(&dir, "BMO-1"), first_blocks_second());
    assert_eq!(graph_lines(&dir, "BMO-2"), second_blocked_by_first());
}

#[test]
fn blockers_and_blocked_issues_are_listed_in_separate_groups() {
    let dir = setup_with_issues(&["First", "Second", "Third", "Fourth", "Fifth"]);
    link(&dir, "BMO-1", "depends-on", "BMO-2");
    link(&dir, "BMO-3", "blocks", "BMO-1");
    link(&dir, "BMO-1", "dependency-of", "BMO-4");
    link(&dir, "BMO-5", "blocked-by", "BMO-1");

    assert_eq!(
        graph_lines(&dir, "BMO-1"),
        [
            "BMO-1 — First",
            BLOCKED_BY_HEADING,
            "      BMO-2 — Second",
            "      BMO-3 — Third",
            BLOCKS_HEADING,
            "      BMO-4 — Fourth",
            "      BMO-5 — Fifth",
        ]
    );
}

#[test]
fn directional_self_relation_lists_the_issue_in_both_groups() {
    for kind in ["blocks", "blocked-by", "depends-on", "dependency-of"] {
        let dir = setup_with_issues(&["First"]);
        inject_self_relation(&dir, 1, kind);

        assert_eq!(
            graph_lines(&dir, "BMO-1"),
            [
                "BMO-1 — First",
                BLOCKED_BY_HEADING,
                "      BMO-1 — First",
                BLOCKS_HEADING,
                "      BMO-1 — First",
            ],
            "self-relation of kind {kind}"
        );
    }
}

#[test]
fn informational_self_relation_has_no_blocking_relations() {
    for kind in ["relates-to", "duplicates", "duplicate-of"] {
        let dir = setup_with_issues(&["First"]);
        inject_self_relation(&dir, 1, kind);

        assert_eq!(
            graph_lines(&dir, "BMO-1"),
            ["BMO-1 — First", "  (no blocking relations)"],
            "self-relation of kind {kind}"
        );
    }
}

#[test]
fn self_relation_is_listed_once_per_group_alongside_other_issues() {
    let dir = setup_with_issues(&["First", "Second"]);
    link(&dir, "BMO-1", "depends-on", "BMO-2");
    inject_self_relation(&dir, 1, "blocks");
    inject_self_relation(&dir, 1, "depends-on");

    assert_eq!(
        graph_lines(&dir, "BMO-1"),
        [
            "BMO-1 — First",
            BLOCKED_BY_HEADING,
            "      BMO-2 — Second",
            "      BMO-1 — First",
            BLOCKS_HEADING,
            "      BMO-1 — First",
        ]
    );
    assert_eq!(graph_lines(&dir, "BMO-2"), second_blocks_first());
}

#[test]
fn json_returns_the_stored_depends_on_row_from_either_endpoint() {
    let dir = setup_with_issues(&["First", "Second"]);
    link(&dir, "BMO-1", "depends-on", "BMO-2");
    let stored_rows = serde_json::json!([
        {"id": 1, "from_id": 1, "to_id": 2, "kind": "depends-on"}
    ]);

    for (issue, id) in [("BMO-1", 1), ("BMO-2", 2)] {
        let envelope = graph_json(&dir, issue);
        assert_eq!(envelope["ok"], true);
        assert_eq!(envelope["data"]["issue"]["id"], id);
        assert_eq!(envelope["data"]["relations"], stored_rows);
    }
}
