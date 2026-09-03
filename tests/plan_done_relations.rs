// Regression tests for the planner crashing on relations that reference a
// `done` issue.
//
// `plan`, `next`, and `agent-init` build their graph from a status-filtered
// issue list (which excludes `done`) but an unfiltered relation list. A
// relation whose endpoint is `done` therefore used to contribute an id with no
// corresponding node, which either panicked the topological sort or inflated a
// node's in-degree so it was never scheduled and was misreported as a cycle.
//
// A satisfied prerequisite imposes no ordering, so the remaining issues must
// plan exactly as if the relation were not there.

use assert_cmd::cargo;
use assert_cmd::prelude::*;
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

fn create_issues(dir: &TempDir, n: usize) -> Vec<String> {
    (1..=n)
        .map(|i| {
            bmo(dir)
                .args(["issue", "create", "--title", &format!("Issue {i}")])
                .assert()
                .success();
            format!("BMO-{i}")
        })
        .collect()
}

fn link(dir: &TempDir, from: &str, rel: &str, to: &str) {
    bmo(dir)
        .args(["issue", "link", "add", from, rel, to])
        .assert()
        .success();
}

fn close(dir: &TempDir, id: &str) {
    bmo(dir).args(["close", id]).assert().success();
}

/// All three planner commands must succeed on this graph.
fn assert_planner_commands_succeed(dir: &TempDir) {
    bmo(dir).args(["plan"]).assert().success();
    bmo(dir).args(["next"]).assert().success();
    bmo(dir).args(["agent-init"]).assert().success();
}

// ── The two dangling-edge shapes ─────────────────────────────────────────────

// A live issue with a forward edge into a done issue. This is the shape that
// panicked outright at `topo.rs:35` with `Option::unwrap()` on a `None` value.
#[test]
fn live_issue_blocking_a_done_issue_does_not_panic() {
    let dir = setup();
    create_issues(&dir, 2);
    link(&dir, "BMO-1", "blocks", "BMO-2");
    close(&dir, "BMO-2");

    assert_planner_commands_succeed(&dir);

    // BMO-1 is the only issue left, so it is the whole plan.
    bmo(&dir)
        .args(["plan"])
        .assert()
        .success()
        .stdout(contains("BMO-1"));
}

// A live issue with a reverse edge from a done issue. This is the shape that
// produced a phantom "cycle detected" on a provably acyclic graph.
#[test]
fn live_issue_blocked_by_a_done_issue_is_not_a_cycle() {
    let dir = setup();
    create_issues(&dir, 2);
    link(&dir, "BMO-1", "blocks", "BMO-2");
    close(&dir, "BMO-1");

    assert_planner_commands_succeed(&dir);

    // The blocker is satisfied, so BMO-2 is ready now — not a cycle.
    bmo(&dir)
        .args(["next"])
        .assert()
        .success()
        .stdout(contains("BMO-2"));
}

// The `depends-on` relation reaches the same code path with its endpoints
// reversed, so it needs its own coverage.
#[test]
fn depends_on_a_done_issue_is_not_a_cycle() {
    let dir = setup();
    create_issues(&dir, 2);
    link(&dir, "BMO-2", "depends-on", "BMO-1");
    close(&dir, "BMO-1");

    assert_planner_commands_succeed(&dir);

    bmo(&dir)
        .args(["next"])
        .assert()
        .success()
        .stdout(contains("BMO-2"));
}

// ── Plan shape after a prerequisite closes ───────────────────────────────────

// Closing the head of a chain must shorten the plan rather than break it: a
// satisfied prerequisite imposes no ordering on what remains.
#[test]
fn closing_a_prerequisite_shortens_the_plan() {
    let dir = setup();
    let ids = create_issues(&dir, 3);
    link(&dir, &ids[0], "blocks", &ids[1]);
    link(&dir, &ids[1], "blocks", &ids[2]);

    bmo(&dir)
        .args(["plan"])
        .assert()
        .success()
        .stdout(contains("3 phases"));

    close(&dir, &ids[0]);

    bmo(&dir)
        .args(["plan"])
        .assert()
        .success()
        .stdout(contains("2 phases"));

    // And BMO-2, whose only blocker is now done, is immediately actionable.
    bmo(&dir)
        .args(["next"])
        .assert()
        .success()
        .stdout(contains("BMO-2"));
}

// Closing every issue that carries a relation must leave a working, empty plan
// rather than a crash.
#[test]
fn plan_works_when_all_linked_issues_are_done() {
    let dir = setup();
    let ids = create_issues(&dir, 2);
    link(&dir, &ids[0], "blocks", &ids[1]);
    close(&dir, &ids[0]);
    close(&dir, &ids[1]);

    assert_planner_commands_succeed(&dir);
}

// A real cycle among live issues must still be reported after all of the above.
#[test]
fn genuine_cycles_are_still_detected() {
    let dir = setup();
    let ids = create_issues(&dir, 3);
    link(&dir, &ids[0], "blocks", &ids[1]);
    link(&dir, &ids[1], "blocks", &ids[2]);

    // Inject the closing edge directly; the CLI guard would reject it.
    let conn = rusqlite::Connection::open(dir.path().join(".bmo/issues.db")).unwrap();
    conn.execute(
        "INSERT INTO issue_relations (from_id, to_id, relation) VALUES (3, 1, 'blocks')",
        [],
    )
    .unwrap();
    drop(conn);

    bmo(&dir)
        .args(["plan"])
        .assert()
        .failure()
        .stderr(contains("cycle"));
}
